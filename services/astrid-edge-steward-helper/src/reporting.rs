use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde_json::Value;
use uuid::Uuid;

use crate::attestation::HmacSigner;
use crate::authored_transaction::SourceReviewOutcome;
use crate::config::Config;
use crate::context_provenance::ContextProvenance;
use crate::inquiry::{InquiryClassification, ProjectionReceipt};
use crate::util::{canonical_json, read_stable_regular, sha256};
use crate::{Error, Result};

const CONTINUITY_SCHEMA: &str = "astrid_edge_scheduled_introspection_continuity_v2";
const STATE_SCHEMA: &str = "astrid_edge_scheduled_introspection_state_v2";
const RECEIPT_SCHEMA: &str = "astrid_edge_scheduled_introspection_v2";
const AUTHORSHIP_CORE_SCHEMA: &str = "astrid.edge.scheduled_authorship.attestation.v2";
const AUTHORSHIP_ENVELOPE_SCHEMA: &str = "astrid.edge.scheduled_authorship.attestation_envelope.v2";

fn projection_root(config: &Config) -> PathBuf {
    config
        .workspace_root
        .join("runtime/scheduled-introspection/projection")
}

fn artifact_root(config: &Config) -> PathBuf {
    config.workspace_root.join("introspections/scheduled")
}

fn authorship_root(config: &Config) -> PathBuf {
    config.state_root.join("scheduled-authorship")
}

#[derive(Debug, Clone)]
pub struct ExportMetadata {
    pub path: String,
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
    pub sha256: String,
}

/// Atomically write one file in an installer-created steward output directory.
///
/// The helper deliberately never creates, chmods, or chowns either output
/// directory. The installer owns their ACL and bind-mount policy.
pub fn workspace_write(config: &Config, path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = checked_output_parent(config, path)?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        Uuid::new_v4()
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true).mode(0o640);
    let mut file = options.open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    validate_output_file(config, &fs::symlink_metadata(path)?, "scheduled output")?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

/// Create an immutable scheduled artifact once, or verify the exact prior bytes
/// while resuming a prepared authored transaction.
pub fn workspace_write_exact(config: &Config, path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() || path.is_symlink() {
        let metadata = fs::symlink_metadata(path)?;
        validate_output_file(config, &metadata, "scheduled immutable output")?;
        if read_stable_regular(path, 2 * 1024 * 1024)? != bytes {
            return Err(Error::new("scheduled immutable output collision"));
        }
        return Ok(());
    }
    workspace_write(config, path, bytes)
}

/// Append one record in the exact installer-created artifact directory.
pub fn workspace_append(config: &Config, path: &Path, bytes: &[u8]) -> Result<()> {
    let _parent = checked_output_parent(config, path)?;
    let mut options = OpenOptions::new();
    options.append(true).create(true).mode(0o640);
    #[cfg(target_os = "linux")]
    options.custom_flags(0o00_400_000);
    let mut file = options.open(path)?;
    let metadata = file.metadata()?;
    validate_output_file(config, &metadata, "scheduled introspection ledger")?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn checked_output_parent<'a>(config: &Config, path: &'a Path) -> Result<&'a Path> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("scheduled output has no parent"))?;
    if parent != projection_root(config) && parent != artifact_root(config) {
        return Err(Error::new(
            "scheduled output escaped the two dedicated writable directories",
        ));
    }
    let metadata = fs::symlink_metadata(parent)?;
    let steward_uid = fs::symlink_metadata(&config.state_root)?.uid();
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != steward_uid
        || metadata.gid() != config.workspace_gid
        || metadata.permissions().mode() & 0o777 != 0o750
    {
        return Err(Error::new(
            "scheduled output directory must be steward:runtime mode 0750",
        ));
    }
    Ok(parent)
}

fn validate_output_file(config: &Config, metadata: &std::fs::Metadata, label: &str) -> Result<()> {
    let steward_uid = fs::symlink_metadata(&config.state_root)?.uid();
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != steward_uid
        || metadata.gid() != config.workspace_gid
        || metadata.permissions().mode() & 0o777 != 0o640
    {
        return Err(Error::new(format!(
            "{label} must be steward:runtime mode 0640, regular, and single-linked"
        )));
    }
    Ok(())
}

