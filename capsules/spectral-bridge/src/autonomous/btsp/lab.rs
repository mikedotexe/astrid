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
const GHOST_NOTE: &str = "I would have opened the ordinary BTSP advisory here, but replay says this family has reconcentrated; holding for study/refusal/counter/new evidence.";

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
    pub summary: String,
}

pub(super) fn sync_causal_lab_v3(
    signal_fingerprint: &str,
    replay_read: Option<&BTSPReplaySummaryV2>,
    anti_loop_state: Option<&BTSPAntiLoopState>,
    teacher_signal: Option<&BTSPOutcomeVectorV2>,
    active_proposal: Option<&ActiveSovereigntyProposal>,
    trace_bank: &BTSPTraceBankV2,
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

fn resolve_lab_entries(
    mut notebook: BTSPCausalLabNotebookV3,
    trace_bank: &BTSPTraceBankV2,
    now: u64,
) -> (BTSPCausalLabNotebookV3, bool) {
    let mut changed = false;
    for entry in &mut notebook.entries {
        let counts = post_registration_counts(entry, &trace_bank.instructive_signals);
        changed |= apply_resolution_counts(entry, counts);
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
    let (status, summary) = resolution_for_counts(counts);
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

fn resolution_for_counts(counts: ResolutionCounts) -> (String, String) {
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
    (
        "pre_registered_holdout".to_string(),
        unresolved_resolution_summary(),
    )
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
