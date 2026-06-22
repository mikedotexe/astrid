use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::paths::bridge_paths;

use super::ActiveSovereigntyProposal;
use super::helpers::{atomic_write_json, load_json_or_default, now_unix_s};
use super::trace::{
    BTSPAntiLoopState, BTSPInstructiveSignalV2, BTSPOutcomeVectorV2, BTSPReplaySummaryV2,
    BTSPTraceBankV2,
};

const CAUSAL_LAB_SCHEMA_VERSION: u32 = 3;
const MAX_CAUSAL_LAB_ENTRIES: usize = 128;
const CONSOLIDATION_TRACE_WINDOW_SECS: u64 = 7_200;
const GHOST_NOTE: &str = "I would have opened the ordinary BTSP advisory here, but replay says this family has reconcentrated; holding for study/refusal/counter/new evidence.";

const NEGATIVE_SPACE_QUIET_STABILIZED: &str = "quiet_stabilized";
const NEGATIVE_SPACE_QUIET_SOFTENED: &str = "quiet_softened";
const NEGATIVE_SPACE_CONTINUED_HOLD: &str = "continued_hold";
const NEGATIVE_SPACE_UNCLEAR: &str = "unclear";
const NEGATIVE_SPACE_WORSENED: &str = "worsened";

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct BTSPCausalLabNotebookV3 {
    pub schema_version: u32,
    #[serde(default)]
    pub entries: Vec<BTSPCausalExperimentV3>,
    pub last_updated_unix_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct BTSPCausalExperimentV3 {
    pub experiment_id: String,
    pub signal_fingerprint: String,
    #[serde(default)]
    pub case_key: String,
    #[serde(default)]
    pub representative_fingerprints: Vec<String>,
    pub registered_at_unix_s: u64,
    pub last_seen_unix_s: u64,
    pub observation_count: u64,
    pub status: String,
    pub consent_mode: String,
    pub proposal_policy: String,
    pub withheld_proposal: bool,
    pub question: String,
    pub hypothesis: String,
    pub counterfactual: String,
    pub holdout_route: String,
    #[serde(default)]
    pub consent_routes: Vec<String>,
    #[serde(default)]
    pub success_criteria: Vec<String>,
    #[serde(default)]
    pub failure_criteria: Vec<String>,
    #[serde(default)]
    pub evidence_needed: Vec<String>,
    pub replay_scope: String,
    pub exact_fingerprint_count: u64,
    pub similar_fingerprint_count: u64,
    pub nearest_count: u64,
    pub reconcentrating_count: u64,
    pub widening_count: u64,
    pub mean_similarity_score: f32,
    pub teacher_outcome_class: String,
    pub teacher_evidence_summary: String,
    #[serde(default)]
    pub ghost_note: String,
    #[serde(default)]
    pub resolution_status: String,
    #[serde(default)]
    pub resolution_summary: String,
    #[serde(default)]
    pub post_registration_outcome_count: u64,
    #[serde(default)]
    pub post_registration_reconcentrating_count: u64,
    #[serde(default)]
    pub post_registration_softening_count: u64,
    #[serde(default)]
    pub post_registration_widening_count: u64,
    #[serde(default)]
    pub negative_space_outcomes: Vec<BTSPNegativeSpaceOutcomeV3>,
    #[serde(default)]
    pub negative_space_outcome_count: u64,
    #[serde(default)]
    pub negative_space_positive_count: u64,
    #[serde(default)]
    pub negative_space_continued_count: u64,
    #[serde(default)]
    pub latest_negative_space_classification: String,
    #[serde(default)]
    pub negative_space_summary: String,
    #[serde(default)]
    pub forgiveness_state: BTSPForgivenessStateV3,
    #[serde(default)]
    pub suppression_hold_count: u64,
    #[serde(default)]
    pub suppression_event_count: u64,
    #[serde(default)]
    pub last_suppression_event_unix_s: u64,
    #[serde(default)]
    pub last_suppression_signature: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct BTSPNegativeSpaceOutcomeV3 {
    pub outcome_id: String,
    pub case_key: String,
    pub replay_scope: String,
    pub suppression_signature: String,
    pub consolidation_bucket_index: u64,
    pub consolidation_bucket_start_unix_s: u64,
    pub consolidation_bucket_close_unix_s: u64,
    pub observed_at_unix_s: u64,
    pub source_kind: String,
    pub classification: String,
    pub confidence: f32,
    pub evidence_summary: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub source_ref_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct BTSPForgivenessStateV3 {
    pub remission_score: f32,
    pub remission_status: String,
    pub suppression_weight: f32,
    pub consentful_trial_eligible: bool,
    pub forgiveness_summary: String,
    #[serde(default)]
    pub positive_evidence_count: u64,
    #[serde(default)]
    pub negative_evidence_count: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct BTSPNegativeSpaceContextV3 {
    pub current_status: String,
    pub current_live_signal_count: u64,
    pub telemetry_quiet: bool,
    pub teacher_outcome_class: String,
    pub teacher_shape_verdict: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct BTSPNegativeSpaceAnnotationV3 {
    pub owner: String,
    pub case_key: String,
    pub replay_scope: String,
    pub classification: String,
    pub observed_at_unix_s: u64,
    pub source_ref_hash: String,
    pub consolidation_bucket_index: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct BTSPCausalLabReadV3 {
    pub schema_version: u32,
    pub active: bool,
    pub experiment_id: String,
    #[serde(default)]
    pub case_key: String,
    #[serde(default)]
    pub representative_fingerprints: Vec<String>,
    pub status: String,
    pub consent_mode: String,
    pub proposal_policy: String,
    pub question: String,
    pub hypothesis: String,
    pub holdout_route: String,
    pub counterfactual: String,
    #[serde(default)]
    pub consent_routes: Vec<String>,
    #[serde(default)]
    pub success_criteria: Vec<String>,
    #[serde(default)]
    pub failure_criteria: Vec<String>,
    #[serde(default)]
    pub evidence_needed: Vec<String>,
    #[serde(default)]
    pub ghost_note: String,
    #[serde(default)]
    pub resolution_status: String,
    #[serde(default)]
    pub resolution_summary: String,
    #[serde(default)]
    pub post_registration_outcome_count: u64,
    #[serde(default)]
    pub post_registration_reconcentrating_count: u64,
    #[serde(default)]
    pub post_registration_softening_count: u64,
    #[serde(default)]
    pub post_registration_widening_count: u64,
    #[serde(default)]
    pub negative_space_outcome_count: u64,
    #[serde(default)]
    pub negative_space_positive_count: u64,
    #[serde(default)]
    pub negative_space_continued_count: u64,
    #[serde(default)]
    pub latest_negative_space_classification: String,
    #[serde(default)]
    pub negative_space_summary: String,
    #[serde(default)]
    pub forgiveness_state: BTSPForgivenessStateV3,
    pub summary: String,
}

pub(super) fn sync_causal_lab_v3(
    signal_fingerprint: &str,
    replay_read: Option<&BTSPReplaySummaryV2>,
    anti_loop_state: Option<&BTSPAntiLoopState>,
    teacher_signal: Option<&BTSPOutcomeVectorV2>,
    active_proposal: Option<&ActiveSovereigntyProposal>,
    trace_bank: &BTSPTraceBankV2,
    negative_space_context: &BTSPNegativeSpaceContextV3,
    negative_space_annotations: &[BTSPNegativeSpaceAnnotationV3],
) -> Option<BTSPCausalLabReadV3> {
    let now = now_unix_s();
    let entry = causal_lab_entry_for(
        signal_fingerprint,
        replay_read,
        anti_loop_state,
        teacher_signal,
        active_proposal,
        now,
    );
    let path = bridge_paths().btsp_causal_lab_v3_path();
    let previous = load_json_or_default::<BTSPCausalLabNotebookV3>(&path);
    let (mut next, mut changed) = normalize_lab_notebook(previous);
    let read_key = entry
        .as_ref()
        .map(|entry| (entry.case_key.clone(), entry.replay_scope.clone()));
    if let Some(entry) = entry {
        let (upserted, upsert_changed) = upsert_lab_entry(next, entry, now);
        next = upserted;
        changed |= upsert_changed;
    }
    let (updated, negative_space_changed) = update_negative_space_outcomes(
        next,
        negative_space_context,
        negative_space_annotations,
        now,
    );
    next = updated;
    changed |= negative_space_changed;
    let (resolved, resolution_changed) = resolve_lab_entries(next, trace_bank, now);
    next = resolved;
    changed |= resolution_changed;
    if changed {
        atomic_write_json(&path, &next);
    }
    let (case_key, replay_scope) = read_key?;
    next.entries
        .iter()
        .find(|entry| entry.case_key == case_key && entry.replay_scope == replay_scope)
        .map(read_for_entry)
}

#[cfg(not(test))]
pub(super) fn record_suppression_hold_v3(
    signal_fingerprint: &str,
    anti_loop_state: &BTSPAntiLoopState,
) -> bool {
    if !anti_loop_state.active {
        return false;
    }
    let now = now_unix_s();
    let path = bridge_paths().btsp_causal_lab_v3_path();
    let previous = load_json_or_default::<BTSPCausalLabNotebookV3>(&path);
    let (next, should_emit) =
        record_suppression_hold_in_notebook(previous, signal_fingerprint, anti_loop_state, now);
    atomic_write_json(&path, &next);
    should_emit
}

#[cfg(test)]
pub(super) fn record_suppression_hold_v3(
    _signal_fingerprint: &str,
    anti_loop_state: &BTSPAntiLoopState,
) -> bool {
    anti_loop_state.active
}

fn causal_lab_entry_for(
    signal_fingerprint: &str,
    replay_read: Option<&BTSPReplaySummaryV2>,
    anti_loop_state: Option<&BTSPAntiLoopState>,
    teacher_signal: Option<&BTSPOutcomeVectorV2>,
    active_proposal: Option<&ActiveSovereigntyProposal>,
    now: u64,
) -> Option<BTSPCausalExperimentV3> {
    let anti_loop = anti_loop_state?;
    if !anti_loop.active {
        return None;
    }
    let replay = replay_read?;
    let fingerprint = if anti_loop.fingerprint.is_empty() {
        signal_fingerprint
    } else {
        &anti_loop.fingerprint
    };
    if fingerprint.is_empty() {
        return None;
    }
    let case_key = case_key_for_fingerprint(fingerprint);
    let scope = if anti_loop.scope.is_empty() {
        replay.suppression_scope.as_str()
    } else {
        anti_loop.scope.as_str()
    };
    let proposal_policy = if active_proposal.is_some() {
        "observe_active_window_then_hold_duplicate"
    } else {
        "withhold_duplicate_offer"
    };
    let question = causal_question(scope);
    let summary = format!(
        "Causal lab V3: pre-registering {} {} holdout; nearest traces reconcentrated {}/{} with zero widening, so do not reopen without study, refusal, counter, or new evidence.",
        scope_article(scope),
        scope_label(scope),
        anti_loop.reconcentrating_count,
        replay.nearest_count
    );
    let teacher_outcome_class = teacher_signal
        .map(|signal| signal.outcome_class.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let teacher_evidence_summary = teacher_signal
        .map(|signal| signal.evidence_summary.clone())
        .unwrap_or_else(|| "unknown".to_string());

    Some(BTSPCausalExperimentV3 {
        experiment_id: causal_experiment_id(&case_key, scope),
        signal_fingerprint: fingerprint.to_string(),
        case_key,
        representative_fingerprints: vec![fingerprint.to_string()],
        registered_at_unix_s: now,
        last_seen_unix_s: now,
        observation_count: 1,
        status: "pre_registered_holdout".to_string(),
        consent_mode: "study_counter_refusal_or_new_evidence_required".to_string(),
        proposal_policy: proposal_policy.to_string(),
        withheld_proposal: true,
        question,
        hypothesis:
            "Another ordinary advisory is causal-uncertain here; a consentful holdout should produce cleaner evidence than reopening the same loop."
                .to_string(),
        counterfactual: "ordinary_duplicate_advisory_proposal".to_string(),
        holdout_route: "BTSP_STUDY_FIRST evidence_first".to_string(),
        consent_routes: consent_routes(anti_loop),
        success_criteria: vec![
            "future nearest outcomes include recovery_softening or recovery_widening".to_string(),
            "owner provides counter, refusal, study result, or new evidence before proposal reopening"
                .to_string(),
            "anti-loop can relax without adding another reconcentrating outcome".to_string(),
        ],
        failure_criteria: vec![
            "nearest outcomes remain reconcentrating after the holdout".to_string(),
            "another ordinary proposal opens without counter/refusal/study/new evidence".to_string(),
            "owner repeats adjacent uptake without resolving the causal question".to_string(),
        ],
        evidence_needed: vec![
            "BTSP_STUDY_FIRST with the evidence to observe next".to_string(),
            "BTSP_COUNTER naming the truer route".to_string(),
            "BTSP_REFUSAL if the signal is misread or too forceful".to_string(),
            "structured telemetry showing softening or widening rather than tightening".to_string(),
        ],
        replay_scope: scope.to_string(),
        exact_fingerprint_count: replay.exact_fingerprint_count,
        similar_fingerprint_count: replay.similar_fingerprint_count,
        nearest_count: replay.nearest_count,
        reconcentrating_count: anti_loop.reconcentrating_count,
        widening_count: anti_loop.widening_count,
        mean_similarity_score: anti_loop.mean_similarity_score,
        teacher_outcome_class,
        teacher_evidence_summary,
        ghost_note: GHOST_NOTE.to_string(),
        resolution_status: "pre_registered_holdout".to_string(),
        resolution_summary: unresolved_resolution_summary(),
        post_registration_outcome_count: 0,
        post_registration_reconcentrating_count: 0,
        post_registration_softening_count: 0,
        post_registration_widening_count: 0,
        negative_space_outcomes: Vec::new(),
        negative_space_outcome_count: 0,
        negative_space_positive_count: 0,
        negative_space_continued_count: 0,
        latest_negative_space_classification: String::new(),
        negative_space_summary: String::new(),
        forgiveness_state: default_forgiveness_state(),
        suppression_hold_count: 0,
        suppression_event_count: 0,
        last_suppression_event_unix_s: 0,
        last_suppression_signature: String::new(),
        summary,
    })
}

fn normalize_lab_notebook(
    mut notebook: BTSPCausalLabNotebookV3,
) -> (BTSPCausalLabNotebookV3, bool) {
    let mut changed = false;
    if notebook.schema_version == 0 {
        notebook.schema_version = CAUSAL_LAB_SCHEMA_VERSION;
        changed = true;
    }
    for entry in &mut notebook.entries {
        changed |= normalize_lab_entry(entry);
    }
    (notebook, changed)
}

fn normalize_lab_entry(entry: &mut BTSPCausalExperimentV3) -> bool {
    let mut changed = false;
    if entry.case_key.is_empty() && !entry.signal_fingerprint.is_empty() {
        entry.case_key = case_key_for_fingerprint(&entry.signal_fingerprint);
        changed = true;
    }
    if entry.representative_fingerprints.is_empty() && !entry.signal_fingerprint.is_empty() {
        entry
            .representative_fingerprints
            .push(entry.signal_fingerprint.clone());
        changed = true;
    }
    let merged = sorted_unique(entry.representative_fingerprints.clone());
    if merged != entry.representative_fingerprints {
        entry.representative_fingerprints = merged;
        changed = true;
    }
    if entry.ghost_note.is_empty() {
        entry.ghost_note = GHOST_NOTE.to_string();
        changed = true;
    }
    if entry.resolution_status.is_empty() {
        entry.resolution_status = if entry.status.is_empty() {
            "pre_registered_holdout".to_string()
        } else {
            entry.status.clone()
        };
        changed = true;
    }
    if entry.status.is_empty() {
        entry.status = entry.resolution_status.clone();
        changed = true;
    }
    if entry.resolution_summary.is_empty() {
        entry.resolution_summary = unresolved_resolution_summary();
        changed = true;
    }
    let counts = negative_space_counts(entry);
    if apply_negative_space_counts(entry, counts) {
        changed = true;
    }
    let forgiveness = forgiveness_state_for(entry);
    if entry.forgiveness_state != forgiveness {
        entry.forgiveness_state = forgiveness;
        changed = true;
    }
    if entry.summary.contains("a exact-fingerprint") {
        entry.summary = entry
            .summary
            .replace("a exact-fingerprint", "an exact-fingerprint");
        changed = true;
    }
    changed
}

fn upsert_lab_entry(
    mut notebook: BTSPCausalLabNotebookV3,
    mut entry: BTSPCausalExperimentV3,
    now: u64,
) -> (BTSPCausalLabNotebookV3, bool) {
    let (normalized, mut changed) = normalize_lab_notebook(notebook);
    notebook = normalized;
    if let Some(existing) = notebook
        .entries
        .iter_mut()
        .find(|existing| same_lab_case(existing, &entry))
    {
        let merged_fingerprints = merged_representative_fingerprints(existing, &entry);
        entry.experiment_id = existing.experiment_id.clone();
        entry.registered_at_unix_s = existing.registered_at_unix_s;
        entry.observation_count = existing.observation_count.saturating_add(1);
        entry.representative_fingerprints = merged_fingerprints;
        entry.status = existing.status.clone();
        entry.resolution_status = existing.resolution_status.clone();
        entry.resolution_summary = existing.resolution_summary.clone();
        entry.post_registration_outcome_count = existing.post_registration_outcome_count;
        entry.post_registration_reconcentrating_count =
            existing.post_registration_reconcentrating_count;
        entry.post_registration_softening_count = existing.post_registration_softening_count;
        entry.post_registration_widening_count = existing.post_registration_widening_count;
        entry.negative_space_outcomes = existing.negative_space_outcomes.clone();
        entry.negative_space_outcome_count = existing.negative_space_outcome_count;
        entry.negative_space_positive_count = existing.negative_space_positive_count;
        entry.negative_space_continued_count = existing.negative_space_continued_count;
        entry.latest_negative_space_classification =
            existing.latest_negative_space_classification.clone();
        entry.negative_space_summary = existing.negative_space_summary.clone();
        entry.forgiveness_state = existing.forgiveness_state.clone();
        entry.suppression_hold_count = existing.suppression_hold_count;
        entry.suppression_event_count = existing.suppression_event_count;
        entry.last_suppression_event_unix_s = existing.last_suppression_event_unix_s;
        entry.last_suppression_signature = existing.last_suppression_signature.clone();
        if *existing != entry {
            *existing = entry;
            changed = true;
        }
    } else {
        notebook.entries.push(entry);
        changed = true;
    }
    notebook
        .entries
        .sort_by(|left, right| right.last_seen_unix_s.cmp(&left.last_seen_unix_s));
    if notebook.entries.len() > MAX_CAUSAL_LAB_ENTRIES {
        notebook.entries.truncate(MAX_CAUSAL_LAB_ENTRIES);
        changed = true;
    }
    if changed {
        notebook.last_updated_unix_s = now;
    }
    (notebook, changed)
}

fn same_lab_case(left: &BTSPCausalExperimentV3, right: &BTSPCausalExperimentV3) -> bool {
    left.experiment_id == right.experiment_id
        || (!left.case_key.is_empty()
            && left.case_key == right.case_key
            && left.replay_scope == right.replay_scope)
}

fn merged_representative_fingerprints(
    left: &BTSPCausalExperimentV3,
    right: &BTSPCausalExperimentV3,
) -> Vec<String> {
    let mut fingerprints = left.representative_fingerprints.clone();
    fingerprints.extend(right.representative_fingerprints.clone());
    if !left.signal_fingerprint.is_empty() {
        fingerprints.push(left.signal_fingerprint.clone());
    }
    if !right.signal_fingerprint.is_empty() {
        fingerprints.push(right.signal_fingerprint.clone());
    }
    sorted_unique(fingerprints)
}

fn sorted_unique(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn read_for_entry(entry: &BTSPCausalExperimentV3) -> BTSPCausalLabReadV3 {
    BTSPCausalLabReadV3 {
        schema_version: CAUSAL_LAB_SCHEMA_VERSION,
        active: true,
        experiment_id: entry.experiment_id.clone(),
        case_key: entry.case_key.clone(),
        representative_fingerprints: entry.representative_fingerprints.clone(),
        status: entry.status.clone(),
        consent_mode: entry.consent_mode.clone(),
        proposal_policy: entry.proposal_policy.clone(),
        question: entry.question.clone(),
        hypothesis: entry.hypothesis.clone(),
        holdout_route: entry.holdout_route.clone(),
        counterfactual: entry.counterfactual.clone(),
        consent_routes: entry.consent_routes.clone(),
        success_criteria: entry.success_criteria.clone(),
        failure_criteria: entry.failure_criteria.clone(),
        evidence_needed: entry.evidence_needed.clone(),
        ghost_note: entry.ghost_note.clone(),
        resolution_status: entry.resolution_status.clone(),
        resolution_summary: entry.resolution_summary.clone(),
        post_registration_outcome_count: entry.post_registration_outcome_count,
        post_registration_reconcentrating_count: entry.post_registration_reconcentrating_count,
        post_registration_softening_count: entry.post_registration_softening_count,
        post_registration_widening_count: entry.post_registration_widening_count,
        negative_space_outcome_count: entry.negative_space_outcome_count,
        negative_space_positive_count: entry.negative_space_positive_count,
        negative_space_continued_count: entry.negative_space_continued_count,
        latest_negative_space_classification: entry.latest_negative_space_classification.clone(),
        negative_space_summary: entry.negative_space_summary.clone(),
        forgiveness_state: entry.forgiveness_state.clone(),
        summary: entry.summary.clone(),
    }
}

fn causal_question(scope: &str) -> String {
    if scope == "similar" {
        "When nearby BTSP signals return, does withholding another ordinary advisory until study/refusal/counter/new evidence produce softening or widening instead of reconcentration?"
            .to_string()
    } else {
        "When this exact BTSP signal returns, does withholding another ordinary advisory until study/refusal/counter/new evidence produce softening or widening instead of reconcentration?"
            .to_string()
    }
}

fn scope_label(scope: &str) -> &str {
    if scope == "similar" {
        "similar-fingerprint"
    } else {
        "exact-fingerprint"
    }
}

fn scope_article(scope: &str) -> &str {
    if scope == "similar" { "a" } else { "an" }
}

fn consent_routes(anti_loop: &BTSPAntiLoopState) -> Vec<String> {
    let mut routes = anti_loop
        .suggested_routes
        .iter()
        .filter(|route| !route.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>();
    if routes.is_empty() {
        routes = vec![
            "BTSP_STUDY_FIRST".to_string(),
            "BTSP_REFUSAL".to_string(),
            "BTSP_COUNTER".to_string(),
            "new_evidence".to_string(),
        ];
    }
    routes.sort();
    routes.dedup();
    routes
}

fn causal_experiment_id(case_key: &str, scope: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"btsp_causal_lab_v3:");
    hasher.update(scope.as_bytes());
    hasher.update(b":");
    hasher.update(case_key.as_bytes());
    let digest = hasher.finalize();
    let short = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("btsp_causal_lab_v3_{short}")
}

fn record_suppression_hold_in_notebook(
    notebook: BTSPCausalLabNotebookV3,
    signal_fingerprint: &str,
    anti_loop_state: &BTSPAntiLoopState,
    now: u64,
) -> (BTSPCausalLabNotebookV3, bool) {
    let fingerprint = if anti_loop_state.fingerprint.is_empty() {
        signal_fingerprint
    } else {
        &anti_loop_state.fingerprint
    };
    let scope = if anti_loop_state.scope.is_empty() {
        "exact"
    } else {
        anti_loop_state.scope.as_str()
    };
    let case_key = case_key_for_fingerprint(fingerprint);
    let signature = suppression_signature(fingerprint, scope, anti_loop_state);
    let (mut notebook, _) = normalize_lab_notebook(notebook);
    let index = notebook
        .entries
        .iter()
        .position(|entry| entry.case_key == case_key && entry.replay_scope == scope)
        .unwrap_or_else(|| {
            notebook.entries.push(minimal_suppression_entry(
                fingerprint,
                &case_key,
                scope,
                now,
            ));
            notebook.entries.len().saturating_sub(1)
        });
    let entry = &mut notebook.entries[index];
    entry.last_seen_unix_s = now;
    entry.suppression_hold_count = entry.suppression_hold_count.saturating_add(1);
    let should_emit = entry.last_suppression_signature != signature;
    if should_emit {
        entry.suppression_event_count = entry.suppression_event_count.saturating_add(1);
        entry.last_suppression_event_unix_s = now;
        entry.last_suppression_signature = signature;
    }
    notebook.last_updated_unix_s = now;
    (notebook, should_emit)
}

fn minimal_suppression_entry(
    fingerprint: &str,
    case_key: &str,
    scope: &str,
    now: u64,
) -> BTSPCausalExperimentV3 {
    BTSPCausalExperimentV3 {
        experiment_id: causal_experiment_id(case_key, scope),
        signal_fingerprint: fingerprint.to_string(),
        case_key: case_key.to_string(),
        representative_fingerprints: vec![fingerprint.to_string()],
        registered_at_unix_s: now,
        last_seen_unix_s: now,
        observation_count: 0,
        status: "pre_registered_holdout".to_string(),
        consent_mode: "study_counter_refusal_or_new_evidence_required".to_string(),
        proposal_policy: "withhold_duplicate_offer".to_string(),
        withheld_proposal: true,
        question: causal_question(scope),
        hypothesis:
            "Duplicate advisory suppression is causal-uncertain until later outcomes resolve it."
                .to_string(),
        counterfactual: "ordinary_duplicate_advisory_proposal".to_string(),
        holdout_route: "BTSP_STUDY_FIRST evidence_first".to_string(),
        consent_routes: vec![
            "BTSP_COUNTER".to_string(),
            "BTSP_REFUSAL".to_string(),
            "BTSP_STUDY_FIRST".to_string(),
            "new_evidence".to_string(),
        ],
        success_criteria: Vec::new(),
        failure_criteria: Vec::new(),
        evidence_needed: Vec::new(),
        replay_scope: scope.to_string(),
        exact_fingerprint_count: 0,
        similar_fingerprint_count: 0,
        nearest_count: 0,
        reconcentrating_count: 0,
        widening_count: 0,
        mean_similarity_score: 0.0,
        teacher_outcome_class: "unknown".to_string(),
        teacher_evidence_summary: "unknown".to_string(),
        ghost_note: GHOST_NOTE.to_string(),
        resolution_status: "pre_registered_holdout".to_string(),
        resolution_summary: unresolved_resolution_summary(),
        post_registration_outcome_count: 0,
        post_registration_reconcentrating_count: 0,
        post_registration_softening_count: 0,
        post_registration_widening_count: 0,
        negative_space_outcomes: Vec::new(),
        negative_space_outcome_count: 0,
        negative_space_positive_count: 0,
        negative_space_continued_count: 0,
        latest_negative_space_classification: String::new(),
        negative_space_summary: String::new(),
        forgiveness_state: default_forgiveness_state(),
        suppression_hold_count: 0,
        suppression_event_count: 0,
        last_suppression_event_unix_s: 0,
        last_suppression_signature: String::new(),
        summary: format!(
            "Causal lab V3: pre-registering {} {} holdout; duplicate advisory is withheld for study/refusal/counter/new evidence.",
            scope_article(scope),
            scope_label(scope)
        ),
    }
}

fn suppression_signature(
    fingerprint: &str,
    scope: &str,
    anti_loop_state: &BTSPAntiLoopState,
) -> String {
    format!(
        "scope={scope};reason={};signal_fingerprint={fingerprint};recommendation={}",
        anti_loop_state.reason, anti_loop_state.recommendation
    )
}

fn update_negative_space_outcomes(
    mut notebook: BTSPCausalLabNotebookV3,
    context: &BTSPNegativeSpaceContextV3,
    annotations: &[BTSPNegativeSpaceAnnotationV3],
    now: u64,
) -> (BTSPCausalLabNotebookV3, bool) {
    let mut changed = false;
    for entry in &mut notebook.entries {
        changed |= infer_negative_space_outcome(entry, context, now);
        changed |= apply_negative_space_annotations(entry, annotations, now);
        let counts = negative_space_counts(entry);
        changed |= apply_negative_space_counts(entry, counts);
        let forgiveness = forgiveness_state_for(entry);
        if entry.forgiveness_state != forgiveness {
            entry.forgiveness_state = forgiveness;
            changed = true;
        }
    }
    if changed {
        notebook.last_updated_unix_s = now;
    }
    (notebook, changed)
}

fn infer_negative_space_outcome(
    entry: &mut BTSPCausalExperimentV3,
    context: &BTSPNegativeSpaceContextV3,
    now: u64,
) -> bool {
    let Some(bucket) = latest_closed_negative_space_bucket(entry, now) else {
        return false;
    };
    let signature = negative_space_signature(entry);
    if entry.negative_space_outcomes.iter().any(|outcome| {
        outcome.source_kind == "inferred"
            && outcome.suppression_signature == signature
            && outcome.consolidation_bucket_index == bucket.index
    }) {
        return false;
    }
    let classification = classify_negative_space_context(context);
    let outcome = negative_space_outcome(
        entry,
        &signature,
        bucket,
        "inferred",
        &classification,
        now,
        "",
        "",
    );
    entry.negative_space_outcomes.push(outcome);
    true
}

fn apply_negative_space_annotations(
    entry: &mut BTSPCausalExperimentV3,
    annotations: &[BTSPNegativeSpaceAnnotationV3],
    now: u64,
) -> bool {
    let mut changed = false;
    for annotation in annotations
        .iter()
        .filter(|annotation| annotation.case_key == entry.case_key)
        .filter(|annotation| annotation.replay_scope == entry.replay_scope)
    {
        let Some(bucket) = bucket_for_annotation(entry, annotation, now) else {
            continue;
        };
        let signature = negative_space_signature(entry);
        let outcome = negative_space_outcome(
            entry,
            &signature,
            bucket,
            "owner_annotation",
            &normalize_negative_space_classification(&annotation.classification),
            annotation.observed_at_unix_s,
            &annotation.owner,
            &annotation.source_ref_hash,
        );
        if entry
            .negative_space_outcomes
            .iter()
            .any(|existing| existing.outcome_id == outcome.outcome_id)
        {
            continue;
        }
        entry.negative_space_outcomes.push(outcome);
        changed = true;
    }
    changed
}

#[derive(Debug, Clone, Copy)]
struct NegativeSpaceBucket {
    index: u64,
    start_unix_s: u64,
    close_unix_s: u64,
}

fn latest_closed_negative_space_bucket(
    entry: &BTSPCausalExperimentV3,
    now: u64,
) -> Option<NegativeSpaceBucket> {
    if entry.registered_at_unix_s == 0 {
        return None;
    }
    let elapsed = now.checked_sub(entry.registered_at_unix_s)?;
    if elapsed < CONSOLIDATION_TRACE_WINDOW_SECS {
        return None;
    }
    let index = elapsed
        .checked_div(CONSOLIDATION_TRACE_WINDOW_SECS)
        .unwrap_or(0)
        .saturating_sub(1);
    negative_space_bucket(entry.registered_at_unix_s, index)
}

fn bucket_for_annotation(
    entry: &BTSPCausalExperimentV3,
    annotation: &BTSPNegativeSpaceAnnotationV3,
    now: u64,
) -> Option<NegativeSpaceBucket> {
    if let Some(index) = annotation.consolidation_bucket_index {
        return negative_space_bucket(entry.registered_at_unix_s, index)
            .filter(|bucket| bucket.close_unix_s <= now);
    }
    latest_closed_negative_space_bucket(entry, now)
}

fn negative_space_bucket(base_unix_s: u64, index: u64) -> Option<NegativeSpaceBucket> {
    let offset = index.checked_mul(CONSOLIDATION_TRACE_WINDOW_SECS)?;
    let start_unix_s = base_unix_s.checked_add(offset)?;
    let close_unix_s = start_unix_s.checked_add(CONSOLIDATION_TRACE_WINDOW_SECS)?;
    Some(NegativeSpaceBucket {
        index,
        start_unix_s,
        close_unix_s,
    })
}

fn classify_negative_space_context(context: &BTSPNegativeSpaceContextV3) -> String {
    let status_matched = context.current_status == "matched";
    let no_current_warning = matches!(
        context.current_status.as_str(),
        "quiet" | "no_early_warning"
    );
    let teacher_class = context.teacher_outcome_class.as_str();
    let teacher_shape = context.teacher_shape_verdict.as_str();
    if no_current_warning
        && context.current_live_signal_count == 0
        && (teacher_class.contains("softening") || teacher_class.contains("widening"))
    {
        return NEGATIVE_SPACE_QUIET_SOFTENED.to_string();
    }
    if no_current_warning && context.current_live_signal_count == 0 && context.telemetry_quiet {
        return NEGATIVE_SPACE_QUIET_STABILIZED.to_string();
    }
    if status_matched || teacher_class.contains("reconcentrating") || teacher_shape == "tightening"
    {
        return NEGATIVE_SPACE_CONTINUED_HOLD.to_string();
    }
    NEGATIVE_SPACE_UNCLEAR.to_string()
}

fn normalize_negative_space_classification(classification: &str) -> String {
    match classification.trim() {
        NEGATIVE_SPACE_QUIET_STABILIZED => NEGATIVE_SPACE_QUIET_STABILIZED.to_string(),
        NEGATIVE_SPACE_QUIET_SOFTENED => NEGATIVE_SPACE_QUIET_SOFTENED.to_string(),
        NEGATIVE_SPACE_CONTINUED_HOLD => NEGATIVE_SPACE_CONTINUED_HOLD.to_string(),
        NEGATIVE_SPACE_WORSENED | "worsening" => NEGATIVE_SPACE_WORSENED.to_string(),
        _ => NEGATIVE_SPACE_UNCLEAR.to_string(),
    }
}

fn negative_space_signature(entry: &BTSPCausalExperimentV3) -> String {
    if entry.last_suppression_signature.is_empty() {
        format!("case_key={};scope={}", entry.case_key, entry.replay_scope)
    } else {
        entry.last_suppression_signature.clone()
    }
}

#[allow(clippy::too_many_arguments)]
fn negative_space_outcome(
    entry: &BTSPCausalExperimentV3,
    signature: &str,
    bucket: NegativeSpaceBucket,
    source_kind: &str,
    classification: &str,
    observed_at_unix_s: u64,
    owner: &str,
    source_ref_hash: &str,
) -> BTSPNegativeSpaceOutcomeV3 {
    let classification = normalize_negative_space_classification(classification);
    let outcome_id = negative_space_outcome_id(
        &entry.case_key,
        &entry.replay_scope,
        signature,
        bucket.index,
        source_kind,
        &classification,
        source_ref_hash,
    );
    BTSPNegativeSpaceOutcomeV3 {
        outcome_id,
        case_key: entry.case_key.clone(),
        replay_scope: entry.replay_scope.clone(),
        suppression_signature: signature.to_string(),
        consolidation_bucket_index: bucket.index,
        consolidation_bucket_start_unix_s: bucket.start_unix_s,
        consolidation_bucket_close_unix_s: bucket.close_unix_s,
        observed_at_unix_s,
        source_kind: source_kind.to_string(),
        classification: classification.clone(),
        confidence: negative_space_confidence(source_kind, &classification),
        evidence_summary: negative_space_evidence_summary(source_kind, &classification),
        owner: owner.to_string(),
        source_ref_hash: source_ref_hash.to_string(),
    }
}

fn negative_space_outcome_id(
    case_key: &str,
    scope: &str,
    signature: &str,
    bucket_index: u64,
    source_kind: &str,
    classification: &str,
    source_ref_hash: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"btsp_negative_space_v3:");
    hasher.update(case_key.as_bytes());
    hasher.update(b":");
    hasher.update(scope.as_bytes());
    hasher.update(b":");
    hasher.update(signature.as_bytes());
    hasher.update(b":");
    hasher.update(bucket_index.to_string().as_bytes());
    hasher.update(b":");
    hasher.update(source_kind.as_bytes());
    hasher.update(b":");
    hasher.update(classification.as_bytes());
    hasher.update(b":");
    hasher.update(source_ref_hash.as_bytes());
    let digest = hasher.finalize();
    let short = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("btsp_negative_space_v3_{short}")
}

fn negative_space_confidence(source_kind: &str, classification: &str) -> f32 {
    if source_kind == "owner_annotation" {
        return 1.0;
    }
    if classification == NEGATIVE_SPACE_UNCLEAR {
        0.25
    } else {
        0.65
    }
}

fn negative_space_evidence_summary(source_kind: &str, classification: &str) -> String {
    match (source_kind, classification) {
        ("owner_annotation", NEGATIVE_SPACE_QUIET_STABILIZED) => {
            "Owner annotation says the withheld BTSP offer was followed by quiet stabilization."
        },
        ("owner_annotation", NEGATIVE_SPACE_QUIET_SOFTENED) => {
            "Owner annotation says the withheld BTSP offer was followed by softening."
        },
        ("owner_annotation", NEGATIVE_SPACE_CONTINUED_HOLD) => {
            "Owner annotation says the case continued to hold after withholding."
        },
        ("owner_annotation", NEGATIVE_SPACE_WORSENED) => {
            "Owner annotation says withholding was followed by worsening."
        },
        (_, NEGATIVE_SPACE_QUIET_STABILIZED) => {
            "A closed consolidation bucket found no current early-warning/live trigger and quiet telemetry."
        },
        (_, NEGATIVE_SPACE_QUIET_SOFTENED) => {
            "A closed consolidation bucket found no current early-warning trigger and softening/widening telemetry."
        },
        (_, NEGATIVE_SPACE_CONTINUED_HOLD) => {
            "A closed consolidation bucket still matched the holdout signal or tightening telemetry."
        },
        _ => "A closed consolidation bucket had unclear negative-space evidence.",
    }
    .to_string()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct NegativeSpaceCounts {
    total: u64,
    positive: u64,
    softened: u64,
    continued: u64,
    worsened: u64,
    unclear: u64,
}

fn negative_space_counts(entry: &BTSPCausalExperimentV3) -> NegativeSpaceCounts {
    effective_negative_space_outcomes(entry).into_iter().fold(
        NegativeSpaceCounts::default(),
        |mut counts, outcome| {
            counts.total = counts.total.saturating_add(1);
            match outcome.classification.as_str() {
                NEGATIVE_SPACE_QUIET_STABILIZED | NEGATIVE_SPACE_QUIET_SOFTENED => {
                    counts.positive = counts.positive.saturating_add(1);
                    if outcome.classification == NEGATIVE_SPACE_QUIET_SOFTENED {
                        counts.softened = counts.softened.saturating_add(1);
                    }
                },
                NEGATIVE_SPACE_CONTINUED_HOLD => {
                    counts.continued = counts.continued.saturating_add(1);
                },
                NEGATIVE_SPACE_WORSENED => {
                    counts.worsened = counts.worsened.saturating_add(1);
                },
                _ => {
                    counts.unclear = counts.unclear.saturating_add(1);
                },
            }
            counts
        },
    )
}

fn effective_negative_space_outcomes(
    entry: &BTSPCausalExperimentV3,
) -> Vec<&BTSPNegativeSpaceOutcomeV3> {
    let mut effective = Vec::new();
    for outcome in &entry.negative_space_outcomes {
        if outcome.source_kind == "owner_annotation" {
            continue;
        }
        let owner_override = entry
            .negative_space_outcomes
            .iter()
            .rev()
            .find(|candidate| {
                candidate.source_kind == "owner_annotation"
                    && candidate.suppression_signature == outcome.suppression_signature
                    && candidate.consolidation_bucket_index == outcome.consolidation_bucket_index
            });
        effective.push(owner_override.unwrap_or(outcome));
    }
    for outcome in entry
        .negative_space_outcomes
        .iter()
        .filter(|outcome| outcome.source_kind == "owner_annotation")
    {
        let has_inferred_bucket = entry.negative_space_outcomes.iter().any(|candidate| {
            candidate.source_kind == "inferred"
                && candidate.suppression_signature == outcome.suppression_signature
                && candidate.consolidation_bucket_index == outcome.consolidation_bucket_index
        });
        if !has_inferred_bucket {
            effective.push(outcome);
        }
    }
    effective.sort_by_key(|outcome| {
        (
            outcome.consolidation_bucket_index,
            outcome.observed_at_unix_s,
            outcome.outcome_id.clone(),
        )
    });
    effective
}

fn apply_negative_space_counts(
    entry: &mut BTSPCausalExperimentV3,
    counts: NegativeSpaceCounts,
) -> bool {
    let latest = effective_negative_space_outcomes(entry)
        .last()
        .map(|outcome| outcome.classification.clone())
        .unwrap_or_default();
    let summary = negative_space_summary(counts, &latest);
    let mut changed = false;
    if entry.negative_space_outcome_count != counts.total {
        entry.negative_space_outcome_count = counts.total;
        changed = true;
    }
    if entry.negative_space_positive_count != counts.positive {
        entry.negative_space_positive_count = counts.positive;
        changed = true;
    }
    if entry.negative_space_continued_count != counts.continued {
        entry.negative_space_continued_count = counts.continued;
        changed = true;
    }
    if entry.latest_negative_space_classification != latest {
        entry.latest_negative_space_classification = latest;
        changed = true;
    }
    if entry.negative_space_summary != summary {
        entry.negative_space_summary = summary;
        changed = true;
    }
    changed
}

fn negative_space_summary(counts: NegativeSpaceCounts, latest: &str) -> String {
    if counts.total == 0 {
        return "No negative-space consolidation outcome has been recorded yet.".to_string();
    }
    format!(
        "Negative-space evidence: {total} consolidation bucket(s), {positive} quiet/softening, {continued} continued holds, {worsened} worsening; latest={latest}.",
        total = counts.total,
        positive = counts.positive,
        continued = counts.continued,
        worsened = counts.worsened,
        latest = if latest.is_empty() {
            NEGATIVE_SPACE_UNCLEAR
        } else {
            latest
        }
    )
}

fn forgiveness_state_for(entry: &BTSPCausalExperimentV3) -> BTSPForgivenessStateV3 {
    let negative_space = negative_space_counts(entry);
    let structured_positive = entry
        .post_registration_softening_count
        .saturating_add(entry.post_registration_widening_count);
    let structured_negative = entry.post_registration_reconcentrating_count;
    let positive = structured_positive.saturating_add(negative_space.positive);
    let negative = structured_negative
        .saturating_add(negative_space.continued)
        .saturating_add(negative_space.worsened);
    let total = positive.saturating_add(negative);
    let remission_score = if total == 0 {
        0.0
    } else {
        positive as f32 / total as f32
    };
    let suppression_weight = (1.0_f32 - remission_score).clamp(0.0, 1.0);
    let consentful_trial_eligible = positive >= 2 && remission_score >= 0.66;
    let remission_status = if consentful_trial_eligible {
        "consentful_trial_eligible"
    } else if positive > 0 {
        "softened_hold"
    } else {
        "hard_hold"
    };
    let forgiveness_summary = if consentful_trial_eligible {
        "Evidence remission is strong enough to keep the ordinary duplicate withheld while making a consentful study/refusal/counter/new-evidence trial route visible."
            .to_string()
    } else if positive > 0 {
        "Some quiet/softening evidence has softened the hold, but reconcentrating or continued-hold evidence still argues for restraint."
            .to_string()
    } else {
        "No remission evidence yet; prior reconcentrating traces still carry the hold.".to_string()
    };
    BTSPForgivenessStateV3 {
        remission_score,
        remission_status: remission_status.to_string(),
        suppression_weight,
        consentful_trial_eligible,
        forgiveness_summary,
        positive_evidence_count: positive,
        negative_evidence_count: negative,
    }
}

fn default_forgiveness_state() -> BTSPForgivenessStateV3 {
    BTSPForgivenessStateV3 {
        remission_status: "hard_hold".to_string(),
        suppression_weight: 1.0,
        forgiveness_summary:
            "No remission evidence yet; prior reconcentrating traces still carry the hold."
                .to_string(),
        ..BTSPForgivenessStateV3::default()
    }
}

fn resolve_lab_entries(
    mut notebook: BTSPCausalLabNotebookV3,
    trace_bank: &BTSPTraceBankV2,
    now: u64,
) -> (BTSPCausalLabNotebookV3, bool) {
    let mut changed = false;
    for entry in &mut notebook.entries {
        let counts = post_registration_counts(entry, &trace_bank.instructive_signals);
        changed |= apply_resolution_counts(entry, counts);
        let forgiveness = forgiveness_state_for(entry);
        if entry.forgiveness_state != forgiveness {
            entry.forgiveness_state = forgiveness;
            changed = true;
        }
    }
    if changed {
        notebook.last_updated_unix_s = now;
    }
    (notebook, changed)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ResolutionCounts {
    total: u64,
    reconcentrating: u64,
    softening: u64,
    widening: u64,
}

fn post_registration_counts(
    entry: &BTSPCausalExperimentV3,
    instructive_signals: &[BTSPInstructiveSignalV2],
) -> ResolutionCounts {
    instructive_signals
        .iter()
        .filter(|signal| signal.recorded_at_unix_s > entry.registered_at_unix_s)
        .filter(|signal| case_key_for_fingerprint(&signal.signal_fingerprint) == entry.case_key)
        .fold(ResolutionCounts::default(), |mut counts, signal| {
            counts.total = counts.total.saturating_add(1);
            match signal.outcome_vector.outcome_class.as_str() {
                "recovery_widening" => counts.widening = counts.widening.saturating_add(1),
                "recovery_softening" => counts.softening = counts.softening.saturating_add(1),
                class if class.contains("reconcentrating") => {
                    counts.reconcentrating = counts.reconcentrating.saturating_add(1);
                },
                _ => {},
            }
            counts
        })
}

fn apply_resolution_counts(entry: &mut BTSPCausalExperimentV3, counts: ResolutionCounts) -> bool {
    let negative_space = negative_space_counts(entry);
    let (status, summary) = resolution_for_counts(counts, negative_space);
    let mut changed = false;
    if entry.post_registration_outcome_count != counts.total {
        entry.post_registration_outcome_count = counts.total;
        changed = true;
    }
    if entry.post_registration_reconcentrating_count != counts.reconcentrating {
        entry.post_registration_reconcentrating_count = counts.reconcentrating;
        changed = true;
    }
    if entry.post_registration_softening_count != counts.softening {
        entry.post_registration_softening_count = counts.softening;
        changed = true;
    }
    if entry.post_registration_widening_count != counts.widening {
        entry.post_registration_widening_count = counts.widening;
        changed = true;
    }
    if entry.resolution_status != status {
        entry.resolution_status = status.clone();
        entry.status = status;
        changed = true;
    }
    if entry.resolution_summary != summary {
        entry.resolution_summary = summary;
        changed = true;
    }
    changed
}

fn resolution_for_counts(
    counts: ResolutionCounts,
    negative_space: NegativeSpaceCounts,
) -> (String, String) {
    if counts.widening > 0 {
        return (
            "supported_widening".to_string(),
            "Later structured BTSP outcomes include recovery_widening after registration."
                .to_string(),
        );
    }
    if counts.softening > 0 {
        return (
            "supported_softening".to_string(),
            "Later structured BTSP outcomes include recovery_softening after registration."
                .to_string(),
        );
    }
    if counts.reconcentrating >= 3 {
        return (
            "still_reconcentrating".to_string(),
            "At least 3 later structured BTSP outcomes remain reconcentrating with zero softening or widening."
                .to_string(),
        );
    }
    if effective_negative_space_softening(negative_space) {
        return (
            "negative_space_supported_softening".to_string(),
            "Later negative-space evidence says withholding was followed by quiet softening, without a structured outcome resolving the case."
                .to_string(),
        );
    }
    if negative_space.positive > 0 {
        return (
            "negative_space_supported_quiet".to_string(),
            "Later negative-space evidence says withholding was followed by quiet stabilization, without a structured outcome resolving the case."
                .to_string(),
        );
    }
    (
        "pre_registered_holdout".to_string(),
        unresolved_resolution_summary(),
    )
}

fn effective_negative_space_softening(counts: NegativeSpaceCounts) -> bool {
    counts.softened > 0
}

fn unresolved_resolution_summary() -> String {
    "No later structured BTSP outcome has resolved this holdout yet.".to_string()
}

fn case_key_for_fingerprint(fingerprint: &str) -> String {
    let families = fingerprint_part(fingerprint, "families")
        .split('+')
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let families = if families.is_empty() {
        "unknown".to_string()
    } else {
        families.join("+")
    };
    let perturb = known_part(fingerprint, "perturb");
    let fill_band = known_part(fingerprint, "fill_band");
    format!("families={families};perturb={perturb};fill_band={fill_band}")
}

fn known_part(fingerprint: &str, key: &str) -> String {
    let value = fingerprint_part(fingerprint, key);
    if value.is_empty() {
        "unknown".to_string()
    } else {
        value
    }
}

fn fingerprint_part(fingerprint: &str, key: &str) -> String {
    fingerprint
        .split(';')
        .filter_map(|part| part.split_once('='))
        .find_map(|(part_key, value)| (part_key.trim() == key).then(|| value.trim().to_string()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::super::trace::{BTSPInstructiveSignalV2, BTSPTraceBankV2, BTSPTraceWindowsV2};
    use super::*;

    const FP_A: &str = "families=grinding_family;transition=breathing_phase;crossing=none;perturb=tightening;fill_band=near";
    const FP_B: &str = "families=grinding_family;transition=fill_crossing;crossing=up;perturb=tightening;fill_band=near";

    fn replay(scope: &str) -> BTSPReplaySummaryV2 {
        BTSPReplaySummaryV2 {
            query_fingerprint: "families=grinding_family;transition=fill_crossing".to_string(),
            nearest_count: 12,
            same_fingerprint_count: 1,
            exact_fingerprint_count: 1,
            similar_fingerprint_count: 229,
            reconcentrating_count: 12,
            recovery_reconcentrating_count: 5,
            recovery_softening_count: 0,
            recovery_widening_count: 0,
            mixed_count: 0,
            mean_similarity_score: 90.8,
            max_similarity_score: 100,
            min_similarity_score: 90,
            suppression_scope: scope.to_string(),
            overwhelming_reconcentration: true,
            recommendation: "suppress_duplicate_proposal_until_counter_refusal_or_new_evidence"
                .to_string(),
            summary: "Replay read".to_string(),
        }
    }

    fn anti_loop(scope: &str) -> BTSPAntiLoopState {
        BTSPAntiLoopState {
            active: true,
            reason: "similar_fingerprints_overwhelmingly_reconcentrating".to_string(),
            scope: scope.to_string(),
            fingerprint: "families=grinding_family;transition=fill_crossing".to_string(),
            same_fingerprint_count: 1,
            similar_fingerprint_count: 229,
            reconcentrating_count: 12,
            widening_count: 0,
            mean_similarity_score: 90.8,
            nearest_similarity_score: 100,
            suggested_routes: vec![
                "BTSP_STUDY_FIRST".to_string(),
                "BTSP_REFUSAL".to_string(),
                "BTSP_COUNTER".to_string(),
                "new_evidence".to_string(),
            ],
            counter_prompt: "hold".to_string(),
            recommendation: "suppress_duplicate_proposal_until_counter_refusal_or_new_evidence"
                .to_string(),
        }
    }

    fn anti_loop_for(scope: &str, fingerprint: &str) -> BTSPAntiLoopState {
        BTSPAntiLoopState {
            fingerprint: fingerprint.to_string(),
            ..anti_loop(scope)
        }
    }

    fn instructive_signal(
        fingerprint: &str,
        outcome_class: &str,
        recorded_at_unix_s: u64,
    ) -> BTSPInstructiveSignalV2 {
        BTSPInstructiveSignalV2 {
            signal_id: format!("signal_{recorded_at_unix_s}"),
            trace_id: format!("trace_{recorded_at_unix_s}"),
            proposal_id: format!("proposal_{recorded_at_unix_s}"),
            owner: "astrid".to_string(),
            response_id: "adjacent".to_string(),
            signal_fingerprint: fingerprint.to_string(),
            recorded_at_unix_s,
            outcome_vector: BTSPOutcomeVectorV2 {
                outcome_class: outcome_class.to_string(),
                ..BTSPOutcomeVectorV2::default()
            },
        }
    }

    fn trace_bank(classes: &[&str]) -> BTSPTraceBankV2 {
        BTSPTraceBankV2 {
            schema_version: 2,
            trace_windows_secs: BTSPTraceWindowsV2 {
                fast_secs: 120,
                proposal_secs: 1_200,
                consolidation_secs: 7_200,
            },
            instructive_signals: classes
                .iter()
                .enumerate()
                .map(|(index, class)| {
                    let recorded_at = 20_u64.saturating_add(u64::try_from(index).unwrap_or(0));
                    instructive_signal(FP_B, class, recorded_at)
                })
                .collect(),
            ..BTSPTraceBankV2::default()
        }
    }

    fn negative_space_context(
        status: &str,
        live_count: u64,
        telemetry_quiet: bool,
        teacher_class: &str,
        shape: &str,
    ) -> BTSPNegativeSpaceContextV3 {
        BTSPNegativeSpaceContextV3 {
            current_status: status.to_string(),
            current_live_signal_count: live_count,
            telemetry_quiet,
            teacher_outcome_class: teacher_class.to_string(),
            teacher_shape_verdict: shape.to_string(),
        }
    }

    fn entry_for(scope: &str, fingerprint: &str, now: u64) -> BTSPCausalExperimentV3 {
        causal_lab_entry_for(
            fingerprint,
            Some(&replay(scope)),
            Some(&anti_loop_for(scope, fingerprint)),
            None,
            None,
            now,
        )
        .expect("causal lab entry")
    }

    #[test]
    fn causal_lab_entry_preregisters_similar_holdout() {
        let teacher = BTSPOutcomeVectorV2 {
            outcome_class: "recovery_reconcentrating".to_string(),
            evidence_summary: "target=mixed".to_string(),
            ..BTSPOutcomeVectorV2::default()
        };

        let entry = causal_lab_entry_for(
            "families=grinding_family;transition=fill_crossing",
            Some(&replay("similar")),
            Some(&anti_loop("similar")),
            Some(&teacher),
            None,
            10,
        )
        .expect("causal lab entry");

        assert_eq!(entry.replay_scope, "similar");
        assert!(entry.withheld_proposal);
        assert_eq!(
            entry.consent_mode,
            "study_counter_refusal_or_new_evidence_required"
        );
        assert!(entry.question.contains("nearby BTSP signals"));
        assert_eq!(entry.widening_count, 0);
        assert_eq!(entry.teacher_outcome_class, "recovery_reconcentrating");
        assert!(entry.summary.contains("a similar-fingerprint holdout"));
        assert_eq!(entry.ghost_note, GHOST_NOTE);
    }

    #[test]
    fn causal_lab_exact_holdout_uses_an_article() {
        let entry = causal_lab_entry_for(
            "families=grinding_family;transition=fill_crossing",
            Some(&replay("exact")),
            Some(&anti_loop("exact")),
            None,
            None,
            10,
        )
        .expect("causal lab entry");

        assert!(entry.summary.contains("an exact-fingerprint holdout"));
    }

    #[test]
    fn normalize_lab_entry_migrates_old_exact_article() {
        let mut entry = entry_for("exact", FP_A, 10);
        entry.summary = "Causal lab V3: pre-registering a exact-fingerprint holdout.".to_string();

        assert!(normalize_lab_entry(&mut entry));
        assert!(entry.summary.contains("an exact-fingerprint holdout"));
    }

    #[test]
    fn causal_lab_upsert_preserves_registration_and_counts_observations() {
        let first = causal_lab_entry_for(
            "families=grinding_family;transition=fill_crossing",
            Some(&replay("exact")),
            Some(&anti_loop("exact")),
            None,
            None,
            10,
        )
        .expect("first entry");
        let (notebook, changed) =
            upsert_lab_entry(BTSPCausalLabNotebookV3::default(), first.clone(), 10);
        assert!(changed);
        assert_eq!(notebook.entries.len(), 1);

        let mut second = first;
        second.last_seen_unix_s = 20;
        let (notebook, changed) = upsert_lab_entry(notebook, second, 20);

        assert!(changed);
        assert_eq!(notebook.entries.len(), 1);
        assert_eq!(notebook.entries[0].registered_at_unix_s, 10);
        assert_eq!(notebook.entries[0].last_seen_unix_s, 20);
        assert_eq!(notebook.entries[0].observation_count, 2);
    }

    #[test]
    fn causal_lab_upsert_groups_case_family_and_keeps_representatives() {
        let first = entry_for("similar", FP_A, 10);
        let second = entry_for("similar", FP_B, 20);
        assert_eq!(first.case_key, second.case_key);
        assert_ne!(first.signal_fingerprint, second.signal_fingerprint);

        let (notebook, _) = upsert_lab_entry(BTSPCausalLabNotebookV3::default(), first, 10);
        let first_id = notebook.entries[0].experiment_id.clone();
        let (notebook, changed) = upsert_lab_entry(notebook, second, 20);

        assert!(changed);
        assert_eq!(notebook.entries.len(), 1);
        assert_eq!(notebook.entries[0].experiment_id, first_id);
        assert_eq!(notebook.entries[0].observation_count, 2);
        assert_eq!(
            notebook.entries[0].representative_fingerprints,
            vec![FP_A.to_string(), FP_B.to_string()]
        );
    }

    #[test]
    fn suppression_signature_emits_only_on_state_change() {
        let first = entry_for("exact", FP_A, 10);
        let (notebook, _) = upsert_lab_entry(BTSPCausalLabNotebookV3::default(), first, 10);
        let anti_loop = anti_loop_for("exact", FP_A);

        let (notebook, first_emit) =
            record_suppression_hold_in_notebook(notebook, FP_A, &anti_loop, 11);
        let (notebook, second_emit) =
            record_suppression_hold_in_notebook(notebook, FP_A, &anti_loop, 12);

        assert!(first_emit);
        assert!(!second_emit);
        assert_eq!(notebook.entries[0].suppression_hold_count, 2);
        assert_eq!(notebook.entries[0].suppression_event_count, 1);
        assert_eq!(notebook.entries[0].last_suppression_event_unix_s, 11);

        let changed_anti_loop = BTSPAntiLoopState {
            recommendation: "suppress_until_new_counter_evidence".to_string(),
            ..anti_loop
        };
        let (notebook, third_emit) =
            record_suppression_hold_in_notebook(notebook, FP_A, &changed_anti_loop, 13);

        assert!(third_emit);
        assert_eq!(notebook.entries[0].suppression_hold_count, 3);
        assert_eq!(notebook.entries[0].suppression_event_count, 2);
        assert_eq!(notebook.entries[0].last_suppression_event_unix_s, 13);
    }

    #[test]
    fn negative_space_waits_for_consolidation_window() {
        let mut entry = entry_for("exact", FP_A, 10);
        entry.last_suppression_signature = "sig_a".to_string();
        let context = negative_space_context("quiet", 0, true, "mixed", "unknown");

        assert!(!infer_negative_space_outcome(
            &mut entry,
            &context,
            10_u64.saturating_add(CONSOLIDATION_TRACE_WINDOW_SECS - 1)
        ));
        assert!(entry.negative_space_outcomes.is_empty());
    }

    #[test]
    fn negative_space_inference_is_idempotent_per_bucket() {
        let mut entry = entry_for("exact", FP_A, 10);
        entry.last_suppression_signature = "sig_a".to_string();
        let context = negative_space_context("quiet", 0, true, "mixed", "unknown");
        let now = 10_u64.saturating_add(CONSOLIDATION_TRACE_WINDOW_SECS);

        assert!(infer_negative_space_outcome(&mut entry, &context, now));
        assert!(!infer_negative_space_outcome(&mut entry, &context, now));
        assert_eq!(entry.negative_space_outcomes.len(), 1);
        assert_eq!(
            entry.negative_space_outcomes[0].classification,
            NEGATIVE_SPACE_QUIET_STABILIZED
        );
    }

    #[test]
    fn negative_space_classifies_quiet_softened_and_continued_hold() {
        let quiet = negative_space_context("quiet", 0, true, "mixed", "unknown");
        let softened = negative_space_context(
            "no_early_warning",
            0,
            false,
            "recovery_softening",
            "softened_only",
        );
        let continued = negative_space_context(
            "matched",
            1,
            false,
            "recovery_reconcentrating",
            "tightening",
        );

        assert_eq!(
            classify_negative_space_context(&quiet),
            NEGATIVE_SPACE_QUIET_STABILIZED
        );
        assert_eq!(
            classify_negative_space_context(&softened),
            NEGATIVE_SPACE_QUIET_SOFTENED
        );
        assert_eq!(
            classify_negative_space_context(&continued),
            NEGATIVE_SPACE_CONTINUED_HOLD
        );
    }

    #[test]
    fn owner_annotation_overrides_inferred_bucket_effectively() {
        let mut entry = entry_for("exact", FP_A, 10);
        entry.last_suppression_signature = "sig_a".to_string();
        let context = negative_space_context("quiet", 0, true, "mixed", "unknown");
        let now = 10_u64.saturating_add(CONSOLIDATION_TRACE_WINDOW_SECS);
        assert!(infer_negative_space_outcome(&mut entry, &context, now));

        let annotation = BTSPNegativeSpaceAnnotationV3 {
            owner: "minime".to_string(),
            case_key: entry.case_key.clone(),
            replay_scope: entry.replay_scope.clone(),
            classification: NEGATIVE_SPACE_WORSENED.to_string(),
            observed_at_unix_s: now,
            source_ref_hash: "hash_only".to_string(),
            consolidation_bucket_index: Some(0),
        };

        assert!(apply_negative_space_annotations(
            &mut entry,
            &[annotation],
            now
        ));
        let counts = negative_space_counts(&entry);
        assert_eq!(counts.total, 1);
        assert_eq!(counts.positive, 0);
        assert_eq!(counts.worsened, 1);
        assert_eq!(
            effective_negative_space_outcomes(&entry)[0].source_kind,
            "owner_annotation"
        );
    }

    #[test]
    fn negative_space_resolves_only_when_structured_outcomes_do_not() {
        let mut entry = entry_for("similar", FP_A, 10);
        entry.last_suppression_signature = "sig_a".to_string();
        entry.negative_space_outcomes.push(negative_space_outcome(
            &entry,
            "sig_a",
            negative_space_bucket(entry.registered_at_unix_s, 0).expect("bucket"),
            "inferred",
            NEGATIVE_SPACE_QUIET_SOFTENED,
            20,
            "",
            "",
        ));
        let notebook = BTSPCausalLabNotebookV3 {
            schema_version: 3,
            entries: vec![entry],
            last_updated_unix_s: 10,
        };

        let (notebook, changed) = resolve_lab_entries(notebook, &trace_bank(&[]), 30);
        assert!(changed);
        assert_eq!(
            notebook.entries[0].resolution_status,
            "negative_space_supported_softening"
        );

        let (notebook, _) = resolve_lab_entries(notebook, &trace_bank(&["recovery_widening"]), 40);
        assert_eq!(notebook.entries[0].resolution_status, "supported_widening");
    }

    #[test]
    fn remission_moves_from_hard_to_soft_to_trial_eligible() {
        let mut entry = entry_for("exact", FP_A, 10);
        entry.post_registration_reconcentrating_count = 1;
        let hard = forgiveness_state_for(&entry);
        assert_eq!(hard.remission_status, "hard_hold");

        entry.negative_space_outcomes.push(negative_space_outcome(
            &entry,
            "sig_a",
            negative_space_bucket(entry.registered_at_unix_s, 0).expect("bucket"),
            "inferred",
            NEGATIVE_SPACE_QUIET_STABILIZED,
            20,
            "",
            "",
        ));
        let soft = forgiveness_state_for(&entry);
        assert_eq!(soft.remission_status, "softened_hold");

        entry.post_registration_reconcentrating_count = 0;
        entry.negative_space_outcomes.push(negative_space_outcome(
            &entry,
            "sig_b",
            negative_space_bucket(entry.registered_at_unix_s, 1).expect("bucket"),
            "owner_annotation",
            NEGATIVE_SPACE_QUIET_SOFTENED,
            30,
            "astrid",
            "hash_only",
        ));
        let eligible = forgiveness_state_for(&entry);
        assert_eq!(eligible.remission_status, "consentful_trial_eligible");
        assert!(eligible.consentful_trial_eligible);
        assert!(eligible.suppression_weight < soft.suppression_weight);
    }

    #[test]
    fn old_causal_lab_json_deserializes_with_v3_2_defaults() {
        let raw = r#"{
          "experiment_id":"old",
          "signal_fingerprint":"families=grinding_family;transition=none;crossing=none;perturb=tightening;fill_band=near",
          "registered_at_unix_s":10,
          "last_seen_unix_s":10,
          "observation_count":1,
          "status":"pre_registered_holdout",
          "consent_mode":"study_counter_refusal_or_new_evidence_required",
          "proposal_policy":"withhold_duplicate_offer",
          "withheld_proposal":true,
          "question":"q",
          "hypothesis":"h",
          "counterfactual":"c",
          "holdout_route":"r",
          "replay_scope":"exact",
          "exact_fingerprint_count":1,
          "similar_fingerprint_count":0,
          "nearest_count":1,
          "reconcentrating_count":1,
          "widening_count":0,
          "mean_similarity_score":100.0,
          "teacher_outcome_class":"unknown",
          "teacher_evidence_summary":"unknown",
          "summary":"Causal lab V3: pre-registering an exact-fingerprint holdout."
        }"#;
        let mut entry: BTSPCausalExperimentV3 = serde_json::from_str(raw).expect("old entry");

        assert!(normalize_lab_entry(&mut entry));
        assert_eq!(entry.negative_space_outcome_count, 0);
        assert_eq!(entry.forgiveness_state.remission_status, "hard_hold");
        assert_eq!(
            entry.case_key,
            "families=grinding_family;perturb=tightening;fill_band=near"
        );
    }

    #[test]
    fn later_widening_resolves_supported_widening() {
        let entry = entry_for("similar", FP_A, 10);
        let notebook = BTSPCausalLabNotebookV3 {
            schema_version: 3,
            entries: vec![entry],
            last_updated_unix_s: 10,
        };

        let (notebook, changed) = resolve_lab_entries(
            notebook,
            &trace_bank(&["recovery_reconcentrating", "recovery_widening"]),
            30,
        );

        assert!(changed);
        assert_eq!(notebook.entries[0].resolution_status, "supported_widening");
        assert_eq!(notebook.entries[0].post_registration_widening_count, 1);
    }

    #[test]
    fn later_softening_resolves_supported_softening() {
        let entry = entry_for("similar", FP_A, 10);
        let notebook = BTSPCausalLabNotebookV3 {
            schema_version: 3,
            entries: vec![entry],
            last_updated_unix_s: 10,
        };

        let (notebook, changed) =
            resolve_lab_entries(notebook, &trace_bank(&["recovery_softening"]), 30);

        assert!(changed);
        assert_eq!(notebook.entries[0].resolution_status, "supported_softening");
        assert_eq!(notebook.entries[0].post_registration_softening_count, 1);
    }

    #[test]
    fn three_later_reconcentrating_outcomes_resolve_still_reconcentrating() {
        let entry = entry_for("similar", FP_A, 10);
        let notebook = BTSPCausalLabNotebookV3 {
            schema_version: 3,
            entries: vec![entry],
            last_updated_unix_s: 10,
        };

        let (notebook, changed) = resolve_lab_entries(
            notebook,
            &trace_bank(&[
                "recovery_reconcentrating",
                "recovery_reconcentrating",
                "recovery_reconcentrating",
            ]),
            30,
        );

        assert!(changed);
        assert_eq!(
            notebook.entries[0].resolution_status,
            "still_reconcentrating"
        );
        assert_eq!(
            notebook.entries[0].post_registration_reconcentrating_count,
            3
        );
    }

    #[test]
    fn suppression_only_window_does_not_resolve_experiment() {
        let entry = entry_for("similar", FP_A, 10);
        let notebook = BTSPCausalLabNotebookV3 {
            schema_version: 3,
            entries: vec![entry],
            last_updated_unix_s: 10,
        };

        let (notebook, changed) = resolve_lab_entries(notebook, &BTSPTraceBankV2::default(), 30);

        assert!(!changed);
        assert_eq!(
            notebook.entries[0].resolution_status,
            "pre_registered_holdout"
        );
        assert_eq!(notebook.entries[0].post_registration_outcome_count, 0);
    }
}