/// Create one immutable, digest-named owner-visible export without overwriting.
pub fn patch_export_write(config: &Config, filename: &str, bytes: &[u8]) -> Result<ExportMetadata> {
    let parent = &config.patch_export_root;
    let steward_uid = fs::symlink_metadata(&config.state_root)?.uid();
    let parent_metadata = fs::symlink_metadata(parent)?;
    if !parent_metadata.is_dir()
        || parent_metadata.file_type().is_symlink()
        || parent_metadata.uid() != steward_uid
        || parent_metadata.gid() != config.workspace_gid
        || parent_metadata.permissions().mode() & 0o777 != 0o750
    {
        return Err(Error::new(
            "patch export directory must be steward:runtime mode 0750",
        ));
    }
    if filename.is_empty()
        || filename.len() > 240
        || !filename
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(Error::new("invalid patch export filename"));
    }
    let path = parent.join(filename);
    if path.exists() {
        if read_stable_regular(&path, 16 * 1024 * 1024)? != bytes {
            return Err(Error::new("patch export collision"));
        }
    } else {
        let partial = parent.join(format!(".{filename}.{}.partial", Uuid::new_v4()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true).mode(0o640);
        let mut file = options.open(&partial)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        match fs::hard_link(&partial, &path) {
            Ok(()) => {},
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if read_stable_regular(&path, 16 * 1024 * 1024)? != bytes {
                    let _ = fs::remove_file(&partial);
                    return Err(Error::new("patch export collision"));
                }
            },
            Err(error) => {
                let _ = fs::remove_file(&partial);
                return Err(error.into());
            },
        }
        fs::remove_file(&partial)?;
        File::open(parent)?.sync_all()?;
    }
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != steward_uid
        || metadata.gid() != config.workspace_gid
        || metadata.permissions().mode() & 0o777 != 0o640
    {
        return Err(Error::new(
            "patch export must be steward:runtime mode 0640, regular, and single-linked",
        ));
    }
    Ok(ExportMetadata {
        path: path.to_string_lossy().into_owned(),
        uid: metadata.uid(),
        gid: metadata.gid(),
        mode: metadata.permissions().mode() & 0o777,
        sha256: sha256(bytes),
    })
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)] // All projections derive from one exact authored completion.
pub fn project_scheduled_contract(
    config: &Config,
    signer: &HmacSigner,
    due_nonce: &str,
    trigger_kind: &str,
    trigger_nonce: &str,
    started_at_unix_ms: u64,
    trace_id: &str,
    session_id: &str,
    turn_id: &str,
    prompt_sha256: &str,
    prompt_chars: usize,
    response_sha256: &str,
    reflection_path: &Path,
    summary: &str,
    tools_used: &[String],
    candidate_id: Option<&str>,
    candidate_digest: Option<&str>,
    intent_emitted: bool,
    completed_at_unix_ms: u64,
    span_id: &str,
    context_provenance: &ContextProvenance,
    inquiry: &InquiryClassification,
    inquiry_projection: Option<&ProjectionReceipt>,
    source_review: Option<&SourceReviewOutcome>,
) -> Result<()> {
    context_provenance.validate()?;
    if inquiry.source_review_requested() != source_review.is_some() {
        return Err(Error::new(
            "scheduled source-review projection does not match the authored request",
        ));
    }
    let inquiry_step_sha256 = inquiry
        .structured()
        .map(|value| canonical_json(&value.step).map(|bytes| sha256(&bytes)))
        .transpose()?;
    let next_due_at_unix_ms = if trigger_kind == "scheduled" {
        crate::schedule::prepare_completion_projection(config, due_nonce)?.saturating_mul(1_000)
    } else if trigger_kind == "evidence_integration" {
        crate::schedule::next_due_at(config)?.saturating_mul(1_000)
    } else {
        return Err(Error::new("scheduled projection trigger kind is invalid"));
    };
    let due_at_unix_ms = (trigger_kind == "scheduled").then(|| due_slot_millis(due_nonce));
    let provenance = crate::inquiry::authored_provenance(trigger_kind);
    let context_provenance_sha256 = context_provenance.digest()?;
    let relative_reflection = reflection_path
        .strip_prefix(&config.workspace_root)
        .map_err(|_| Error::new("reflection path escaped appliance workspace"))?
        .to_string_lossy()
        .into_owned();
    let trace = serde_json::json!({
        "schema_version": 1,
        "trace_id": trace_id,
        "turn_id": turn_id,
        "span_id": span_id,
        "session_id": session_id
    });
    let projection = projection_root(config);
    let continuity_bytes = if inquiry.is_structured() {
        let inquiry_projection = inquiry_projection.ok_or_else(|| {
            Error::new("structured inquiry is missing its signed current projection")
        })?;
        let continuity = serde_json::json!({
            "schema": CONTINUITY_SCHEMA,
            "appliance_id": config.appliance_id,
            "model": config.model,
            "trigger_kind": trigger_kind,
            "trigger_nonce": trigger_nonce,
            "due_nonce": due_nonce,
            "recorded_at_unix_ms": completed_at_unix_ms,
            "summary": summary,
            "summary_sha256": sha256(summary.as_bytes()),
            "response_sha256": response_sha256,
            "prompt_sha256": prompt_sha256,
            "reflection_path": relative_reflection,
            "signed_entry_id": inquiry_projection.signed_entry_id,
            "step_id": inquiry_projection.step_id,
            "admission_id": inquiry_projection.admission_id,
            "inquiry_current_projection_sha256": inquiry_projection.sha256,
            "trace": trace,
            "provenance": provenance,
            "authority": "bounded_signed_inquiry_continuity_projection_not_code_or_action_authority",
            "context_provenance": context_provenance,
            "context_provenance_sha256": context_provenance_sha256,
            "candidate_authoring_eligible": false,
            "reflection_lane": context_provenance.reflection_lane(),
            "taint_causes": context_provenance.taint_causes()
        });
        let bytes = canonical_json(&continuity)?;
        workspace_write(config, &projection.join("continuity.json"), &bytes)?;
        Some(bytes)
    } else {
        if inquiry_projection.is_some() {
            return Err(Error::new(
                "unstructured inquiry unexpectedly carries a current projection",
            ));
        }
        None
    };
    let state_path = projection.join("state.json");
    let previous = if state_path.exists() {
        serde_json::from_slice::<Value>(&read_stable_regular(&state_path, 128 * 1024)?)?
    } else {
        Value::Null
    };
    let same_transaction = previous.get("due_nonce").and_then(Value::as_str) == Some(due_nonce)
        && previous.get("last_response_sha256").and_then(Value::as_str) == Some(response_sha256)
        && previous.get("last_status").and_then(Value::as_str) == Some(inquiry.status.as_str());
    let attempts = previous
        .get("total_attempts")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(u64::from(!same_transaction));
    let authored = previous
        .get("total_authored")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(u64::from(!same_transaction));
    let structured = previous
        .get("total_structured")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(u64::from(!same_transaction && inquiry.is_structured()));
    let unstructured = previous
        .get("total_unstructured")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(u64::from(!same_transaction && !inquiry.is_structured()));
    let state = serde_json::json!({
        "schema": STATE_SCHEMA,
        "next_due_at_unix_ms": next_due_at_unix_ms,
        "last_started_at_unix_ms": started_at_unix_ms,
        "last_completed_at_unix_ms": completed_at_unix_ms,
        "last_status": inquiry.status,
        "last_trace": trace,
        "last_response_sha256": response_sha256,
        "last_artifact_path": relative_reflection,
        "total_attempts": attempts,
        "total_authored": authored,
        "total_structured": structured,
        "total_unstructured": unstructured,
        "consecutive_failures": 0,
        "running": false,
        "due_nonce": due_nonce,
        "steward_profile": "immutable_native_model_profile",
        "last_context_provenance_sha256": context_provenance_sha256,
        "last_candidate_authoring_eligible": context_provenance.candidate_authoring_eligible(),
        "last_reflection_lane": context_provenance.reflection_lane(),
        "last_taint_causes": context_provenance.taint_causes()
    });
    let state_bytes = canonical_json(&state)?;
    workspace_write(config, &state_path, &state_bytes)?;
    let source_review_projection = source_review.map(|review| {
        let trace = match (
            review.trace_id.as_deref(),
            review.turn_id.as_deref(),
            review.span_id.as_deref(),
            review.session_id.as_deref(),
        ) {
            (Some(trace_id), Some(turn_id), Some(span_id), Some(session_id)) => {
                Some(serde_json::json!({
                    "schema_version": 1,
                    "trace_id": trace_id,
                    "turn_id": turn_id,
                    "span_id": span_id,
                    "session_id": session_id
                }))
            },
            _ => None,
        };
        serde_json::json!({
            "status": review.status,
            "trace": trace,
            "response_sha256": review.response_sha256,
            "prompt_sha256": review.prompt_sha256,
            "candidate_attested": review.status == "candidate_attested",
            "failure_class": review.failure_class,
            "authority": review.authority
        })
    });
    let receipt = serde_json::json!({
        "schema": RECEIPT_SCHEMA,
        "appliance": config.appliance_id,
        "due_nonce": due_nonce,
        "due_at_unix_ms": due_at_unix_ms,
        "started_at_unix_ms": started_at_unix_ms,
        "completed_at_unix_ms": completed_at_unix_ms,
        "status": inquiry.status,
        "trigger_kind": trigger_kind,
        "trigger_nonce": trigger_nonce,
        "inquiry_status": inquiry.status,
        "inquiry_failure_class": inquiry.failure_class,
        "provenance": provenance,
        "model_id": config.model,
        "prompt_sha256": prompt_sha256,
        "prompt_chars": prompt_chars,
        "response_sha256": response_sha256,
        "reflection_path": relative_reflection,
        "continuity_admitted": false,
        "signed_entry_id": inquiry_projection.map(|value| &value.signed_entry_id),
        "step_id": inquiry_projection.map(|value| &value.step_id),
        "admission_id": inquiry_projection.map(|value| &value.admission_id),
        "inquiry_step_sha256": inquiry_step_sha256,
        "inquiry_declaration_sha256": inquiry.structured().map(|value| value.declaration_sha256.clone()),
        "inquiry_current_projection_sha256": inquiry_projection.map(|value| &value.sha256),
        "continuity_projection_sha256": continuity_bytes.as_ref().map(|bytes| sha256(bytes)),
        "continuity_projection_written": continuity_bytes.is_some(),
        "continuity_admission_status": if inquiry.is_structured() { "pending_runtime_verification" } else { "not_admitted_model_authored_unstructured" },
        "reservoir_admission_eligible": inquiry.is_structured(),
        "reservoir_admission_status": if inquiry.is_structured() { "pending_runtime_ack" } else { "not_eligible_model_authored_unstructured" },
        "source_review_relation": inquiry.structured().filter(|value| value.source_review.requested()).map(|_| "separate_clean_source_review"),
        "source_review": source_review_projection,
        "introspection_tool": "immutable_native_bounded_tool_loop",
        "introspection_result_sha256": sha256(summary.as_bytes()),
        "candidate_id": candidate_id,
        "candidate_digest": candidate_digest,
        "next_due_at_unix_ms": next_due_at_unix_ms,
        "trace": trace,
        "tools_used": tools_used,
        "intent_emitted": intent_emitted,
        "context_provenance": context_provenance,
        "context_provenance_sha256": context_provenance_sha256,
        "candidate_authoring_eligible": context_provenance.candidate_authoring_eligible(),
        "reflection_lane": context_provenance.reflection_lane(),
        "taint_causes": context_provenance.taint_causes(),
        "authority": if trigger_kind == "scheduled" {
            "scheduler_controls_cadence_model_authors_content_immutable_steward_attests_candidates"
        } else {
            "evidence_scheduler_controls_cadence_model_authors_interpretation_no_source_or_candidate_authority"
        }
    });
    let receipt_bytes = canonical_json(&receipt)?;
    let mut line = receipt_bytes.clone();
    line.push(b'\n');
    workspace_append_once(
        config,
        &artifact_root(config).join("receipts.jsonl"),
        due_nonce,
        response_sha256,
        &line,
    )?;
    write_authorship_attestation(
        config,
        signer,
        due_nonce,
        trigger_kind,
        trigger_nonce,
        started_at_unix_ms,
        completed_at_unix_ms,
        &trace,
        prompt_sha256,
        response_sha256,
        &relative_reflection,
        inquiry,
        inquiry_projection,
        continuity_bytes.as_deref(),
        &state_bytes,
        &receipt_bytes,
        &context_provenance_sha256,
        candidate_id,
        candidate_digest,
    )
}

