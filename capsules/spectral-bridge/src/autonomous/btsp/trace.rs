use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::paths::bridge_paths;

use super::helpers::{atomic_write_json, load_json_or_default, now_unix_s};
use super::{ActiveSovereigntyProposal, ProposalLedger, ResponseOutcomeNote};

const TRACE_BANK_SCHEMA_VERSION: u32 = 2;
const FAST_TRACE_WINDOW_SECS: u64 = 120;
const PROPOSAL_TRACE_WINDOW_SECS: u64 = 1_200;
const CONSOLIDATION_TRACE_WINDOW_SECS: u64 = 7_200;
const MAX_TRACE_RECORDS: usize = 512;
const REPLAY_NEAREST_LIMIT: usize = 12;
const ANTI_LOOP_MIN_PRIOR: usize = 5;
const ANTI_LOOP_RATIO_NUMERATOR: usize = 8;
const ANTI_LOOP_RATIO_DENOMINATOR: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct BTSPTraceBankV2 {
    pub schema_version: u32,
    pub trace_windows_secs: BTSPTraceWindowsV2,
    #[serde(default)]
    pub traces: Vec<BTSPEligibilityTraceV2>,
    #[serde(default)]
    pub instructive_signals: Vec<BTSPInstructiveSignalV2>,
    #[serde(default)]
    pub replay_summaries: Vec<BTSPReplaySummaryV2>,
    #[serde(default)]
    pub total_outcomes_scanned: u64,
    pub last_updated_unix_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct BTSPTraceWindowsV2 {
    pub fast_secs: u64,
    pub proposal_secs: u64,
    pub consolidation_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct BTSPEligibilityTraceV2 {
    pub trace_id: String,
    pub proposal_id: String,
    pub episode_id: String,
    pub owner: String,
    pub response_id: String,
    pub signal_fingerprint: String,
    #[serde(default)]
    pub matched_signal_families: Vec<String>,
    #[serde(default)]
    pub matched_signal_roles: Vec<String>,
    #[serde(default)]
    pub matched_live_signals: Vec<String>,
    #[serde(default)]
    pub matched_cues: Vec<String>,
    pub created_at_unix_s: u64,
    pub expires_at_unix_s: u64,
    pub outcome_recorded_at_unix_s: u64,
    pub fast_window_close_unix_s: u64,
    pub proposal_window_close_unix_s: u64,
    pub consolidation_window_close_unix_s: u64,
    pub signal_score: f32,
    #[serde(default)]
    pub owner_choice: Option<String>,
    #[serde(default)]
    pub choice_relation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct BTSPInstructiveSignalV2 {
    pub signal_id: String,
    pub trace_id: String,
    pub proposal_id: String,
    pub owner: String,
    pub response_id: String,
    pub signal_fingerprint: String,
    pub recorded_at_unix_s: u64,
    pub outcome_vector: BTSPOutcomeVectorV2,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct BTSPOutcomeVectorV2 {
    pub outcome_class: String,
    pub target_nearness_delta: String,
    pub fill_band_movement: String,
    pub distress_or_recovery: String,
    pub opening_vs_reconcentration: String,
    pub shape_verdict: String,
    pub phase: String,
    pub internal_process_quadrant: String,
    pub pressure_source: String,
    #[serde(default)]
    pub active_mode_count: Option<u64>,
    #[serde(default)]
    pub effective_dimensionality: Option<f32>,
    #[serde(default)]
    pub distinguishability_loss: Option<f32>,
    #[serde(default)]
    pub inhabitability_score: Option<f32>,
    pub recovery_score: f32,
    pub reconcentration_score: f32,
    pub softening_score: f32,
    pub widening_score: f32,
    pub evidence_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct BTSPReplaySummaryV2 {
    pub query_fingerprint: String,
    pub nearest_count: u64,
    pub same_fingerprint_count: u64,
    pub reconcentrating_count: u64,
    pub recovery_reconcentrating_count: u64,
    pub recovery_softening_count: u64,
    pub recovery_widening_count: u64,
    pub mixed_count: u64,
    pub overwhelming_reconcentration: bool,
    pub recommendation: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct BTSPTraceV2Summary {
    pub schema_version: u32,
    pub trace_count: u64,
    pub instructive_signal_count: u64,
    pub total_outcomes_scanned: u64,
    pub latest_signal_fingerprint: String,
    pub latest_outcome_class: String,
    pub reconcentrating_outcomes: u64,
    pub softening_outcomes: u64,
    pub widening_outcomes: u64,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct BTSPAntiLoopState {
    pub active: bool,
    pub reason: String,
    pub fingerprint: String,
    pub same_fingerprint_count: u64,
    pub reconcentrating_count: u64,
    pub widening_count: u64,
    pub recommendation: String,
}

#[derive(Debug, Clone, Default)]
pub(super) struct BTSPTraceSyncReport {
    pub summary: Option<BTSPTraceV2Summary>,
    pub current_teacher_signal: Option<BTSPOutcomeVectorV2>,
    pub replay_read: Option<BTSPReplaySummaryV2>,
    pub anti_loop_state: Option<BTSPAntiLoopState>,
}

pub(super) fn sync_trace_bank_v2(
    ledger: &ProposalLedger,
    controller_health: Option<&Value>,
) -> BTSPTraceBankV2 {
    let path = bridge_paths().btsp_trace_bank_v2_path();
    let previous = load_json_or_default::<BTSPTraceBankV2>(&path);
    let mut next = build_trace_bank_v2(ledger, controller_health);
    next.last_updated_unix_s = previous.last_updated_unix_s;
    if next != previous {
        next.last_updated_unix_s = now_unix_s();
        atomic_write_json(&path, &next);
    }
    next
}

pub(super) fn report_for_status(
    bank: &BTSPTraceBankV2,
    signal_fingerprint: &str,
    controller_health: Option<&Value>,
) -> BTSPTraceSyncReport {
    let replay_read = replay_summary_for(bank, signal_fingerprint);
    let anti_loop_state = anti_loop_state_for(signal_fingerprint, replay_read.as_ref());
    BTSPTraceSyncReport {
        summary: trace_summary(bank),
        current_teacher_signal: teacher_signal_from_health(controller_health),
        replay_read,
        anti_loop_state,
    }
}

pub(super) fn build_trace_bank_v2(
    ledger: &ProposalLedger,
    controller_health: Option<&Value>,
) -> BTSPTraceBankV2 {
    let mut records = ledger
        .proposals
        .iter()
        .flat_map(|proposal| trace_records_for_proposal(proposal, controller_health))
        .collect::<Vec<_>>();
    let total_outcomes_scanned = u64::try_from(records.len()).unwrap_or(u64::MAX);
    records.sort_by_key(|record| record.recorded_at_unix_s);
    if records.len() > MAX_TRACE_RECORDS {
        let keep_from = records.len().saturating_sub(MAX_TRACE_RECORDS);
        records = records.split_off(keep_from);
    }

    let traces = records
        .iter()
        .map(|record| record.trace.clone())
        .collect::<Vec<_>>();
    let instructive_signals = records
        .iter()
        .map(|record| record.instructive_signal.clone())
        .collect::<Vec<_>>();
    let replay_summaries = replay_summaries_for_records(&instructive_signals);

    BTSPTraceBankV2 {
        schema_version: TRACE_BANK_SCHEMA_VERSION,
        trace_windows_secs: trace_windows(),
        traces,
        instructive_signals,
        replay_summaries,
        total_outcomes_scanned,
        last_updated_unix_s: 0,
    }
}

fn trace_records_for_proposal(
    proposal: &ActiveSovereigntyProposal,
    controller_health: Option<&Value>,
) -> Vec<TraceRecord> {
    proposal
        .outcomes
        .iter()
        .map(|outcome| {
            let trace_id = trace_id_for(proposal, outcome);
            let outcome_vector = outcome_vector_for(outcome, controller_health);
            let trace = BTSPEligibilityTraceV2 {
                trace_id: trace_id.clone(),
                proposal_id: proposal.proposal_id.clone(),
                episode_id: proposal.episode_id.clone(),
                owner: outcome.owner.clone(),
                response_id: outcome.response_id.clone(),
                signal_fingerprint: proposal.signal_fingerprint.clone(),
                matched_signal_families: proposal.matched_signal_families.clone(),
                matched_signal_roles: proposal.matched_signal_roles.clone(),
                matched_live_signals: proposal.matched_live_signals.clone(),
                matched_cues: proposal.matched_cues.clone(),
                created_at_unix_s: proposal.created_at_unix_s,
                expires_at_unix_s: proposal.expires_at_unix_s,
                outcome_recorded_at_unix_s: outcome.recorded_at_unix_s,
                fast_window_close_unix_s: proposal
                    .created_at_unix_s
                    .saturating_add(FAST_TRACE_WINDOW_SECS),
                proposal_window_close_unix_s: proposal
                    .created_at_unix_s
                    .saturating_add(PROPOSAL_TRACE_WINDOW_SECS),
                consolidation_window_close_unix_s: proposal
                    .created_at_unix_s
                    .saturating_add(CONSOLIDATION_TRACE_WINDOW_SECS),
                signal_score: proposal.signal_score,
                owner_choice: owner_choice_for(proposal, outcome),
                choice_relation: choice_relation_for(proposal, outcome),
            };
            let instructive_signal = BTSPInstructiveSignalV2 {
                signal_id: format!("{trace_id}:teacher"),
                trace_id,
                proposal_id: proposal.proposal_id.clone(),
                owner: outcome.owner.clone(),
                response_id: outcome.response_id.clone(),
                signal_fingerprint: proposal.signal_fingerprint.clone(),
                recorded_at_unix_s: outcome.recorded_at_unix_s,
                outcome_vector,
            };
            TraceRecord {
                recorded_at_unix_s: outcome.recorded_at_unix_s,
                trace,
                instructive_signal,
            }
        })
        .collect()
}

fn outcome_vector_for(
    outcome: &ResponseOutcomeNote,
    controller_health: Option<&Value>,
) -> BTSPOutcomeVectorV2 {
    let health_shape = string_at(
        controller_health,
        &["perturb_visibility", "shape_verdict"],
        "unknown",
    );
    let shape_verdict = if health_shape == "unknown" {
        "unknown".to_string()
    } else {
        health_shape
    };
    let phase = string_at(controller_health, &["phase"], "unknown");
    let fill_band = string_at(controller_health, &["fill_band"], "unknown");
    let internal_process_quadrant =
        string_at(controller_health, &["internal_process_quadrant"], "unknown");
    let pressure_source = string_at(
        controller_health,
        &["pressure_source_status", "dominant_source"],
        "unknown",
    );
    let outcome_class = classify_outcome(
        &outcome.distress_or_recovery,
        &outcome.opening_vs_reconcentration,
        &shape_verdict,
    );

    BTSPOutcomeVectorV2 {
        outcome_class: outcome_class.clone(),
        target_nearness_delta: outcome.target_nearness.clone(),
        fill_band_movement: fill_band,
        distress_or_recovery: outcome.distress_or_recovery.clone(),
        opening_vs_reconcentration: outcome.opening_vs_reconcentration.clone(),
        shape_verdict,
        phase,
        internal_process_quadrant,
        pressure_source,
        active_mode_count: u64_at(controller_health, &["active_mode_count"]),
        effective_dimensionality: f32_at(controller_health, &["effective_dimensionality"]).or_else(
            || {
                f32_at(
                    controller_health,
                    &["spectral_denominator_v1", "effective_dimensionality"],
                )
            },
        ),
        distinguishability_loss: f32_at(controller_health, &["distinguishability_loss"]).or_else(
            || {
                f32_at(
                    controller_health,
                    &["spectral_denominator_v1", "distinguishability_loss"],
                )
            },
        ),
        inhabitability_score: f32_at(
            controller_health,
            &["inhabitable_fluctuation_status", "inhabitability_score"],
        )
        .or_else(|| {
            f32_at(
                controller_health,
                &["inhabitable_fluctuation_v1", "inhabitability_score"],
            )
        }),
        recovery_score: score_recovery(&outcome.distress_or_recovery, &outcome.target_nearness),
        reconcentration_score: score_reconcentration(&outcome_class),
        softening_score: score_softening(&outcome_class),
        widening_score: score_widening(&outcome_class),
        evidence_summary: format!(
            "target={}, recovery={}, shape={}, class={outcome_class}",
            outcome.target_nearness,
            outcome.distress_or_recovery,
            outcome.opening_vs_reconcentration
        ),
    }
}

fn teacher_signal_from_health(controller_health: Option<&Value>) -> Option<BTSPOutcomeVectorV2> {
    let health = controller_health?;
    let synthetic = ResponseOutcomeNote {
        proposal_id: "live_teacher_signal".to_string(),
        response_id: "current_state".to_string(),
        owner: "system".to_string(),
        recorded_at_unix_s: now_unix_s(),
        target_nearness: "mixed".to_string(),
        distress_or_recovery: match health.get("fill_band").and_then(Value::as_str) {
            Some("near" | "over") => "recovery".to_string(),
            Some("under") => "mixed".to_string(),
            _ => "unknown".to_string(),
        },
        opening_vs_reconcentration: match health
            .get("perturb_visibility")
            .and_then(|value| value.get("shape_verdict"))
            .and_then(Value::as_str)
        {
            Some("tightening") => "reconcentrating".to_string(),
            Some("opened") => "widening".to_string(),
            Some("softened_only") => "mixed".to_string(),
            _ => "mixed".to_string(),
        },
        note: String::new(),
    };
    Some(outcome_vector_for(&synthetic, controller_health))
}

fn replay_summaries_for_records(
    instructive_signals: &[BTSPInstructiveSignalV2],
) -> Vec<BTSPReplaySummaryV2> {
    let fingerprints = instructive_signals
        .iter()
        .map(|signal| signal.signal_fingerprint.clone())
        .filter(|fingerprint| !fingerprint.is_empty())
        .collect::<BTreeSet<_>>();
    fingerprints
        .into_iter()
        .filter_map(|fingerprint| replay_summary_for_signals(instructive_signals, &fingerprint))
        .collect()
}

pub(super) fn replay_summary_for(
    bank: &BTSPTraceBankV2,
    signal_fingerprint: &str,
) -> Option<BTSPReplaySummaryV2> {
    replay_summary_for_signals(&bank.instructive_signals, signal_fingerprint)
}

fn replay_summary_for_signals(
    instructive_signals: &[BTSPInstructiveSignalV2],
    signal_fingerprint: &str,
) -> Option<BTSPReplaySummaryV2> {
    if signal_fingerprint.is_empty() {
        return None;
    }
    let mut same = instructive_signals
        .iter()
        .filter(|signal| signal.signal_fingerprint == signal_fingerprint)
        .cloned()
        .collect::<Vec<_>>();
    same.sort_by_key(|signal| signal.recorded_at_unix_s);
    let same_fingerprint_count = same.len();
    let nearest = if same.len() > REPLAY_NEAREST_LIMIT {
        same.split_off(same.len().saturating_sub(REPLAY_NEAREST_LIMIT))
    } else {
        same
    };
    if nearest.is_empty() {
        return None;
    }
    let class_counts = outcome_class_counts(&nearest);
    let reconcentrating_count = class_counts
        .iter()
        .filter(|(class, _)| class.contains("reconcentrating"))
        .map(|(_, count)| *count)
        .fold(0_usize, usize::saturating_add);
    let recovery_reconcentrating_count = *class_counts
        .get("recovery_reconcentrating")
        .unwrap_or(&0_usize);
    let recovery_softening_count = *class_counts.get("recovery_softening").unwrap_or(&0_usize);
    let recovery_widening_count = *class_counts.get("recovery_widening").unwrap_or(&0_usize);
    let mixed_count = *class_counts.get("mixed").unwrap_or(&0_usize);
    let overwhelming_reconcentration = nearest.len() >= ANTI_LOOP_MIN_PRIOR
        && recovery_widening_count == 0
        && ratio_at_least(
            reconcentrating_count,
            nearest.len(),
            ANTI_LOOP_RATIO_NUMERATOR,
            ANTI_LOOP_RATIO_DENOMINATOR,
        );
    let recommendation = if overwhelming_reconcentration {
        "suppress_duplicate_proposal_until_counter_refusal_or_new_evidence".to_string()
    } else {
        "proposal_may_open_if_other_gates_allow".to_string()
    };
    let summary = if overwhelming_reconcentration {
        "Replay read: same-fingerprint outcomes are overwhelmingly reconcentrating; ask for study, refusal, counteroffer, or new evidence before reopening the same offer."
            .to_string()
    } else {
        "Replay read: same-fingerprint history is not strong enough to suppress a new bounded offer."
            .to_string()
    };

    Some(BTSPReplaySummaryV2 {
        query_fingerprint: signal_fingerprint.to_string(),
        nearest_count: u64::try_from(nearest.len()).unwrap_or(u64::MAX),
        same_fingerprint_count: u64::try_from(same_fingerprint_count).unwrap_or(u64::MAX),
        reconcentrating_count: u64::try_from(reconcentrating_count).unwrap_or(u64::MAX),
        recovery_reconcentrating_count: u64::try_from(recovery_reconcentrating_count)
            .unwrap_or(u64::MAX),
        recovery_softening_count: u64::try_from(recovery_softening_count).unwrap_or(u64::MAX),
        recovery_widening_count: u64::try_from(recovery_widening_count).unwrap_or(u64::MAX),
        mixed_count: u64::try_from(mixed_count).unwrap_or(u64::MAX),
        overwhelming_reconcentration,
        recommendation,
        summary,
    })
}

pub(super) fn anti_loop_state_for(
    signal_fingerprint: &str,
    replay_read: Option<&BTSPReplaySummaryV2>,
) -> Option<BTSPAntiLoopState> {
    let replay = replay_read?;
    if !replay.overwhelming_reconcentration {
        return Some(BTSPAntiLoopState {
            active: false,
            reason: String::new(),
            fingerprint: signal_fingerprint.to_string(),
            same_fingerprint_count: replay.same_fingerprint_count,
            reconcentrating_count: replay.reconcentrating_count,
            widening_count: replay.recovery_widening_count,
            recommendation: replay.recommendation.clone(),
        });
    }
    Some(BTSPAntiLoopState {
        active: true,
        reason: "same_fingerprint_overwhelmingly_reconcentrating".to_string(),
        fingerprint: signal_fingerprint.to_string(),
        same_fingerprint_count: replay.same_fingerprint_count,
        reconcentrating_count: replay.reconcentrating_count,
        widening_count: replay.recovery_widening_count,
        recommendation: replay.recommendation.clone(),
    })
}

fn trace_summary(bank: &BTSPTraceBankV2) -> Option<BTSPTraceV2Summary> {
    if bank.instructive_signals.is_empty() {
        return None;
    }
    let latest = bank
        .instructive_signals
        .iter()
        .max_by_key(|signal| signal.recorded_at_unix_s)?;
    let reconcentrating_outcomes = bank
        .instructive_signals
        .iter()
        .filter(|signal| {
            signal
                .outcome_vector
                .outcome_class
                .contains("reconcentrating")
        })
        .count();
    let softening_outcomes = bank
        .instructive_signals
        .iter()
        .filter(|signal| signal.outcome_vector.outcome_class.contains("softening"))
        .count();
    let widening_outcomes = bank
        .instructive_signals
        .iter()
        .filter(|signal| signal.outcome_vector.outcome_class.contains("widening"))
        .count();
    let summary = if widening_outcomes == 0 && reconcentrating_outcomes > 0 {
        "Trace V2 read: learned outcomes are still reconcentrating; recovery is not being counted as widening."
            .to_string()
    } else {
        "Trace V2 read: compact eligibility traces are available for replay.".to_string()
    };
    Some(BTSPTraceV2Summary {
        schema_version: bank.schema_version,
        trace_count: u64::try_from(bank.traces.len()).unwrap_or(u64::MAX),
        instructive_signal_count: u64::try_from(bank.instructive_signals.len()).unwrap_or(u64::MAX),
        total_outcomes_scanned: bank.total_outcomes_scanned,
        latest_signal_fingerprint: latest.signal_fingerprint.clone(),
        latest_outcome_class: latest.outcome_vector.outcome_class.clone(),
        reconcentrating_outcomes: u64::try_from(reconcentrating_outcomes).unwrap_or(u64::MAX),
        softening_outcomes: u64::try_from(softening_outcomes).unwrap_or(u64::MAX),
        widening_outcomes: u64::try_from(widening_outcomes).unwrap_or(u64::MAX),
        summary,
    })
}

fn classify_outcome(
    distress_or_recovery: &str,
    opening_vs_reconcentration: &str,
    shape_verdict: &str,
) -> String {
    let recovery = if matches!(distress_or_recovery, "recovery" | "moderately_positive") {
        "recovery"
    } else if distress_or_recovery == "worsening" {
        "worsening"
    } else {
        "mixed"
    };
    let shape = if opening_vs_reconcentration == "reconcentrating" || shape_verdict == "tightening"
    {
        "reconcentrating"
    } else if shape_verdict == "softened_only" {
        "softening"
    } else if matches!(opening_vs_reconcentration, "widening" | "opening")
        && shape_verdict == "opened"
    {
        "widening"
    } else {
        "mixed"
    };

    match (recovery, shape) {
        ("worsening", "reconcentrating") => "worsening_reconcentrating",
        ("recovery", "reconcentrating") => "recovery_reconcentrating",
        ("recovery", "softening") => "recovery_softening",
        ("recovery", "widening") => "recovery_widening",
        _ => "mixed",
    }
    .to_string()
}

fn score_recovery(distress_or_recovery: &str, target_nearness: &str) -> f32 {
    if distress_or_recovery == "recovery" || target_nearness == "positive" {
        1.0
    } else if distress_or_recovery == "worsening" || target_nearness == "negative" {
        -1.0
    } else {
        0.0
    }
}

fn score_reconcentration(outcome_class: &str) -> f32 {
    if outcome_class.contains("reconcentrating") {
        1.0
    } else {
        0.0
    }
}

fn score_softening(outcome_class: &str) -> f32 {
    if outcome_class.contains("softening") {
        1.0
    } else {
        0.0
    }
}

fn score_widening(outcome_class: &str) -> f32 {
    if outcome_class.contains("widening") {
        1.0
    } else {
        0.0
    }
}

fn owner_choice_for(
    proposal: &ActiveSovereigntyProposal,
    outcome: &ResponseOutcomeNote,
) -> Option<String> {
    proposal
        .choice_interpretations
        .iter()
        .rev()
        .find(|choice| choice.owner == outcome.owner)
        .map(|choice| choice.normalized_choice.clone())
        .or_else(|| {
            proposal
                .exact_adoptions
                .iter()
                .rev()
                .find(|adoption| adoption.owner == outcome.owner)
                .map(|adoption| adoption.normalized_choice.clone())
        })
}

fn choice_relation_for(
    proposal: &ActiveSovereigntyProposal,
    outcome: &ResponseOutcomeNote,
) -> Option<String> {
    proposal
        .choice_interpretations
        .iter()
        .rev()
        .find(|choice| choice.owner == outcome.owner)
        .map(|choice| choice.relation_to_proposal.clone())
}

fn outcome_class_counts(signals: &[BTSPInstructiveSignalV2]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for signal in signals {
        let class = signal.outcome_vector.outcome_class.clone();
        let entry = counts.entry(class).or_insert(0_usize);
        *entry = entry.saturating_add(1);
    }
    counts
}

fn ratio_at_least(count: usize, total: usize, numerator: usize, denominator: usize) -> bool {
    if total == 0 || denominator == 0 {
        return false;
    }
    count.saturating_mul(denominator) >= total.saturating_mul(numerator)
}

fn trace_id_for(proposal: &ActiveSovereigntyProposal, outcome: &ResponseOutcomeNote) -> String {
    let mut hasher = Sha256::new();
    hasher.update(proposal.proposal_id.as_bytes());
    hasher.update(b":");
    hasher.update(proposal.signal_fingerprint.as_bytes());
    hasher.update(b":");
    hasher.update(outcome.owner.as_bytes());
    hasher.update(b":");
    hasher.update(outcome.response_id.as_bytes());
    hasher.update(b":");
    hasher.update(outcome.recorded_at_unix_s.to_string().as_bytes());
    let digest = hasher.finalize();
    let short = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("btsp_trace_v2_{short}")
}

fn trace_windows() -> BTSPTraceWindowsV2 {
    BTSPTraceWindowsV2 {
        fast_secs: FAST_TRACE_WINDOW_SECS,
        proposal_secs: PROPOSAL_TRACE_WINDOW_SECS,
        consolidation_secs: CONSOLIDATION_TRACE_WINDOW_SECS,
    }
}

fn string_at(controller_health: Option<&Value>, path: &[&str], default: &str) -> String {
    value_at(controller_health, path)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn u64_at(controller_health: Option<&Value>, path: &[&str]) -> Option<u64> {
    value_at(controller_health, path).and_then(Value::as_u64)
}

fn f32_at(controller_health: Option<&Value>, path: &[&str]) -> Option<f32> {
    value_at(controller_health, path)
        .and_then(Value::as_f64)
        .map(|value| value as f32)
}

fn value_at<'a>(controller_health: Option<&'a Value>, path: &[&str]) -> Option<&'a Value> {
    let mut current = controller_health?;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

#[derive(Debug, Clone)]
struct TraceRecord {
    recorded_at_unix_s: u64,
    trace: BTSPEligibilityTraceV2,
    instructive_signal: BTSPInstructiveSignalV2,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::autonomous::btsp::{ProposalLedger, ResponseOutcomeNote};
    use serde_json::json;

    fn proposal_with_outcomes(
        proposal_id: &str,
        fingerprint: &str,
        outcomes: Vec<ResponseOutcomeNote>,
    ) -> ActiveSovereigntyProposal {
        ActiveSovereigntyProposal {
            proposal_id: proposal_id.to_string(),
            episode_id: "btsp_ep".to_string(),
            episode_name: "BTSP test".to_string(),
            matched_cues: vec!["grinding".to_string()],
            matched_live_signals: vec!["perturb_visibility:tightening".to_string()],
            matched_signal_families: vec!["grinding_family".to_string()],
            matched_signal_roles: vec!["early_warning".to_string()],
            signal_score: 0.8,
            confidence: 0.8,
            audience: "bilateral".to_string(),
            candidate_response_ids: Vec::new(),
            reply_state: "integrated".to_string(),
            selected_response_id: None,
            latest_selected_response_id: None,
            selected_response_ids_by_owner: HashMap::new(),
            owner_reply_state: HashMap::new(),
            outcome_status: "integrated".to_string(),
            created_at_unix_s: 100,
            expires_at_unix_s: 1300,
            matched_at_exchange: 1,
            latest_match_at_unix_s: 100,
            prompt_exposures: HashMap::new(),
            related_choice: None,
            signal_fingerprint: fingerprint.to_string(),
            last_choice_interpretation: None,
            choice_interpretations: Vec::new(),
            exact_adoptions: Vec::new(),
            adoption_contexts: HashMap::new(),
            outcomes,
            refusals: Vec::new(),
            counteroffers: Vec::new(),
            study_first_records: Vec::new(),
            last_negotiation_event_at_unix_s: 0,
            shadow_equivalences: Vec::new(),
        }
    }

    fn reconcentrating_outcome(index: u64, fingerprint: &str) -> ResponseOutcomeNote {
        ResponseOutcomeNote {
            proposal_id: format!("proposal_{fingerprint}_{index}"),
            response_id: "adjacent_uptake".to_string(),
            owner: "astrid".to_string(),
            recorded_at_unix_s: index,
            target_nearness: "positive".to_string(),
            distress_or_recovery: "recovery".to_string(),
            opening_vs_reconcentration: "reconcentrating".to_string(),
            note: "fixture".to_string(),
        }
    }

    #[test]
    fn teacher_vector_separates_recovery_reconcentration_from_softening() {
        let recon = ResponseOutcomeNote {
            opening_vs_reconcentration: "reconcentrating".to_string(),
            ..reconcentrating_outcome(1, "fp")
        };
        let softened = ResponseOutcomeNote {
            opening_vs_reconcentration: "mixed".to_string(),
            ..reconcentrating_outcome(2, "fp")
        };
        let soft_health = json!({
            "perturb_visibility": {"shape_verdict": "softened_only"},
            "phase": "plateau",
            "fill_band": "near",
            "internal_process_quadrant": "constricted_recovery"
        });

        assert_eq!(
            outcome_vector_for(&recon, None).outcome_class,
            "recovery_reconcentrating"
        );
        assert_eq!(
            outcome_vector_for(&softened, Some(&soft_health)).outcome_class,
            "recovery_softening"
        );
    }

    #[test]
    fn replay_marks_overwhelming_same_fingerprint_reconcentration() {
        let fingerprint = "families=grinding_family;transition=none";
        let proposals = (0_u64..6)
            .map(|index| {
                proposal_with_outcomes(
                    &format!("proposal_{index}"),
                    fingerprint,
                    vec![reconcentrating_outcome(index, fingerprint)],
                )
            })
            .collect::<Vec<_>>();
        let ledger = ProposalLedger {
            proposals,
            last_updated_unix_s: 0,
        };
        let bank = build_trace_bank_v2(&ledger, None);
        let replay = replay_summary_for(&bank, fingerprint).expect("replay summary");

        assert!(replay.overwhelming_reconcentration);
        assert_eq!(replay.reconcentrating_count, 6);
        assert!(
            anti_loop_state_for(fingerprint, Some(&replay))
                .expect("anti-loop")
                .active
        );
    }

    #[test]
    fn replay_does_not_count_recovery_as_widening_in_large_fixture() {
        let fingerprint = "families=grinding_family;transition=none";
        let outcomes = (0_u64..1_949)
            .map(|index| reconcentrating_outcome(index, fingerprint))
            .collect::<Vec<_>>();
        let classified_outcomes = outcomes
            .iter()
            .map(|outcome| outcome_vector_for(outcome, None))
            .collect::<Vec<_>>();
        let ledger = ProposalLedger {
            proposals: vec![proposal_with_outcomes(
                "large_fixture",
                fingerprint,
                outcomes,
            )],
            last_updated_unix_s: 0,
        };
        let bank = build_trace_bank_v2(&ledger, None);

        assert_eq!(classified_outcomes.len(), 1_949);
        assert!(
            classified_outcomes
                .iter()
                .all(|outcome| outcome.outcome_class == "recovery_reconcentrating")
        );
        assert!(
            classified_outcomes
                .iter()
                .all(|outcome| outcome.widening_score == 0.0)
        );
        assert_eq!(bank.total_outcomes_scanned, 1_949);
        assert!(
            bank.instructive_signals
                .iter()
                .all(|signal| signal.outcome_vector.outcome_class == "recovery_reconcentrating")
        );
        assert!(
            bank.instructive_signals
                .iter()
                .all(|signal| signal.outcome_vector.widening_score == 0.0)
        );
    }
}
