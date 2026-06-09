use super::*;

#[test]
fn empty_read_is_treated_as_absent_audit() {
    let summary = CausalityAuditSummary::default();
    assert!(map_causality_audit_summary(summary).is_none());
}

#[test]
fn mapped_audit_preserves_candidate_damp_fields() {
    let summary = CausalityAuditSummary {
        generated_at: "2026-04-20T12:00:00".to_string(),
        read: "inquiry_load_candidate".to_string(),
        summary: "Recent read: heavy inquiry underperforms.".to_string(),
        heavy_inquiry_reconcentrating_rate: "97.0%".to_string(),
        bounded_regulation_reconcentrating_rate: "86.0%".to_string(),
        fragile_recovery_observations: 18,
        candidate_damp_lane: Some("minime_inquiry_heavy_lane".to_string()),
        candidate_damp_summary: Some("Temporary damp candidate.".to_string()),
    };
    let mapped = map_causality_audit_summary(summary).expect("audit status");
    assert_eq!(mapped.read, "inquiry_load_candidate");
    assert_eq!(mapped.fragile_recovery_observations, 18);
    assert_eq!(
        mapped.candidate_damp_lane.as_deref(),
        Some("minime_inquiry_heavy_lane")
    );
    assert_eq!(
        mapped.candidate_damp_summary.as_deref(),
        Some("Temporary damp candidate.")
    );
}
