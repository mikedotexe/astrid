use std::fs::{self, File};

use serde::Deserialize;
use serde_json::Value;

use crate::attestation::HmacSigner;
use crate::candidate::{ActiveDraft, CandidateManager};
use crate::config::Config;
use crate::reporting::patch_export_write;
use crate::util::{
    atomic_private_write, canonical_json, read_stable_regular, sha256, unix_seconds,
    validate_hex64, validate_identifier,
};
use crate::{Error, Result};

const STATUS_SCHEMA: &str = "astrid.edge_self_change.steward_status.v1";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SupervisorStatus {
    schema: String,
    appliance_id: String,
    generated_at: u64,
    current_generation: String,
    supervisor_mode: String,
    pipeline_busy: bool,
    candidate: Option<CandidateStatus>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateStatus {
    candidate_id: String,
    candidate_sha256: String,
    status: LifecycleStatus,
    #[serde(default)]
    terminal_reason_sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum LifecycleStatus {
    IntentPending,
    Building,
    Staged,
    Probation,
    Accepted,
    Rejected,
    RolledBack,
    Abandoned,
}

impl LifecycleStatus {
    const fn terminal(self) -> bool {
        matches!(
            self,
            Self::Accepted | Self::Rejected | Self::RolledBack | Self::Abandoned
        )
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::IntentPending => "intent_pending",
            Self::Building => "building",
            Self::Staged => "staged",
            Self::Probation => "probation",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::RolledBack => "rolled_back",
            Self::Abandoned => "abandoned",
        }
    }
}

#[derive(Debug, Clone)]
pub enum LifecycleCheck {
    Ready,
    Reconciled { candidate_id: String },
    Deferred { reason: String },
}

/// Reconcile terminal submitted candidates before checking whether a reflection is due.
#[allow(clippy::too_many_lines)] // One auditable archive/export/clear transaction boundary.
pub fn reconcile(config: &Config) -> Result<LifecycleCheck> {
    let bytes = read_stable_regular(&config.supervisor_status, 64 * 1024)?;
    let status: SupervisorStatus = serde_json::from_slice(&bytes)?;
    validate_status(config, &status)?;
    if status.supervisor_mode == "rescue" {
        return Ok(LifecycleCheck::Deferred {
            reason: format!("immutable supervisor is {}", status.supervisor_mode),
        });
    }
    let signer = HmacSigner::from_file(&config.attestor_key)?;
    let state = config.state_root.join("candidate");
    let outbox = config.state_root.join("candidate-outbox");
    let manager =
        CandidateManager::new_reconciler(&state, &outbox, &signer, &config.current_generation)?;
    let mut active = manager.active()?;
    if let Some(ActiveDraft::Prepared(candidate)) = active.as_ref() {
        let projection = supervisor_projection(&status, candidate);
        match crate::publication::recover(config, &manager, &signer, candidate, projection)? {
            crate::publication::Recovery::RestoredEditing => {
                return Ok(LifecycleCheck::Ready);
            },
            crate::publication::Recovery::FinalizedSubmitted => {
                active = manager.active()?;
            },
            crate::publication::Recovery::NoTransaction => {
                return Err(Error::new(
                    "prepared candidate recovery returned an impossible empty transaction",
                ));
            },
        }
    }
    if let Some(ActiveDraft::Submitted(candidate)) = active.as_ref() {
        let _ = crate::publication::recover_submitted(config, &manager, &signer, candidate)?;
    }
    if config
        .state_root
        .join("reconciliation-pending.json")
        .exists()
    {
        return recover_pending(config, &manager, &signer, active);
    }
    if status.pipeline_busy {
        return Ok(LifecycleCheck::Deferred {
            reason: "supervisor candidate/build/probation transaction is nonterminal".to_owned(),
        });
    }
    let Some(ActiveDraft::Submitted(submitted)) = active else {
        return Ok(LifecycleCheck::Ready);
    };
    let Some(projected) = status.candidate else {
        return Ok(LifecycleCheck::Deferred {
            reason: "published candidate intent awaits immutable supervisor ingestion".to_owned(),
        });
    };
    if projected.candidate_id != submitted.candidate_id
        || projected.candidate_sha256 != submitted.candidate_sha256
    {
        return Err(Error::new(
            "submitted draft does not match supervisor lifecycle projection",
        ));
    }
    if !projected.status.terminal() {
        return Ok(LifecycleCheck::Deferred {
            reason: format!("submitted candidate remains {}", projected.status.as_str()),
        });
    }
    let archive = manager.archive_terminal(
        projected.status.as_str(),
        projected.terminal_reason_sha256.as_deref(),
    )?;
    let export_core = serde_json::json!({
        "schema": "astrid.edge.steward_helper.owner_patch_export.v1",
        "recorded_at": unix_seconds(),
        "appliance_id": config.appliance_id,
        "candidate_id": archive.candidate.candidate_id,
        "candidate_sha256": archive.candidate.candidate_sha256,
        "patch_sha256": archive.candidate.patch_sha256,
        "source_id": archive.source_id,
        "base_generation": archive.candidate.manifest.base_generation,
        "terminal_status": projected.status.as_str(),
        "terminal_reason_sha256": projected.terminal_reason_sha256,
        "patch": archive.patch,
        "authority": "owner_export_only_never_reingested_or_authorizing"
    });
    let core_bytes = canonical_json(&export_core)?;
    let export = serde_json::json!({
        "schema": "astrid.edge.steward_helper.owner_patch_export_envelope.v1",
        "core": export_core,
        "core_sha256": sha256(&core_bytes),
        "auth": {
            "algorithm": "hmac-sha256",
            "key_id": signer.key_id,
            "signature": signer.sign(&core_bytes)
        }
    });
    let export_bytes = canonical_json(&export)?;
    let export_name = format!(
        "candidate-change-{}-{}.json",
        archive.candidate.candidate_id, archive.candidate.candidate_sha256
    );
    let exported = patch_export_write(config, &export_name, &export_bytes)?;
    let summary_core = serde_json::json!({
        "schema": "astrid.edge.steward_helper.owner_patch_export_summary.v1",
        "recorded_at": unix_seconds(),
        "appliance_id": config.appliance_id,
        "candidate_id": archive.candidate.candidate_id,
        "candidate_sha256": archive.candidate.candidate_sha256,
        "patch_sha256": archive.candidate.patch_sha256,
        "source_id": archive.source_id,
        "base_generation": archive.candidate.manifest.base_generation,
        "terminal_status": projected.status.as_str(),
        "terminal_reason_sha256": projected.terminal_reason_sha256,
        "touched_paths": archive.touched_paths,
        "file_count": archive.candidate.manifest.changed_paths.len(),
        "added_lines": archive.added_lines,
        "removed_lines": archive.removed_lines,
        "changed_lines": archive.changed_lines,
        "full_export_sha256": exported.sha256,
        "source_bodies_retained": false,
        "authority": "reporting_summary_only_never_reingested_or_authorizing"
    });
    let summary_core_bytes = canonical_json(&summary_core)?;
    let summary = serde_json::json!({
        "schema": "astrid.edge.steward_helper.owner_patch_export_summary_envelope.v1",
        "core": summary_core,
        "core_sha256": sha256(&summary_core_bytes),
        "auth": {
            "algorithm": "hmac-sha256",
            "key_id": signer.key_id,
            "signature": signer.sign(&summary_core_bytes)
        }
    });
    let summary_bytes = canonical_json(&summary)?;
    if summary_bytes.len() > 16 * 1024 {
        return Err(Error::new("owner patch export summary exceeds 16 KiB"));
    }
    let summary_name = format!(
        "candidate-change-{}-{}.summary.json",
        archive.candidate.candidate_id, archive.candidate.candidate_sha256
    );
    let summary_exported = patch_export_write(config, &summary_name, &summary_bytes)?;
    let reconciliation_common = serde_json::json!({
        "schema": "astrid.edge.steward_helper.candidate_reconciliation.v1",
        "recorded_at": unix_seconds(),
        "appliance_id": config.appliance_id,
        "candidate_id": archive.candidate.candidate_id,
        "candidate_sha256": archive.candidate.candidate_sha256,
        "patch_sha256": archive.candidate.patch_sha256,
        "terminal_status": projected.status.as_str(),
        "terminal_reason_sha256": projected.terminal_reason_sha256,
        "supervisor_projection_sha256": sha256(&bytes),
        "history_root": archive.history_root,
        "export_path": exported.path,
        "export_uid": exported.uid,
        "export_gid": exported.gid,
        "export_mode": exported.mode,
        "export_sha256": exported.sha256,
        "summary_export_path": summary_exported.path,
        "summary_export_uid": summary_exported.uid,
        "summary_export_gid": summary_exported.gid,
        "summary_export_mode": summary_exported.mode,
        "summary_export_sha256": summary_exported.sha256
    });
    let mut prepared_core = reconciliation_common.clone();
    prepared_core["phase"] = serde_json::json!("prepared_before_active_clear");
    prepared_core["active_draft_cleared"] = serde_json::json!(false);
    let prepared_bytes = signed_reconciliation(&signer, &prepared_core)?;
    atomic_private_write(
        &archive.history_root.join("reconciliation-prepared.json"),
        &prepared_bytes,
    )?;
    atomic_private_write(
        &config.state_root.join("reconciliation-pending.json"),
        &prepared_bytes,
    )?;
    manager.clear_terminal(
        &archive.candidate.candidate_id,
        &archive.candidate.candidate_sha256,
    )?;
    let mut completed_core = reconciliation_common;
    completed_core["phase"] = serde_json::json!("completed_after_active_clear");
    completed_core["active_draft_cleared"] = serde_json::json!(true);
    let receipt_bytes = signed_reconciliation(&signer, &completed_core)?;
    let receipt_path = archive.history_root.join("reconciliation-receipt.json");
    atomic_private_write(&receipt_path, &receipt_bytes)?;
    fs::remove_file(config.state_root.join("reconciliation-pending.json"))?;
    File::open(&config.state_root)?.sync_all()?;
    Ok(LifecycleCheck::Reconciled {
        candidate_id: archive.candidate.candidate_id,
    })
}

fn supervisor_projection(
    status: &SupervisorStatus,
    candidate: &crate::candidate::SubmittedCandidate,
) -> crate::publication::SupervisorProjection {
    match status.candidate.as_ref() {
        Some(projected)
            if projected.candidate_id == candidate.candidate_id
                && projected.candidate_sha256 == candidate.candidate_sha256 =>
        {
            crate::publication::SupervisorProjection::Matching
        },
        None if !status.pipeline_busy => crate::publication::SupervisorProjection::Idle,
        _ => crate::publication::SupervisorProjection::BusyOrDifferent,
    }
}

fn recover_pending(
    config: &Config,
    manager: &CandidateManager<'_>,
    signer: &HmacSigner,
    active: Option<ActiveDraft>,
) -> Result<LifecycleCheck> {
    let pending_path = config.state_root.join("reconciliation-pending.json");
    let pending = read_stable_regular(&pending_path, 128 * 1024)?;
    let mut core = verify_reconciliation(signer, &pending)?;
    if core.get("phase").and_then(Value::as_str) != Some("prepared_before_active_clear")
        || core.get("active_draft_cleared").and_then(Value::as_bool) != Some(false)
    {
        return Err(Error::new("pending reconciliation phase is invalid"));
    }
    let candidate_id = core
        .get("candidate_id")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("pending reconciliation candidate_id is absent"))?
        .to_owned();
    let candidate_sha256 = core
        .get("candidate_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("pending reconciliation candidate hash is absent"))?
        .to_owned();
    validate_identifier(&candidate_id, "pending reconciliation candidate_id")?;
    validate_hex64(&candidate_sha256, "pending reconciliation candidate_sha256")?;
    match active {
        Some(ActiveDraft::Submitted(candidate))
            if candidate.candidate_id == candidate_id
                && candidate.candidate_sha256 == candidate_sha256 =>
        {
            manager.clear_terminal(&candidate_id, &candidate_sha256)?;
        },
        None => {},
        _ => {
            return Err(Error::new(
                "pending reconciliation does not match active candidate",
            ));
        },
    }
    core["phase"] = serde_json::json!("completed_after_active_clear");
    core["active_draft_cleared"] = serde_json::json!(true);
    let history_root = config
        .state_root
        .join("candidate-outbox/history")
        .join(format!("{candidate_id}-{candidate_sha256}"));
    atomic_private_write(
        &history_root.join("reconciliation-receipt.json"),
        &signed_reconciliation(signer, &core)?,
    )?;
    fs::remove_file(&pending_path)?;
    File::open(&config.state_root)?.sync_all()?;
    Ok(LifecycleCheck::Reconciled { candidate_id })
}

fn signed_reconciliation(signer: &HmacSigner, core: &serde_json::Value) -> Result<Vec<u8>> {
    let core_bytes = canonical_json(core)?;
    canonical_json(&serde_json::json!({
        "schema": "astrid.edge.steward_helper.candidate_reconciliation_envelope.v1",
        "core": core,
        "core_sha256": sha256(&core_bytes),
        "auth": {
            "algorithm": "hmac-sha256",
            "key_id": signer.key_id,
            "signature": signer.sign(&core_bytes)
        }
    }))
}

fn verify_reconciliation(signer: &HmacSigner, bytes: &[u8]) -> Result<Value> {
    let envelope: Value = serde_json::from_slice(bytes)?;
    let object = envelope
        .as_object()
        .ok_or_else(|| Error::new("pending reconciliation is not an object"))?;
    let keys = object
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    if keys != std::collections::BTreeSet::from(["auth", "core", "core_sha256", "schema"])
        || envelope.get("schema").and_then(Value::as_str)
            != Some("astrid.edge.steward_helper.candidate_reconciliation_envelope.v1")
    {
        return Err(Error::new("pending reconciliation envelope shape failed"));
    }
    let core = envelope
        .get("core")
        .cloned()
        .ok_or_else(|| Error::new("pending reconciliation core is absent"))?;
    let core_bytes = canonical_json(&core)?;
    let core_sha256 = sha256(&core_bytes);
    let auth = envelope
        .get("auth")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("pending reconciliation auth is absent"))?;
    if envelope.get("core_sha256").and_then(Value::as_str) != Some(core_sha256.as_str())
        || auth.get("algorithm").and_then(Value::as_str) != Some("hmac-sha256")
        || auth.get("key_id").and_then(Value::as_str) != Some(signer.key_id.as_str())
        || !auth
            .get("signature")
            .and_then(Value::as_str)
            .is_some_and(|signature| signer.verify(&core_bytes, signature))
    {
        return Err(Error::new("pending reconciliation authentication failed"));
    }
    Ok(core)
}

