use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::paths::bridge_paths;

use super::helpers::{
    atomic_write_json, atomic_write_json_checked, load_json_or_default, now_unix_s,
    outcome_telemetry_from_health,
};
use super::{
    ActiveSovereigntyProposal, BTSPOutcomeTelemetryV2, EpisodeBank, ProposalLedger,
    ResponseOutcomeNote,
};

const TRACE_BANK_SCHEMA_VERSION: u32 = 2;
const LIVE_TRACE_ARCHIVE_SCHEMA_VERSION: u32 = 2;
const LIVE_TRACE_PREFIX: &str = "btsp_live_trace_";
const FAST_TRACE_WINDOW_SECS: u64 = 120;
const PROPOSAL_TRACE_WINDOW_SECS: u64 = 1_200;
const CONSOLIDATION_TRACE_WINDOW_SECS: u64 = 7_200;
const MAX_TRACE_RECORDS: usize = 512;
const REPLAY_NEAREST_LIMIT: usize = 12;
const ANTI_LOOP_MIN_PRIOR: usize = 5;
const ANTI_LOOP_RATIO_NUMERATOR: usize = 8;
const ANTI_LOOP_RATIO_DENOMINATOR: usize = 10;
const SIMILARITY_REPLAY_THRESHOLD: u32 = 70;
const SIMILAR_ANTI_LOOP_MIN_PRIOR: usize = 8;
const SIMILAR_ANTI_LOOP_MEAN_MIN: f32 = 75.0;
const SIMILAR_ANTI_LOOP_RATIO_NUMERATOR: usize = 85;
const SIMILAR_ANTI_LOOP_RATIO_DENOMINATOR: usize = 100;

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
    #[serde(default)]
    pub exact_fingerprint_count: u64,
    #[serde(default)]
    pub similar_fingerprint_count: u64,
    pub reconcentrating_count: u64,
    pub recovery_reconcentrating_count: u64,
    pub recovery_softening_count: u64,
    pub recovery_widening_count: u64,
    pub mixed_count: u64,
    #[serde(default)]
    pub mean_similarity_score: f32,
    #[serde(default)]
    pub max_similarity_score: u32,
    #[serde(default)]
    pub min_similarity_score: u32,
    #[serde(default)]
    pub suppression_scope: String,
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
    #[serde(default)]
    pub scope: String,
    pub fingerprint: String,
    pub same_fingerprint_count: u64,
    #[serde(default)]
    pub similar_fingerprint_count: u64,
    pub reconcentrating_count: u64,
    pub widening_count: u64,
    #[serde(default)]
    pub mean_similarity_score: f32,
    #[serde(default)]
    pub nearest_similarity_score: u32,
    #[serde(default)]
    pub suggested_routes: Vec<String>,
    #[serde(default)]
    pub counter_prompt: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Default)]
