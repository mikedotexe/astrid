use serde::{Deserialize, Serialize};

use crate::paths::bridge_paths;

use super::helpers::load_json_or_default;

const CAUSALITY_AUDIT_STALE_SECS: i64 = 86_400;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct CausalityAuditStatus {
    pub generated_at: String,
    pub read: String,
    pub summary: String,
    pub heavy_inquiry_reconcentrating_rate: String,
    pub bounded_regulation_reconcentrating_rate: String,
    pub fragile_recovery_observations: u64,
    #[serde(default)]
    pub candidate_damp_lane: Option<String>,
    #[serde(default)]
    pub candidate_damp_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
struct CausalityAuditSummary {
    #[serde(default)]
    generated_at: String,
    #[serde(default)]
    read: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    heavy_inquiry_reconcentrating_rate: String,
    #[serde(default)]
    bounded_regulation_reconcentrating_rate: String,
    #[serde(default)]
    fragile_recovery_observations: u64,
    #[serde(default)]
    candidate_damp_lane: Option<String>,
    #[serde(default)]
    candidate_damp_summary: Option<String>,
}

pub(super) fn load_latest_causality_audit() -> Option<CausalityAuditStatus> {
    let path = bridge_paths()
        .bridge_workspace()
        .join("diagnostics")
        .join("btsp_causality_audit")
        .join("summary.json");
    let summary = load_json_or_default::<CausalityAuditSummary>(&path);
    map_causality_audit_summary(summary)
}

pub(super) fn is_causality_audit_stale(audit: &CausalityAuditStatus) -> bool {
    let Some(generated_at_unix_s) = parse_generated_at_unix_s(&audit.generated_at) else {
        return true;
    };
    let now = chrono::Utc::now().timestamp();
    now.saturating_sub(generated_at_unix_s) > CAUSALITY_AUDIT_STALE_SECS
}

fn map_causality_audit_summary(summary: CausalityAuditSummary) -> Option<CausalityAuditStatus> {
    if summary.read.trim().is_empty() {
        return None;
    }
    Some(CausalityAuditStatus {
        generated_at: summary.generated_at,
        read: summary.read,
        summary: summary.summary,
        heavy_inquiry_reconcentrating_rate: summary.heavy_inquiry_reconcentrating_rate,
        bounded_regulation_reconcentrating_rate: summary.bounded_regulation_reconcentrating_rate,
        fragile_recovery_observations: summary.fragile_recovery_observations,
        candidate_damp_lane: summary.candidate_damp_lane,
        candidate_damp_summary: summary.candidate_damp_summary,
    })
}

fn parse_generated_at_unix_s(raw: &str) -> Option<i64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    chrono::DateTime::parse_from_rfc3339(trimmed)
        .map(|dt| dt.timestamp())
        .ok()
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|dt| dt.and_utc().timestamp())
        })
}

#[cfg(test)]
#[path = "causality_tests.rs"]
mod causality_tests;