#[allow(clippy::too_many_arguments)]
fn write_authorship_attestation(
    config: &Config,
    signer: &HmacSigner,
    due_nonce: &str,
    trigger_kind: &str,
    trigger_nonce: &str,
    started_at_unix_ms: u64,
    completed_at_unix_ms: u64,
    trace: &Value,
    prompt_sha256: &str,
    response_sha256: &str,
    relative_reflection: &str,
    inquiry: &InquiryClassification,
    inquiry_projection: Option<&ProjectionReceipt>,
    continuity_bytes: Option<&[u8]>,
    state_bytes: &[u8],
    receipt_bytes: &[u8],
    context_provenance_sha256: &str,
    candidate_id: Option<&str>,
    candidate_digest: Option<&str>,
) -> Result<()> {
    let reflection = config.workspace_root.join(relative_reflection);
    let reflection_metadata = reflection.with_extension("json");
    let response = read_stable_regular(&reflection, 64 * 1_024)?;
    let metadata = read_stable_regular(&reflection_metadata, 16 * 1_024)?;
    if sha256(&response) != response_sha256 {
        return Err(Error::new(
            "authored reflection changed before immutable authorship attestation",
        ));
    }
    let structured = inquiry.structured();
    let inquiry_step_sha256 = structured
        .map(|value| canonical_json(&value.step).map(|bytes| sha256(&bytes)))
        .transpose()?;
    if inquiry.is_structured() != inquiry_projection.is_some()
        || inquiry.is_structured() != continuity_bytes.is_some()
        || inquiry_projection
            .is_some_and(|projection| projection.sha256 != sha256(&projection.bytes))
    {
        return Err(Error::new(
            "authorship attestation has a partial inquiry binding",
        ));
    }
    let core = serde_json::json!({
        "schema": AUTHORSHIP_CORE_SCHEMA,
        "appliance_id": config.appliance_id,
        "due_nonce": due_nonce,
        "trigger_kind": trigger_kind,
        "trigger_nonce": trigger_nonce,
        "due_at_unix_ms": (trigger_kind == "scheduled").then(|| due_slot_millis(due_nonce)),
        "started_at_unix_ms": started_at_unix_ms,
        "completed_at_unix_ms": completed_at_unix_ms,
        "terminal_status": inquiry.status,
        "model": config.model,
        "prompt_sha256": prompt_sha256,
        "response_sha256": response_sha256,
        "reflection_path": relative_reflection,
        "reflection_sha256": sha256(&response),
        "reflection_metadata_sha256": sha256(&metadata),
        "continuity_projection_sha256": continuity_bytes.map(sha256),
        "inquiry_current_projection_sha256": inquiry_projection.map(|value| value.sha256.clone()),
        "signed_entry_id": inquiry_projection.map(|value| value.signed_entry_id.clone()),
        "step_id": inquiry_projection.map(|value| value.step_id.clone()),
        "admission_id": inquiry_projection.map(|value| value.admission_id.clone()),
        "inquiry_step_sha256": inquiry_step_sha256,
        "inquiry_declaration_sha256": structured.map(|value| value.declaration_sha256.clone()),
        "state_projection_sha256": sha256(state_bytes),
        "terminal_receipt_sha256": sha256(receipt_bytes),
        "context_provenance_sha256": context_provenance_sha256,
        "candidate_id": candidate_id,
        "candidate_digest": candidate_digest,
        "trace": trace,
        "provenance": crate::inquiry::authored_provenance(trigger_kind),
        "authority": "immutable_steward_signed_exact_authorship_join"
    });
    let unsigned = serde_json::json!({
        "schema": AUTHORSHIP_ENVELOPE_SCHEMA,
        "core": core
    });
    let unsigned_bytes = canonical_json(&unsigned)?;
    let envelope = serde_json::json!({
        "schema": AUTHORSHIP_ENVELOPE_SCHEMA,
        "core": unsigned["core"].clone(),
        "auth": {
            "algorithm": "ed25519",
            "key_id": signer.scheduled_authorship_key_id(),
            "signature": signer.sign_scheduled_authorship(&unsigned_bytes)
        }
    });
    let envelope_bytes = canonical_json(&envelope)?;
    let root = authorship_root(config);
    validate_authorship_root(config, &root)?;
    let record = root.join(format!("attestation_{due_nonce}_{response_sha256}.json"));
    private_write_exact(config, &record, &envelope_bytes)?;
    private_write(config, &root.join("current.json"), &envelope_bytes)?;
    // This copy is owner-visible presentation only.  Runtime continuity and
    // reservoir admission consume the read-only immutable-root projection.
    workspace_write_exact(
        config,
        &artifact_root(config).join(format!(
            "authorship_attestation_{due_nonce}_{response_sha256}.json"
        )),
        &envelope_bytes,
    )?;
    workspace_write(
        config,
        &projection_root(config).join("authorship.current.json"),
        &envelope_bytes,
    )
}

