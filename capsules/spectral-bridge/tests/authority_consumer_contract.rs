//! Synthetic grants exercise the verifier, not a serialization-as-approval proxy.
use serde_json::json;
use spectral_bridge_server::authority_temporal::{
    AuthorityTemporalContextV1, current_context, verify_live, verify_read_only_research,
};
use std::{fs, path::Path};

fn workspace() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    set_pause(root.path(), 12);
    set_source(root.path(), "synthetic-source-a");
    root
}

fn set_pause(root: &Path, generation: u64) {
    let path = root.join("diagnostics/steward_control_v1/control.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, json!({"pause_generation":generation}).to_string()).unwrap();
}

fn set_source(root: &Path, source: &str) {
    let path = root.join("deployment_manifests/spectral-bridge.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        json!({"repository":{"source_identity_sha256":source}}).to_string(),
    )
    .unwrap();
}

#[test]
fn decodable_claims_still_fail_each_actual_research_verifier_condition() {
    let root = workspace();
    let context = current_context(
        "read_only_research",
        "synthetic-token",
        1_000,
        1_060,
        1,
        root.path(),
    );
    verify_read_only_research(&context, "synthetic-token", 1_000, root.path()).unwrap();
    for (field, value, reason) in [
        (
            "schema",
            json!("other"),
            "unsupported_authority_temporal_context",
        ),
        (
            "schema_version",
            json!(2),
            "unsupported_authority_temporal_context",
        ),
        (
            "scope",
            json!("non_live"),
            "authority_temporal_scope_mismatch",
        ),
        (
            "token_id",
            json!("other-token"),
            "authority_temporal_token_mismatch",
        ),
        (
            "issued_at_unix_s",
            json!(1_006),
            "authority_issued_in_future",
        ),
        (
            "expires_at_unix_s",
            json!(999),
            "authority_temporal_context_expired",
        ),
        (
            "remaining_budget",
            json!(0),
            "authority_temporal_budget_exhausted",
        ),
        (
            "lifecycle_state",
            json!("accepted"),
            "authority_temporal_lifecycle_not_executable",
        ),
        (
            "pause_generation",
            json!(11),
            "authority_pause_generation_mismatch",
        ),
        (
            "source_identity",
            json!("other-source"),
            "authority_source_identity_mismatch",
        ),
        (
            "deployment_identity",
            json!("other-deployment"),
            "authority_deployment_identity_mismatch",
        ),
    ] {
        let mut wire = serde_json::to_value(&context).unwrap();
        wire[field] = value;
        // Extra declarations do not replace the fields consumed by verification.
        wire["safe"] = json!(true);
        wire["scope_claim"] = json!("read_only");
        let decoded: AuthorityTemporalContextV1 = serde_json::from_value(wire).unwrap();
        let before = serde_json::to_value(&decoded).unwrap();
        assert_eq!(
            verify_read_only_research(&decoded, "synthetic-token", 1_000, root.path())
                .unwrap_err()
                .to_string(),
            reason,
            "{field}"
        );
        assert_eq!(serde_json::to_value(decoded).unwrap(), before);
    }
}

#[test]
fn stored_grant_is_rechecked_against_changed_source_pause_and_time() {
    let root = workspace();
    let context = current_context(
        "read_only_research",
        "synthetic-token",
        1_000,
        1_060,
        1,
        root.path(),
    );
    let original = serde_json::to_vec(&context).unwrap();
    set_source(root.path(), "synthetic-source-b");
    assert_eq!(
        verify_read_only_research(&context, "synthetic-token", 1_000, root.path())
            .unwrap_err()
            .to_string(),
        "authority_source_identity_mismatch"
    );
    set_source(root.path(), "synthetic-source-a");
    set_pause(root.path(), 13);
    assert_eq!(
        verify_read_only_research(&context, "synthetic-token", 1_000, root.path())
            .unwrap_err()
            .to_string(),
        "authority_pause_generation_mismatch"
    );
    set_pause(root.path(), 12);
    verify_read_only_research(&context, "synthetic-token", 1_060, root.path()).unwrap();
    assert_eq!(
        verify_read_only_research(&context, "synthetic-token", 1_061, root.path())
            .unwrap_err()
            .to_string(),
        "authority_temporal_context_expired"
    );
    assert_eq!(serde_json::to_vec(&context).unwrap(), original);
}

#[test]
fn process_rebinding_is_specific_to_the_read_only_verifier() {
    let root = workspace();
    let mut context = current_context(
        "read_only_research",
        "synthetic-token",
        1_000,
        1_060,
        1,
        root.path(),
    );
    context.process_identity = "synthetic-earlier-process".into();
    verify_read_only_research(&context, "synthetic-token", 1_000, root.path()).unwrap();
    assert_eq!(
        verify_live(
            &context,
            "read_only_research",
            "synthetic-token",
            1_000,
            root.path()
        )
        .unwrap_err()
        .to_string(),
        "authority_process_identity_mismatch"
    );
    context.deployment_identity = "synthetic-earlier-deployment".into();
    assert_eq!(
        verify_read_only_research(&context, "synthetic-token", 1_000, root.path())
            .unwrap_err()
            .to_string(),
        "authority_deployment_identity_mismatch"
    );
}