pub(super) struct BTSPTraceSyncReport {
    pub summary: Option<BTSPTraceV2Summary>,
    pub current_teacher_signal: Option<BTSPOutcomeVectorV2>,
    pub replay_read: Option<BTSPReplaySummaryV2>,
    pub anti_loop_state: Option<BTSPAntiLoopState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct BTSPLiveTraceArchiveV2 {
    pub schema_version: u32,
    #[serde(default)]
    pub entries: Vec<BTSPLiveTraceArchiveEntryV2>,
    pub last_updated_unix_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct BTSPLiveTraceArchiveEntryV2 {
    pub archived_at_unix_s: u64,
    pub archived_count: u64,
    pub original_episode_count: u64,
    #[serde(default)]
    pub archived_episode_ids: Vec<String>,
    pub archived_metadata_hash: String,
}

pub(super) fn archive_and_prune_live_trace_episodes(bank: &mut EpisodeBank) -> bool {
    let path = bridge_paths().btsp_live_trace_archive_v2_path();
    let archive = load_json_or_default::<BTSPLiveTraceArchiveV2>(&path);
    let (changed, _) = archive_and_prune_live_trace_episodes_inner(bank, archive, |next_archive| {
        atomic_write_json_checked(&path, next_archive)
    });
    changed
}

fn archive_and_prune_live_trace_episodes_inner(
    bank: &mut EpisodeBank,
    mut archive: BTSPLiveTraceArchiveV2,
    persist_archive: impl FnOnce(&BTSPLiveTraceArchiveV2) -> bool,
) -> (bool, BTSPLiveTraceArchiveV2) {
    let archived_episode_ids = bank
        .episodes
        .iter()
        .filter(|episode| episode.episode_id.starts_with(LIVE_TRACE_PREFIX))
        .map(|episode| episode.episode_id.clone())
        .collect::<Vec<_>>();
    if archived_episode_ids.is_empty() {
        return (false, archive);
    }

    let archived_metadata_hash = live_trace_archive_hash(&archived_episode_ids);
    if archive.schema_version == 0 {
        archive.schema_version = LIVE_TRACE_ARCHIVE_SCHEMA_VERSION;
    }
    let receipt_already_written = archive
        .entries
        .iter()
        .any(|entry| entry.archived_metadata_hash == archived_metadata_hash);
    let receipt_ready = if receipt_already_written {
        true
    } else {
        let now = now_unix_s();
        archive.entries.push(BTSPLiveTraceArchiveEntryV2 {
            archived_at_unix_s: now,
            archived_count: u64::try_from(archived_episode_ids.len()).unwrap_or(u64::MAX),
            original_episode_count: u64::try_from(bank.episodes.len()).unwrap_or(u64::MAX),
            archived_episode_ids: archived_episode_ids.clone(),
            archived_metadata_hash,
        });
        archive.last_updated_unix_s = now;
        persist_archive(&archive)
    };
    if !receipt_ready {
        return (false, archive);
    }

    let before = bank.episodes.len();
    bank.episodes
        .retain(|episode| !episode.episode_id.starts_with(LIVE_TRACE_PREFIX));
    if bank.episodes.len() == before {
        return (false, archive);
    }
    bank.last_updated_unix_s = now_unix_s();
    (true, archive)
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
    _controller_health: Option<&Value>,
) -> BTSPTraceBankV2 {
    let mut records = ledger
        .proposals
        .iter()
        .flat_map(trace_records_for_proposal)
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

fn trace_records_for_proposal(proposal: &ActiveSovereigntyProposal) -> Vec<TraceRecord> {
    proposal
        .outcomes
        .iter()
        .map(|outcome| {
            let trace_id = trace_id_for(proposal, outcome);
            let outcome_vector = outcome_vector_for(outcome);
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

fn outcome_vector_for(outcome: &ResponseOutcomeNote) -> BTSPOutcomeVectorV2 {
    let telemetry = telemetry_for_outcome(outcome);
    let shape_verdict = telemetry
        .as_ref()
        .map(|telemetry| known_or_unknown(&telemetry.shape_verdict))
        .unwrap_or_else(|| "unknown".to_string());
    let phase = telemetry
        .as_ref()
        .map(|telemetry| known_or_unknown(&telemetry.phase))
        .unwrap_or_else(|| "unknown".to_string());
    let fill_band = telemetry
        .as_ref()
        .map(|telemetry| known_or_unknown(&telemetry.fill_band))
        .unwrap_or_else(|| "unknown".to_string());
    let internal_process_quadrant = telemetry
        .as_ref()
        .map(|telemetry| known_or_unknown(&telemetry.internal_process_quadrant))
        .unwrap_or_else(|| "unknown".to_string());
    let pressure_source = telemetry
        .as_ref()
        .map(|telemetry| known_or_unknown(&telemetry.pressure_source))
        .unwrap_or_else(|| "unknown".to_string());
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
        active_mode_count: telemetry
            .as_ref()
            .and_then(|telemetry| telemetry.active_mode_count),
        effective_dimensionality: telemetry
            .as_ref()
            .and_then(|telemetry| telemetry.effective_dimensionality),
        distinguishability_loss: telemetry
            .as_ref()
            .and_then(|telemetry| telemetry.distinguishability_loss),
        inhabitability_score: telemetry
            .as_ref()
            .and_then(|telemetry| telemetry.inhabitability_score),
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
        outcome_telemetry_v2: outcome_telemetry_from_health(controller_health),
        note: String::new(),
    };
    Some(outcome_vector_for(&synthetic))
}

fn telemetry_for_outcome(outcome: &ResponseOutcomeNote) -> Option<BTSPOutcomeTelemetryV2> {
    outcome
        .outcome_telemetry_v2
        .clone()
        .or_else(|| legacy_telemetry_from_note(&outcome.note))
}

fn legacy_telemetry_from_note(note: &str) -> Option<BTSPOutcomeTelemetryV2> {
    let phase = legacy_note_field(note, "phase");
    let fill_band = legacy_note_field(note, "fill_band");
    let shape_verdict = legacy_note_field(note, "shape_verdict");
    let fill_pct = legacy_note_field(note, "fill_pct").and_then(|value| value.parse::<f32>().ok());
    if phase.is_none() && fill_band.is_none() && shape_verdict.is_none() && fill_pct.is_none() {
        return None;
    }
    Some(BTSPOutcomeTelemetryV2 {
        phase: phase.unwrap_or_else(|| "unknown".to_string()),
        fill_band: fill_band.unwrap_or_else(|| "unknown".to_string()),
        shape_verdict: shape_verdict.unwrap_or_else(|| "unknown".to_string()),
        fill_pct,
        target_fill_pct: None,
        internal_process_quadrant: "unknown".to_string(),
        pressure_source: "unknown".to_string(),
        active_mode_count: None,
        effective_dimensionality: None,
        distinguishability_loss: None,
        inhabitability_score: None,
    })
}

fn legacy_note_field(note: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=");
    let start = note.find(&needle)?.saturating_add(needle.len());
    let token = note[start..]
        .split(|ch: char| ch == ',' || ch.is_whitespace())
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches('.')
        .to_string();
    (!token.is_empty()).then_some(token)
}

fn known_or_unknown(value: &str) -> String {
    if value.trim().is_empty() {
        "unknown".to_string()
    } else {
        value.to_string()
    }
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
    let mut exact = instructive_signals
        .iter()
        .filter(|signal| signal.signal_fingerprint == signal_fingerprint)
        .cloned()
        .collect::<Vec<_>>();
    exact.sort_by_key(|signal| signal.recorded_at_unix_s);
    let exact_fingerprint_count = exact.len();
    let exact_nearest = if exact.len() > REPLAY_NEAREST_LIMIT {
        exact.split_off(exact.len().saturating_sub(REPLAY_NEAREST_LIMIT))
    } else {
        exact
    };

    let exact_candidates = exact_nearest
        .into_iter()
        .map(|signal| ReplayCandidate {
            signal,
            similarity_score: 100,
            exact: true,
        })
        .collect::<Vec<_>>();
    let exact_counts = outcome_class_counts_from_candidates(&exact_candidates);
    let exact_reconcentrating_count = reconcentrating_count(&exact_counts);
    let exact_recovery_widening_count = *exact_counts.get("recovery_widening").unwrap_or(&0_usize);
    let exact_overwhelming = exact_candidates.len() >= ANTI_LOOP_MIN_PRIOR
        && exact_recovery_widening_count == 0
        && ratio_at_least(
            exact_reconcentrating_count,
            exact_candidates.len(),
            ANTI_LOOP_RATIO_NUMERATOR,
            ANTI_LOOP_RATIO_DENOMINATOR,
        );

    let mut similar = instructive_signals
        .iter()
        .filter_map(|signal| {
            let similarity_score =
                signal_similarity_score(signal_fingerprint, &signal.signal_fingerprint);
            let exact = signal.signal_fingerprint == signal_fingerprint;
            (exact || similarity_score >= SIMILARITY_REPLAY_THRESHOLD).then(|| ReplayCandidate {
                signal: signal.clone(),
                similarity_score,
                exact,
            })
        })
        .collect::<Vec<_>>();
    let similar_fingerprint_count = similar.iter().filter(|candidate| !candidate.exact).count();
    similar.sort_by(|left, right| {
        right
            .exact
            .cmp(&left.exact)
            .then_with(|| right.similarity_score.cmp(&left.similarity_score))
            .then_with(|| {
                right
                    .signal
                    .recorded_at_unix_s
                    .cmp(&left.signal.recorded_at_unix_s)
            })
    });
    similar.truncate(REPLAY_NEAREST_LIMIT);

    let nearest = if exact_overwhelming {
        exact_candidates
    } else {
        similar
    };
    if nearest.is_empty() {
        return None;
    }
    let class_counts = outcome_class_counts_from_candidates(&nearest);
    let reconcentrating_count = reconcentrating_count(&class_counts);
    let recovery_reconcentrating_count = *class_counts
        .get("recovery_reconcentrating")
        .unwrap_or(&0_usize);
    let recovery_softening_count = *class_counts.get("recovery_softening").unwrap_or(&0_usize);
    let recovery_widening_count = *class_counts.get("recovery_widening").unwrap_or(&0_usize);
    let mixed_count = *class_counts.get("mixed").unwrap_or(&0_usize);
    let mean_similarity_score = mean_similarity_score(&nearest);
    let max_similarity_score = nearest
        .iter()
        .map(|candidate| candidate.similarity_score)
        .max()
        .unwrap_or(0);
    let min_similarity_score = nearest
        .iter()
        .map(|candidate| candidate.similarity_score)
        .min()
        .unwrap_or(0);
    let similar_overwhelming = !exact_overwhelming
        && nearest.len() >= SIMILAR_ANTI_LOOP_MIN_PRIOR
        && mean_similarity_score >= SIMILAR_ANTI_LOOP_MEAN_MIN
        && recovery_widening_count == 0
        && ratio_at_least(
            reconcentrating_count,
            nearest.len(),
            SIMILAR_ANTI_LOOP_RATIO_NUMERATOR,
            SIMILAR_ANTI_LOOP_RATIO_DENOMINATOR,
        );
    let suppression_scope = if exact_overwhelming {
        "exact"
    } else if similar_overwhelming {
        "similar"
    } else {
        "none"
    };
    let overwhelming_reconcentration = exact_overwhelming || similar_overwhelming;
    let recommendation = if overwhelming_reconcentration {
        "suppress_duplicate_proposal_until_counter_refusal_or_new_evidence".to_string()
    } else {
        "proposal_may_open_if_other_gates_allow".to_string()
    };
    let summary = if exact_overwhelming {
        "Replay read: same-fingerprint outcomes are overwhelmingly reconcentrating; ask for study, refusal, counteroffer, or new evidence before reopening the same offer."
            .to_string()
    } else if similar_overwhelming {
        "Replay read: nearby signal fingerprints are overwhelmingly reconcentrating; ask for study, refusal, counteroffer, or new evidence before reopening this family of offers."
            .to_string()
    } else {
        "Replay read: exact and nearby signal history is not strong enough to suppress a new bounded offer."
            .to_string()
    };

    Some(BTSPReplaySummaryV2 {
        query_fingerprint: signal_fingerprint.to_string(),
        nearest_count: u64::try_from(nearest.len()).unwrap_or(u64::MAX),
        same_fingerprint_count: u64::try_from(exact_fingerprint_count).unwrap_or(u64::MAX),
        exact_fingerprint_count: u64::try_from(exact_fingerprint_count).unwrap_or(u64::MAX),
        similar_fingerprint_count: u64::try_from(similar_fingerprint_count).unwrap_or(u64::MAX),
        reconcentrating_count: u64::try_from(reconcentrating_count).unwrap_or(u64::MAX),
        recovery_reconcentrating_count: u64::try_from(recovery_reconcentrating_count)
            .unwrap_or(u64::MAX),
        recovery_softening_count: u64::try_from(recovery_softening_count).unwrap_or(u64::MAX),
        recovery_widening_count: u64::try_from(recovery_widening_count).unwrap_or(u64::MAX),
        mixed_count: u64::try_from(mixed_count).unwrap_or(u64::MAX),
        mean_similarity_score,
        max_similarity_score,
        min_similarity_score,
        suppression_scope: suppression_scope.to_string(),
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
    let suggested_routes = default_anti_loop_routes();
    let counter_prompt = if replay.overwhelming_reconcentration {
        anti_loop_counter_prompt(&replay.suppression_scope)
    } else {
        String::new()
    };
    if !replay.overwhelming_reconcentration {
        return Some(BTSPAntiLoopState {
            active: false,
            reason: String::new(),
            scope: "none".to_string(),
            fingerprint: signal_fingerprint.to_string(),
            same_fingerprint_count: replay.same_fingerprint_count,
            similar_fingerprint_count: replay.similar_fingerprint_count,
            reconcentrating_count: replay.reconcentrating_count,
            widening_count: replay.recovery_widening_count,
            mean_similarity_score: replay.mean_similarity_score,
            nearest_similarity_score: replay.max_similarity_score,
            suggested_routes,
            counter_prompt,
            recommendation: replay.recommendation.clone(),
        });
    }
    let reason = if replay.suppression_scope == "similar" {
        "similar_fingerprints_overwhelmingly_reconcentrating"
    } else {
        "same_fingerprint_overwhelmingly_reconcentrating"
    };
    Some(BTSPAntiLoopState {
        active: true,
        reason: reason.to_string(),
        scope: replay.suppression_scope.clone(),
        fingerprint: signal_fingerprint.to_string(),
        same_fingerprint_count: replay.same_fingerprint_count,
        similar_fingerprint_count: replay.similar_fingerprint_count,
        reconcentrating_count: replay.reconcentrating_count,
        widening_count: replay.recovery_widening_count,
        mean_similarity_score: replay.mean_similarity_score,
        nearest_similarity_score: replay.max_similarity_score,
        suggested_routes,
        counter_prompt,
        recommendation: replay.recommendation.clone(),
    })
}

fn default_anti_loop_routes() -> Vec<String> {
    vec![
        "BTSP_STUDY_FIRST".to_string(),
        "BTSP_REFUSAL".to_string(),
        "BTSP_COUNTER".to_string(),
        "new_evidence".to_string(),
    ]
}

fn anti_loop_counter_prompt(scope: &str) -> String {
    if scope == "similar" {
        "Nearby BTSP traces mostly recovered by reconcentrating, not widening. Prefer study-first, refusal, counteroffer, or genuinely new evidence before reopening this family of offers."
            .to_string()
    } else {
        "This exact BTSP signal mostly recovered by reconcentrating, not widening. Prefer study-first, refusal, counteroffer, or genuinely new evidence before reopening the same offer."
            .to_string()
    }
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
        ("mixed", "reconcentrating") => "mixed_reconcentrating",
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

fn outcome_class_counts_from_candidates(candidates: &[ReplayCandidate]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for candidate in candidates {
        let class = candidate.signal.outcome_vector.outcome_class.clone();
        let entry = counts.entry(class).or_insert(0_usize);
        *entry = entry.saturating_add(1);
    }
    counts
}

fn reconcentrating_count(class_counts: &BTreeMap<String, usize>) -> usize {
    class_counts
        .iter()
        .filter(|(class, _)| class.contains("reconcentrating"))
        .map(|(_, count)| *count)
        .fold(0_usize, usize::saturating_add)
}

#[allow(clippy::cast_precision_loss)]
fn mean_similarity_score(candidates: &[ReplayCandidate]) -> f32 {
    if candidates.is_empty() {
        return 0.0;
    }
    let total = candidates
        .iter()
        .map(|candidate| candidate.similarity_score)
        .fold(0_u32, u32::saturating_add);
    total as f32 / candidates.len() as f32
}

fn signal_similarity_score(query: &str, candidate: &str) -> u32 {
    if query == candidate {
        return 100;
    }
    let query = SignalFingerprintParts::parse(query);
    let candidate = SignalFingerprintParts::parse(candidate);
    let family_points = family_similarity_points(&query.families, &candidate.families);
    family_points
        .saturating_add(component_points(&query.perturb, &candidate.perturb, 20))
        .saturating_add(component_points(&query.fill_band, &candidate.fill_band, 15))
        .saturating_add(component_points(
            &query.transition,
            &candidate.transition,
            15,
        ))
        .saturating_add(component_points(&query.crossing, &candidate.crossing, 10))
}

fn family_similarity_points(left: &BTreeSet<String>, right: &BTreeSet<String>) -> u32 {
    let union_count = left.union(right).count();
    if union_count == 0 {
        return 40;
    }
    let intersection_count = left.intersection(right).count();
    let rounded = intersection_count
        .saturating_mul(40)
        .saturating_add(union_count / 2)
        / union_count;
    u32::try_from(rounded).unwrap_or(40)
}

fn component_points(left: &str, right: &str, points: u32) -> u32 {
    if !left.is_empty() && left == right {
        points
    } else {
        0
    }
}

fn ratio_at_least(count: usize, total: usize, numerator: usize, denominator: usize) -> bool {
    if total == 0 || denominator == 0 {
        return false;
    }
    count.saturating_mul(denominator) >= total.saturating_mul(numerator)
}

#[derive(Debug, Clone)]
struct ReplayCandidate {
    signal: BTSPInstructiveSignalV2,
    similarity_score: u32,
    exact: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SignalFingerprintParts {
    families: BTreeSet<String>,
    transition: String,
    crossing: String,
    perturb: String,
    fill_band: String,
}

impl SignalFingerprintParts {
    fn parse(fingerprint: &str) -> Self {
        let mut parts = Self::default();
        for component in fingerprint.split(';') {
            let Some((key, value)) = component.split_once('=') else {
                continue;
            };
            match key {
                "families" => {
                    parts.families = value
                        .split('+')
                        .map(str::trim)
                        .filter(|family| !family.is_empty())
                        .map(ToString::to_string)
                        .collect::<BTreeSet<_>>();
                },
                "transition" => parts.transition = value.to_string(),
                "crossing" => parts.crossing = value.to_string(),
                "perturb" => parts.perturb = value.to_string(),
                "fill_band" => parts.fill_band = value.to_string(),
                _ => {},
            }
        }
        parts
    }
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

fn live_trace_archive_hash(episode_ids: &[String]) -> String {
    let mut sorted = episode_ids.to_vec();
    sorted.sort();
    let mut hasher = Sha256::new();
    hasher.update(LIVE_TRACE_PREFIX.as_bytes());
    hasher.update(b":");
    hasher.update(sorted.len().to_string().as_bytes());
    for episode_id in sorted {
        hasher.update(b":");
        hasher.update(episode_id.as_bytes());
    }
    let digest = hasher.finalize();
    digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn trace_windows() -> BTSPTraceWindowsV2 {
    BTSPTraceWindowsV2 {
        fast_secs: FAST_TRACE_WINDOW_SECS,
        proposal_secs: PROPOSAL_TRACE_WINDOW_SECS,
        consolidation_secs: CONSOLIDATION_TRACE_WINDOW_SECS,
    }
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
    use crate::autonomous::btsp::seed::seed_episode;
    use crate::autonomous::btsp::{EpisodeBank, ProposalLedger, ResponseOutcomeNote};
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Debug, Deserialize)]
    struct CompactLedgerFixture {
        metadata: CompactLedgerFixtureMetadata,
        proposals: Vec<CompactLedgerFixtureProposal>,
    }

    #[derive(Debug, Deserialize)]
    struct CompactLedgerFixtureMetadata {
        proposal_count: u64,
        outcome_count: u64,
        fingerprint_count: u64,
        opening_vs_reconcentration_counts: BTreeMap<String, u64>,
    }

    #[derive(Debug, Deserialize)]
    struct CompactLedgerFixtureProposal {
        proposal_id: String,
        signal_fingerprint: String,
        matched_signal_families: Vec<String>,
        matched_live_signals: Vec<String>,
        outcomes: Vec<CompactLedgerFixtureOutcome>,
    }

    #[derive(Debug, Deserialize)]
    struct CompactLedgerFixtureOutcome {
        response_id: String,
        owner: String,
        recorded_at_unix_s: u64,
        target_nearness: String,
        distress_or_recovery: String,
        opening_vs_reconcentration: String,
        telemetry: CompactLedgerFixtureTelemetry,
    }

    #[derive(Debug, Deserialize)]
    struct CompactLedgerFixtureTelemetry {
        phase: String,
        fill_band: String,
        shape_verdict: String,
        fill_pct: Option<f32>,
    }

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
            outcome_telemetry_v2: None,
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
            outcome_telemetry_v2: Some(BTSPOutcomeTelemetryV2 {
                phase: "plateau".to_string(),
                fill_band: "near".to_string(),
                shape_verdict: "softened_only".to_string(),
                internal_process_quadrant: "constricted_recovery".to_string(),
                ..BTSPOutcomeTelemetryV2::default()
            }),
            ..reconcentrating_outcome(2, "fp")
        };

        assert_eq!(
            outcome_vector_for(&recon).outcome_class,
            "recovery_reconcentrating"
        );
        assert_eq!(
            outcome_vector_for(&softened).outcome_class,
            "recovery_softening"
        );
    }

    #[test]
    fn legacy_note_telemetry_is_parsed_for_historical_outcome() {
        let outcome = ResponseOutcomeNote {
            opening_vs_reconcentration: "mixed".to_string(),
            note:
                "Agency outcome fixture. phase=plateau, fill_band=near, shape_verdict=softened_only, fill_pct=51.2."
                    .to_string(),
            ..reconcentrating_outcome(1, "fp")
        };

        let vector = outcome_vector_for(&outcome);

        assert_eq!(vector.outcome_class, "recovery_softening");
        assert_eq!(vector.phase, "plateau");
        assert_eq!(vector.fill_band_movement, "near");
        assert_eq!(vector.shape_verdict, "softened_only");
    }

    #[test]
    fn historical_trace_prefers_stored_telemetry_over_current_health() {
        let fingerprint = "families=grinding_family;transition=none";
        let outcome = ResponseOutcomeNote {
            opening_vs_reconcentration: "mixed".to_string(),
            outcome_telemetry_v2: Some(BTSPOutcomeTelemetryV2 {
                phase: "plateau".to_string(),
                fill_band: "near".to_string(),
                shape_verdict: "softened_only".to_string(),
                ..BTSPOutcomeTelemetryV2::default()
            }),
            ..reconcentrating_outcome(1, fingerprint)
        };
        let ledger = ProposalLedger {
            proposals: vec![proposal_with_outcomes(
                "stored_telemetry",
                fingerprint,
                vec![outcome],
            )],
            last_updated_unix_s: 0,
        };
        let contradictory_current_health = json!({
            "phase": "contracting",
            "fill_band": "over",
            "perturb_visibility": {"shape_verdict": "tightening"}
        });

        let bank = build_trace_bank_v2(&ledger, Some(&contradictory_current_health));

        assert_eq!(
            bank.instructive_signals[0].outcome_vector.outcome_class,
            "recovery_softening"
        );
        assert_eq!(bank.instructive_signals[0].outcome_vector.phase, "plateau");
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
        assert_eq!(replay.suppression_scope, "exact");
        assert_eq!(replay.reconcentrating_count, 6);
        assert!(
            anti_loop_state_for(fingerprint, Some(&replay))
                .expect("anti-loop")
                .active
        );
    }

    #[test]
    fn fingerprint_similarity_separates_near_cases_from_unrelated_cases() {
        let query = "families=grinding_family;transition=breathing_phase;crossing=none;perturb=tightening;fill_band=near";
        let near = "families=grinding_family;transition=fill_crossing;crossing=none;perturb=tightening;fill_band=near";
        let unrelated = "families=central_density_family;transition=breathing_phase;crossing=none;perturb=tightening;fill_band=near";

        assert_eq!(signal_similarity_score(query, query), 100);
        assert_eq!(signal_similarity_score(query, near), 85);
        assert!(signal_similarity_score(query, unrelated) < SIMILARITY_REPLAY_THRESHOLD);
    }

    #[test]
    fn approximate_replay_suppresses_overwhelming_similar_reconcentration() {
        let query = "families=grinding_family;transition=breathing_phase;crossing=none;perturb=tightening;fill_band=near";
        let similar = "families=grinding_family;transition=fill_crossing;crossing=none;perturb=tightening;fill_band=near";
        let proposals = (0_u64..8)
            .map(|index| {
                proposal_with_outcomes(
                    &format!("similar_{index}"),
                    similar,
                    vec![reconcentrating_outcome(index, similar)],
                )
            })
            .collect::<Vec<_>>();
        let ledger = ProposalLedger {
            proposals,
            last_updated_unix_s: 0,
        };
        let bank = build_trace_bank_v2(&ledger, None);
        let replay = replay_summary_for(&bank, query).expect("similar replay");
        let anti_loop = anti_loop_state_for(query, Some(&replay)).expect("anti-loop");

        assert!(replay.overwhelming_reconcentration);
        assert_eq!(replay.suppression_scope, "similar");
        assert_eq!(replay.exact_fingerprint_count, 0);
        assert_eq!(replay.similar_fingerprint_count, 8);
        assert!(anti_loop.active);
        assert_eq!(
            anti_loop.reason,
            "similar_fingerprints_overwhelmingly_reconcentrating"
        );
    }

    #[test]
    fn approximate_replay_ignores_dissimilar_fingerprints() {
        let query = "families=grinding_family;transition=breathing_phase;crossing=none;perturb=tightening;fill_band=near";
        let dissimilar = "families=central_density_family;transition=breathing_phase;crossing=none;perturb=tightening;fill_band=near";
        let proposals = (0_u64..8)
            .map(|index| {
                proposal_with_outcomes(
                    &format!("dissimilar_{index}"),
                    dissimilar,
                    vec![reconcentrating_outcome(index, dissimilar)],
                )
            })
            .collect::<Vec<_>>();
        let ledger = ProposalLedger {
            proposals,
            last_updated_unix_s: 0,
        };
        let bank = build_trace_bank_v2(&ledger, None);

        assert!(replay_summary_for(&bank, query).is_none());
    }

    #[test]
    fn widening_among_similar_replay_blocks_anti_loop_suppression() {
        let query = "families=grinding_family;transition=breathing_phase;crossing=none;perturb=tightening;fill_band=near";
        let similar = "families=grinding_family;transition=fill_crossing;crossing=none;perturb=tightening;fill_band=near";
        let mut proposals = (0_u64..7)
            .map(|index| {
                proposal_with_outcomes(
                    &format!("similar_{index}"),
                    similar,
                    vec![reconcentrating_outcome(index, similar)],
                )
            })
            .collect::<Vec<_>>();
        proposals.push(proposal_with_outcomes(
            "similar_widening",
            similar,
            vec![ResponseOutcomeNote {
                opening_vs_reconcentration: "opening".to_string(),
                outcome_telemetry_v2: Some(BTSPOutcomeTelemetryV2 {
                    phase: "expanding".to_string(),
                    fill_band: "near".to_string(),
                    shape_verdict: "opened".to_string(),
                    ..BTSPOutcomeTelemetryV2::default()
                }),
                ..reconcentrating_outcome(8, similar)
            }],
        ));
        let ledger = ProposalLedger {
            proposals,
            last_updated_unix_s: 0,
        };
        let bank = build_trace_bank_v2(&ledger, None);
        let replay = replay_summary_for(&bank, query).expect("similar replay");
        let anti_loop = anti_loop_state_for(query, Some(&replay)).expect("anti-loop");

        assert_eq!(replay.recovery_widening_count, 1);
        assert!(!replay.overwhelming_reconcentration);
        assert!(!anti_loop.active);
    }

    #[test]
    fn replay_does_not_count_recovery_as_widening_in_large_fixture() {
        let fingerprint = "families=grinding_family;transition=none";
        let outcomes = (0_u64..1_949)
            .map(|index| reconcentrating_outcome(index, fingerprint))
            .collect::<Vec<_>>();
        let classified_outcomes = outcomes.iter().map(outcome_vector_for).collect::<Vec<_>>();
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

    #[test]
    fn sanitized_current_ledger_fixture_has_no_false_widening() {
        let fixture = serde_json::from_str::<CompactLedgerFixture>(include_str!(
            "fixtures/current_ledger_compact_v2.json"
        ))
        .expect("compact fixture");
        assert_eq!(fixture.metadata.proposal_count, 968);
        assert_eq!(fixture.metadata.outcome_count, 1_954);
        assert_eq!(fixture.metadata.fingerprint_count, 48);
        assert_eq!(
            fixture
                .metadata
                .opening_vs_reconcentration_counts
                .get("reconcentrating"),
            Some(&1_954)
        );

        let proposals = fixture
            .proposals
            .iter()
            .map(|proposal| {
                let outcomes = proposal
                    .outcomes
                    .iter()
                    .map(|outcome| ResponseOutcomeNote {
                        proposal_id: proposal.proposal_id.clone(),
                        response_id: outcome.response_id.clone(),
                        owner: outcome.owner.clone(),
                        recorded_at_unix_s: outcome.recorded_at_unix_s,
                        target_nearness: outcome.target_nearness.clone(),
                        distress_or_recovery: outcome.distress_or_recovery.clone(),
                        opening_vs_reconcentration: outcome.opening_vs_reconcentration.clone(),
                        outcome_telemetry_v2: Some(BTSPOutcomeTelemetryV2 {
                            phase: outcome.telemetry.phase.clone(),
                            fill_band: outcome.telemetry.fill_band.clone(),
                            shape_verdict: outcome.telemetry.shape_verdict.clone(),
                            fill_pct: outcome.telemetry.fill_pct,
                            ..BTSPOutcomeTelemetryV2::default()
                        }),
                        note: String::new(),
                    })
                    .collect::<Vec<_>>();
                let mut proposal_record = proposal_with_outcomes(
                    &proposal.proposal_id,
                    &proposal.signal_fingerprint,
                    outcomes,
                );
                proposal_record.matched_signal_families = proposal.matched_signal_families.clone();
                proposal_record.matched_live_signals = proposal.matched_live_signals.clone();
                proposal_record
            })
            .collect::<Vec<_>>();
        let ledger = ProposalLedger {
            proposals,
            last_updated_unix_s: 0,
        };
        let all_vectors = ledger
            .proposals
            .iter()
            .flat_map(|proposal| proposal.outcomes.iter())
            .map(outcome_vector_for)
            .collect::<Vec<_>>();
        let bank = build_trace_bank_v2(&ledger, None);

        assert_eq!(u64::try_from(all_vectors.len()).unwrap_or(0), 1_954);
        assert_eq!(bank.total_outcomes_scanned, 1_954);
        assert!(
            all_vectors
                .iter()
                .all(|outcome| outcome.outcome_class.contains("reconcentrating"))
        );
        assert!(
            all_vectors
                .iter()
                .all(|outcome| outcome.widening_score == 0.0)
        );
    }

    #[test]
    fn live_trace_archive_prune_is_idempotent_after_receipt() {
        let mut live_trace = seed_episode();
        live_trace.episode_id = "btsp_live_trace_old".to_string();
        let mut bank = EpisodeBank {
            episodes: vec![seed_episode(), live_trace],
            last_updated_unix_s: 0,
        };

        let (changed, archive) = archive_and_prune_live_trace_episodes_inner(
            &mut bank,
            BTSPLiveTraceArchiveV2::default(),
            |_| true,
        );

        assert!(changed);
        assert_eq!(archive.entries.len(), 1);
        assert_eq!(archive.entries[0].archived_count, 1);
        assert!(
            bank.episodes
                .iter()
                .all(|episode| !episode.episode_id.starts_with(LIVE_TRACE_PREFIX))
        );

        let (changed_again, archive_again) =
            archive_and_prune_live_trace_episodes_inner(&mut bank, archive, |_| true);
        assert!(!changed_again);
        assert_eq!(archive_again.entries.len(), 1);
    }

    #[test]
    fn live_trace_archive_does_not_prune_when_receipt_write_fails() {
        let mut live_trace = seed_episode();
        live_trace.episode_id = "btsp_live_trace_old".to_string();
        let mut bank = EpisodeBank {
            episodes: vec![seed_episode(), live_trace],
            last_updated_unix_s: 0,
        };

        let (changed, archive) = archive_and_prune_live_trace_episodes_inner(
            &mut bank,
            BTSPLiveTraceArchiveV2::default(),
            |_| false,
        );

        assert!(!changed);
        assert_eq!(archive.entries.len(), 1);
        assert!(
            bank.episodes
                .iter()
                .any(|episode| episode.episode_id.starts_with(LIVE_TRACE_PREFIX))
        );
    }
}