fn validate_authorship_root(config: &Config, path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let steward_uid = fs::symlink_metadata(&config.state_root)?.uid();
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != steward_uid
        || metadata.gid() != config.workspace_gid
        || metadata.permissions().mode() & 0o777 != 0o750
    {
        return Err(Error::new(
            "scheduled-authorship root must be steward:runtime mode 0750",
        ));
    }
    Ok(())
}

fn private_write(config: &Config, path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("scheduled-authorship output has no parent"))?;
    validate_authorship_root(config, parent)?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        Uuid::new_v4()
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true).mode(0o640);
    let mut file = options.open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    validate_output_file(
        config,
        &fs::symlink_metadata(path)?,
        "authorship attestation",
    )?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn private_write_exact(config: &Config, path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() || path.is_symlink() {
        validate_output_file(
            config,
            &fs::symlink_metadata(path)?,
            "append-only authorship attestation",
        )?;
        if read_stable_regular(path, 64 * 1_024)? != bytes {
            return Err(Error::new("scheduled-authorship attestation collision"));
        }
        return Ok(());
    }
    private_write(config, path, bytes)
}

fn workspace_append_once(
    config: &Config,
    path: &Path,
    due_nonce: &str,
    response_sha256: &str,
    line: &[u8],
) -> Result<()> {
    if path.exists() || path.is_symlink() {
        let metadata = fs::symlink_metadata(path)?;
        validate_output_file(config, &metadata, "scheduled introspection ledger")?;
        let bytes = read_stable_regular(path, 64 * 1024 * 1024)?;
        if !bytes.is_empty() && !bytes.ends_with(b"\n") {
            return Err(Error::new(
                "scheduled introspection ledger has an incomplete tail",
            ));
        }
        let mut exact = 0_u8;
        for prior in bytes
            .split(|byte| *byte == b'\n')
            .filter(|prior| !prior.is_empty())
        {
            let value: Value = serde_json::from_slice(prior)?;
            if value.get("due_nonce").and_then(Value::as_str) == Some(due_nonce) {
                if value.get("response_sha256").and_then(Value::as_str) != Some(response_sha256)
                    || prior != &line[..line.len().saturating_sub(1)]
                {
                    return Err(Error::new(
                        "scheduled due nonce already has a different terminal receipt",
                    ));
                }
                exact = exact.saturating_add(1);
            }
        }
        if exact > 1 {
            return Err(Error::new(
                "scheduled due nonce has duplicate terminal receipts",
            ));
        }
        if exact == 1 {
            return Ok(());
        }
    }
    workspace_append(config, path, line)
}

fn due_slot_millis(due_nonce: &str) -> u64 {
    due_nonce
        .strip_prefix("due-")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
        .saturating_mul(1_000)
}

#[cfg(test)]
mod tests {
    #[test]
    fn due_nonce_seconds_are_projected_as_unix_milliseconds() {
        assert_eq!(super::due_slot_millis("due-10000"), 10_000_000);
    }
}
