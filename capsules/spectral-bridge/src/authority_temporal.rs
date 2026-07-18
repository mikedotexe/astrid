//! Temporal and runtime binding for persisted authority grants.

use std::fs;
use std::path::Path;
use std::process;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::signal_spine::signal_deployment_identity_v1;

pub const MAX_FUTURE_CLOCK_SKEW_SECS: u64 = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityTemporalContextV1 {
    pub schema: String,
    pub schema_version: u32,
    pub scope: String,
    pub token_id: String,
    pub issued_at_unix_s: u64,
    pub expires_at_unix_s: u64,
    pub remaining_budget: u64,
    pub pause_generation: u64,
    pub source_identity: String,
    pub deployment_identity: String,
    pub process_identity: String,
    pub lifecycle_state: String,
}

#[derive(Debug, Clone)]
pub struct VerifiedAuthorityContextV1 {
    context: AuthorityTemporalContextV1,
}

#[derive(Debug, Clone, Serialize)]
pub struct DispatchReservationV1 {
    pub schema: &'static str,
    pub schema_version: u32,
    pub record_schema: &'static str,
    pub record_type: &'static str,
    pub reservation_id: String,
    pub request_id: String,
    pub token_id: String,
    pub scope: String,
    pub status: &'static str,
    pub reserved_at_unix_ms: u64,
    pub process_identity: String,
    pub deployment_identity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchOutcomeKindV1 {
    Sent,
    Released,
    OutcomeUnknownConsumed,
}

#[derive(Debug, Clone, Serialize)]
pub struct DispatchOutcomeV1 {
    pub schema: &'static str,
    pub schema_version: u32,
    pub record_schema: &'static str,
    pub record_type: &'static str,
    pub reservation_id: String,
    pub request_id: String,
    pub token_id: String,
    pub scope: String,
    pub outcome: DispatchOutcomeKindV1,
    pub recorded_at_unix_ms: u64,
    pub reason: String,
}

#[must_use]
pub fn current_context(
    scope: &str,
    token_id: &str,
    issued_at_unix_s: u64,
    expires_at_unix_s: u64,
    remaining_budget: u64,
    bridge_workspace: &Path,
) -> AuthorityTemporalContextV1 {
    AuthorityTemporalContextV1 {
        schema: "authority_temporal_context_v1".to_string(),
        schema_version: 1,
        scope: scope.to_string(),
        token_id: token_id.to_string(),
        issued_at_unix_s,
        expires_at_unix_s,
        remaining_budget,
        pause_generation: pause_generation(bridge_workspace),
        source_identity: source_identity(bridge_workspace),
        deployment_identity: signal_deployment_identity_v1(),
        process_identity: process_identity(),
        lifecycle_state: "approved_unconsumed".to_string(),
    }
}

pub fn verify_live(
    untrusted: &AuthorityTemporalContextV1,
    expected_scope: &str,
    expected_token_id: &str,
    now_unix_s: u64,
    bridge_workspace: &Path,
) -> Result<VerifiedAuthorityContextV1> {
    verify_common(
        untrusted,
        expected_scope,
        expected_token_id,
        now_unix_s,
        bridge_workspace,
    )?;
    if untrusted.process_identity != process_identity() {
        return Err(anyhow!("authority_process_identity_mismatch"));
    }
    Ok(VerifiedAuthorityContextV1 {
        context: untrusted.clone(),
    })
}

pub fn verify_read_only_research(
    untrusted: &AuthorityTemporalContextV1,
    expected_token_id: &str,
    now_unix_s: u64,
    bridge_workspace: &Path,
) -> Result<VerifiedAuthorityContextV1> {
    verify_common(
        untrusted,
        "read_only_research",
        expected_token_id,
        now_unix_s,
        bridge_workspace,
    )?;
    Ok(VerifiedAuthorityContextV1 {
        context: untrusted.clone(),
    })
}

fn verify_common(
    untrusted: &AuthorityTemporalContextV1,
    expected_scope: &str,
    expected_token_id: &str,
    now_unix_s: u64,
    bridge_workspace: &Path,
) -> Result<()> {
    if untrusted.schema != "authority_temporal_context_v1" || untrusted.schema_version != 1 {
        return Err(anyhow!("unsupported_authority_temporal_context"));
    }
    if untrusted.scope != expected_scope {
        return Err(anyhow!("authority_temporal_scope_mismatch"));
    }
    if untrusted.token_id != expected_token_id {
        return Err(anyhow!("authority_temporal_token_mismatch"));
    }
    if untrusted.issued_at_unix_s > now_unix_s.saturating_add(MAX_FUTURE_CLOCK_SKEW_SECS) {
        return Err(anyhow!("authority_issued_in_future"));
    }
    if untrusted.expires_at_unix_s < now_unix_s {
        return Err(anyhow!("authority_temporal_context_expired"));
    }
    if untrusted.remaining_budget == 0 {
        return Err(anyhow!("authority_temporal_budget_exhausted"));
    }
    if untrusted.lifecycle_state != "approved_unconsumed" {
        return Err(anyhow!("authority_temporal_lifecycle_not_executable"));
    }
    if untrusted.pause_generation != pause_generation(bridge_workspace) {
        return Err(anyhow!("authority_pause_generation_mismatch"));
    }
    if untrusted.source_identity != source_identity(bridge_workspace) {
        return Err(anyhow!("authority_source_identity_mismatch"));
    }
    if untrusted.deployment_identity != signal_deployment_identity_v1() {
        return Err(anyhow!("authority_deployment_identity_mismatch"));
    }
    Ok(())
}

#[must_use]
pub fn reservation(
    verified: &VerifiedAuthorityContextV1,
    request_id: &str,
) -> DispatchReservationV1 {
    let now = unix_now_ms();
    let seed = format!(
        "{}:{}:{}:{}",
        request_id,
        verified.context.token_id,
        process_identity(),
        now
    );
    DispatchReservationV1 {
        schema: "dispatch_reservation_v1",
        schema_version: 1,
        record_schema: "authority_dispatch_v1",
        record_type: "dispatch_reservation",
        reservation_id: format!("dispatch_{}", short_sha256(seed.as_bytes())),
        request_id: request_id.to_string(),
        token_id: verified.context.token_id.clone(),
        scope: verified.context.scope.clone(),
        status: "reserved",
        reserved_at_unix_ms: now,
        process_identity: process_identity(),
        deployment_identity: verified.context.deployment_identity.clone(),
    }
}

#[must_use]
pub fn outcome(
    reservation: &DispatchReservationV1,
    outcome: DispatchOutcomeKindV1,
    reason: impl Into<String>,
) -> DispatchOutcomeV1 {
    DispatchOutcomeV1 {
        schema: "dispatch_outcome_v1",
        schema_version: 1,
        record_schema: "authority_dispatch_v1",
        record_type: "dispatch_outcome",
        reservation_id: reservation.reservation_id.clone(),
        request_id: reservation.request_id.clone(),
        token_id: reservation.token_id.clone(),
        scope: reservation.scope.clone(),
        outcome,
        recorded_at_unix_ms: unix_now_ms(),
        reason: reason.into(),
    }
}

#[must_use]
pub fn process_identity() -> String {
    format!(
        "pid:{}:started_ms:{}",
        process::id(),
        process_started_unix_ms()
    )
}

fn process_started_unix_ms() -> u64 {
    static STARTED: OnceLock<u64> = OnceLock::new();
    *STARTED.get_or_init(unix_now_ms)
}

fn pause_generation(bridge_workspace: &Path) -> u64 {
    fs::read(bridge_workspace.join("diagnostics/steward_control_v1/control.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|value| value.get("pause_generation").and_then(Value::as_u64))
        .unwrap_or(0)
}

fn source_identity(bridge_workspace: &Path) -> String {
    let manifest = bridge_workspace.join("deployment_manifests/spectral-bridge.json");
    let value = fs::read(manifest)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    value
        .as_ref()
        .and_then(|item| item.pointer("/repository/source_identity_sha256"))
        .or_else(|| {
            value
                .as_ref()
                .and_then(|item| item.pointer("/repository/head"))
        })
        .and_then(Value::as_str)
        .unwrap_or("unknown_source")
        .to_string()
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn short_sha256(bytes: &[u8]) -> String {
    let digest = format!("{:x}", Sha256::digest(bytes));
    digest.chars().take(24).collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    fn workspace(pause_generation: u64) -> tempfile::TempDir {
        let root = tempdir().expect("tempdir");
        let state = root
            .path()
            .join("diagnostics/steward_control_v1/control.json");
        fs::create_dir_all(state.parent().expect("parent")).expect("mkdir");
        fs::write(
            state,
            format!(r#"{{"pause_generation":{pause_generation}}}"#),
        )
        .expect("state");
        root
    }

    #[test]
    fn live_verification_rejects_future_issue_time() {
        let workspace = workspace(2);
        let now = 1_000;
        let mut context = current_context(
            "semantic_microdose",
            "token",
            now,
            now + 60,
            1,
            workspace.path(),
        );
        context.issued_at_unix_s = now + MAX_FUTURE_CLOCK_SKEW_SECS + 1;
        let error = verify_live(
            &context,
            "semantic_microdose",
            "token",
            now,
            workspace.path(),
        )
        .expect_err("future issue time must fail");
        assert!(error.to_string().contains("issued_in_future"));
    }

    #[test]
    fn live_verification_rejects_pause_generation_change() {
        let workspace = workspace(2);
        let now = 1_000;
        let context = current_context(
            "semantic_microdose",
            "token",
            now,
            now + 60,
            1,
            workspace.path(),
        );
        fs::write(
            workspace
                .path()
                .join("diagnostics/steward_control_v1/control.json"),
            r#"{"pause_generation":3}"#,
        )
        .expect("state");
        let error = verify_live(
            &context,
            "semantic_microdose",
            "token",
            now,
            workspace.path(),
        )
        .expect_err("pause generation must bind grants");
        assert!(error.to_string().contains("pause_generation"));
    }

    #[test]
    fn read_only_research_allows_process_change_but_not_deployment_change() {
        let workspace = workspace(2);
        let now = 1_000;
        let mut context = current_context(
            "read_only_research",
            "token",
            now,
            now + 60,
            1,
            workspace.path(),
        );
        context.process_identity = "another-process".to_string();
        verify_read_only_research(&context, "token", now, workspace.path())
            .expect("read-only research is rebound by fresh verification");
    }

    #[test]
    fn reservation_and_outcome_do_not_expose_a_constructor_from_json() {
        let workspace = workspace(2);
        let now = 1_000;
        let context = current_context(
            "semantic_microdose",
            "token",
            now,
            now + 60,
            1,
            workspace.path(),
        );
        let verified = verify_live(
            &context,
            "semantic_microdose",
            "token",
            now,
            workspace.path(),
        )
        .expect("verified");
        let reservation = reservation(&verified, "request");
        let outcome = outcome(&reservation, DispatchOutcomeKindV1::Sent, "sent");
        assert_eq!(outcome.reservation_id, reservation.reservation_id);
    }
}
