use spectral_bridge_server::evidence_study_capture::StudyCaptureWindowV1;

fn main() {
    let _forged = StudyCaptureWindowV1 {
        schema: "study_capture_window_v1",
        schema_version: 1,
        window_id: "studywindow_forged".to_string(),
        campaign_id: "campaign_forged".to_string(),
        study_id: "concordance_forged".to_string(),
        plan_id: "studyplan_forged".to_string(),
        plan_sha256: "a".repeat(64),
        sample_kind: "telemetry".to_string(),
        started_at_unix_ms: 1,
        expires_at_unix_ms: 2,
        sample_limit: 1,
        actor: "forged".to_string(),
        artifact_authority_state_v1: unreachable!(),
    };
}