fn validate_status(config: &Config, status: &SupervisorStatus) -> Result<()> {
    let now = unix_seconds();
    if status.schema != STATUS_SCHEMA
        || status.appliance_id != config.appliance_id
        || !matches!(
            status.supervisor_mode.as_str(),
            "running" | "paused" | "rescue"
        )
        || status.generated_at > now.saturating_add(60)
        || now.saturating_sub(status.generated_at) > 20 * 60
    {
        return Err(Error::new(
            "supervisor status schema, identity, mode, or freshness mismatch",
        ));
    }
    validate_identifier(&status.current_generation, "supervisor current generation")?;
    let current = std::str::from_utf8(&read_stable_regular(&config.current_generation, 256)?)
        .map_err(|_| Error::new("current generation is not UTF-8"))?
        .trim()
        .to_owned();
    if current != status.current_generation {
        return Err(Error::new(
            "supervisor status and immutable generation binding disagree",
        ));
    }
    if let Some(candidate) = &status.candidate {
        validate_identifier(&candidate.candidate_id, "supervisor candidate_id")?;
        validate_hex64(&candidate.candidate_sha256, "supervisor candidate_sha256")?;
        if let Some(reason) = &candidate.terminal_reason_sha256 {
            validate_hex64(reason, "terminal_reason_sha256")?;
        }
        if matches!(status.supervisor_mode.as_str(), "running" | "paused")
            && status.pipeline_busy == candidate.status.terminal()
        {
            return Err(Error::new(
                "supervisor pipeline_busy conflicts with candidate lifecycle",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{LifecycleStatus, SupervisorStatus};

    #[test]
    fn supervisor_status_rejects_unknown_fields_and_knows_terminal_states() {
        let invalid = br#"{"schema":"astrid.edge_self_change.steward_status.v1","appliance_id":"a","generated_at":1,"current_generation":"g","supervisor_mode":"running","pipeline_busy":false,"candidate":null,"extra":true}"#;
        assert!(serde_json::from_slice::<SupervisorStatus>(invalid).is_err());
        assert!(LifecycleStatus::Accepted.terminal());
        assert!(!LifecycleStatus::Probation.terminal());
    }
}
