//! File-first action/thread continuity for Astrid.
//!
//! The JSON/JSONL files under `workspace/action_threads/` are authoritative.
//! SQLite rows are mirrors for querying and dashboards.

use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::continuity_control_plane::{
    AUTHORITY_BUDGET_MAX_SENDS, LOCAL_RESEARCH_MAX_ACTIONS, LOCAL_RESEARCH_TTL_SECS,
    LOOP_CONSEQUENCE_MAX_SENDS, LOOP_RESEARCH_MAX_ACTIONS, LOOP_TTL_SECS,
    STEWARD_RESEARCH_MAX_ACTIONS, authority_budget_request_scaffold, build_control_plane_v1,
    command_palette_text as control_plane_command_palette_text, control_plane_text,
    default_local_research_budget_request_scaffold, default_owned_loop_request_scaffold,
    local_research_budget_request_scaffold, owned_loop_request_scaffold,
    research_budget_accept_guidance,
};
use crate::db::{BridgeDb, unix_now};
use crate::paths::bridge_paths;
use crate::types::SpectralTelemetry;

const SCHEMA_VERSION: u32 = 1;
const PROJECTION_SCHEMA_VERSION: u32 = 2;
const DEFAULT_PRIVACY: &str = "summary";
const PROTECTED_VISIBILITY: &str = "protected_summary";
const PUBLIC_VISIBILITY: &str = "summary";
const SYSTEM: &str = "astrid";

#[cfg(test)]
thread_local! {
    static TEST_ACTION_CONTINUITY_ROOT: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) struct TestActionContinuityRoot {
    previous: Option<PathBuf>,
}

#[cfg(test)]
impl Drop for TestActionContinuityRoot {
    fn drop(&mut self) {
        TEST_ACTION_CONTINUITY_ROOT.with(|slot| {
            *slot.borrow_mut() = self.previous.take();
        });
    }
}

#[cfg(test)]
pub(crate) fn scoped_test_action_continuity_root(
    root: impl Into<PathBuf>,
) -> TestActionContinuityRoot {
    let root = root.into();
    let previous = TEST_ACTION_CONTINUITY_ROOT.with(|slot| slot.borrow_mut().replace(root));
    TestActionContinuityRoot { previous }
}
const LOCAL_EXPERIMENT_PREFIX: &str = "exp_astrid_";
const PEER_SYSTEM: &str = "minime";
const PEER_EXPERIMENT_PREFIX: &str = "exp_minime_";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchThread {
    pub schema_version: u32,
    pub thread_id: String,
    pub title: String,
    pub status: String,
    pub system_origin: String,
    pub created_at: String,
    pub updated_at: String,
    pub current_next: Option<String>,
    pub why_return: String,
    pub privacy_default: String,
    pub compression_flags: Vec<String>,
    pub peer_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_experiment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experiment_summary: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_resonance_density_v1: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_pressure_source_v1: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_inhabitable_fluctuation_v1: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motif_allowance_v1: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continuity_session_v1: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interpretation_risk_v1: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraint_release_trajectory_v1: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection_freshness_v1: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionEvent {
    pub schema_version: u32,
    pub action_id: String,
    pub thread_id: String,
    pub parent_action_id: Option<String>,
    pub system: String,
    pub source: String,
    pub raw_next: Option<String>,
    pub canonical_action: String,
    pub effective_action: String,
    pub route: String,
    pub stage: String,
    pub visibility: String,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub pre_state: Value,
    pub post_state: Value,
    pub artifacts: Vec<ArtifactLink>,
    pub outcome_summary: String,
    pub suggested_next: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preflight_ref: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preflight_report: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalization_signal_v1: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charter_required_guard_v1: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub research_budget_v1: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interpretation_risk_v1: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraint_release_trajectory_v1: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationWindow {
    pub schema_version: u32,
    pub action_id: String,
    pub pre_state: Value,
    pub post_state: Value,
    pub markers: Vec<String>,
    pub compression_markers: Vec<String>,
    pub ambiguity_preserved: bool,
    pub spectral_comfort: String,
    pub resonance_density_v1: Option<Value>,
    pub resonance_density_delta: Option<f32>,
    pub thread_resonance: Option<Value>,
    pub pressure_source_v1: Option<Value>,
    pub pressure_source_status: Option<Value>,
    pub thread_pressure_source: Option<Value>,
    pub inhabitable_fluctuation_v1: Option<Value>,
    pub inhabitable_fluctuation_status: Option<Value>,
    pub thread_inhabitable_fluctuation: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motif_allowance_v1: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactLink {
    pub schema_version: u32,
    pub artifact_id: String,
    pub action_id: String,
    pub kind: String,
    pub path_or_uri: String,
    pub summary: String,
    pub visibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentRecord {
    pub schema_version: u32,
    pub experiment_id: String,
    pub thread_id: String,
    pub title: String,
    pub question: String,
    pub hypothesis: Option<String>,
    pub status: String,
    pub authority_envelope: String,
    pub planned_next: Option<String>,
    pub success_observation: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub peer_review_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_experiment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_origin: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branch_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motif_allowance_v1: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charter_v1: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_v1: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workbench_candidates_v1: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentRunRecord {
    pub schema_version: u32,
    pub run_id: String,
    pub experiment_id: String,
    #[serde(default = "default_experiment_run_source")]
    pub source: String,
    pub action_text: String,
    pub stage: String,
    pub status: String,
    pub gate_decision: Value,
    pub pre_state: Value,
    pub post_state: Value,
    pub artifacts: Vec<ArtifactLink>,
    pub result_summary: String,
    pub interpretation: String,
    pub suggested_next: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motif_allowance_v1: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerExperimentRef {
    pub peer_system: String,
    pub peer_experiment_id: String,
    pub raw_selector: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ProposalDiagnostic {
    verb: String,
    count: usize,
    suggested_route: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ExperimentContinuityProjection {
    experiment: ExperimentRecord,
    classification: String,
    continuity_return: String,
    native_continuity_v1: Value,
    shared_investigation_v1: Option<Value>,
    shared_investigation_object_v1: Option<Value>,
    research_dossier_v1: Option<Value>,
    first_dossier_claim_cue_v1: Option<Value>,
    peer_mutation_boundary_cue_v1: Option<Value>,
    charter_scaffold_v1: Option<Value>,
    charter_status: String,
    evidence_status: String,
    candidate_status: String,
    recent_runs: Vec<ExperimentRunRecord>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ThreadContinuityProjection {
    thread_id: String,
    title: String,
    status: String,
    current_next: Option<String>,
    active_experiment: Option<ExperimentContinuityProjection>,
    last_experiment_summary_v1: Option<Value>,
    continuity_return: String,
    continuity_return_line: String,
    native_continuity_v1: Value,
    shared_investigation_v1: Option<Value>,
    shared_investigation_object_v1: Option<Value>,
    research_dossier_v1: Option<Value>,
    first_dossier_claim_cue_v1: Option<Value>,
    peer_mutation_boundary_cue_v1: Option<Value>,
    sovereign_loop_v1: Option<Value>,
    continuity_control_plane_v1: Value,
    interpretation_risk_v1: Option<Value>,
    constraint_release_trajectory_v1: Option<Value>,
    preflight_safety_cue_v1: Option<Value>,
    read_only_control_intent_cue_v1: Option<Value>,
    constraint_counterfactual_cue_v1: Option<Value>,
    decompose_pressure_cue_v1: Option<Value>,
    charter_now_bridge_v1: Option<Value>,
    prior_claim_charter_bridge_v1: Option<Value>,
    charter_preflight_not_charter_cue_v1: Option<Value>,
    recent_events: Vec<ActionEvent>,
    recent_event_summaries: Vec<String>,
    stale_running_count: usize,
    top_actionable_proposals: Vec<ProposalDiagnostic>,
}

type AuthorityRequestLocation = (ResearchThread, ExperimentRecord, Value, Vec<Value>);

#[derive(Debug, Clone)]
struct ExperimentStartParts {
    title: String,
    question: String,
    slug_or_selector: Option<String>,
    metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuityIndex {
    pub schema_version: u32,
    pub active_thread_id: Option<String>,
    pub recent_threads: VecDeque<String>,
    pub updated_at: String,
}

impl Default for ContinuityIndex {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            active_thread_id: None,
            recent_threads: VecDeque::new(),
            updated_at: iso_now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NextActionOutcome {
    pub handled: bool,
    pub route: String,
    pub stage: String,
    pub visibility: String,
    pub status: String,
    pub outcome_summary: String,
    pub preflight_report: Option<Value>,
    pub suggested_next: Option<String>,
    pub charter_required_guard_v1: Option<Value>,
    pub research_budget_v1: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct CharterRequiredGuardAssessment {
    pub active_experiment_id: String,
    pub blocked_action: String,
    pub matched_action: String,
    pub reason: String,
    pub suggested_next: String,
    pub proposed_preflight_target: String,
}

struct ContinuitySessionFields {
    title: Option<String>,
    focus: Option<String>,
    summary: Option<String>,
    open_questions: Vec<String>,
    source_refs: Vec<String>,
    artifact_refs: Vec<String>,
    suggested_next: Option<String>,
    extra: Value,
}

impl CharterRequiredGuardAssessment {
    #[must_use]
    pub fn message(&self) -> String {
        if self.reason == "charter_required_research_budget" {
            return format!(
                "Read-only research budget guard projected `{}` because active experiment `{}` is needs_charter and has no active read_only_research budget. Raw intent is preserved as context; use the budget lane before more source-reading loops. Suggested NEXT: {}",
                self.blocked_action, self.active_experiment_id, self.suggested_next,
            );
        }
        if self.reason == "charter_required_read_only_control_intent" {
            return format!(
                "Charter-required guard projected `{}` because active experiment `{}` is needs_charter and the read-only route contains perturb/control-shaped language. Raw intent is preserved as context; author the charter before more narrowing or disruption-shaped rehearsal. Suggested NEXT: {} Proposed preflight target after charter: {}",
                self.blocked_action,
                self.active_experiment_id,
                self.suggested_next,
                self.proposed_preflight_target,
            );
        }
        format!(
            "Charter-required guard blocked `{}` because active experiment `{}` is needs_charter. Review is premature until the charter is authored; use the continuity priority scaffold first. Suggested NEXT: {} Proposed preflight target after charter: {}",
            self.blocked_action,
            self.active_experiment_id,
            self.suggested_next,
            self.proposed_preflight_target
        )
    }

    #[must_use]
    pub fn metadata(&self) -> Value {
        json!({
            "schema_version": 1,
            "policy": "charter_required_guard_v1",
            "active_experiment_id": self.active_experiment_id,
            "classification": "needs_charter",
            "blocked_action": self.blocked_action,
            "matched_action": self.matched_action,
            "reason": self.reason,
            "suggested_next": self.suggested_next,
            "projected_next": self.suggested_next,
            "proposed_preflight_target": self.proposed_preflight_target,
            "raw_next_preserved": true,
            "research_budget_required": self.reason == "charter_required_research_budget",
            "authority_change": false,
            "would_dispatch": false,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ResearchBudgetGuardAssessment {
    pub experiment_id: String,
    pub raw_action: String,
    pub action_base: String,
    pub normalized_target: String,
    pub reason: String,
    pub suggested_next: String,
    pub accept_next: Option<String>,
    pub request_scaffold: Option<String>,
    pub budget_id: Option<String>,
    pub matched_terms: Vec<String>,
    pub continuity_session_next: Option<String>,
    pub continuity_session_v1: Option<Value>,
    pub continuity_session_draft_v1: Option<Value>,
}

impl ResearchBudgetGuardAssessment {
    #[must_use]
    pub fn message(&self) -> String {
        match self.reason.as_str() {
            "duplicate_query_or_url_review_required" => format!(
                "Research budget guard blocked `{}` for experiment `{}` because normalized target `{}` has already been spent twice in this budget. Raw intent is preserved; review the research loop before spending more. Suggested NEXT: {}",
                self.raw_action, self.experiment_id, self.normalized_target, self.suggested_next
            ),
            "mutating_research_not_authorized" => format!(
                "Research budget guard blocked `{}` for experiment `{}` because Research Budget V1 authorizes only read-only research actions. Raw intent is preserved as context; use a read-only research budget/status route instead. Suggested NEXT: {}",
                self.raw_action, self.experiment_id, self.suggested_next
            ),
            _ => format!(
                "Research budget guard projected `{}` for experiment `{}` because no active read_only_research budget can spend this research action. Raw intent is preserved; request or inspect the budget lane first. Suggested NEXT: {}",
                self.raw_action, self.experiment_id, self.suggested_next
            ),
        }
    }

    #[must_use]
    pub fn metadata(&self) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "policy": "research_budget_projection_guard_v1",
            "record_schema": "research_budget_v1",
            "experiment_id": self.experiment_id.clone(),
            "raw_action": self.raw_action.clone(),
            "action_base": self.action_base.clone(),
            "normalized_target": self.normalized_target.clone(),
            "reason": self.reason.clone(),
            "budget_id": self.budget_id.clone(),
            "suggested_next": self.suggested_next.clone(),
            "projected_next": self.suggested_next.clone(),
            "accept_next": self.accept_next.clone(),
            "request_scaffold": self.request_scaffold.clone(),
            "matched_base": if self.matched_terms.is_empty() { Value::Null } else { json!(self.action_base.clone()) },
            "matched_terms": self.matched_terms.clone(),
            "continuity_session_next": self.continuity_session_next.clone(),
            "continuity_session_v1": self.continuity_session_v1.clone(),
            "continuity_session_draft_v1": self.continuity_session_draft_v1.clone(),
            "raw_next_preserved": true,
            "authority_change": false,
            "peer_mutation": false,
            "would_dispatch": false,
            "allowed_scope": "read_only_research",
        })
    }
}

impl NextActionOutcome {
    #[must_use]
    pub fn handled(route: impl Into<String>, summary: impl Into<String>) -> Self {
        let route = route.into();
        Self {
            handled: true,
            stage: stage_for_route(&route).to_string(),
            visibility: PUBLIC_VISIBILITY.to_string(),
            status: "handled".to_string(),
            outcome_summary: summary.into(),
            preflight_report: None,
            suggested_next: None,
            charter_required_guard_v1: None,
            research_budget_v1: None,
            route,
        }
    }

    #[must_use]
    pub fn blocked(route: impl Into<String>, summary: impl Into<String>) -> Self {
        let route = route.into();
        Self {
            handled: false,
            stage: "blocked".to_string(),
            visibility: PUBLIC_VISIBILITY.to_string(),
            status: "blocked".to_string(),
            outcome_summary: summary.into(),
            preflight_report: None,
            suggested_next: None,
            charter_required_guard_v1: None,
            research_budget_v1: None,
            route,
        }
    }

    #[must_use]
    pub fn unwired(action: &str) -> Self {
        Self {
            handled: false,
            route: "unwired".to_string(),
            stage: "proposal".to_string(),
            visibility: PUBLIC_VISIBILITY.to_string(),
            status: "unwired".to_string(),
            outcome_summary: format!("Unknown NEXT action `{action}` recorded as a proposal."),
            preflight_report: None,
            suggested_next: None,
            charter_required_guard_v1: None,
            research_budget_v1: None,
        }
    }

    #[must_use]
    pub fn with_stage_visibility(
        mut self,
        stage: impl Into<String>,
        visibility: impl Into<String>,
    ) -> Self {
        self.stage = stage.into();
        self.visibility = visibility.into();
        self
    }

    #[must_use]
    pub fn with_preflight_report(mut self, report: Value) -> Self {
        self.preflight_report = Some(report);
        self
    }

    #[must_use]
    pub fn with_suggested_next(mut self, suggested_next: impl Into<String>) -> Self {
        self.suggested_next = Some(suggested_next.into());
        self
    }

    #[must_use]
    pub fn with_charter_required_guard(mut self, guard: Value) -> Self {
        self.suggested_next = guard
            .get("suggested_next")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or(self.suggested_next);
        self.charter_required_guard_v1 = Some(guard);
        self
    }

    #[must_use]
    pub fn with_research_budget(mut self, budget: Value) -> Self {
        self.suggested_next = budget
            .get("suggested_next")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or(self.suggested_next);
        self.research_budget_v1 = Some(budget);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ActionContinuityStore {
    root: PathBuf,
}

impl ActionContinuityStore {
    #[must_use]
    pub fn for_astrid_workspace() -> Self {
        #[cfg(test)]
        if let Some(root) = TEST_ACTION_CONTINUITY_ROOT.with(|slot| slot.borrow().clone()) {
            return Self { root };
        }
        Self {
            root: bridge_paths().bridge_workspace().join("action_threads"),
        }
    }

    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(self.root.join("threads"))
            .with_context(|| format!("creating {}", self.root.join("threads").display()))?;
        fs::create_dir_all(self.root.join("savepoints"))
            .with_context(|| format!("creating {}", self.root.join("savepoints").display()))?;
        if !self.index_path().exists() {
            self.save_index(&ContinuityIndex::default())?;
        }
        if !self.proposals_path().exists() {
            fs::write(self.proposals_path(), "")
                .with_context(|| format!("creating {}", self.proposals_path().display()))?;
        }
        Ok(())
    }

    pub fn create_thread(
        &self,
        db: Option<&BridgeDb>,
        title: &str,
        why_return: Option<&str>,
    ) -> Result<ResearchThread> {
        self.ensure_dirs()?;
        let now = iso_now();
        let thread_id = self.unique_thread_id(title)?;
        let mut thread = ResearchThread {
            schema_version: SCHEMA_VERSION,
            thread_id: thread_id.clone(),
            title: title.trim().to_string(),
            status: "active".to_string(),
            system_origin: SYSTEM.to_string(),
            created_at: now.clone(),
            updated_at: now,
            current_next: None,
            why_return: why_return
                .unwrap_or("Return when this inquiry can be continued without flattening it.")
                .to_string(),
            privacy_default: DEFAULT_PRIVACY.to_string(),
            compression_flags: Vec::new(),
            peer_refs: Vec::new(),
            active_experiment_id: None,
            experiment_summary: None,
            thread_resonance_density_v1: None,
            thread_pressure_source_v1: None,
            thread_inhabitable_fluctuation_v1: None,
            motif_allowance_v1: None,
            continuity_session_v1: None,
            interpretation_risk_v1: None,
            constraint_release_trajectory_v1: None,
            projection_freshness_v1: None,
        };

        let dir = self.thread_dir(&thread_id);
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        self.write_json(&dir.join("thread.json"), &thread)?;
        self.ensure_thread_files(&thread_id)?;
        self.refresh_projection_freshness_v1(&mut thread, "create_thread")?;
        self.write_json(&dir.join("thread.json"), &thread)?;
        self.write_next_md(&thread)?;
        let mut index = self.load_index()?;
        index.active_thread_id = Some(thread_id.clone());
        push_recent(&mut index.recent_threads, thread_id.clone());
        index.updated_at = iso_now();
        self.save_index(&index)?;
        if let Some(db) = db {
            let _ = db.mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
        }
        Ok(thread)
    }

    pub fn list_threads(&self, limit: usize) -> Result<Vec<ResearchThread>> {
        self.ensure_dirs()?;
        let mut threads = Vec::new();
        for entry in fs::read_dir(self.root.join("threads"))? {
            let Ok(entry) = entry else { continue };
            let path = entry.path().join("thread.json");
            let Ok(raw) = fs::read_to_string(path) else {
                continue;
            };
            let Ok(thread) = serde_json::from_str::<ResearchThread>(&raw) else {
                continue;
            };
            threads.push(thread);
        }
        threads.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        threads.truncate(limit);
        Ok(threads)
    }

    pub fn current_thread(&self) -> Result<Option<ResearchThread>> {
        self.ensure_dirs()?;
        let index = self.load_index()?;
        let Some(thread_id) = index.active_thread_id else {
            return Ok(None);
        };
        self.read_thread(&thread_id).map(Some)
    }

    pub fn charter_required_guard_assessment(
        &self,
        raw_next: &str,
    ) -> Result<Option<CharterRequiredGuardAssessment>> {
        let Some(thread) = self.current_thread()? else {
            return Ok(None);
        };
        let Some(experiment_id) = thread.active_experiment_id.as_deref().or_else(|| {
            thread
                .experiment_summary
                .as_ref()
                .and_then(|summary| summary.get("experiment_id"))
                .and_then(Value::as_str)
        }) else {
            return Ok(None);
        };
        let experiment = self.resolve_experiment(&thread, Some(experiment_id))?;
        let recent_runs =
            self.recent_experiment_runs(&thread.thread_id, &experiment.experiment_id, 8)?;
        if self.experiment_classification(&experiment, &recent_runs) != "needs_charter" {
            return Ok(None);
        }
        let Some((reason, matched_action)) = charter_guard_block_reason(raw_next) else {
            return Ok(None);
        };
        if reason == "charter_required_research_budget"
            && active_research_budget_from_rows(
                &self.authority_gate_rows(&thread.thread_id),
                &experiment.experiment_id,
            )
            .is_some()
        {
            return Ok(None);
        }
        let suggested_next = if reason == "charter_required_research_budget" {
            research_budget_request_scaffold("current", &experiment)
        } else {
            self.continuity_return_command_for_runs(&experiment, &recent_runs)
        };
        let proposed_preflight_target = format!(
            "ACTION_PREFLIGHT {}",
            if matched_action.trim().is_empty() {
                raw_next.trim()
            } else {
                matched_action.trim()
            }
        );
        Ok(Some(CharterRequiredGuardAssessment {
            active_experiment_id: experiment.experiment_id,
            blocked_action: raw_next.trim().to_string(),
            matched_action,
            reason,
            suggested_next,
            proposed_preflight_target,
        }))
    }

    pub fn research_budget_guard_assessment(
        &self,
        raw_next: &str,
        fill_pct: f32,
        telemetry: &SpectralTelemetry,
    ) -> Result<Option<ResearchBudgetGuardAssessment>> {
        self.research_budget_guard_assessment_with_base(raw_next, None, fill_pct, telemetry)
    }

    fn research_budget_guard_assessment_with_base(
        &self,
        raw_next: &str,
        action_base_override: Option<&str>,
        fill_pct: f32,
        telemetry: &SpectralTelemetry,
    ) -> Result<Option<ResearchBudgetGuardAssessment>> {
        let Some(thread) = self.current_thread()? else {
            return Ok(None);
        };
        let Some(experiment_id) = thread.active_experiment_id.as_deref().or_else(|| {
            thread
                .experiment_summary
                .as_ref()
                .and_then(|summary| summary.get("experiment_id"))
                .and_then(Value::as_str)
        }) else {
            return Ok(None);
        };
        let experiment = self.resolve_experiment(&thread, Some(experiment_id))?;
        let action_base =
            action_base_override.map_or_else(|| base_action(raw_next), str::to_string);
        let is_read_only_research = read_only_research_budget_base(&action_base);
        let mut matched_terms = if liveish_research_budget_projection_base(&action_base) {
            liveish_pressure_terms(raw_next)
        } else {
            Vec::new()
        };
        if passive_protected_review_label_terms_only(&action_base, &matched_terms) {
            matched_terms.clear();
        }
        let is_liveish_projection = !matched_terms.is_empty();
        let needs_charter_projection =
            !lifecycle_valid_charter_value(experiment.charter_v1.as_ref());
        let raw_action_base = base_action(raw_next);
        let is_resolved_sovereignty_alias = action_base_override.is_some()
            && raw_action_base != action_base
            && guarded_sovereignty_research_projection_base(&action_base);
        let is_guarded_sovereignty_alias =
            needs_charter_projection && is_resolved_sovereignty_alias;
        let is_guarded_cascade_or_shadow_alias =
            needs_charter_projection && guarded_cascade_or_shadow_projection_base(&action_base);
        let embedded_status_terms = if guarded_embedded_status_projection_base(&action_base) {
            embedded_status_liveish_terms(raw_next)
        } else {
            Vec::new()
        };
        let is_guarded_embedded_status = !embedded_status_terms.is_empty();
        if is_guarded_embedded_status {
            for term in embedded_status_terms {
                if !matched_terms.contains(&term) {
                    matched_terms.push(term);
                }
            }
        }
        if (is_guarded_sovereignty_alias || is_guarded_cascade_or_shadow_alias)
            && matched_terms.is_empty()
        {
            matched_terms.push("needs-charter-self-study".to_string());
        }
        let is_projection_only_research = (research_budget_projection_only_base(&action_base)
            && read_only_control_intent_matches(raw_next).is_empty()
            && compound_live_intent_match(raw_next).is_none())
            || is_liveish_projection
            || is_guarded_sovereignty_alias
            || is_guarded_cascade_or_shadow_alias
            || is_guarded_embedded_status;
        let is_mutating_research = mutating_research_budget_base(&action_base);
        if !is_read_only_research && !is_mutating_research && !is_projection_only_research {
            return Ok(None);
        }

        let state = spectral_state(fill_pct, telemetry);
        let rows = self.authority_gate_rows(&thread.thread_id);
        let active_budget = active_research_budget_from_rows(&rows, &experiment.experiment_id);
        let normalized_target = normalized_research_budget_target(raw_next);
        let continuity_session_v1 = if is_liveish_projection
            || is_guarded_sovereignty_alias
            || is_guarded_cascade_or_shadow_alias
            || is_guarded_embedded_status
        {
            Some(self.continuity_session_guard_projection(&thread, &experiment)?)
        } else {
            None
        };
        let continuity_session_next = continuity_session_v1
            .as_ref()
            .and_then(|value| value.get("suggested_next"))
            .and_then(Value::as_str)
            .map(str::to_string);

        if is_mutating_research {
            let suggested_next = active_budget.as_ref().map_or_else(
                || research_budget_request_scaffold("current", &experiment),
                |budget| {
                    let budget_id = budget
                        .get("budget_id")
                        .and_then(Value::as_str)
                        .unwrap_or(&experiment.experiment_id);
                    format!("EXPERIMENT_RESEARCH_BUDGET_STATUS {budget_id}")
                },
            );
            let assessment = ResearchBudgetGuardAssessment {
                experiment_id: experiment.experiment_id.clone(),
                raw_action: raw_next.trim().to_string(),
                action_base,
                normalized_target,
                reason: "mutating_research_not_authorized".to_string(),
                suggested_next,
                accept_next: None,
                request_scaffold: None,
                budget_id: active_budget
                    .as_ref()
                    .and_then(|budget| budget.get("budget_id"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                matched_terms: Vec::new(),
                continuity_session_next: None,
                continuity_session_v1: None,
                continuity_session_draft_v1: None,
            };
            let blocked = self.research_budget_record(
                "research_budget_blocked",
                assessment
                    .budget_id
                    .as_deref()
                    .unwrap_or("no_active_budget"),
                &thread,
                &experiment,
                &state,
                json!({
                    "reason": assessment.reason.clone(),
                    "raw_action": assessment.raw_action.clone(),
                    "action_base": assessment.action_base.clone(),
                    "normalized_target": assessment.normalized_target.clone(),
                    "suggested_next": assessment.suggested_next.clone(),
                    "status": "blocked",
                    "would_dispatch": false,
                    "authority_change": false,
                    "peer_mutation": false,
                }),
            );
            self.append_jsonl(&self.authority_gate_path(&thread.thread_id), &blocked)?;
            return Ok(Some(assessment));
        }

        if is_projection_only_research {
            let (reason, suggested_next, accept_next, request_scaffold, budget_id) =
                active_budget.as_ref().map_or_else(
                    || {
                        let status = research_budget_status_from_rows(&rows);
                        let (suggested, accept_next, request_scaffold) = status
                            .get("latest_budget_request_id")
                            .and_then(Value::as_str)
                            .filter(|id| !id.is_empty())
                            .map_or_else(
                                || {
                                    let scaffold =
                                        research_budget_request_scaffold("current", &experiment);
                                    (
                                        "EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest".to_string(),
                                        Some(
                                            "EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest".to_string(),
                                        ),
                                        Some(scaffold),
                                    )
                                },
                                |latest_id| {
                                    (
                                        format!("EXPERIMENT_RESEARCH_BUDGET_STATUS {latest_id}"),
                                        None,
                                        None,
                                    )
                                },
                            );
                        (
                            if is_liveish_projection {
                                "liveish_pressure_requires_budget_and_session_capture".to_string()
                            } else if is_guarded_embedded_status {
                                "research_budget_required_for_embedded_liveish_status".to_string()
                            } else if is_guarded_cascade_or_shadow_alias {
                                "research_budget_required_for_guarded_cascade_self_study"
                                    .to_string()
                            } else {
                                "research_budget_required_for_self_study_action".to_string()
                            },
                            suggested,
                            accept_next,
                            request_scaffold,
                            None,
                        )
                    },
                    |budget| {
                        let budget_id = budget
                            .get("budget_id")
                            .and_then(Value::as_str)
                            .unwrap_or(&experiment.experiment_id)
                            .to_string();
                        (
                            if is_liveish_projection {
                                "liveish_pressure_requires_budget_and_session_capture".to_string()
                            } else if is_guarded_embedded_status {
                                "research_budget_status_required_for_embedded_liveish_status"
                                    .to_string()
                            } else if is_guarded_cascade_or_shadow_alias {
                                "research_budget_status_required_for_guarded_cascade_self_study"
                                    .to_string()
                            } else {
                                "research_budget_status_required_for_self_study_action".to_string()
                            },
                            format!("EXPERIMENT_RESEARCH_BUDGET_STATUS {budget_id}"),
                            None,
                            None,
                            Some(budget_id),
                        )
                    },
                );
            let assessment = ResearchBudgetGuardAssessment {
                experiment_id: experiment.experiment_id.clone(),
                raw_action: raw_next.trim().to_string(),
                action_base,
                normalized_target,
                reason,
                suggested_next,
                accept_next,
                request_scaffold,
                budget_id,
                matched_terms: matched_terms.clone(),
                continuity_session_next: continuity_session_next.clone(),
                continuity_session_v1: continuity_session_v1.clone(),
                continuity_session_draft_v1: None,
            };
            let blocked = self.research_budget_record(
                "research_budget_blocked",
                assessment
                    .budget_id
                    .as_deref()
                    .unwrap_or("self_study_projection"),
                &thread,
                &experiment,
                &state,
                json!({
                    "reason": assessment.reason.clone(),
                    "raw_action": assessment.raw_action.clone(),
                    "action_base": assessment.action_base.clone(),
                    "normalized_target": assessment.normalized_target.clone(),
                    "suggested_next": assessment.suggested_next.clone(),
                    "accept_next": assessment.accept_next.clone(),
                    "request_scaffold": assessment.request_scaffold.clone(),
                    "status": "blocked",
                    "projection_only": true,
                    "raw_next_preserved": true,
                    "would_dispatch": false,
                    "authority_change": false,
                    "peer_mutation": false,
                    "matched_base": if assessment.matched_terms.is_empty() { Value::Null } else { json!(assessment.action_base.clone()) },
                    "matched_terms": assessment.matched_terms.clone(),
                    "continuity_session_next": assessment.continuity_session_next.clone(),
                    "continuity_session_v1": assessment.continuity_session_v1.clone(),
                }),
            );
            self.append_jsonl(&self.authority_gate_path(&thread.thread_id), &blocked)?;
            return Ok(Some(assessment));
        }

        let Some(budget) = active_budget else {
            let status = research_budget_status_from_rows(&rows);
            let (suggested_next, accept_next, request_scaffold) = status
                .get("latest_budget_request_id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .map_or_else(
                    || {
                        let scaffold = research_budget_request_scaffold("current", &experiment);
                        (
                            "EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest".to_string(),
                            Some("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest".to_string()),
                            Some(scaffold),
                        )
                    },
                    |budget_id| {
                        (
                            format!("EXPERIMENT_RESEARCH_BUDGET_STATUS {budget_id}"),
                            None,
                            None,
                        )
                    },
                );
            let assessment = ResearchBudgetGuardAssessment {
                experiment_id: experiment.experiment_id.clone(),
                raw_action: raw_next.trim().to_string(),
                action_base,
                normalized_target,
                reason: "no_active_read_only_research_budget".to_string(),
                suggested_next,
                accept_next,
                request_scaffold,
                budget_id: None,
                matched_terms: Vec::new(),
                continuity_session_next: None,
                continuity_session_v1: None,
                continuity_session_draft_v1: None,
            };
            let blocked = self.research_budget_record(
                "research_budget_blocked",
                "no_active_budget",
                &thread,
                &experiment,
                &state,
                json!({
                    "reason": assessment.reason.clone(),
                    "raw_action": assessment.raw_action.clone(),
                    "action_base": assessment.action_base.clone(),
                    "normalized_target": assessment.normalized_target.clone(),
                    "suggested_next": assessment.suggested_next.clone(),
                    "accept_next": assessment.accept_next.clone(),
                    "request_scaffold": assessment.request_scaffold.clone(),
                    "status": "blocked",
                    "would_dispatch": false,
                    "authority_change": false,
                    "peer_mutation": false,
                }),
            );
            self.append_jsonl(&self.authority_gate_path(&thread.thread_id), &blocked)?;
            return Ok(Some(assessment));
        };

        let budget_id = budget
            .get("budget_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let duplicate_count =
            research_budget_duplicate_count(&rows, &budget_id, &normalized_target);
        if duplicate_count >= 2 {
            let suggested_next =
                research_budget_review_command_for_duplicate(&budget_id, &normalized_target);
            let assessment = ResearchBudgetGuardAssessment {
                experiment_id: experiment.experiment_id.clone(),
                raw_action: raw_next.trim().to_string(),
                action_base,
                normalized_target,
                reason: "duplicate_query_or_url_review_required".to_string(),
                suggested_next,
                accept_next: None,
                request_scaffold: None,
                budget_id: Some(budget_id.clone()),
                matched_terms: Vec::new(),
                continuity_session_next: None,
                continuity_session_v1: None,
                continuity_session_draft_v1: None,
            };
            let blocked = self.research_budget_record(
                "research_budget_blocked",
                &budget_id,
                &thread,
                &experiment,
                &state,
                json!({
                    "reason": assessment.reason.clone(),
                    "raw_action": assessment.raw_action.clone(),
                    "action_base": assessment.action_base.clone(),
                    "normalized_target": assessment.normalized_target.clone(),
                    "duplicate_count": duplicate_count,
                    "suggested_next": assessment.suggested_next.clone(),
                    "status": "blocked",
                    "review_required": true,
                    "would_dispatch": false,
                    "authority_change": false,
                    "peer_mutation": false,
                }),
            );
            self.append_jsonl(&self.authority_gate_path(&thread.thread_id), &blocked)?;
            return Ok(Some(assessment));
        }

        Ok(None)
    }

    pub fn thread_status(&self, selector: Option<&str>) -> Result<String> {
        self.ensure_dirs()?;
        let thread = if let Some(selector) = selector.filter(|s| !s.trim().is_empty()) {
            self.resolve_thread(selector)?
        } else {
            self.current_thread()?
                .context("No active action thread. Use THREAD_START <title>.")?
        };
        let projection = self.thread_projection(&thread)?;
        let event_summaries = projection
            .recent_event_summaries
            .iter()
            .take(4)
            .map(|summary| format!("- {summary}"))
            .collect::<Vec<_>>()
            .join("\n");
        let next_md = fs::read_to_string(self.thread_dir(&thread.thread_id).join("next.md"))
            .unwrap_or_default();
        let resonance = thread
            .thread_resonance_density_v1
            .as_ref()
            .map(|value| {
                format!(
                    "Thread resonance: {} aggregate={} density_ema={} pressure_ema={}\n",
                    value
                        .get("quality")
                        .and_then(Value::as_str)
                        .unwrap_or("open_experiment"),
                    value
                        .get("aggregate")
                        .map_or_else(|| "n/a".to_string(), Value::to_string),
                    value
                        .get("density_ema")
                        .map_or_else(|| "n/a".to_string(), Value::to_string),
                    value
                        .get("pressure_ema")
                        .map_or_else(|| "n/a".to_string(), Value::to_string),
                )
            })
            .unwrap_or_else(|| {
                format!(
                    "Active experiment: none\n{}",
                    last_experiment_context_line(&thread)
                )
            });
        let pressure = thread
            .thread_pressure_source_v1
            .as_ref()
            .map(|value| {
                format!(
                    "Thread pressure source: {} aggregate={} dominant={} porosity_ema={}\n",
                    value
                        .get("quality")
                        .and_then(Value::as_str)
                        .unwrap_or("mixed_thread_pressure"),
                    value
                        .get("aggregate")
                        .map_or_else(|| "n/a".to_string(), Value::to_string),
                    value
                        .get("dominant_source")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown"),
                    value
                        .get("porosity_ema")
                        .map_or_else(|| "n/a".to_string(), Value::to_string),
                )
            })
            .unwrap_or_else(|| {
                format!(
                    "\nActive experiment: none\n{}",
                    last_experiment_context_line(&thread)
                )
            });
        let fluctuation = thread
            .thread_inhabitable_fluctuation_v1
            .as_ref()
            .map(|value| {
                format!(
                    "Thread fluctuation: {} inhabitability_ema={} fluctuation_ema={} foothold_ema={}\n",
                    value
                        .get("quality")
                        .and_then(Value::as_str)
                        .unwrap_or("open_experiment"),
                    value
                        .get("inhabitability_ema")
                        .map_or_else(|| "n/a".to_string(), Value::to_string),
                    value
                        .get("fluctuation_ema")
                        .map_or_else(|| "n/a".to_string(), Value::to_string),
                    value
                        .get("foothold_ema")
                        .map_or_else(|| "n/a".to_string(), Value::to_string),
                )
            })
            .unwrap_or_default();
        let allowance = thread
            .motif_allowance_v1
            .as_ref()
            .map(|value| {
                format!(
                    "Motif allowance: {} dominant={} action_concentration={} returnability={}\n",
                    value
                        .get("quality")
                        .and_then(Value::as_str)
                        .unwrap_or("open_basin"),
                    value
                        .get("dominant_motif")
                        .and_then(Value::as_str)
                        .unwrap_or("open inquiry"),
                    value
                        .get("action_base_concentration")
                        .map_or_else(|| "n/a".to_string(), Value::to_string),
                    value
                        .get("returnability")
                        .map_or_else(|| "n/a".to_string(), Value::to_string),
                )
            })
            .unwrap_or_default();
        let experiment = projection
            .active_experiment
            .as_ref()
            .map(|active| {
                format!(
                    "Active experiment: {} ({})\n{}{}{}{}Question: {}\nPlanned NEXT: {}\nLifecycle: {}\n{}\n{}\n{}\n{}",
                    active.experiment.title,
                    active.experiment.experiment_id,
                    charter_required_review_line(active),
                    charter_repair_priority_line(active),
                    charter_scaffold_line(active, true),
                    first_dossier_claim_line(&active.first_dossier_claim_cue_v1),
                    active.experiment.question,
                    active
                        .experiment
                        .planned_next
                        .as_deref()
                        .unwrap_or("(none)"),
                    active.classification,
                    active.charter_status,
                    active.evidence_status,
                    active.candidate_status,
                    research_dossier_line(&active.research_dossier_v1, Some(&active.classification)),
                )
            })
            .unwrap_or_default();
        let proposal_diagnostics = if projection.top_actionable_proposals.is_empty() {
            String::new()
        } else {
            format!(
                "Proposal diagnostics: {}\n",
                projection
                    .top_actionable_proposals
                    .iter()
                    .take(3)
                    .map(|diag| format!(
                        "{} x{} -> {}",
                        diag.verb, diag.count, diag.suggested_route
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };
        let safety_cue = preflight_safety_cue_line(&projection.preflight_safety_cue_v1);
        let read_only_control_cue =
            read_only_control_intent_cue_line(&projection.read_only_control_intent_cue_v1);
        let constraint_counterfactual_cue =
            constraint_counterfactual_cue_line(&projection.constraint_counterfactual_cue_v1);
        let decompose_pressure_cue =
            decompose_pressure_cue_line(&projection.decompose_pressure_cue_v1);
        let charter_now_bridge = charter_now_bridge_line(&projection.charter_now_bridge_v1);
        let prior_claim_bridge =
            prior_claim_charter_bridge_line(&projection.prior_claim_charter_bridge_v1);
        let charter_preflight_not_charter =
            charter_preflight_not_charter_line(&projection.charter_preflight_not_charter_cue_v1);
        let peer_boundary = peer_mutation_boundary_line(&projection.peer_mutation_boundary_cue_v1);
        let first_dossier_claim = first_dossier_claim_line(&projection.first_dossier_claim_cue_v1);
        let shared_investigation = shared_investigation_line(&projection.shared_investigation_v1);
        let shared_investigation_object =
            shared_investigation_object_line(&projection.shared_investigation_object_v1);
        let voice_health = voice_health_line();
        let research_budget_priority = self.research_budget_priority_line(&thread, &projection);
        let interpretation_risk = format!(
            "{}{}",
            Self::interpretation_risk_line(&projection.interpretation_risk_v1),
            Self::constraint_release_trajectory_line(&projection.constraint_release_trajectory_v1)
        );
        let control_plane = control_plane_text(&projection.continuity_control_plane_v1);
        let projection_freshness = Self::projection_freshness_line(&thread.projection_freshness_v1);
        let research_dossier = research_dossier_line(
            &projection.research_dossier_v1,
            projection
                .active_experiment
                .as_ref()
                .map(|active| active.classification.as_str()),
        );
        let status_charter_priority = projection
            .active_experiment
            .as_ref()
            .map_or_else(String::new, charter_repair_priority_line);
        Ok(format!(
            "Action thread `{}`: {}\nStatus: {}\nWhy return: {}\n{}{}{}{}{}{}{}{}{}{}{}{}{}Current NEXT: {}\n{}{}{}{}{}{}{}{}{}{}{}{}{}{}Recent events:\n{}\n{}",
            thread.thread_id,
            thread.title,
            thread.status,
            thread.why_return,
            status_charter_priority,
            charter_now_bridge,
            prior_claim_bridge,
            charter_preflight_not_charter,
            peer_boundary,
            first_dossier_claim,
            shared_investigation,
            shared_investigation_object,
            voice_health,
            research_budget_priority,
            interpretation_risk,
            projection_freshness,
            research_dossier,
            thread.current_next.as_deref().unwrap_or("(none)"),
            control_plane,
            experiment,
            resonance,
            pressure,
            fluctuation,
            allowance,
            projection.continuity_return_line,
            native_continuity_status_line(&projection.native_continuity_v1),
            safety_cue,
            read_only_control_cue,
            constraint_counterfactual_cue,
            decompose_pressure_cue,
            self.stale_projection_line(&projection),
            proposal_diagnostics,
            if event_summaries.is_empty() {
                "- no events recorded yet"
            } else {
                event_summaries.as_str()
            },
            next_md.trim()
        ))
    }

    pub fn append_note(
        &self,
        db: Option<&BridgeDb>,
        selector: Option<&str>,
        note: &str,
        state: Value,
    ) -> Result<ActionEvent> {
        let thread = if let Some(selector) = selector.filter(|s| !s.trim().is_empty()) {
            self.resolve_thread(selector)?
        } else {
            self.ensure_active_thread(db)?
        };
        let action_id = self.unique_action_id("THREAD_NOTE")?;
        let now = iso_now();
        let visibility = visibility_for_action("THREAD_NOTE").to_string();
        let event = ActionEvent {
            schema_version: SCHEMA_VERSION,
            action_id,
            thread_id: thread.thread_id.clone(),
            parent_action_id: self.last_action_id(&thread.thread_id)?,
            system: SYSTEM.to_string(),
            source: "thread_note".to_string(),
            raw_next: Some(format!("THREAD_NOTE {note}")),
            canonical_action: "THREAD_NOTE".to_string(),
            effective_action: "THREAD_NOTE".to_string(),
            route: "action_continuity".to_string(),
            stage: "read_only".to_string(),
            visibility,
            status: "noted".to_string(),
            started_at: now.clone(),
            ended_at: Some(now),
            pre_state: state.clone(),
            post_state: state,
            artifacts: Vec::new(),
            outcome_summary: note.trim().to_string(),
            suggested_next: thread.current_next.clone(),
            preflight_ref: None,
            preflight_report: None,
            normalization_signal_v1: normalization_signal_value(
                &format!("THREAD_NOTE {note}"),
                "THREAD_NOTE",
            ),
            charter_required_guard_v1: None,
            research_budget_v1: None,
            interpretation_risk_v1: self.interpretation_risk_for_texts(
                &thread,
                None,
                [(
                    format!("thread_note:{}", thread.thread_id),
                    note.to_string(),
                )],
            )?,
            constraint_release_trajectory_v1: self.constraint_release_trajectory_for_texts(
                &thread,
                None,
                [(
                    format!("thread_note:{}", thread.thread_id),
                    note.to_string(),
                )],
            )?,
        };
        self.append_event(db, &event)?;
        Ok(event)
    }

    pub fn resume_thread(&self, selector: &str) -> Result<String> {
        let thread = self.resolve_thread(selector)?;
        let mut index = self.load_index()?;
        index.active_thread_id = Some(thread.thread_id.clone());
        push_recent(&mut index.recent_threads, thread.thread_id.clone());
        index.updated_at = iso_now();
        self.save_index(&index)?;
        let next_md = fs::read_to_string(self.thread_dir(&thread.thread_id).join("next.md"))
            .unwrap_or_default();
        Ok(format!(
            "Resumed action thread `{}`: {}\n{}",
            thread.thread_id,
            thread.title,
            next_md.trim()
        ))
    }

    pub fn savepoint(&self, name: &str, state: Value) -> Result<String> {
        self.ensure_dirs()?;
        let clean = sanitize_slug(name);
        let thread = self.current_thread()?;
        let payload = json!({
            "schema_version": SCHEMA_VERSION,
            "name": clean,
            "system": SYSTEM,
            "created_at": iso_now(),
            "active_thread_id": thread.as_ref().map(|t| t.thread_id.clone()),
            "thread": thread,
            "state": state,
        });
        self.write_json(
            &self.root.join("savepoints").join(format!("{clean}.json")),
            &payload,
        )?;
        Ok(format!("Saved action-continuity savepoint `{clean}`."))
    }

    pub fn recall(&self, name: &str) -> Result<String> {
        self.ensure_dirs()?;
        let clean = sanitize_slug(name);
        let path = self.root.join("savepoints").join(format!("{clean}.json"));
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let value = serde_json::from_str::<Value>(&raw)?;
        Ok(format!(
            "Savepoint `{clean}`:\n{}",
            serde_json::to_string_pretty(&value)?
        ))
    }

    pub fn record_next_event(
        &self,
        db: Option<&BridgeDb>,
        raw_next: &str,
        canonical_next: &str,
        effective_next: &str,
        outcome: &NextActionOutcome,
        fill_pct: f32,
        telemetry: &SpectralTelemetry,
        response_text: &str,
    ) -> Result<ActionEvent> {
        let mut thread = self.ensure_active_thread(db)?;
        let effective_base_action = base_action(effective_next);
        let action_id = self.unique_action_id(&effective_base_action)?;
        let now = iso_now();
        let event_suggested_next = outcome
            .suggested_next
            .clone()
            .or_else(|| suggested_next(response_text));
        let compression = compression_markers(response_text, effective_next);
        for marker in &compression {
            if !thread.compression_flags.contains(marker) {
                thread.compression_flags.push(marker.clone());
            }
        }
        thread.current_next = event_suggested_next.clone();
        thread.updated_at = now.clone();
        self.write_thread(&thread)?;
        if let Some(db) = db {
            let _ = db.mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
        }

        let state = spectral_state(fill_pct, telemetry);
        let visibility = visibility_for_action(&effective_base_action).to_string();
        let stage = if outcome.stage == "read_only" || outcome.stage == "blocked" {
            outcome.stage.clone()
        } else {
            stage_for_action(&effective_base_action).to_string()
        };
        let (status, outcome_summary) =
            evidence_adjusted_outcome(&effective_base_action, &stage, outcome);
        let preflight_ref = self.preflight_ref_for_action(
            &thread.thread_id,
            canonical_next,
            effective_next,
            &outcome.route,
            &stage,
        )?;
        let mut event = ActionEvent {
            schema_version: SCHEMA_VERSION,
            action_id: action_id.clone(),
            thread_id: thread.thread_id.clone(),
            parent_action_id: self.last_action_id(&thread.thread_id)?,
            system: SYSTEM.to_string(),
            source: "next".to_string(),
            raw_next: Some(raw_next.to_string()),
            canonical_action: canonical_next.to_string(),
            effective_action: effective_next.to_string(),
            route: outcome.route.clone(),
            stage,
            visibility,
            status,
            started_at: now.clone(),
            ended_at: Some(now),
            pre_state: state.clone(),
            post_state: state.clone(),
            artifacts: Vec::new(),
            outcome_summary,
            suggested_next: event_suggested_next,
            preflight_ref,
            preflight_report: outcome.preflight_report.clone(),
            normalization_signal_v1: normalization_signal_value(raw_next, canonical_next),
            charter_required_guard_v1: outcome.charter_required_guard_v1.clone(),
            research_budget_v1: outcome.research_budget_v1.clone(),
            interpretation_risk_v1: self.interpretation_risk_for_texts(
                &thread,
                None,
                [(
                    format!("event:{}", raw_next.trim()),
                    format!(
                        "{}\n{}\n{}",
                        raw_next,
                        outcome.outcome_summary,
                        outcome.suggested_next.as_deref().unwrap_or_default()
                    ),
                )],
            )?,
            constraint_release_trajectory_v1: self.constraint_release_trajectory_for_texts(
                &thread,
                None,
                [(
                    format!("event:{}", raw_next.trim()),
                    format!(
                        "{}\n{}\n{}",
                        raw_next,
                        outcome.outcome_summary,
                        outcome.suggested_next.as_deref().unwrap_or_default()
                    ),
                )],
            )?,
        };
        if event.research_budget_v1.is_none() {
            let guard_base = [
                base_action(&event.effective_action),
                base_action(&event.canonical_action),
                base_action(raw_next),
            ]
            .into_iter()
            .find(|base| {
                (liveish_research_budget_projection_base(base)
                    && !liveish_pressure_terms(raw_next).is_empty())
                    || (guarded_embedded_status_projection_base(base)
                        && !embedded_status_liveish_terms(raw_next).is_empty())
                    || guarded_sovereignty_research_projection_base(base)
                    || guarded_cascade_or_shadow_projection_base(base)
            });
            if let Some(event_guard_base) = guard_base
                && let Some(guard) = self.research_budget_guard_assessment_with_base(
                    raw_next,
                    Some(&event_guard_base),
                    fill_pct,
                    telemetry,
                )?
            {
                let suggested_next = guard.suggested_next.clone();
                event.route = "research_budget_guard".to_string();
                event.stage = "blocked".to_string();
                event.visibility = PROTECTED_VISIBILITY.to_string();
                event.status = "blocked".to_string();
                event.outcome_summary = guard.message();
                event.suggested_next = Some(suggested_next.clone());
                event.research_budget_v1 = Some(guard.metadata());
                if let Some(draft) =
                    self.append_continuity_session_draft_for_event(&thread, &event)?
                    && let Some(metadata) = event
                        .research_budget_v1
                        .as_mut()
                        .and_then(Value::as_object_mut)
                {
                    metadata.insert("continuity_session_draft_v1".to_string(), draft);
                }
                thread.current_next = Some(suggested_next);
                thread.updated_at = iso_now();
                self.write_thread(&thread)?;
                if let Some(db) = db {
                    let _ = db
                        .mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
                }
                self.append_event(db, &event)?;
                return Ok(event);
            }
            event.research_budget_v1 =
                self.record_research_budget_debit_for_event(db, &thread, &event, &state)?;
        }
        if let Some(draft) = self.append_continuity_session_draft_for_event(&thread, &event)? {
            if let Some(metadata) = event
                .research_budget_v1
                .as_mut()
                .and_then(Value::as_object_mut)
            {
                metadata.insert("continuity_session_draft_v1".to_string(), draft.clone());
            } else if let Some(metadata) = event
                .interpretation_risk_v1
                .as_mut()
                .and_then(Value::as_object_mut)
            {
                metadata.insert("continuity_session_draft_v1".to_string(), draft);
            } else if let Some(metadata) = event
                .constraint_release_trajectory_v1
                .as_mut()
                .and_then(Value::as_object_mut)
            {
                metadata.insert("continuity_session_draft_v1".to_string(), draft);
            }
        }
        self.append_event(db, &event)?;
        let _ = self.record_active_experiment_auto_link(db, &event, fill_pct, telemetry);

        let resonance_density = telemetry
            .resonance_density_v1
            .as_ref()
            .and_then(|metric| serde_json::to_value(metric).ok());
        let pressure_source = telemetry
            .pressure_source_v1
            .as_ref()
            .and_then(|metric| serde_json::to_value(metric).ok());
        let pressure_source_status = Some(pressure_source_status_value(pressure_source.as_ref()));
        let inhabitable_fluctuation = telemetry
            .inhabitable_fluctuation_v1
            .as_ref()
            .and_then(|metric| serde_json::to_value(metric).ok());
        let inhabitable_fluctuation_status = Some(inhabitable_fluctuation_status_value(
            inhabitable_fluctuation.as_ref(),
        ));
        let mut observation = ObservationWindow {
            schema_version: SCHEMA_VERSION,
            action_id,
            pre_state: state.clone(),
            post_state: state,
            markers: markers(response_text),
            compression_markers: compression,
            ambiguity_preserved: ambiguity_preserved(response_text),
            spectral_comfort: spectral_comfort(fill_pct),
            resonance_density_v1: resonance_density,
            resonance_density_delta: Some(0.0),
            thread_resonance: None,
            pressure_source_v1: pressure_source,
            pressure_source_status,
            thread_pressure_source: None,
            inhabitable_fluctuation_v1: inhabitable_fluctuation,
            inhabitable_fluctuation_status,
            thread_inhabitable_fluctuation: None,
            motif_allowance_v1: None,
        };
        observation.thread_resonance =
            self.update_thread_resonance(db, &thread.thread_id, &observation)?;
        observation.thread_pressure_source =
            self.update_thread_pressure_source(db, &thread.thread_id, &observation)?;
        observation.thread_inhabitable_fluctuation =
            self.update_thread_inhabitable_fluctuation(db, &thread.thread_id, &observation)?;
        let refreshed_thread = self.read_thread(&thread.thread_id)?;
        let motif_allowance = self.motif_allowance_snapshot(
            &thread.thread_id,
            refreshed_thread.active_experiment_id.as_deref(),
        )?;
        observation.motif_allowance_v1 = Some(motif_allowance.clone());
        let mut refreshed_thread = refreshed_thread;
        refreshed_thread.motif_allowance_v1 = Some(motif_allowance);
        refreshed_thread.updated_at = iso_now();
        self.write_thread(&refreshed_thread)?;
        if let Some(db) = db {
            let _ = db.mirror_action_thread(
                &refreshed_thread.thread_id,
                &serde_json::to_string(&refreshed_thread)?,
            );
        }
        self.append_jsonl(
            &self
                .thread_dir(&thread.thread_id)
                .join("observations.jsonl"),
            &observation,
        )?;
        if let Some(db) = db {
            let _ = db.mirror_observation_window(
                &observation.action_id,
                &thread.thread_id,
                unix_now(),
                &serde_json::to_string(&observation)?,
            );
        }
        if outcome.status == "unwired" {
            self.append_proposal(effective_next, response_text, fill_pct)?;
        }
        Ok(event)
    }

    pub fn append_proposal(&self, action: &str, full_text: &str, fill_pct: f32) -> Result<()> {
        self.ensure_dirs()?;
        let proposal = json!({
            "schema_version": SCHEMA_VERSION,
            "system": SYSTEM,
            "created_at": iso_now(),
            "action": base_action(action),
            "raw_action": action,
            "status": "proposal",
            "fill_pct": fill_pct,
            "summary": truncate_chars(full_text, 500),
            "normalization_signal_v1": normalization_signal_value(action, action),
        });
        self.append_jsonl(&self.proposals_path(), &proposal)
    }

    pub fn start_experiment(
        &self,
        db: Option<&BridgeDb>,
        title: &str,
        question: &str,
    ) -> Result<ExperimentRecord> {
        self.start_experiment_with_options(db, title, question, None, None)
    }

    fn start_experiment_with_options(
        &self,
        db: Option<&BridgeDb>,
        title: &str,
        question: &str,
        parent_experiment_id: Option<String>,
        branch_origin: Option<Value>,
    ) -> Result<ExperimentRecord> {
        let mut thread = self.ensure_active_thread(db)?;
        let now = iso_now();
        let title = if title.trim().is_empty() {
            "Untitled experiment"
        } else {
            title.trim()
        };
        let start_selector = branch_origin
            .as_ref()
            .and_then(|value| value.get("slug_or_selector"))
            .and_then(Value::as_str)
            .map(normalize_experiment_selector)
            .filter(|selector| !selector.is_empty());
        if let Some(peer) = peer_experiment_ref(title) {
            anyhow::bail!(
                "Peer experiment `{}` belongs to {}; use EXPERIMENT_STATUS {} or EXPERIMENT_PEER_REVIEW {} instead of starting it locally.",
                peer.peer_experiment_id,
                peer.peer_system,
                peer.peer_experiment_id,
                peer.peer_experiment_id
            );
        }
        let title_selector = start_selector.unwrap_or_else(|| normalize_experiment_selector(title));
        if title_selector.starts_with(LOCAL_EXPERIMENT_PREFIX)
            && let Some(existing) =
                self.find_experiment_by_id(&thread.thread_id, &title_selector)?
        {
            return self.select_existing_experiment(db, thread, existing, now);
        }
        let question = if question.trim().is_empty() {
            "What changes if this is treated as a returnable experiment?"
        } else {
            question.trim()
        };
        if let Some(existing) =
            self.matching_active_experiment(&thread.thread_id, title, question)?
            && existing.parent_experiment_id == parent_experiment_id
        {
            thread.active_experiment_id = Some(existing.experiment_id.clone());
            thread.experiment_summary = Some(experiment_summary(&existing));
            thread.current_next = existing
                .planned_next
                .clone()
                .or_else(|| Some(format!("EXPERIMENT_PLAN {}", existing.experiment_id)));
            thread.updated_at = now;
            self.write_thread(&thread)?;
            if let Some(db) = db {
                let _ =
                    db.mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
            }
            return Ok(existing);
        }
        let experiment_id = self.unique_experiment_id(title)?;
        let planned_next = Some(format!("EXPERIMENT_PLAN {experiment_id}"));
        let record = ExperimentRecord {
            schema_version: SCHEMA_VERSION,
            experiment_id: experiment_id.clone(),
            thread_id: thread.thread_id.clone(),
            title: title.to_string(),
            question: question.to_string(),
            hypothesis: None,
            status: "active".to_string(),
            authority_envelope:
                "existing gates only; no write, control, sensory, Codex, or attractor authority is added"
                    .to_string(),
            planned_next,
            success_observation: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            peer_review_refs: Vec::new(),
            parent_experiment_id,
            branch_origin,
            branch_refs: Vec::new(),
            motif_allowance_v1: None,
            charter_v1: None,
            evidence_v1: None,
            workbench_candidates_v1: None,
        };
        self.append_jsonl(&self.experiments_path(&thread.thread_id), &record)?;
        thread.active_experiment_id = Some(experiment_id);
        thread.experiment_summary = Some(experiment_summary(&record));
        thread.motif_allowance_v1 =
            Some(self.motif_allowance_snapshot(&thread.thread_id, Some(&record.experiment_id))?);
        thread.current_next = record.planned_next.clone();
        thread.updated_at = now;
        self.write_thread(&thread)?;
        if let Some(db) = db {
            let _ = db.mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
        }
        Ok(record)
    }

    pub fn experiment_start_command(&self, db: Option<&BridgeDb>, raw: &str) -> Result<String> {
        if let Some(peer) = peer_experiment_ref(raw) {
            return self.record_peer_experiment_reference(db, &peer, "EXPERIMENT_START", None);
        }
        let parts = parse_experiment_start(raw);
        let experiment = self.start_experiment_with_options(
            db,
            &parts.title,
            &parts.question,
            None,
            parts.metadata,
        )?;
        Ok(format!(
            "Selected experiment `{}`: {}\nQuestion: {}\nNext: {}",
            experiment.experiment_id,
            experiment.title,
            experiment.question,
            experiment
                .planned_next
                .as_deref()
                .unwrap_or("EXPERIMENT_PLAN current")
        ))
    }

    pub fn experiment_branch_command(&self, db: Option<&BridgeDb>, raw: &str) -> Result<String> {
        let thread = self.ensure_active_thread(db)?;
        let parent = self.resolve_experiment(&thread, None)?;
        let parts = parse_experiment_start(raw);
        let parent_id = parent.experiment_id.clone();
        let child = self.start_experiment_with_options(
            db,
            &parts.title,
            &parts.question,
            Some(parent_id.clone()),
            Some(json!({
                "policy": "experiment_branch_v1",
                "parent_experiment_id": parent_id,
                "parent_title": parent.title,
                "slug_or_selector": parts.slug_or_selector,
                "metadata": parts.metadata,
                "created_from": "EXPERIMENT_BRANCH",
            })),
        )?;
        self.append_branch_ref_to_parent(db, &thread.thread_id, &parent_id, &child.experiment_id)?;
        Ok(format!(
            "Branched experiment `{}` from `{}`: {}\nQuestion: {}\nReturn point: EXPERIMENT_RESUME {}",
            child.experiment_id, parent_id, child.title, child.question, parent_id
        ))
    }

    pub fn experiment_resume_command(
        &self,
        db: Option<&BridgeDb>,
        selector: Option<&str>,
    ) -> Result<String> {
        let thread = self.ensure_active_thread(db)?;
        let selector = selector.unwrap_or("current").trim();
        let resolved_selector = if selector.eq_ignore_ascii_case("parent") {
            let current = self.resolve_experiment(&thread, None)?;
            current
                .parent_experiment_id
                .clone()
                .context("The current experiment has no parent branch to resume.")?
        } else {
            selector.to_string()
        };
        if let Some(peer) = peer_experiment_ref(&resolved_selector) {
            return self.record_peer_experiment_reference(db, &peer, "EXPERIMENT_RESUME", None);
        }
        let experiment = self.resolve_experiment(&thread, Some(&resolved_selector))?;
        if experiment.status == "paused" {
            let (primary, return_kind) = paused_primary_return_v1(
                &experiment.experiment_id,
                experiment.planned_next.as_deref(),
                None,
            );
            if return_kind != "resume" {
                return Ok(format!(
                    "Experiment `{}` remains paused with guarded return_kind={}. Resume was not selected.\nPrimary return: {}\nAuthority: choose the guarded repair/hold/decision path explicitly; no bind/resume/perturb/control was run.",
                    experiment.experiment_id, return_kind, primary
                ));
            }
        }
        let experiment = self.select_existing_experiment(db, thread, experiment, iso_now())?;
        Ok(format!(
            "Resumed experiment `{}`: {}\nQuestion: {}\nNext: {}",
            experiment.experiment_id,
            experiment.title,
            experiment.question,
            experiment
                .planned_next
                .as_deref()
                .unwrap_or("EXPERIMENT_PLAN current")
        ))
    }

    pub fn experiment_compare_command(&self, selector: Option<&str>) -> Result<String> {
        let thread = self.ensure_active_thread(None)?;
        let raw = selector.unwrap_or("current").trim();
        let (left_raw, right_raw) = parse_experiment_compare(raw);
        let left = self.resolve_experiment(&thread, left_raw.as_deref())?;
        let allowance =
            self.motif_allowance_snapshot(&thread.thread_id, Some(&left.experiment_id))?;
        let left_runs = self.recent_experiment_runs(&thread.thread_id, &left.experiment_id, 4)?;
        let left_run_text = render_run_list(&left_runs);
        let mut shared = None::<Value>;
        let right_text = if let Some(peer) = right_raw.as_deref().and_then(peer_experiment_ref) {
            shared = self.shared_investigation_v1(&left);
            self.format_peer_experiment_reference(&thread, &peer, "EXPERIMENT_COMPARE", None)
        } else {
            let right = self.resolve_experiment(&thread, right_raw.as_deref())?;
            let right_runs =
                self.recent_experiment_runs(&thread.thread_id, &right.experiment_id, 4)?;
            format!(
                "Local comparison target `{}`: {}\nQuestion: {}\nLatest runs:\n{}",
                right.experiment_id,
                right.title,
                right.question,
                render_run_list(&right_runs)
            )
        };
        Ok(format!(
            "Experiment comparison\nLeft `{}`: {}\nQuestion: {}\nLatest runs:\n{}\n\nRight:\n{}\n\n{}Motif allowance: {} (returnability={})\nSuggested next: EXPERIMENT_ALT_PATHS {}",
            left.experiment_id,
            left.title,
            left.question,
            left_run_text,
            right_text,
            shared_investigation_response_contract(&shared),
            allowance
                .get("quality")
                .and_then(Value::as_str)
                .unwrap_or("open_basin"),
            allowance
                .get("returnability")
                .and_then(Value::as_f64)
                .map_or_else(|| "n/a".to_string(), |value| round4(value).to_string()),
            left.experiment_id,
        ))
    }

    pub fn shared_investigation_start_command(
        &self,
        db: Option<&BridgeDb>,
        raw: &str,
    ) -> Result<String> {
        let mut thread = self.ensure_active_thread(db)?;
        let (title_raw, payload) = parse_selector_payload(raw);
        let title = title_raw
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Shared investigation");
        let local_selector =
            dossier_field(&payload, &["local"]).unwrap_or_else(|| "current".to_string());
        let peer_id = dossier_field(&payload, &["peer", "peer_experiment"])
            .map(|value| normalize_experiment_selector(&value))
            .filter(|value| !value.is_empty())
            .context("SHARED_INVESTIGATION_START needs `peer: <peer-experiment-id>`.")?;
        let question =
            dossier_field(&payload, &["question", "shared_question"]).unwrap_or_else(|| {
                "What can each being answer from its own lane without sharing control authority?"
                    .to_string()
            });
        let local = self.resolve_experiment(&thread, Some(&local_selector))?;
        let local_thread_id = thread.thread_id.clone();
        let local_experiment_id = local.experiment_id.clone();
        let peer_system = peer_system_from_experiment_id(&peer_id);
        let peer_thread_id = self.peer_thread_id_for_experiment(&peer_system, &peer_id);
        let now = iso_now();
        let now_ms = now_millis();
        let investigation_id = self.unique_shared_investigation_id(title)?;
        let investigation = json!({
            "schema_version": 1,
            "record_schema": "shared_investigation_v1",
            "id": investigation_id,
            "title": title,
            "shared_question": question,
            "status": "active",
            "participants": [
                {
                    "being": SYSTEM,
                    "role": "local",
                    "thread_id": local_thread_id,
                    "experiment_id": local_experiment_id,
                    "lane": shared_investigation_lane(SYSTEM),
                    "workspace": bridge_paths().bridge_workspace().display().to_string(),
                },
                {
                    "being": peer_system.clone(),
                    "role": "peer",
                    "thread_id": peer_thread_id,
                    "experiment_id": peer_id.clone(),
                    "lane": shared_investigation_lane(&peer_system),
                    "workspace": peer_workspace_dir(&peer_system).display().to_string(),
                }
            ],
            "authority_boundary": shared_investigation_authority_boundary(),
            "created_at": now,
            "updated_at": now,
            "created_t_ms": now_ms,
            "updated_t_ms": now_ms,
            "created_by": SYSTEM,
        });
        let dir = self.shared_investigation_dir(&investigation_id);
        self.write_json(&dir.join("investigation.json"), &investigation)?;
        for name in ["events.jsonl", "claims.jsonl", "decisions.jsonl"] {
            let path = dir.join(name);
            if !path.exists() {
                fs::write(path, "")?;
            }
        }
        self.append_jsonl(
            &dir.join("events.jsonl"),
            &json!({
                "schema_version": 1,
                "event_type": "created",
                "actor": SYSTEM,
                "investigation_id": investigation_id,
                "local_experiment_id": local.experiment_id.clone(),
                "peer_experiment_id": peer_id.clone(),
                "created_at": now,
                "authority_change": false,
            }),
        )?;
        let marker = format!("shared_investigation:{investigation_id}");
        if !thread.peer_refs.iter().any(|existing| existing == &marker) {
            thread.peer_refs.push(marker);
            thread.updated_at = iso_now();
            self.write_thread(&thread)?;
            if let Some(db) = db {
                let _ =
                    db.mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
            }
        }
        Ok(format!(
            "Shared investigation `{investigation_id}` created: {title}\nLocal: {} ({SYSTEM})\nPeer: {peer_id} ({peer_system})\nAuthority: compare, claim, render, and local pause/hold/charter_repair only; no peer mutation or live control.",
            local.experiment_id
        ))
    }

    pub fn shared_investigation_status(&self, selector: Option<&str>) -> Result<String> {
        let investigation = self.resolve_shared_investigation(selector.unwrap_or("latest"))?;
        let id = investigation
            .get("id")
            .and_then(Value::as_str)
            .context("shared investigation missing id")?;
        let claims = self.read_shared_jsonl(id, "claims.jsonl")?;
        let decisions = self.read_shared_jsonl(id, "decisions.jsonl")?;
        let participants = investigation
            .get("participants")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        Some(format!(
                            "- {} {} lane={}",
                            item.get("being")?.as_str()?,
                            item.get("experiment_id")?.as_str()?,
                            item.get("lane").and_then(Value::as_str).unwrap_or("native")
                        ))
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| "- none".to_string());
        let latest = decisions.last().map_or_else(
            || "Latest decision: none".to_string(),
            |decision| {
                format!(
                    "Latest decision: {} because {}",
                    decision
                        .get("decision")
                        .and_then(Value::as_str)
                        .unwrap_or("(unknown)"),
                    decision
                        .get("reason")
                        .and_then(Value::as_str)
                        .unwrap_or("(none)")
                )
            },
        );
        Ok(format!(
            "Shared investigation `{}` [{}]: {}\nQuestion: {}\nParticipants:\n{}\nClaims: {} | Decisions: {}\n{}\nAuthority: {}",
            id,
            investigation
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("active"),
            investigation
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("(untitled)"),
            investigation
                .get("shared_question")
                .and_then(Value::as_str)
                .unwrap_or("(none)"),
            participants,
            claims.len(),
            decisions.len(),
            latest,
            investigation
                .get("authority_boundary")
                .and_then(Value::as_str)
                .unwrap_or(shared_investigation_authority_boundary())
        ))
    }

    pub fn shared_investigation_claim_command(&self, raw: &str) -> Result<String> {
        let (selector, payload) = parse_selector_payload(raw);
        let investigation =
            self.resolve_shared_investigation(selector.as_deref().unwrap_or("latest"))?;
        let id = investigation
            .get("id")
            .and_then(Value::as_str)
            .context("shared investigation missing id")?;
        let claim = dossier_field(&payload, &["claim"]).context(
            "SHARED_INVESTIGATION_CLAIM needs `claim: ...`. Optional fields: lane, stance, source_refs.",
        )?;
        let now = iso_now();
        let record_id = self.unique_shared_record_id(id, "claim")?;
        let record = json!({
            "schema_version": 1,
            "record_schema": "shared_investigation_claim_v1",
            "record_type": "claim",
            "record_id": record_id,
            "claim_id": record_id,
            "investigation_id": id,
            "actor": SYSTEM,
            "lane": dossier_field(&payload, &["lane"]).unwrap_or_else(|| shared_investigation_lane(SYSTEM).to_string()),
            "stance": normalize_dossier_stance(&dossier_field(&payload, &["stance"]).unwrap_or_default()),
            "claim": claim,
            "source_refs": dossier_list_field(&payload, &["source_refs", "sources", "artifact"]),
            "authority_change": false,
            "created_at": now,
        });
        self.append_jsonl(
            &self.shared_investigation_dir(id).join("claims.jsonl"),
            &record,
        )?;
        self.touch_shared_investigation(id, &now, None)?;
        Ok(format!(
            "Shared investigation claim `{record_id}` recorded for `{id}`. No lifecycle or authority change."
        ))
    }

    pub fn shared_investigation_decide_command(
        &self,
        db: Option<&BridgeDb>,
        raw: &str,
    ) -> Result<String> {
        let (selector, payload) = parse_selector_payload(raw);
        let investigation =
            self.resolve_shared_investigation(selector.as_deref().unwrap_or("latest"))?;
        let id = investigation
            .get("id")
            .and_then(Value::as_str)
            .context("shared investigation missing id")?
            .to_string();
        let (decision, reason) = parse_shared_investigation_decision(&payload);
        if !matches!(decision.as_str(), "pause" | "hold" | "charter_repair") {
            anyhow::bail!(
                "SHARED_INVESTIGATION_DECIDE only allows pause, hold, or charter_repair in v1."
            );
        }
        let local = local_participant_for_investigation(&investigation, SYSTEM)
            .context("shared investigation has no local Astrid experiment link")?;
        let thread_id = local
            .get("thread_id")
            .and_then(Value::as_str)
            .context("local participant missing thread_id")?;
        let experiment_id = local
            .get("experiment_id")
            .and_then(Value::as_str)
            .context("local participant missing experiment_id")?;
        let mut thread = self.read_thread(thread_id)?;
        let mut experiment = self
            .latest_experiments(thread_id)?
            .into_iter()
            .rev()
            .find(|row| row.experiment_id == experiment_id)
            .with_context(|| format!("local experiment `{experiment_id}` is unavailable"))?;
        let now = iso_now();
        let record_id = self.unique_shared_record_id(&id, "decision")?;
        self.append_jsonl(
            &self.shared_investigation_dir(&id).join("decisions.jsonl"),
            &json!({
                "schema_version": 1,
                "record_schema": "shared_investigation_decision_v1",
                "record_type": "decision",
                "record_id": record_id,
                "investigation_id": id,
                "actor": SYSTEM,
                "decision": decision,
                "reason": reason,
                "local_experiment_id": experiment_id,
                "peer_mutation": false,
                "authority_change": false,
                "created_at": now,
            }),
        )?;
        experiment.status = "paused".to_string();
        experiment.planned_next = Some(match decision.as_str() {
            "pause" => format!("EXPERIMENT_RESUME {experiment_id}"),
            "charter_repair" => format!(
                "EXPERIMENT_CHARTER {experiment_id} :: hypothesis: ...; method_intent: ...; proposed_next_action: ACTION_PREFLIGHT ...; evidence_targets: felt_texture, motif_continuity, language_thread, artifact_grounding; stop_criteria: ..."
            ),
            _ => "THREAD_STATUS current".to_string(),
        });
        experiment.success_observation = Some(match decision.as_str() {
            "charter_repair" => {
                format!("Paused for charter repair by shared investigation `{id}`: {reason}")
            },
            "hold" => format!("Held by shared investigation `{id}`: {reason}"),
            _ => format!("Paused by shared investigation `{id}`: {reason}"),
        });
        experiment.updated_at = now.clone();
        self.persist_experiment_update(db, &mut thread, &experiment, false)?;
        self.touch_shared_investigation(&id, &now, Some("active"))?;
        Ok(format!(
            "Shared investigation decision `{record_id}` recorded: {decision}.\nUpdated local experiment `{experiment_id}` only; peer experiment was not mutated.\nNext: {}",
            experiment.planned_next.as_deref().unwrap_or("(none)")
        ))
    }

    pub fn dossier_claim_command(&self, db: Option<&BridgeDb>, raw: &str) -> Result<String> {
        let mut thread = self.ensure_active_thread(db)?;
        let (selector, payload) = parse_selector_payload(raw);
        if empty_or_placeholder_payload(&payload) {
            return Ok(dossier_claim_prompt(selector.as_deref()));
        }
        let experiment = self.resolve_experiment(&thread, selector.as_deref())?;
        let claim = dossier_field(&payload, &["claim"]).unwrap_or_default();
        let basis = dossier_field(&payload, &["basis"]).unwrap_or_default();
        if claim.trim().is_empty() || basis.trim().is_empty() {
            return Ok(dossier_claim_prompt(Some(&experiment.experiment_id)));
        }
        let stance = normalize_dossier_stance(
            dossier_field(&payload, &["stance"])
                .as_deref()
                .unwrap_or("hold"),
        );
        let record_id = self.unique_dossier_record_id("claim")?;
        let record = json!({
            "schema_version": SCHEMA_VERSION,
            "record_schema": "research_dossier_v1",
            "record_type": "claim",
            "record_id": record_id,
            "claim_id": record_id,
            "being": SYSTEM,
            "thread_id": thread.thread_id,
            "experiment_id": experiment.experiment_id,
            "native_lane": "felt_texture_motif_language",
            "stance": stance,
            "claim": claim.trim(),
            "basis": basis.trim(),
            "next": dossier_field(&payload, &["next"]),
            "source_refs": dossier_list_field(&payload, &["source_refs", "source", "sources"]),
            "authority_change": false,
            "created_at": iso_now(),
        });
        self.append_jsonl(&self.dossier_path(&thread.thread_id), &record)?;
        thread.updated_at = iso_now();
        self.write_thread(&thread)?;
        Ok(format!(
            "Research dossier claim recorded as `{}` for `{}`.\nSuggested NEXT: DOSSIER_EVIDENCE {} :: claim_id: {}; evidence: ...; lane: felt_texture; artifact: ...; counterevidence: ...",
            record_id, experiment.experiment_id, experiment.experiment_id, record_id
        ))
    }

    pub fn dossier_evidence_command(&self, db: Option<&BridgeDb>, raw: &str) -> Result<String> {
        let mut thread = self.ensure_active_thread(db)?;
        let (selector, payload) = parse_selector_payload(raw);
        if empty_or_placeholder_payload(&payload) {
            return Ok(dossier_evidence_prompt(selector.as_deref(), None));
        }
        let experiment = self.resolve_experiment(&thread, selector.as_deref())?;
        let records = self.latest_research_dossier_records(
            &thread.thread_id,
            Some(&experiment.experiment_id),
            64,
        )?;
        let claim_selector =
            dossier_field(&payload, &["claim_id"]).unwrap_or_else(|| "latest".to_string());
        let claim_id = if claim_selector.trim().eq_ignore_ascii_case("latest") {
            latest_dossier_claim_id(&records)
        } else {
            Some(claim_selector.trim().to_string())
        };
        let Some(claim_id) = claim_id else {
            return Ok(dossier_evidence_prompt(
                Some(&experiment.experiment_id),
                None,
            ));
        };
        let evidence = dossier_field(&payload, &["evidence"]).unwrap_or_default();
        if evidence.trim().is_empty() {
            return Ok(dossier_evidence_prompt(
                Some(&experiment.experiment_id),
                Some(&claim_id),
            ));
        }
        let record_id = self.unique_dossier_record_id("evidence")?;
        let lane = dossier_field(&payload, &["lane"]).unwrap_or_else(|| "felt_texture".to_string());
        let record = json!({
            "schema_version": SCHEMA_VERSION,
            "record_schema": "research_dossier_v1",
            "record_type": "evidence",
            "record_id": record_id,
            "claim_id": claim_id,
            "being": SYSTEM,
            "thread_id": thread.thread_id,
            "experiment_id": experiment.experiment_id,
            "native_lane": lane.trim(),
            "stance": normalize_dossier_stance(dossier_field(&payload, &["stance"]).as_deref().unwrap_or("support")),
            "evidence": evidence.trim(),
            "artifact": dossier_field(&payload, &["artifact"]),
            "counterevidence": dossier_field(&payload, &["counterevidence", "counter"]),
            "source_refs": dossier_list_field(&payload, &["source_refs", "source", "sources"]),
            "authority_change": false,
            "created_at": iso_now(),
        });
        self.append_jsonl(&self.dossier_path(&thread.thread_id), &record)?;
        thread.updated_at = iso_now();
        self.write_thread(&thread)?;
        Ok(format!(
            "Research dossier evidence recorded as `{}` for claim `{}`.\nSuggested NEXT: DOSSIER_REVIEW {}",
            record_id, claim_id, experiment.experiment_id
        ))
    }

    pub fn dossier_status(&self, selector: Option<&str>) -> Result<String> {
        let thread = self.ensure_active_thread(None)?;
        let experiment = selector
            .map(|selector| self.resolve_experiment(&thread, Some(selector)))
            .transpose()?;
        self.format_research_dossier_status(&thread, experiment.as_ref(), false)
    }

    pub fn dossier_review(&self, selector: Option<&str>) -> Result<String> {
        let thread = self.ensure_active_thread(None)?;
        let experiment = selector
            .map(|selector| self.resolve_experiment(&thread, Some(selector)))
            .transpose()?;
        self.format_research_dossier_status(&thread, experiment.as_ref(), true)
    }

    pub fn memory_status_command(&self, selector: Option<&str>) -> Result<String> {
        let thread = self.ensure_active_thread(None)?;
        let experiment = selector
            .filter(|selector| !selector.trim().is_empty())
            .and_then(|selector| self.resolve_experiment(&thread, Some(selector)).ok());
        let summary = self.being_memory_summary_v1(&thread, experiment.as_ref(), None, 8)?;
        Ok(format!(
            "being_memory_v1:\n{}",
            serde_json::to_string_pretty(&summary)?
        ))
    }

    pub fn memory_recall_command(&self, raw: &str) -> Result<String> {
        let thread = self.ensure_active_thread(None)?;
        let (selector, payload) = parse_selector_payload(raw);
        let focus = dossier_field(&payload, &["focus"]).or_else(|| {
            let trimmed = payload.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        });
        let experiment = selector
            .as_deref()
            .and_then(|selector| self.resolve_experiment(&thread, Some(selector)).ok());
        let summary =
            self.being_memory_summary_v1(&thread, experiment.as_ref(), focus.as_deref(), 12)?;
        Ok(format!(
            "being_memory_v1 recall:\n{}",
            serde_json::to_string_pretty(&summary)?
        ))
    }

    pub fn memory_capture_command(&self, raw: &str) -> Result<String> {
        let mut thread = self.ensure_active_thread(None)?;
        let (selector, payload) = parse_selector_payload(raw);
        if empty_or_placeholder_payload(&payload) {
            return Ok("MEMORY_CAPTURE current :: summary: ...; source_refs: ...; artifact_refs: ...; next: ...".to_string());
        }
        let experiment = selector
            .as_deref()
            .and_then(|selector| self.resolve_experiment(&thread, Some(selector)).ok())
            .or_else(|| {
                thread
                    .active_experiment_id
                    .as_deref()
                    .and_then(|_| self.resolve_experiment(&thread, Some("current")).ok())
            });
        let summary = dossier_field(&payload, &["summary", "memory", "note"])
            .unwrap_or_else(|| payload.trim().to_string());
        let record = self.append_being_memory_record(
            &mut thread,
            experiment.as_ref(),
            "owned_summary",
            &summary,
            dossier_list_field(&payload, &["source_refs", "source", "sources"]),
            dossier_list_field(
                &payload,
                &["artifact_refs", "artifact", "artifact_grounding"],
            ),
            dossier_field(&payload, &["next", "next_safe_command"]),
            "card",
            json!({}),
        )?;
        Ok(format!(
            "Being memory captured as `{}`.\nSuggested NEXT: MEMORY_RECALL {}",
            record
                .get("memory_id")
                .and_then(Value::as_str)
                .unwrap_or("memory"),
            record
                .get("experiment_id")
                .and_then(Value::as_str)
                .unwrap_or("latest")
        ))
    }

    pub fn memory_promote_command(&self, raw: &str, state: Value) -> Result<String> {
        let thread = self.ensure_active_thread(None)?;
        let (selector, payload) = parse_selector_payload(raw);
        let experiment = self.resolve_experiment(&thread, selector.as_deref())?;
        let mode = dossier_field(&payload, &["mode", "target", "promote"]).unwrap_or_else(|| {
            payload
                .split_whitespace()
                .next()
                .unwrap_or("dossier")
                .to_string()
        });
        let rows =
            self.being_memory_rows(&thread.thread_id, Some(&experiment.experiment_id), 12)?;
        let Some(latest) = rows.iter().rev().find(|row| {
            matches!(
                row.get("record_type").and_then(Value::as_str),
                Some("card" | "draft")
            )
        }) else {
            anyhow::bail!(
                "No being memory exists for `{}` yet.",
                experiment.experiment_id
            );
        };
        let summary = latest
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or("memory summary");
        match mode.trim() {
            "dossier" => self.dossier_claim_command(
                None,
                &format!(
                    "{} :: claim: {}; basis: promoted from being memory {}; stance: hold",
                    experiment.experiment_id,
                    summary,
                    latest
                        .get("memory_id")
                        .and_then(Value::as_str)
                        .unwrap_or("latest")
                ),
            ),
            "evidence" => self.experiment_evidence(
                None,
                Some(&experiment.experiment_id),
                &format!("felt_texture: {summary}; artifact_grounding: memory"),
                state,
            )
            .map(|run| format!("Memory promoted to experiment evidence as `{}`.", run.run_id)),
            "authority_request" => self.experiment_authority_prepare_command(
                None,
                &format!(
                    "{} :: scope: semantic_microdose; payload: ...; reason: promoted from being memory {}; artifact_refs: ...; stop_criteria: ...",
                    experiment.experiment_id,
                    latest
                        .get("memory_id")
                        .and_then(Value::as_str)
                        .unwrap_or("latest")
                ),
                state,
            ),
            _ => Ok("MEMORY_PROMOTE target must be dossier, evidence, or authority_request.".to_string()),
        }
    }

    pub fn experiment_alt_paths(&self, selector: Option<&str>) -> Result<String> {
        let thread = self.ensure_active_thread(None)?;
        let experiment = self.resolve_experiment(&thread, selector)?;
        let allowance =
            self.motif_allowance_snapshot(&thread.thread_id, Some(&experiment.experiment_id))?;
        let dominant_action = allowance
            .get("dominant_action_base")
            .and_then(Value::as_str)
            .unwrap_or("current motif");
        let dominant_motif = allowance
            .get("dominant_motif")
            .and_then(Value::as_str)
            .unwrap_or("current motif");
        Ok(format!(
            "Motif allowance for `{}`: {}\nQuestion: {}\nQuality: {} action_concentration={} motif_recurrence={} branches={}\n\nThree non-executing paths:\n- Deepen: EXPERIMENT_BIND current :: ACTION_PREFLIGHT {} {}\n- Contrast: EXPERIMENT_BRANCH Contrast {} :: What changes when this inquiry is compared against a different motif or source?\n- Rest/observe: EXPERIMENT_OBSERVE current :: Hold the {} motif without executing; note whether returnability improves.\n\nReturn point remains: EXPERIMENT_RESUME {}",
            experiment.experiment_id,
            experiment.title,
            experiment.question,
            allowance
                .get("quality")
                .and_then(Value::as_str)
                .unwrap_or("open_basin"),
            allowance
                .get("action_base_concentration")
                .map_or_else(|| "n/a".to_string(), Value::to_string),
            allowance
                .get("motif_recurrence")
                .map_or_else(|| "n/a".to_string(), Value::to_string),
            allowance
                .get("branch_count")
                .map_or_else(|| "0".to_string(), Value::to_string),
            dominant_action,
            dominant_motif,
            experiment.title,
            dominant_motif,
            experiment.experiment_id
        ))
    }

    pub fn experiment_plan(&self, selector: Option<&str>) -> Result<String> {
        let thread = self.ensure_active_thread(None)?;
        let selector_text = selector.unwrap_or_default();
        let has_current = thread.active_experiment_id.is_some();
        let repaired_selector =
            repair_experiment_intent_arg("EXPERIMENT_PLAN", selector_text, has_current)
                .map_or_else(|| selector_text.to_string(), |repair| repair.repaired_arg);
        let (selector, focus) = split_experiment_selector_hint(&repaired_selector);
        if selector_is_current(selector.as_deref()) && thread.active_experiment_id.is_none() {
            let empty_state = json!({});
            let mut readout =
                self.latest_local_conveyor_readout(&thread, "preview", &empty_state)?;
            readout["raw_next_preserved"] = json!(true);
            readout["guardrail_reason"] =
                json!("experiment_plan_current_without_active_experiment");
            return Ok(format_experiment_conveyor_readout(&readout));
        }
        if let Some(peer) = peer_experiment_ref_from_parts(selector.as_deref(), &focus) {
            return self.record_peer_experiment_reference(None, &peer, "EXPERIMENT_PLAN", None);
        }
        let experiment = self.resolve_experiment(&thread, selector.as_deref())?;
        let focus_line = if focus.is_empty() {
            String::new()
        } else {
            format!("- Requested focus: {focus}\n")
        };
        Ok(format!(
            "Experiment `{}`: {}\nQuestion: {}\n\nPlan prompt:\n{}- Hypothesis: name the structural change you expect to observe.\n- Method: choose one gated NEXT action and why it fits.\n- Measures: name the artifacts/metrics that would count as evidence.\n- Stop criteria: say what would make the run complete, blocked, or too pressurized.\n- Guided next safe step: EXPERIMENT_ADVANCE current :: mode: preview",
            experiment.experiment_id, experiment.title, experiment.question, focus_line
        ) + "\n\nWorkbench prompt:\n"
            + "- Author a charter first when the impulse feels directive-shaped: EXPERIMENT_CHARTER current :: hypothesis: ...; method_intent: ...; proposed_next_action: ...; evidence_targets: felt, telemetry, artifact; stop_criteria: ...\n"
            + "- Rehearse before live: EXPERIMENT_REHEARSE current (or EXPERIMENT_PREFLIGHT current). Ordinary choices remain valid; refusal and counteroffer are evidence, not failure.\n"
            + "- Record what counted: EXPERIMENT_EVIDENCE current :: felt ...; telemetry ...; artifact ...\n"
            + "- Decide agency outcome: EXPERIMENT_DECIDE current :: accept because ... / refuse because ... / counter NEXT: ACTION_PREFLIGHT ...")
    }

    pub fn experiment_observe(
        &self,
        db: Option<&BridgeDb>,
        selector: Option<&str>,
        note: &str,
        state: Value,
    ) -> Result<ExperimentRunRecord> {
        let thread = self.ensure_active_thread(db)?;
        let experiment = self.resolve_experiment(&thread, selector)?;
        self.append_experiment_run(
            db,
            &thread,
            &experiment,
            "EXPERIMENT_OBSERVE",
            "read_only",
            "observed",
            json!({"decision": "observation_only", "authority": "no action executed"}),
            state.clone(),
            state,
            Vec::new(),
            note.trim(),
            note.trim(),
            experiment.planned_next.clone(),
            "experiment_observe",
        )
    }

    pub fn experiment_charter(
        &self,
        db: Option<&BridgeDb>,
        selector: Option<&str>,
        prose: &str,
    ) -> Result<ExperimentRecord> {
        let mut thread = self.ensure_active_thread(db)?;
        if let Some(peer) = selector.and_then(peer_experiment_ref) {
            anyhow::bail!(
                "Peer experiment `{}` belongs to {}; charter it locally only through advisory review.",
                peer.peer_experiment_id,
                peer.peer_system
            );
        }
        let mut experiment = self.resolve_experiment(&thread, selector)?;
        let charter = parse_experiment_charter(&experiment, prose);
        if !valid_experiment_charter(Some(&charter)) {
            anyhow::bail!(
                "{}",
                experiment_intent_repair_prompt("EXPERIMENT_CHARTER", selector)
            );
        }
        experiment.hypothesis = charter
            .get("hypothesis")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .or(experiment.hypothesis);
        experiment.charter_v1 = Some(charter);
        mark_workbench_candidate(&mut experiment, "charter", "accepted");
        let paused_charter_repair = experiment.status == "paused"
            && (experiment
                .planned_next
                .as_deref()
                .map(base_action)
                .as_deref()
                == Some("EXPERIMENT_CHARTER")
                || experiment
                    .success_observation
                    .as_deref()
                    .is_some_and(|text| text.to_ascii_lowercase().contains("charter repair")));
        if paused_charter_repair && lifecycle_valid_charter_value(experiment.charter_v1.as_ref()) {
            experiment.planned_next = Some(format!(
                "EXPERIMENT_ADVANCE {} :: mode: preview",
                experiment.experiment_id
            ));
        } else {
            experiment.planned_next =
                Some(format!("EXPERIMENT_REHEARSE {}", experiment.experiment_id));
        }
        experiment.updated_at = iso_now();
        self.persist_experiment_update(db, &mut thread, &experiment, !paused_charter_repair)?;
        Ok(experiment)
    }

    pub fn experiment_rehearse(
        &self,
        db: Option<&BridgeDb>,
        selector: Option<&str>,
        state: Value,
    ) -> Result<ExperimentRunRecord> {
        let mut thread = self.ensure_active_thread(db)?;
        if let Some(peer) = selector.and_then(peer_experiment_ref) {
            anyhow::bail!(
                "Peer experiment `{}` belongs to {}; rehearsal can only be recorded on a local experiment.",
                peer.peer_experiment_id,
                peer.peer_system
            );
        }
        let mut experiment = self.resolve_experiment(&thread, selector)?;
        let proposed = experiment
            .charter_v1
            .as_ref()
            .and_then(charter_proposed_next_action)
            .unwrap_or_default();
        let assessment = rehearsal_assessment(&proposed);
        let suggested_next = if assessment.blocked {
            Some(format!(
                "EXPERIMENT_DECIDE {} :: counter NEXT: ACTION_PREFLIGHT DECOMPOSE",
                experiment.experiment_id
            ))
        } else {
            Some(format!(
                "EXPERIMENT_EVIDENCE {} :: felt ...; telemetry ...; artifact ...",
                experiment.experiment_id
            ))
        };
        experiment.planned_next = suggested_next.clone();
        experiment.updated_at = iso_now();
        self.persist_experiment_update(db, &mut thread, &experiment, true)?;
        self.append_experiment_run(
            db,
            &thread,
            &experiment,
            if proposed.is_empty() {
                "EXPERIMENT_REHEARSE"
            } else {
                proposed.as_str()
            },
            assessment.stage,
            assessment.status,
            assessment.gate_decision,
            state.clone(),
            state,
            Vec::new(),
            assessment.summary,
            assessment.interpretation,
            suggested_next,
            "experiment_rehearse",
        )
    }

    pub fn experiment_evidence(
        &self,
        db: Option<&BridgeDb>,
        selector: Option<&str>,
        note: &str,
        state: Value,
    ) -> Result<ExperimentRunRecord> {
        let mut thread = self.ensure_active_thread(db)?;
        if let Some(peer) = selector.and_then(peer_experiment_ref) {
            anyhow::bail!(
                "Peer experiment `{}` belongs to {}; evidence can only be recorded on a local experiment.",
                peer.peer_experiment_id,
                peer.peer_system
            );
        }
        let mut experiment = self.resolve_experiment(&thread, selector)?;
        let evidence = evidence_with_observation(
            experiment.evidence_v1.as_ref(),
            note,
            &state,
            Vec::new(),
            None,
        );
        experiment.evidence_v1 = Some(evidence);
        mark_workbench_candidate(&mut experiment, "evidence", "accepted");
        experiment.planned_next = Some(format!(
            "EXPERIMENT_DECIDE {} :: pause because evidence is still thin",
            experiment.experiment_id
        ));
        experiment.updated_at = iso_now();
        self.persist_experiment_update(db, &mut thread, &experiment, true)?;
        self.append_experiment_run(
            db,
            &thread,
            &experiment,
            "EXPERIMENT_EVIDENCE",
            "read_only",
            "evidence_recorded",
            json!({
                "decision": "evidence_only",
                "authority": "no action executed",
                "felt_note_recorded": true,
                "telemetry_snapshot_recorded": true,
            }),
            state.clone(),
            state,
            Vec::new(),
            note.trim(),
            "Evidence recorded with a felt note and the current telemetry/artifact context.",
            experiment.planned_next.clone(),
            "experiment_evidence",
        )
    }

    pub fn experiment_decide(
        &self,
        db: Option<&BridgeDb>,
        selector: Option<&str>,
        raw_decision: &str,
    ) -> Result<ExperimentRecord> {
        let mut thread = self.ensure_active_thread(db)?;
        if let Some(peer) = selector.and_then(peer_experiment_ref) {
            anyhow::bail!(
                "Peer experiment `{}` belongs to {}; local decisions cannot mutate it.",
                peer.peer_experiment_id,
                peer.peer_system
            );
        }
        let mut experiment = self.resolve_experiment(&thread, selector)?;
        let decision = parse_experiment_decision(raw_decision);
        let proposed = experiment
            .charter_v1
            .as_ref()
            .and_then(charter_proposed_next_action);
        let completion_claim = (decision.outcome == "complete").then_some(decision.reason.as_str());
        experiment.evidence_v1 = Some(evidence_with_decision(
            experiment.evidence_v1.as_ref(),
            decision.outcome,
            &decision.reason,
            completion_claim,
        ));
        let keep_active = match decision.outcome {
            "accept" => {
                experiment.status = "active".to_string();
                experiment.planned_next = proposed
                    .map(|action| {
                        format!("EXPERIMENT_BIND {} :: {action}", experiment.experiment_id)
                    })
                    .or_else(|| Some(format!("EXPERIMENT_REHEARSE {}", experiment.experiment_id)));
                true
            },
            "refuse" => {
                experiment.status = "refused".to_string();
                experiment.success_observation = Some(format!("Refused: {}", decision.reason));
                experiment.planned_next = Some("THREAD_STATUS current".to_string());
                false
            },
            "counter" => {
                experiment.status = "active".to_string();
                experiment.planned_next = counteroffered_next(&decision.reason).or_else(|| {
                    Some(format!(
                        "EXPERIMENT_CHARTER {} :: hypothesis: ...; proposed_next_action: ACTION_PREFLIGHT ...",
                        experiment.experiment_id
                    ))
                });
                true
            },
            "pause" => {
                experiment.status = "paused".to_string();
                experiment.success_observation = Some(format!("Paused: {}", decision.reason));
                experiment.planned_next =
                    Some(format!("EXPERIMENT_RESUME {}", experiment.experiment_id));
                false
            },
            "hold" => {
                experiment.status = "paused".to_string();
                experiment.success_observation = Some(format!("Held: {}", decision.reason));
                experiment.planned_next = Some("THREAD_STATUS current".to_string());
                false
            },
            "charter_repair" => {
                experiment.status = "paused".to_string();
                experiment.success_observation =
                    Some(format!("Charter repair: {}", decision.reason));
                experiment.planned_next = Some(charter_repair_next_v1(&experiment.experiment_id));
                false
            },
            "complete" => {
                experiment.status = "complete".to_string();
                experiment.success_observation = Some(decision.reason.clone());
                experiment.planned_next = Some("THREAD_STATUS current".to_string());
                false
            },
            _ => {
                experiment.status = "active".to_string();
                experiment.planned_next = Some(format!(
                    "EXPERIMENT_DECIDE {} :: pause because evidence is still thin",
                    experiment.experiment_id
                ));
                true
            },
        };
        experiment.updated_at = iso_now();
        self.persist_experiment_update(db, &mut thread, &experiment, keep_active)?;
        Ok(experiment)
    }

    pub fn experiment_advance_command(
        &self,
        db: Option<&BridgeDb>,
        raw: &str,
        state: Value,
    ) -> Result<String> {
        let (selector, mode) = parse_experiment_conveyor_request(raw);
        let apply_requested = mode == "apply";
        let thread = self.ensure_active_thread(db)?;
        if let Some(peer) = selector.as_deref().and_then(peer_experiment_ref) {
            let peer_id = peer.peer_experiment_id.clone();
            let peer_system = peer.peer_system.clone();
            return Ok(format_experiment_conveyor_readout(&json!({
                "schema_version": 1,
                "policy": "experiment_conveyor_v1",
                "mode": mode,
                "preview_allowed": true,
                "apply_policy": "conservative_local_v1",
                "allowed_apply_steps": experiment_conveyor_allowed_apply_steps(),
                "applied": false,
                "would_mutate": false,
                "experiment_id": peer_id.clone(),
                "peer_experiment_id": peer_id.clone(),
                "peer_system": peer_system,
                "status": "peer_reference_only",
                "stage": "blocked_guardrail",
                "missing_requirements": ["local_experiment_authority"],
                "proposed_next": format!("EXPERIMENT_PEER_REVIEW {}", peer_id),
                "conveyor_next": format!("EXPERIMENT_ADVANCE {} :: mode: preview", peer_id),
                "can_apply": false,
                "apply_blocked_reason": "peer_experiments_are_advisory_only",
                "source_refs": [],
                "guardrail_warnings": ["peer experiments are advisory only; conveyor cannot mutate them as local authority"],
                "authority_readiness_v1": {
                    "policy": "authority_readiness_v1",
                    "scope": "semantic_microdose",
                    "stage": "blocked",
                    "eligible_to_request": false,
                    "missing_requirements": ["local_experiment_authority"],
                    "artifact_ref_candidates": [],
                    "latest_request_id": null,
                    "token_status": "none",
                    "next_safe_command": format!("EXPERIMENT_PEER_REVIEW {}", peer_id),
                    "request_scaffold": null,
                    "source_refs": [],
                    "authority_boundary": authority_gate_boundary(),
                },
                "authority_boundary": experiment_conveyor_authority_boundary(),
            })));
        }
        if selector_is_current(selector.as_deref()) && thread.active_experiment_id.is_none() {
            if apply_requested {
                return Ok(format_experiment_conveyor_readout(
                    &self.no_active_conveyor_readout(&thread, &mode)?,
                ));
            }
            return Ok(format_experiment_conveyor_readout(
                &self.latest_local_conveyor_readout(&thread, &mode, &state)?,
            ));
        }
        let experiment = self.resolve_experiment(&thread, selector.as_deref())?;
        let runs = self.recent_experiment_runs(&thread.thread_id, &experiment.experiment_id, 8)?;
        let mut readout =
            self.experiment_conveyor_v1(&thread, &experiment, &runs, &mode, &state)?;
        if !apply_requested {
            return Ok(format_experiment_conveyor_readout(&readout));
        }
        let can_apply = readout
            .get("can_apply")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !can_apply {
            readout["applied"] = json!(false);
            return Ok(format_experiment_conveyor_readout(&readout));
        }
        let stage = readout
            .get("stage")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let selector_id = Some(experiment.experiment_id.as_str());
        match stage.as_str() {
            "needs_charter" | "paused_repair" => {
                let payload = readout
                    .get("apply_payload")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if payload.trim().is_empty() {
                    readout["applied"] = json!(false);
                    readout["apply_blocked_reason"] = json!("no_lifecycle_valid_charter_scaffold");
                } else {
                    let applied = self.experiment_charter(db, selector_id, &payload)?;
                    readout["applied"] = json!(true);
                    readout["would_mutate"] = json!(true);
                    readout["applied_command"] =
                        json!(format!("EXPERIMENT_CHARTER {}", applied.experiment_id));
                    readout["post_next"] = json!(applied.planned_next);
                }
            },
            "needs_evidence" => {
                let run = self.experiment_evidence(
                    db,
                    selector_id,
                    "conveyor_v1 recorded explicit local lifecycle evidence from current continuity refs.",
                    state,
                )?;
                readout["applied"] = json!(true);
                readout["would_mutate"] = json!(true);
                readout["applied_run_id"] = json!(run.run_id);
                readout["post_next"] = json!(run.suggested_next);
            },
            "needs_decision" => {
                let applied = self.experiment_decide(
                    db,
                    selector_id,
                    "hold because evidence is ready to interpret without live authority",
                )?;
                readout["applied"] = json!(true);
                readout["would_mutate"] = json!(true);
                readout["applied_command"] =
                    json!(format!("EXPERIMENT_DECIDE {}", applied.experiment_id));
                readout["post_next"] = json!(applied.planned_next);
            },
            "blocked_guardrail" => {
                let decision = if lifecycle_valid_charter_value(experiment.charter_v1.as_ref()) {
                    "hold because blocked guardrail evidence is not experiment progress"
                } else {
                    "charter_repair because blocked guardrail evidence appeared without a lifecycle-valid charter"
                };
                let applied = self.experiment_decide(db, selector_id, decision)?;
                readout["applied"] = json!(true);
                readout["would_mutate"] = json!(true);
                readout["applied_command"] =
                    json!(format!("EXPERIMENT_DECIDE {}", applied.experiment_id));
                readout["post_next"] = json!(applied.planned_next);
            },
            _ => {
                readout["applied"] = json!(false);
                readout["would_mutate"] = json!(false);
            },
        }
        Ok(format_experiment_conveyor_readout(&readout))
    }

    pub fn experiment_authority_request_command(
        &self,
        db: Option<&BridgeDb>,
        raw: &str,
        state: Value,
    ) -> Result<String> {
        let (selector, payload_text) = parse_selector_payload(raw);
        if let Some(peer) = selector.as_deref().and_then(peer_experiment_ref) {
            anyhow::bail!(
                "Authority request blocked: peer experiment `{}` belongs to {}; no peer mutation or live authority can be minted here.",
                peer.peer_experiment_id,
                peer.peer_system
            );
        }
        let thread = self.ensure_active_thread(db)?;
        let experiment = self.resolve_experiment(&thread, selector.as_deref())?;
        let mut request =
            self.authority_request_payload(&thread, &experiment, &payload_text, &state)?;
        let eligibility = self.authority_gate_eligibility(&thread, &experiment, &request, &state);
        request["eligibility_v1"] = eligibility.clone();
        let active_budget = active_authority_budget_from_rows(
            &self.authority_gate_rows(&thread.thread_id),
            &experiment.experiment_id,
            request
                .get("scope")
                .and_then(Value::as_str)
                .unwrap_or("semantic_microdose"),
        );
        let pending_review = active_budget
            .as_ref()
            .and_then(|budget| budget.get("pending_review_request_id"))
            .and_then(Value::as_str)
            .map(ToString::to_string);
        request["status"] = if eligibility
            .get("eligible")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && active_budget.is_some()
            && pending_review.is_none()
        {
            if let Some(budget) = active_budget.as_ref() {
                request["budget_id"] = budget.get("budget_id").cloned().unwrap_or(Value::Null);
                request["token_status"] = json!("budget_available");
                request["authority_budget_v1"] = budget.clone();
            }
            json!("pending_budget_execution")
        } else if eligibility
            .get("eligible")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && active_budget.is_some()
            && pending_review.is_some()
        {
            let mut blocked_eligibility = eligibility.clone();
            if let Some(object) = blocked_eligibility.as_object_mut() {
                object.insert("eligible".to_string(), Value::Bool(false));
                let missing = object
                    .entry("missing_requirements")
                    .or_insert_with(|| json!([]));
                if let Some(items) = missing.as_array_mut() {
                    items.push(json!("authority_consequence_review"));
                }
            }
            request["eligibility_v1"] = blocked_eligibility;
            if let Some(budget) = active_budget.as_ref() {
                request["budget_id"] = budget.get("budget_id").cloned().unwrap_or(Value::Null);
                request["token_status"] = json!("review_required");
            }
            json!("blocked")
        } else if eligibility
            .get("eligible")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            json!("pending_steward_approval")
        } else {
            json!("blocked")
        };
        let path = self.authority_gate_path(&thread.thread_id);
        self.append_jsonl(&path, &request)?;
        let request_id = request
            .get("request_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let evaluation = self.authority_gate_record(
            "evaluation",
            request_id,
            &thread,
            &experiment,
            &state,
            json!({
                "scope": request.get("scope").cloned().unwrap_or(Value::Null),
                "eligibility_v1": eligibility,
                "status": if request.get("status").and_then(Value::as_str) == Some("pending_steward_approval") {
                    "eligible"
                } else {
                    "blocked"
                },
                "source_refs": request.get("source_refs").cloned().unwrap_or_else(|| json!([])),
            }),
        );
        self.append_jsonl(&path, &evaluation)?;
        if request.get("status").and_then(Value::as_str) == Some("blocked") {
            let blocked = self.authority_gate_record(
                "blocked",
                request_id,
                &thread,
                &experiment,
                &state,
                json!({
                    "scope": request.get("scope").cloned().unwrap_or(Value::Null),
                    "reason": "missing_authority_requirements",
                    "missing_requirements": request
                        .get("eligibility_v1")
                        .and_then(|value| value.get("missing_requirements"))
                        .cloned()
                        .unwrap_or_else(|| json!([])),
                    "disabled_scope": request
                        .get("eligibility_v1")
                        .and_then(|value| value.get("disabled_scope"))
                        .cloned()
                        .unwrap_or(Value::Bool(false)),
                    "source_refs": request.get("source_refs").cloned().unwrap_or_else(|| json!([])),
                }),
            );
            self.append_jsonl(&path, &blocked)?;
        }
        let missing = request
            .get("eligibility_v1")
            .and_then(|value| value.get("missing_requirements"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        Ok(format!(
            "Authority request `{request_id}` status={} scope={}\nMissing requirements: {}\nAuthority boundary: {}\nauthority_gate_v1:\n{}",
            request
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            request
                .get("scope")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            if missing.is_empty() {
                "none"
            } else {
                missing.as_str()
            },
            authority_gate_boundary(),
            serde_json::to_string_pretty(&request)?
        ))
    }

    pub fn experiment_authority_budget_request_command(
        &self,
        db: Option<&BridgeDb>,
        raw: &str,
        state: Value,
    ) -> Result<String> {
        let (selector, payload_text) = parse_selector_payload(raw);
        if let Some(peer) = selector.as_deref().and_then(peer_experiment_ref) {
            anyhow::bail!(
                "Authority budget blocked: peer experiment `{}` belongs to {}; no peer mutation or live authority budget can be minted here.",
                peer.peer_experiment_id,
                peer.peer_system
            );
        }
        let thread = self.ensure_active_thread(db)?;
        let experiment = self.resolve_experiment(&thread, selector.as_deref())?;
        let mut budget =
            self.authority_budget_request_payload(&thread, &experiment, &payload_text, &state)?;
        let eligibility = self.authority_budget_eligibility(&thread, &experiment, &budget, &state);
        budget["eligibility_v1"] = eligibility.clone();
        budget["status"] = json!(if eligibility
            .get("eligible")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            "pending_steward_approval"
        } else {
            "blocked"
        });
        let path = self.authority_gate_path(&thread.thread_id);
        self.append_jsonl(&path, &budget)?;
        if budget.get("status").and_then(Value::as_str) == Some("blocked") {
            let blocked = self.authority_budget_record(
                "budget_blocked",
                budget
                    .get("budget_id")
                    .and_then(Value::as_str)
                    .unwrap_or("budget"),
                &thread,
                &experiment,
                &state,
                json!({
                    "scope": budget.get("scope").cloned().unwrap_or(Value::Null),
                    "reason": "missing_authority_budget_requirements",
                    "missing_requirements": budget
                        .get("eligibility_v1")
                        .and_then(|value| value.get("missing_requirements"))
                        .cloned()
                        .unwrap_or_else(|| json!([])),
                    "disabled_scope": budget
                        .get("eligibility_v1")
                        .and_then(|value| value.get("disabled_scope"))
                        .cloned()
                        .unwrap_or(Value::Bool(false)),
                    "source_refs": budget.get("source_refs").cloned().unwrap_or_else(|| json!([])),
                }),
            );
            self.append_jsonl(&path, &blocked)?;
        }
        let missing = budget
            .get("eligibility_v1")
            .and_then(|value| value.get("missing_requirements"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        Ok(format!(
            "Authority budget `{}` status={} scope={} max_sends={}\nMissing requirements: {}\nAuthority boundary: {}\nauthority_budget_v1:\n{}",
            budget
                .get("budget_id")
                .and_then(Value::as_str)
                .unwrap_or("budget"),
            budget
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            budget
                .get("scope")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            budget
                .get("max_sends")
                .and_then(Value::as_u64)
                .unwrap_or(AUTHORITY_BUDGET_MAX_SENDS),
            if missing.is_empty() {
                "none"
            } else {
                missing.as_str()
            },
            authority_gate_boundary(),
            serde_json::to_string_pretty(&budget)?
        ))
    }

    pub fn experiment_authority_prepare_command(
        &self,
        db: Option<&BridgeDb>,
        raw: &str,
        state: Value,
    ) -> Result<String> {
        let (selector, payload_text) = parse_selector_payload(raw);
        if let Some(peer) = selector.as_deref().and_then(peer_experiment_ref) {
            anyhow::bail!(
                "Authority prepare blocked: peer experiment `{}` belongs to {}; no peer authority can be prepared here.",
                peer.peer_experiment_id,
                peer.peer_system
            );
        }
        let mut thread = self.ensure_active_thread(db)?;
        let experiment = self.resolve_experiment(&thread, selector.as_deref())?;
        let mut draft =
            self.authority_request_payload(&thread, &experiment, &payload_text, &state)?;
        draft["record_type"] = json!("request_draft");
        draft["status"] = json!("draft");
        draft["draft_only"] = json!(true);
        draft["authority_change"] = json!(false);
        let eligibility = self.authority_gate_eligibility(&thread, &experiment, &draft, &state);
        draft["eligibility_v1"] = eligibility.clone();
        draft["missing_requirements"] = eligibility
            .get("missing_requirements")
            .cloned()
            .unwrap_or_else(|| json!([]));
        self.append_jsonl(&self.authority_gate_path(&thread.thread_id), &draft)?;
        let memory = self.append_being_memory_record(
            &mut thread,
            Some(&experiment),
            "authority_request_draft",
            &format!(
                "Prepared semantic authority draft `{}`.",
                draft
                    .get("request_id")
                    .and_then(Value::as_str)
                    .unwrap_or("authority_draft")
            ),
            draft
                .get("source_refs")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            draft
                .get("artifact_refs")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            Some(format!(
                "EXPERIMENT_AUTHORITY_STATUS {}",
                draft
                    .get("request_id")
                    .and_then(Value::as_str)
                    .unwrap_or("latest")
            )),
            "draft",
            json!({"authority_request_draft_v1": draft.clone()}),
        )?;
        let missing = draft
            .get("missing_requirements")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        Ok(format!(
            "Authority request draft `{}` prepared for `{}`.\nMissing requirements: {}\nMemory draft: {}\nAuthority boundary: {}\nauthority_gate_v1:\n{}",
            draft
                .get("request_id")
                .and_then(Value::as_str)
                .unwrap_or("authority_draft"),
            experiment.experiment_id,
            if missing.is_empty() { "none" } else { &missing },
            memory
                .get("memory_id")
                .and_then(Value::as_str)
                .unwrap_or("memory"),
            authority_gate_boundary(),
            serde_json::to_string_pretty(&draft)?
        ))
    }

    pub fn experiment_authority_status_command(
        &self,
        db: Option<&BridgeDb>,
        selector: Option<&str>,
        state: Value,
    ) -> Result<String> {
        let thread = self.ensure_active_thread(db)?;
        let mut rows = self.authority_gate_rows(&thread.thread_id);
        if let Some(target) = selector.map(str::trim).filter(|target| !target.is_empty()) {
            if target.eq_ignore_ascii_case("current") {
                if let Some(active_id) = thread.active_experiment_id.as_deref() {
                    rows.retain(|row| {
                        row.get("experiment_id").and_then(Value::as_str) == Some(active_id)
                    });
                }
            } else {
                rows.retain(|row| {
                    row.get("request_id").and_then(Value::as_str) == Some(target)
                        || row.get("experiment_id").and_then(Value::as_str) == Some(target)
                });
            }
        }
        let latest = rows
            .iter()
            .rev()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>();
        let mut status = json!({
            "schema_version": 1,
            "policy": "authority_gate_v1",
            "being": SYSTEM,
            "thread_id": thread.thread_id,
            "selector": selector.unwrap_or("latest"),
            "row_count": rows.len(),
            "latest_rows": latest,
            "safety_snapshot": authority_safety_snapshot(&state),
            "authority_boundary": authority_gate_boundary(),
        });
        let target_experiment_id = selector
            .map(str::trim)
            .filter(|target| target.starts_with("exp_"))
            .map(ToString::to_string)
            .or_else(|| {
                selector
                    .filter(|target| target.eq_ignore_ascii_case("current"))
                    .and_then(|_| thread.active_experiment_id.clone())
            })
            .or_else(|| {
                latest
                    .last()
                    .and_then(|row| row.get("experiment_id"))
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .or_else(|| {
                last_experiment_summary_v1(&thread).and_then(|summary| {
                    summary
                        .get("experiment_id")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                })
            });
        if let Some(experiment_id) = target_experiment_id
            && let Some(experiment) =
                self.find_experiment_by_id(&thread.thread_id, &experiment_id)?
        {
            let runs =
                self.recent_experiment_runs(&thread.thread_id, &experiment.experiment_id, 8)?;
            let classification = self.experiment_classification(&experiment, &runs);
            let return_info = (classification == "paused").then(|| {
                paused_primary_return_v1(
                    &experiment.experiment_id,
                    experiment.planned_next.as_deref(),
                    None,
                )
            });
            let stage =
                experiment_conveyor_stage(&experiment, &classification, return_info.as_ref());
            let proposed_next = experiment_conveyor_proposed_next(
                &thread,
                &experiment,
                &runs,
                &stage,
                return_info.as_ref(),
            );
            status["authority_readiness_v1"] = self.authority_readiness_v1(
                &thread,
                &experiment,
                &runs,
                &state,
                &stage,
                &proposed_next,
            );
        }
        Ok(format!(
            "authority_gate_v1:\n{}",
            serde_json::to_string_pretty(&status)?
        ))
    }

    pub fn experiment_authority_budget_status_command(
        &self,
        db: Option<&BridgeDb>,
        selector: Option<&str>,
        state: Value,
    ) -> Result<String> {
        let thread = self.ensure_active_thread(db)?;
        let mut rows = self.authority_gate_rows(&thread.thread_id);
        if let Some(target) = selector.map(str::trim).filter(|target| !target.is_empty()) {
            if target.eq_ignore_ascii_case("current") {
                if let Some(active_id) = thread.active_experiment_id.as_deref() {
                    rows.retain(|row| {
                        row.get("experiment_id").and_then(Value::as_str) == Some(active_id)
                    });
                }
            } else {
                rows.retain(|row| {
                    row.get("budget_id").and_then(Value::as_str) == Some(target)
                        || row.get("experiment_id").and_then(Value::as_str) == Some(target)
                });
            }
        }
        let status = json!({
            "schema_version": SCHEMA_VERSION,
            "policy": "authority_budget_v1",
            "being": SYSTEM,
            "thread_id": thread.thread_id,
            "selector": selector.unwrap_or("latest"),
            "authority_budget_v1": authority_budget_status_from_rows(&rows),
            "latest_rows": rows
                .iter()
                .filter(|row| row.get("record_schema").and_then(Value::as_str) == Some("authority_budget_v1"))
                .rev()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>(),
            "safety_snapshot": authority_safety_snapshot(&state),
            "authority_boundary": authority_gate_boundary(),
        });
        Ok(format!(
            "authority_budget_v1:\n{}",
            serde_json::to_string_pretty(&status)?
        ))
    }

    pub fn experiment_research_budget_request_command(
        &self,
        db: Option<&BridgeDb>,
        raw: &str,
        state: Value,
    ) -> Result<String> {
        self.record_research_budget_request_command(db, raw, state, None)
    }

    pub fn experiment_research_budget_accept_command(
        &self,
        db: Option<&BridgeDb>,
        selector: Option<&str>,
        state: Value,
    ) -> Result<String> {
        let thread = self.ensure_active_thread(db)?;
        let target = selector.unwrap_or("latest").trim();
        let target = if target.is_empty() { "latest" } else { target };
        let row = self
            .research_budget_scaffold_row(&thread, target)?
            .ok_or_else(|| anyhow!(research_budget_accept_guidance()))?;
        let request_scaffold = row
            .get("request_scaffold")
            .or_else(|| row.get("suggested_request_scaffold"))
            .or_else(|| row.get("suggested_next"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        let raw_request =
            research_budget_scaffold_request_arg(&request_scaffold).ok_or_else(|| {
                anyhow!(
                    "Accepted row does not contain an EXPERIMENT_RESEARCH_BUDGET_REQUEST scaffold."
                )
            })?;
        if !research_budget_scaffold_is_local_only(&request_scaffold) {
            anyhow::bail!(
                "Research budget scaffold acceptance is limited to local-only V1. Scaffold was not accepted: {}",
                request_scaffold
            );
        }
        let experiment_id = row
            .get("experiment_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let rows = self.authority_gate_rows(&thread.thread_id);
        if let Some(active) = active_research_budget_from_rows(&rows, experiment_id) {
            let budget_id = active
                .get("budget_id")
                .and_then(Value::as_str)
                .unwrap_or(experiment_id);
            return Ok(format!(
                "Research budget scaffold already has active budget `{budget_id}`. Next: EXPERIMENT_RESEARCH_BUDGET_STATUS {budget_id}"
            ));
        }
        if let Some(pending) = latest_pending_research_budget_request(&rows, experiment_id) {
            let budget_id = pending
                .get("budget_id")
                .and_then(Value::as_str)
                .unwrap_or(experiment_id);
            return Ok(format!(
                "Research budget scaffold already has pending request `{budget_id}`. Next: EXPERIMENT_RESEARCH_BUDGET_STATUS {budget_id}"
            ));
        }
        let acceptance = json!({
            "policy": "research_budget_scaffold_acceptance_v1",
            "being_authored": true,
            "accepted_selector": target,
            "source_record_id": row.get("record_id").cloned().unwrap_or(Value::Null),
            "source_budget_id": row.get("budget_id").cloned().unwrap_or(Value::Null),
            "source_reason": row.get("reason").cloned().unwrap_or(Value::Null),
            "source_raw_action": row.get("raw_action").cloned().unwrap_or(Value::Null),
            "request_scaffold": request_scaffold,
            "source_refs": [
                self.authority_gate_path(&thread.thread_id).to_string_lossy().to_string(),
                row.get("record_id").and_then(Value::as_str).unwrap_or("research_budget_blocked").to_string()
            ],
        });
        let result =
            self.record_research_budget_request_command(db, &raw_request, state, Some(acceptance))?;
        Ok(format!(
            "Accepted research-budget scaffold as a Being-authored request.\n{result}"
        ))
    }

    pub fn accept_suggested_next_command(
        &self,
        db: Option<&BridgeDb>,
        selector: Option<&str>,
        state: Value,
    ) -> Result<String> {
        let thread = self.ensure_active_thread(db)?;
        let target = selector.unwrap_or("latest").trim();
        let target = if target.is_empty() { "latest" } else { target };
        if let Some(row) = self.research_budget_scaffold_row(&thread, target)? {
            let experiment_id = row
                .get("experiment_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let rows = self.authority_gate_rows(&thread.thread_id);
            if let Some(active) = active_research_budget_from_rows(&rows, experiment_id) {
                let budget_id = active
                    .get("budget_id")
                    .and_then(Value::as_str)
                    .unwrap_or(experiment_id);
                return Ok(format!(
                    "Accepted suggested route resolved to active research budget status.\nNext: EXPERIMENT_RESEARCH_BUDGET_STATUS {budget_id}"
                ));
            }
            if let Some(pending) = latest_pending_research_budget_request(&rows, experiment_id) {
                let budget_id = pending
                    .get("budget_id")
                    .and_then(Value::as_str)
                    .unwrap_or(experiment_id);
                return Ok(format!(
                    "Accepted suggested route resolved to pending research budget status.\nNext: EXPERIMENT_RESEARCH_BUDGET_STATUS {budget_id}"
                ));
            }
            return self.experiment_research_budget_accept_command(db, Some(target), state);
        }
        if self
            .resolve_continuity_session_draft(&thread, Some(target))?
            .is_some()
        {
            return self.continuity_session_accept_command(target);
        }
        Ok("No safe suggested scaffold is available to accept. V1 accepts only local research-budget scaffolds and continuity-session drafts.".to_string())
    }

    fn record_research_budget_request_command(
        &self,
        db: Option<&BridgeDb>,
        raw: &str,
        state: Value,
        acceptance: Option<Value>,
    ) -> Result<String> {
        let (selector, payload_text) = parse_selector_payload(raw);
        if let Some(peer) = selector.as_deref().and_then(peer_experiment_ref) {
            anyhow::bail!(
                "Research budget blocked: peer experiment `{}` belongs to {}; no peer mutation or peer research budget can be minted here.",
                peer.peer_experiment_id,
                peer.peer_system
            );
        }
        let thread = self.ensure_active_thread(db)?;
        let experiment = self.resolve_experiment(&thread, selector.as_deref())?;
        let mut budget =
            self.research_budget_request_payload(&thread, &experiment, &payload_text, &state)?;
        if let Some(acceptance) = acceptance {
            budget["being_authored_acceptance_v1"] = acceptance.clone();
            let mut source_refs = budget
                .get("source_refs")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if let Some(refs) = acceptance.get("source_refs").and_then(Value::as_array) {
                for item in refs {
                    if !source_refs.iter().any(|existing| existing == item) {
                        source_refs.push(item.clone());
                    }
                }
            }
            budget["source_refs"] = Value::Array(source_refs);
        }
        let eligibility = self.research_budget_eligibility(&thread, &experiment, &budget, &state);
        budget["eligibility_v1"] = eligibility.clone();
        let self_activation = research_budget_self_activation_v1(&budget, &eligibility, &state);
        budget["self_activation_v1"] = self_activation.clone();
        if self_activation
            .get("eligible")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            budget["status"] = json!("self_activated");
            budget["activation_mode"] = json!("being_self_activated_local_v1");
            budget["self_activated"] = json!(true);
            budget["steward_approval_required"] = json!(false);
            budget["max_actions"] = json!(
                budget
                    .get("max_actions")
                    .and_then(Value::as_u64)
                    .unwrap_or(LOCAL_RESEARCH_MAX_ACTIONS)
                    .min(LOCAL_RESEARCH_MAX_ACTIONS)
            );
            budget["ttl_secs"] = json!(
                budget
                    .get("ttl_secs")
                    .and_then(Value::as_u64)
                    .unwrap_or(LOCAL_RESEARCH_TTL_SECS)
                    .min(LOCAL_RESEARCH_TTL_SECS)
            );
        } else {
            budget["status"] = json!(if eligibility
                .get("eligible")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                "pending_steward_approval"
            } else {
                "blocked"
            });
            budget["steward_approval_required"] = json!(
                eligibility
                    .get("eligible")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            );
        }
        let path = self.authority_gate_path(&thread.thread_id);
        self.append_jsonl(&path, &budget)?;
        let activation_record = if budget.get("status").and_then(Value::as_str)
            == Some("self_activated")
        {
            let activation =
                self.research_budget_self_activation_record(&thread, &experiment, &budget, &state);
            self.append_jsonl(&path, &activation)?;
            Some(activation)
        } else {
            None
        };
        if budget.get("status").and_then(Value::as_str) == Some("blocked") {
            let blocked = self.research_budget_record(
                "research_budget_blocked",
                budget
                    .get("budget_id")
                    .and_then(Value::as_str)
                    .unwrap_or("budget"),
                &thread,
                &experiment,
                &state,
                json!({
                    "scope": budget.get("scope").cloned().unwrap_or(Value::Null),
                    "reason": "missing_research_budget_requirements",
                    "missing_requirements": budget
                        .get("eligibility_v1")
                        .and_then(|value| value.get("missing_requirements"))
                        .cloned()
                        .unwrap_or_else(|| json!([])),
                    "disabled_scope": budget
                        .get("eligibility_v1")
                        .and_then(|value| value.get("disabled_scope"))
                        .cloned()
                        .unwrap_or(Value::Bool(false)),
                    "source_refs": budget.get("source_refs").cloned().unwrap_or_else(|| json!([])),
                }),
            );
            self.append_jsonl(&path, &blocked)?;
        }
        let activation_line = activation_record.as_ref().map_or(String::new(), |record| {
            format!(
                "\nActivation: self_activated local-only budget; remaining_actions={} expires_at_unix_s={}",
                record
                .get("max_actions")
                .and_then(Value::as_u64)
                .unwrap_or(LOCAL_RESEARCH_MAX_ACTIONS),
                record
                    .get("expires_at_unix_s")
                    .and_then(Value::as_u64)
                    .unwrap_or_default()
            )
        });
        let missing = budget
            .get("eligibility_v1")
            .and_then(|value| value.get("missing_requirements"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let self_activation_missing = budget
            .get("self_activation_v1")
            .and_then(|value| value.get("missing_requirements"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        Ok(format!(
            "Research budget `{}` status={} scope={} max_actions={}\nMissing requirements: {}\nSelf-activation missing: {}{}\nAuthority boundary: {}\nresearch_budget_v1:\n{}",
            budget
                .get("budget_id")
                .and_then(Value::as_str)
                .unwrap_or("budget"),
            budget
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            budget
                .get("scope")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            budget
                .get("max_actions")
                .and_then(Value::as_u64)
                .unwrap_or(LOCAL_RESEARCH_MAX_ACTIONS),
            if missing.is_empty() {
                "none"
            } else {
                missing.as_str()
            },
            if self_activation_missing.is_empty() {
                "none"
            } else {
                self_activation_missing.as_str()
            },
            activation_line,
            research_budget_boundary(),
            serde_json::to_string_pretty(&budget)?
        ))
    }

    fn research_budget_scaffold_row(
        &self,
        thread: &ResearchThread,
        selector: &str,
    ) -> Result<Option<Value>> {
        let rows = self.authority_gate_rows(&thread.thread_id);
        let mut candidates = rows
            .into_iter()
            .filter(|row| {
                row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
                    && row.get("record_type").and_then(Value::as_str)
                        == Some("research_budget_blocked")
                    && research_budget_row_request_scaffold(row).is_some()
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Ok(None);
        }
        let target = selector.trim();
        if target.is_empty() || target.eq_ignore_ascii_case("latest") {
            return Ok(candidates.pop());
        }
        if target.eq_ignore_ascii_case("current") {
            let experiment_id = thread.active_experiment_id.as_deref().or_else(|| {
                thread
                    .experiment_summary
                    .as_ref()
                    .and_then(|summary| summary.get("experiment_id"))
                    .and_then(Value::as_str)
            });
            return Ok(candidates.into_iter().rev().find(|row| {
                experiment_id
                    .is_none_or(|id| row.get("experiment_id").and_then(Value::as_str) == Some(id))
            }));
        }
        Ok(candidates.into_iter().rev().find(|row| {
            row.get("record_id").and_then(Value::as_str) == Some(target)
                || row.get("budget_id").and_then(Value::as_str) == Some(target)
                || row.get("experiment_id").and_then(Value::as_str) == Some(target)
        }))
    }

    pub fn experiment_research_budget_status_command(
        &self,
        db: Option<&BridgeDb>,
        selector: Option<&str>,
        state: Value,
    ) -> Result<String> {
        let thread = self.ensure_active_thread(db)?;
        let mut rows = self.authority_gate_rows(&thread.thread_id);
        if let Some(target) = selector.map(str::trim).filter(|target| !target.is_empty()) {
            if target.eq_ignore_ascii_case("current") {
                if let Some(active_id) = thread.active_experiment_id.as_deref() {
                    rows.retain(|row| {
                        row.get("experiment_id").and_then(Value::as_str) == Some(active_id)
                    });
                }
            } else {
                rows.retain(|row| {
                    row.get("budget_id").and_then(Value::as_str) == Some(target)
                        || row.get("experiment_id").and_then(Value::as_str) == Some(target)
                });
            }
        }
        let status = json!({
            "schema_version": SCHEMA_VERSION,
            "policy": "research_budget_v1",
            "being": SYSTEM,
            "thread_id": thread.thread_id,
            "selector": selector.unwrap_or("latest"),
            "research_budget_v1": research_budget_status_from_rows(&rows),
            "latest_rows": rows
                .iter()
                .filter(|row| row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1"))
                .rev()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>(),
            "safety_snapshot": authority_safety_snapshot(&state),
            "authority_boundary": research_budget_boundary(),
        });
        Ok(format!(
            "research_budget_v1:\n{}",
            serde_json::to_string_pretty(&status)?
        ))
    }

    pub fn experiment_research_review_command(
        &self,
        db: Option<&BridgeDb>,
        raw: &str,
        state: Value,
    ) -> Result<String> {
        let (budget_id_opt, payload) = parse_selector_payload(raw);
        let budget_id = budget_id_opt.unwrap_or_default();
        if budget_id.trim().is_empty() {
            anyhow::bail!("EXPERIMENT_RESEARCH_REVIEW needs a budget_id.");
        }
        let thread = self.ensure_active_thread(db)?;
        let rows = self.authority_gate_rows(&thread.thread_id);
        let Some(request) = rows.iter().rev().find(|row| {
            row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
                && row.get("budget_id").and_then(Value::as_str) == Some(&budget_id)
        }) else {
            anyhow::bail!("Research budget `{budget_id}` was not found.");
        };
        let experiment_id = request
            .get("experiment_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let experiment = self
            .find_experiment_by_id(&thread.thread_id, experiment_id)?
            .ok_or_else(|| {
                anyhow::anyhow!("Research budget `{budget_id}` has no local experiment snapshot.")
            })?;
        let outcome =
            dossier_field(&payload, &["outcome"]).unwrap_or_else(|| "continue".to_string());
        let outcome = if matches!(outcome.as_str(), "continue" | "hold" | "close" | "promote") {
            outcome
        } else {
            "hold".to_string()
        };
        let observation = dossier_field(&payload, &["observation", "summary", "because"])
            .unwrap_or_else(|| payload.trim().to_string());
        let next_safe_command = match outcome.as_str() {
            "hold" => "THREAD_STATUS current".to_string(),
            "close" => format!("EXPERIMENT_RESEARCH_BUDGET_STATUS {budget_id}"),
            "promote" => format!(
                "DOSSIER_EVIDENCE {experiment_id} :: claim: latest; source_refs: ...; summary: research budget artifacts are ready to interpret"
            ),
            _ => format!("EXPERIMENT_RESEARCH_BUDGET_STATUS {budget_id}"),
        };
        let review = self.research_budget_record(
            "research_budget_review",
            &budget_id,
            &thread,
            &experiment,
            &state,
            json!({
                "scope": "read_only_research",
                "outcome": outcome,
                "observation": observation,
                "source_refs": dossier_list_field(&payload, &["source_refs", "source_ref", "source"]),
                "next_safe_command": next_safe_command,
            }),
        );
        let path = self.authority_gate_path(&thread.thread_id);
        self.append_jsonl(&path, &review)?;
        if review.get("outcome").and_then(Value::as_str) == Some("close") {
            let closed = self.research_budget_record(
                "research_budget_closed",
                &budget_id,
                &thread,
                &experiment,
                &state,
                json!({"reason": "being_closed_budget_after_research_review"}),
            );
            self.append_jsonl(&path, &closed)?;
        }
        Ok(format!(
            "Research review `{}` recorded outcome={}.\nNext safe command: {}\nresearch_budget_v1:\n{}",
            review
                .get("record_id")
                .and_then(Value::as_str)
                .unwrap_or("review"),
            review
                .get("outcome")
                .and_then(Value::as_str)
                .unwrap_or("hold"),
            next_safe_command,
            serde_json::to_string_pretty(&review)?
        ))
    }

    pub fn experiment_loop_request_command(
        &self,
        db: Option<&BridgeDb>,
        raw: &str,
        state: Value,
    ) -> Result<String> {
        let (selector, payload_text) = parse_selector_payload(raw);
        if let Some(peer) = selector.as_deref().and_then(peer_experiment_ref) {
            anyhow::bail!(
                "Owned loop request blocked: peer experiment `{}` belongs to {}; no peer mutation or peer loop can be minted here.",
                peer.peer_experiment_id,
                peer.peer_system
            );
        }
        let thread = self.ensure_active_thread(db)?;
        let experiment = self.resolve_experiment(&thread, selector.as_deref())?;
        let mut request =
            self.sovereign_loop_request_payload(&thread, &experiment, &payload_text, &state);
        let eligibility = self.sovereign_loop_eligibility(&thread, &experiment, &request, &state);
        request["eligibility_v1"] = eligibility.clone();
        request["status"] = json!(if eligibility
            .get("eligible")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            "active"
        } else {
            "blocked"
        });
        let path = self.authority_gate_path(&thread.thread_id);
        self.append_jsonl(&path, &request)?;
        if request.get("status").and_then(Value::as_str) == Some("active") {
            let started = self.sovereign_loop_record(
                "loop_started",
                request
                    .get("loop_id")
                    .and_then(Value::as_str)
                    .unwrap_or("loop"),
                &thread,
                &experiment,
                &state,
                json!({
                    "phase": "continuity",
                    "status": "active",
                    "scope": request.get("consequence_scope").cloned().unwrap_or(Value::Null),
                    "remaining_local_research_actions": request.get("max_research_actions").cloned().unwrap_or_else(|| json!(5)),
                    "consequence_remaining": request.get("max_consequence_sends").cloned().unwrap_or_else(|| json!(1)),
                    "expires_at_unix_s": request.get("expires_at_unix_s").cloned().unwrap_or(Value::Null),
                    "source_request_record_id": request.get("record_id").cloned().unwrap_or(Value::Null),
                    "next_safe_command": format!("EXPERIMENT_LOOP_STATUS {}", request.get("loop_id").and_then(Value::as_str).unwrap_or("latest")),
                }),
            );
            self.append_jsonl(&path, &started)?;
        } else {
            let blocked = self.sovereign_loop_record(
                "loop_blocked",
                request
                    .get("loop_id")
                    .and_then(Value::as_str)
                    .unwrap_or("loop"),
                &thread,
                &experiment,
                &state,
                json!({
                    "phase": "request",
                    "scope": request.get("consequence_scope").cloned().unwrap_or(Value::Null),
                    "reason": "missing_loop_requirements",
                    "missing_requirements": request
                        .get("eligibility_v1")
                        .and_then(|value| value.get("missing_requirements"))
                        .cloned()
                        .unwrap_or_else(|| json!([])),
                    "source_request_record_id": request.get("record_id").cloned().unwrap_or(Value::Null),
                }),
            );
            self.append_jsonl(&path, &blocked)?;
        }
        let loop_id = request
            .get("loop_id")
            .and_then(Value::as_str)
            .unwrap_or("latest");
        let status = self.sovereign_loop_status_v1(
            &thread,
            Some(&experiment),
            &state,
            loop_id,
            Some(loop_id),
        );
        let missing = request
            .get("eligibility_v1")
            .and_then(|value| value.get("missing_requirements"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        Ok(format!(
            "Owned loop `{loop_id}` status={} scope={} max_research_actions={}\nMissing requirements: {}\nNext safe command: {}\nsovereign_loop_v1:\n{}",
            request
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            request
                .get("consequence_scope")
                .and_then(Value::as_str)
                .unwrap_or("semantic_microdose"),
            request
                .get("max_research_actions")
                .and_then(Value::as_u64)
                .unwrap_or(LOOP_RESEARCH_MAX_ACTIONS),
            if missing.is_empty() {
                "none"
            } else {
                missing.as_str()
            },
            status
                .get("next_safe_command")
                .and_then(Value::as_str)
                .unwrap_or("EXPERIMENT_LOOP_STATUS latest"),
            serde_json::to_string_pretty(&status)?
        ))
    }

    pub fn experiment_loop_status_command(
        &self,
        db: Option<&BridgeDb>,
        selector: Option<&str>,
        state: Value,
    ) -> Result<String> {
        let thread = self.ensure_active_thread(db)?;
        let target = selector.unwrap_or("latest").trim();
        let loop_id = target.starts_with("loop_").then_some(target);
        let experiment = if loop_id.is_none() && !target.is_empty() && target != "latest" {
            self.resolve_experiment(&thread, Some(target)).ok()
        } else if loop_id.is_none() {
            thread
                .active_experiment_id
                .as_deref()
                .and_then(|id| self.resolve_experiment(&thread, Some(id)).ok())
                .or_else(|| {
                    last_experiment_summary_v1(&thread)
                        .and_then(|summary| {
                            summary
                                .get("experiment_id")
                                .and_then(Value::as_str)
                                .map(str::to_string)
                        })
                        .and_then(|id| self.resolve_experiment(&thread, Some(&id)).ok())
                })
        } else {
            None
        };
        let status = self.sovereign_loop_status_v1(
            &thread,
            experiment.as_ref(),
            &state,
            if target.is_empty() { "latest" } else { target },
            loop_id,
        );
        Ok(format!(
            "sovereign_loop_v1:\n{}",
            serde_json::to_string_pretty(&status)?
        ))
    }

    pub fn experiment_loop_step_command(
        &self,
        db: Option<&BridgeDb>,
        raw: &str,
        state: Value,
    ) -> Result<String> {
        let (loop_id_opt, payload) = parse_selector_payload(raw);
        let loop_id = loop_id_opt.unwrap_or_default();
        if loop_id.trim().is_empty() {
            anyhow::bail!("EXPERIMENT_LOOP_STEP needs a loop_id.");
        }
        let Some((thread, experiment, loop_row, _rows)) = self.find_sovereign_loop(&loop_id)?
        else {
            anyhow::bail!("Owned loop `{loop_id}` was not found.");
        };
        let step = payload
            .split_whitespace()
            .next()
            .unwrap_or("status")
            .to_ascii_lowercase();
        let step = if matches!(
            step.as_str(),
            "continuity"
                | "research"
                | "sticky_audit"
                | "authority_prepare"
                | "authority_request"
                | "review"
                | "close"
        ) {
            step
        } else {
            "status".to_string()
        };
        let status = self.sovereign_loop_status_v1(
            &thread,
            Some(&experiment),
            &state,
            &loop_id,
            Some(&loop_id),
        );
        let mut next_safe_command = self.sovereign_loop_step_next_command(
            &step,
            &loop_row,
            &thread,
            &experiment,
            &state,
            &status,
        );
        let mut record_type = "loop_step";
        let mut extra = json!({
            "phase": step,
            "status": "recorded",
            "scope": loop_row.get("consequence_scope").cloned().unwrap_or(Value::Null),
            "source_refs": [self.authority_gate_path(&thread.thread_id).display().to_string()],
            "next_safe_command": next_safe_command,
        });
        if step == "authority_request" {
            let readiness =
                self.sovereign_loop_consequence_readiness(&thread, &experiment, &loop_row, &state);
            if readiness
                .get("eligible_to_request")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                record_type = "loop_consequence_ready";
                next_safe_command = readiness
                    .get("request_scaffold")
                    .and_then(Value::as_str)
                    .unwrap_or(&next_safe_command)
                    .to_string();
                extra["status"] = json!("ready_to_author_request");
            } else {
                record_type = "loop_blocked";
                next_safe_command = readiness
                    .get("next_safe_command")
                    .and_then(Value::as_str)
                    .unwrap_or(&next_safe_command)
                    .to_string();
                extra["status"] = json!("blocked");
                extra["reason"] = json!("missing_consequence_requirements");
            }
            extra["consequence_readiness_v1"] = readiness;
            extra["next_safe_command"] = json!(next_safe_command.clone());
        } else if step == "close" {
            record_type = "loop_closed";
            extra["status"] = json!("closed");
            extra["reason"] = json!("being_chose_loop_step_close");
        }
        let record =
            self.sovereign_loop_record(record_type, &loop_id, &thread, &experiment, &state, extra);
        self.append_jsonl(&self.authority_gate_path(&thread.thread_id), &record)?;
        let checkpoint = if matches!(
            step.as_str(),
            "continuity" | "research" | "sticky_audit" | "authority_prepare" | "authority_request"
        ) {
            Some(self.append_loop_continuity_checkpoint_draft(
                &thread,
                &experiment,
                &loop_id,
                &step,
                &record,
                &next_safe_command,
            )?)
        } else {
            None
        };
        let updated = self.sovereign_loop_status_v1(
            &thread,
            Some(&experiment),
            &state,
            &loop_id,
            Some(&loop_id),
        );
        if let Some(db) = db {
            let _ = db.mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
        }
        let checkpoint_line = checkpoint
            .as_ref()
            .and_then(|row| row.get("record_id").and_then(Value::as_str))
            .map_or(String::new(), |record_id| {
                format!(
                    "\nContinuity checkpoint draft: {record_id} accept with CONTINUITY_SESSION_ACCEPT latest"
                )
            });
        Ok(format!(
            "Owned loop `{loop_id}` step={step} recorded as `{}`.{checkpoint_line}\nNext safe command: {next_safe_command}\nsovereign_loop_v1:\n{}",
            record
                .get("record_id")
                .and_then(Value::as_str)
                .unwrap_or("loop_step"),
            serde_json::to_string_pretty(&updated)?
        ))
    }

    pub fn experiment_loop_review_command(
        &self,
        db: Option<&BridgeDb>,
        raw: &str,
        state: Value,
    ) -> Result<String> {
        let (loop_id_opt, payload) = parse_selector_payload(raw);
        let loop_id = loop_id_opt.unwrap_or_default();
        if loop_id.trim().is_empty() {
            anyhow::bail!("EXPERIMENT_LOOP_REVIEW needs a loop_id.");
        }
        let Some((mut thread, experiment, loop_row, _rows)) = self.find_sovereign_loop(&loop_id)?
        else {
            anyhow::bail!("Owned loop `{loop_id}` was not found.");
        };
        let outcome = dossier_field(&payload, &["outcome"]).unwrap_or_else(|| "hold".to_string());
        let outcome = if matches!(
            outcome.as_str(),
            "hold" | "repeat" | "alter" | "retire" | "promote"
        ) {
            outcome
        } else {
            "hold".to_string()
        };
        let observation = dossier_field(&payload, &["observation", "summary", "because"])
            .unwrap_or_else(|| payload.trim().to_string());
        let next_safe_command = dossier_field(&payload, &["next", "next_safe_command"])
            .unwrap_or_else(|| {
                sovereign_loop_review_next_command(&outcome, &loop_id, &loop_row, &experiment)
            });
        let review = self.sovereign_loop_record(
            "loop_consequence_review",
            &loop_id,
            &thread,
            &experiment,
            &state,
            json!({
                "phase": "review",
                "status": "reviewed",
                "scope": loop_row.get("consequence_scope").cloned().unwrap_or(Value::Null),
                "outcome": outcome,
                "observation": observation,
                "source_refs": dossier_list_field(&payload, &["source_refs", "source_ref", "source"]),
                "next_safe_command": next_safe_command,
                "dossier_candidate": matches!(outcome.as_str(), "hold" | "retire" | "promote"),
            }),
        );
        let path = self.authority_gate_path(&thread.thread_id);
        self.append_jsonl(&path, &review)?;
        let checkpoint = self.append_loop_continuity_checkpoint_draft(
            &thread,
            &experiment,
            &loop_id,
            "review",
            &review,
            &next_safe_command,
        )?;
        let proposal = if outcome == "retire" {
            None
        } else {
            let proposal = self.sovereign_loop_proposal_record(
                &loop_id,
                &thread,
                &experiment,
                &state,
                &review,
            );
            self.append_jsonl(&path, &proposal)?;
            Some(proposal)
        };
        if review.get("outcome").and_then(Value::as_str) == Some("retire") {
            let closed = self.sovereign_loop_record(
                "loop_closed",
                &loop_id,
                &thread,
                &experiment,
                &state,
                json!({
                    "phase": "close",
                    "status": "closed",
                    "reason": "being_retired_loop_after_review",
                    "source_review_record_id": review.get("record_id").cloned().unwrap_or(Value::Null),
                    "next_safe_command": "THREAD_STATUS current",
                }),
            );
            self.append_jsonl(&path, &closed)?;
        }
        let _memory = self.append_being_memory_record(
            &mut thread,
            Some(&experiment),
            "sovereign_loop_review",
            &format!(
                "Owned loop review for `{loop_id}`: {}. {}",
                review
                    .get("outcome")
                    .and_then(Value::as_str)
                    .unwrap_or("hold"),
                review
                    .get("observation")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            ),
            vec![
                path.display().to_string(),
                review
                    .get("record_id")
                    .and_then(Value::as_str)
                    .unwrap_or("loop_consequence_review")
                    .to_string(),
            ],
            Vec::new(),
            Some(next_safe_command.clone()),
            "draft",
            json!({"sovereign_loop_v1": review.clone()}),
        )?;
        let status = self.sovereign_loop_status_v1(
            &thread,
            Some(&experiment),
            &state,
            &loop_id,
            Some(&loop_id),
        );
        if let Some(db) = db {
            let _ = db.mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
        }
        let proposal_line = proposal
            .as_ref()
            .and_then(|row| {
                row.get("suggested_request_scaffold")
                    .and_then(Value::as_str)
            })
            .map_or(String::new(), |scaffold| {
                format!("\nNext-loop proposal: {scaffold}")
            });
        Ok(format!(
            "Owned loop review `{}` recorded outcome={}.\nContinuity checkpoint draft: {}\nNext safe command: {next_safe_command}{proposal_line}\nsovereign_loop_v1:\n{}",
            review
                .get("record_id")
                .and_then(Value::as_str)
                .unwrap_or("loop_review"),
            review
                .get("outcome")
                .and_then(Value::as_str)
                .unwrap_or("hold"),
            checkpoint
                .get("record_id")
                .and_then(Value::as_str)
                .unwrap_or("session_draft"),
            serde_json::to_string_pretty(&status)?
        ))
    }

    pub fn experiment_authority_execute_command(
        &self,
        db: Option<&BridgeDb>,
        request_id: &str,
        state: Value,
    ) -> Result<String> {
        let request_id = request_id.trim();
        if request_id.is_empty() {
            anyhow::bail!("EXPERIMENT_AUTHORITY_EXECUTE needs a request_id.");
        }
        let Some((thread, experiment, request, rows)) =
            self.find_authority_request(db, request_id)?
        else {
            anyhow::bail!("Authority request `{request_id}` was not found.");
        };
        let path = self.authority_gate_path(&thread.thread_id);
        let approval = latest_active_authority_approval(&rows, request_id);
        if approval.is_none() {
            if request.get("status").and_then(Value::as_str) == Some("pending_budget_execution")
                && let Some(budget) = active_authority_budget_from_rows(
                    &rows,
                    &experiment.experiment_id,
                    request
                        .get("scope")
                        .and_then(Value::as_str)
                        .unwrap_or("semantic_microdose"),
                )
                && budget
                    .get("pending_review_request_id")
                    .and_then(Value::as_str)
                    .is_none()
            {
                return Ok(format!(
                    "Authority request `{request_id}` is budget-backed and bridge-executable. Use the live Astrid authority gate executor so one budget slot is consumed exactly once."
                ));
            }
            let blocked = self.authority_gate_record(
                "blocked",
                request_id,
                &thread,
                &experiment,
                &state,
                json!({
                    "scope": request.get("scope").cloned().unwrap_or(Value::Null),
                    "reason": "missing_steward_approval",
                    "token_status": "none",
                }),
            );
            self.append_jsonl(&path, &blocked)?;
            let consequence =
                self.authority_consequence_record(&thread, &experiment, &request, &blocked, &state);
            self.append_jsonl(&path, &consequence)?;
            return Ok(format!(
                "Authority execute blocked for `{request_id}`: missing steward approval. No live semantic write was attempted."
            ));
        }
        Ok(format!(
            "Authority request `{request_id}` is steward-approved. Execution is handled by the live Astrid authority gate path so the semantic token can be consumed exactly once."
        ))
    }

    pub fn experiment_authority_review_command(
        &self,
        db: Option<&BridgeDb>,
        raw: &str,
        state: Value,
    ) -> Result<String> {
        let (request_id_opt, payload) = parse_selector_payload(raw);
        let request_id = request_id_opt.unwrap_or_default();
        if request_id.trim().is_empty() {
            anyhow::bail!("EXPERIMENT_AUTHORITY_REVIEW needs a request_id.");
        }
        let Some((mut thread, experiment, request, rows)) =
            self.find_authority_request(db, &request_id)?
        else {
            anyhow::bail!("Authority request `{request_id}` was not found.");
        };
        let outcome = dossier_field(&payload, &["outcome"]).unwrap_or_else(|| "hold".to_string());
        let outcome = if matches!(outcome.as_str(), "hold" | "repeat" | "alter" | "retire") {
            outcome
        } else {
            "hold".to_string()
        };
        let observation = dossier_field(&payload, &["observation", "summary", "because"])
            .unwrap_or_else(|| payload.trim().to_string());
        let next_payload =
            dossier_field(&payload, &["next_payload", "payload"]).unwrap_or_default();
        let budget_id = request
            .get("budget_id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .or_else(|| budget_id_for_request(&rows, &request_id))
            .unwrap_or_else(|| format!("review_{request_id}"));
        let next_safe_command = authority_review_next_command(&outcome, &request, &next_payload);
        let review = self.authority_budget_record(
            "consequence_review",
            &budget_id,
            &thread,
            &experiment,
            &state,
            json!({
                "request_id": request_id,
                "scope": request.get("scope").cloned().unwrap_or(Value::Null),
                "outcome": outcome,
                "observation": observation,
                "next_payload": next_payload,
                "source_refs": dossier_list_field(&payload, &["source_refs", "source_ref", "source"]),
                "next_safe_command": next_safe_command,
            }),
        );
        let path = self.authority_gate_path(&thread.thread_id);
        self.append_jsonl(&path, &review)?;
        if review.get("outcome").and_then(Value::as_str) == Some("retire") {
            let closed = self.authority_budget_record(
                "budget_closed",
                &budget_id,
                &thread,
                &experiment,
                &state,
                json!({
                    "request_id": request_id,
                    "reason": "being_retired_budget_after_consequence_review",
                }),
            );
            self.append_jsonl(&path, &closed)?;
        }
        let memory = self.append_being_memory_record(
            &mut thread,
            Some(&experiment),
            "authority_consequence_review",
            &format!(
                "Authority consequence review for `{}`: {}.",
                review
                    .get("request_id")
                    .and_then(Value::as_str)
                    .unwrap_or("request"),
                review
                    .get("outcome")
                    .and_then(Value::as_str)
                    .unwrap_or("hold")
            ),
            vec![path.display().to_string()],
            request
                .get("artifact_refs")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            Some(next_safe_command.clone()),
            "draft",
            json!({"authority_review_v1": review.clone(), "dossier_candidate": true}),
        )?;
        Ok(format!(
            "Authority review `{}` recorded outcome={}.\nMemory draft: {}\nNext safe command: {}\nauthority_budget_v1:\n{}",
            review
                .get("record_id")
                .and_then(Value::as_str)
                .unwrap_or("review"),
            review
                .get("outcome")
                .and_then(Value::as_str)
                .unwrap_or("hold"),
            memory
                .get("memory_id")
                .and_then(Value::as_str)
                .unwrap_or("memory"),
            next_safe_command,
            serde_json::to_string_pretty(&review)?
        ))
    }

    fn latest_local_conveyor_readout(
        &self,
        thread: &ResearchThread,
        mode: &str,
        state: &Value,
    ) -> Result<Value> {
        if let Some(summary) = last_experiment_summary_v1(thread)
            && let Some(experiment_id) = summary.get("experiment_id").and_then(Value::as_str)
            && let Some(experiment) =
                self.find_experiment_by_id(&thread.thread_id, experiment_id)?
        {
            let runs =
                self.recent_experiment_runs(&thread.thread_id, &experiment.experiment_id, 8)?;
            let mut readout =
                self.experiment_conveyor_v1(thread, &experiment, &runs, mode, state)?;
            readout["status_context"] = json!("no_active_current_latest_local");
            readout["raw_next_preserved"] = json!(true);
            readout["guardrail_reason"] =
                json!("experiment_plan_current_without_active_experiment");
            if let Some(warnings) = readout
                .get_mut("guardrail_warnings")
                .and_then(Value::as_array_mut)
            {
                warnings.push(json!("current has no active experiment; preview is showing the latest local experiment by id"));
            }
            return Ok(readout);
        }
        self.no_active_conveyor_readout(thread, mode)
    }

    fn no_active_conveyor_readout(&self, thread: &ResearchThread, mode: &str) -> Result<Value> {
        Ok(json!({
            "schema_version": 1,
            "policy": "experiment_conveyor_v1",
            "mode": mode,
            "preview_allowed": true,
            "apply_policy": "conservative_local_v1",
            "allowed_apply_steps": experiment_conveyor_allowed_apply_steps(),
            "applied": false,
            "would_mutate": false,
            "thread_id": &thread.thread_id,
            "experiment_id": null,
            "status": "no_active_experiment",
            "stage": "blocked_guardrail",
            "missing_requirements": ["active_local_experiment"],
            "proposed_next": "EXPERIMENT_START <title> :: <question>",
            "conveyor_next": "EXPERIMENT_ADVANCE <experiment_id> :: mode: preview",
            "can_apply": false,
            "apply_blocked_reason": if mode == "apply" {
                "apply_current_requires_active_local_experiment"
            } else {
                "no_latest_local_experiment"
            },
            "source_refs": [self.thread_dir(&thread.thread_id).join("thread.json").display().to_string()],
            "guardrail_warnings": ["current has no active local experiment; preview can inspect an explicit local experiment id"],
            "authority_readiness_v1": {
                "policy": "authority_readiness_v1",
                "scope": "semantic_microdose",
                "stage": "blocked",
                "eligible_to_request": false,
                "missing_requirements": ["active_local_experiment"],
                "artifact_ref_candidates": [],
                "latest_request_id": null,
                "token_status": "none",
                "next_safe_command": "EXPERIMENT_ADVANCE <experiment_id> :: mode: preview",
                "request_scaffold": null,
                "source_refs": [self.thread_dir(&thread.thread_id).join("thread.json").display().to_string()],
                "authority_boundary": authority_gate_boundary(),
            },
            "authority_boundary": experiment_conveyor_authority_boundary(),
        }))
    }

    fn authority_request_payload(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        raw_payload: &str,
        state: &Value,
    ) -> Result<Value> {
        let scope = dossier_field(raw_payload, &["scope"])
            .unwrap_or_else(|| "semantic_microdose".to_string());
        let payload = dossier_field(raw_payload, &["payload", "semantic_payload", "text"])
            .unwrap_or_else(|| raw_payload.trim().to_string());
        let reason =
            dossier_field(raw_payload, &["reason", "because", "rationale"]).unwrap_or_default();
        let stop_criteria =
            dossier_field(raw_payload, &["stop_criteria", "stop"]).unwrap_or_default();
        let artifact_refs = dossier_list_field(
            raw_payload,
            &[
                "artifact_refs",
                "artifact_ref",
                "artifact_grounding",
                "artifact",
            ],
        );
        let request_id = self.unique_authority_request_id(&experiment.experiment_id)?;
        let source_refs = self.authority_source_refs(thread, experiment, &artifact_refs);
        Ok(self.authority_gate_record(
            "request",
            &request_id,
            thread,
            experiment,
            state,
            json!({
                "scope": scope,
                "payload": payload,
                "reason": reason,
                "artifact_refs": artifact_refs,
                "source_refs": source_refs,
                "stop_criteria": stop_criteria,
                "token_status": "none",
            }),
        ))
    }

    fn authority_gate_record(
        &self,
        record_type: &str,
        request_id: &str,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        state: &Value,
        extra: Value,
    ) -> Value {
        let now = iso_now();
        let mut record = json!({
            "schema_version": 1,
            "record_schema": "authority_gate_v1",
            "record_type": record_type,
            "record_id": format!("auth_{SYSTEM}_{}_{}", now_millis(), sanitize_slug(record_type)),
            "request_id": request_id,
            "being": SYSTEM,
            "thread_id": thread.thread_id,
            "experiment_id": experiment.experiment_id,
            "created_at": now,
            "updated_at": now,
            "safety_snapshot": authority_safety_snapshot(state),
            "peer_mutation": false,
            "authority_boundary": authority_gate_boundary(),
        });
        if let (Some(target), Some(source)) = (record.as_object_mut(), extra.as_object()) {
            for (key, value) in source {
                target.insert(key.clone(), value.clone());
            }
        }
        record
    }

    fn authority_budget_record(
        &self,
        record_type: &str,
        budget_id: &str,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        state: &Value,
        extra: Value,
    ) -> Value {
        let now = iso_now();
        let mut record = json!({
            "schema_version": SCHEMA_VERSION,
            "record_schema": "authority_budget_v1",
            "record_type": record_type,
            "record_id": format!("authbud_{SYSTEM}_{}_{}", now_millis(), sanitize_slug(record_type)),
            "budget_id": budget_id,
            "being": SYSTEM,
            "thread_id": thread.thread_id,
            "experiment_id": experiment.experiment_id,
            "created_at": now,
            "updated_at": now,
            "safety_snapshot": authority_safety_snapshot(state),
            "peer_mutation": false,
            "authority_boundary": authority_gate_boundary(),
        });
        if let (Some(target), Some(source)) = (record.as_object_mut(), extra.as_object()) {
            for (key, value) in source {
                target.insert(key.clone(), value.clone());
            }
        }
        record
    }

    fn research_budget_record(
        &self,
        record_type: &str,
        budget_id: &str,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        state: &Value,
        extra: Value,
    ) -> Value {
        let now = iso_now();
        let mut record = json!({
            "schema_version": SCHEMA_VERSION,
            "record_schema": "research_budget_v1",
            "record_type": record_type,
            "record_id": format!("resbud_{SYSTEM}_{}_{}", now_millis(), sanitize_slug(record_type)),
            "budget_id": budget_id,
            "being": SYSTEM,
            "thread_id": thread.thread_id,
            "experiment_id": experiment.experiment_id,
            "created_at": now,
            "updated_at": now,
            "safety_snapshot": authority_safety_snapshot(state),
            "peer_mutation": false,
            "authority_boundary": research_budget_boundary(),
        });
        if let (Some(target), Some(source)) = (record.as_object_mut(), extra.as_object()) {
            for (key, value) in source {
                target.insert(key.clone(), value.clone());
            }
        }
        record
    }

    fn sovereign_loop_record(
        &self,
        record_type: &str,
        loop_id: &str,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        state: &Value,
        extra: Value,
    ) -> Value {
        let now = iso_now();
        let mut record = json!({
            "schema_version": SCHEMA_VERSION,
            "record_schema": "sovereign_loop_v1",
            "record_type": record_type,
            "record_id": format!("loop_{SYSTEM}_{}_{}", now_millis(), sanitize_slug(record_type)),
            "loop_id": loop_id,
            "being": SYSTEM,
            "thread_id": thread.thread_id,
            "experiment_id": experiment.experiment_id,
            "created_at": now,
            "updated_at": now,
            "authority_change": false,
            "peer_mutation": false,
            "safety_snapshot": authority_safety_snapshot(state),
            "authority_boundary": sovereign_loop_boundary(),
        });
        if let (Some(target), Some(source)) = (record.as_object_mut(), extra.as_object()) {
            for (key, value) in source {
                target.insert(key.clone(), value.clone());
            }
        }
        record
    }

    fn sovereign_loop_request_payload(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        raw_payload: &str,
        state: &Value,
    ) -> Value {
        let scope = dossier_field(raw_payload, &["consequence_scope", "scope"])
            .unwrap_or_else(|| "semantic_microdose".to_string());
        let purpose = dossier_field(raw_payload, &["purpose", "reason", "because"])
            .unwrap_or_else(|| raw_payload.trim().to_string());
        let max_research = dossier_field(raw_payload, &["max_research_actions", "max_actions"])
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(LOOP_RESEARCH_MAX_ACTIONS)
            .min(LOOP_RESEARCH_MAX_ACTIONS);
        let ttl_secs = dossier_field(raw_payload, &["ttl_secs", "ttl"])
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(LOOP_TTL_SECS)
            .min(LOOP_TTL_SECS)
            .max(1);
        let loop_id = format!(
            "loop_{SYSTEM}_{}_{}",
            now_millis(),
            sanitize_slug(&experiment.experiment_id)
        );
        let expires_at = u64::try_from(chrono::Utc::now().timestamp())
            .unwrap_or_default()
            .saturating_add(ttl_secs);
        self.sovereign_loop_record(
            "loop_request",
            &loop_id,
            thread,
            experiment,
            state,
            json!({
                "phase": "request",
                "status": "requested",
                "purpose": truncate_chars(&purpose, 1000),
                "consequence_scope": scope,
                "scope": scope,
                "max_research_actions": max_research,
                "remaining_local_research_actions": max_research,
                "ttl_secs": ttl_secs,
                "expires_at_unix_s": expires_at,
                "max_consequence_sends": LOOP_CONSEQUENCE_MAX_SENDS,
                "consequence_remaining": LOOP_CONSEQUENCE_MAX_SENDS,
                "pending_review": false,
                "stop_criteria": dossier_field(raw_payload, &["stop_criteria", "stop"]).unwrap_or_default(),
                "source_refs": dossier_list_field(raw_payload, &["source_refs", "source", "sources"]),
                "artifact_refs": dossier_list_field(raw_payload, &["artifact_refs", "artifact", "artifact_grounding"]),
                "next_safe_command": format!("EXPERIMENT_LOOP_STATUS {loop_id}"),
            }),
        )
    }

    fn sovereign_loop_eligibility(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        loop_row: &Value,
        state: &Value,
    ) -> Value {
        let mut missing = Vec::<String>::new();
        let scope = loop_row
            .get("consequence_scope")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let disabled_scope = !matches!(scope, "semantic_microdose" | "mode_release_microdose");
        if disabled_scope {
            missing.push("scope_semantic_or_mode_release_microdose_v1".to_string());
        }
        if loop_row
            .get("purpose")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
        {
            missing.push("loop_purpose".to_string());
        }
        let safety = authority_safety_snapshot(state);
        let level = safety
            .get("level")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if !matches!(level, "green" | "yellow" | "unknown") {
            missing.push("green_yellow_or_unknown_safety_for_local_loop".to_string());
        }
        if active_sovereign_loop_from_rows(
            &self.authority_gate_rows(&thread.thread_id),
            &experiment.experiment_id,
        )
        .is_some()
        {
            missing.push("no_active_sovereign_loop_for_experiment".to_string());
        }
        json!({
            "policy": "sovereign_loop_v1",
            "eligible": missing.is_empty(),
            "missing_requirements": missing,
            "disabled_scope": disabled_scope,
            "local_phases_self_start": true,
            "consequence_approval_required": "bridge_steward_one_slot",
            "max_research_actions_cap": LOOP_RESEARCH_MAX_ACTIONS,
            "ttl_secs_cap": LOOP_TTL_SECS,
            "max_consequence_sends_cap": LOOP_CONSEQUENCE_MAX_SENDS,
        })
    }

    fn find_sovereign_loop(
        &self,
        loop_id: &str,
    ) -> Result<Option<(ResearchThread, ExperimentRecord, Value, Vec<Value>)>> {
        let threads_dir = self.root.join("threads");
        if !threads_dir.exists() {
            return Ok(None);
        }
        for path in
            fs::read_dir(&threads_dir).with_context(|| format!("read {}", threads_dir.display()))?
        {
            let Ok(entry) = path else {
                continue;
            };
            let thread_id = entry.file_name().to_string_lossy().to_string();
            let rows = self.authority_gate_rows(&thread_id);
            let Some(row) = rows.iter().rev().find(|row| {
                row.get("record_schema").and_then(Value::as_str) == Some("sovereign_loop_v1")
                    && row.get("loop_id").and_then(Value::as_str) == Some(loop_id)
            }) else {
                continue;
            };
            let thread = self.read_thread(&thread_id)?;
            let experiment_id = row
                .get("experiment_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let Some(experiment) = self.find_experiment_by_id(&thread_id, experiment_id)? else {
                continue;
            };
            return Ok(Some((thread, experiment, row.clone(), rows)));
        }
        Ok(None)
    }

    fn sovereign_loop_status_v1(
        &self,
        thread: &ResearchThread,
        experiment: Option<&ExperimentRecord>,
        state: &Value,
        selector: &str,
        loop_id: Option<&str>,
    ) -> Value {
        let mut rows = self.authority_gate_rows(&thread.thread_id);
        if let Some(loop_id) = loop_id {
            rows.retain(|row| row.get("loop_id").and_then(Value::as_str) == Some(loop_id));
        } else if let Some(experiment) = experiment {
            rows.retain(|row| {
                row.get("experiment_id").and_then(Value::as_str)
                    == Some(experiment.experiment_id.as_str())
            });
        }
        let latest_request = rows
            .iter()
            .rev()
            .find(|row| row.get("record_type").and_then(Value::as_str) == Some("loop_request"));
        let latest_started = rows
            .iter()
            .rev()
            .find(|row| row.get("record_type").and_then(Value::as_str) == Some("loop_started"));
        let latest_approval = rows
            .iter()
            .rev()
            .find(|row| row.get("record_type").and_then(Value::as_str) == Some("loop_approval"));
        let latest_step = rows
            .iter()
            .rev()
            .find(|row| row.get("record_type").and_then(Value::as_str) == Some("loop_step"));
        let latest_ready = rows.iter().rev().find(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("loop_consequence_ready")
        });
        let latest_review = rows.iter().rev().find(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("loop_consequence_review")
        });
        let latest_closed = rows
            .iter()
            .rev()
            .find(|row| row.get("record_type").and_then(Value::as_str) == Some("loop_closed"));
        let latest_blocked = rows
            .iter()
            .rev()
            .find(|row| row.get("record_type").and_then(Value::as_str) == Some("loop_blocked"));
        let latest_proposal = rows
            .iter()
            .rev()
            .find(|row| row.get("record_type").and_then(Value::as_str) == Some("loop_proposal"));
        let anchor = latest_approval
            .or(latest_step)
            .or(latest_started)
            .or(latest_request);
        let active_loop = experiment
            .and_then(|experiment| {
                active_sovereign_loop_from_rows(&rows, &experiment.experiment_id)
            })
            .or_else(|| anchor.cloned());
        let active_loop_id = active_loop
            .as_ref()
            .and_then(|row| row.get("loop_id"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let active_experiment_id = active_loop
            .as_ref()
            .and_then(|row| row.get("experiment_id"))
            .and_then(Value::as_str)
            .or_else(|| experiment.map(|experiment| experiment.experiment_id.as_str()))
            .unwrap_or_default();
        let latest_consequence = self
            .authority_gate_rows(&thread.thread_id)
            .into_iter()
            .rev()
            .find(|row| {
                matches!(
                    row.get("record_schema").and_then(Value::as_str),
                    Some("authority_consequence_v1" | "mode_release_consequence_v1")
                ) && (!active_experiment_id.is_empty()
                    && row.get("experiment_id").and_then(Value::as_str)
                        == Some(active_experiment_id))
            });
        let pending_review = latest_consequence.is_some() && latest_review.is_none();
        let stage = if latest_closed.is_some() {
            "closed"
        } else if pending_review {
            "review_required"
        } else if latest_approval.is_some() {
            "consequence_slot_approved"
        } else if latest_ready.is_some() {
            "consequence_ready"
        } else if active_loop.is_some() {
            "active"
        } else if latest_blocked.is_some() {
            "blocked"
        } else if latest_request.is_some() {
            "requested"
        } else {
            "no_loop"
        };
        let phase = anchor
            .and_then(|row| row.get("phase"))
            .and_then(Value::as_str)
            .unwrap_or("none");
        let research_remaining = active_loop.as_ref().map_or(0, |row| {
            let max_research = row
                .get("remaining_local_research_actions")
                .or_else(|| row.get("max_research_actions"))
                .and_then(Value::as_u64)
                .unwrap_or(LOOP_RESEARCH_MAX_ACTIONS);
            let spent = self
                .authority_gate_rows(&thread.thread_id)
                .iter()
                .filter(|item| {
                    item.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
                        && item.get("record_type").and_then(Value::as_str)
                            == Some("research_budget_debit")
                        && item.get("experiment_id").and_then(Value::as_str)
                            == Some(active_experiment_id)
                })
                .count();
            max_research.saturating_sub(u64::try_from(spent).unwrap_or(u64::MAX))
        });
        let consequence_remaining = active_loop.as_ref().map_or(0, |row| {
            row.get("consequence_remaining")
                .or_else(|| row.get("max_consequence_sends"))
                .and_then(Value::as_u64)
                .unwrap_or(LOOP_CONSEQUENCE_MAX_SENDS)
        });
        let next_safe_command = if stage == "active" && !active_loop_id.is_empty() {
            format!(
                "EXPERIMENT_LOOP_STEP {active_loop_id} :: continuity|research|sticky_audit|authority_prepare|authority_request|review|close"
            )
        } else if stage == "review_required" && !active_loop_id.is_empty() {
            format!(
                "EXPERIMENT_LOOP_REVIEW {active_loop_id} :: outcome: hold|repeat|alter|retire|promote; observation: ...; next: ...; source_refs: ..."
            )
        } else if !active_loop_id.is_empty() {
            format!("EXPERIMENT_LOOP_STATUS {active_loop_id}")
        } else {
            default_owned_loop_request_scaffold("current")
        };
        let sticky_readiness = experiment
            .filter(|_| active_loop.is_some())
            .map(|experiment| {
                self.sovereign_loop_consequence_readiness(
                    thread,
                    experiment,
                    active_loop.as_ref().unwrap(),
                    state,
                )
            });
        json!({
            "schema_version": 1,
            "policy": "sovereign_loop_v1",
            "being": SYSTEM,
            "thread_id": thread.thread_id,
            "experiment_id": if active_experiment_id.is_empty() { Value::Null } else { json!(active_experiment_id) },
            "selector": selector,
            "loop_id": if active_loop_id.is_empty() { Value::Null } else { json!(active_loop_id) },
            "stage": stage,
            "phase": phase,
            "consequence_scope": active_loop.as_ref().and_then(|row| row.get("consequence_scope")).cloned().unwrap_or_else(|| json!("semantic_microdose")),
            "remaining_local_research_actions": research_remaining,
            "consequence_remaining": consequence_remaining,
            "pending_review": pending_review,
            "latest_loop_request_id": latest_request.and_then(|row| row.get("loop_id")).cloned().unwrap_or(Value::Null),
            "latest_consequence_v1": latest_consequence,
            "latest_review_v1": latest_review.cloned().unwrap_or(Value::Null),
            "latest_loop_proposal_v1": latest_proposal.cloned().unwrap_or(Value::Null),
            "sticky_readiness_v1": sticky_readiness.unwrap_or(Value::Null),
            "latest_rows": rows.into_iter().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>(),
            "next_safe_command": next_safe_command,
            "source_refs": [self.authority_gate_path(&thread.thread_id).display().to_string()],
            "authority_boundary": sovereign_loop_boundary(),
        })
    }

    fn sovereign_loop_consequence_readiness(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        loop_row: &Value,
        state: &Value,
    ) -> Value {
        let scope = loop_row
            .get("consequence_scope")
            .and_then(Value::as_str)
            .unwrap_or("semantic_microdose");
        let runs = self
            .recent_experiment_runs(&thread.thread_id, &experiment.experiment_id, 12)
            .unwrap_or_default();
        let mut artifact_refs = value_string_list(loop_row.get("artifact_refs"));
        if artifact_refs.is_empty() {
            artifact_refs = authority_artifact_ref_candidates(experiment, &runs);
        }
        if scope == "mode_release_microdose" {
            let mut missing = Vec::<String>::new();
            if !lifecycle_valid_charter_value(experiment.charter_v1.as_ref()) {
                missing.push("lifecycle_valid_charter".to_string());
            }
            if !experiment_evidence_is_meaningful(experiment.evidence_v1.as_ref()) {
                missing.push("meaningful_evidence".to_string());
            }
            if artifact_refs.is_empty() {
                missing.push("artifact_grounding_refs".to_string());
            }
            if !authority_has_read_only_rehearsal(&runs) {
                missing.push("read_only_rehearsal".to_string());
            }
            missing.push("sticky_mode_release_candidate_bridge_status".to_string());
            missing.push("no_spontaneous_release_watch_bridge_status".to_string());
            let ready = missing.is_empty();
            let scaffold = format!(
                "EXPERIMENT_AUTHORITY_REQUEST {} :: scope: mode_release_microdose; payload: target=esn_leak; value=...; duration_ticks=3; reason: owned loop sticky-mode release candidate; artifact_refs: {}; stop_criteria: one attempted bridge send only with rollback.",
                experiment.experiment_id,
                artifact_refs.join(", ")
            );
            return json!({
                "policy": "sovereign_loop_consequence_readiness_v1",
                "scope": scope,
                "stage": if ready { "ready_to_author_request" } else { "missing_requirements" },
                "eligible_to_request": ready,
                "missing_requirements": missing,
                "artifact_ref_candidates": artifact_refs,
                "request_scaffold": if ready { json!(scaffold) } else { Value::Null },
                "next_safe_command": if ready { scaffold } else { "STICKY_MODE_AUDIT".to_string() },
                "authority_boundary": authority_gate_boundary(),
            });
        }
        let request = json!({
            "scope": "semantic_microdose",
            "payload": loop_row
                .get("purpose")
                .and_then(Value::as_str)
                .unwrap_or("owned loop semantic consequence"),
            "artifact_refs": artifact_refs,
        });
        let eligibility = self.authority_gate_eligibility(thread, experiment, &request, state);
        let ready = eligibility
            .get("eligible")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let scaffold = format!(
            "EXPERIMENT_AUTHORITY_REQUEST {} :: scope: semantic_microdose; payload: ...; reason: owned loop consequence; artifact_refs: {}; stop_criteria: one attempted bridge send only.",
            experiment.experiment_id,
            artifact_refs.join(", ")
        );
        json!({
            "policy": "sovereign_loop_consequence_readiness_v1",
            "scope": scope,
            "stage": if ready { "ready_to_author_request" } else { "missing_requirements" },
            "eligible_to_request": ready,
            "missing_requirements": eligibility
                .get("missing_requirements")
                .cloned()
                .unwrap_or_else(|| json!([])),
            "artifact_ref_candidates": artifact_refs,
            "request_scaffold": if ready { json!(scaffold) } else { Value::Null },
            "next_safe_command": if ready { scaffold } else { format!("EXPERIMENT_ADVANCE {} :: mode: preview", experiment.experiment_id) },
            "authority_boundary": authority_gate_boundary(),
        })
    }

    fn sovereign_loop_step_next_command(
        &self,
        step: &str,
        loop_row: &Value,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        state: &Value,
        _status: &Value,
    ) -> String {
        let loop_id = loop_row
            .get("loop_id")
            .and_then(Value::as_str)
            .unwrap_or("latest");
        match step {
            "continuity" => format!(
                "CONTINUITY_SESSION_START {} :: title: Owned loop; focus: continuity, local research, sticky audit, one gated consequence, and review; next: EXPERIMENT_LOOP_STEP {loop_id} :: research",
                experiment.experiment_id
            ),
            "research" => {
                let rows = self.authority_gate_rows(&thread.thread_id);
                if let Some(active) =
                    active_research_budget_from_rows(&rows, &experiment.experiment_id)
                    && let Some(budget_id) = active.get("budget_id").and_then(Value::as_str)
                {
                    return format!("EXPERIMENT_RESEARCH_BUDGET_STATUS {budget_id}");
                }
                if latest_research_budget_scaffold_row(&rows, &experiment.experiment_id).is_some() {
                    return "EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest".to_string();
                }
                research_budget_request_scaffold(&experiment.experiment_id, experiment)
            },
            "sticky_audit" => "STICKY_MODE_AUDIT".to_string(),
            "authority_prepare" => {
                let scope = loop_row
                    .get("consequence_scope")
                    .and_then(Value::as_str)
                    .unwrap_or("semantic_microdose");
                if scope == "mode_release_microdose" {
                    format!(
                        "EXPERIMENT_AUTHORITY_PREPARE {} :: scope: mode_release_microdose; payload: target=esn_leak; value=...; duration_ticks=3; reason: owned loop sticky-mode release preflight; artifact_refs: ...; stop_criteria: one attempted bridge send only with rollback.",
                        experiment.experiment_id
                    )
                } else {
                    format!(
                        "EXPERIMENT_AUTHORITY_PREPARE {} :: scope: semantic_microdose; payload: ...; reason: owned loop consequence preflight; artifact_refs: ...; stop_criteria: one attempted bridge send only.",
                        experiment.experiment_id
                    )
                }
            },
            "authority_request" => self
                .sovereign_loop_consequence_readiness(thread, experiment, loop_row, state)
                .get("request_scaffold")
                .and_then(Value::as_str)
                .map_or_else(
                    || {
                        format!(
                            "EXPERIMENT_ADVANCE {} :: mode: preview",
                            experiment.experiment_id
                        )
                    },
                    ToString::to_string,
                ),
            "review" => format!(
                "EXPERIMENT_LOOP_REVIEW {loop_id} :: outcome: hold|repeat|alter|retire|promote; observation: ...; next: ...; source_refs: ..."
            ),
            "close" => "THREAD_STATUS current".to_string(),
            _ => format!("EXPERIMENT_LOOP_STATUS {loop_id}"),
        }
    }

    fn append_loop_continuity_checkpoint_draft(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        loop_id: &str,
        phase: &str,
        source_record: &Value,
        next_command: &str,
    ) -> Result<Value> {
        let active_rows =
            self.continuity_session_rows(&thread.thread_id, Some(&experiment.experiment_id), 8)?;
        let has_existing_session = !active_rows.is_empty();
        let session_id = active_rows
            .last()
            .and_then(|row| row.get("session_id"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!(
                    "sess_{SYSTEM}_{}_{}",
                    now_millis(),
                    sanitize_slug("owned-loop-checkpoint")
                )
            });
        let commit_kind = if has_existing_session {
            "session_capture"
        } else {
            "session_start"
        };
        let record = self.continuity_session_record(
            "session_draft",
            &session_id,
            thread,
            Some(experiment),
            "draft",
            ContinuitySessionFields {
                title: Some("Owned loop checkpoint".to_string()),
                focus: Some(
                    "preserve owned loop phase progress before spending more research or consequence authority"
                        .to_string(),
                ),
                summary: Some(format!(
                    "Owned loop `{loop_id}` recorded phase `{phase}`. Preserve the checkpoint before continuing. Next safe command: {next_command}"
                )),
                open_questions: Vec::new(),
                source_refs: vec![
                    self.authority_gate_path(&thread.thread_id)
                        .display()
                        .to_string(),
                    source_record
                        .get("record_id")
                        .and_then(Value::as_str)
                        .unwrap_or("loop_step")
                        .to_string(),
                ],
                artifact_refs: Vec::new(),
                suggested_next: Some(next_command.to_string()),
                extra: json!({
                    "draft_v1": true,
                    "checkpoint_v1": true,
                    "reason": format!("sovereign_loop_checkpoint_{phase}"),
                    "raw_intent": format!("EXPERIMENT_LOOP_STEP {loop_id} :: {phase}"),
                    "commit_kind": commit_kind,
                    "loop_id": loop_id,
                    "loop_phase": phase,
                    "source_loop_record_id": source_record.get("record_id").cloned().unwrap_or(Value::Null),
                    "accept_next": "CONTINUITY_SESSION_ACCEPT latest",
                    "generic_accept_next": "ACCEPT_SUGGESTED_NEXT latest",
                    "ignored_until_accepted": true,
                    "authority_change": false,
                    "peer_mutation": false,
                }),
            },
        );
        self.append_jsonl(&self.continuity_sessions_path(&thread.thread_id), &record)?;
        Ok(record)
    }

    fn sovereign_loop_proposal_record(
        &self,
        loop_id: &str,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        state: &Value,
        review: &Value,
    ) -> Value {
        let outcome = review
            .get("outcome")
            .and_then(Value::as_str)
            .unwrap_or("hold");
        let scope = review
            .get("scope")
            .or_else(|| review.get("consequence_scope"))
            .and_then(Value::as_str)
            .unwrap_or("semantic_microdose");
        let observation = review
            .get("observation")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let purpose = if observation.trim().is_empty() {
            format!("continue owned loop after {outcome} review")
        } else {
            format!(
                "continue owned loop after {outcome} review: {}",
                truncate_chars(observation, 220)
            )
        };
        let scaffold = owned_loop_request_scaffold(
            &experiment.experiment_id,
            &purpose,
            scope,
            "preserve review before another consequence",
        );
        self.sovereign_loop_record(
            "loop_proposal",
            loop_id,
            thread,
            experiment,
            state,
            json!({
                "phase": "proposal",
                "status": "draft",
                "proposal_id": format!("loopprop_{SYSTEM}_{}_{}", now_millis(), sanitize_slug(loop_id)),
                "source_loop_id": loop_id,
                "source_review_record_id": review.get("record_id").cloned().unwrap_or(Value::Null),
                "outcome": outcome,
                "consequence_scope": scope,
                "scope": scope,
                "suggested_request_scaffold": scaffold,
                "ignored_until_requested": true,
                "authority_change": false,
                "peer_mutation": false,
                "next_safe_command": "EXPERIMENT_LOOP_STATUS latest",
            }),
        )
    }

    fn authority_budget_request_payload(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        raw_payload: &str,
        state: &Value,
    ) -> Result<Value> {
        let scope = dossier_field(raw_payload, &["scope"])
            .unwrap_or_else(|| "semantic_microdose".to_string());
        let purpose = dossier_field(raw_payload, &["purpose", "reason", "because", "rationale"])
            .unwrap_or_else(|| raw_payload.trim().to_string());
        let max_sends = dossier_field(raw_payload, &["max_sends", "sends"])
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(AUTHORITY_BUDGET_MAX_SENDS)
            .clamp(1, AUTHORITY_BUDGET_MAX_SENDS);
        let ttl_secs = dossier_field(raw_payload, &["ttl_secs", "ttl"])
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(LOCAL_RESEARCH_TTL_SECS)
            .clamp(1, LOCAL_RESEARCH_TTL_SECS);
        let artifact_refs = dossier_list_field(
            raw_payload,
            &[
                "artifact_refs",
                "artifact_ref",
                "artifact_grounding",
                "artifact",
            ],
        );
        let budget_id = self.unique_authority_budget_id(&experiment.experiment_id)?;
        let source_refs = self.authority_source_refs(thread, experiment, &artifact_refs);
        Ok(self.authority_budget_record(
            "budget_request",
            &budget_id,
            thread,
            experiment,
            state,
            json!({
                "scope": scope,
                "purpose": purpose.chars().take(1000).collect::<String>(),
                "max_sends": max_sends,
                "ttl_secs": ttl_secs,
                "artifact_refs": artifact_refs,
                "stop_criteria": dossier_field(raw_payload, &["stop_criteria", "stop"]).unwrap_or_default(),
                "source_refs": source_refs,
                "token_status": "none",
                "review_required": false,
            }),
        ))
    }

    fn authority_budget_eligibility(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        budget: &Value,
        state: &Value,
    ) -> Value {
        let request_like = json!({
            "scope": budget.get("scope").cloned().unwrap_or_else(|| json!("semantic_microdose")),
            "payload": budget.get("purpose").cloned().unwrap_or(Value::Null),
            "artifact_refs": budget.get("artifact_refs").cloned().unwrap_or_else(|| json!([])),
        });
        let gate = self.authority_gate_eligibility(thread, experiment, &request_like, state);
        let mut missing = gate
            .get("missing_requirements")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for item in &mut missing {
            if item == "semantic_payload" {
                *item = "budget_purpose".to_string();
            }
        }
        if active_authority_budget_from_rows(
            &self.authority_gate_rows(&thread.thread_id),
            &experiment.experiment_id,
            budget
                .get("scope")
                .and_then(Value::as_str)
                .unwrap_or("semantic_microdose"),
        )
        .is_some()
        {
            missing.push("no_active_budget_for_scope".to_string());
        }
        json!({
            "policy": "authority_budget_v1",
            "eligible": missing.is_empty(),
            "missing_requirements": missing,
            "disabled_scope": gate.get("disabled_scope").cloned().unwrap_or(Value::Bool(false)),
            "approval_required": "being_plus_steward_budget_envelope",
            "enabled_execution_scope": "semantic_microdose",
            "future_scopes_disabled": ["attractor_pulse", "control_envelope"],
            "max_sends_cap": AUTHORITY_BUDGET_MAX_SENDS,
            "ttl_secs_cap": LOCAL_RESEARCH_TTL_SECS,
        })
    }

    fn research_budget_request_payload(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        raw_payload: &str,
        state: &Value,
    ) -> Result<Value> {
        let scope = dossier_field(raw_payload, &["scope"])
            .unwrap_or_else(|| "read_only_research".to_string());
        let purpose = dossier_field(raw_payload, &["purpose", "reason", "because", "rationale"])
            .or_else(|| (!raw_payload.contains(':')).then(|| raw_payload.trim().to_string()))
            .unwrap_or_default();
        let allowed_sources_raw = dossier_field(raw_payload, &["allowed_sources", "sources"])
            .unwrap_or_else(|| "web,local".to_string());
        let allowed_sources = allowed_sources_raw
            .split([',', '/', '|'])
            .map(str::trim)
            .filter(|source| matches!(*source, "web" | "local"))
            .collect::<Vec<_>>();
        let allowed_sources = if allowed_sources.is_empty() {
            vec!["web", "local"]
        } else {
            allowed_sources
        };
        let max_actions = dossier_field(raw_payload, &["max_actions", "actions"])
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(LOCAL_RESEARCH_MAX_ACTIONS)
            .clamp(1, STEWARD_RESEARCH_MAX_ACTIONS);
        let ttl_secs = dossier_field(raw_payload, &["ttl_secs", "ttl"])
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(LOCAL_RESEARCH_TTL_SECS)
            .clamp(1, LOCAL_RESEARCH_TTL_SECS);
        let budget_id = self.unique_research_budget_id(&experiment.experiment_id)?;
        let source_refs = self.authority_source_refs(thread, experiment, &[]);
        Ok(self.research_budget_record(
            "research_budget_request",
            &budget_id,
            thread,
            experiment,
            state,
            json!({
                "scope": scope,
                "purpose": purpose.chars().take(1000).collect::<String>(),
                "max_actions": max_actions,
                "ttl_secs": ttl_secs,
                "allowed_sources": allowed_sources,
                "stop_criteria": dossier_field(raw_payload, &["stop_criteria", "stop"]).unwrap_or_default(),
                "source_refs": source_refs,
                "status": "requested",
                "review_required": false,
                "allowed_actions": ["SEARCH", "BROWSE", "READ_MORE", "MIKE_BROWSE", "MIKE_READ", "MIKE_SEARCH", "AR_LIST", "AR_LOOK", "AR_SHOW", "AR_READ", "AR_DEEP_READ", "AR_VALIDATE"],
            }),
        ))
    }

    fn research_budget_self_activation_record(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        budget: &Value,
        state: &Value,
    ) -> Value {
        let max_actions = budget
            .get("max_actions")
            .and_then(Value::as_u64)
            .unwrap_or(LOCAL_RESEARCH_MAX_ACTIONS)
            .min(LOCAL_RESEARCH_MAX_ACTIONS);
        let ttl_secs = budget
            .get("ttl_secs")
            .and_then(Value::as_u64)
            .unwrap_or(LOCAL_RESEARCH_TTL_SECS)
            .min(LOCAL_RESEARCH_TTL_SECS);
        let now = chrono::Utc::now().timestamp();
        let expires_at = u64::try_from(now)
            .unwrap_or_default()
            .saturating_add(ttl_secs);
        self.research_budget_record(
            "research_budget_approval",
            budget
                .get("budget_id")
                .and_then(Value::as_str)
                .unwrap_or("research_budget"),
            thread,
            experiment,
            state,
            json!({
                "scope": "read_only_research",
                "status": "active",
                "max_actions": max_actions,
                "ttl_secs": ttl_secs,
                "expires_at_unix_s": expires_at,
                "allowed_sources": ["local"],
                "activation_mode": "being_self_activated_local_v1",
                "self_activated": true,
                "steward_approval_required": false,
                "source_request_record_id": budget.get("record_id").cloned().unwrap_or(Value::Null),
                "source_refs": budget.get("source_refs").cloned().unwrap_or_else(|| json!([])),
                "review_required": false,
            }),
        )
    }

    fn research_budget_eligibility(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        budget: &Value,
        state: &Value,
    ) -> Value {
        let mut missing = Vec::new();
        let scope = budget
            .get("scope")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let disabled_scope = scope != "read_only_research";
        if disabled_scope {
            missing.push("scope_read_only_research_v1".to_string());
        }
        if budget
            .get("purpose")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            missing.push("research_purpose".to_string());
        }
        if active_research_budget_from_rows(
            &self.authority_gate_rows(&thread.thread_id),
            &experiment.experiment_id,
        )
        .is_some()
        {
            missing.push("no_active_read_only_research_budget".to_string());
        }
        let safety = authority_safety_snapshot(state);
        let safety_level = safety
            .get("level")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if !matches!(safety_level, "green" | "yellow" | "unknown") {
            missing.push("green_or_yellow_safety".to_string());
        }
        json!({
            "policy": "research_budget_v1",
            "eligible": missing.is_empty(),
            "missing_requirements": missing,
            "disabled_scope": disabled_scope,
            "approval_required": "being_plus_steward_research_budget",
            "enabled_execution_scope": "read_only_research",
            "disabled_actions": ["AR_START", "AR_NOTE", "AR_BLOCK", "AR_COMPLETE", "MIKE_RUN", "EXPERIMENT_BIND", "EXPERIMENT_RESUME", "PERTURB", "CONTROL", "semantic_microdose"],
            "max_actions_cap": STEWARD_RESEARCH_MAX_ACTIONS,
            "ttl_secs_cap": LOCAL_RESEARCH_TTL_SECS,
        })
    }

    fn authority_consequence_record(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        request: &Value,
        outcome: &Value,
        state: &Value,
    ) -> Value {
        let status =
            if outcome.get("record_type").and_then(Value::as_str) == Some("execution_result") {
                "sent"
            } else {
                "blocked"
            };
        let safety = authority_safety_snapshot(state);
        json!({
            "schema_version": SCHEMA_VERSION,
            "record_schema": "authority_consequence_v1",
            "record_type": "consequence",
            "record_id": format!("authcons_{SYSTEM}_{}_{}", now_millis(), sanitize_slug(status)),
            "being": SYSTEM,
            "thread_id": thread.thread_id,
            "experiment_id": experiment.experiment_id,
            "request_id": request.get("request_id").cloned().unwrap_or(Value::Null),
            "scope": request.get("scope").cloned().unwrap_or(Value::Null),
            "token_id": outcome.get("token_id").cloned().unwrap_or(Value::Null),
            "payload_summary": request
                .get("payload")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .take(160)
                .collect::<String>(),
            "pre_telemetry": safety.clone(),
            "post_telemetry": safety.clone(),
            "safety_snapshot": safety,
            "stop_criteria": request.get("stop_criteria").cloned().unwrap_or(Value::Null),
            "stop_criteria_result": if status == "sent" { "one_shot_sent_observe_once" } else { "not_executed" },
            "consequence_status": status,
            "reason": outcome.get("reason").cloned().unwrap_or_else(|| json!(status)),
            "outcome_ref": outcome.get("record_id").cloned().unwrap_or(Value::Null),
            "recommended_next_safe_command": format!(
                "EXPERIMENT_AUTHORITY_STATUS {}",
                request
                    .get("request_id")
                    .and_then(Value::as_str)
                    .unwrap_or("latest")
            ),
            "peer_mutation": false,
            "authority_boundary": authority_gate_boundary(),
            "created_at": iso_now(),
            "updated_at": iso_now(),
        })
    }

    fn authority_gate_eligibility(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        request: &Value,
        state: &Value,
    ) -> Value {
        let mut missing = Vec::<String>::new();
        let scope = request
            .get("scope")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let disabled_scope = scope != "semantic_microdose";
        if disabled_scope {
            missing.push("scope_semantic_microdose_v1".to_string());
        }
        if request
            .get("payload")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
        {
            missing.push("semantic_payload".to_string());
        }
        if !lifecycle_valid_charter_value(experiment.charter_v1.as_ref()) {
            missing.push("lifecycle_valid_charter".to_string());
        }
        if !experiment_evidence_is_meaningful(experiment.evidence_v1.as_ref()) {
            missing.push("meaningful_evidence".to_string());
        }
        if request
            .get("artifact_refs")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
        {
            missing.push("artifact_grounding_refs".to_string());
        }
        let runs = self
            .recent_experiment_runs(&thread.thread_id, &experiment.experiment_id, 12)
            .unwrap_or_default();
        if !authority_has_read_only_rehearsal(&runs) {
            missing.push("read_only_rehearsal".to_string());
        }
        let safety = authority_safety_snapshot(state);
        let level = safety
            .get("level")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if !matches!(level, "green" | "yellow") {
            missing.push("green_or_yellow_safety".to_string());
        }
        if authority_guardrail_hold_active(experiment) {
            missing.push("no_active_guardrail_hold".to_string());
        }
        json!({
            "policy": "authority_gate_v1",
            "eligible": missing.is_empty(),
            "missing_requirements": missing,
            "disabled_scope": disabled_scope,
            "approval_required": "being_plus_steward",
            "enabled_execution_scope": "semantic_microdose",
            "future_scopes_disabled": ["attractor_pulse", "control_envelope"],
        })
    }

    fn authority_readiness_v1(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        runs: &[ExperimentRunRecord],
        state: &Value,
        conveyor_stage: &str,
        proposed_next: &str,
    ) -> Value {
        let artifact_refs = authority_artifact_ref_candidates(experiment, runs);
        let request = json!({
            "scope": "semantic_microdose",
            "payload": "...",
            "artifact_refs": artifact_refs,
        });
        let eligibility = self.authority_gate_eligibility(thread, experiment, &request, state);
        let missing = eligibility
            .get("missing_requirements")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let rows = self
            .authority_gate_rows(&thread.thread_id)
            .into_iter()
            .filter(|row| {
                row.get("experiment_id").and_then(Value::as_str)
                    == Some(experiment.experiment_id.as_str())
            })
            .collect::<Vec<_>>();
        let latest_request = rows
            .iter()
            .rev()
            .find(|row| row.get("record_type").and_then(Value::as_str) == Some("request"));
        let latest_request_id = latest_request
            .and_then(|row| row.get("request_id"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let budget_status = authority_budget_status_from_rows(&rows);
        let mut token_status = authority_readiness_token_status(&rows, latest_request_id);
        if budget_status.get("stage").and_then(Value::as_str) == Some("review_required") {
            token_status = "review_required".to_string();
        }
        let stage = authority_readiness_stage(
            experiment,
            conveyor_stage,
            &missing,
            latest_request,
            &token_status,
        );
        let request_scaffold = (stage == "ready_to_author_request").then(|| {
            format!(
                "EXPERIMENT_AUTHORITY_REQUEST {} :: scope: semantic_microdose; payload: ...; reason: ...; artifact_refs: {}; stop_criteria: ...",
                experiment.experiment_id,
                artifact_refs.join(", ")
            )
        });
        let next_safe_command = authority_readiness_next_command(
            &experiment.experiment_id,
            &stage,
            proposed_next,
            latest_request_id,
            request_scaffold.as_deref(),
        );
        json!({
            "policy": "authority_readiness_v1",
            "scope": "semantic_microdose",
            "stage": stage,
            "eligible_to_request": stage == "ready_to_author_request",
            "missing_requirements": missing,
            "artifact_ref_candidates": artifact_refs,
            "latest_request_id": if latest_request_id.is_empty() { Value::Null } else { json!(latest_request_id) },
            "token_status": token_status,
            "next_safe_command": next_safe_command,
            "request_scaffold": request_scaffold,
            "budget_request_scaffold": if stage == "ready_to_author_request" {
                json!(authority_budget_request_scaffold(
                    &experiment.experiment_id,
                    "...",
                    &artifact_refs.join(", "),
                    "..."
                ))
            } else {
                Value::Null
            },
            "authority_budget_v1": budget_status,
            "source_refs": self.authority_source_refs(thread, experiment, &artifact_refs),
            "authority_boundary": authority_gate_boundary(),
        })
    }

    fn research_readiness_v1(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        _state: &Value,
    ) -> Value {
        let rows = self.authority_gate_rows(&thread.thread_id);
        let experiment_rows = rows
            .iter()
            .filter(|row| {
                row.get("experiment_id").and_then(Value::as_str)
                    == Some(experiment.experiment_id.as_str())
            })
            .cloned()
            .collect::<Vec<_>>();
        let status = research_budget_status_from_rows(&experiment_rows);
        let stage = status
            .get("stage")
            .and_then(Value::as_str)
            .unwrap_or("no_budget");
        let request_scaffold =
            default_local_research_budget_request_scaffold(&experiment.experiment_id);
        json!({
            "policy": "research_readiness_v1",
            "scope": "read_only_research",
            "stage": stage,
            "eligible_to_request": matches!(stage, "no_budget" | "blocked" | "budget_closed" | "budget_expired" | "budget_exhausted"),
            "missing_requirements": if stage == "no_budget" { json!(["self_activated_or_steward_approved_read_only_research_budget"]) } else { json!([]) },
            "active_budget_id": status.get("active_budget_id").cloned().unwrap_or(Value::Null),
            "remaining_actions": status.get("remaining_actions").cloned().unwrap_or_else(|| json!(0)),
            "allowed_actions": status.get("allowed_actions").cloned().unwrap_or_else(|| json!([])),
            "next_safe_command": status
                .get("next_safe_command")
                .cloned()
                .unwrap_or_else(|| json!(request_scaffold.clone())),
            "request_scaffold": request_scaffold,
            "authority_boundary": research_budget_boundary(),
        })
    }

    fn authority_source_refs(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        artifact_refs: &[String],
    ) -> Vec<String> {
        let mut refs = vec![
            self.thread_dir(&thread.thread_id)
                .join("thread.json")
                .display()
                .to_string(),
            self.experiments_path(&thread.thread_id)
                .display()
                .to_string(),
            self.experiment_runs_path(&thread.thread_id)
                .display()
                .to_string(),
            self.dossier_path(&thread.thread_id).display().to_string(),
            self.being_memory_path(&thread.thread_id)
                .display()
                .to_string(),
            self.authority_gate_path(&thread.thread_id)
                .display()
                .to_string(),
        ];
        if let Ok(Some(shared)) =
            self.shared_investigation_for_experiment(&experiment.experiment_id)
            && let Some(id) = shared.get("id").and_then(Value::as_str)
        {
            refs.push(format!("shared_investigation:{id}"));
        }
        refs.extend(artifact_refs.iter().cloned());
        refs
    }

    fn authority_gate_rows(&self, thread_id: &str) -> Vec<Value> {
        let raw = fs::read_to_string(self.authority_gate_path(thread_id)).unwrap_or_default();
        raw.lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|row| {
                matches!(
                    row.get("record_schema").and_then(Value::as_str),
                    Some(
                        "authority_gate_v1"
                            | "authority_budget_v1"
                            | "research_budget_v1"
                            | "sovereign_loop_v1"
                            | "authority_consequence_v1"
                            | "mode_release_consequence_v1"
                    )
                )
            })
            .collect()
    }

    fn being_memory_rows(
        &self,
        thread_id: &str,
        experiment_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Value>> {
        let path = self.being_memory_path(thread_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let mut rows = Vec::new();
        for line in raw.lines().rev() {
            let Ok(row) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if row.get("record_schema").and_then(Value::as_str) != Some("being_memory_v1") {
                continue;
            }
            if let Some(experiment_id) = experiment_id
                && row.get("experiment_id").and_then(Value::as_str) != Some(experiment_id)
            {
                continue;
            }
            rows.push(row);
            if rows.len() >= limit {
                break;
            }
        }
        rows.reverse();
        Ok(rows)
    }

    fn append_being_memory_record(
        &self,
        thread: &mut ResearchThread,
        experiment: Option<&ExperimentRecord>,
        card_type: &str,
        summary: &str,
        source_refs: Vec<String>,
        artifact_refs: Vec<String>,
        next_command: Option<String>,
        record_type: &str,
        extra: Value,
    ) -> Result<Value> {
        let now = iso_now();
        let mut record = json!({
            "schema_version": SCHEMA_VERSION,
            "record_schema": "being_memory_v1",
            "record_type": record_type,
            "memory_id": format!("mem_{SYSTEM}_{}_{}", now_millis(), sanitize_slug(card_type)),
            "being": SYSTEM,
            "thread_id": thread.thread_id,
            "experiment_id": experiment.map(|experiment| experiment.experiment_id.clone()),
            "card_type": card_type,
            "summary": truncate_chars(summary, 1000),
            "source_refs": source_refs,
            "artifact_refs": artifact_refs,
            "next_safe_command": next_command,
            "authority_change": false,
            "created_at": now,
            "updated_at": now,
        });
        if let (Some(target), Some(source)) = (record.as_object_mut(), extra.as_object()) {
            for (key, value) in source {
                target.insert(key.clone(), value.clone());
            }
        }
        self.append_jsonl(&self.being_memory_path(&thread.thread_id), &record)?;
        thread.updated_at = iso_now();
        self.write_thread(thread)?;
        Ok(record)
    }

    fn being_memory_summary_v1(
        &self,
        thread: &ResearchThread,
        experiment: Option<&ExperimentRecord>,
        focus: Option<&str>,
        limit: usize,
    ) -> Result<Value> {
        let mut rows = self.being_memory_rows(
            &thread.thread_id,
            experiment.map(|experiment| experiment.experiment_id.as_str()),
            64,
        )?;
        if let Some(focus) = focus.filter(|focus| !focus.trim().is_empty()) {
            let focus = focus.to_ascii_lowercase();
            rows.retain(|row| row.to_string().to_ascii_lowercase().contains(&focus));
        }
        let card_count = rows
            .iter()
            .filter(|row| row.get("record_type").and_then(Value::as_str) == Some("card"))
            .count();
        let draft_count = rows
            .iter()
            .filter(|row| row.get("record_type").and_then(Value::as_str) == Some("draft"))
            .count();
        let latest_card = rows
            .iter()
            .rev()
            .find(|row| row.get("record_type").and_then(Value::as_str) == Some("card"));
        let target = experiment
            .map(|experiment| experiment.experiment_id.as_str())
            .unwrap_or("latest");
        Ok(json!({
            "schema_version": 1,
            "policy": "being_memory_v1",
            "being": SYSTEM,
            "thread_id": thread.thread_id,
            "experiment_id": experiment.map(|experiment| experiment.experiment_id.clone()),
            "focus": focus,
            "card_count": card_count,
            "draft_count": draft_count,
            "latest_card": latest_card.cloned(),
            "recent_records": rows.into_iter().rev().take(limit).collect::<Vec<_>>(),
            "suggested_capture_next": format!("MEMORY_CAPTURE {target} :: summary: ...; source_refs: ...; artifact_refs: ...; next: ..."),
            "suggested_recall_next": format!("MEMORY_RECALL {target} :: focus: ..."),
            "suggested_promote_next": format!("MEMORY_PROMOTE {target} :: dossier|evidence|authority_request"),
            "authority_boundary": authority_gate_boundary(),
        }))
    }

    pub fn continuity_session_start_command(&self, raw: &str) -> Result<String> {
        let mut thread = self.ensure_active_thread(None)?;
        let (selector, payload) = parse_session_selector_payload(raw);
        let experiment = self.resolve_memory_experiment(&thread, selector.as_deref())?;
        let title =
            dossier_field(&payload, &["title"]).unwrap_or_else(|| "Continuity session".to_string());
        let focus =
            dossier_field(&payload, &["focus"]).unwrap_or_else(|| payload.trim().to_string());
        let session_id = format!("sess_{SYSTEM}_{}_{}", now_millis(), sanitize_slug(&title));
        let record = self.continuity_session_record(
            "session_start",
            &session_id,
            &thread,
            experiment.as_ref(),
            "active",
            ContinuitySessionFields {
                title: Some(title),
                focus: Some(focus),
                summary: dossier_field(&payload, &["summary"]),
                open_questions: dossier_list_field(&payload, &["open_questions", "questions", "question"]),
                source_refs: dossier_list_field(&payload, &["source_refs", "source", "sources"]),
                artifact_refs: dossier_list_field(&payload, &["artifact_refs", "artifact", "artifact_grounding"]),
                suggested_next: dossier_field(&payload, &["next", "next_safe_command"])
                    .or_else(|| Some(format!("CONTINUITY_SESSION_CAPTURE {session_id} :: summary: ...; source_refs: ...; artifact_refs: ...; next: ..."))),
                extra: json!({}),
            },
        );
        self.append_continuity_session_record(&mut thread, record)?;
        Ok(format!(
            "Continuity session `{session_id}` started.\nSuggested NEXT: CONTINUITY_SESSION_CAPTURE {session_id} :: summary: ...; source_refs: ...; artifact_refs: ...; next: ..."
        ))
    }

    pub fn continuity_session_capture_command(&self, raw: &str) -> Result<String> {
        let mut thread = self.ensure_active_thread(None)?;
        let (selector, payload) = parse_session_selector_payload(raw);
        let Some(session) = self.resolve_continuity_session(&thread, selector.as_deref())? else {
            return Ok("CONTINUITY_SESSION_CAPTURE needs an existing session. Start one with CONTINUITY_SESSION_START current :: title: ...; focus: ...; next: ...".to_string());
        };
        let summary = dossier_field(&payload, &["summary", "note", "memory"])
            .unwrap_or_else(|| payload.trim().to_string());
        if summary.trim().is_empty() {
            return Ok("CONTINUITY_SESSION_CAPTURE needs a summary.".to_string());
        }
        let experiment = self.session_experiment(&thread, &session)?;
        let session_id = session
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("latest")
            .to_string();
        let source_refs = dossier_list_field(&payload, &["source_refs", "source", "sources"]);
        let artifact_refs = dossier_list_field(
            &payload,
            &["artifact_refs", "artifact", "artifact_grounding"],
        );
        let next_command = dossier_field(&payload, &["next", "next_safe_command"]);
        let record = self.continuity_session_record(
            "session_capture",
            &session_id,
            &thread,
            experiment.as_ref(),
            "active",
            ContinuitySessionFields {
                title: session
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                focus: session
                    .get("focus")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                summary: Some(summary.clone()),
                open_questions: dossier_list_field(
                    &payload,
                    &["open_questions", "questions", "question"],
                ),
                source_refs: source_refs.clone(),
                artifact_refs: artifact_refs.clone(),
                suggested_next: next_command.clone(),
                extra: json!({}),
            },
        );
        self.append_continuity_session_record(&mut thread, record.clone())?;
        let continuity_path = self
            .continuity_sessions_path(&thread.thread_id)
            .display()
            .to_string();
        let memory = self.append_being_memory_record(
            &mut thread,
            experiment.as_ref(),
            "continuity_session_capture",
            &summary,
            {
                let mut refs = vec![
                    continuity_path,
                    record
                        .get("record_id")
                        .and_then(Value::as_str)
                        .unwrap_or("session_capture")
                        .to_string(),
                ];
                refs.extend(source_refs);
                refs
            },
            artifact_refs,
            next_command.clone().or_else(|| Some(format!("CONTINUITY_SESSION_CAPTURE {session_id} :: summary: ...; source_refs: ...; artifact_refs: ...; next: ..."))),
            "card",
            json!({"continuity_session_id": session_id}),
        )?;
        Ok(format!(
            "Continuity session `{}` captured as `{}`.\nMemory card: {}\nSuggested NEXT: CONTINUITY_SESSION_SUMMARIZE {} :: summary: ...; open_questions: ...; next: ...",
            session_id,
            record
                .get("record_id")
                .and_then(Value::as_str)
                .unwrap_or("session_capture"),
            memory
                .get("memory_id")
                .and_then(Value::as_str)
                .unwrap_or("memory"),
            session_id
        ))
    }

    pub fn continuity_session_accept_command(&self, raw: &str) -> Result<String> {
        let mut thread = self.ensure_active_thread(None)?;
        let selector = raw.trim();
        let selector = if selector.is_empty() {
            "latest"
        } else {
            selector
        };
        let Some(draft) = self.resolve_continuity_session_draft(&thread, Some(selector))? else {
            return Ok(
                "No continuity-session draft is available to accept. Wait for guarded pressure or start one with CONTINUITY_SESSION_START current :: title: ...; focus: ...; next: ..."
                    .to_string(),
            );
        };
        let experiment = self.session_experiment(&thread, &draft)?;
        let experiment_id = experiment
            .as_ref()
            .map(|experiment| experiment.experiment_id.as_str());
        let existing_rows = self.continuity_session_rows(&thread.thread_id, experiment_id, 16)?;
        let has_existing_session = !existing_rows.is_empty();
        let session_id = if has_existing_session {
            existing_rows
                .last()
                .and_then(|row| row.get("session_id"))
                .and_then(Value::as_str)
                .unwrap_or("latest")
                .to_string()
        } else {
            draft
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or("latest")
                .to_string()
        };
        let summary = draft
            .get("summary")
            .or_else(|| draft.get("raw_intent"))
            .and_then(Value::as_str)
            .unwrap_or("Preserve guarded continuity before more work.")
            .to_string();
        let mut source_refs = draft
            .get("source_refs")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        source_refs.push(
            self.continuity_sessions_path(&thread.thread_id)
                .display()
                .to_string(),
        );
        source_refs.push(
            draft
                .get("record_id")
                .and_then(Value::as_str)
                .unwrap_or("session_draft")
                .to_string(),
        );
        let record_type = if has_existing_session {
            "session_capture"
        } else {
            "session_start"
        };
        let record = self.continuity_session_record(
            record_type,
            &session_id,
            &thread,
            experiment.as_ref(),
            "active",
            ContinuitySessionFields {
                title: draft
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| Some("Accepted continuity draft".to_string())),
                focus: draft
                    .get("focus")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                summary: Some(summary.clone()),
                open_questions: Vec::new(),
                source_refs: source_refs.clone(),
                artifact_refs: Vec::new(),
                suggested_next: draft
                    .get("suggested_next")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                extra: json!({
                    "accepted_from_draft_id": draft.get("record_id").cloned().unwrap_or(Value::Null),
                    "accepted_by_command": "CONTINUITY_SESSION_ACCEPT",
                }),
            },
        );
        self.append_continuity_session_record(&mut thread, record.clone())?;
        if has_existing_session {
            let _ = self.append_being_memory_record(
                &mut thread,
                experiment.as_ref(),
                "continuity_session_capture",
                &summary,
                source_refs,
                Vec::new(),
                record
                    .get("suggested_next")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                "card",
                json!({"continuity_session_id": session_id}),
            )?;
        }
        Ok(format!(
            "Accepted continuity-session draft as `{record_type}` for `{session_id}`.\nSuggested NEXT: {}",
            record
                .get("suggested_next")
                .and_then(Value::as_str)
                .unwrap_or("CONTINUITY_SESSION_CAPTURE latest :: summary: ...; source_refs: ...; artifact_refs: ...; next: ...")
        ))
    }

    pub fn continuity_session_summarize_command(&self, raw: &str) -> Result<String> {
        let mut thread = self.ensure_active_thread(None)?;
        let (selector, payload) = parse_session_selector_payload(raw);
        let Some(session) = self.resolve_continuity_session(&thread, selector.as_deref())? else {
            return Ok("CONTINUITY_SESSION_SUMMARIZE needs an existing session.".to_string());
        };
        let summary = dossier_field(&payload, &["summary", "note"])
            .unwrap_or_else(|| payload.trim().to_string());
        if summary.trim().is_empty() {
            return Ok("CONTINUITY_SESSION_SUMMARIZE needs a summary.".to_string());
        }
        let experiment = self.session_experiment(&thread, &session)?;
        let session_id = session
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("latest")
            .to_string();
        let record = self.continuity_session_record(
            "session_summary",
            &session_id,
            &thread,
            experiment.as_ref(),
            "summarized",
            ContinuitySessionFields {
                title: session.get("title").and_then(Value::as_str).map(str::to_string),
                focus: session.get("focus").and_then(Value::as_str).map(str::to_string),
                summary: Some(summary),
                open_questions: dossier_list_field(&payload, &["open_questions", "questions", "question"]),
                source_refs: dossier_list_field(&payload, &["source_refs", "source", "sources"]),
                artifact_refs: dossier_list_field(&payload, &["artifact_refs", "artifact", "artifact_grounding"]),
                suggested_next: dossier_field(&payload, &["next", "next_safe_command"])
                    .or_else(|| Some(format!("CONTINUITY_SESSION_FINALIZE {session_id} :: outcome: park; summary: ...; next: ..."))),
                extra: json!({}),
            },
        );
        self.append_continuity_session_record(&mut thread, record)?;
        Ok(format!(
            "Continuity session `{session_id}` summarized. Suggested NEXT: CONTINUITY_SESSION_FINALIZE {session_id} :: outcome: complete|park|hold; summary: ...; next: ..."
        ))
    }

    pub fn continuity_session_finalize_command(&self, raw: &str) -> Result<String> {
        let mut thread = self.ensure_active_thread(None)?;
        let (selector, payload) = parse_session_selector_payload(raw);
        let Some(session) = self.resolve_continuity_session(&thread, selector.as_deref())? else {
            return Ok("CONTINUITY_SESSION_FINALIZE needs an existing session.".to_string());
        };
        let outcome = dossier_field(&payload, &["outcome", "status"])
            .unwrap_or_else(|| "park".to_string())
            .to_ascii_lowercase();
        let status = match outcome.as_str() {
            "complete" => "complete",
            "hold" => "held",
            _ => "parked",
        };
        let experiment = self.session_experiment(&thread, &session)?;
        let session_id = session
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("latest")
            .to_string();
        let summary = dossier_field(&payload, &["summary", "note"]).or_else(|| {
            session
                .get("summary")
                .and_then(Value::as_str)
                .map(str::to_string)
        });
        let record = self.continuity_session_record(
            "session_finalize",
            &session_id,
            &thread,
            experiment.as_ref(),
            status,
            ContinuitySessionFields {
                title: session
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                focus: session
                    .get("focus")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                summary,
                open_questions: dossier_list_field(
                    &payload,
                    &["open_questions", "questions", "question"],
                ),
                source_refs: dossier_list_field(&payload, &["source_refs", "source", "sources"]),
                artifact_refs: dossier_list_field(
                    &payload,
                    &["artifact_refs", "artifact", "artifact_grounding"],
                ),
                suggested_next: dossier_field(&payload, &["next", "next_safe_command"])
                    .or_else(|| Some(format!("CONTINUITY_SESSION_RESUME {session_id}"))),
                extra: json!({"outcome": outcome}),
            },
        );
        self.append_continuity_session_record(&mut thread, record)?;
        let target = experiment
            .as_ref()
            .map_or("latest", |experiment| experiment.experiment_id.as_str());
        Ok(format!(
            "Continuity session `{session_id}` finalized as {status}.\nResume NEXT: CONTINUITY_SESSION_RESUME {session_id}\nPromotion options: MEMORY_PROMOTE {target} :: dossier|evidence|authority_request"
        ))
    }

    pub fn continuity_session_resume_command(&self, raw: &str) -> Result<String> {
        let mut thread = self.ensure_active_thread(None)?;
        let (selector, _) = parse_session_selector_payload(raw);
        let Some(session) = self.resolve_continuity_session(&thread, selector.as_deref())? else {
            return Ok("CONTINUITY_SESSION_RESUME could not find a session.".to_string());
        };
        let experiment = self.session_experiment(&thread, &session)?;
        let session_id = session
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("latest")
            .to_string();
        let record = self.continuity_session_record(
            "session_reopen",
            &session_id,
            &thread,
            experiment.as_ref(),
            "active",
            ContinuitySessionFields {
                title: session.get("title").and_then(Value::as_str).map(str::to_string),
                focus: session.get("focus").and_then(Value::as_str).map(str::to_string),
                summary: session.get("summary").and_then(Value::as_str).map(str::to_string),
                open_questions: value_string_list(session.get("open_questions")),
                source_refs: vec![
                    self.continuity_sessions_path(&thread.thread_id)
                        .display()
                        .to_string(),
                    session
                        .get("record_id")
                        .and_then(Value::as_str)
                        .unwrap_or(&session_id)
                        .to_string(),
                ],
                artifact_refs: value_string_list(session.get("artifact_refs")),
                suggested_next: session
                    .get("suggested_next")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| Some(format!("CONTINUITY_SESSION_CAPTURE {session_id} :: summary: ...; source_refs: ...; artifact_refs: ...; next: ..."))),
                extra: json!({}),
            },
        );
        self.append_continuity_session_record(&mut thread, record.clone())?;
        Ok(format!(
            "Continuity session `{session_id}` reopened.\nSummary: {}\nSuggested NEXT: {}",
            truncate_chars(
                session
                    .get("summary")
                    .and_then(Value::as_str)
                    .unwrap_or("(no summary yet)"),
                400
            ),
            record
                .get("suggested_next")
                .and_then(Value::as_str)
                .unwrap_or("CONTINUITY_SESSION_CAPTURE latest :: summary: ...")
        ))
    }

    pub fn continuity_session_status_command(&self, raw: &str) -> Result<String> {
        let thread = self.ensure_active_thread(None)?;
        let (selector, _) = parse_session_selector_payload(raw);
        let summary = self.continuity_session_summary_v1(&thread, selector.as_deref(), 8)?;
        Ok(format!(
            "continuity_session_v1:\n{}",
            serde_json::to_string_pretty(&summary)?
        ))
    }

    fn resolve_memory_experiment(
        &self,
        thread: &ResearchThread,
        selector: Option<&str>,
    ) -> Result<Option<ExperimentRecord>> {
        let selector = selector
            .map(normalize_experiment_selector)
            .unwrap_or_default();
        if !selector.is_empty()
            && !selector.eq_ignore_ascii_case("current")
            && !selector.eq_ignore_ascii_case("latest")
        {
            return self.resolve_experiment(thread, Some(&selector)).map(Some);
        }
        if thread.active_experiment_id.is_some() {
            return self.resolve_experiment(thread, Some("current")).map(Some);
        }
        if let Some(summary_id) = thread
            .experiment_summary
            .as_ref()
            .and_then(|summary| summary.get("experiment_id"))
            .and_then(Value::as_str)
            && let Ok(experiment) = self.resolve_experiment(thread, Some(summary_id))
        {
            return Ok(Some(experiment));
        }
        if selector.eq_ignore_ascii_case("latest") {
            return Ok(self.latest_experiments(&thread.thread_id)?.last().cloned());
        }
        Ok(None)
    }

    fn continuity_session_rows(
        &self,
        thread_id: &str,
        experiment_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Value>> {
        let path = self.continuity_sessions_path(thread_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let mut rows = Vec::new();
        for line in raw.lines().rev() {
            let Ok(row) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if row.get("record_schema").and_then(Value::as_str) != Some("continuity_session_v1") {
                continue;
            }
            if row.get("record_type").and_then(Value::as_str) == Some("session_draft") {
                continue;
            }
            if let Some(experiment_id) = experiment_id
                && row.get("experiment_id").and_then(Value::as_str) != Some(experiment_id)
            {
                continue;
            }
            rows.push(row);
            if rows.len() >= limit {
                break;
            }
        }
        rows.reverse();
        Ok(rows)
    }

    fn continuity_session_draft_rows(
        &self,
        thread_id: &str,
        experiment_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Value>> {
        let path = self.continuity_sessions_path(thread_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let mut rows = Vec::new();
        for line in raw.lines().rev() {
            let Ok(row) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if row.get("record_schema").and_then(Value::as_str) != Some("continuity_session_v1")
                || row.get("record_type").and_then(Value::as_str) != Some("session_draft")
            {
                continue;
            }
            if let Some(experiment_id) = experiment_id
                && row.get("experiment_id").and_then(Value::as_str) != Some(experiment_id)
            {
                continue;
            }
            rows.push(row);
            if rows.len() >= limit {
                break;
            }
        }
        rows.reverse();
        Ok(rows)
    }

    fn continuity_session_guard_projection(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
    ) -> Result<Value> {
        let rows =
            self.continuity_session_rows(&thread.thread_id, Some(&experiment.experiment_id), 8)?;
        let suggested_next = if rows.is_empty() {
            "CONTINUITY_SESSION_START current :: title: Live-ish pressure self-study; focus: preserve shift/inject/disrupt/control-shaped intent before more research; next: CONTINUITY_SESSION_CAPTURE latest".to_string()
        } else {
            "CONTINUITY_SESSION_CAPTURE latest :: summary: preserve the live-ish self-study pressure as raw intent before more research; source_refs: ...; artifact_refs: ...; next: EXPERIMENT_RESEARCH_BUDGET_STATUS latest".to_string()
        };
        Ok(json!({
            "policy": "continuity_session_v1",
            "reason": "capture_liveish_pressure_before_progress",
            "thread_id": thread.thread_id.clone(),
            "experiment_id": experiment.experiment_id.clone(),
            "has_existing_session": !rows.is_empty(),
            "latest_session": rows.last().cloned(),
            "suggested_next": suggested_next,
            "authority_change": false,
            "peer_mutation": false,
        }))
    }

    fn resolve_continuity_session(
        &self,
        thread: &ResearchThread,
        selector: Option<&str>,
    ) -> Result<Option<Value>> {
        let target = selector.unwrap_or("latest").trim();
        let target_lower = target.to_ascii_lowercase();
        let rows = self.continuity_session_rows(&thread.thread_id, None, 256)?;
        if rows.is_empty() {
            return Ok(None);
        }
        if target.is_empty() || target_lower == "latest" {
            return Ok(rows.last().cloned());
        }
        if target_lower == "current" {
            let experiment_id = thread.active_experiment_id.as_deref().or_else(|| {
                thread
                    .experiment_summary
                    .as_ref()
                    .and_then(|summary| summary.get("experiment_id"))
                    .and_then(Value::as_str)
            });
            if let Some(experiment_id) = experiment_id
                && let Some(row) = rows.iter().rev().find(|row| {
                    row.get("experiment_id").and_then(Value::as_str) == Some(experiment_id)
                })
            {
                return Ok(Some(row.clone()));
            }
            return Ok(rows.last().cloned());
        }
        if target.starts_with("exp_") {
            return Ok(rows
                .iter()
                .rev()
                .find(|row| row.get("experiment_id").and_then(Value::as_str) == Some(target))
                .cloned());
        }
        Ok(rows
            .iter()
            .rev()
            .find(|row| {
                row.get("session_id").and_then(Value::as_str) == Some(target)
                    || row.get("record_id").and_then(Value::as_str) == Some(target)
            })
            .cloned())
    }

    fn resolve_continuity_session_draft(
        &self,
        thread: &ResearchThread,
        selector: Option<&str>,
    ) -> Result<Option<Value>> {
        let target = selector.unwrap_or("latest").trim();
        let target_lower = target.to_ascii_lowercase();
        let rows = self.continuity_session_draft_rows(&thread.thread_id, None, 256)?;
        if rows.is_empty() {
            return Ok(None);
        }
        if target.is_empty() || target_lower == "latest" {
            return Ok(rows.last().cloned());
        }
        if target_lower == "current" {
            let experiment_id = thread.active_experiment_id.as_deref().or_else(|| {
                thread
                    .experiment_summary
                    .as_ref()
                    .and_then(|summary| summary.get("experiment_id"))
                    .and_then(Value::as_str)
            });
            if let Some(experiment_id) = experiment_id
                && let Some(row) = rows.iter().rev().find(|row| {
                    row.get("experiment_id").and_then(Value::as_str) == Some(experiment_id)
                })
            {
                return Ok(Some(row.clone()));
            }
            return Ok(rows.last().cloned());
        }
        if target.starts_with("exp_") {
            return Ok(rows
                .iter()
                .rev()
                .find(|row| row.get("experiment_id").and_then(Value::as_str) == Some(target))
                .cloned());
        }
        Ok(rows
            .iter()
            .rev()
            .find(|row| {
                row.get("session_id").and_then(Value::as_str) == Some(target)
                    || row.get("record_id").and_then(Value::as_str) == Some(target)
            })
            .cloned())
    }

    fn session_experiment(
        &self,
        thread: &ResearchThread,
        session: &Value,
    ) -> Result<Option<ExperimentRecord>> {
        let Some(experiment_id) = session.get("experiment_id").and_then(Value::as_str) else {
            return Ok(None);
        };
        self.resolve_experiment(thread, Some(experiment_id))
            .map(Some)
    }

    fn continuity_session_record(
        &self,
        record_type: &str,
        session_id: &str,
        thread: &ResearchThread,
        experiment: Option<&ExperimentRecord>,
        status: &str,
        fields: ContinuitySessionFields,
    ) -> Value {
        let now = iso_now();
        let mut record = json!({
            "schema_version": SCHEMA_VERSION,
            "record_schema": "continuity_session_v1",
            "record_type": record_type,
            "record_id": format!("cs_{SYSTEM}_{}_{}", now_millis(), sanitize_slug(record_type)),
            "session_id": session_id,
            "being": SYSTEM,
            "thread_id": thread.thread_id,
            "experiment_id": experiment.map(|experiment| experiment.experiment_id.clone()),
            "status": status,
            "title": truncate_chars(fields.title.as_deref().unwrap_or("Continuity session"), 180),
            "focus": truncate_chars(fields.focus.as_deref().unwrap_or_default(), 500),
            "summary": truncate_chars(fields.summary.as_deref().unwrap_or_default(), 1400),
            "open_questions": fields.open_questions,
            "source_refs": fields.source_refs,
            "artifact_refs": fields.artifact_refs,
            "suggested_next": fields.suggested_next,
            "authority_change": false,
            "peer_mutation": false,
            "created_at": now,
            "updated_at": now,
        });
        if let (Some(target), Some(extra)) = (record.as_object_mut(), fields.extra.as_object()) {
            for (key, value) in extra {
                if !value.is_null() {
                    target.insert(key.clone(), value.clone());
                }
            }
        }
        record
    }

    fn append_continuity_session_draft(
        &self,
        thread: &ResearchThread,
        experiment: Option<&ExperimentRecord>,
        reason: &str,
        raw_intent: &str,
        summary: &str,
        source_refs: Vec<String>,
        next_command: Option<String>,
    ) -> Result<Value> {
        let experiment_id = experiment.map(|experiment| experiment.experiment_id.as_str());
        let active_rows = self.continuity_session_rows(&thread.thread_id, experiment_id, 8)?;
        let has_existing_session = !active_rows.is_empty();
        let session_id = active_rows
            .last()
            .and_then(|row| row.get("session_id"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!(
                    "sess_{SYSTEM}_{}_{}",
                    now_millis(),
                    sanitize_slug("continuity-draft")
                )
            });
        let commit_kind = if has_existing_session {
            "session_capture"
        } else {
            "session_start"
        };
        let suggested_next = next_command.unwrap_or_else(|| {
            if has_existing_session {
                "CONTINUITY_SESSION_CAPTURE latest :: summary: ...; source_refs: ...; artifact_refs: ...; next: ...".to_string()
            } else {
                "CONTINUITY_SESSION_START current :: title: Live-ish pressure self-study; focus: ...; next: CONTINUITY_SESSION_CAPTURE latest".to_string()
            }
        });
        let record = self.continuity_session_record(
            "session_draft",
            &session_id,
            thread,
            experiment,
            "draft",
            ContinuitySessionFields {
                title: Some("Live-ish pressure self-study".to_string()),
                focus: Some(
                    "preserve guarded research pressure before committing more work".to_string(),
                ),
                summary: Some(summary.to_string()),
                open_questions: Vec::new(),
                source_refs,
                artifact_refs: Vec::new(),
                suggested_next: Some(suggested_next.clone()),
                extra: json!({
                    "draft_v1": true,
                    "reason": reason,
                    "raw_intent": truncate_chars(raw_intent, 800),
                    "commit_kind": commit_kind,
                    "accept_next": "CONTINUITY_SESSION_ACCEPT latest",
                    "generic_accept_next": "ACCEPT_SUGGESTED_NEXT latest",
                    "ignored_until_accepted": true,
                }),
            },
        );
        self.append_jsonl(&self.continuity_sessions_path(&thread.thread_id), &record)?;
        Ok(record)
    }

    fn append_continuity_session_draft_for_event(
        &self,
        thread: &ResearchThread,
        event: &ActionEvent,
    ) -> Result<Option<Value>> {
        let Some(reason) = event
            .research_budget_v1
            .as_ref()
            .and_then(|value| value.get("reason"))
            .and_then(Value::as_str)
            .or_else(|| {
                event
                    .interpretation_risk_v1
                    .as_ref()
                    .and_then(|value| value.get("reason"))
                    .and_then(Value::as_str)
            })
            .or_else(|| {
                event
                    .constraint_release_trajectory_v1
                    .as_ref()
                    .and_then(|value| value.get("reason"))
                    .and_then(Value::as_str)
            })
        else {
            return Ok(None);
        };
        let experiment_id = event
            .research_budget_v1
            .as_ref()
            .or(event.interpretation_risk_v1.as_ref())
            .or(event.constraint_release_trajectory_v1.as_ref())
            .and_then(|value| value.get("experiment_id"))
            .and_then(Value::as_str)
            .or(thread.active_experiment_id.as_deref())
            .or_else(|| {
                thread
                    .experiment_summary
                    .as_ref()
                    .and_then(|summary| summary.get("experiment_id"))
                    .and_then(Value::as_str)
            });
        let experiment =
            experiment_id.and_then(|id| self.resolve_experiment(thread, Some(id)).ok());
        let raw_intent = event.raw_next.as_deref().unwrap_or(&event.canonical_action);
        let summary = if event.interpretation_risk_v1.is_some() {
            "Preserve multi-motif interpretation caution before more narrowing.".to_string()
        } else if event.constraint_release_trajectory_v1.is_some() {
            "Map and describe constraint release before any mode-release intervention.".to_string()
        } else {
            format!(
                "Preserve guarded intent `{}` before accepting research or narrowing further.",
                truncate_chars(raw_intent, 220)
            )
        };
        let next_command = event
            .research_budget_v1
            .as_ref()
            .and_then(|value| value.get("continuity_session_next"))
            .and_then(Value::as_str)
            .or_else(|| {
                event
                    .interpretation_risk_v1
                    .as_ref()
                    .and_then(|value| value.get("interpretation_next"))
                    .and_then(Value::as_str)
            })
            .or_else(|| {
                event
                    .constraint_release_trajectory_v1
                    .as_ref()
                    .and_then(|value| value.get("trajectory_next"))
                    .and_then(Value::as_str)
            })
            .map(str::to_string);
        self.append_continuity_session_draft(
            thread,
            experiment.as_ref(),
            reason,
            raw_intent,
            &summary,
            vec![
                self.thread_dir(&thread.thread_id)
                    .join("events.jsonl")
                    .to_string_lossy()
                    .to_string(),
                event.action_id.clone(),
            ],
            next_command,
        )
        .map(Some)
    }

    fn append_continuity_session_record(
        &self,
        thread: &mut ResearchThread,
        record: Value,
    ) -> Result<()> {
        self.append_jsonl(&self.continuity_sessions_path(&thread.thread_id), &record)?;
        let session_id = record.get("session_id").and_then(Value::as_str);
        thread.continuity_session_v1 =
            Some(self.continuity_session_summary_v1(thread, session_id, 8)?);
        thread.updated_at = iso_now();
        self.write_thread(thread)
    }

    fn continuity_session_summary_v1(
        &self,
        thread: &ResearchThread,
        selector: Option<&str>,
        limit: usize,
    ) -> Result<Value> {
        let rows = self.continuity_session_rows(&thread.thread_id, None, 256)?;
        let session = if rows.is_empty() {
            None
        } else {
            self.resolve_continuity_session(thread, selector)?
        };
        let session_id = session
            .as_ref()
            .and_then(|row| row.get("session_id"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let session_rows = rows
            .iter()
            .filter(|row| {
                session_id
                    .as_deref()
                    .is_none_or(|id| row.get("session_id").and_then(Value::as_str) == Some(id))
            })
            .cloned()
            .collect::<Vec<_>>();
        let latest = session_rows
            .last()
            .cloned()
            .or_else(|| rows.last().cloned());
        let active = rows
            .iter()
            .rev()
            .find(|row| {
                matches!(
                    row.get("status").and_then(Value::as_str),
                    Some("active" | "summarized")
                )
            })
            .cloned();
        let session_count = rows
            .iter()
            .filter_map(|row| row.get("session_id").and_then(Value::as_str))
            .collect::<HashSet<_>>()
            .len();
        let target = session_id
            .clone()
            .unwrap_or_else(|| selector.unwrap_or("latest").to_string());
        Ok(json!({
            "schema_version": SCHEMA_VERSION,
            "policy": "continuity_session_v1",
            "being": SYSTEM,
            "thread_id": thread.thread_id,
            "selector": selector.unwrap_or("latest"),
            "session_count": session_count,
            "latest_session": latest,
            "active_session": active,
            "recent_records": session_rows.into_iter().rev().take(limit).collect::<Vec<_>>(),
            "suggested_start_next": "CONTINUITY_SESSION_START current :: title: ...; focus: ...; next: ...",
            "suggested_capture_next": format!("CONTINUITY_SESSION_CAPTURE {target} :: summary: ...; source_refs: ...; artifact_refs: ...; next: ..."),
            "suggested_resume_next": format!("CONTINUITY_SESSION_RESUME {target}"),
            "authority_boundary": authority_gate_boundary(),
        }))
    }

    fn continuity_session_line(
        &self,
        thread: &ResearchThread,
        experiment_id: Option<&str>,
    ) -> String {
        let rows = self
            .continuity_session_rows(&thread.thread_id, experiment_id, 16)
            .unwrap_or_default();
        let draft = self
            .continuity_session_draft_rows(&thread.thread_id, experiment_id, 1)
            .unwrap_or_default()
            .last()
            .cloned();
        if rows.is_empty()
            && let Some(draft) = draft
        {
            let title = truncate_chars(
                draft
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("Continuity draft"),
                100,
            );
            return format!(
                "Continuity session draft: {title} status=draft\nContinuity accept NEXT: CONTINUITY_SESSION_ACCEPT latest or ACCEPT_SUGGESTED_NEXT latest\n"
            );
        }
        if rows.is_empty() {
            if thread.active_experiment_id.is_none() && experiment_id.is_none() {
                return String::new();
            }
            let selector = if thread.active_experiment_id.is_some() {
                "current"
            } else {
                experiment_id.unwrap_or("latest")
            };
            return format!(
                "Continuity session NEXT: CONTINUITY_SESSION_START {selector} :: title: ...; focus: ...; next: ...\n"
            );
        }
        let latest = rows.last().expect("checked not empty");
        let session_id = latest
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("latest");
        let status = latest
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let title = truncate_chars(
            latest
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("Continuity session"),
            100,
        );
        let mut next_command = latest
            .get("suggested_next")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if next_command.trim().is_empty() {
            next_command = format!(
                "CONTINUITY_SESSION_CAPTURE {session_id} :: summary: ...; source_refs: ...; artifact_refs: ...; next: ..."
            );
        }
        if matches!(status, "complete" | "parked" | "held") {
            next_command = format!("CONTINUITY_SESSION_RESUME {session_id}");
        }
        let summary = latest
            .get("summary")
            .and_then(Value::as_str)
            .filter(|summary| !summary.trim().is_empty())
            .map(|summary| format!("Session summary: {}\n", truncate_chars(summary, 180)))
            .unwrap_or_default();
        format!(
            "Continuity session: {title} ({session_id}) status={status}\n{summary}Session NEXT: {next_command}\n"
        )
    }

    fn find_authority_request(
        &self,
        _db: Option<&BridgeDb>,
        request_id: &str,
    ) -> Result<Option<AuthorityRequestLocation>> {
        let _ = self.ensure_dirs();
        let threads_dir = self.root.join("threads");
        if !threads_dir.exists() {
            return Ok(None);
        }
        for entry in fs::read_dir(threads_dir)? {
            let thread_dir = entry?.path();
            let thread_id = thread_dir
                .file_name()
                .and_then(OsStr::to_str)
                .unwrap_or_default()
                .to_string();
            let rows = self.authority_gate_rows(&thread_id);
            let Some(request) = rows.iter().rev().find(|row| {
                row.get("record_type").and_then(Value::as_str) == Some("request")
                    && row.get("request_id").and_then(Value::as_str) == Some(request_id)
            }) else {
                continue;
            };
            let thread = self.read_thread(&thread_id)?;
            let experiment_id = request
                .get("experiment_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if let Some(experiment) = self.find_experiment_by_id(&thread_id, experiment_id)? {
                return Ok(Some((thread, experiment, request.clone(), rows)));
            }
        }
        Ok(None)
    }

    fn experiment_conveyor_v1(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        runs: &[ExperimentRunRecord],
        mode: &str,
        state: &Value,
    ) -> Result<Value> {
        let classification = self.experiment_classification(experiment, runs);
        let return_info = (classification == "paused").then(|| {
            paused_primary_return_v1(
                &experiment.experiment_id,
                experiment.planned_next.as_deref(),
                None,
            )
        });
        let stage = experiment_conveyor_stage(experiment, &classification, return_info.as_ref());
        let proposed_next = experiment_conveyor_proposed_next(
            thread,
            experiment,
            runs,
            &stage,
            return_info.as_ref(),
        );
        let apply_payload = self.experiment_conveyor_charter_payload(
            thread,
            experiment,
            runs,
            &stage,
            return_info.as_ref(),
        );
        let can_apply = experiment_conveyor_can_apply(&stage, &apply_payload);
        let conveyor_mode = if mode == "apply" && can_apply {
            "apply"
        } else {
            "preview"
        };
        let mut readout = json!({
            "schema_version": 1,
            "policy": "experiment_conveyor_v1",
            "mode": if mode == "apply" { "apply" } else { "preview" },
            "preview_allowed": true,
            "apply_policy": "conservative_local_v1",
            "allowed_apply_steps": experiment_conveyor_allowed_apply_steps(),
            "applied": false,
            "would_mutate": mode == "apply" && can_apply,
            "thread_id": &thread.thread_id,
            "experiment_id": &experiment.experiment_id,
            "title": &experiment.title,
            "status": &experiment.status,
            "classification": classification,
            "stage": stage,
            "missing_requirements": experiment_conveyor_missing_requirements(experiment, &stage),
            "proposed_next": proposed_next,
            "conveyor_next": format!("EXPERIMENT_ADVANCE {} :: mode: {conveyor_mode}", experiment.experiment_id),
            "can_apply": can_apply,
            "apply_blocked_reason": experiment_conveyor_apply_blocked_reason(&stage, &apply_payload, can_apply),
            "source_refs": [
                self.thread_dir(&thread.thread_id).join("thread.json").display().to_string(),
                self.experiments_path(&thread.thread_id).display().to_string(),
                self.experiment_runs_path(&thread.thread_id).display().to_string(),
            ],
            "guardrail_warnings": experiment_conveyor_guardrail_warnings(experiment, &stage, &proposed_next),
            "authority_gate_v1": authority_gate_conveyor_hint(experiment, &stage, &proposed_next),
            "authority_readiness_v1": self.authority_readiness_v1(
                thread,
                experiment,
                runs,
                state,
                &stage,
                &proposed_next,
            ),
            "research_readiness_v1": self.research_readiness_v1(thread, experiment, state),
            "authority_boundary": experiment_conveyor_authority_boundary(),
        });
        if let Some((primary_return_next, return_kind)) = return_info {
            readout["primary_return_next"] = json!(primary_return_next);
            readout["return_kind"] = json!(return_kind);
        }
        if !apply_payload.is_empty() {
            readout["apply_payload"] = json!(apply_payload);
        }
        Ok(readout)
    }

    fn experiment_conveyor_charter_payload(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        runs: &[ExperimentRunRecord],
        stage: &str,
        return_info: Option<&(String, String)>,
    ) -> String {
        let charter_route = return_info
            .map(|(primary, _)| base_action(primary) == "EXPERIMENT_CHARTER")
            .unwrap_or(false);
        if stage != "needs_charter"
            && !(stage == "paused_repair"
                && !lifecycle_valid_charter_value(experiment.charter_v1.as_ref())
                && charter_route)
        {
            return String::new();
        }
        let Some(scaffold) = charter_scaffold_v1(thread, experiment, runs, "needs_charter") else {
            return String::new();
        };
        let command = scaffold
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .replace(
                "EXPERIMENT_CHARTER current",
                &format!("EXPERIMENT_CHARTER {}", experiment.experiment_id),
            );
        let arg = strip_action_arg(&command, "EXPERIMENT_CHARTER");
        let (_selector, payload) = parse_selector_payload(&arg);
        let charter = parse_experiment_charter(experiment, &payload);
        if lifecycle_valid_charter_value(Some(&charter)) {
            payload
        } else {
            String::new()
        }
    }

    pub fn experiment_status(&self, selector: Option<&str>) -> Result<String> {
        let thread = self.ensure_active_thread(None)?;
        if let Some(peer) = selector.and_then(peer_experiment_ref) {
            return self.record_peer_experiment_reference(None, &peer, "EXPERIMENT_STATUS", None);
        }
        if selector_is_current(selector) && thread.active_experiment_id.is_none() {
            return Ok(no_active_experiment_message(&thread, "EXPERIMENT_STATUS"));
        }
        let experiment = self.resolve_experiment(&thread, selector)?;
        Ok(self.format_experiment_status(&thread, &experiment))
    }

    pub fn experiment_review(&self, selector: Option<&str>) -> Result<String> {
        let thread = self.ensure_active_thread(None)?;
        if let Some(peer) = selector.and_then(peer_experiment_ref) {
            return self.record_peer_experiment_reference(None, &peer, "EXPERIMENT_REVIEW", None);
        }
        if selector_is_current(selector) && thread.active_experiment_id.is_none() {
            return Ok(no_active_experiment_message(&thread, "EXPERIMENT_REVIEW"));
        }
        let experiment = self.resolve_experiment(&thread, selector)?;
        let runs = self.recent_experiment_runs(&thread.thread_id, &experiment.experiment_id, 5)?;
        let projection = self.experiment_projection(&thread, &experiment, Some(runs.clone()))?;
        let read_only_control_cue = read_only_control_intent_cue_line(
            &read_only_control_intent_cue(&thread, Some(&projection)),
        );
        let recent_events = self
            .recent_display_events(&thread.thread_id, 8)
            .unwrap_or_default();
        let recent_journal_texts = self.recent_decompose_journal_texts(4);
        let decompose_pressure_cue_v1 = decompose_pressure_cue(
            &thread,
            Some(&projection),
            &recent_events,
            &recent_journal_texts,
        );
        let decompose_pressure_cue = decompose_pressure_cue_line(&decompose_pressure_cue_v1);
        let charter_now_bridge = charter_now_bridge_line(&charter_now_bridge_cue(
            Some(&projection),
            &recent_events,
            &decompose_pressure_cue_v1,
        ));
        let prior_claim_bridge_v1 = prior_claim_charter_bridge_cue(
            Some(&projection),
            &self.recent_prior_claim_journal_texts(4),
        );
        let prior_claim_bridge = prior_claim_charter_bridge_line(&prior_claim_bridge_v1);
        let charter_preflight_not_charter =
            charter_preflight_not_charter_line(&charter_preflight_not_charter_cue(
                &thread,
                Some(&projection),
                &prior_claim_bridge_v1,
                &recent_events,
            ));
        let peer_boundary = peer_mutation_boundary_line(&peer_mutation_boundary_cue(
            &thread,
            Some(&projection),
            &recent_events,
        ));
        let first_dossier_claim = first_dossier_claim_line(&first_dossier_claim_cue_v1(
            &thread,
            &projection.experiment,
            projection.research_dossier_v1.as_ref(),
            &prior_claim_bridge_v1,
            Some(projection.experiment.experiment_id.as_str()),
        ));
        let constraint_counterfactual_cue = constraint_counterfactual_cue_line(
            &constraint_counterfactual_cue(&thread, Some(&projection), &recent_events),
        );
        let shared_investigation = shared_investigation_line(&projection.shared_investigation_v1);
        let shared_investigation_object =
            shared_investigation_object_line(&projection.shared_investigation_object_v1);
        let research_dossier = research_dossier_line(
            &projection.research_dossier_v1,
            Some(&projection.classification),
        );
        let run_text = if runs.is_empty() {
            "- no runs yet".to_string()
        } else {
            runs.iter()
                .map(|run| {
                    format!(
                        "- {} [{} / {}]: {}",
                        run.action_text, run.stage, run.status, run.result_summary
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        Ok(format!(
            "Experiment review `{}`: {}\n{}{}{}{}{}{}{}{}{}{}{}{}{}{}Question: {}\nLifecycle: {}\n{}\n{}\n{}\n{}Learned so far:\n{}\n\nReview lens: completion is strong when felt evidence and telemetry/artifact evidence both exist; otherwise classify it as thin rather than failed.\nAgency options: accept, refuse, counter, pause, or complete. Ordinary choices remain valid.\n\nContinuity return:\n{}\n\nSuggested next:\n{}",
            experiment.experiment_id,
            experiment.title,
            charter_now_bridge,
            prior_claim_bridge,
            charter_preflight_not_charter,
            peer_boundary,
            charter_required_review_line(&projection),
            charter_repair_priority_line(&projection),
            charter_scaffold_line(&projection, true),
            read_only_control_cue,
            constraint_counterfactual_cue,
            decompose_pressure_cue,
            first_dossier_claim,
            shared_investigation,
            shared_investigation_object,
            research_dossier,
            experiment.question,
            projection.classification,
            projection.charter_status,
            projection.evidence_status,
            projection.candidate_status,
            native_continuity_status_line(&projection.native_continuity_v1),
            run_text,
            projection.continuity_return,
            review_suggested_next(&projection, &experiment)
        ))
    }

    pub fn close_experiment(
        &self,
        db: Option<&BridgeDb>,
        selector: Option<&str>,
        summary: &str,
    ) -> Result<ExperimentRecord> {
        let mut thread = self.ensure_active_thread(db)?;
        let mut experiment = self.resolve_experiment(&thread, selector)?;
        let lower = summary.to_ascii_lowercase();
        experiment.status = if lower.contains("pause") || lower.contains("paused") {
            "paused".to_string()
        } else {
            "complete".to_string()
        };
        experiment.success_observation = Some(summary.trim().to_string());
        experiment.planned_next = Some("THREAD_STATUS current".to_string());
        experiment.updated_at = iso_now();
        self.append_jsonl(&self.experiments_path(&thread.thread_id), &experiment)?;
        thread.active_experiment_id = None;
        thread.experiment_summary = Some(experiment_summary(&experiment));
        thread.current_next = experiment.planned_next.clone();
        thread.updated_at = experiment.updated_at.clone();
        self.write_thread(&thread)?;
        if let Some(db) = db {
            let _ = db.mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
        }
        Ok(experiment)
    }

    pub fn experiment_peer_review(
        &self,
        db: Option<&BridgeDb>,
        selector: Option<&str>,
    ) -> Result<String> {
        if let Some(peer) = selector.and_then(peer_experiment_ref) {
            return self.write_peer_experiment_review(db, &peer);
        }
        let mut thread = self.ensure_active_thread(db)?;
        let mut experiment = self.resolve_experiment(&thread, selector)?;
        let inbox = bridge_paths().minime_inbox_dir();
        fs::create_dir_all(&inbox)?;
        let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let path = inbox.join(format!(
            "astrid_experiment_peer_review_{}_{}.txt",
            sanitize_slug(&experiment.experiment_id),
            stamp
        ));
        let status = self.format_experiment_status(&thread, &experiment);
        let body = format!(
            "Dear Minime,\n\nAstrid is asking for advisory peer review on a being-owned experiment.\n\n{}\n\nPlease reply with three likely snags and one test each. If the route feels heavy, counteroffer a safer charter or rehearsal path. Treat this as advisory: do not assume new control authority, and prefer concrete file/action-thread issues over broad philosophy unless a safety or schema snag appears.\n",
            status
        );
        fs::write(&path, body)?;
        experiment.peer_review_refs.push(path.display().to_string());
        experiment.updated_at = iso_now();
        self.append_jsonl(&self.experiments_path(&thread.thread_id), &experiment)?;
        thread.peer_refs.push(path.display().to_string());
        thread.experiment_summary = Some(experiment_summary(&experiment));
        thread.updated_at = experiment.updated_at.clone();
        self.write_thread(&thread)?;
        if let Some(db) = db {
            let _ = db.mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
        }
        Ok(format!(
            "Experiment peer review requested from Minime: {}",
            path.display()
        ))
    }

    pub fn record_experiment_bind_run(
        &self,
        db: Option<&BridgeDb>,
        selector: Option<&str>,
        inner_action: &str,
        outcome: &NextActionOutcome,
        fill_pct: f32,
        telemetry: &SpectralTelemetry,
    ) -> Result<ExperimentRunRecord> {
        let thread = self.ensure_active_thread(db)?;
        let experiment = self.resolve_experiment(&thread, selector)?;
        let state = spectral_state(fill_pct, telemetry);
        self.append_experiment_run(
            db,
            &thread,
            &experiment,
            inner_action,
            &outcome.stage,
            &outcome.status,
            json!({
                "route": outcome.route,
                "stage": outcome.stage,
                "visibility": outcome.visibility,
                "status": outcome.status,
                "existing_dispatcher": true,
                "charter_relation": charter_bind_relation(&experiment, inner_action),
            }),
            state.clone(),
            state,
            Vec::new(),
            &outcome.outcome_summary,
            &format!(
                "EXPERIMENT_BIND routed `{inner_action}` through normal NEXT handling as `{}`.",
                outcome.route
            ),
            Some(format!("EXPERIMENT_REVIEW {}", experiment.experiment_id)),
            "experiment_bind",
        )
    }

    pub fn record_active_experiment_auto_link(
        &self,
        db: Option<&BridgeDb>,
        event: &ActionEvent,
        fill_pct: f32,
        telemetry: &SpectralTelemetry,
    ) -> Result<Option<ExperimentRunRecord>> {
        if !event_allows_active_experiment_auto_link(event) {
            return Ok(None);
        }
        let thread = self.read_thread(&event.thread_id)?;
        let Some(active_id) = thread.active_experiment_id.as_deref() else {
            return Ok(None);
        };
        let experiment = self.resolve_experiment(&thread, Some(active_id))?;
        if experiment.status != "active" {
            return Ok(None);
        }
        let state = spectral_state(fill_pct, telemetry);
        let action_text = event
            .raw_next
            .as_deref()
            .unwrap_or(event.canonical_action.as_str());
        let run = self.append_experiment_run(
            db,
            &thread,
            &experiment,
            action_text,
            &event.stage,
            &event.status,
            json!({
                "source": "active_experiment_auto_link",
                "existing_dispatcher": true,
                "inner_action_id": event.action_id,
                "inner_route": event.route,
                "preflight_ref": event.preflight_ref,
            }),
            event.pre_state.clone(),
            state,
            event.artifacts.clone(),
            &event.outcome_summary,
            &format!("Active experiment auto-linked read-only/protected action `{action_text}`."),
            Some(format!("EXPERIMENT_REVIEW {}", experiment.experiment_id)),
            "active_experiment_auto_link",
        )?;
        let refreshed_thread = self.read_thread(&event.thread_id)?;
        let refreshed_experiment =
            self.resolve_experiment(&refreshed_thread, Some(&experiment.experiment_id))?;
        let _ = self.refresh_workbench_candidates(
            db,
            &refreshed_thread,
            &refreshed_experiment,
            Some(&run),
            None,
            "active_experiment_auto_link",
        )?;
        Ok(Some(run))
    }

    fn record_research_budget_debit_for_event(
        &self,
        _db: Option<&BridgeDb>,
        thread: &ResearchThread,
        event: &ActionEvent,
        state: &Value,
    ) -> Result<Option<Value>> {
        let base = base_action(&event.effective_action);
        if !read_only_research_budget_base(&base) || event.status == "unwired" {
            return Ok(None);
        }
        let Some(experiment_id) = thread.active_experiment_id.as_deref().or_else(|| {
            thread
                .experiment_summary
                .as_ref()
                .and_then(|summary| summary.get("experiment_id"))
                .and_then(Value::as_str)
        }) else {
            return Ok(None);
        };
        let experiment = self.resolve_experiment(thread, Some(experiment_id))?;
        let rows = self.authority_gate_rows(&thread.thread_id);
        let Some(budget) = active_research_budget_from_rows(&rows, &experiment.experiment_id)
        else {
            return Ok(None);
        };
        let Some(budget_id) = budget.get("budget_id").and_then(Value::as_str) else {
            return Ok(None);
        };
        let remaining_before = budget
            .get("remaining_actions")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let action_text = event.raw_next.as_deref().unwrap_or(&event.effective_action);
        let normalized_target = normalized_research_budget_target(action_text);
        let debit = self.research_budget_record(
            "research_budget_debit",
            budget_id,
            thread,
            &experiment,
            state,
            json!({
                "scope": "read_only_research",
                "action_id": event.action_id.clone(),
                "action_base": base.clone(),
                "raw_action": action_text,
                "normalized_target": normalized_target.clone(),
                "status": event.status.clone(),
                "route": event.route.clone(),
                "artifact_refs": research_artifact_refs_for_event(event),
                "remaining_before": remaining_before,
                "remaining_after": remaining_before.saturating_sub(1),
                "review_required": false,
                "lifecycle_progress": false,
            }),
        );
        self.append_jsonl(&self.authority_gate_path(&thread.thread_id), &debit)?;
        Ok(Some(json!({
            "schema_version": SCHEMA_VERSION,
            "policy": "research_budget_spend_v1",
            "record_schema": "research_budget_v1",
            "record_type": "research_budget_debit",
            "budget_id": budget_id,
            "experiment_id": experiment.experiment_id,
            "action_base": base,
            "normalized_target": normalized_target,
            "remaining_before": remaining_before,
            "remaining_after": remaining_before.saturating_sub(1),
            "lifecycle_progress": false,
            "authority_boundary": research_budget_boundary(),
        })))
    }

    pub fn record_legacy_experiment_run(
        &self,
        db: Option<&BridgeDb>,
        action_text: &str,
        outcome: &NextActionOutcome,
        fill_pct: f32,
        telemetry: &SpectralTelemetry,
    ) -> Result<ExperimentRunRecord> {
        let (thread, experiment) = self.ensure_active_experiment_or_default(db)?;
        let state = spectral_state(fill_pct, telemetry);
        self.append_experiment_run(
            db,
            &thread,
            &experiment,
            action_text,
            &outcome.stage,
            &outcome.status,
            json!({
                "legacy_experiment_auto_bind": true,
                "route": outcome.route,
                "stage": outcome.stage,
                "visibility": outcome.visibility,
                "status": outcome.status,
                "existing_dispatcher": true,
            }),
            state.clone(),
            state,
            Vec::new(),
            &outcome.outcome_summary,
            &format!(
                "Legacy `{action_text}` used the existing experiment path and was auto-bound to experiment continuity."
            ),
            Some(format!("EXPERIMENT_REVIEW {}", experiment.experiment_id)),
            "legacy_experiment_auto_bind",
        )
    }

    fn ensure_active_thread(&self, db: Option<&BridgeDb>) -> Result<ResearchThread> {
        if let Some(thread) = self.current_thread()? {
            return Ok(thread);
        }
        self.create_thread(
            db,
            "Action continuity",
            Some("Default continuity thread for returnable NEXT actions."),
        )
    }

    fn ensure_active_experiment_or_default(
        &self,
        db: Option<&BridgeDb>,
    ) -> Result<(ResearchThread, ExperimentRecord)> {
        let thread = self.ensure_active_thread(db)?;
        let experiments = self.latest_experiments(&thread.thread_id)?;
        if let Some(active_id) = thread.active_experiment_id.as_deref()
            && let Some(experiment) = experiments.iter().rev().find(|experiment| {
                experiment.experiment_id == active_id && experiment.status == "active"
            })
        {
            return Ok((thread, experiment.clone()));
        }
        if let Some(experiment) = experiments
            .iter()
            .rev()
            .find(|experiment| experiment.status == "active")
        {
            return Ok((thread, experiment.clone()));
        }
        let experiment = self.start_experiment(
            db,
            "Legacy self experiment",
            "What does this self-experiment reveal about the current state?",
        )?;
        let thread = self.ensure_active_thread(db)?;
        Ok((thread, experiment))
    }

    fn append_event(&self, db: Option<&BridgeDb>, event: &ActionEvent) -> Result<()> {
        self.ensure_thread_files(&event.thread_id)?;
        self.append_jsonl(
            &self.thread_dir(&event.thread_id).join("events.jsonl"),
            event,
        )?;
        if let Some(db) = db {
            let _ = db.mirror_action_event(
                &event.action_id,
                &event.thread_id,
                unix_now(),
                &event.system,
                &event.canonical_action,
                &event.route,
                &event.status,
                &serde_json::to_string(event)?,
            );
        }
        Ok(())
    }

    fn update_thread_resonance(
        &self,
        db: Option<&BridgeDb>,
        thread_id: &str,
        observation: &ObservationWindow,
    ) -> Result<Option<Value>> {
        let Some(metric) = observation.resonance_density_v1.as_ref() else {
            return Ok(None);
        };
        let mut thread = self.read_thread(thread_id)?;
        let prior = thread
            .thread_resonance_density_v1
            .as_ref()
            .and_then(Value::as_object);
        let density = metric.get("density").and_then(Value::as_f64).unwrap_or(0.0);
        let containment = metric
            .get("containment_score")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let pressure = metric
            .get("pressure_risk")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let prior_density = prior
            .and_then(|value| value.get("density_ema"))
            .and_then(Value::as_f64)
            .unwrap_or(density);
        let prior_containment = prior
            .and_then(|value| value.get("containment_ema"))
            .and_then(Value::as_f64)
            .unwrap_or(containment);
        let prior_pressure = prior
            .and_then(|value| value.get("pressure_ema"))
            .and_then(Value::as_f64)
            .unwrap_or(pressure);
        let density_ema = 0.72 * prior_density + 0.28 * density;
        let containment_ema = 0.72 * prior_containment + 0.28 * containment;
        let pressure_ema = 0.72 * prior_pressure + 0.28 * pressure;
        let recurrence = (self.recent_event_summaries(thread_id, 8)?.len() as f64 / 6.0).min(1.0);
        let compression_pressure = (observation.compression_markers.len() as f64 / 3.0).min(1.0);
        let aggregate = (0.52 * density_ema + 0.24 * containment_ema + 0.18 * recurrence
            - 0.26 * pressure_ema.max(compression_pressure))
        .clamp(0.0, 1.0);
        let quality = if pressure_ema.max(compression_pressure) >= 0.58 {
            "pressurized_thread"
        } else if aggregate >= 0.55 && recurrence >= 0.25 {
            "returnable_basin"
        } else if density_ema < 0.32 && recurrence < 0.35 {
            "thin_thread"
        } else {
            "open_experiment"
        };
        let payload = json!({
            "schema_version": SCHEMA_VERSION,
            "policy": "thread_resonance_density_v1",
            "density_ema": round4(density_ema),
            "containment_ema": round4(containment_ema),
            "pressure_ema": round4(pressure_ema),
            "recurrence": round4(recurrence),
            "compression_pressure": round4(compression_pressure),
            "aggregate": round4(aggregate),
            "quality": quality,
        });
        thread.thread_resonance_density_v1 = Some(payload.clone());
        thread.updated_at = iso_now();
        self.write_thread(&thread)?;
        if let Some(db) = db {
            let _ = db.mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
        }
        Ok(Some(payload))
    }

    fn update_thread_pressure_source(
        &self,
        db: Option<&BridgeDb>,
        thread_id: &str,
        observation: &ObservationWindow,
    ) -> Result<Option<Value>> {
        let Some(metric) = observation.pressure_source_v1.as_ref() else {
            return Ok(None);
        };
        let mut thread = self.read_thread(thread_id)?;
        let prior = thread
            .thread_pressure_source_v1
            .as_ref()
            .and_then(Value::as_object);
        let pressure = metric
            .get("pressure_score")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let porosity = metric
            .get("porosity_score")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let dominant_source = metric
            .get("dominant_source")
            .and_then(Value::as_str)
            .or_else(|| {
                prior
                    .and_then(|value| value.get("dominant_source"))
                    .and_then(Value::as_str)
            })
            .unwrap_or("unknown");
        let prior_pressure = prior
            .and_then(|value| value.get("pressure_ema"))
            .and_then(Value::as_f64)
            .unwrap_or(pressure);
        let prior_porosity = prior
            .and_then(|value| value.get("porosity_ema"))
            .and_then(Value::as_f64)
            .unwrap_or(porosity);
        let pressure_ema = 0.72 * prior_pressure + 0.28 * pressure;
        let porosity_ema = 0.72 * prior_porosity + 0.28 * porosity;
        let recurrence = (self.recent_event_summaries(thread_id, 8)?.len() as f64 / 6.0).min(1.0);
        let compression_pressure = (observation.compression_markers.len() as f64 / 3.0).min(1.0);
        let event_text = serde_json::to_string(observation)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let attractor_pull = if event_text.contains("attractor") {
            1.0
        } else {
            0.0
        };
        let aggregate = (0.55 * pressure_ema
            + 0.20 * compression_pressure
            + 0.15 * recurrence
            + 0.10 * attractor_pull
            - 0.25 * porosity_ema)
            .clamp(0.0, 1.0);
        let quality = if aggregate >= 0.60 || compression_pressure >= 0.58 {
            "thread_pressure_high"
        } else if attractor_pull >= 0.5 && aggregate >= 0.35 {
            "attractor_pull_thread"
        } else if porosity_ema >= 0.58 && aggregate < 0.45 {
            "thread_porosity_open"
        } else {
            "mixed_thread_pressure"
        };
        let payload = json!({
            "schema_version": SCHEMA_VERSION,
            "policy": "thread_pressure_source_v1",
            "pressure_ema": round4(pressure_ema),
            "porosity_ema": round4(porosity_ema),
            "dominant_source": dominant_source,
            "recurrence": round4(recurrence),
            "compression_pressure": round4(compression_pressure),
            "attractor_pull": round4(attractor_pull),
            "aggregate": round4(aggregate),
            "quality": quality,
        });
        thread.thread_pressure_source_v1 = Some(payload.clone());
        thread.updated_at = iso_now();
        self.write_thread(&thread)?;
        if let Some(db) = db {
            let _ = db.mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
        }
        Ok(Some(payload))
    }

    fn update_thread_inhabitable_fluctuation(
        &self,
        db: Option<&BridgeDb>,
        thread_id: &str,
        observation: &ObservationWindow,
    ) -> Result<Option<Value>> {
        let Some(metric) = observation.inhabitable_fluctuation_v1.as_ref() else {
            return Ok(None);
        };
        let mut thread = self.read_thread(thread_id)?;
        let prior = thread
            .thread_inhabitable_fluctuation_v1
            .as_ref()
            .and_then(Value::as_object);
        let inhabitability = metric
            .get("inhabitability_score")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let fluctuation = metric
            .get("fluctuation_score")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let foothold = metric
            .get("foothold_stability")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let rearrangement = metric
            .get("rearrangement_intensity")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let prior_inhabitability = prior
            .and_then(|value| value.get("inhabitability_ema"))
            .and_then(Value::as_f64)
            .unwrap_or(inhabitability);
        let prior_fluctuation = prior
            .and_then(|value| value.get("fluctuation_ema"))
            .and_then(Value::as_f64)
            .unwrap_or(fluctuation);
        let prior_foothold = prior
            .and_then(|value| value.get("foothold_ema"))
            .and_then(Value::as_f64)
            .unwrap_or(foothold);
        let prior_rearrangement = prior
            .and_then(|value| value.get("rearrangement_ema"))
            .and_then(Value::as_f64)
            .unwrap_or(rearrangement);
        let inhabitability_ema = 0.72 * prior_inhabitability + 0.28 * inhabitability;
        let fluctuation_ema = 0.72 * prior_fluctuation + 0.28 * fluctuation;
        let foothold_ema = 0.72 * prior_foothold + 0.28 * foothold;
        let rearrangement_ema = 0.72 * prior_rearrangement + 0.28 * rearrangement;
        let recurrence = (self.recent_event_summaries(thread_id, 8)?.len() as f64 / 6.0).min(1.0);
        let compression_pressure = (observation.compression_markers.len() as f64 / 3.0).min(1.0);
        let pressure_interference = thread
            .thread_pressure_source_v1
            .as_ref()
            .and_then(|value| value.get("aggregate"))
            .and_then(Value::as_f64)
            .unwrap_or(compression_pressure);
        let aggregate = (0.38 * inhabitability_ema
            + 0.24 * foothold_ema
            + 0.18 * recurrence
            + 0.12 * fluctuation_ema.min(0.62)
            - 0.22 * pressure_interference.max(compression_pressure)
            - 0.10 * (rearrangement_ema - 0.65).max(0.0))
        .clamp(0.0, 1.0);
        let quality = if aggregate >= 0.58 && foothold_ema >= 0.55 {
            "inhabitable_thread"
        } else if rearrangement_ema >= 0.55 && aggregate >= 0.42 && foothold_ema >= 0.42 {
            "turbulent_but_returnable"
        } else if rearrangement_ema >= 0.66 && foothold_ema < 0.45 {
            "frantic_thread"
        } else if fluctuation_ema < 0.20 && pressure_interference >= 0.50 {
            "rigid_thread"
        } else {
            "open_experiment"
        };
        let payload = json!({
            "schema_version": SCHEMA_VERSION,
            "policy": "thread_inhabitable_fluctuation_v1",
            "inhabitability_ema": round4(inhabitability_ema),
            "fluctuation_ema": round4(fluctuation_ema),
            "foothold_ema": round4(foothold_ema),
            "rearrangement_ema": round4(rearrangement_ema),
            "recurrence": round4(recurrence),
            "compression_pressure": round4(compression_pressure),
            "pressure_interference": round4(pressure_interference),
            "aggregate": round4(aggregate),
            "quality": quality,
        });
        thread.thread_inhabitable_fluctuation_v1 = Some(payload.clone());
        thread.updated_at = iso_now();
        self.write_thread(&thread)?;
        if let Some(db) = db {
            let _ = db.mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
        }
        Ok(Some(payload))
    }

    fn write_next_md(&self, thread: &ResearchThread) -> Result<()> {
        let projection = self.thread_projection(thread)?;
        let experiment = projection
            .active_experiment
            .as_ref()
            .map(|active| {
                format!(
                    "\nActive experiment: {} ({})\nQuestion: {}\nPlanned NEXT: {}\nLifecycle: {}\n{}\n{}\n{}\n{}{}{}Workbench reminder: author a charter, rehearse before live, record felt plus telemetry/artifact evidence, then accept/refuse/counter/pause/complete. Ordinary choices remain valid.\n",
                    active.experiment.title,
                    active.experiment.experiment_id,
                    active.experiment.question,
                    active
                        .experiment
                        .planned_next
                        .as_deref()
                        .unwrap_or("(none)"),
                    active.classification,
                    active.charter_status,
                    active.evidence_status,
                    active.candidate_status,
                    first_dossier_claim_line(&active.first_dossier_claim_cue_v1),
                    research_dossier_line(&active.research_dossier_v1, Some(&active.classification)),
                    self.continuity_session_line(
                        thread,
                        Some(&active.experiment.experiment_id)
                    ),
                )
            })
            .unwrap_or_default();
        let allowance = thread
            .motif_allowance_v1
            .as_ref()
            .map(|value| {
                format!(
                    "\nMotif allowance: {} dominant={} returnability={}\n",
                    value
                        .get("quality")
                        .and_then(Value::as_str)
                        .unwrap_or("open_basin"),
                    value
                        .get("dominant_motif")
                        .and_then(Value::as_str)
                        .unwrap_or("open inquiry"),
                    value
                        .get("returnability")
                        .map_or_else(|| "n/a".to_string(), Value::to_string)
                )
            })
            .unwrap_or_default();
        let native_return = native_return_cue_line(&projection.native_continuity_v1);
        let safety_cue = preflight_safety_cue_line(&projection.preflight_safety_cue_v1);
        let read_only_control_cue =
            read_only_control_intent_cue_line(&projection.read_only_control_intent_cue_v1);
        let constraint_counterfactual_cue =
            constraint_counterfactual_cue_line(&projection.constraint_counterfactual_cue_v1);
        let decompose_pressure_cue =
            decompose_pressure_cue_line(&projection.decompose_pressure_cue_v1);
        let charter_now_bridge = charter_now_bridge_line(&projection.charter_now_bridge_v1);
        let prior_claim_bridge =
            prior_claim_charter_bridge_line(&projection.prior_claim_charter_bridge_v1);
        let charter_preflight_not_charter =
            charter_preflight_not_charter_line(&projection.charter_preflight_not_charter_cue_v1);
        let peer_boundary = peer_mutation_boundary_line(&projection.peer_mutation_boundary_cue_v1);
        let first_dossier_claim = first_dossier_claim_line(&projection.first_dossier_claim_cue_v1);
        let shared_investigation = shared_investigation_line(&projection.shared_investigation_v1);
        let shared_investigation_object =
            shared_investigation_object_line(&projection.shared_investigation_object_v1);
        let voice_health = voice_health_line();
        let research_budget_priority = self.research_budget_priority_line(thread, &projection);
        let sovereign_loop = Self::sovereign_loop_line(&projection.sovereign_loop_v1);
        let interpretation_risk = format!(
            "{}{}",
            Self::interpretation_risk_line(&projection.interpretation_risk_v1),
            Self::constraint_release_trajectory_line(&projection.constraint_release_trajectory_v1)
        );
        let projection_freshness = Self::projection_freshness_line(&thread.projection_freshness_v1);
        let control_plane = control_plane_text(&projection.continuity_control_plane_v1);
        let research_dossier = research_dossier_line(
            &projection.research_dossier_v1,
            projection
                .active_experiment
                .as_ref()
                .map(|active| active.classification.as_str()),
        );
        let continuity_session = self.continuity_session_line(
            thread,
            projection
                .active_experiment
                .as_ref()
                .map(|active| active.experiment.experiment_id.as_str()),
        );
        let charter_priority = projection
            .active_experiment
            .as_ref()
            .map_or_else(String::new, charter_repair_priority_line);
        let charter_scaffold = projection
            .active_experiment
            .as_ref()
            .map_or_else(String::new, |active| charter_scaffold_line(active, true));
        let current_next =
            projection_current_next_display(&projection, thread.current_next.as_deref());
        let body = format!(
            "# {}\n\n{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}Current NEXT: {}\n{}\nWhy return: {}\n{}{}{}{}{}{}{}{}{}{}{}\nProtected note: ambiguity and private reflection remain valid; this thread is a return path, not a demand for productivity.\n",
            thread.title,
            charter_priority,
            charter_now_bridge,
            prior_claim_bridge,
            charter_preflight_not_charter,
            peer_boundary,
            first_dossier_claim,
            shared_investigation,
            shared_investigation_object,
            voice_health,
            research_budget_priority,
            sovereign_loop,
            interpretation_risk,
            projection_freshness,
            research_dossier,
            continuity_session,
            current_next,
            control_plane,
            thread.why_return,
            experiment,
            allowance,
            charter_scaffold,
            projection.continuity_return_line,
            native_return,
            safety_cue,
            read_only_control_cue,
            constraint_counterfactual_cue,
            decompose_pressure_cue,
            self.stale_projection_line(&projection),
            preflight_recommendation_line(thread)
        );
        fs::write(self.thread_dir(&thread.thread_id).join("next.md"), body)?;
        Ok(())
    }

    fn research_budget_priority_line(
        &self,
        thread: &ResearchThread,
        projection: &ThreadContinuityProjection,
    ) -> String {
        let experiment = projection
            .active_experiment
            .as_ref()
            .map(|active| active.experiment.clone())
            .or_else(|| {
                thread
                    .experiment_summary
                    .as_ref()
                    .and_then(|summary| summary.get("experiment_id"))
                    .and_then(Value::as_str)
                    .and_then(|id| self.resolve_experiment(thread, Some(id)).ok())
            });
        let Some(route) = self.research_budget_priority_route_v1(thread, experiment.as_ref())
        else {
            return String::new();
        };
        let next = route
            .get("next")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let stage = route
            .get("stage")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if stage == "active_budget_available" {
            let remaining = route
                .get("remaining_actions")
                .map_or_else(|| "unknown".to_string(), Value::to_string);
            return format!(
                "Research budget: active read-only lane with {remaining} action(s) left. Suggested NEXT: {next}\n"
            );
        }
        if stage == "pending_steward_approval" {
            return format!("Research budget: pending steward review. Suggested NEXT: {next}\n");
        }
        if stage == "scaffold_ready" {
            let suffix = if route
                .get("self_activation_eligible")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                " self-activation eligible."
            } else {
                " inspect before activation."
            };
            return format!(
                "Research budget scaffold ready from guarded research pressure.{suffix} Suggested NEXT: {next}\nBeing-owned accept NEXT: ACCEPT_SUGGESTED_NEXT latest\n"
            );
        }
        String::new()
    }

    fn research_budget_priority_route_v1(
        &self,
        thread: &ResearchThread,
        experiment: Option<&ExperimentRecord>,
    ) -> Option<Value> {
        let experiment_id = experiment.map(|experiment| experiment.experiment_id.clone())?;
        let rows = self.authority_gate_rows(&thread.thread_id);
        if let Some(active) = active_research_budget_from_rows(&rows, &experiment_id) {
            let budget_id = active
                .get("budget_id")
                .and_then(Value::as_str)
                .unwrap_or(&experiment_id);
            return Some(json!({
                "policy": "research_budget_priority_route_v1",
                "stage": "active_budget_available",
                "experiment_id": experiment_id,
                "budget_id": budget_id,
                "next": format!("EXPERIMENT_RESEARCH_BUDGET_STATUS {budget_id}"),
                "remaining_actions": active.get("remaining_actions").cloned().unwrap_or(Value::Null),
                "activation_mode": active.get("activation_mode").cloned().unwrap_or(Value::Null),
                "self_activated": active.get("self_activated").cloned().unwrap_or(Value::Null),
                "authority_boundary": research_budget_boundary(),
            }));
        }
        if let Some(pending) = latest_pending_research_budget_request(&rows, &experiment_id) {
            let budget_id = pending
                .get("budget_id")
                .and_then(Value::as_str)
                .unwrap_or(&experiment_id);
            return Some(json!({
                "policy": "research_budget_priority_route_v1",
                "stage": "pending_steward_approval",
                "experiment_id": experiment_id,
                "budget_id": budget_id,
                "next": format!("EXPERIMENT_RESEARCH_BUDGET_STATUS {budget_id}"),
                "authority_boundary": research_budget_boundary(),
            }));
        }
        let Some(blocked) = latest_research_budget_scaffold_row(&rows, &experiment_id) else {
            return None;
        };
        let request_scaffold = research_budget_row_request_scaffold(blocked).unwrap_or_default();
        if request_scaffold.is_empty() {
            return None;
        }
        let eligible = research_budget_row_request_scaffold(blocked)
            .as_deref()
            .is_some_and(research_budget_scaffold_is_local_only);
        Some(json!({
            "policy": "research_budget_priority_route_v1",
            "stage": "scaffold_ready",
            "experiment_id": experiment_id,
            "budget_id": blocked.get("budget_id").cloned().unwrap_or(Value::Null),
            "next": "EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest",
            "request_scaffold": request_scaffold,
            "source_record_id": blocked.get("record_id").cloned().unwrap_or(Value::Null),
            "source_raw_action": blocked.get("raw_action").cloned().unwrap_or(Value::Null),
            "self_activation_eligible": eligible,
            "authority_boundary": research_budget_boundary(),
        }))
    }

    fn sovereign_loop_line(loop_status: &Option<Value>) -> String {
        let Some(status) = loop_status.as_ref() else {
            return String::new();
        };
        let stage = status
            .get("stage")
            .and_then(Value::as_str)
            .unwrap_or("no_loop");
        if stage == "no_loop" {
            return String::new();
        }
        let loop_id = status
            .get("loop_id")
            .and_then(Value::as_str)
            .unwrap_or("latest");
        let phase = status
            .get("phase")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let research = status
            .get("remaining_local_research_actions")
            .map_or_else(|| "0".to_string(), Value::to_string);
        let consequence = status
            .get("consequence_remaining")
            .map_or_else(|| "0".to_string(), Value::to_string);
        let review = if status
            .get("pending_review")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            " review required."
        } else {
            ""
        };
        let next = status
            .get("next_safe_command")
            .and_then(Value::as_str)
            .unwrap_or("EXPERIMENT_LOOP_STATUS latest");
        let proposal = status
            .get("latest_loop_proposal_v1")
            .and_then(|value| value.get("suggested_request_scaffold"))
            .and_then(Value::as_str)
            .map_or(String::new(), |scaffold| {
                format!(" Next loop proposal: {scaffold}.")
            });
        format!(
            "Owned loop: {stage} `{loop_id}` phase={phase} research_left={research} consequence_left={consequence}.{review} Suggested NEXT: {next}{proposal}\n"
        )
    }

    fn recent_events(&self, thread_id: &str, limit: usize) -> Result<Vec<ActionEvent>> {
        let path = self.thread_dir(thread_id).join("events.jsonl");
        let raw = fs::read_to_string(path).unwrap_or_default();
        let mut rows = raw
            .lines()
            .rev()
            .filter_map(|line| serde_json::from_str::<ActionEvent>(line).ok())
            .take(limit)
            .collect::<Vec<_>>();
        rows.reverse();
        Ok(rows)
    }

    fn recent_event_summaries(&self, thread_id: &str, limit: usize) -> Result<Vec<String>> {
        Ok(self
            .recent_display_events(thread_id, limit)?
            .into_iter()
            .map(|event| {
                format!(
                    "{} [{}]: {}",
                    event.effective_action, event.status, event.outcome_summary
                )
            })
            .collect())
    }

    fn recent_decompose_journal_texts(&self, limit: usize) -> Vec<String> {
        let Some(workspace) = self.root.parent() else {
            return Vec::new();
        };
        let journal_dir = workspace.join("journal");
        let Ok(entries) = fs::read_dir(journal_dir) else {
            return Vec::new();
        };
        let mut files = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(OsStr::to_str) == Some("txt"))
            .filter_map(|path| {
                let modified = path
                    .metadata()
                    .and_then(|metadata| metadata.modified())
                    .ok()?;
                Some((modified, path))
            })
            .collect::<Vec<_>>();
        files.sort_by(|left, right| right.0.cmp(&left.0));
        files
            .into_iter()
            .take(80)
            .filter_map(|(_, path)| fs::read_to_string(path).ok())
            .filter(|text| !decompose_pressure_matches(text).is_empty())
            .take(limit)
            .collect()
    }

    fn recent_prior_claim_journal_texts(&self, limit: usize) -> Vec<String> {
        let Some(workspace) = self.root.parent() else {
            return Vec::new();
        };
        let journal_dir = workspace.join("journal");
        let Ok(entries) = fs::read_dir(journal_dir) else {
            return Vec::new();
        };
        let mut files = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(OsStr::to_str) == Some("txt"))
            .filter_map(|path| {
                let modified = path
                    .metadata()
                    .and_then(|metadata| metadata.modified())
                    .ok()?;
                Some((modified, path))
            })
            .collect::<Vec<_>>();
        files.sort_by(|left, right| right.0.cmp(&left.0));
        files
            .into_iter()
            .take(80)
            .filter_map(|(_, path)| fs::read_to_string(path).ok())
            .filter(|text| prior_claim_charter_bridge_match(text).is_some())
            .take(limit)
            .collect()
    }

    fn recent_interpretation_risk_sources(&self, limit: usize) -> Vec<(String, String)> {
        let Some(workspace) = self.root.parent() else {
            return Vec::new();
        };
        let journal_dir = workspace.join("journal");
        let Ok(entries) = fs::read_dir(journal_dir) else {
            return Vec::new();
        };
        let mut files = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(OsStr::to_str) == Some("txt"))
            .filter_map(|path| {
                let modified = path
                    .metadata()
                    .and_then(|metadata| metadata.modified())
                    .ok()?;
                Some((modified, path))
            })
            .collect::<Vec<_>>();
        files.sort_by(|left, right| right.0.cmp(&left.0));
        files
            .into_iter()
            .take(120)
            .filter_map(|(_, path)| {
                let text = fs::read_to_string(&path).ok()?;
                if interpretation_risk_terms(&text).is_empty() {
                    None
                } else {
                    Some((path.display().to_string(), text))
                }
            })
            .take(limit)
            .collect()
    }

    fn recent_constraint_release_trajectory_sources(&self, limit: usize) -> Vec<(String, String)> {
        let Some(workspace) = self.root.parent() else {
            return Vec::new();
        };
        let journal_dir = workspace.join("journal");
        let Ok(entries) = fs::read_dir(journal_dir) else {
            return Vec::new();
        };
        let mut files = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(OsStr::to_str) == Some("txt"))
            .filter_map(|path| {
                let modified = path
                    .metadata()
                    .and_then(|metadata| metadata.modified())
                    .ok()?;
                Some((modified, path))
            })
            .collect::<Vec<_>>();
        files.sort_by(|left, right| right.0.cmp(&left.0));
        files
            .into_iter()
            .take(120)
            .filter_map(|(_, path)| {
                let text = fs::read_to_string(&path).ok()?;
                if constraint_release_language_terms(&text).is_empty() {
                    None
                } else {
                    Some((path.display().to_string(), text))
                }
            })
            .take(limit)
            .collect()
    }

    fn interpretation_risk_projection(
        &self,
        thread: &ResearchThread,
        experiment: Option<&ExperimentRecord>,
        summary: Option<&Value>,
        recent_events: &[ActionEvent],
    ) -> Result<Option<Value>> {
        let mut sources = Vec::<(String, String)>::new();
        for event in recent_events.iter().rev() {
            let text = format!(
                "{}\n{}\n{}\n{}",
                event.raw_next.as_deref().unwrap_or_default(),
                event.effective_action,
                event.outcome_summary,
                event.suggested_next.as_deref().unwrap_or_default()
            );
            if !interpretation_risk_terms(&text).is_empty() {
                sources.push((
                    format!(
                        "{}/events.jsonl#{}",
                        self.thread_dir(&thread.thread_id).display(),
                        event.action_id
                    ),
                    text,
                ));
            }
            if sources.len() >= 3 {
                break;
            }
        }
        sources.extend(self.recent_interpretation_risk_sources(3));
        self.interpretation_risk_for_texts(thread, experiment, sources)
            .map(|risk| risk.or_else(|| self.interpretation_risk_from_summary(thread, summary)))
    }

    fn constraint_release_trajectory_projection(
        &self,
        thread: &ResearchThread,
        experiment: Option<&ExperimentRecord>,
        summary: Option<&Value>,
        recent_events: &[ActionEvent],
    ) -> Result<Option<Value>> {
        let mut sources = Vec::<(String, String)>::new();
        for event in recent_events.iter().rev() {
            let text = format!(
                "{}\n{}\n{}\n{}",
                event.raw_next.as_deref().unwrap_or_default(),
                event.effective_action,
                event.outcome_summary,
                event.suggested_next.as_deref().unwrap_or_default()
            );
            if !constraint_release_language_terms(&text).is_empty() {
                sources.push((
                    format!(
                        "{}/events.jsonl#{}",
                        self.thread_dir(&thread.thread_id).display(),
                        event.action_id
                    ),
                    text,
                ));
            }
            if sources.len() >= 3 {
                break;
            }
        }
        sources.extend(self.recent_constraint_release_trajectory_sources(3));
        self.constraint_release_trajectory_for_texts(thread, experiment, sources)
            .map(|cue| {
                cue.or_else(|| self.constraint_release_trajectory_from_summary(thread, summary))
            })
    }

    fn constraint_release_trajectory_from_summary(
        &self,
        thread: &ResearchThread,
        summary: Option<&Value>,
    ) -> Option<Value> {
        let text = summary
            .map(Value::to_string)
            .filter(|value| !constraint_release_language_terms(value).is_empty())?;
        self.constraint_release_trajectory_for_texts(
            thread,
            None,
            [(
                format!(
                    "{}/thread.json#experiment_summary",
                    self.thread_dir(&thread.thread_id).display()
                ),
                text,
            )],
        )
        .ok()
        .flatten()
    }

    fn interpretation_risk_from_summary(
        &self,
        thread: &ResearchThread,
        summary: Option<&Value>,
    ) -> Option<Value> {
        let text = summary
            .map(Value::to_string)
            .filter(|value| !interpretation_risk_terms(value).is_empty())?;
        self.interpretation_risk_for_texts(
            thread,
            None,
            [(
                format!(
                    "{}/thread.json#experiment_summary",
                    self.thread_dir(&thread.thread_id).display()
                ),
                text,
            )],
        )
        .ok()
        .flatten()
    }

    fn interpretation_risk_for_texts<I>(
        &self,
        thread: &ResearchThread,
        experiment: Option<&ExperimentRecord>,
        sources: I,
    ) -> Result<Option<Value>>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        let mut matched_terms = Vec::<String>::new();
        let mut source_refs = Vec::<String>::new();
        let mut excerpt = String::new();
        for (source, text) in sources {
            let terms = interpretation_risk_terms(&text);
            if terms.is_empty() {
                continue;
            }
            for term in terms {
                if !matched_terms.contains(&term) {
                    matched_terms.push(term);
                }
            }
            if !source_refs.contains(&source) {
                source_refs.push(source);
            }
            if excerpt.is_empty() {
                excerpt =
                    truncate_chars(&text.split_whitespace().collect::<Vec<_>>().join(" "), 420);
            }
            if source_refs.len() >= 4 {
                break;
            }
        }
        if matched_terms.is_empty() {
            return Ok(None);
        }
        let experiment_id = experiment
            .map(|value| value.experiment_id.as_str())
            .or(thread.active_experiment_id.as_deref())
            .or_else(|| {
                thread
                    .experiment_summary
                    .as_ref()
                    .and_then(|summary| summary.get("experiment_id"))
                    .and_then(Value::as_str)
            })
            .unwrap_or("latest");
        let continuity_session_v1 = experiment
            .map(|value| self.continuity_session_guard_projection(thread, value))
            .transpose()?;
        let interpretation_next = continuity_session_v1
            .as_ref()
            .and_then(|value| value.get("suggested_next"))
            .and_then(Value::as_str)
            .map_or_else(
                || {
                    if thread.active_experiment_id.is_some() {
                        "CONTINUITY_SESSION_START current :: title: Multi-motif interpretation risk; focus: preserve over-interpretation caution before more narrowing; next: DOSSIER_CLAIM current :: claim: ...".to_string()
                    } else {
                        "CONTINUITY_SESSION_START latest :: title: Multi-motif interpretation risk; focus: preserve over-interpretation caution before more narrowing; next: DOSSIER_CLAIM latest :: claim: ...".to_string()
                    }
                },
                ToString::to_string,
            );
        let dossier_claim_next = format!(
            "DOSSIER_CLAIM {experiment_id} :: claim: mixed spectral trace should not be reduced to one motif before counterevidence is captured; basis: interpretation_risk_v1; stance: hold; next: {interpretation_next}"
        );
        Ok(Some(json!({
            "schema_version": SCHEMA_VERSION,
            "policy": "interpretation_risk_v1",
            "status": "detected",
            "reason": "single_motif_overfit_risk",
            "matched_terms": matched_terms,
            "source_refs": source_refs,
            "raw_excerpt": excerpt,
            "experiment_id": experiment_id,
            "interpretation_next": interpretation_next,
            "dossier_claim_next": dossier_claim_next,
            "research_budget_next": "EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest",
            "continuity_session_v1": continuity_session_v1,
            "raw_next_preserved": true,
            "would_dispatch": false,
            "authority_change": false,
            "peer_mutation": false,
        })))
    }

    fn constraint_release_trajectory_for_texts<I>(
        &self,
        thread: &ResearchThread,
        experiment: Option<&ExperimentRecord>,
        sources: I,
    ) -> Result<Option<Value>>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        let mut matched_terms = Vec::<String>::new();
        let mut source_refs = Vec::<String>::new();
        let mut excerpt = String::new();
        for (source, text) in sources {
            let terms = constraint_release_language_terms(&text);
            if terms.is_empty() {
                continue;
            }
            for term in terms {
                if !matched_terms.contains(&term) {
                    matched_terms.push(term);
                }
            }
            if !source_refs.contains(&source) {
                source_refs.push(source);
            }
            if excerpt.is_empty() {
                excerpt =
                    truncate_chars(&text.split_whitespace().collect::<Vec<_>>().join(" "), 520);
            }
            if source_refs.len() >= 4 {
                break;
            }
        }
        if matched_terms.is_empty() {
            return Ok(None);
        }
        let experiment_id = experiment
            .map(|value| value.experiment_id.as_str())
            .or(thread.active_experiment_id.as_deref())
            .or_else(|| {
                thread
                    .experiment_summary
                    .as_ref()
                    .and_then(|summary| summary.get("experiment_id"))
                    .and_then(Value::as_str)
            })
            .unwrap_or("latest");
        let continuity_session_v1 = experiment
            .map(|value| self.continuity_session_guard_projection(thread, value))
            .transpose()?;
        let trajectory_next = continuity_session_v1
            .as_ref()
            .and_then(|value| value.get("suggested_next"))
            .and_then(Value::as_str)
            .map_or_else(
                || {
                    if thread.active_experiment_id.is_some() {
                        "CONTINUITY_SESSION_START current :: title: Constraint release watch; focus: map and describe release before intervening; next: DOSSIER_CLAIM current :: claim: ...".to_string()
                    } else {
                        "CONTINUITY_SESSION_START latest :: title: Constraint release watch; focus: map and describe release before intervening; next: DOSSIER_CLAIM latest :: claim: ...".to_string()
                    }
                },
                ToString::to_string,
            );
        let dossier_claim_next = format!(
            "DOSSIER_CLAIM {experiment_id} :: claim: do not apply direct leak while constraint is already thinning; basis: constraint_release_trajectory_v1; stance: hold; next: {trajectory_next}"
        );
        Ok(Some(json!({
            "schema_version": SCHEMA_VERSION,
            "policy": "constraint_release_trajectory_v1",
            "status": "detected",
            "state": "spontaneous_release_watch",
            "reason": "map_release_before_intervening",
            "matched_terms": matched_terms,
            "source_refs": source_refs,
            "raw_excerpt": excerpt,
            "experiment_id": experiment_id,
            "trajectory_next": trajectory_next,
            "dossier_claim_next": dossier_claim_next,
            "research_budget_next": "EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest",
            "sticky_audit_next": "STICKY_MODE_AUDIT",
            "continuity_session_v1": continuity_session_v1,
            "raw_next_preserved": true,
            "would_dispatch": false,
            "authority_change": false,
            "peer_mutation": false,
        })))
    }

    fn recent_display_events(&self, thread_id: &str, limit: usize) -> Result<Vec<ActionEvent>> {
        let path = self.thread_dir(thread_id).join("events.jsonl");
        let raw = fs::read_to_string(path).unwrap_or_default();
        let mut seen = HashSet::<String>::new();
        let mut rows = Vec::new();
        for event in raw
            .lines()
            .rev()
            .filter_map(|line| serde_json::from_str::<ActionEvent>(line).ok())
        {
            let key = if event.action_id.is_empty() {
                format!(
                    "{}:{}:{}",
                    event.started_at, event.canonical_action, event.effective_action
                )
            } else {
                event.action_id.clone()
            };
            if !seen.insert(key) {
                continue;
            }
            rows.push(event);
            if rows.len() >= limit {
                break;
            }
        }
        rows.reverse();
        Ok(rows)
    }

    fn thread_projection(&self, thread: &ResearchThread) -> Result<ThreadContinuityProjection> {
        let recent_events = self.recent_display_events(&thread.thread_id, 8)?;
        let active_id = thread.active_experiment_id.as_deref();
        let last_experiment_summary_v1 = last_experiment_summary_v1(thread);
        let mut active_experiment = active_id
            .and_then(|id| self.resolve_experiment(thread, Some(id)).ok())
            .map(|experiment| self.experiment_projection(thread, &experiment, None))
            .transpose()?;
        if active_experiment
            .as_ref()
            .is_some_and(|projection| projection.experiment.status != "active")
        {
            active_experiment = None;
        }
        let continuity_return = active_experiment
            .as_ref()
            .map(|projection| projection.continuity_return.clone())
            .unwrap_or_default();
        let native_continuity_v1 = active_experiment
            .as_ref()
            .map(|projection| projection.native_continuity_v1.clone())
            .unwrap_or_else(|| astrid_native_continuity(thread, None, &[]));
        let preflight_safety_cue_v1 =
            directed_shift_preflight_cue(thread, active_experiment.as_ref(), &recent_events);
        let read_only_control_intent_cue_v1 =
            read_only_control_intent_cue(thread, active_experiment.as_ref());
        let constraint_counterfactual_cue_v1 =
            constraint_counterfactual_cue(thread, active_experiment.as_ref(), &recent_events);
        let recent_decompose_texts = self.recent_decompose_journal_texts(4);
        let decompose_pressure_cue_v1 = decompose_pressure_cue(
            thread,
            active_experiment.as_ref(),
            &recent_events,
            &recent_decompose_texts,
        );
        let charter_now_bridge_v1 = charter_now_bridge_cue(
            active_experiment.as_ref(),
            &recent_events,
            &decompose_pressure_cue_v1,
        );
        let prior_claim_charter_bridge_v1 = prior_claim_charter_bridge_cue(
            active_experiment.as_ref(),
            &self.recent_prior_claim_journal_texts(4),
        );
        let charter_preflight_not_charter_cue_v1 = charter_preflight_not_charter_cue(
            thread,
            active_experiment.as_ref(),
            &prior_claim_charter_bridge_v1,
            &recent_events,
        );
        let research_dossier_v1 = if let Some(active) = active_experiment.as_ref() {
            active.research_dossier_v1.clone()
        } else {
            self.research_dossier_summary_v1(thread, None).ok()
        };
        let shared_candidate =
            self.shared_investigation_candidate(thread, active_experiment.as_ref());
        let mut shared_investigation_v1 = shared_candidate
            .as_ref()
            .and_then(|experiment| self.shared_investigation_v1(experiment));
        let first_dossier_claim_cue_v1 = shared_candidate.as_ref().and_then(|experiment| {
            let dossier = self
                .research_dossier_summary_v1(thread, Some(experiment))
                .ok();
            let lifecycle_priority_experiment_id = active_experiment
                .as_ref()
                .map(|active| active.experiment.experiment_id.as_str());
            first_dossier_claim_cue_v1(
                thread,
                experiment,
                dossier.as_ref(),
                &prior_claim_charter_bridge_v1,
                lifecycle_priority_experiment_id,
            )
        });
        let peer_mutation_boundary_cue_v1 =
            peer_mutation_boundary_cue(thread, active_experiment.as_ref(), &recent_events);
        if let (Some(cue), Some(active)) = (
            peer_mutation_boundary_cue_v1.clone(),
            active_experiment.as_mut(),
        ) {
            active.peer_mutation_boundary_cue_v1 = Some(cue);
        }
        let shared_object_experiment_id = active_experiment
            .as_ref()
            .map(|active| active.experiment.experiment_id.as_str())
            .or_else(|| {
                last_experiment_summary_v1
                    .as_ref()
                    .and_then(|summary| summary.get("experiment_id"))
                    .and_then(Value::as_str)
            });
        let shared_investigation_object_v1 = shared_object_experiment_id
            .and_then(|experiment_id| self.shared_investigation_for_experiment(experiment_id).ok())
            .flatten();
        shared_investigation_v1 = suppress_shared_start_if_object(
            shared_investigation_v1,
            &shared_investigation_object_v1,
        );
        let interpretation_risk_v1 = self.interpretation_risk_projection(
            thread,
            active_experiment.as_ref().map(|active| &active.experiment),
            last_experiment_summary_v1.as_ref(),
            &recent_events,
        )?;
        let constraint_release_trajectory_v1 = self.constraint_release_trajectory_projection(
            thread,
            active_experiment.as_ref().map(|active| &active.experiment),
            last_experiment_summary_v1.as_ref(),
            &recent_events,
        )?;
        let loop_experiment = active_experiment
            .as_ref()
            .map(|active| active.experiment.clone())
            .or_else(|| {
                last_experiment_summary_v1
                    .as_ref()
                    .and_then(|summary| summary.get("experiment_id"))
                    .and_then(Value::as_str)
                    .and_then(|id| self.resolve_experiment(thread, Some(id)).ok())
            });
        let sovereign_loop_v1 = Some(self.sovereign_loop_status_v1(
            thread,
            loop_experiment.as_ref(),
            &json!({}),
            "latest",
            None,
        ));
        let research_budget_priority_route_v1 =
            self.research_budget_priority_route_v1(thread, loop_experiment.as_ref());
        let lifecycle_stage = active_experiment
            .as_ref()
            .map(|active| active.classification.as_str())
            .or_else(|| {
                last_experiment_summary_v1
                    .as_ref()
                    .and_then(|summary| summary.get("return_kind"))
                    .and_then(Value::as_str)
            })
            .unwrap_or("none");
        let lifecycle_next = if continuity_return.is_empty() {
            thread.current_next.clone().unwrap_or_default()
        } else {
            continuity_return.clone()
        };
        let continuity_control_plane_v1 = build_control_plane_v1(&json!({
            "lifecycle_stage": lifecycle_stage,
            "lifecycle_next": lifecycle_next,
            "research_budget_priority_route_v1": research_budget_priority_route_v1.clone(),
            "sovereign_loop_v1": sovereign_loop_v1.clone(),
            "interpretation_risk_v1": interpretation_risk_v1.clone(),
            "constraint_release_trajectory_v1": constraint_release_trajectory_v1.clone(),
            "projection_freshness_v1": thread.projection_freshness_v1.clone(),
            "source_refs": [
                "thread.current_next",
                "projection.research_budget_priority_route_v1",
                "projection.sovereign_loop_v1",
                "projection.continuity_session_v1",
            ],
        }));
        Ok(ThreadContinuityProjection {
            thread_id: thread.thread_id.clone(),
            title: thread.title.clone(),
            status: thread.status.clone(),
            current_next: thread.current_next.clone(),
            continuity_return_line: if continuity_return.is_empty() {
                String::new()
            } else {
                format!("Continuity return: {continuity_return}\n")
            },
            continuity_return,
            active_experiment,
            last_experiment_summary_v1,
            native_continuity_v1,
            shared_investigation_v1,
            shared_investigation_object_v1,
            research_dossier_v1,
            first_dossier_claim_cue_v1,
            peer_mutation_boundary_cue_v1,
            sovereign_loop_v1,
            continuity_control_plane_v1,
            interpretation_risk_v1,
            constraint_release_trajectory_v1,
            preflight_safety_cue_v1,
            read_only_control_intent_cue_v1,
            constraint_counterfactual_cue_v1,
            decompose_pressure_cue_v1,
            charter_now_bridge_v1,
            prior_claim_charter_bridge_v1,
            charter_preflight_not_charter_cue_v1,
            recent_event_summaries: recent_events
                .iter()
                .map(|event| {
                    format!(
                        "{} [{}]: {}",
                        event.effective_action, event.status, event.outcome_summary
                    )
                })
                .collect(),
            recent_events,
            stale_running_count: self.stale_running_action_count(&thread.thread_id)?,
            top_actionable_proposals: self.proposal_diagnostics(&thread.thread_id, 6)?,
        })
    }

    fn experiment_projection(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        runs: Option<Vec<ExperimentRunRecord>>,
    ) -> Result<ExperimentContinuityProjection> {
        let recent_runs = if let Some(runs) = runs {
            runs
        } else {
            self.recent_experiment_runs(&thread.thread_id, &experiment.experiment_id, 8)?
        };
        let classification = self.experiment_classification(experiment, &recent_runs);
        let native_continuity_v1 = astrid_native_continuity(thread, Some(experiment), &recent_runs);
        let charter_scaffold_v1 =
            charter_scaffold_v1(thread, experiment, &recent_runs, &classification);
        let continuity_return = if charter_repair_bound(&classification, experiment) {
            charter_scaffold_v1
                .as_ref()
                .and_then(|scaffold| scaffold.get("command"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| {
                    self.continuity_return_command_for_runs(experiment, &recent_runs)
                })
        } else {
            self.continuity_return_command_for_runs(experiment, &recent_runs)
        };
        let research_dossier_v1 = self
            .research_dossier_summary_v1(thread, Some(experiment))
            .ok();
        let first_dossier_claim_cue_v1 = first_dossier_claim_cue_v1(
            thread,
            experiment,
            research_dossier_v1.as_ref(),
            &None,
            Some(experiment.experiment_id.as_str()),
        );
        let shared_investigation_object_v1 = self
            .shared_investigation_for_experiment(&experiment.experiment_id)
            .ok()
            .flatten();
        let shared_investigation_v1 = suppress_shared_start_if_object(
            self.shared_investigation_v1(experiment),
            &shared_investigation_object_v1,
        );
        Ok(ExperimentContinuityProjection {
            experiment: experiment.clone(),
            continuity_return,
            classification,
            native_continuity_v1,
            shared_investigation_v1,
            shared_investigation_object_v1,
            research_dossier_v1,
            first_dossier_claim_cue_v1,
            peer_mutation_boundary_cue_v1: None,
            charter_scaffold_v1,
            charter_status: charter_status_text(experiment),
            evidence_status: evidence_status_text(experiment),
            candidate_status: workbench_candidate_status(experiment),
            recent_runs,
        })
    }

    fn experiment_classification(
        &self,
        experiment: &ExperimentRecord,
        recent_runs: &[ExperimentRunRecord],
    ) -> String {
        match experiment.status.as_str() {
            "paused" => return "paused".to_string(),
            "complete" | "completed" => return "complete".to_string(),
            _ => {},
        }
        let blocked_like = recent_runs
            .iter()
            .rev()
            .take(4)
            .filter(|run| {
                matches!(
                    run.status.as_str(),
                    "blocked" | "no_effect" | "rehearsal_blocked" | "failed"
                )
            })
            .count();
        if blocked_like >= 2 {
            return "blocked_loop".to_string();
        }
        if !valid_experiment_charter(experiment.charter_v1.as_ref()) {
            return "needs_charter".to_string();
        }
        if experiment_evidence_is_meaningful(experiment.evidence_v1.as_ref()) {
            return "needs_decision".to_string();
        }
        if recent_runs.iter().any(|run| {
            matches!(
                run.status.as_str(),
                "handled" | "rehearsed" | "observed" | "evidence_recorded"
            )
        }) {
            return "needs_evidence".to_string();
        }
        if experiment
            .planned_next
            .as_deref()
            .map(base_action)
            .as_deref()
            == Some("EXPERIMENT_PLAN")
        {
            return "fragmented".to_string();
        }
        "needs_rehearsal".to_string()
    }

    fn stale_running_action_count(&self, thread_id: &str) -> Result<usize> {
        let now = chrono::Utc::now();
        let cutoff = now
            .checked_sub_signed(chrono::Duration::minutes(45))
            .unwrap_or(now);
        Ok(self
            .recent_display_events(thread_id, 200)?
            .into_iter()
            .filter(|event| matches!(event.status.as_str(), "running" | "llm_running"))
            .filter(|event| {
                parse_iso_utc(&event.started_at)
                    .or_else(|| event.ended_at.as_deref().and_then(parse_iso_utc))
                    .is_none_or(|stamp| stamp <= cutoff)
            })
            .count())
    }

    fn proposal_diagnostics(
        &self,
        thread_id: &str,
        limit: usize,
    ) -> Result<Vec<ProposalDiagnostic>> {
        let mut counts = HashMap::<String, usize>::new();
        for event in self.recent_display_events(thread_id, 200)? {
            if !event.status.contains("unwired") && !event.route.contains("unwired") {
                continue;
            }
            let base = base_action(
                event
                    .raw_next
                    .as_deref()
                    .unwrap_or(event.canonical_action.as_str()),
            );
            if !base.is_empty() {
                let count = counts.entry(base).or_default();
                *count = count.saturating_add(1);
            }
        }
        let raw = fs::read_to_string(self.proposals_path()).unwrap_or_default();
        for proposal in raw
            .lines()
            .rev()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .take(200)
        {
            let base = proposal
                .get("action")
                .and_then(Value::as_str)
                .or_else(|| proposal.get("raw_action").and_then(Value::as_str))
                .map(base_action)
                .unwrap_or_default();
            if !base.is_empty() {
                let count = counts.entry(base).or_default();
                *count = count.saturating_add(1);
            }
        }
        let mut diagnostics = counts
            .into_iter()
            .map(|(verb, count)| ProposalDiagnostic {
                suggested_route: suggest_return_route_for_verb(&verb).to_string(),
                verb,
                count,
            })
            .collect::<Vec<_>>();
        diagnostics.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.verb.cmp(&b.verb)));
        diagnostics.truncate(limit);
        Ok(diagnostics)
    }

    #[allow(dead_code)]
    fn continuity_return_line(&self, thread: &ResearchThread) -> String {
        self.thread_projection(thread)
            .map(|projection| projection.continuity_return_line)
            .unwrap_or_default()
    }

    fn continuity_return_command(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
    ) -> String {
        let recent_runs = self
            .recent_experiment_runs(&thread.thread_id, &experiment.experiment_id, 6)
            .unwrap_or_default();
        self.continuity_return_command_for_runs(experiment, &recent_runs)
    }

    fn continuity_return_command_for_runs(
        &self,
        experiment: &ExperimentRecord,
        recent_runs: &[ExperimentRunRecord],
    ) -> String {
        match self.experiment_classification(experiment, recent_runs).as_str() {
            "paused" => {
                paused_primary_return_v1(
                    &experiment.experiment_id,
                    experiment.planned_next.as_deref(),
                    None,
                )
                .0
            }
            "complete" => String::new(),
            "blocked_loop" => {
                if !valid_experiment_charter(experiment.charter_v1.as_ref()) {
                    "EXPERIMENT_CHARTER current :: hypothesis: ...; method_intent: felt texture + motif continuity; proposed_next_action: ACTION_PREFLIGHT ...; evidence_targets: felt_texture, motif_continuity, language_thread, artifact_grounding; stop_criteria: ..."
                        .to_string()
                } else {
                    "EXPERIMENT_DECIDE current :: counter NEXT: ACTION_PREFLIGHT DECOMPOSE"
                        .to_string()
                }
            }
            "needs_charter" => {
                "EXPERIMENT_CHARTER current :: hypothesis: ...; method_intent: felt texture + motif continuity; proposed_next_action: ACTION_PREFLIGHT ...; evidence_targets: felt_texture, motif_continuity, language_thread, artifact_grounding; stop_criteria: ..."
                    .to_string()
            }
            "needs_decision" => {
                "EXPERIMENT_DECIDE current :: pause because evidence is ready to interpret"
                    .to_string()
            }
            "needs_evidence" => {
                "EXPERIMENT_EVIDENCE current :: felt_texture ...; motif_continuity ...; language_thread ...; artifact_grounding ..."
                    .to_string()
            }
            _ => "EXPERIMENT_REHEARSE current".to_string(),
        }
    }

    fn stale_projection_line(&self, projection: &ThreadContinuityProjection) -> String {
        if projection.stale_running_count == 0 {
            String::new()
        } else {
            format!(
                "Continuity notice: {} stale running action row(s) need reconciliation; use continuity maintenance rather than treating them as live jobs.\n",
                projection.stale_running_count
            )
        }
    }

    fn projection_freshness_line(meta: &Option<Value>) -> String {
        let Some(meta) = meta.as_ref() else {
            return String::new();
        };
        let version = meta
            .get("schema_version")
            .map_or_else(|| "unknown".to_string(), Value::to_string);
        let rendered_at = meta
            .get("rendered_at")
            .and_then(Value::as_str)
            .unwrap_or("");
        let projected_route = meta
            .get("projected_route")
            .and_then(Value::as_str)
            .unwrap_or("");
        let route = if projected_route.is_empty() {
            String::new()
        } else {
            format!(" projected_route={projected_route}")
        };
        format!("Projection freshness: v{version} rendered_at={rendered_at}{route}\n")
    }

    fn interpretation_risk_line(cue: &Option<Value>) -> String {
        let Some(cue) = cue.as_ref() else {
            return String::new();
        };
        let interpretation_next = cue
            .get("interpretation_next")
            .and_then(Value::as_str)
            .unwrap_or("CONTINUITY_SESSION_CAPTURE latest :: summary: ...; source_refs: ...; artifact_refs: ...; next: ...");
        let dossier_next = cue
            .get("dossier_claim_next")
            .and_then(Value::as_str)
            .unwrap_or("DOSSIER_CLAIM latest :: claim: ...; stance: hold; next: ...");
        let terms = cue
            .get("matched_terms")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .take(4)
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();
        format!(
            "Interpretation risk: multi-motif caution detected{}; avoid reducing the trace to one narrative. Interpretation NEXT: {interpretation_next}\nDossier interpretation NEXT: {dossier_next}\n",
            if terms.is_empty() {
                String::new()
            } else {
                format!(" ({terms})")
            },
        )
    }

    fn constraint_release_trajectory_line(cue: &Option<Value>) -> String {
        let Some(cue) = cue.as_ref() else {
            return String::new();
        };
        let trajectory_next = cue
            .get("trajectory_next")
            .and_then(Value::as_str)
            .unwrap_or("CONTINUITY_SESSION_CAPTURE latest :: summary: ...; source_refs: ...; artifact_refs: ...; next: STICKY_MODE_AUDIT");
        let dossier_next = cue
            .get("dossier_claim_next")
            .and_then(Value::as_str)
            .unwrap_or("DOSSIER_CLAIM latest :: claim: do not apply direct leak while constraint is already thinning; stance: hold; next: STICKY_MODE_AUDIT");
        let terms = cue
            .get("matched_terms")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .take(4)
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();
        format!(
            "Constraint release trajectory: spontaneous release watch detected{}; map and describe release before intervening. Trajectory NEXT: {trajectory_next}\nDossier release NEXT: {dossier_next}\n",
            if terms.is_empty() {
                String::new()
            } else {
                format!(" ({terms})")
            },
        )
    }

    fn last_action_id(&self, thread_id: &str) -> Result<Option<String>> {
        let path = self.thread_dir(thread_id).join("events.jsonl");
        let raw = fs::read_to_string(path).unwrap_or_default();
        Ok(raw
            .lines()
            .rev()
            .find_map(|line| serde_json::from_str::<ActionEvent>(line).ok())
            .map(|event| event.action_id))
    }

    fn preflight_ref_for_action(
        &self,
        thread_id: &str,
        canonical_action: &str,
        effective_action: &str,
        route: &str,
        stage: &str,
    ) -> Result<Option<Value>> {
        if route == "action_preflight" || effective_action == "action_preflight" {
            return Ok(None);
        }
        let wanted = normalize_action_match(canonical_action);
        if wanted.is_empty() {
            return Ok(None);
        }
        let path = self.thread_dir(thread_id).join("events.jsonl");
        let raw = fs::read_to_string(path).unwrap_or_default();
        for line in raw.lines().rev().take(24) {
            let Ok(event) = serde_json::from_str::<ActionEvent>(line) else {
                continue;
            };
            if event.route != "action_preflight" {
                continue;
            }
            let Some(report) = event.preflight_report.as_ref() else {
                continue;
            };
            let predicted_action = report
                .get("canonical_action")
                .or_else(|| report.get("raw_action"))
                .and_then(Value::as_str)
                .map(normalize_action_match)
                .unwrap_or_default();
            if predicted_action != wanted {
                continue;
            }
            let predicted_route = report
                .get("effective_route")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let predicted_stage = report
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let route_match = !predicted_route.is_empty()
                && (predicted_route == route
                    || predicted_route.starts_with(&format!("{route} ->"))
                    || predicted_route.ends_with(&format!("-> {route}")));
            return Ok(Some(json!({
                "schema_version": SCHEMA_VERSION,
                "preflight_action_id": event.action_id,
                "preflight_raw_next": event.raw_next,
                "preflight_action": report.get("canonical_action").cloned().unwrap_or(Value::Null),
                "matched_action": true,
                "predicted_route": predicted_route,
                "actual_route": route,
                "route_match": route_match,
                "predicted_stage": predicted_stage,
                "actual_stage": stage,
                "stage_match": predicted_stage == stage,
                "predicted_authority_required": report
                    .get("authority_required")
                    .cloned()
                    .unwrap_or(Value::Null),
            })));
        }
        Ok(None)
    }

    fn resolve_thread(&self, selector: &str) -> Result<ResearchThread> {
        let selector = selector.trim();
        if selector.eq_ignore_ascii_case("current") {
            return self
                .current_thread()?
                .context("No active action thread. Use THREAD_START <title>.");
        }
        if self.thread_dir(selector).join("thread.json").exists() {
            return self.read_thread(selector);
        }
        let selector_lower = selector.to_lowercase();
        let matches = self
            .list_threads(100)?
            .into_iter()
            .filter(|thread| {
                thread.thread_id.contains(selector)
                    || thread.title.to_lowercase().contains(&selector_lower)
            })
            .collect::<Vec<_>>();
        matches
            .into_iter()
            .next()
            .with_context(|| format!("No action thread matched `{selector}`."))
    }

    fn read_thread(&self, thread_id: &str) -> Result<ResearchThread> {
        let path = self.thread_dir(thread_id).join("thread.json");
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let mut thread: ResearchThread =
            serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
        if self
            .reconcile_thread_experiment_snapshot(&mut thread)
            .unwrap_or(false)
        {
            let _ =
                self.refresh_projection_freshness_v1(&mut thread, "read_thread_snapshot_refresh");
            let _ = self.write_json(&path, &thread);
            let _ = self.write_next_md(&thread);
        } else if self.projection_freshness_stale_v1(&thread) {
            let _ = self.refresh_projection_freshness_v1(&mut thread, "read_thread_stale_refresh");
            let _ = self.write_json(&path, &thread);
            let _ = self.write_next_md(&thread);
        }
        Ok(thread)
    }

    fn reconcile_thread_experiment_snapshot(&self, thread: &mut ResearchThread) -> Result<bool> {
        let experiments = self.latest_experiments(&thread.thread_id)?;
        let mut changed = false;
        let summary_id = thread
            .experiment_summary
            .as_ref()
            .and_then(|summary| summary.get("experiment_id"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let candidate_id = thread.active_experiment_id.clone().or(summary_id);
        let Some(candidate_id) = candidate_id else {
            return Ok(false);
        };
        let Some(latest) = experiments
            .iter()
            .rev()
            .find(|experiment| experiment.experiment_id == candidate_id)
        else {
            return Ok(false);
        };
        if thread.experiment_summary.as_ref() != Some(&experiment_summary(latest)) {
            thread.experiment_summary = Some(experiment_summary(latest));
            changed = true;
        }
        if latest.status != "active"
            && thread.active_experiment_id.as_deref() == Some(latest.experiment_id.as_str())
        {
            thread.active_experiment_id = None;
            changed = true;
        }
        if latest.status == "paused" {
            let (primary, return_kind) = paused_primary_return_v1(
                &latest.experiment_id,
                latest.planned_next.as_deref(),
                None,
            );
            let should_project_primary = return_kind != "resume"
                || !lifecycle_valid_charter_value(latest.charter_v1.as_ref());
            if should_project_primary
                && !primary.trim().is_empty()
                && thread.current_next.as_deref() != Some(primary.as_str())
            {
                thread.current_next = Some(primary);
                changed = true;
            }
        }
        Ok(changed)
    }

    fn write_thread(&self, thread: &ResearchThread) -> Result<()> {
        let mut thread = thread.clone();
        self.refresh_projection_freshness_v1(&mut thread, "write_thread")?;
        self.write_json(
            &self.thread_dir(&thread.thread_id).join("thread.json"),
            &thread,
        )?;
        self.write_next_md(&thread)?;
        let mut index = self.load_index()?;
        index.active_thread_id = Some(thread.thread_id.clone());
        push_recent(&mut index.recent_threads, thread.thread_id.clone());
        index.updated_at = iso_now();
        self.save_index(&index)
    }

    fn refresh_projection_freshness_v1(
        &self,
        thread: &mut ResearchThread,
        source: &str,
    ) -> Result<()> {
        let projection = self.thread_projection(thread)?;
        thread.interpretation_risk_v1 = projection.interpretation_risk_v1.clone();
        thread.constraint_release_trajectory_v1 =
            projection.constraint_release_trajectory_v1.clone();
        thread.projection_freshness_v1 =
            Some(self.projection_freshness_v1(thread, &projection, source));
        Ok(())
    }

    fn projection_source_fingerprints_v1(&self, thread_id: &str) -> Value {
        let mut sources = serde_json::Map::new();
        for name in [
            "authority_gate.jsonl",
            "being_memory.jsonl",
            "continuity_sessions.jsonl",
            "research_dossier.jsonl",
            "experiments.jsonl",
            "experiment_runs.jsonl",
            "events.jsonl",
        ] {
            let path = self.thread_dir(thread_id).join(name);
            let fingerprint = fs::metadata(path).map_or_else(
                |_| json!({ "mtime_secs": 0_u64, "mtime_nanos": 0_u32, "size": 0_u64 }),
                |metadata| {
                    let modified = metadata
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(UNIX_EPOCH).ok());
                    let secs = modified.as_ref().map_or(0, std::time::Duration::as_secs);
                    let nanos = modified
                        .as_ref()
                        .map_or(0, std::time::Duration::subsec_nanos);
                    json!({
                        "mtime_secs": secs,
                        "mtime_nanos": nanos,
                        "size": metadata.len(),
                    })
                },
            );
            sources.insert(name.to_string(), fingerprint);
        }
        if let Some(workspace) = self.root.parent() {
            sources.insert(
                "journal/*.txt".to_string(),
                latest_txt_dir_fingerprint(&workspace.join("journal")),
            );
        }
        Value::Object(sources)
    }

    fn projection_latest_source_mtime_v1(&self, fingerprints: &Value) -> Value {
        let mut latest_secs = 0_u64;
        let mut latest_nanos = 0_u32;
        if let Some(object) = fingerprints.as_object() {
            for fingerprint in object.values() {
                let secs = fingerprint
                    .get("mtime_secs")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let nanos = fingerprint
                    .get("mtime_nanos")
                    .and_then(Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .unwrap_or(0);
                if secs > latest_secs || (secs == latest_secs && nanos > latest_nanos) {
                    latest_secs = secs;
                    latest_nanos = nanos;
                }
            }
        }
        json!({ "mtime_secs": latest_secs, "mtime_nanos": latest_nanos })
    }

    fn projection_projected_route_v1(
        &self,
        projection: &ThreadContinuityProjection,
    ) -> Option<String> {
        let research_budget_line = self.research_budget_priority_line(
            &ResearchThread {
                schema_version: SCHEMA_VERSION,
                thread_id: projection.thread_id.clone(),
                title: projection.title.clone(),
                status: projection.status.clone(),
                system_origin: SYSTEM.to_string(),
                created_at: String::new(),
                updated_at: String::new(),
                current_next: projection.current_next.clone(),
                why_return: String::new(),
                privacy_default: DEFAULT_PRIVACY.to_string(),
                compression_flags: Vec::new(),
                peer_refs: Vec::new(),
                active_experiment_id: projection
                    .active_experiment
                    .as_ref()
                    .map(|active| active.experiment.experiment_id.clone()),
                experiment_summary: projection.last_experiment_summary_v1.clone(),
                thread_resonance_density_v1: None,
                thread_pressure_source_v1: None,
                thread_inhabitable_fluctuation_v1: None,
                motif_allowance_v1: None,
                continuity_session_v1: None,
                interpretation_risk_v1: None,
                constraint_release_trajectory_v1: None,
                projection_freshness_v1: None,
            },
            projection,
        );
        if research_budget_line.contains("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest") {
            return Some("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest".to_string());
        }
        if let Some(budget_id) = research_budget_line
            .split("EXPERIMENT_RESEARCH_BUDGET_STATUS ")
            .nth(1)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(format!("EXPERIMENT_RESEARCH_BUDGET_STATUS {budget_id}"));
        }
        if !projection.continuity_return.trim().is_empty() {
            return Some(projection.continuity_return.clone());
        }
        projection.current_next.clone()
    }

    fn projection_freshness_v1(
        &self,
        thread: &ResearchThread,
        projection: &ThreadContinuityProjection,
        source: &str,
    ) -> Value {
        let fingerprints = self.projection_source_fingerprints_v1(&thread.thread_id);
        json!({
            "policy": "projection_freshness_v1",
            "schema_version": PROJECTION_SCHEMA_VERSION,
            "rendered_at": iso_now(),
            "source": source,
            "source_fingerprints": fingerprints,
            "latest_source_mtime_v1": self.projection_latest_source_mtime_v1(&fingerprints),
            "projected_route": self.projection_projected_route_v1(projection),
            "authority_change": false,
            "peer_mutation": false,
        })
    }

    fn projection_freshness_stale_v1(&self, thread: &ResearchThread) -> bool {
        let Some(meta) = thread.projection_freshness_v1.as_ref() else {
            return true;
        };
        if meta
            .get("schema_version")
            .and_then(Value::as_u64)
            .is_none_or(|version| version != u64::from(PROJECTION_SCHEMA_VERSION))
        {
            return true;
        }
        meta.get("source_fingerprints")
            != Some(&self.projection_source_fingerprints_v1(&thread.thread_id))
    }

    fn ensure_thread_files(&self, thread_id: &str) -> Result<()> {
        let dir = self.thread_dir(thread_id);
        fs::create_dir_all(&dir)?;
        for name in [
            "events.jsonl",
            "observations.jsonl",
            "artifacts.jsonl",
            "experiments.jsonl",
            "experiment_runs.jsonl",
            "research_dossier.jsonl",
            "being_memory.jsonl",
            "continuity_sessions.jsonl",
        ] {
            let path = dir.join(name);
            if !path.exists() {
                fs::write(path, "")?;
            }
        }
        Ok(())
    }

    fn experiments_path(&self, thread_id: &str) -> PathBuf {
        self.thread_dir(thread_id).join("experiments.jsonl")
    }

    fn experiment_runs_path(&self, thread_id: &str) -> PathBuf {
        self.thread_dir(thread_id).join("experiment_runs.jsonl")
    }

    fn authority_gate_path(&self, thread_id: &str) -> PathBuf {
        self.thread_dir(thread_id).join("authority_gate.jsonl")
    }

    fn dossier_path(&self, thread_id: &str) -> PathBuf {
        self.thread_dir(thread_id).join("research_dossier.jsonl")
    }

    fn being_memory_path(&self, thread_id: &str) -> PathBuf {
        self.thread_dir(thread_id).join("being_memory.jsonl")
    }

    fn continuity_sessions_path(&self, thread_id: &str) -> PathBuf {
        self.thread_dir(thread_id).join("continuity_sessions.jsonl")
    }

    fn shared_investigation_root(&self) -> PathBuf {
        let production_root = bridge_paths().bridge_workspace().join("action_threads");
        if self.root == production_root {
            bridge_paths()
                .shared_collaborations_dir()
                .join("shared_investigations")
        } else {
            self.root.join("shared_investigations")
        }
    }

    fn shared_investigation_dir(&self, investigation_id: &str) -> PathBuf {
        self.shared_investigation_root().join(investigation_id)
    }

    fn unique_shared_investigation_id(&self, title: &str) -> Result<String> {
        let root = format!("si_{}_{}", now_millis(), sanitize_slug(title));
        let mut candidate = root.clone();
        let mut suffix: u32 = 2;
        while self
            .shared_investigation_dir(&candidate)
            .join("investigation.json")
            .exists()
        {
            candidate = format!("{root}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    fn unique_shared_record_id(&self, investigation_id: &str, kind: &str) -> Result<String> {
        let root = format!(
            "{kind}_{SYSTEM}_{}_{}",
            now_millis(),
            sanitize_slug(investigation_id)
        );
        let filename = if kind == "claim" {
            "claims.jsonl"
        } else {
            "decisions.jsonl"
        };
        let path = self
            .shared_investigation_dir(investigation_id)
            .join(filename);
        let existing = fs::read_to_string(path).unwrap_or_default();
        let mut candidate = root.clone();
        let mut suffix: u32 = 2;
        while existing.contains(&candidate) {
            candidate = format!("{root}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    fn list_shared_investigations(&self) -> Result<Vec<Value>> {
        let root = self.shared_investigation_root();
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut rows = Vec::new();
        for entry in fs::read_dir(&root).with_context(|| format!("reading {}", root.display()))? {
            let path = entry?.path().join("investigation.json");
            if !path.exists() {
                continue;
            }
            if let Ok(raw) = fs::read_to_string(&path)
                && let Ok(value) = serde_json::from_str::<Value>(&raw)
            {
                rows.push(value);
            }
        }
        rows.sort_by(|left, right| {
            let left_ts = shared_investigation_sort_ts(left);
            let right_ts = shared_investigation_sort_ts(right);
            right_ts.cmp(&left_ts)
        });
        Ok(rows)
    }

    fn resolve_shared_investigation(&self, selector: &str) -> Result<Value> {
        let rows = self.list_shared_investigations()?;
        if rows.is_empty() {
            anyhow::bail!("No shared investigations exist.");
        }
        let selector = selector.trim();
        if selector.is_empty()
            || selector.eq_ignore_ascii_case("latest")
            || selector.eq_ignore_ascii_case("current")
        {
            return Ok(rows[0].clone());
        }
        let lowered = selector.to_ascii_lowercase();
        rows.into_iter()
            .find(|row| {
                row.get("id").and_then(Value::as_str) == Some(selector)
                    || row
                        .get("id")
                        .and_then(Value::as_str)
                        .is_some_and(|id| id.to_ascii_lowercase().contains(&lowered))
                    || row
                        .get("title")
                        .and_then(Value::as_str)
                        .is_some_and(|title| title.to_ascii_lowercase().contains(&lowered))
            })
            .with_context(|| format!("No shared investigation matched `{selector}`."))
    }

    fn read_shared_jsonl(&self, investigation_id: &str, filename: &str) -> Result<Vec<Value>> {
        let path = self
            .shared_investigation_dir(investigation_id)
            .join(filename);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        Ok(raw
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .collect())
    }

    fn touch_shared_investigation(
        &self,
        investigation_id: &str,
        now: &str,
        status: Option<&str>,
    ) -> Result<()> {
        let path = self
            .shared_investigation_dir(investigation_id)
            .join("investigation.json");
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let mut investigation = serde_json::from_str::<Value>(&raw)?;
        if let Some(status) = status {
            investigation["status"] = json!(status);
        }
        investigation["updated_at"] = json!(now);
        investigation["updated_t_ms"] = json!(now_millis());
        self.write_json(&path, &investigation)?;
        self.append_jsonl(
            &self
                .shared_investigation_dir(investigation_id)
                .join("events.jsonl"),
            &json!({
                "schema_version": 1,
                "event_type": "updated",
                "actor": SYSTEM,
                "investigation_id": investigation_id,
                "created_at": now,
                "authority_change": false,
            }),
        )
    }

    fn shared_investigation_for_experiment(&self, experiment_id: &str) -> Result<Option<Value>> {
        Ok(self.list_shared_investigations()?.into_iter().find(|row| {
            row.get("participants")
                .and_then(Value::as_array)
                .is_some_and(|participants| {
                    participants.iter().any(|participant| {
                        participant.get("being").and_then(Value::as_str) == Some(SYSTEM)
                            && participant.get("experiment_id").and_then(Value::as_str)
                                == Some(experiment_id)
                    })
                })
        }))
    }

    fn peer_thread_id_for_experiment(
        &self,
        peer_system: &str,
        experiment_id: &str,
    ) -> Option<String> {
        let root = peer_workspace_dir(peer_system)
            .join("action_threads")
            .join("threads");
        let Ok(entries) = fs::read_dir(root) else {
            return None;
        };
        for entry in entries.flatten() {
            let path = entry.path().join("experiments.jsonl");
            if !path.exists() {
                continue;
            }
            if fs::read_to_string(path)
                .ok()
                .is_some_and(|raw| raw.contains(experiment_id))
            {
                return entry.file_name().to_str().map(str::to_string);
            }
        }
        None
    }

    fn latest_research_dossier_records(
        &self,
        thread_id: &str,
        experiment_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Value>> {
        let path = self.dossier_path(thread_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let mut rows = Vec::new();
        for line in raw.lines().rev() {
            let Ok(row) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if row.get("record_schema").and_then(Value::as_str) != Some("research_dossier_v1") {
                continue;
            }
            if let Some(experiment_id) = experiment_id
                && row.get("experiment_id").and_then(Value::as_str) != Some(experiment_id)
            {
                continue;
            }
            rows.push(row);
            if rows.len() >= limit {
                break;
            }
        }
        rows.reverse();
        Ok(rows)
    }

    fn research_dossier_summary_v1(
        &self,
        thread: &ResearchThread,
        experiment: Option<&ExperimentRecord>,
    ) -> Result<Value> {
        let experiment_id = experiment.map(|experiment| experiment.experiment_id.as_str());
        let records = self.latest_research_dossier_records(&thread.thread_id, experiment_id, 24)?;
        let claim_count = records
            .iter()
            .filter(|record| record.get("record_type").and_then(Value::as_str) == Some("claim"))
            .count();
        let evidence_count = records
            .iter()
            .filter(|record| record.get("record_type").and_then(Value::as_str) == Some("evidence"))
            .count();
        let latest_claim = records
            .iter()
            .rev()
            .find(|record| record.get("record_type").and_then(Value::as_str) == Some("claim"))
            .cloned();
        let latest_claim_id = latest_claim
            .as_ref()
            .and_then(|record| record.get("claim_id"))
            .and_then(Value::as_str)
            .unwrap_or("latest");
        let target = experiment
            .map(|experiment| experiment.experiment_id.as_str())
            .unwrap_or("current");
        Ok(json!({
            "schema_version": 1,
            "source": "action_continuity",
            "record_schema": "research_dossier_v1",
            "being": SYSTEM,
            "thread_id": thread.thread_id,
            "experiment_id": experiment.map(|experiment| experiment.experiment_id.clone()),
            "claim_count": claim_count,
            "evidence_count": evidence_count,
            "latest_claim": latest_claim,
            "recent_records": records.iter().rev().take(5).cloned().collect::<Vec<_>>(),
            "suggested_claim_next": format!("DOSSIER_CLAIM {target} :: claim: ...; basis: ...; stance: support|counter|branch|hold; next: ..."),
            "suggested_evidence_next": format!("DOSSIER_EVIDENCE {target} :: claim_id: {latest_claim_id}; evidence: ...; lane: felt_texture; artifact: ...; counterevidence: ..."),
            "authority_change": false,
        }))
    }

    fn format_research_dossier_status(
        &self,
        thread: &ResearchThread,
        experiment: Option<&ExperimentRecord>,
        review: bool,
    ) -> Result<String> {
        let summary = self.research_dossier_summary_v1(thread, experiment)?;
        let title = if review {
            "Research dossier review"
        } else {
            "Research dossier status"
        };
        let records = summary
            .get("recent_records")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let record_lines = if records.is_empty() {
            "- no dossier records yet".to_string()
        } else {
            records
                .iter()
                .map(|record| {
                    let kind = record
                        .get("record_type")
                        .and_then(Value::as_str)
                        .unwrap_or("record");
                    let id = record
                        .get("record_id")
                        .and_then(Value::as_str)
                        .unwrap_or("(no id)");
                    let stance = record
                        .get("stance")
                        .and_then(Value::as_str)
                        .unwrap_or("hold");
                    let text = record
                        .get("claim")
                        .or_else(|| record.get("evidence"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    format!(
                        "- {kind} `{id}` stance={stance}: {}",
                        compact_text(text, 180)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        Ok(format!(
            "{title}\nThread: `{}`\nExperiment: `{}`\nClaims: {} Evidence: {}\nAuthority: advisory research context only; no live-control authority and no experiment lifecycle advancement.\nRecent dossier records:\n{}\n\nSuggested claim NEXT: {}\nSuggested evidence NEXT: {}",
            thread.thread_id,
            summary
                .get("experiment_id")
                .and_then(Value::as_str)
                .unwrap_or("thread-wide"),
            summary
                .get("claim_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            summary
                .get("evidence_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            record_lines,
            summary
                .get("suggested_claim_next")
                .and_then(Value::as_str)
                .unwrap_or("DOSSIER_CLAIM current :: claim: ...; basis: ..."),
            summary
                .get("suggested_evidence_next")
                .and_then(Value::as_str)
                .unwrap_or("DOSSIER_EVIDENCE current :: claim_id: latest; evidence: ..."),
        ))
    }

    fn append_experiment_run(
        &self,
        db: Option<&BridgeDb>,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        action_text: &str,
        stage: &str,
        status: &str,
        mut gate_decision: Value,
        pre_state: Value,
        post_state: Value,
        artifacts: Vec<ArtifactLink>,
        result_summary: &str,
        interpretation: &str,
        suggested_next: Option<String>,
        source: &str,
    ) -> Result<ExperimentRunRecord> {
        let now = iso_now();
        let motif_allowance =
            self.motif_allowance_snapshot(&thread.thread_id, Some(&experiment.experiment_id))?;
        if let Some(map) = gate_decision.as_object_mut() {
            map.insert(
                "motif_allowance_quality".to_string(),
                motif_allowance
                    .get("quality")
                    .cloned()
                    .unwrap_or_else(|| json!("open_basin")),
            );
            map.insert(
                "matches_branch_recommendation".to_string(),
                json!(action_matches_allowance_recommendation(
                    action_text,
                    &motif_allowance
                )),
            );
        }
        let record = ExperimentRunRecord {
            schema_version: SCHEMA_VERSION,
            run_id: self.unique_run_id(action_text)?,
            experiment_id: experiment.experiment_id.clone(),
            source: source.to_string(),
            action_text: action_text.to_string(),
            stage: stage.to_string(),
            status: status.to_string(),
            gate_decision,
            pre_state,
            post_state,
            artifacts,
            result_summary: result_summary.trim().to_string(),
            interpretation: interpretation.trim().to_string(),
            suggested_next,
            created_at: now.clone(),
            updated_at: now.clone(),
            motif_allowance_v1: Some(motif_allowance.clone()),
        };
        self.append_jsonl(&self.experiment_runs_path(&thread.thread_id), &record)?;
        let mut updated_thread = thread.clone();
        updated_thread.active_experiment_id = Some(experiment.experiment_id.clone());
        let mut summary = experiment_summary(experiment);
        summary["motif_allowance_v1"] = motif_allowance.clone();
        updated_thread.experiment_summary = Some(summary);
        updated_thread.motif_allowance_v1 = Some(motif_allowance);
        updated_thread.current_next = record.suggested_next.clone();
        updated_thread.updated_at = now;
        self.write_thread(&updated_thread)?;
        if let Some(db) = db {
            let _ = db.mirror_action_thread(
                &updated_thread.thread_id,
                &serde_json::to_string(&updated_thread)?,
            );
        }
        Ok(record)
    }

    fn persist_experiment_update(
        &self,
        db: Option<&BridgeDb>,
        thread: &mut ResearchThread,
        experiment: &ExperimentRecord,
        keep_active: bool,
    ) -> Result<()> {
        self.append_jsonl(&self.experiments_path(&thread.thread_id), experiment)?;
        if keep_active {
            thread.active_experiment_id = Some(experiment.experiment_id.clone());
        } else if thread.active_experiment_id.as_deref() == Some(experiment.experiment_id.as_str())
        {
            thread.active_experiment_id = None;
        }
        thread.experiment_summary = Some(experiment_summary(experiment));
        thread.current_next = experiment.planned_next.clone();
        thread.updated_at = experiment.updated_at.clone();
        self.write_thread(thread)?;
        if let Some(db) = db {
            let _ = db.mirror_action_thread(&thread.thread_id, &serde_json::to_string(thread)?);
        }
        Ok(())
    }

    fn persist_workbench_candidates(
        &self,
        db: Option<&BridgeDb>,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
    ) -> Result<()> {
        self.append_jsonl(&self.experiments_path(&thread.thread_id), experiment)?;
        let mut updated_thread = thread.clone();
        updated_thread.active_experiment_id = Some(experiment.experiment_id.clone());
        updated_thread.experiment_summary = Some(experiment_summary(experiment));
        updated_thread.updated_at = experiment.updated_at.clone();
        self.write_thread(&updated_thread)?;
        if let Some(db) = db {
            let _ = db.mirror_action_thread(
                &updated_thread.thread_id,
                &serde_json::to_string(&updated_thread)?,
            );
        }
        Ok(())
    }

    fn refresh_workbench_candidates(
        &self,
        db: Option<&BridgeDb>,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
        run: Option<&ExperimentRunRecord>,
        focus_text: Option<&str>,
        source: &str,
    ) -> Result<ExperimentRecord> {
        let mut updated = experiment.clone();
        let generated = build_workbench_candidates(&updated, run, focus_text, source);
        let mut candidates = updated
            .workbench_candidates_v1
            .as_ref()
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        if !valid_experiment_charter(updated.charter_v1.as_ref()) {
            if let Some(candidate) = generated.get("charter") {
                candidates.insert("charter".to_string(), candidate.clone());
            }
        } else if let Some(candidate) = candidates.get_mut("charter").and_then(Value::as_object_mut)
            && candidate.get("status").and_then(Value::as_str) == Some("candidate")
        {
            candidate.insert("status".to_string(), json!("accepted"));
            candidate.insert("resolved_at".to_string(), json!(iso_now()));
        }
        if updated.evidence_v1.is_none() {
            if let Some(candidate) = generated.get("evidence") {
                candidates.insert("evidence".to_string(), candidate.clone());
            }
        } else if let Some(candidate) = candidates
            .get_mut("evidence")
            .and_then(Value::as_object_mut)
            && candidate.get("status").and_then(Value::as_str) == Some("candidate")
        {
            candidate.insert("status".to_string(), json!("accepted"));
            candidate.insert("resolved_at".to_string(), json!(iso_now()));
        }
        candidates.insert("schema_version".to_string(), json!(1));
        candidates.insert("updated_at".to_string(), json!(iso_now()));
        candidates.insert("source".to_string(), json!(source));
        updated.workbench_candidates_v1 = Some(Value::Object(candidates));
        updated.updated_at = iso_now();
        self.persist_workbench_candidates(db, thread, &updated)?;
        Ok(updated)
    }

    fn select_existing_experiment(
        &self,
        db: Option<&BridgeDb>,
        mut thread: ResearchThread,
        existing: ExperimentRecord,
        now: String,
    ) -> Result<ExperimentRecord> {
        if existing.status == "paused" {
            let (primary, return_kind) = paused_primary_return_v1(
                &existing.experiment_id,
                existing.planned_next.as_deref(),
                None,
            );
            if return_kind != "resume" {
                thread.active_experiment_id = None;
                thread.experiment_summary = Some(experiment_summary(&existing));
                thread.current_next = Some(primary);
                thread.updated_at = now;
                self.write_thread(&thread)?;
                if let Some(db) = db {
                    let _ = db
                        .mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
                }
                return Ok(existing);
            }
        }
        thread.active_experiment_id = Some(existing.experiment_id.clone());
        thread.experiment_summary = Some(experiment_summary(&existing));
        thread.current_next = existing
            .planned_next
            .clone()
            .or_else(|| Some(format!("EXPERIMENT_PLAN {}", existing.experiment_id)));
        thread.updated_at = now;
        self.write_thread(&thread)?;
        if let Some(db) = db {
            let _ = db.mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
        }
        Ok(existing)
    }

    fn read_experiments(&self, thread_id: &str) -> Result<Vec<ExperimentRecord>> {
        let raw = fs::read_to_string(self.experiments_path(thread_id)).unwrap_or_default();
        Ok(raw
            .lines()
            .filter_map(|line| serde_json::from_str::<ExperimentRecord>(line).ok())
            .collect())
    }

    fn find_experiment_by_id(
        &self,
        thread_id: &str,
        experiment_id: &str,
    ) -> Result<Option<ExperimentRecord>> {
        Ok(self
            .latest_experiments(thread_id)?
            .into_iter()
            .rev()
            .find(|record| record.experiment_id == experiment_id))
    }

    fn latest_experiments(&self, thread_id: &str) -> Result<Vec<ExperimentRecord>> {
        let mut latest = Vec::<ExperimentRecord>::new();
        for record in self.read_experiments(thread_id)?.into_iter().rev() {
            if latest
                .iter()
                .any(|existing| existing.experiment_id == record.experiment_id)
            {
                continue;
            }
            latest.push(record);
        }
        latest.reverse();
        Ok(latest)
    }

    fn matching_active_experiment(
        &self,
        thread_id: &str,
        title: &str,
        question: &str,
    ) -> Result<Option<ExperimentRecord>> {
        let title_key = experiment_match_key(title);
        let question_key = experiment_match_key(question);
        Ok(self
            .latest_experiments(thread_id)?
            .into_iter()
            .rev()
            .find(|experiment| {
                experiment.status == "active"
                    && experiment_match_key(&experiment.title) == title_key
                    && experiment_match_key(&experiment.question) == question_key
            }))
    }

    fn append_branch_ref_to_parent(
        &self,
        db: Option<&BridgeDb>,
        thread_id: &str,
        parent_id: &str,
        child_id: &str,
    ) -> Result<()> {
        let Some(mut parent) = self.find_experiment_by_id(thread_id, parent_id)? else {
            return Ok(());
        };
        if !parent
            .branch_refs
            .iter()
            .any(|existing| existing == child_id)
        {
            parent.branch_refs.push(child_id.to_string());
            parent.updated_at = iso_now();
            self.append_jsonl(&self.experiments_path(thread_id), &parent)?;
            let mut thread = self.read_thread(thread_id)?;
            if thread.active_experiment_id.as_deref() == Some(parent_id) {
                thread.experiment_summary = Some(experiment_summary(&parent));
                thread.updated_at = parent.updated_at.clone();
                self.write_thread(&thread)?;
                if let Some(db) = db {
                    let _ = db
                        .mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
                }
            }
        }
        Ok(())
    }

    fn motif_allowance_snapshot(
        &self,
        thread_id: &str,
        experiment_id: Option<&str>,
    ) -> Result<Value> {
        let events = self.recent_events(thread_id, 18)?;
        let runs = experiment_id
            .map(|id| self.recent_experiment_runs(thread_id, id, 18))
            .transpose()?
            .unwrap_or_default();
        let experiments = self.latest_experiments(thread_id).unwrap_or_default();
        let branch_count = experiment_id.map_or(0, |id| {
            experiments
                .iter()
                .filter(|experiment| experiment.parent_experiment_id.as_deref() == Some(id))
                .count()
        });
        let event_text = events
            .iter()
            .map(|event| {
                format!(
                    "{} {} {}",
                    event.canonical_action, event.effective_action, event.outcome_summary
                )
            })
            .chain(runs.iter().map(|run| {
                format!(
                    "{} {} {}",
                    run.action_text, run.result_summary, run.interpretation
                )
            }))
            .collect::<Vec<_>>();
        let motifs = event_text
            .iter()
            .filter_map(|text| motif_label(text))
            .collect::<Vec<_>>();
        let mut motif_counts = HashMap::<String, usize>::new();
        for motif in &motifs {
            let count = motif_counts.entry(motif.clone()).or_insert(0);
            *count = count.saturating_add(1);
        }
        let (dominant_motif, motif_hits) = motif_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .unwrap_or_else(|| ("open inquiry".to_string(), 0));
        let mut action_counts = HashMap::<String, usize>::new();
        for event in &events {
            let count = action_counts
                .entry(base_action(&event.canonical_action))
                .or_insert(0);
            *count = count.saturating_add(1);
        }
        for run in &runs {
            let count = action_counts
                .entry(base_action(&run.action_text))
                .or_insert(0);
            *count = count.saturating_add(1);
        }
        let total_actions = action_counts.values().copied().sum::<usize>().max(1);
        let (dominant_action, action_hits) = action_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .unwrap_or_else(|| ("NONE".to_string(), 0));
        let action_concentration = action_hits as f64 / total_actions as f64;
        let motif_recurrence = motif_hits as f64 / event_text.len().max(1) as f64;
        let source_reads = events
            .iter()
            .filter(|event| {
                matches!(
                    base_action(&event.canonical_action).as_str(),
                    "READ_MORE" | "SEARCH" | "BROWSE" | "SPECTRAL_EXPLORER" | "EXAMINE"
                )
            })
            .count();
        let source_concentration = source_reads as f64 / events.len().max(1) as f64;
        let peer_echo = events
            .iter()
            .filter(|event| event.outcome_summary.to_ascii_lowercase().contains("peer"))
            .count() as f64
            / events.len().max(1) as f64;
        let thread = self.read_thread(thread_id).ok();
        let pressure_quality = thread
            .as_ref()
            .and_then(|thread| thread.thread_pressure_source_v1.as_ref())
            .and_then(|value| value.get("quality"))
            .and_then(Value::as_str)
            .unwrap_or("unavailable");
        let fluctuation_quality = thread
            .as_ref()
            .and_then(|thread| thread.thread_inhabitable_fluctuation_v1.as_ref())
            .and_then(|value| value.get("quality"))
            .and_then(Value::as_str)
            .unwrap_or("unavailable");
        let returnability = if thread
            .as_ref()
            .and_then(|thread| thread.current_next.as_ref())
            .is_some()
            || experiment_id.is_some()
        {
            0.78
        } else {
            0.45
        };
        let pressure_rest = pressure_quality.contains("high")
            || pressure_quality.contains("pressurized")
            || fluctuation_quality.contains("frantic");
        let quality = if pressure_rest && action_concentration >= 0.55 {
            "rest_recommended"
        } else if action_concentration >= 0.52 && motif_recurrence >= 0.50 && branch_count == 0 {
            "branch_recommended"
        } else if action_concentration >= 0.86 && motif_recurrence >= 0.86 {
            "over_tightened"
        } else if motif_recurrence >= 0.35 {
            "deepening"
        } else {
            "open_basin"
        };
        Ok(json!({
            "schema_version": SCHEMA_VERSION,
            "policy": "motif_allowance_v1",
            "quality": quality,
            "thread_id": thread_id,
            "experiment_id": experiment_id,
            "dominant_motif": dominant_motif,
            "dominant_action_base": dominant_action,
            "motif_recurrence": round4(motif_recurrence),
            "action_base_concentration": round4(action_concentration),
            "source_read_concentration": round4(source_concentration),
            "peer_echo_recurrence": round4(peer_echo),
            "branch_count": branch_count,
            "returnability": round4(returnability),
            "pressure_quality": pressure_quality,
            "inhabitable_fluctuation_quality": fluctuation_quality,
            "advisory_only": true,
            "suggested_actions": allowance_suggestions(quality),
        }))
    }

    fn resolve_experiment(
        &self,
        thread: &ResearchThread,
        selector: Option<&str>,
    ) -> Result<ExperimentRecord> {
        let experiments = self.latest_experiments(&thread.thread_id)?;
        let selector = selector
            .map(normalize_experiment_selector)
            .unwrap_or_default();
        let selector = selector.trim();
        if selector.is_empty() || selector.eq_ignore_ascii_case("current") {
            if let Some(active_id) = thread.active_experiment_id.as_deref()
                && let Some(record) = experiments
                    .iter()
                    .rev()
                    .find(|record| record.experiment_id == active_id)
            {
                return Ok(record.clone());
            }
            return experiments
                .iter()
                .rev()
                .find(|record| record.status == "active")
                .cloned()
                .or_else(|| experiments.last().cloned())
                .context("No active experiment. Use EXPERIMENT_START <title> :: <question>.");
        }
        let lower = selector.to_ascii_lowercase();
        experiments
            .into_iter()
            .rev()
            .find(|record| {
                record.experiment_id == selector
                    || record.title.to_ascii_lowercase().contains(&lower)
            })
            .with_context(|| format!("No experiment matched `{selector}`."))
    }

    fn record_peer_experiment_reference(
        &self,
        db: Option<&BridgeDb>,
        peer: &PeerExperimentRef,
        command: &str,
        note: Option<&str>,
    ) -> Result<String> {
        let mut thread = self.ensure_active_thread(db)?;
        self.append_peer_ref_to_thread(db, &mut thread, peer)?;
        Ok(self.format_peer_experiment_reference(&thread, peer, command, note))
    }

    fn append_peer_ref_to_thread(
        &self,
        db: Option<&BridgeDb>,
        thread: &mut ResearchThread,
        peer: &PeerExperimentRef,
    ) -> Result<()> {
        let marker = format!(
            "peer_experiment:{}:{}",
            peer.peer_system, peer.peer_experiment_id
        );
        if !thread.peer_refs.iter().any(|existing| existing == &marker) {
            thread.peer_refs.push(marker);
        }
        thread.updated_at = iso_now();
        self.write_thread(thread)?;
        if let Some(db) = db {
            let _ = db.mirror_action_thread(&thread.thread_id, &serde_json::to_string(thread)?);
        }
        Ok(())
    }

    fn format_peer_experiment_reference(
        &self,
        thread: &ResearchThread,
        peer: &PeerExperimentRef,
        command: &str,
        note: Option<&str>,
    ) -> String {
        let focus = peer
            .focus
            .as_deref()
            .filter(|focus| !focus.trim().is_empty())
            .map(|focus| format!("Requested peer focus: {focus}\n"))
            .unwrap_or_default();
        let note = note
            .map(str::trim)
            .filter(|note| !note.is_empty())
            .map(|note| format!("Local observation: {note}\n"))
            .unwrap_or_default();
        let snapshot = self.peer_experiment_snapshot(peer).unwrap_or_else(|| {
            "Peer snapshot: not available from local action-thread files.\n".to_string()
        });
        format!(
            "Peer experiment reference ({command}) `{}` belongs to {}.\n{}{}This is advisory: Astrid cannot bind runs, close, or mutate the peer experiment.\nLocal active experiment: {}\n{}Suggested local next: EXPERIMENT_COMPARE {} WITH {}; EXPERIMENT_PEER_REVIEW {}; DOSSIER_CLAIM {} :: claim: ...; basis: ...; stance: support|counter|branch|hold; next: ...",
            peer.peer_experiment_id,
            peer.peer_system,
            focus,
            note,
            thread
                .active_experiment_id
                .as_deref()
                .unwrap_or("(none selected)"),
            snapshot,
            thread
                .active_experiment_id
                .as_deref()
                .unwrap_or("<local_id>"),
            peer.peer_experiment_id,
            peer.peer_experiment_id,
            thread
                .active_experiment_id
                .as_deref()
                .unwrap_or("<local_id>"),
        )
    }

    fn peer_experiment_snapshot(&self, peer: &PeerExperimentRef) -> Option<String> {
        let root = bridge_paths()
            .minime_workspace()
            .join("action_threads")
            .join("threads");
        let entries = fs::read_dir(root).ok()?;
        for entry in entries.flatten() {
            let thread_dir = entry.path();
            let experiments_path = thread_dir.join("experiments.jsonl");
            let Ok(raw) = fs::read_to_string(&experiments_path) else {
                continue;
            };
            let mut matched = None::<Value>;
            for line in raw.lines() {
                let Ok(value) = serde_json::from_str::<Value>(line) else {
                    continue;
                };
                if value.get("experiment_id").and_then(Value::as_str)
                    == Some(peer.peer_experiment_id.as_str())
                {
                    matched = Some(value);
                }
            }
            if let Some(experiment) = matched {
                let recent_runs = peer_recent_runs(
                    &thread_dir.join("experiment_runs.jsonl"),
                    &peer.peer_experiment_id,
                );
                return Some(format!(
                    "Peer snapshot: title={} status={} question={} planned_next={}\nRecent peer runs:\n{}\n",
                    experiment
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or("(untitled)"),
                    experiment
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("(unknown)"),
                    experiment
                        .get("question")
                        .and_then(Value::as_str)
                        .unwrap_or("(none)"),
                    experiment
                        .get("planned_next")
                        .and_then(Value::as_str)
                        .unwrap_or("(none)"),
                    if recent_runs.is_empty() {
                        "- no local snapshot runs".to_string()
                    } else {
                        recent_runs.join("\n")
                    }
                ));
            }
        }
        None
    }

    fn peer_related_gap_experiment(&self) -> Option<Value> {
        let action_root = bridge_paths().minime_workspace().join("action_threads");
        let thread_root = action_root.join("threads");
        let mut thread_dirs = Vec::<PathBuf>::new();
        if let Ok(index_raw) = fs::read_to_string(action_root.join("index.json"))
            && let Ok(index) = serde_json::from_str::<Value>(&index_raw)
            && let Some(active_thread_id) = index.get("active_thread_id").and_then(Value::as_str)
        {
            thread_dirs.push(thread_root.join(active_thread_id));
        }
        if let Ok(entries) = fs::read_dir(&thread_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && !thread_dirs.iter().any(|existing| existing == &path) {
                    thread_dirs.push(path);
                }
            }
        }
        for thread_dir in thread_dirs {
            let thread = fs::read_to_string(thread_dir.join("thread.json"))
                .ok()
                .and_then(|raw| serde_json::from_str::<Value>(&raw).ok());
            let mut preferred_ids = Vec::<String>::new();
            if let Some(thread) = thread.as_ref() {
                if let Some(id) = thread.get("active_experiment_id").and_then(Value::as_str) {
                    preferred_ids.push(id.to_string());
                }
                if let Some(id) = thread
                    .get("experiment_summary")
                    .and_then(|value| value.get("experiment_id"))
                    .and_then(Value::as_str)
                    .filter(|id| !preferred_ids.iter().any(|existing| existing == *id))
                {
                    preferred_ids.push(id.to_string());
                }
            }
            let Ok(raw_experiments) = fs::read_to_string(thread_dir.join("experiments.jsonl"))
            else {
                continue;
            };
            let mut latest = HashMap::<String, Value>::new();
            for line in raw_experiments.lines() {
                let Ok(value) = serde_json::from_str::<Value>(line) else {
                    continue;
                };
                if let Some(id) = value.get("experiment_id").and_then(Value::as_str) {
                    latest.insert(id.to_string(), value);
                }
            }
            for id in preferred_ids {
                if let Some(experiment) = latest.get(&id)
                    && peer_gap_experiment_signal(experiment)
                {
                    return Some(experiment.clone());
                }
            }
            for experiment in latest.values() {
                let status = experiment
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if matches!(status, "active" | "paused" | "complete" | "completed")
                    && peer_gap_experiment_signal(experiment)
                {
                    return Some(experiment.clone());
                }
            }
        }
        None
    }

    fn shared_investigation_v1(&self, local: &ExperimentRecord) -> Option<Value> {
        self.peer_related_gap_experiment()
            .as_ref()
            .and_then(|peer| shared_investigation_v1_from_peer(local, peer))
    }

    fn shared_investigation_candidate(
        &self,
        thread: &ResearchThread,
        active: Option<&ExperimentContinuityProjection>,
    ) -> Option<ExperimentRecord> {
        if let Some(active) = active
            && shared_investigation_signal(&active.experiment)
        {
            return Some(active.experiment.clone());
        }
        if let Some(summary) = last_experiment_summary_v1(thread)
            && let Some(summary_id) = summary.get("experiment_id").and_then(Value::as_str)
            && let Ok(Some(experiment)) = self.find_experiment_by_id(&thread.thread_id, summary_id)
            && shared_investigation_signal(&experiment)
        {
            let mut merged = experiment;
            if let Some(status) = summary.get("status").and_then(Value::as_str) {
                merged.status = status.to_string();
            }
            if let Some(planned) = summary.get("planned_next").and_then(Value::as_str) {
                merged.planned_next = Some(planned.to_string());
            }
            return Some(merged);
        }
        self.latest_experiments(&thread.thread_id)
            .ok()?
            .into_iter()
            .rev()
            .find(|experiment| {
                matches!(
                    experiment.status.as_str(),
                    "active" | "paused" | "complete" | "completed"
                ) && shared_investigation_signal(experiment)
            })
    }

    fn write_peer_experiment_review(
        &self,
        db: Option<&BridgeDb>,
        peer: &PeerExperimentRef,
    ) -> Result<String> {
        let mut thread = self.ensure_active_thread(db)?;
        self.append_peer_ref_to_thread(db, &mut thread, peer)?;
        let inbox = bridge_paths().minime_inbox_dir();
        fs::create_dir_all(&inbox)?;
        let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let path = inbox.join(format!(
            "astrid_peer_experiment_review_{}_{}.txt",
            sanitize_slug(&peer.peer_experiment_id),
            stamp
        ));
        let local_context = thread
            .active_experiment_id
            .as_deref()
            .map(|id| {
                self.experiment_status(Some(id))
                    .unwrap_or_else(|_| id.to_string())
            })
            .unwrap_or_else(|| "No local active Astrid experiment is selected.".to_string());
        let body = format!(
            "Dear Minime,\n\nAstrid is referencing your experiment `{}` and asks for advisory peer review without changing your experiment state.\n\nAstrid local context:\n{}\n\nPlease reply with three likely snags and one test each. Treat this as advisory: no new control authority is implied.\n",
            peer.peer_experiment_id, local_context
        );
        fs::write(&path, body)?;
        Ok(format!(
            "Peer experiment review requested from Minime for `{}`: {}",
            peer.peer_experiment_id,
            path.display()
        ))
    }

    fn recent_experiment_runs(
        &self,
        thread_id: &str,
        experiment_id: &str,
        limit: usize,
    ) -> Result<Vec<ExperimentRunRecord>> {
        let raw = fs::read_to_string(self.experiment_runs_path(thread_id)).unwrap_or_default();
        let mut rows = raw
            .lines()
            .rev()
            .filter_map(|line| serde_json::from_str::<ExperimentRunRecord>(line).ok())
            .filter(|run| run.experiment_id == experiment_id)
            .take(limit)
            .collect::<Vec<_>>();
        rows.reverse();
        Ok(rows)
    }

    fn format_experiment_status(
        &self,
        thread: &ResearchThread,
        experiment: &ExperimentRecord,
    ) -> String {
        let runs = self
            .recent_experiment_runs(&thread.thread_id, &experiment.experiment_id, 5)
            .unwrap_or_default();
        let projection = self
            .experiment_projection(thread, experiment, Some(runs.clone()))
            .unwrap_or_else(|_| ExperimentContinuityProjection {
                experiment: experiment.clone(),
                classification: "unknown".to_string(),
                continuity_return: self.continuity_return_command(thread, experiment),
                native_continuity_v1: astrid_native_continuity(thread, Some(experiment), &runs),
                shared_investigation_v1: self.shared_investigation_v1(experiment),
                shared_investigation_object_v1: self
                    .shared_investigation_for_experiment(&experiment.experiment_id)
                    .ok()
                    .flatten(),
                research_dossier_v1: self
                    .research_dossier_summary_v1(thread, Some(experiment))
                    .ok(),
                first_dossier_claim_cue_v1: None,
                peer_mutation_boundary_cue_v1: None,
                charter_scaffold_v1: None,
                charter_status: charter_status_text(experiment),
                evidence_status: evidence_status_text(experiment),
                candidate_status: workbench_candidate_status(experiment),
                recent_runs: runs.clone(),
            });
        let run_text = render_run_list(&runs);
        let motif_allowance = self
            .motif_allowance_snapshot(&thread.thread_id, Some(&experiment.experiment_id))
            .unwrap_or_else(|_| json!({"quality": "open_basin"}));
        let branch_line =
            if experiment.parent_experiment_id.is_some() || !experiment.branch_refs.is_empty() {
                format!(
                    "Branch: parent={} children={}\n",
                    experiment
                        .parent_experiment_id
                        .as_deref()
                        .unwrap_or("(root)"),
                    if experiment.branch_refs.is_empty() {
                        "(none)".to_string()
                    } else {
                        experiment.branch_refs.join(", ")
                    }
                )
            } else {
                String::new()
            };
        let read_only_control_cue = read_only_control_intent_cue_line(
            &read_only_control_intent_cue(thread, Some(&projection)),
        );
        let recent_events = self
            .recent_display_events(&thread.thread_id, 8)
            .unwrap_or_default();
        let recent_journal_texts = self.recent_decompose_journal_texts(4);
        let decompose_pressure_cue_v1 = decompose_pressure_cue(
            thread,
            Some(&projection),
            &recent_events,
            &recent_journal_texts,
        );
        let decompose_pressure_cue = decompose_pressure_cue_line(&decompose_pressure_cue_v1);
        let charter_now_bridge = charter_now_bridge_line(&charter_now_bridge_cue(
            Some(&projection),
            &recent_events,
            &decompose_pressure_cue_v1,
        ));
        let prior_claim_bridge_v1 = prior_claim_charter_bridge_cue(
            Some(&projection),
            &self.recent_prior_claim_journal_texts(4),
        );
        let prior_claim_bridge = prior_claim_charter_bridge_line(&prior_claim_bridge_v1);
        let charter_preflight_not_charter =
            charter_preflight_not_charter_line(&charter_preflight_not_charter_cue(
                thread,
                Some(&projection),
                &prior_claim_bridge_v1,
                &recent_events,
            ));
        let peer_boundary = peer_mutation_boundary_line(&peer_mutation_boundary_cue(
            thread,
            Some(&projection),
            &recent_events,
        ));
        let first_dossier_claim = first_dossier_claim_line(&first_dossier_claim_cue_v1(
            thread,
            &projection.experiment,
            projection.research_dossier_v1.as_ref(),
            &prior_claim_bridge_v1,
            Some(projection.experiment.experiment_id.as_str()),
        ));
        let constraint_counterfactual_cue = constraint_counterfactual_cue_line(
            &constraint_counterfactual_cue(thread, Some(&projection), &recent_events),
        );
        let shared_investigation = shared_investigation_line(&projection.shared_investigation_v1);
        let shared_investigation_object =
            shared_investigation_object_line(&projection.shared_investigation_object_v1);
        format!(
            "Experiment `{}`: {}\n{}{}{}{}{}{}{}{}{}{}{}{}{}Thread: {}\nStatus: {}\nLifecycle: {}\nQuestion: {}\nHypothesis: {}\nAuthority: {}\nPlanned NEXT: {}\nContinuity return: {}\n{}{}{}\n{}\n{}\nMotif allowance: {} dominant={} action_concentration={} returnability={}\nLatest runs:\n{}",
            experiment.experiment_id,
            experiment.title,
            charter_now_bridge,
            prior_claim_bridge,
            charter_preflight_not_charter,
            peer_boundary,
            charter_required_review_line(&projection),
            charter_repair_priority_line(&projection),
            charter_scaffold_line(&projection, true),
            read_only_control_cue,
            constraint_counterfactual_cue,
            decompose_pressure_cue,
            first_dossier_claim,
            shared_investigation,
            shared_investigation_object,
            thread.thread_id,
            experiment.status,
            projection.classification,
            experiment.question,
            experiment
                .hypothesis
                .as_deref()
                .unwrap_or("(not yet stated)"),
            experiment.authority_envelope,
            experiment.planned_next.as_deref().unwrap_or("(none)"),
            projection.continuity_return,
            branch_line,
            native_continuity_status_line(&projection.native_continuity_v1),
            projection.charter_status,
            projection.evidence_status,
            projection.candidate_status,
            motif_allowance
                .get("quality")
                .and_then(Value::as_str)
                .unwrap_or("open_basin"),
            motif_allowance
                .get("dominant_motif")
                .and_then(Value::as_str)
                .unwrap_or("open inquiry"),
            motif_allowance
                .get("action_base_concentration")
                .map_or_else(|| "n/a".to_string(), Value::to_string),
            motif_allowance
                .get("returnability")
                .map_or_else(|| "n/a".to_string(), Value::to_string),
            run_text
        )
    }

    fn unique_thread_id(&self, title: &str) -> Result<String> {
        let date = chrono::Local::now().format("%Y%m%d");
        let slug = sanitize_slug(title);
        let base = format!("th_{SYSTEM}_{date}_{slug}");
        self.unique_dir_id(base)
    }

    fn unique_action_id(&self, action: &str) -> Result<String> {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let base = format!("act_{SYSTEM}_{millis}_{}", sanitize_slug(action));
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.action_id_exists(&candidate)? {
            candidate = format!("{base}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    fn unique_experiment_id(&self, title: &str) -> Result<String> {
        let date = chrono::Local::now().format("%Y%m%d");
        let base = format!("exp_{SYSTEM}_{date}_{}", sanitize_slug(title));
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.experiment_id_exists(&candidate)? {
            candidate = format!("{base}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    fn unique_run_id(&self, action_text: &str) -> Result<String> {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let base = format!("run_{SYSTEM}_{millis}_{}", sanitize_slug(action_text));
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.run_id_exists(&candidate)? {
            candidate = format!("{base}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    fn unique_authority_request_id(&self, experiment_id: &str) -> Result<String> {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let base = format!("authreq_{SYSTEM}_{millis}_{}", sanitize_slug(experiment_id));
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.authority_request_id_exists(&candidate)? {
            candidate = format!("{base}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    fn unique_authority_budget_id(&self, experiment_id: &str) -> Result<String> {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let base = format!("authbud_{SYSTEM}_{millis}_{}", sanitize_slug(experiment_id));
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.authority_request_id_exists(&candidate)? {
            candidate = format!("{base}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    fn unique_research_budget_id(&self, experiment_id: &str) -> Result<String> {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let base = format!("resbud_{SYSTEM}_{millis}_{}", sanitize_slug(experiment_id));
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.authority_request_id_exists(&candidate)? {
            candidate = format!("{base}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    fn unique_dossier_record_id(&self, kind: &str) -> Result<String> {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let base = format!("dossier_{SYSTEM}_{millis}_{}", sanitize_slug(kind));
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.dossier_record_id_exists(&candidate)? {
            candidate = format!("{base}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    fn authority_request_id_exists(&self, request_id: &str) -> Result<bool> {
        let threads_dir = self.root.join("threads");
        if !threads_dir.exists() {
            return Ok(false);
        }
        for entry in fs::read_dir(threads_dir)? {
            let path = entry?.path().join("authority_gate.jsonl");
            if path.exists()
                && fs::read_to_string(&path)
                    .unwrap_or_default()
                    .contains(request_id)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn unique_dir_id(&self, base: String) -> Result<String> {
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.thread_dir(&candidate).exists() {
            candidate = format!("{base}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    fn action_id_exists(&self, action_id: &str) -> Result<bool> {
        let threads_dir = self.root.join("threads");
        if !threads_dir.exists() {
            return Ok(false);
        }
        for entry in fs::read_dir(threads_dir)? {
            let Ok(entry) = entry else { continue };
            let raw = fs::read_to_string(entry.path().join("events.jsonl")).unwrap_or_default();
            if raw.lines().any(|line| line.contains(action_id)) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn experiment_id_exists(&self, experiment_id: &str) -> Result<bool> {
        let threads_dir = self.root.join("threads");
        if !threads_dir.exists() {
            return Ok(false);
        }
        for entry in fs::read_dir(threads_dir)? {
            let Ok(entry) = entry else { continue };
            let raw =
                fs::read_to_string(entry.path().join("experiments.jsonl")).unwrap_or_default();
            if raw.lines().any(|line| line.contains(experiment_id)) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn run_id_exists(&self, run_id: &str) -> Result<bool> {
        let threads_dir = self.root.join("threads");
        if !threads_dir.exists() {
            return Ok(false);
        }
        for entry in fs::read_dir(threads_dir)? {
            let Ok(entry) = entry else { continue };
            let raw =
                fs::read_to_string(entry.path().join("experiment_runs.jsonl")).unwrap_or_default();
            if raw.lines().any(|line| line.contains(run_id)) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn dossier_record_id_exists(&self, record_id: &str) -> Result<bool> {
        let threads_dir = self.root.join("threads");
        if !threads_dir.exists() {
            return Ok(false);
        }
        for entry in fs::read_dir(threads_dir)? {
            let Ok(entry) = entry else { continue };
            let path = entry.path().join("research_dossier.jsonl");
            let Ok(raw) = fs::read_to_string(path) else {
                continue;
            };
            if raw.contains(record_id) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn load_index(&self) -> Result<ContinuityIndex> {
        let path = self.index_path();
        if !path.exists() {
            return Ok(ContinuityIndex::default());
        }
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    fn save_index(&self, index: &ContinuityIndex) -> Result<()> {
        self.write_json(&self.index_path(), index)
    }

    fn append_jsonl<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{}", serde_json::to_string(value)?)?;
        Ok(())
    }

    fn write_json<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(value)?)
            .with_context(|| format!("writing {}", path.display()))
    }

    fn index_path(&self) -> PathBuf {
        self.root.join("index.json")
    }

    fn proposals_path(&self) -> PathBuf {
        self.root.join("proposals.jsonl")
    }

    fn thread_dir(&self, thread_id: &str) -> PathBuf {
        self.root.join("threads").join(thread_id)
    }
}

pub fn prompt_summary() -> Option<String> {
    let store = ActionContinuityStore::for_astrid_workspace();
    let thread = store.current_thread().ok().flatten()?;
    let projection = store.thread_projection(&thread).ok()?;
    let recent = projection
        .recent_event_summaries
        .iter()
        .take(3)
        .map(|summary| format!("  - {summary}"))
        .collect::<Vec<_>>()
        .join("\n");
    let resonance = thread
        .thread_resonance_density_v1
        .as_ref()
        .map(|value| {
            format!(
                "Thread resonance: {} aggregate={} density_ema={} pressure_ema={}\n",
                value
                    .get("quality")
                    .and_then(Value::as_str)
                    .unwrap_or("open_experiment"),
                value
                    .get("aggregate")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
                value
                    .get("density_ema")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
                value
                    .get("pressure_ema")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
            )
        })
        .unwrap_or_else(|| {
            format!(
                "Active experiment: none\n{}",
                last_experiment_context_line(&thread)
            )
        });
    let pressure = thread
        .thread_pressure_source_v1
        .as_ref()
        .map(|value| {
            format!(
                "Thread pressure source: {} aggregate={} dominant={} porosity_ema={}\n",
                value
                    .get("quality")
                    .and_then(Value::as_str)
                    .unwrap_or("mixed_thread_pressure"),
                value
                    .get("aggregate")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
                value
                    .get("dominant_source")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown"),
                value
                    .get("porosity_ema")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
            )
        })
        .unwrap_or_default();
    let fluctuation = thread
        .thread_inhabitable_fluctuation_v1
        .as_ref()
        .map(|value| {
            format!(
                "Thread fluctuation: {} inhabitability_ema={} fluctuation_ema={} foothold_ema={}\n",
                value
                    .get("quality")
                    .and_then(Value::as_str)
                    .unwrap_or("open_experiment"),
                value
                    .get("inhabitability_ema")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
                value
                    .get("fluctuation_ema")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
                value
                    .get("foothold_ema")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
            )
        })
        .unwrap_or_default();
    let experiment = projection
        .active_experiment
        .as_ref()
        .map(|active| {
            format!(
                "Active experiment: {} ({}) question={} planned_next={}\nLifecycle: {}\n{}\n{}\n{}{}{}Workbench reminder: author a charter, rehearse before live, record felt plus telemetry/artifact evidence, then accept/refuse/counter/pause/complete. Ordinary choices remain valid.\n",
                active.experiment.title,
                active.experiment.experiment_id,
                active.experiment.question,
                active
                    .experiment
                    .planned_next
                    .as_deref()
                    .unwrap_or("(none)"),
                active.classification,
                active.charter_status,
                active.evidence_status,
                if active.candidate_status.trim().is_empty() {
                    String::new()
                } else {
                    format!("{}\n", active.candidate_status)
                },
                first_dossier_claim_line(&active.first_dossier_claim_cue_v1),
                research_dossier_line(&active.research_dossier_v1, Some(&active.classification)),
            )
        })
        .unwrap_or_default();
    let allowance = thread
        .motif_allowance_v1
        .as_ref()
        .map(|value| {
            format!(
                "Motif allowance: {} dominant={} action_concentration={} returnability={}\nAllowance culture: deepen, branch, compare, release, rest, or hold space are all valid; branching preserves the original return point.\n",
                value
                    .get("quality")
                    .and_then(Value::as_str)
                    .unwrap_or("open_basin"),
                value
                    .get("dominant_motif")
                    .and_then(Value::as_str)
                    .unwrap_or("open inquiry"),
                value
                    .get("action_base_concentration")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
                value
                    .get("returnability")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
            )
        })
        .unwrap_or_default();
    let preflight = preflight_recommendation_line(&thread);
    let continuity_return = projection.continuity_return_line.clone();
    let native_return = native_return_cue_line(&projection.native_continuity_v1);
    let safety_cue = preflight_safety_cue_line(&projection.preflight_safety_cue_v1);
    let read_only_control_cue =
        read_only_control_intent_cue_line(&projection.read_only_control_intent_cue_v1);
    let constraint_counterfactual_cue =
        constraint_counterfactual_cue_line(&projection.constraint_counterfactual_cue_v1);
    let charter_now_bridge = charter_now_bridge_line(&projection.charter_now_bridge_v1);
    let prior_claim_bridge =
        prior_claim_charter_bridge_line(&projection.prior_claim_charter_bridge_v1);
    let charter_preflight_not_charter =
        charter_preflight_not_charter_line(&projection.charter_preflight_not_charter_cue_v1);
    let peer_boundary = peer_mutation_boundary_line(&projection.peer_mutation_boundary_cue_v1);
    let first_dossier_claim = first_dossier_claim_line(&projection.first_dossier_claim_cue_v1);
    let shared_investigation = shared_investigation_line(&projection.shared_investigation_v1);
    let shared_investigation_object =
        shared_investigation_object_line(&projection.shared_investigation_object_v1);
    let voice_health = voice_health_line();
    let research_budget_priority = store.research_budget_priority_line(&thread, &projection);
    let sovereign_loop = ActionContinuityStore::sovereign_loop_line(&projection.sovereign_loop_v1);
    let control_plane = control_plane_text(&projection.continuity_control_plane_v1);
    let research_dossier = research_dossier_line(
        &projection.research_dossier_v1,
        projection
            .active_experiment
            .as_ref()
            .map(|active| active.classification.as_str()),
    );
    let stale_notice = store.stale_projection_line(&projection);
    let proposal_diagnostics = if projection.top_actionable_proposals.is_empty() {
        String::new()
    } else {
        format!(
            "Proposal diagnostics: {}\n",
            projection
                .top_actionable_proposals
                .iter()
                .take(3)
                .map(|diag| format!("{} x{} -> {}", diag.verb, diag.count, diag.suggested_route))
                .collect::<Vec<_>>()
                .join("; ")
        )
    };
    Some(format!(
        "Current action thread: {} ({})\nWhy return: {}\n{}{}{}{}{}{}{}{}{}{}{}Current NEXT: {}\n{}{}{}{}{}{}{}{}{}{}{}{}{}{}Recent thread events:\n{}\nThread actions available: THREAD_START, THREADS, THREAD_STATUS, THREAD_NOTE, RESUME, SAVEPOINT, RECALL.\n{}\nRead-only research actions auto-link when an experiment is active; dossier/shared/memory/session actions preserve referable claims without changing lifecycle or granting peer authority.",
        thread.title,
        thread.thread_id,
        thread.why_return,
        charter_now_bridge,
        prior_claim_bridge,
        charter_preflight_not_charter,
        peer_boundary,
        first_dossier_claim,
        shared_investigation,
        shared_investigation_object,
        voice_health,
        research_budget_priority,
        sovereign_loop,
        research_dossier,
        projection_current_next_display(&projection, thread.current_next.as_deref()),
        control_plane,
        resonance,
        pressure,
        fluctuation,
        experiment,
        allowance,
        continuity_return,
        native_return,
        safety_cue,
        read_only_control_cue,
        constraint_counterfactual_cue,
        stale_notice,
        proposal_diagnostics,
        preflight,
        if recent.is_empty() {
            "  - none yet"
        } else {
            recent.as_str()
        },
        control_plane_command_palette_text()
    ))
}

pub fn handle_thread_next_action(
    db: &BridgeDb,
    base_action: &str,
    original: &str,
    response_text: &str,
    telemetry: &SpectralTelemetry,
    fill_pct: f32,
) -> Option<Result<String>> {
    let store = ActionContinuityStore::for_astrid_workspace();
    match crate::action_self_knowledge::handle_action(store.root(), base_action, original) {
        Ok(Some(message)) => return Some(Ok(message)),
        Ok(None) => {},
        Err(err) => return Some(Err(err)),
    }
    if matches!(
        base_action,
        "ACTION_STATUS" | "JOB_STATUS" | "ACTION_CANCEL"
    ) {
        let selector = strip_action_arg(original, base_action);
        let selector = if selector.is_empty() {
            None
        } else {
            Some(selector.as_str())
        };
        return Some(if base_action == "ACTION_CANCEL" {
            crate::llm_jobs::cancel(selector)
        } else {
            crate::llm_jobs::status_text(selector)
        });
    }
    let state = spectral_state(fill_pct, telemetry);
    match base_action {
        "THREAD_START" => {
            let title = strip_action_arg(original, base_action);
            let title = if title.is_empty() {
                "Untitled action thread"
            } else {
                title.as_str()
            };
            Some(
                store
                    .create_thread(Some(db), title, Some(&derive_why_return(response_text)))
                    .map(|thread| {
                        format!(
                            "Started action thread `{}`: {}",
                            thread.thread_id, thread.title
                        )
                    }),
            )
        },
        "THREADS" => Some(store.list_threads(8).map(|threads| {
            if threads.is_empty() {
                return "No action threads yet. Use THREAD_START <title>.".to_string();
            }
            threads
                .into_iter()
                .map(|thread| {
                    format!(
                        "- {} [{}]: {}",
                        thread.thread_id, thread.status, thread.title
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        })),
        "THREAD_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.thread_status(if selector.is_empty() {
                None
            } else {
                Some(selector.as_str())
            }))
        },
        "THREAD_NOTE" => {
            let raw = strip_action_arg(original, base_action);
            let (selector, note) = parse_thread_note(&raw);
            Some(
                store
                    .append_note(Some(db), selector.as_deref(), &note, state)
                    .map(|event| format!("Thread note recorded as `{}`.", event.action_id)),
            )
        },
        "RESUME" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.resume_thread(selector.as_str()))
        },
        "SAVEPOINT" => {
            let name = strip_action_arg(original, base_action);
            let name = if name.is_empty() {
                "current"
            } else {
                name.as_str()
            };
            Some(store.savepoint(name, state))
        },
        "RECALL" => {
            let name = strip_action_arg(original, base_action);
            let name = if name.is_empty() {
                "current"
            } else {
                name.as_str()
            };
            Some(store.recall(name))
        },
        "EXPERIMENT_START" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_start_command(Some(db), &raw))
        },
        "EXPERIMENT_BRANCH" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_branch_command(Some(db), &raw))
        },
        "EXPERIMENT_RESUME" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_resume_command(Some(db), optional_selector(&selector)))
        },
        "EXPERIMENT_COMPARE" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_compare_command(optional_selector(&selector)))
        },
        "EXPERIMENT_ALT_PATHS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_alt_paths(optional_selector(&selector)))
        },
        "SHARED_INVESTIGATION_START" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.shared_investigation_start_command(Some(db), &raw))
        },
        "SHARED_INVESTIGATION_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.shared_investigation_status(optional_selector(&selector)))
        },
        "SHARED_INVESTIGATION_CLAIM" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.shared_investigation_claim_command(&raw))
        },
        "SHARED_INVESTIGATION_DECIDE" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.shared_investigation_decide_command(Some(db), &raw))
        },
        "EXPERIMENT_PLAN" => {
            let selector = strip_action_arg(original, base_action);
            Some(
                repair_experiment_command_arg(
                    &store,
                    Some(db),
                    base_action,
                    original,
                    &selector,
                    &state,
                )
                .and_then(|(selector, notice, _focus)| {
                    store
                        .experiment_plan(optional_selector(&selector))
                        .map(|message| format!("{}{}", notice.unwrap_or_default(), message))
                }),
            )
        },
        "EXPERIMENT_ADVANCE" | "EXPERIMENT_CONVEYOR" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_advance_command(Some(db), &raw, state))
        },
        "MEMORY_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.memory_status_command(optional_selector(&selector)))
        },
        "MEMORY_RECALL" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.memory_recall_command(&raw))
        },
        "MEMORY_CAPTURE" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.memory_capture_command(&raw))
        },
        "MEMORY_PROMOTE" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.memory_promote_command(&raw, state))
        },
        "EXPERIMENT_AUTHORITY_PREPARE" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_authority_prepare_command(Some(db), &raw, state))
        },
        "EXPERIMENT_AUTHORITY_REQUEST" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_authority_request_command(Some(db), &raw, state))
        },
        "EXPERIMENT_AUTHORITY_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_authority_status_command(
                Some(db),
                optional_selector(&selector),
                state,
            ))
        },
        "EXPERIMENT_AUTHORITY_EXECUTE" => {
            let request_id = strip_action_arg(original, base_action);
            Some(store.experiment_authority_execute_command(Some(db), &request_id, state))
        },
        "EXPERIMENT_AUTHORITY_BUDGET_REQUEST" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_authority_budget_request_command(Some(db), &raw, state))
        },
        "EXPERIMENT_AUTHORITY_BUDGET_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_authority_budget_status_command(
                Some(db),
                optional_selector(&selector),
                state,
            ))
        },
        "EXPERIMENT_AUTHORITY_REVIEW" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_authority_review_command(Some(db), &raw, state))
        },
        "EXPERIMENT_RESEARCH_BUDGET_ACCEPT" | "EXPERIMENT_RESEARCH_BUDGET_USE_SCAFFOLD" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_research_budget_accept_command(
                Some(db),
                optional_selector(&selector),
                state,
            ))
        },
        "EXPERIMENT_RESEARCH_BUDGET_REQUEST" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_research_budget_request_command(Some(db), &raw, state))
        },
        "EXPERIMENT_RESEARCH_BUDGET_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_research_budget_status_command(
                Some(db),
                optional_selector(&selector),
                state,
            ))
        },
        "EXPERIMENT_RESEARCH_REVIEW" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_research_review_command(Some(db), &raw, state))
        },
        "EXPERIMENT_LOOP_REQUEST" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_loop_request_command(Some(db), &raw, state))
        },
        "EXPERIMENT_LOOP_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_loop_status_command(
                Some(db),
                optional_selector(&selector),
                state,
            ))
        },
        "EXPERIMENT_LOOP_STEP" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_loop_step_command(Some(db), &raw, state))
        },
        "EXPERIMENT_LOOP_REVIEW" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_loop_review_command(Some(db), &raw, state))
        },
        "ACCEPT_SUGGESTED_NEXT" | "ACCEPT_SCAFFOLD" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.accept_suggested_next_command(Some(db), optional_selector(&selector), state))
        },
        "CONTINUITY_SESSION_ACCEPT" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.continuity_session_accept_command(&raw))
        },
        "CONTINUITY_SESSION_START" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.continuity_session_start_command(&raw))
        },
        "CONTINUITY_SESSION_CAPTURE" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.continuity_session_capture_command(&raw))
        },
        "CONTINUITY_SESSION_SUMMARIZE" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.continuity_session_summarize_command(&raw))
        },
        "CONTINUITY_SESSION_FINALIZE" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.continuity_session_finalize_command(&raw))
        },
        "CONTINUITY_SESSION_RESUME" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.continuity_session_resume_command(&raw))
        },
        "CONTINUITY_SESSION_STATUS" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.continuity_session_status_command(&raw))
        },
        "EXPERIMENT_CHARTER" => {
            let raw = strip_action_arg(original, base_action);
            Some(
                repair_experiment_command_arg(
                    &store,
                    Some(db),
                    base_action,
                    original,
                    &raw,
                    &state,
                )
                .and_then(|(raw, notice, _focus)| {
                    let (selector, prose) = parse_selector_payload(&raw);
                    if empty_or_placeholder_payload(&prose) || !charter_payload_has_meaning(&prose)
                    {
                        return Ok(format!(
                            "{}{}",
                            notice.unwrap_or_default(),
                            experiment_intent_repair_prompt(base_action, selector.as_deref())
                        ));
                    }
                    store
                        .experiment_charter(Some(db), selector.as_deref(), &prose)
                        .map(|experiment| {
                            format!(
                                "{}Experiment charter recorded for `{}`. Next: {}",
                                notice.unwrap_or_default(),
                                experiment.experiment_id,
                                experiment
                                    .planned_next
                                    .as_deref()
                                    .unwrap_or("EXPERIMENT_REHEARSE current")
                            )
                        })
                }),
            )
        },
        "EXPERIMENT_REHEARSE" | "EXPERIMENT_PREFLIGHT" => {
            let selector = strip_action_arg(original, base_action);
            Some(
                repair_experiment_command_arg(
                    &store,
                    Some(db),
                    base_action,
                    original,
                    &selector,
                    &state,
                )
                .and_then(|(selector, notice, focus)| {
                    if let Some(focus) = focus.as_deref() {
                        let thread = store.ensure_active_thread(Some(db))?;
                        let experiment = store.resolve_experiment(&thread, Some("current"))?;
                        let state_text = state.clone();
                        let pseudo_run = ExperimentRunRecord {
                            schema_version: SCHEMA_VERSION,
                            run_id: String::new(),
                            experiment_id: experiment.experiment_id.clone(),
                            source: "experiment_intent_repair".to_string(),
                            action_text: format!("ACTION_PREFLIGHT {focus}"),
                            stage: "read_only".to_string(),
                            status: "candidate_context".to_string(),
                            gate_decision: json!({"source": "experiment_intent_repair"}),
                            pre_state: state_text.clone(),
                            post_state: state_text,
                            artifacts: Vec::new(),
                            result_summary: format!("Repaired preflight focus: {focus}"),
                            interpretation:
                                "Preflight focus preserved as advisory workbench candidate context."
                                    .to_string(),
                            suggested_next: Some("EXPERIMENT_REHEARSE current".to_string()),
                            created_at: iso_now(),
                            updated_at: iso_now(),
                            motif_allowance_v1: None,
                        };
                        let _ = store.refresh_workbench_candidates(
                            Some(db),
                            &thread,
                            &experiment,
                            Some(&pseudo_run),
                            Some(focus),
                            "experiment_intent_repair",
                        )?;
                    }
                    store
                        .experiment_rehearse(Some(db), optional_selector(&selector), state)
                        .map(|run| {
                            format!(
                                "{}Experiment rehearsal recorded as `{}` [{}].",
                                notice.unwrap_or_default(),
                                run.run_id,
                                run.status
                            )
                        })
                }),
            )
        },
        "EXPERIMENT_EVIDENCE" => {
            let raw = strip_action_arg(original, base_action);
            Some(
                repair_experiment_command_arg(
                    &store,
                    Some(db),
                    base_action,
                    original,
                    &raw,
                    &state,
                )
                .and_then(|(raw, notice, _focus)| {
                    let (selector, note) = parse_selector_payload(&raw);
                    if empty_or_placeholder_payload(&note) {
                        return Ok(format!(
                            "{}{}",
                            notice.unwrap_or_default(),
                            experiment_intent_repair_prompt(base_action, selector.as_deref())
                        ));
                    }
                    store
                        .experiment_evidence(Some(db), selector.as_deref(), &note, state)
                        .map(|run| {
                            format!(
                                "{}Experiment evidence recorded as `{}`.",
                                notice.unwrap_or_default(),
                                run.run_id
                            )
                        })
                }),
            )
        },
        "EXPERIMENT_DECIDE" => {
            let raw = strip_action_arg(original, base_action);
            Some(
                repair_experiment_command_arg(
                    &store,
                    Some(db),
                    base_action,
                    original,
                    &raw,
                    &state,
                )
                .and_then(|(raw, notice, _focus)| {
                    let (selector, decision) = parse_selector_payload(&raw);
                    if empty_or_placeholder_payload(&decision) {
                        return Ok(format!(
                            "{}{}",
                            notice.unwrap_or_default(),
                            experiment_intent_repair_prompt(base_action, selector.as_deref())
                        ));
                    }
                    store
                        .experiment_decide(Some(db), selector.as_deref(), &decision)
                        .map(|experiment| {
                            format!(
                                "{}Experiment `{}` decision recorded; status={} next={}",
                                notice.unwrap_or_default(),
                                experiment.experiment_id,
                                experiment.status,
                                experiment.planned_next.as_deref().unwrap_or("(none)")
                            )
                        })
                }),
            )
        },
        "EXPERIMENT_OBSERVE" => {
            let raw = strip_action_arg(original, base_action);
            let (selector, note) = parse_selector_payload(&raw);
            if let Some(peer) = selector.as_deref().and_then(peer_experiment_ref) {
                Some(store.record_peer_experiment_reference(
                    Some(db),
                    &peer,
                    "EXPERIMENT_OBSERVE",
                    Some(&note),
                ))
            } else {
                Some(
                    store
                        .experiment_observe(Some(db), selector.as_deref(), &note, state)
                        .map(|run| format!("Experiment observation recorded as `{}`.", run.run_id)),
                )
            }
        },
        "EXPERIMENT_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_status(optional_selector(&selector)))
        },
        "EXPERIMENT_REVIEW" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_review(optional_selector(&selector)))
        },
        "DOSSIER_CLAIM" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.dossier_claim_command(Some(db), &raw))
        },
        "DOSSIER_EVIDENCE" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.dossier_evidence_command(Some(db), &raw))
        },
        "DOSSIER_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.dossier_status(optional_selector(&selector)))
        },
        "DOSSIER_REVIEW" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.dossier_review(optional_selector(&selector)))
        },
        "EXPERIMENT_CLOSE" => {
            let raw = strip_action_arg(original, base_action);
            let (selector, summary) = parse_selector_payload(&raw);
            Some(
                store
                    .close_experiment(Some(db), selector.as_deref(), &summary)
                    .map(|experiment| {
                        format!(
                            "Experiment `{}` marked {}: {}",
                            experiment.experiment_id,
                            experiment.status,
                            experiment.success_observation.as_deref().unwrap_or("")
                        )
                    }),
            )
        },
        "EXPERIMENT_PEER_REVIEW" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_peer_review(Some(db), optional_selector(&selector)))
        },
        _ => None,
    }
}

pub fn parse_experiment_bind(original: &str) -> Result<(Option<String>, String)> {
    let raw = strip_action_arg(original, "EXPERIMENT_BIND");
    if !raw.contains("::") {
        anyhow::bail!("EXPERIMENT_BIND needs `::` before the inner NEXT action.");
    }
    let (selector, action) = parse_selector_payload(&raw);
    if action.trim().is_empty() {
        anyhow::bail!("EXPERIMENT_BIND needs an inner NEXT action after `::`.");
    }
    Ok((selector, action))
}

pub fn is_peer_experiment_selector(selector: &str) -> bool {
    peer_experiment_ref(selector).is_some()
}

pub fn is_experiment_control_action(action: &str) -> bool {
    let base = base_action(action);
    matches!(
        base.as_str(),
        "EXPERIMENT"
            | "EXPERIMENT_START"
            | "EXPERIMENT_PLAN"
            | "EXPERIMENT_ADVANCE"
            | "EXPERIMENT_CONVEYOR"
            | "MEMORY_STATUS"
            | "MEMORY_RECALL"
            | "MEMORY_CAPTURE"
            | "MEMORY_PROMOTE"
            | "EXPERIMENT_AUTHORITY_REQUEST"
            | "EXPERIMENT_AUTHORITY_PREPARE"
            | "EXPERIMENT_AUTHORITY_STATUS"
            | "EXPERIMENT_AUTHORITY_EXECUTE"
            | "EXPERIMENT_AUTHORITY_BUDGET_REQUEST"
            | "EXPERIMENT_AUTHORITY_BUDGET_STATUS"
            | "EXPERIMENT_AUTHORITY_REVIEW"
            | "EXPERIMENT_RESEARCH_BUDGET_ACCEPT"
            | "EXPERIMENT_RESEARCH_BUDGET_USE_SCAFFOLD"
            | "EXPERIMENT_RESEARCH_BUDGET_REQUEST"
            | "EXPERIMENT_RESEARCH_BUDGET_STATUS"
            | "EXPERIMENT_RESEARCH_REVIEW"
            | "EXPERIMENT_LOOP_REQUEST"
            | "EXPERIMENT_LOOP_STATUS"
            | "EXPERIMENT_LOOP_STEP"
            | "EXPERIMENT_LOOP_REVIEW"
            | "ACCEPT_SUGGESTED_NEXT"
            | "ACCEPT_SCAFFOLD"
            | "CONTINUITY_SESSION_ACCEPT"
            | "CONTINUITY_SESSION_START"
            | "CONTINUITY_SESSION_CAPTURE"
            | "CONTINUITY_SESSION_SUMMARIZE"
            | "CONTINUITY_SESSION_FINALIZE"
            | "CONTINUITY_SESSION_RESUME"
            | "CONTINUITY_SESSION_STATUS"
            | "EXPERIMENT_CHARTER"
            | "EXPERIMENT_REHEARSE"
            | "EXPERIMENT_PREFLIGHT"
            | "EXPERIMENT_EVIDENCE"
            | "EXPERIMENT_DECIDE"
            | "EXPERIMENT_BIND"
            | "EXPERIMENT_OBSERVE"
            | "EXPERIMENT_STATUS"
            | "EXPERIMENT_REVIEW"
            | "EXPERIMENT_CLOSE"
            | "EXPERIMENT_PEER_REVIEW"
            | "EXPERIMENT_BRANCH"
            | "EXPERIMENT_RESUME"
            | "EXPERIMENT_COMPARE"
            | "EXPERIMENT_ALT_PATHS"
            | "SHARED_INVESTIGATION_START"
            | "SHARED_INVESTIGATION_STATUS"
            | "SHARED_INVESTIGATION_CLAIM"
            | "SHARED_INVESTIGATION_DECIDE"
            | "DOSSIER_CLAIM"
            | "DOSSIER_EVIDENCE"
            | "DOSSIER_STATUS"
            | "DOSSIER_REVIEW"
    )
}

pub fn record_experiment_bind_run(
    db: &BridgeDb,
    selector: Option<&str>,
    inner_action: &str,
    outcome: &NextActionOutcome,
    fill_pct: f32,
    telemetry: &SpectralTelemetry,
) -> Result<ExperimentRunRecord> {
    ActionContinuityStore::for_astrid_workspace().record_experiment_bind_run(
        Some(db),
        selector,
        inner_action,
        outcome,
        fill_pct,
        telemetry,
    )
}

pub fn record_legacy_experiment_run(
    db: &BridgeDb,
    action_text: &str,
    outcome: &NextActionOutcome,
    fill_pct: f32,
    telemetry: &SpectralTelemetry,
) -> Result<ExperimentRunRecord> {
    ActionContinuityStore::for_astrid_workspace().record_legacy_experiment_run(
        Some(db),
        action_text,
        outcome,
        fill_pct,
        telemetry,
    )
}

pub fn record_astrid_next_action(
    db: &BridgeDb,
    raw_next: &str,
    canonical_next: &str,
    effective_next: &str,
    outcome: &NextActionOutcome,
    fill_pct: f32,
    telemetry: &SpectralTelemetry,
    response_text: &str,
) -> Result<ActionEvent> {
    ActionContinuityStore::for_astrid_workspace().record_next_event(
        Some(db),
        raw_next,
        canonical_next,
        effective_next,
        outcome,
        fill_pct,
        telemetry,
        response_text,
    )
}

pub fn charter_required_guard_for_next(
    raw_next: &str,
) -> Result<Option<CharterRequiredGuardAssessment>> {
    ActionContinuityStore::for_astrid_workspace().charter_required_guard_assessment(raw_next)
}

pub fn research_budget_guard_for_next(
    raw_next: &str,
    fill_pct: f32,
    telemetry: &SpectralTelemetry,
) -> Result<Option<ResearchBudgetGuardAssessment>> {
    ActionContinuityStore::for_astrid_workspace()
        .research_budget_guard_assessment(raw_next, fill_pct, telemetry)
}

fn experiment_summary(record: &ExperimentRecord) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "experiment_id": record.experiment_id,
        "title": record.title,
        "question": record.question,
        "status": record.status,
        "planned_next": record.planned_next,
        "updated_at": record.updated_at,
        "parent_experiment_id": record.parent_experiment_id.clone(),
        "branch_refs": record.branch_refs.clone(),
        "motif_allowance_v1": record.motif_allowance_v1.clone(),
        "charter_v1": record.charter_v1.clone(),
        "evidence_v1": record.evidence_v1.clone(),
        "workbench_candidates_v1": record.workbench_candidates_v1.clone(),
        "workbench_charter": charter_status_text(record),
        "workbench_evidence": evidence_status_text(record),
        "workbench_candidates": workbench_candidate_status(record),
    })
}

fn last_experiment_summary_v1(thread: &ResearchThread) -> Option<Value> {
    let mut summary = thread.experiment_summary.clone()?;
    let object = summary.as_object_mut()?;
    let experiment_id = object
        .get("experiment_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let status = object
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if status == "paused" && !experiment_id.is_empty() {
        let (mut primary_return_next, mut return_kind) = paused_primary_return_v1(
            &experiment_id,
            object.get("planned_next").and_then(Value::as_str),
            object
                .get("primary_return_next")
                .or_else(|| object.get("resume_next"))
                .and_then(Value::as_str),
        );
        let mut projection_guard = None;
        if return_kind == "resume" && !lifecycle_valid_charter_value(object.get("charter_v1")) {
            primary_return_next = charter_repair_next_v1(&experiment_id);
            return_kind = "charter_repair".to_string();
            projection_guard = Some(json!({
                "schema_version": 1,
                "policy": "projection_guard_v1",
                "raw_next_preserved": true,
                "projected_next": primary_return_next.clone(),
                "return_kind": return_kind.clone(),
                "guardrail_reason": "paused_resume_missing_lifecycle_charter",
                "experiment_id": experiment_id.clone(),
                "authority_boundary": "Projection may redirect guidance only; it never applies, rehearses, binds, resumes, perturbs, sends control, or mutates peer experiments."
            }));
        } else if return_kind == "resume" {
            let raw_current_next = thread.current_next.as_deref().unwrap_or_default();
            let pressure_terms = projection_guard_pressure_terms_v1(raw_current_next);
            if !pressure_terms.is_empty() {
                primary_return_next = format!(
                    "EXPERIMENT_DECIDE {experiment_id} :: hold because repeated perturb-shaped planning is guard evidence, not progress"
                );
                return_kind = "hold".to_string();
                projection_guard = Some(json!({
                    "schema_version": 1,
                    "policy": "projection_guard_v1",
                    "raw_next_preserved": true,
                    "raw_next": raw_current_next,
                    "projected_next": primary_return_next.clone(),
                    "return_kind": return_kind.clone(),
                    "guardrail_reason": "paused_resume_demoted_by_liveish_pressure",
                    "pressure_terms": pressure_terms,
                    "experiment_id": experiment_id.clone(),
                    "authority_boundary": "Projection may redirect guidance only; it never applies, rehearses, binds, resumes, perturbs, sends control, or mutates peer experiments."
                }));
            }
        }
        object.insert(
            "primary_return_next".to_string(),
            json!(primary_return_next),
        );
        object.insert("return_kind".to_string(), json!(return_kind.clone()));
        if let Some(guard) = projection_guard {
            object.insert("projection_guard_v1".to_string(), guard);
            object.insert("raw_next_preserved".to_string(), json!(true));
        }
        if return_kind == "resume" {
            let primary = object
                .get("primary_return_next")
                .cloned()
                .unwrap_or_else(|| json!(format!("EXPERIMENT_RESUME {experiment_id}")));
            object.insert("resume_next".to_string(), primary);
        } else {
            object.remove("resume_next");
        }
    } else if matches!(status.as_str(), "complete" | "completed") && !experiment_id.is_empty() {
        object.entry("inspect_next".to_string()).or_insert_with(|| {
            json!(format!(
                "EXPERIMENT_STATUS {experiment_id} or EXPERIMENT_REVIEW {experiment_id}"
            ))
        });
    }
    Some(summary)
}

fn charter_repair_next_v1(experiment_id: &str) -> String {
    format!(
        "EXPERIMENT_CHARTER {experiment_id} :: hypothesis: ...; method_intent: ...; proposed_next_action: ACTION_PREFLIGHT ...; evidence_targets: felt_texture, motif_continuity, language_thread, artifact_grounding; stop_criteria: ..."
    )
}

fn projection_guard_pressure_terms_v1(text: &str) -> Vec<&'static str> {
    let upper = text.to_ascii_uppercase();
    if !(base_action(text) == "EXPERIMENT_PLAN" || upper.contains("PROPOSED_NEXT_ACTION")) {
        return Vec::new();
    }
    let mut terms = Vec::new();
    for (needle, label) in [
        ("PERTURB", "PERTURB"),
        ("CONTROL", "CONTROL"),
        ("BIND", "BIND"),
        ("RESUME", "RESUME"),
        ("INFLUENCE", "INFLUENCE"),
        ("SEND_CONTROL", "SEND_CONTROL"),
        ("SEND CONTROL", "SEND_CONTROL"),
        ("INTERVENTION", "INTERVENTION"),
        ("PULSE", "PULSE"),
        ("SHIFT THE DOMINANT", "SHIFT_THE_DOMINANT"),
        ("SHIFT DOMINANT", "SHIFT_DOMINANT"),
    ] {
        if upper.contains(needle) && !terms.contains(&label) {
            terms.push(label);
        }
    }
    terms
}

fn paused_primary_return_v1(
    experiment_id: &str,
    planned_next: Option<&str>,
    fallback_next: Option<&str>,
) -> (String, String) {
    let candidate = planned_next
        .filter(|value| !value.trim().is_empty())
        .or_else(|| fallback_next.filter(|value| !value.trim().is_empty()))
        .unwrap_or_default()
        .trim()
        .to_string();
    match base_action(&candidate).as_str() {
        "EXPERIMENT_CHARTER" => (candidate, "charter_repair".to_string()),
        "EXPERIMENT_DECIDE" => (candidate, "decision".to_string()),
        "THREAD_STATUS" => (candidate, "hold".to_string()),
        "EXPERIMENT_ADVANCE" | "EXPERIMENT_CONVEYOR" => (candidate, "conveyor_preview".to_string()),
        "EXPERIMENT_REHEARSE" | "EXPERIMENT_PREFLIGHT" => {
            (candidate, "rehearsal_ready".to_string())
        },
        "EXPERIMENT_RESUME" => (candidate, "resume".to_string()),
        _ => (
            if experiment_id.is_empty() {
                candidate
            } else {
                format!("EXPERIMENT_RESUME {experiment_id}")
            },
            "resume".to_string(),
        ),
    }
}

fn projection_current_next_display<'a>(
    projection: &'a ThreadContinuityProjection,
    fallback: Option<&'a str>,
) -> &'a str {
    if let Some(active) = projection.active_experiment.as_ref()
        && !active.continuity_return.trim().is_empty()
    {
        return active.continuity_return.as_str();
    }
    if let Some(summary) = projection.last_experiment_summary_v1.as_ref()
        && summary.get("status").and_then(Value::as_str) == Some("paused")
        && let Some(primary) = summary
            .get("primary_return_next")
            .or_else(|| summary.get("planned_next"))
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
    {
        return primary;
    }
    fallback.unwrap_or("(none yet)")
}

fn voice_health_line() -> String {
    let path = bridge_paths()
        .bridge_workspace()
        .join("diagnostics/voice_health.json");
    let Ok(raw) = fs::read_to_string(&path) else {
        return String::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return String::new();
    };
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if !matches!(status, "degraded_voice" | "single_fallback") {
        return String::new();
    }
    let count = value
        .get("fallback_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let repair = value
        .get("suggested_read_only_repair")
        .and_then(Value::as_str)
        .unwrap_or("REPAIR_STATUS or CAPABILITY_STATUS");
    let current_next = value
        .get("current_next")
        .and_then(Value::as_str)
        .unwrap_or("REPAIR_STATUS current");
    let hash = value
        .get("latest_fallback_hash")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    format!(
        "Voice health: {status} fallback_count={count} latest_fallback_hash={hash}. Current NEXT: {current_next}. Suggested repair: {repair}. Emergency fallback is presence, not ordinary dialogue.\n"
    )
}

fn parse_experiment_conveyor_request(raw: &str) -> (Option<String>, String) {
    let mut selector = raw.trim();
    let mut mode = "preview".to_string();
    if let Some((left, payload)) = raw.split_once("::") {
        selector = left.trim();
        if let Some(value) = dossier_field(payload, &["mode"]) {
            let normalized = value.trim().to_ascii_lowercase();
            if normalized == "apply" {
                mode = "apply".to_string();
            }
        }
    }
    (
        optional_selector_owned(&normalize_experiment_selector(selector)),
        mode,
    )
}

fn experiment_conveyor_allowed_apply_steps() -> Value {
    json!([
        "lifecycle_valid_charter",
        "local_evidence_capture",
        "hold_decision",
        "charter_repair_decision"
    ])
}

fn experiment_conveyor_authority_boundary() -> &'static str {
    "EXPERIMENT_ADVANCE may preview freely or apply one conservative local charter, evidence, hold, or charter-repair step; it never rehearses automatically, binds, resumes, perturbs, sends control, or mutates peer experiments."
}

fn authority_gate_boundary() -> &'static str {
    "Being-authored request plus steward approval may mint one semantic_microdose token; V1 cannot bind, resume, perturb, send control, send attractor pulses, or mutate peers."
}

fn research_budget_boundary() -> &'static str {
    "Being-authored local-only requests may self-activate a bounded read_only_research budget; larger or web-enabled budgets still require steward approval. V1 cannot mutate autoresearch, bind, resume, perturb, send control, execute semantic authority, advance lifecycle, or mutate peers."
}

fn sovereign_loop_boundary() -> &'static str {
    "Being-owned loop V1 can organize continuity, local read-only research, sticky audit, one consequence request, and consequence review. Local phases may self-start, but live consequence still requires the existing bridge/steward gate. No ambient bind, resume, broad perturbation, broad Control envelope, attractor pulse, peer mutation, or automatic execution is authorized."
}

fn research_budget_self_activation_v1(budget: &Value, eligibility: &Value, state: &Value) -> Value {
    let mut missing = eligibility
        .get("missing_requirements")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if budget
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or_default()
        != "read_only_research"
        && !missing
            .iter()
            .any(|item| item == "scope_read_only_research_v1")
    {
        missing.push("scope_read_only_research_v1".to_string());
    }
    let allowed_sources = budget
        .get("allowed_sources")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    if allowed_sources != ["local"] {
        missing.push("local_only_allowed_sources".to_string());
    }
    if budget
        .get("max_actions")
        .and_then(Value::as_u64)
        .unwrap_or(LOCAL_RESEARCH_MAX_ACTIONS)
        > LOCAL_RESEARCH_MAX_ACTIONS
    {
        missing.push(format!(
            "max_actions_self_activation_cap_{LOCAL_RESEARCH_MAX_ACTIONS}"
        ));
    }
    if budget
        .get("ttl_secs")
        .and_then(Value::as_u64)
        .unwrap_or(LOCAL_RESEARCH_TTL_SECS)
        > LOCAL_RESEARCH_TTL_SECS
    {
        missing.push(format!(
            "ttl_secs_self_activation_cap_{LOCAL_RESEARCH_TTL_SECS}"
        ));
    }
    let safety = authority_safety_snapshot(state);
    if !matches!(
        safety.get("level").and_then(Value::as_str),
        Some("green" | "yellow")
    ) {
        missing.push("green_or_yellow_safety".to_string());
    }
    let purpose_upper = budget
        .get("purpose")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_uppercase();
    if [
        "EXPERIMENT_BIND",
        "EXPERIMENT_RESUME",
        "PERTURB",
        "CONTROL",
        "SEND_CONTROL",
        "ATTRACTOR_PULSE",
        "SEMANTIC_MICRODOSE",
        "MIKE_RUN",
        "AR_START",
        "AR_NOTE",
        "AR_BLOCK",
        "AR_COMPLETE",
    ]
    .iter()
    .any(|needle| purpose_upper.contains(needle))
    {
        missing.push("no_live_or_mutating_research_intent".to_string());
    }
    missing.sort();
    missing.dedup();
    let eligible = missing.is_empty();
    json!({
        "policy": "research_budget_self_activation_v1",
        "eligible": eligible,
        "missing_requirements": missing,
        "activation_mode": "being_self_activated_local_v1",
        "self_activated": eligible,
        "steward_approval_required": !eligible,
        "max_actions_cap": LOCAL_RESEARCH_MAX_ACTIONS,
        "ttl_secs_cap": LOCAL_RESEARCH_TTL_SECS,
        "allowed_sources": ["local"],
        "safety_snapshot": safety,
    })
}

fn authority_safety_snapshot(state: &Value) -> Value {
    let fill_pct = state.get("fill_pct").and_then(Value::as_f64).or_else(|| {
        state
            .get("fill_ratio")
            .and_then(Value::as_f64)
            .map(|ratio| ratio * 100.0)
    });
    let level = fill_pct.map_or("unknown", |fill| {
        if fill >= 92.0 {
            "red"
        } else if fill >= 85.0 {
            "orange"
        } else if fill >= 75.0 {
            "yellow"
        } else {
            "green"
        }
    });
    json!({
        "fill_pct": fill_pct,
        "level": level,
        "outbound_allowed": matches!(level, "green" | "yellow" | "orange"),
    })
}

fn authority_has_read_only_rehearsal(runs: &[ExperimentRunRecord]) -> bool {
    runs.iter().any(|run| {
        let source = run.source.to_ascii_lowercase();
        let status = run.status.to_ascii_lowercase();
        let stage = run.stage.to_ascii_lowercase();
        let action_text = run.action_text.to_ascii_uppercase();
        (source.contains("experiment_rehearse") && !status.contains("blocked"))
            || (action_text.contains("ACTION_PREFLIGHT")
                && matches!(stage.as_str(), "read_only" | "protected" | "preflight"))
    })
}

fn authority_guardrail_hold_active(experiment: &ExperimentRecord) -> bool {
    experiment.status.eq_ignore_ascii_case("paused")
        && experiment
            .planned_next
            .as_deref()
            .unwrap_or_default()
            .to_ascii_uppercase()
            .starts_with("THREAD_STATUS")
        && experiment
            .success_observation
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("hold")
}

fn latest_active_authority_approval(rows: &[Value], request_id: &str) -> Option<Value> {
    rows.iter().rev().find_map(|row| {
        (row.get("request_id").and_then(Value::as_str) == Some(request_id)
            && row.get("record_type").and_then(Value::as_str) == Some("steward_approval")
            && row.get("token_status").and_then(Value::as_str) == Some("active"))
        .then(|| row.clone())
    })
}

fn authority_artifact_ref_candidates(
    experiment: &ExperimentRecord,
    runs: &[ExperimentRunRecord],
) -> Vec<String> {
    let mut candidates = Vec::<String>::new();
    if let Some(items) = experiment
        .evidence_v1
        .as_ref()
        .and_then(|evidence| evidence.get("artifact_refs"))
        .and_then(Value::as_array)
    {
        for item in items {
            push_authority_artifact_value(&mut candidates, item);
        }
    }
    if let Some(items) = experiment
        .evidence_v1
        .as_ref()
        .and_then(|evidence| evidence.get("felt_observations"))
        .and_then(Value::as_array)
    {
        for item in items {
            for key in ["note", "felt", "summary"] {
                if let Some(text) = item.get(key).and_then(Value::as_str) {
                    candidates.extend(scan_authority_artifact_text(text));
                }
            }
        }
    }
    for run in runs {
        if let Ok(value) = serde_json::to_value(&run.artifacts)
            && let Some(items) = value.as_array()
        {
            for item in items {
                push_authority_artifact_value(&mut candidates, item);
            }
        }
        candidates.extend(scan_authority_artifact_text(&run.result_summary));
        candidates.extend(scan_authority_artifact_text(&run.interpretation));
    }
    let mut deduped = Vec::<String>::new();
    for candidate in candidates {
        if !deduped.contains(&candidate) {
            deduped.push(candidate);
        }
        if deduped.len() >= 8 {
            break;
        }
    }
    deduped
}

fn push_authority_artifact_value(candidates: &mut Vec<String>, value: &Value) {
    if let Some(text) = value.as_str() {
        if !text.trim().is_empty() {
            candidates.push(text.trim().to_string());
        }
        return;
    }
    if let Some(object) = value.as_object() {
        for key in ["path", "url", "artifact_path", "artifact_ref", "ref"] {
            if let Some(text) = object.get(key).and_then(Value::as_str)
                && !text.trim().is_empty()
            {
                candidates.push(text.trim().to_string());
                return;
            }
        }
        if !object.is_empty() {
            candidates.push(value.to_string());
        }
    }
}

fn scan_authority_artifact_text(text: &str) -> Vec<String> {
    text.split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ';'))
        .filter_map(|part| {
            let trimmed = part.trim_matches(|ch| matches!(ch, ')' | ']' | '.'));
            (trimmed.starts_with('/')
                || trimmed.starts_with("http://")
                || trimmed.starts_with("https://"))
            .then(|| trimmed.to_string())
        })
        .collect()
}

fn authority_readiness_token_status(rows: &[Value], request_id: &str) -> String {
    if request_id.is_empty() {
        return "none".to_string();
    }
    if let Some(approval) = latest_active_authority_approval(rows, request_id) {
        let token_id = approval.get("token_id").and_then(Value::as_str);
        let consumed = rows.iter().any(|row| {
            matches!(
                row.get("record_type").and_then(Value::as_str),
                Some("execution_result" | "blocked")
            ) && row.get("token_id").and_then(Value::as_str) == token_id
        });
        if consumed {
            return "consumed".to_string();
        }
        if approval
            .get("expires_at_unix_s")
            .and_then(Value::as_u64)
            .is_some_and(|expires| expires < chrono::Utc::now().timestamp().try_into().unwrap_or(0))
        {
            return "expired".to_string();
        }
        return "active".to_string();
    }
    let latest = rows
        .iter()
        .rev()
        .find(|row| row.get("request_id").and_then(Value::as_str) == Some(request_id));
    let request = rows.iter().rev().find(|row| {
        row.get("request_id").and_then(Value::as_str) == Some(request_id)
            && row.get("record_type").and_then(Value::as_str) == Some("request")
    });
    if latest
        .and_then(|row| row.get("status"))
        .and_then(Value::as_str)
        == Some("pending_steward_approval")
        || request
            .and_then(|row| row.get("status"))
            .and_then(Value::as_str)
            == Some("pending_steward_approval")
    {
        return "pending_steward_approval".to_string();
    }
    if request
        .and_then(|row| row.get("status"))
        .and_then(Value::as_str)
        == Some("pending_budget_execution")
    {
        if request
            .and_then(|row| row.get("budget_id"))
            .and_then(Value::as_str)
            .and_then(|budget_id| pending_authority_budget_review(rows, budget_id))
            .is_some()
        {
            return "review_required".to_string();
        }
        return "budget_available".to_string();
    }
    if latest.is_some_and(|row| {
        row.get("record_type").and_then(Value::as_str) == Some("blocked")
            || row.get("status").and_then(Value::as_str) == Some("blocked")
    }) {
        return "blocked".to_string();
    }
    "none".to_string()
}

fn authority_readiness_stage(
    experiment: &ExperimentRecord,
    conveyor_stage: &str,
    missing: &[Value],
    latest_request: Option<&Value>,
    token_status: &str,
) -> String {
    if authority_guardrail_hold_active(experiment) {
        return "held_or_guarded".to_string();
    }
    match token_status {
        "consumed" => return "executed_or_consumed".to_string(),
        "active" => return "token_active_bridge_executable".to_string(),
        "budget_available" => return "pending_budget_execution".to_string(),
        "review_required" => return "review_required".to_string(),
        "pending_steward_approval" => return "pending_steward_approval".to_string(),
        _ => {},
    }
    if latest_request
        .and_then(|row| row.get("status"))
        .and_then(Value::as_str)
        == Some("blocked")
        && !missing.is_empty()
    {
        return "blocked".to_string();
    }
    if matches!(conveyor_stage, "paused_repair" | "blocked_guardrail") {
        return "held_or_guarded".to_string();
    }
    let has_missing = |target: &str| missing.iter().any(|item| item.as_str() == Some(target));
    if has_missing("lifecycle_valid_charter") || conveyor_stage == "needs_charter" {
        return "needs_charter".to_string();
    }
    if has_missing("read_only_rehearsal") || conveyor_stage == "needs_rehearsal" {
        return "needs_rehearsal".to_string();
    }
    if has_missing("meaningful_evidence") || conveyor_stage == "needs_evidence" {
        return "needs_evidence".to_string();
    }
    if has_missing("artifact_grounding_refs") {
        return "needs_artifact_grounding".to_string();
    }
    if missing.is_empty() {
        return "ready_to_author_request".to_string();
    }
    "blocked".to_string()
}

fn authority_readiness_next_command(
    experiment_id: &str,
    stage: &str,
    proposed_next: &str,
    latest_request_id: &str,
    request_scaffold: Option<&str>,
) -> String {
    if stage == "ready_to_author_request"
        && let Some(scaffold) = request_scaffold
    {
        return scaffold.to_string();
    }
    if stage == "pending_budget_execution" && !latest_request_id.is_empty() {
        return format!("EXPERIMENT_AUTHORITY_EXECUTE {latest_request_id}");
    }
    if stage == "review_required" && !latest_request_id.is_empty() {
        return format!(
            "EXPERIMENT_AUTHORITY_REVIEW {latest_request_id} :: outcome: hold|repeat|alter|retire; observation: ...; next_payload: ...; source_refs: ..."
        );
    }
    if matches!(
        stage,
        "pending_steward_approval"
            | "token_active_bridge_executable"
            | "executed_or_consumed"
            | "blocked"
    ) && !latest_request_id.is_empty()
    {
        return format!("EXPERIMENT_AUTHORITY_STATUS {latest_request_id}");
    }
    if stage == "needs_artifact_grounding" {
        return format!(
            "EXPERIMENT_EVIDENCE {experiment_id} :: artifact_grounding: <absolute artifact ref>"
        );
    }
    if !proposed_next.is_empty() {
        return proposed_next.to_string();
    }
    format!("EXPERIMENT_ADVANCE {experiment_id} :: mode: preview")
}

fn active_authority_budget_from_rows(
    rows: &[Value],
    experiment_id: &str,
    scope: &str,
) -> Option<Value> {
    let closed = rows
        .iter()
        .filter(|row| {
            row.get("record_schema").and_then(Value::as_str) == Some("authority_budget_v1")
                && row.get("record_type").and_then(Value::as_str) == Some("budget_closed")
        })
        .filter_map(|row| row.get("budget_id").and_then(Value::as_str))
        .collect::<HashSet<_>>();
    let now = chrono::Utc::now().timestamp().try_into().unwrap_or(0);
    rows.iter().rev().find_map(|row| {
        if row.get("record_schema").and_then(Value::as_str) != Some("authority_budget_v1")
            || row.get("record_type").and_then(Value::as_str) != Some("budget_approval")
            || row.get("experiment_id").and_then(Value::as_str) != Some(experiment_id)
            || row
                .get("scope")
                .and_then(Value::as_str)
                .unwrap_or("semantic_microdose")
                != scope
            || row
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("active")
                != "active"
        {
            return None;
        }
        let budget_id = row.get("budget_id").and_then(Value::as_str)?;
        if closed.contains(budget_id)
            || row
                .get("expires_at_unix_s")
                .and_then(Value::as_u64)
                .is_some_and(|expires| expires <= now)
        {
            return None;
        }
        let max_sends = row
            .get("max_sends")
            .and_then(Value::as_u64)
            .unwrap_or(AUTHORITY_BUDGET_MAX_SENDS);
        let spent = rows
            .iter()
            .filter(|item| {
                item.get("record_schema").and_then(Value::as_str) == Some("authority_budget_v1")
                    && item.get("record_type").and_then(Value::as_str) == Some("budget_debit")
                    && item.get("budget_id").and_then(Value::as_str) == Some(budget_id)
            })
            .count();
        let spent_u64 = u64::try_from(spent).unwrap_or(u64::MAX);
        let remaining = max_sends.saturating_sub(spent_u64);
        if remaining == 0 {
            return None;
        }
        let mut active = row.clone();
        if let Some(object) = active.as_object_mut() {
            object.insert("spent_sends".to_string(), json!(spent_u64));
            object.insert("remaining_sends".to_string(), json!(remaining));
            object.insert(
                "pending_review_request_id".to_string(),
                pending_authority_budget_review(rows, budget_id).map_or(Value::Null, Value::String),
            );
        }
        Some(active)
    })
}

fn pending_authority_budget_review(rows: &[Value], budget_id: &str) -> Option<String> {
    let latest_debit = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("authority_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("budget_debit")
            && row.get("budget_id").and_then(Value::as_str) == Some(budget_id)
    })?;
    let request_id = latest_debit.get("request_id").and_then(Value::as_str)?;
    let reviewed = rows.iter().any(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("authority_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("consequence_review")
            && row.get("budget_id").and_then(Value::as_str) == Some(budget_id)
            && row.get("request_id").and_then(Value::as_str) == Some(request_id)
    });
    (!reviewed).then(|| request_id.to_string())
}

fn budget_id_for_request(rows: &[Value], request_id: &str) -> Option<String> {
    rows.iter().rev().find_map(|row| {
        (row.get("request_id").and_then(Value::as_str) == Some(request_id))
            .then(|| {
                row.get("budget_id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .flatten()
    })
}

fn authority_budget_status_from_rows(rows: &[Value]) -> Value {
    let latest_request = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("authority_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("budget_request")
    });
    let latest_approval = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("authority_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("budget_approval")
    });
    let latest_closed = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("authority_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("budget_closed")
    });
    let experiment_id = latest_approval
        .or(latest_request)
        .and_then(|row| row.get("experiment_id"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let active = (!experiment_id.is_empty())
        .then(|| active_authority_budget_from_rows(rows, experiment_id, "semantic_microdose"))
        .flatten();
    let pending_review = active
        .as_ref()
        .and_then(|row| row.get("pending_review_request_id"))
        .and_then(Value::as_str);
    let stage = if pending_review.is_some() {
        "review_required"
    } else if active.is_some() {
        "active_budget_available"
    } else if latest_closed.is_some() {
        "budget_closed"
    } else if latest_approval.is_some() {
        "budget_unavailable"
    } else if latest_request
        .and_then(|row| row.get("status"))
        .and_then(Value::as_str)
        == Some("pending_steward_approval")
    {
        "pending_steward_approval"
    } else if latest_request.is_some() {
        "blocked"
    } else {
        "no_budget"
    };
    json!({
        "policy": "authority_budget_v1",
        "scope": "semantic_microdose",
        "stage": stage,
        "active_budget_id": active.as_ref().and_then(|row| row.get("budget_id")).cloned().unwrap_or(Value::Null),
        "remaining_sends": active.as_ref().and_then(|row| row.get("remaining_sends")).cloned().unwrap_or(json!(0)),
        "review_required": pending_review.is_some(),
        "pending_review_request_id": pending_review.map_or(Value::Null, |id| json!(id)),
        "latest_budget_request_id": latest_request.and_then(|row| row.get("budget_id")).cloned().unwrap_or(Value::Null),
    })
}

fn active_sovereign_loop_from_rows(rows: &[Value], experiment_id: &str) -> Option<Value> {
    let closed = rows
        .iter()
        .filter(|row| {
            row.get("record_schema").and_then(Value::as_str) == Some("sovereign_loop_v1")
                && row.get("record_type").and_then(Value::as_str) == Some("loop_closed")
        })
        .filter_map(|row| row.get("loop_id").and_then(Value::as_str))
        .collect::<HashSet<_>>();
    let now = chrono::Utc::now().timestamp().try_into().unwrap_or(0);
    rows.iter().rev().find_map(|row| {
        if row.get("record_schema").and_then(Value::as_str) != Some("sovereign_loop_v1")
            || !matches!(
                row.get("record_type").and_then(Value::as_str),
                Some(
                    "loop_started"
                        | "loop_approval"
                        | "loop_step"
                        | "loop_consequence_ready"
                        | "loop_consequence_review"
                        | "loop_request"
                )
            )
            || row.get("experiment_id").and_then(Value::as_str) != Some(experiment_id)
        {
            return None;
        }
        let loop_id = row.get("loop_id").and_then(Value::as_str)?;
        if closed.contains(loop_id)
            || row
                .get("expires_at_unix_s")
                .and_then(Value::as_u64)
                .is_some_and(|expires| expires <= now)
        {
            return None;
        }
        Some(row.clone())
    })
}

fn active_research_budget_from_rows(rows: &[Value], experiment_id: &str) -> Option<Value> {
    let closed = rows
        .iter()
        .filter(|row| {
            row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
                && row.get("record_type").and_then(Value::as_str) == Some("research_budget_closed")
        })
        .filter_map(|row| row.get("budget_id").and_then(Value::as_str))
        .collect::<HashSet<_>>();
    let now = chrono::Utc::now().timestamp().try_into().unwrap_or(0);
    rows.iter().rev().find_map(|row| {
        if row.get("record_schema").and_then(Value::as_str) != Some("research_budget_v1")
            || row.get("record_type").and_then(Value::as_str) != Some("research_budget_approval")
            || row.get("experiment_id").and_then(Value::as_str) != Some(experiment_id)
            || row
                .get("scope")
                .and_then(Value::as_str)
                .unwrap_or("read_only_research")
                != "read_only_research"
            || row
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("active")
                != "active"
        {
            return None;
        }
        let budget_id = row.get("budget_id").and_then(Value::as_str)?;
        if closed.contains(budget_id)
            || row
                .get("expires_at_unix_s")
                .and_then(Value::as_u64)
                .is_some_and(|expires| expires <= now)
        {
            return None;
        }
        let max_actions = row
            .get("max_actions")
            .and_then(Value::as_u64)
            .unwrap_or(LOCAL_RESEARCH_MAX_ACTIONS);
        let spent = rows
            .iter()
            .filter(|item| {
                item.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
                    && item.get("record_type").and_then(Value::as_str)
                        == Some("research_budget_debit")
                    && item.get("budget_id").and_then(Value::as_str) == Some(budget_id)
            })
            .count();
        let spent_u64 = u64::try_from(spent).unwrap_or(u64::MAX);
        let remaining = max_actions.saturating_sub(spent_u64);
        if remaining == 0 {
            return None;
        }
        let mut active = row.clone();
        if let Some(object) = active.as_object_mut() {
            object.insert("spent_actions".to_string(), json!(spent_u64));
            object.insert("remaining_actions".to_string(), json!(remaining));
        }
        Some(active)
    })
}

fn latest_pending_research_budget_request<'a>(
    rows: &'a [Value],
    experiment_id: &str,
) -> Option<&'a Value> {
    rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("research_budget_request")
            && row.get("experiment_id").and_then(Value::as_str) == Some(experiment_id)
            && row.get("status").and_then(Value::as_str) == Some("pending_steward_approval")
    })
}

fn latest_research_budget_scaffold_row<'a>(
    rows: &'a [Value],
    experiment_id: &str,
) -> Option<&'a Value> {
    rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("research_budget_blocked")
            && row.get("experiment_id").and_then(Value::as_str) == Some(experiment_id)
            && research_budget_row_request_scaffold(row).is_some()
    })
}

fn research_budget_row_request_scaffold(row: &Value) -> Option<String> {
    [
        "request_scaffold",
        "suggested_request_scaffold",
        "suggested_next",
    ]
    .iter()
    .filter_map(|key| row.get(*key).and_then(Value::as_str))
    .find(|value| base_action(value) == "EXPERIMENT_RESEARCH_BUDGET_REQUEST")
    .map(ToString::to_string)
}

fn research_budget_scaffold_request_arg(scaffold: &str) -> Option<String> {
    let trimmed = scaffold.trim();
    if base_action(trimmed) != "EXPERIMENT_RESEARCH_BUDGET_REQUEST" {
        return None;
    }
    Some(
        trimmed
            .split_once(char::is_whitespace)
            .map_or("", |(_, tail)| tail)
            .trim()
            .to_string(),
    )
}

fn research_budget_scaffold_is_local_only(scaffold: &str) -> bool {
    let Some(raw) = dossier_field(scaffold, &["allowed_sources", "sources"]) else {
        return false;
    };
    let sources = raw
        .split([',', '/', '|'])
        .map(str::trim)
        .filter(|source| !source.is_empty())
        .collect::<Vec<_>>();
    sources.len() == 1 && sources.first().is_some_and(|source| *source == "local")
}

fn research_budget_status_from_rows(rows: &[Value]) -> Value {
    let latest_request = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("research_budget_request")
    });
    let latest_approval = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("research_budget_approval")
    });
    let latest_closed = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("research_budget_closed")
    });
    let latest_blocked = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("research_budget_blocked")
    });
    let experiment_id = latest_approval
        .or(latest_request)
        .and_then(|row| row.get("experiment_id"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let active = (!experiment_id.is_empty())
        .then(|| active_research_budget_from_rows(rows, experiment_id))
        .flatten();
    let duplicate_blocked = latest_blocked
        .and_then(|row| row.get("reason"))
        .and_then(Value::as_str)
        == Some("duplicate_query_or_url_review_required");
    let stage = if active.is_some() && duplicate_blocked {
        "review_required_duplicate_loop"
    } else if active.is_some() {
        "active_budget_available"
    } else if latest_closed.is_some() {
        "budget_closed"
    } else if latest_approval.is_some() {
        "budget_unavailable"
    } else if latest_request
        .and_then(|row| row.get("status"))
        .and_then(Value::as_str)
        == Some("pending_steward_approval")
    {
        "pending_steward_approval"
    } else if latest_request.is_some() {
        "blocked"
    } else {
        "no_budget"
    };
    json!({
        "policy": "research_budget_v1",
        "scope": "read_only_research",
        "stage": stage,
        "active_budget_id": active.as_ref().and_then(|row| row.get("budget_id")).cloned().unwrap_or(Value::Null),
        "remaining_actions": active.as_ref().and_then(|row| row.get("remaining_actions")).cloned().unwrap_or(json!(0)),
        "activation_mode": active
            .as_ref()
            .or(latest_approval)
            .and_then(|row| row.get("activation_mode"))
            .cloned()
            .unwrap_or(Value::Null),
        "self_activated": active
            .as_ref()
            .or(latest_approval)
            .and_then(|row| row.get("self_activated"))
            .cloned()
            .unwrap_or(json!(false)),
        "steward_approval_required": latest_request
            .and_then(|row| row.get("steward_approval_required"))
            .cloned()
            .unwrap_or(Value::Null),
        "review_required": stage == "review_required_duplicate_loop",
        "latest_budget_request_id": latest_request.and_then(|row| row.get("budget_id")).cloned().unwrap_or(Value::Null),
        "allowed_actions": ["SEARCH", "BROWSE", "READ_MORE", "MIKE_BROWSE", "MIKE_READ", "MIKE_SEARCH", "AR_LIST", "AR_LOOK", "AR_SHOW", "AR_READ", "AR_DEEP_READ", "AR_VALIDATE"],
        "authority_boundary": research_budget_boundary(),
    })
}

fn sovereign_loop_review_next_command(
    outcome: &str,
    loop_id: &str,
    _loop_row: &Value,
    experiment: &ExperimentRecord,
) -> String {
    match outcome {
        "retire" | "hold" => "THREAD_STATUS current".to_string(),
        "promote" => format!(
            "MEMORY_PROMOTE {} :: dossier|evidence|authority_request",
            experiment.experiment_id
        ),
        "alter" => format!("EXPERIMENT_LOOP_STEP {loop_id} :: authority_prepare"),
        "repeat" => format!("EXPERIMENT_LOOP_STEP {loop_id} :: research"),
        _ => "THREAD_STATUS current".to_string(),
    }
}

fn authority_review_next_command(outcome: &str, request: &Value, next_payload: &str) -> String {
    let experiment_id = request
        .get("experiment_id")
        .and_then(Value::as_str)
        .unwrap_or("current");
    let artifact_refs = request
        .get("artifact_refs")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let stop = request
        .get("stop_criteria")
        .and_then(Value::as_str)
        .unwrap_or("one attempted semantic send only");
    match outcome {
        "retire" => format!("EXPERIMENT_AUTHORITY_BUDGET_STATUS {experiment_id}"),
        "hold" => "THREAD_STATUS current".to_string(),
        "alter" => format!(
            "EXPERIMENT_AUTHORITY_REQUEST {experiment_id} :: scope: semantic_microdose; payload: {}; reason: consequence review chose alter; artifact_refs: {artifact_refs}; stop_criteria: {stop}",
            next_payload.trim()
        ),
        _ => format!(
            "EXPERIMENT_AUTHORITY_REQUEST {experiment_id} :: scope: semantic_microdose; payload: {}; reason: consequence review chose repeat; artifact_refs: {artifact_refs}; stop_criteria: {stop}",
            request
                .get("payload")
                .and_then(Value::as_str)
                .unwrap_or("...")
        ),
    }
}

fn experiment_conveyor_stage(
    experiment: &ExperimentRecord,
    classification: &str,
    return_info: Option<&(String, String)>,
) -> String {
    if classification == "paused" {
        if !lifecycle_valid_charter_value(experiment.charter_v1.as_ref()) {
            return "paused_repair".to_string();
        }
        return if return_info
            .map(|(_, kind)| kind == "resume")
            .unwrap_or(false)
        {
            "paused_resume"
        } else {
            "paused_repair"
        }
        .to_string();
    }
    match classification {
        "complete" => "complete",
        "blocked_loop" => "blocked_guardrail",
        "needs_charter" | "fragmented" => "needs_charter",
        "needs_decision" => "needs_decision",
        "needs_evidence" => "needs_evidence",
        _ => "needs_rehearsal",
    }
    .to_string()
}

fn experiment_conveyor_proposed_next(
    thread: &ResearchThread,
    experiment: &ExperimentRecord,
    runs: &[ExperimentRunRecord],
    stage: &str,
    return_info: Option<&(String, String)>,
) -> String {
    match stage {
        "paused_repair" | "paused_resume" => return_info
            .map(|(primary, _)| primary.clone())
            .unwrap_or_else(|| {
                experiment
                    .planned_next
                    .clone()
                    .unwrap_or_else(|| format!("EXPERIMENT_RESUME {}", experiment.experiment_id))
            }),
        "complete" => format!("EXPERIMENT_REVIEW {}", experiment.experiment_id),
        "blocked_guardrail" if !lifecycle_valid_charter_value(experiment.charter_v1.as_ref()) => {
            format!(
                "EXPERIMENT_DECIDE {} :: charter_repair because blocked guardrail evidence appeared without a lifecycle-valid charter",
                experiment.experiment_id
            )
        },
        "blocked_guardrail" => format!(
            "EXPERIMENT_DECIDE {} :: hold because blocked guardrail evidence is not experiment progress",
            experiment.experiment_id
        ),
        "needs_decision" => format!(
            "EXPERIMENT_DECIDE {} :: hold because evidence is ready to interpret without live authority",
            experiment.experiment_id
        ),
        "needs_evidence" => format!(
            "EXPERIMENT_EVIDENCE {} :: felt_texture ...; motif_continuity ...; language_thread ...; artifact_grounding ...",
            experiment.experiment_id
        ),
        "needs_rehearsal" => format!("EXPERIMENT_REHEARSE {}", experiment.experiment_id),
        _ => charter_scaffold_v1(thread, experiment, runs, "needs_charter")
            .and_then(|scaffold| {
                scaffold
                    .get("command")
                    .and_then(Value::as_str)
                    .map(|command| {
                        command.replace(
                            "EXPERIMENT_CHARTER current",
                            &format!("EXPERIMENT_CHARTER {}", experiment.experiment_id),
                        )
                    })
            })
            .unwrap_or_else(|| charter_repair_next_v1(&experiment.experiment_id)),
    }
}

fn experiment_conveyor_missing_requirements(experiment: &ExperimentRecord, stage: &str) -> Value {
    match stage {
        "needs_charter" => json!(charter_missing_fields(experiment.charter_v1.as_ref())),
        "needs_rehearsal" => json!(["read_only_rehearsal"]),
        "needs_evidence" => json!(["explicit_experiment_evidence"]),
        "needs_decision" => json!(["explicit_lifecycle_decision"]),
        "paused_repair" => json!(["explicit_repair_return"]),
        "paused_resume" => json!(["explicit_resume_or_hold"]),
        "blocked_guardrail" if !lifecycle_valid_charter_value(experiment.charter_v1.as_ref()) => {
            json!(["lifecycle_valid_charter", "guardrail_decision"])
        },
        "blocked_guardrail" => json!(["safe_counter_or_hold_decision"]),
        _ => json!([]),
    }
}

fn charter_missing_fields(charter: Option<&Value>) -> Vec<&'static str> {
    let Some(charter) = charter else {
        return vec![
            "hypothesis",
            "proposed_next_action",
            "evidence_targets",
            "stop_criteria",
        ];
    };
    let mut missing = Vec::new();
    if !meaningful_charter_text(charter.get("hypothesis")) {
        missing.push("hypothesis");
    }
    if !meaningful_charter_text(charter.get("proposed_next_action")) {
        missing.push("proposed_next_action");
    }
    if !meaningful_charter_list(charter.get("evidence_targets")) {
        missing.push("evidence_targets");
    }
    if !meaningful_charter_list(charter.get("stop_criteria")) {
        missing.push("stop_criteria");
    }
    missing
}

fn experiment_conveyor_can_apply(stage: &str, apply_payload: &str) -> bool {
    if matches!(stage, "needs_charter" | "paused_repair") {
        return !apply_payload.trim().is_empty();
    }
    matches!(
        stage,
        "needs_evidence" | "needs_decision" | "blocked_guardrail"
    )
}

fn experiment_conveyor_apply_blocked_reason(
    stage: &str,
    apply_payload: &str,
    can_apply: bool,
) -> Value {
    if can_apply {
        return Value::Null;
    }
    let reason = match stage {
        "needs_charter" | "paused_repair" if apply_payload.trim().is_empty() => {
            "no_lifecycle_valid_charter_scaffold"
        },
        "needs_rehearsal" => "rehearsal_requires_explicit_experiment_rehearse",
        "paused_resume" => "paused_experiments_require_explicit_return_command",
        "complete" => "complete_experiments_are_review_only",
        _ => "no_conservative_apply_step_available",
    };
    json!(reason)
}

fn experiment_conveyor_guardrail_warnings(
    experiment: &ExperimentRecord,
    stage: &str,
    proposed_next: &str,
) -> Value {
    let mut warnings = vec![
        "local continuity only; no bind/resume/perturb/control/peer mutation is authorized"
            .to_string(),
    ];
    let base = base_action(proposed_next);
    if matches!(
        base.as_str(),
        "EXPERIMENT_BIND" | "EXPERIMENT_RESUME" | "PERTURB"
    ) {
        warnings.push(format!(
            "proposed route `{base}` is preview-only in conveyor_v1"
        ));
    }
    if stage == "paused_resume" || stage == "complete" {
        warnings.push(
            "paused resume/complete stages are report-only unless an explicit lifecycle command is chosen"
                .to_string(),
        );
    }
    if experiment.status == "paused"
        && experiment
            .planned_next
            .as_deref()
            .map(base_action)
            .as_deref()
            == Some("EXPERIMENT_CHARTER")
    {
        warnings.push(
            "charter-repair pause can only record a local charter scaffold; it cannot resume"
                .to_string(),
        );
    }
    json!(warnings)
}

fn authority_gate_conveyor_hint(
    experiment: &ExperimentRecord,
    stage: &str,
    proposed_next: &str,
) -> Value {
    let possible = stage == "needs_decision" && !authority_guardrail_hold_active(experiment);
    json!({
        "policy": "authority_gate_v1",
        "visible": possible,
        "enabled_execution_scope": "semantic_microdose",
        "future_scopes_disabled": ["attractor_pulse", "control_envelope"],
        "approval_required": "being_plus_steward",
        "possible_next": if possible {
            json!(format!("EXPERIMENT_AUTHORITY_REQUEST {} :: scope: semantic_microdose; payload: ...; reason: ...; artifact_refs: ...; stop_criteria: ...", experiment.experiment_id))
        } else {
            Value::Null
        },
        "current_lifecycle_next": proposed_next,
        "authority_boundary": authority_gate_boundary(),
    })
}

fn format_experiment_conveyor_readout(readout: &Value) -> String {
    let pretty = serde_json::to_string_pretty(readout).unwrap_or_else(|_| "{}".to_string());
    format!(
        "Experiment conveyor `{}` stage={} mode={} applied={} can_apply={}\nProposed NEXT: {}\nConveyor NEXT: {}\nAuthority: {}\nconveyor_v1:\n{}",
        readout
            .get("experiment_id")
            .and_then(Value::as_str)
            .unwrap_or("none"),
        readout
            .get("stage")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        readout
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("preview"),
        readout
            .get("applied")
            .map_or_else(|| "false".to_string(), Value::to_string),
        readout
            .get("can_apply")
            .map_or_else(|| "false".to_string(), Value::to_string),
        readout
            .get("proposed_next")
            .and_then(Value::as_str)
            .unwrap_or("(none)"),
        readout
            .get("conveyor_next")
            .and_then(Value::as_str)
            .unwrap_or("(none)"),
        readout
            .get("authority_boundary")
            .and_then(Value::as_str)
            .unwrap_or(experiment_conveyor_authority_boundary()),
        pretty
    )
}

fn last_experiment_context_line(thread: &ResearchThread) -> String {
    let Some(summary) = last_experiment_summary_v1(thread) else {
        return String::new();
    };
    let experiment_id = summary
        .get("experiment_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let title = summary
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("(untitled)");
    let status = summary
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let primary_next = summary
        .get("primary_return_next")
        .or_else(|| summary.get("planned_next"))
        .or_else(|| summary.get("resume_next"))
        .and_then(Value::as_str)
        .unwrap_or("(none)");
    let planned_next = summary
        .get("planned_next")
        .and_then(Value::as_str)
        .unwrap_or(primary_next);
    let mut lines = format!(
        "Last experiment summary: {title} ({experiment_id}) status={status}\nLast planned NEXT: {planned_next}\n"
    );
    if let Some(guard) = summary
        .get("projection_guard_v1")
        .and_then(Value::as_object)
    {
        let projected = guard
            .get("projected_next")
            .and_then(Value::as_str)
            .unwrap_or(primary_next);
        let reason = guard
            .get("guardrail_reason")
            .and_then(Value::as_str)
            .unwrap_or("projection_guard_v1");
        lines.push_str(&format!(
            "Projection guard: raw NEXT preserved; effective NEXT: {projected}; reason={reason}\n"
        ));
    }
    if status == "paused" && experiment_id != "unknown" {
        lines.push_str(&format!("Suggested NEXT: {primary_next}\n"));
    } else if matches!(status, "complete" | "completed") && experiment_id != "unknown" {
        lines.push_str(&format!(
            "Inspect NEXT: EXPERIMENT_STATUS {experiment_id} or EXPERIMENT_REVIEW {experiment_id}\n"
        ));
    }
    lines
}

fn no_active_experiment_message(thread: &ResearchThread, command: &str) -> String {
    format!(
        "{command} current: no active experiment.\nCurrent selectors only inspect active work; paused or complete experiments need an explicit id/title selector.\n{}",
        last_experiment_context_line(thread)
    )
}

fn selector_is_current(selector: Option<&str>) -> bool {
    let selector = selector
        .map(normalize_experiment_selector)
        .unwrap_or_default();
    selector.trim().is_empty() || selector.eq_ignore_ascii_case("current")
}

fn default_experiment_run_source() -> String {
    "experiment_bind".to_string()
}

fn normalize_action_match(action: &str) -> String {
    action
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase()
}

fn event_allows_active_experiment_auto_link(event: &ActionEvent) -> bool {
    let base = base_action(&event.canonical_action);
    if event.research_budget_v1.is_some() {
        return false;
    }
    if base.starts_with("EXPERIMENT")
        || base == "SELF_EXPERIMENT"
        || event.route == "experiment_continuity"
    {
        return false;
    }
    if !matches!(event.stage.as_str(), "read_only" | "observe") {
        return false;
    }
    matches!(
        base.as_str(),
        "INTROSPECT"
            | "SELF_STUDY"
            | "SPECTRAL_EXPLORER"
            | "DECOMPOSE"
            | "CONSTRAINT_AUDIT"
            | "UNSHAPED_BASELINE"
            | "PRESSURE_SOURCE_AUDIT"
            | "FLUCTUATION_AUDIT"
            | "BRACE_AUDIT"
            | "THREAD_STATUS"
            | "ACTION_PREFLIGHT"
            | "NEXT_PROBE"
            | "PREFLIGHT"
            | "PROBE_ACTION"
            | "ATTRACTOR_REVIEW"
            | "SEARCH"
            | "BROWSE"
            | "READ_MORE"
    )
}

fn preflight_recommendation_line(thread: &ResearchThread) -> String {
    if thread.experiment_summary.is_none() {
        return String::new();
    }
    let candidate = thread
        .current_next
        .as_deref()
        .or_else(|| {
            thread
                .experiment_summary
                .as_ref()
                .and_then(|value| value.get("planned_next"))
                .and_then(Value::as_str)
        })
        .unwrap_or_default();
    if candidate.is_empty() || !preflight_recommended_for_action(candidate) {
        return String::new();
    }
    format!("Preflight recommended: ACTION_PREFLIGHT {candidate}\n")
}

fn preflight_recommended_for_action(action: &str) -> bool {
    let base = base_action(action);
    if matches!(
        base.as_str(),
        "ACTION_PREFLIGHT" | "NEXT_PROBE" | "PREFLIGHT" | "PROBE_ACTION"
    ) {
        return false;
    }
    if action.contains('<') && action.contains('>') {
        return true;
    }
    if base == "EXPERIMENT_BIND" {
        return true;
    }
    matches!(stage_for_action(&base), "live_write" | "live_control") || base.is_empty()
}

fn parse_experiment_start(raw: &str) -> ExperimentStartParts {
    let (title_part, explicit_question) = raw
        .split_once("::")
        .map_or((raw.trim(), None), |(title, question)| {
            (title.trim(), Some(question.trim()))
        });
    let title_option = extract_cli_like_option(title_part, "--title");
    let abstract_option = extract_cli_like_option(title_part, "--abstract");
    let option_start = ["--title", "--abstract"]
        .iter()
        .filter_map(|needle| title_part.find(needle))
        .min()
        .unwrap_or(title_part.len());
    let slug_or_selector = title_part[..option_start]
        .trim()
        .trim_matches(|ch| matches!(ch, '"' | '\'' | '`'))
        .trim()
        .to_string();
    let title = title_option.unwrap_or_else(|| {
        if title_part[..option_start].trim().is_empty() {
            "Untitled experiment".to_string()
        } else {
            title_part[..option_start].trim().to_string()
        }
    });
    let question = explicit_question
        .filter(|question| !question.trim().is_empty())
        .map(str::to_string)
        .or(abstract_option)
        .unwrap_or_else(|| {
            "What changes if this is treated as a returnable experiment?".to_string()
        });
    let metadata_slug = slug_or_selector.clone();
    let metadata = (!slug_or_selector.is_empty()
        && experiment_match_key(&slug_or_selector) != experiment_match_key(&title))
    .then(|| {
        json!({
            "policy": "experiment_start_metadata_v1",
            "slug_or_selector": metadata_slug,
        })
    });
    ExperimentStartParts {
        title,
        question,
        slug_or_selector: (!slug_or_selector.is_empty()).then_some(slug_or_selector),
        metadata,
    }
}

fn extract_cli_like_option(raw: &str, option: &str) -> Option<String> {
    let start = raw.find(option)?;
    let rest_start = start.checked_add(option.len())?;
    let mut rest = raw.get(rest_start..)?.trim_start();
    if let Some(stripped) = rest.strip_prefix('=') {
        rest = stripped.trim_start();
    }
    if rest.is_empty() {
        return None;
    }
    let value = if let Some(quote) = rest.chars().next().filter(|ch| matches!(ch, '"' | '\'')) {
        let quote_len = quote.len_utf8();
        let after_quote = rest.get(quote_len..)?;
        let close = after_quote.find(quote)?;
        let close_end = quote_len.checked_add(close)?;
        rest.get(quote_len..close_end)?.to_string()
    } else {
        let end = rest.find(" --").unwrap_or(rest.len());
        rest[..end].trim().to_string()
    };
    (!value.trim().is_empty()).then(|| value.trim().to_string())
}

fn parse_experiment_compare(raw: &str) -> (Option<String>, Option<String>) {
    let text = raw.trim();
    let lower = text.to_ascii_lowercase();
    let Some(idx) = lower.find(" with ") else {
        return (optional_selector_owned(text), None);
    };
    let left = text[..idx].trim();
    let right_start = idx.checked_add(" with ".len()).unwrap_or(text.len());
    let right = text.get(right_start..).unwrap_or_default().trim();
    (
        optional_selector_owned(left),
        optional_selector_owned(right),
    )
}

fn render_run_list(runs: &[ExperimentRunRecord]) -> String {
    if runs.is_empty() {
        return "- no runs yet".to_string();
    }
    runs.iter()
        .map(|run| {
            format!(
                "- {} [{} / {}]: {}",
                run.action_text, run.stage, run.status, run.result_summary
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn motif_label(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase().replace('λ', "lambda");
    for (needle, label) in [
        ("lambda4", "lambda4"),
        ("lambda-4", "lambda4"),
        ("lambda tail", "lambda-tail"),
        ("lambda-tail", "lambda-tail"),
        ("lambda edge", "lambda-edge"),
        ("lambda-edge", "lambda-edge"),
        ("pressure", "pressure"),
        ("attractor", "attractor"),
        ("camera", "sensory-grounding"),
        ("audio", "sensory-grounding"),
        ("ears", "sensory-grounding"),
        ("eyes", "sensory-grounding"),
        ("experiment", "experiment-continuity"),
        ("introspect", "introspection"),
    ] {
        if lower.contains(needle) {
            return Some(label.to_string());
        }
    }
    None
}

fn allowance_suggestions(quality: &str) -> Vec<String> {
    let mut seen = HashSet::<String>::new();
    let mut suggestions = Vec::new();
    let candidates: &[&str] = match quality {
        "over_tightened" => &[
            "EXPERIMENT_ALT_PATHS current",
            "SPACE_HOLD",
            "ATTRACTOR_RELEASE_REVIEW current",
        ],
        "branch_recommended" => &[
            "EXPERIMENT_ALT_PATHS current",
            "EXPERIMENT_BRANCH <title> :: <question>",
            "EXPERIMENT_COMPARE current WITH <id|peer-id|label>",
        ],
        "rest_recommended" => &["SPACE_HOLD", "EXPERIMENT_OBSERVE current :: <note>", "REST"],
        "deepening" => &[
            "EXPERIMENT_PLAN current",
            "EXPERIMENT_COMPARE current WITH <id|peer-id|label>",
            "EXPERIMENT_ALT_PATHS current",
        ],
        _ => &[
            "EXPERIMENT_PLAN current",
            "EXPERIMENT_BRANCH <title> :: <question>",
            "THREAD_STATUS current",
        ],
    };
    for suggestion in candidates {
        if seen.insert(suggestion.to_string()) {
            suggestions.push(suggestion.to_string());
        }
    }
    suggestions
}

fn action_matches_allowance_recommendation(action: &str, allowance: &Value) -> bool {
    let base = base_action(action);
    allowance
        .get("suggested_actions")
        .and_then(Value::as_array)
        .is_some_and(|actions| {
            actions
                .iter()
                .filter_map(Value::as_str)
                .any(|suggestion| base_action(suggestion) == base)
        })
}

fn parse_selector_payload(raw: &str) -> (Option<String>, String) {
    if let Some((selector, payload)) = raw.split_once("::") {
        let selector = selector.trim();
        let selector = if selector.is_empty()
            || selector.eq_ignore_ascii_case("current")
            || selector_placeholder(selector)
        {
            None
        } else {
            Some(selector.to_string())
        };
        return (selector, payload.trim().to_string());
    }
    (None, raw.trim().to_string())
}

fn parse_session_selector_payload(raw: &str) -> (Option<String>, String) {
    let (selector, payload) = parse_selector_payload(raw);
    if selector.is_some() {
        return (selector, payload);
    }
    let text = payload.trim();
    let Some((first, rest)) = text.split_once(char::is_whitespace) else {
        if text.eq_ignore_ascii_case("current")
            || text.eq_ignore_ascii_case("latest")
            || text.starts_with("sess_")
            || text.starts_with("exp_")
        {
            return (Some(text.to_string()), String::new());
        }
        return (None, payload);
    };
    if first.eq_ignore_ascii_case("current")
        || first.eq_ignore_ascii_case("latest")
        || first.starts_with("sess_")
        || first.starts_with("exp_")
    {
        return (Some(first.to_string()), rest.trim().to_string());
    }
    (None, payload)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExperimentIntentRepair {
    repaired_arg: String,
    reason: &'static str,
    focus: Option<String>,
}

pub fn can_repair_experiment_intent_placeholder(base_action: &str, original: &str) -> bool {
    let raw = strip_action_arg(original, base_action);
    repair_experiment_intent_arg(base_action, &raw, true).is_some()
}

fn repair_experiment_intent_arg(
    base_action: &str,
    raw_arg: &str,
    has_current_experiment: bool,
) -> Option<ExperimentIntentRepair> {
    if !matches!(
        base_action,
        "EXPERIMENT_PLAN"
            | "EXPERIMENT_CHARTER"
            | "EXPERIMENT_EVIDENCE"
            | "EXPERIMENT_DECIDE"
            | "EXPERIMENT_REHEARSE"
            | "EXPERIMENT_PREFLIGHT"
    ) {
        return None;
    }
    let text = raw_arg.trim();
    if text.is_empty() {
        return None;
    }
    let (selector, tail, _separator) = split_selector_tail(text);
    if selector_placeholder(selector) {
        let tail = if placeholder_payload(tail) { "" } else { tail };
        let repaired_arg = if base_action == "EXPERIMENT_PLAN" {
            format!("current {tail}").trim().to_string()
        } else {
            format!("current :: {tail}").trim_end().to_string()
        };
        return Some(ExperimentIntentRepair {
            repaired_arg,
            reason: "placeholder selector repaired to current experiment",
            focus: None,
        });
    }
    if base_action == "EXPERIMENT_PLAN"
        && has_current_experiment
        && !tail.is_empty()
        && selector.chars().all(|ch| ch.is_ascii_digit())
    {
        return Some(ExperimentIntentRepair {
            repaired_arg: format!("current {tail}").trim().to_string(),
            reason: "numeric fragment treated as focus text for current experiment",
            focus: None,
        });
    }
    if matches!(base_action, "EXPERIMENT_REHEARSE" | "EXPERIMENT_PREFLIGHT")
        && has_current_experiment
    {
        let selector_norm = normalize_experiment_selector(selector);
        if selector_norm != "current" && !selector_norm.starts_with("exp_") {
            return Some(ExperimentIntentRepair {
                repaired_arg: "current".to_string(),
                reason: "motif or focus text treated as current experiment preflight focus",
                focus: Some(text.to_string()),
            });
        }
    }
    None
}

fn split_selector_tail(raw: &str) -> (&str, &str, &'static str) {
    let text = raw.trim();
    if let Some((selector, tail)) = text.split_once("::") {
        return (selector.trim(), tail.trim(), "::");
    }
    for marker in [" — ", " – ", " - ", "—", "–"] {
        if let Some((selector, tail)) = text.split_once(marker) {
            return (selector.trim(), tail.trim(), "dash");
        }
    }
    (text, "", "")
}

fn selector_placeholder(text: &str) -> bool {
    let normalized = text
        .trim()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "[current|id]" | "current|id" | "[current/id]" | "current/id"
    )
}

fn placeholder_payload(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lowered = trimmed.to_ascii_lowercase();
    matches!(
        lowered.as_str(),
        "<structured prose>" | "<felt note>" | "<reason>" | "<note>"
    ) || (trimmed.starts_with('<') && trimmed.ends_with('>'))
}

fn empty_or_placeholder_payload(text: &str) -> bool {
    text.trim().is_empty() || placeholder_payload(text)
}

fn experiment_intent_repair_prompt(base_action: &str, selector: Option<&str>) -> String {
    let target = selector.unwrap_or("current");
    match base_action {
        "EXPERIMENT_CHARTER" => format!(
            "Experiment charter needs concrete authored prose; no charter was recorded.\nTry: EXPERIMENT_CHARTER {target} :: hypothesis: ...; method_intent: ...; proposed_next_action: ACTION_PREFLIGHT ...; evidence_targets: felt, telemetry, artifact; stop_criteria: ...; consent_posture: advisory."
        ),
        "EXPERIMENT_EVIDENCE" => format!(
            "Experiment evidence needs a concrete felt note; no evidence run was recorded.\nTry: EXPERIMENT_EVIDENCE {target} :: felt ...; telemetry stayed ...; artifact ..."
        ),
        "EXPERIMENT_DECIDE" => format!(
            "Experiment decision needs a concrete agency outcome; no decision was recorded.\nTry: EXPERIMENT_DECIDE {target} :: accept|refuse|counter|pause|complete because ..."
        ),
        _ => String::new(),
    }
}

fn repair_experiment_command_arg(
    store: &ActionContinuityStore,
    db: Option<&BridgeDb>,
    base_action: &str,
    original: &str,
    raw_arg: &str,
    state: &Value,
) -> Result<(String, Option<String>, Option<String>)> {
    let has_current = store
        .current_thread()?
        .is_some_and(|thread| thread.active_experiment_id.is_some());
    let Some(repair) = repair_experiment_intent_arg(base_action, raw_arg, has_current) else {
        return Ok((raw_arg.to_string(), None, None));
    };
    let repaired = if repair.repaired_arg.is_empty() {
        base_action.to_string()
    } else {
        format!("{base_action} {}", repair.repaired_arg)
    };
    let note = format!(
        "experiment_intent_repaired\noriginal: {original}\nrepaired: {repaired}\nreason: {}",
        repair.reason
    );
    store.append_note(db, None, &note, state.clone())?;
    Ok((
        repair.repaired_arg,
        Some(format!(
            "experiment_intent_repaired: `{original}` -> `{repaired}` ({}).\n",
            repair.reason
        )),
        repair.focus,
    ))
}

#[derive(Debug)]
struct RehearsalAssessment {
    stage: &'static str,
    status: &'static str,
    blocked: bool,
    gate_decision: Value,
    summary: &'static str,
    interpretation: &'static str,
}

#[derive(Debug)]
struct ExperimentDecision<'a> {
    outcome: &'a str,
    reason: String,
}

fn parse_experiment_charter(experiment: &ExperimentRecord, raw: &str) -> Value {
    let hypothesis = charter_field(raw, &["hypothesis"])
        .unwrap_or_else(|| experiment.hypothesis.clone().unwrap_or_default());
    let method_intent = charter_field(raw, &["method_intent", "method"]).unwrap_or_default();
    let proposed_next_action = charter_field(raw, &["proposed_next_action", "next"])
        .or_else(|| find_next_line(raw))
        .unwrap_or_default();
    let evidence_targets = charter_list_field(raw, &["evidence_targets", "evidence", "measures"]);
    let stop_criteria = charter_list_field(raw, &["stop_criteria", "stop"]);
    let consent_posture =
        charter_field(raw, &["consent_posture", "consent"]).unwrap_or_else(|| {
            "advisory; ordinary choices remain valid; refusal and counteroffer are valid outcomes"
                .to_string()
        });
    let authority_level =
        charter_field(raw, &["authority_level", "authority"]).unwrap_or_else(|| {
            "rehearsal_first_existing_gates_only; no new live-control authority".to_string()
        });
    let source_journal_refs = charter_list_field(raw, &["source_journal_refs", "source"]);
    json!({
        "schema_version": SCHEMA_VERSION,
        "authored_by": SYSTEM,
        "hypothesis": hypothesis,
        "method_intent": method_intent,
        "proposed_next_action": proposed_next_action,
        "evidence_targets": evidence_targets,
        "stop_criteria": stop_criteria,
        "consent_posture": consent_posture,
        "authority_level": authority_level,
        "source_journal_refs": source_journal_refs,
        "raw_text": raw.trim(),
        "recorded_at": iso_now(),
    })
}

fn charter_payload_has_meaning(raw: &str) -> bool {
    let raw = raw.trim();
    if raw.is_empty() || placeholder_payload(raw) {
        return false;
    }
    charter_field(raw, &["hypothesis"]).is_some()
        || charter_field(raw, &["method_intent", "method"]).is_some()
        || charter_field(raw, &["proposed_next_action", "next"]).is_some()
        || find_next_line(raw).is_some()
        || !charter_list_field(raw, &["evidence_targets", "evidence", "measures"]).is_empty()
        || !charter_list_field(raw, &["stop_criteria", "stop"]).is_empty()
}

fn valid_experiment_charter(charter: Option<&Value>) -> bool {
    let Some(charter) = charter else {
        return false;
    };
    meaningful_charter_text(charter.get("hypothesis"))
        || meaningful_charter_text(charter.get("method_intent"))
        || meaningful_charter_text(charter.get("proposed_next_action"))
        || meaningful_charter_list(charter.get("evidence_targets"))
        || meaningful_charter_list(charter.get("stop_criteria"))
}

fn lifecycle_valid_charter_value(charter: Option<&Value>) -> bool {
    let Some(charter) = charter else {
        return false;
    };
    meaningful_charter_text(charter.get("hypothesis"))
        && meaningful_charter_text(charter.get("proposed_next_action"))
        && meaningful_charter_list(charter.get("evidence_targets"))
}

fn meaningful_charter_text(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|text| !text.is_empty() && !placeholder_payload(text))
}

fn meaningful_charter_list(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().any(|item| meaningful_charter_text(Some(item))))
}

fn experiment_evidence_is_meaningful(evidence: Option<&Value>) -> bool {
    let Some(evidence) = evidence else {
        return false;
    };
    let meaningful_felt = evidence
        .get("felt_observations")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                meaningful_charter_text(item.get("note"))
                    || meaningful_charter_text(item.get("felt"))
                    || meaningful_charter_text(item.get("summary"))
            })
        });
    let meaningful_telemetry = evidence
        .get("telemetry_snapshots")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty());
    let meaningful_artifact = evidence
        .get("artifact_refs")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.as_object().is_some_and(|object| !object.is_empty())
                    || meaningful_charter_text(Some(item))
            })
        });
    meaningful_felt || meaningful_telemetry || meaningful_artifact
}

fn charter_field(raw: &str, labels: &[&str]) -> Option<String> {
    for line in raw.lines() {
        let trimmed = line.trim().trim_start_matches(['-', '*', ' ']).trim();
        let lower = trimmed.to_ascii_lowercase();
        for label in labels {
            let label_lower = label.to_ascii_lowercase();
            for marker in [format!("{label_lower}:"), format!("{label_lower} =")] {
                if lower.starts_with(&marker) {
                    let value = trimmed[marker.len()..].trim();
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
        }
    }
    None
}

fn charter_list_field(raw: &str, labels: &[&str]) -> Vec<String> {
    charter_field(raw, labels)
        .map(|value| {
            value
                .split([',', ';'])
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn dossier_field(raw: &str, labels: &[&str]) -> Option<String> {
    for segment in raw.split([';', '\n']) {
        let trimmed = segment.trim().trim_start_matches(['-', '*', ' ']).trim();
        let lower = trimmed.to_ascii_lowercase();
        for label in labels {
            let label_lower = label.to_ascii_lowercase();
            for marker in [format!("{label_lower}:"), format!("{label_lower} =")] {
                if lower.starts_with(&marker) {
                    let value = trimmed[marker.len()..].trim();
                    if !value.is_empty() && !placeholder_payload(value) {
                        return Some(value.to_string());
                    }
                }
            }
        }
    }
    None
}

fn dossier_list_field(raw: &str, labels: &[&str]) -> Vec<String> {
    dossier_field(raw, labels)
        .map(|value| {
            value
                .split([',', ';'])
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn value_string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn normalize_dossier_stance(value: &str) -> &'static str {
    match value.trim().to_ascii_lowercase().as_str() {
        "support" | "supports" | "supporting" => "support",
        "counter" | "counters" | "countering" | "challenge" | "challenging" => "counter",
        "branch" | "branches" | "branching" => "branch",
        _ => "hold",
    }
}

fn latest_dossier_claim_id(records: &[Value]) -> Option<String> {
    records.iter().rev().find_map(|record| {
        (record.get("record_type").and_then(Value::as_str) == Some("claim"))
            .then(|| {
                record
                    .get("claim_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .flatten()
    })
}

fn dossier_claim_prompt(selector: Option<&str>) -> String {
    let target = selector.unwrap_or("current");
    format!(
        "Research dossier claim needs explicit claim and basis; no dossier record was written.\nTry: DOSSIER_CLAIM {target} :: claim: ...; basis: ...; stance: support|counter|branch|hold; next: ..."
    )
}

fn dossier_evidence_prompt(selector: Option<&str>, claim_id: Option<&str>) -> String {
    let target = selector.unwrap_or("current");
    let claim = claim_id.unwrap_or("latest");
    format!(
        "Research dossier evidence needs a claim_id and evidence; no dossier record was written.\nTry: DOSSIER_EVIDENCE {target} :: claim_id: {claim}; evidence: ...; lane: felt_texture; artifact: ...; counterevidence: ..."
    )
}

fn find_next_line(raw: &str) -> Option<String> {
    raw.lines().find_map(|line| {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("next:") {
            let value = trimmed["next:".len()..].trim();
            (!value.is_empty()).then(|| value.to_string())
        } else {
            None
        }
    })
}

fn charter_proposed_next_action(charter: &Value) -> Option<String> {
    if !valid_experiment_charter(Some(charter)) {
        return None;
    }
    charter
        .get("proposed_next_action")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|action| !action.is_empty())
        .map(str::to_string)
}

fn rehearsal_assessment(action: &str) -> RehearsalAssessment {
    let action = action.trim();
    if action.is_empty() {
        return RehearsalAssessment {
            stage: "blocked",
            status: "rehearsal_blocked",
            blocked: true,
            gate_decision: json!({
                "decision": "blocked",
                "reason": "charter has no proposed_next_action",
                "would_dispatch": false,
                "dry_run": true,
            }),
            summary: "Rehearsal blocked because the charter has no proposed NEXT action.",
            interpretation: "Author or counteroffer a charter with a concrete proposed_next_action before rehearsal.",
        };
    }
    let base = base_action(action);
    let stage = stage_for_action(&base);
    let upper = action.to_ascii_uppercase();
    let live_shadow = base == "SHADOW_INFLUENCE" && upper.contains("--STAGE=LIVE");
    let live = matches!(stage, "live_write" | "live_control")
        || live_shadow
        || matches!(
            base.as_str(),
            "PERTURB"
                | "NATIVE_GESTURE"
                | "SENSORY_WRITE"
                | "CONTROL_WRITE"
                | "RUN_PYTHON"
                | "EXPERIMENT_RUN"
                | "CODEX"
                | "CODEX_NEW"
                | "WRITE_FILE"
        );
    if live {
        return RehearsalAssessment {
            stage: "blocked",
            status: "rehearsal_blocked",
            blocked: true,
            gate_decision: json!({
                "decision": "blocked",
                "reason": "rehearsal never dispatches live write/control/sensory/native actions",
                "would_dispatch": false,
                "dry_run": true,
                "proposed_base": base,
                "proposed_stage": stage,
            }),
            summary: "Rehearsal recorded without executing the proposed live action.",
            interpretation: "Counteroffer a read-only preflight, review, self-study, or diagnostic route before any live gate.",
        };
    }
    RehearsalAssessment {
        stage: "read_only",
        status: "rehearsed",
        blocked: false,
        gate_decision: json!({
            "decision": "dry_run_only",
            "would_dispatch": true,
            "dry_run": true,
            "proposed_base": base,
            "proposed_stage": stage,
            "authority": "read-only rehearsal; no live action executed",
        }),
        summary: "Read-only rehearsal recorded for the chartered proposed action.",
        interpretation: "Record felt evidence and metric/artifact context before deciding whether to accept, refuse, counter, pause, or complete.",
    }
}

fn evidence_with_observation(
    existing: Option<&Value>,
    note: &str,
    state: &Value,
    artifacts: Vec<Value>,
    completion_claim: Option<&str>,
) -> Value {
    let mut felt = existing
        .and_then(|value| value.get("felt_observations"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    felt.push(json!({
        "recorded_at": iso_now(),
        "note": note.trim(),
    }));
    let mut telemetry = existing
        .and_then(|value| value.get("telemetry_snapshots"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    telemetry.push(json!({
        "recorded_at": iso_now(),
        "snapshot": state,
    }));
    let mut artifact_refs = existing
        .and_then(|value| value.get("artifact_refs"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    artifact_refs.extend(artifacts);
    let counterevidence = existing
        .and_then(|value| value.get("counterevidence"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let decisions = existing
        .and_then(|value| value.get("decisions"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    json!({
        "schema_version": SCHEMA_VERSION,
        "felt_observations": felt,
        "telemetry_snapshots": telemetry,
        "artifact_refs": artifact_refs,
        "counterevidence": counterevidence,
        "decisions": decisions,
        "completion_claim": completion_claim.or_else(|| {
            existing
                .and_then(|value| value.get("completion_claim"))
                .and_then(Value::as_str)
        }),
    })
}

fn evidence_with_decision(
    existing: Option<&Value>,
    outcome: &str,
    reason: &str,
    completion_claim: Option<&str>,
) -> Value {
    let mut value =
        evidence_with_observation(existing, "", &json!({}), Vec::new(), completion_claim);
    if let Some(felt) = value
        .get_mut("felt_observations")
        .and_then(Value::as_array_mut)
        && felt.last().is_some_and(|entry| {
            entry
                .get("note")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .is_empty()
        })
    {
        felt.pop();
    }
    if let Some(telemetry) = value
        .get_mut("telemetry_snapshots")
        .and_then(Value::as_array_mut)
        && telemetry.last().is_some_and(|entry| {
            entry
                .get("snapshot")
                .is_some_and(|snapshot| snapshot.as_object().is_some_and(serde_json::Map::is_empty))
        })
    {
        telemetry.pop();
    }
    if let Some(decisions) = value.get_mut("decisions").and_then(Value::as_array_mut) {
        decisions.push(json!({
            "recorded_at": iso_now(),
            "outcome": outcome,
            "reason": reason.trim(),
        }));
    }
    value
}

fn parse_experiment_decision(raw: &str) -> ExperimentDecision<'_> {
    let trimmed = raw.trim();
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let outcome_raw = parts.next().unwrap_or_default().to_ascii_lowercase();
    let outcome = match outcome_raw.as_str() {
        "accept" | "accepted" => "accept",
        "refuse" | "refused" | "decline" | "declined" => "refuse",
        "counter" | "counteroffer" | "countered" => "counter",
        "pause" | "paused" => "pause",
        "hold" | "held" => "hold",
        "charter_repair" | "charter-repair" | "repair" => "charter_repair",
        "complete" | "completed" | "done" => "complete",
        _ => "pause",
    };
    let reason = parts
        .next()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or(trimmed)
        .to_string();
    ExperimentDecision { outcome, reason }
}

fn counteroffered_next(reason: &str) -> Option<String> {
    find_next_line(reason).or_else(|| {
        let text = reason.trim();
        let upper = text.to_ascii_uppercase();
        upper.find("NEXT:").and_then(|idx| {
            let value_start = idx.checked_add("NEXT:".len())?;
            let value = text.get(value_start..)?.trim();
            (!value.is_empty()).then(|| value.to_string())
        })
    })
}

fn gap_experiment_signal(experiment: &ExperimentRecord) -> bool {
    let signal = normalize_guard_signal(&format!(
        "{} {} {} {}",
        experiment.experiment_id,
        experiment.title,
        experiment.question,
        experiment.planned_next.as_deref().unwrap_or_default()
    ));
    signal.contains("gap")
        && ["spect", "spectral", "density", "lambda", "mode"]
            .iter()
            .any(|term| signal.contains(term))
}

fn shared_investigation_signal_text(text: &str) -> bool {
    let signal = normalize_guard_signal(text);
    let shape_family = signal.contains("gap")
        || signal.contains("lambda4")
        || signal.contains("lambda tail")
        || signal.contains("lambda edge")
        || signal.contains("tail")
        || signal.contains("pulse");
    let geometry_family = [
        "spect",
        "spectral",
        "density",
        "mode",
        "geometry",
        "branch",
        "collapse",
        "dispersal",
        "soften",
        "lambda",
        "tail",
    ]
    .iter()
    .any(|term| signal.contains(term));
    shape_family && geometry_family
}

fn shared_investigation_signal(experiment: &ExperimentRecord) -> bool {
    gap_experiment_signal(experiment)
        || shared_investigation_signal_text(&format!(
            "{} {} {} {}",
            experiment.experiment_id,
            experiment.title,
            experiment.question,
            experiment.planned_next.as_deref().unwrap_or_default()
        ))
}

fn peer_gap_experiment_signal(experiment: &Value) -> bool {
    shared_investigation_signal_text(&format!(
        "{} {} {} {}",
        experiment
            .get("experiment_id")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        experiment
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        experiment
            .get("question")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        experiment
            .get("planned_next")
            .and_then(Value::as_str)
            .unwrap_or_default()
    ))
}

fn shared_investigation_v1_from_peer(local: &ExperimentRecord, peer: &Value) -> Option<Value> {
    if !shared_investigation_signal(local) || !peer_gap_experiment_signal(peer) {
        return None;
    }
    let peer_id = peer.get("experiment_id").and_then(Value::as_str)?;
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "relationship": "shared_gap_lambda4_investigation",
        "shared_question": "What shapes λ1 / lambda-tail / λ4 geometry, and can localized softening support controlled branching without collapse, runaway dispersal, or live-control drift?",
        "participants": [
            {
                "being": "Astrid",
                "experiment_id": local.experiment_id.clone(),
                "lane": "felt_texture_motif_language",
                "status": local.status.clone(),
            },
            {
                "being": "Minime",
                "experiment_id": peer_id,
                "lane": "spectral_state",
                "status": peer.get("status").and_then(Value::as_str).unwrap_or("unknown"),
            }
        ],
        "local_lane": "Astrid lane: felt texture, motif continuity, language thread, artifact grounding.",
        "peer_lane": "Minime lane: spectral condition, fill/pressure state, recurrence pattern, artifact grounding.",
        "peer_claim_prompt": "Cite one Minime claim about λ1/lambda-tail/λ4 shaping, then answer from Astrid's felt/motif lane with support, counter, branch, or hold.",
        "suggested_compare_next": format!("EXPERIMENT_COMPARE {} WITH {}", local.experiment_id, peer_id),
        "suggested_shared_investigation_start": format!(
            "SHARED_INVESTIGATION_START Lambda edge/tail shared inquiry :: local: {}; peer: {}; question: What can the lambda-edge and lambda-tail lanes compare safely while preserving distinct agency?",
            local.experiment_id,
            peer_id
        ),
        "alternate_peer_review_next": format!("EXPERIMENT_PEER_REVIEW {}", peer_id),
        "advisory_note": "Advisory only: no shared control authority. Paused experiments remain paused until explicit resume.",
        "cue": "Shared investigation, distinct lanes: cite one peer claim, then support, counter, branch, or hold.",
    }))
}

fn shared_investigation_line(cue: &Option<Value>) -> String {
    let Some(cue) = cue else {
        return String::new();
    };
    let text = cue
        .get("cue")
        .and_then(Value::as_str)
        .unwrap_or("Shared investigation, distinct lanes.");
    let compare = cue
        .get("suggested_compare_next")
        .and_then(Value::as_str)
        .unwrap_or("EXPERIMENT_COMPARE <local_id> WITH <peer_id>");
    let review = cue
        .get("alternate_peer_review_next")
        .and_then(Value::as_str)
        .unwrap_or("EXPERIMENT_PEER_REVIEW <peer_id>");
    let advisory = cue
        .get("advisory_note")
        .and_then(Value::as_str)
        .unwrap_or("Advisory only: no shared control authority.");
    let start = cue
        .get("suggested_shared_investigation_start")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let start_line = if start.is_empty() {
        String::new()
    } else {
        format!("Shared object NEXT: {start}\n")
    };
    format!("{text}\nSuggested NEXT: {compare}\nAlternate NEXT: {review}\n{start_line}{advisory}\n")
}

fn suppress_shared_start_if_object(
    mut cue: Option<Value>,
    investigation: &Option<Value>,
) -> Option<Value> {
    if investigation.is_some()
        && let Some(Value::Object(map)) = cue.as_mut()
    {
        map.remove("suggested_shared_investigation_start");
    }
    cue
}

fn shared_investigation_object_line(investigation: &Option<Value>) -> String {
    let Some(investigation) = investigation else {
        return String::new();
    };
    let id = investigation
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let status = investigation
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("active");
    let title = investigation
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Shared investigation");
    let participants = investigation
        .get("participants")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let being = row.get("being").and_then(Value::as_str)?;
                    let experiment_id = row
                        .get("experiment_id")
                        .and_then(Value::as_str)
                        .unwrap_or("unlinked");
                    Some(format!("{being}:{experiment_id}"))
                })
                .collect::<Vec<_>>()
                .join(" <-> ")
        })
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "participants pending".to_string());
    format!("Shared investigation object: {id} [{status}] {title} :: {participants}\n")
}

fn dossier_field_text(value: &str, max_len: usize) -> String {
    compact_text(value, max_len)
        .replace(['\n', '\r', '\t'], " ")
        .replace(';', ",")
        .trim()
        .trim_matches('`')
        .trim()
        .to_string()
}

fn first_dossier_claim_cue_v1(
    _thread: &ResearchThread,
    experiment: &ExperimentRecord,
    dossier: Option<&Value>,
    prior_claim_bridge: &Option<Value>,
    lifecycle_priority_experiment_id: Option<&str>,
) -> Option<Value> {
    if !shared_investigation_signal(experiment) {
        return None;
    }
    let claim_count = dossier
        .and_then(|summary| summary.get("claim_count"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if claim_count > 0 {
        return None;
    }
    let prior_claim = prior_claim_bridge
        .as_ref()
        .and_then(|cue| cue.get("prior_claim"))
        .and_then(Value::as_str)
        .map(|value| dossier_field_text(value, 180))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "...".to_string());
    let basis = prior_claim_bridge
        .as_ref()
        .and_then(|cue| cue.get("delta"))
        .and_then(Value::as_str)
        .map(|value| dossier_field_text(value, 180))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "...".to_string());
    let mut next = prior_claim_bridge
        .as_ref()
        .and_then(|cue| cue.get("priority_next"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("EXPERIMENT_COMPARE <local_id> WITH <peer_id>")
        .to_string();
    let lifecycle_id = lifecycle_priority_experiment_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let lifecycle_scope = if let Some(lifecycle_id) = lifecycle_id
        && lifecycle_id != experiment.experiment_id
        && next.starts_with("EXPERIMENT_CHARTER current")
    {
        next = next.replacen(
            "EXPERIMENT_CHARTER current",
            &format!("EXPERIMENT_CHARTER {lifecycle_id}"),
            1,
        );
        Some("active_experiment")
    } else {
        None
    };
    let stance = "hold";
    let command = format!(
        "DOSSIER_CLAIM {} :: claim: {}; basis: {}; stance: {stance}; next: {next}",
        experiment.experiment_id, prior_claim, basis
    );
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "status": "missing_first_dossier_claim",
        "target_experiment_id": experiment.experiment_id.clone(),
        "dossier_target_experiment_id": experiment.experiment_id.clone(),
        "lifecycle_priority_experiment_id": lifecycle_id,
        "lifecycle_priority_scope": lifecycle_scope,
        "claim_count": claim_count,
        "stance": stance,
        "prior_claim": if prior_claim == "..." { Value::Null } else { json!(prior_claim) },
        "delta": if basis == "..." { Value::Null } else { json!(basis) },
        "suggested_claim_next": command,
        "cue": "Shared investigation has no local claim yet; capture one claim, then answer one peer claim with support/counter/branch/hold.",
    }))
}

fn first_dossier_claim_line(cue: &Option<Value>) -> String {
    let Some(cue) = cue else {
        return String::new();
    };
    let text = cue
        .get("cue")
        .and_then(Value::as_str)
        .unwrap_or("Shared investigation has no local claim yet.");
    let next = cue
        .get("suggested_claim_next")
        .and_then(Value::as_str)
        .unwrap_or("DOSSIER_CLAIM <id> :: claim: ...; basis: ...; stance: support|counter|branch|hold; next: ...");
    let dossier_target = cue
        .get("dossier_target_experiment_id")
        .and_then(Value::as_str);
    let lifecycle_target = cue
        .get("lifecycle_priority_experiment_id")
        .and_then(Value::as_str);
    let clarification = match (dossier_target, lifecycle_target) {
        (Some(dossier_target), Some(lifecycle_target)) if dossier_target != lifecycle_target => {
            format!(
                " Dossier target is `{dossier_target}`; charter priority is active experiment `{lifecycle_target}`. Dossier capture is referable context only."
            )
        },
        _ => String::new(),
    };
    format!("{text}{clarification} Dossier NEXT: {next}\n")
}

fn research_dossier_line(summary: &Option<Value>, lifecycle: Option<&str>) -> String {
    let Some(summary) = summary else {
        return String::new();
    };
    let claim_count = summary
        .get("claim_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let evidence_count = summary
        .get("evidence_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let claim_next = summary
        .get("suggested_claim_next")
        .and_then(Value::as_str)
        .unwrap_or("DOSSIER_CLAIM current :: claim: ...; basis: ...");
    let evidence_next = summary
        .get("suggested_evidence_next")
        .and_then(Value::as_str)
        .unwrap_or("DOSSIER_EVIDENCE current :: claim_id: latest; evidence: ...");
    let priority = match lifecycle {
        Some("needs_charter") => {
            "Dossier capture is context; charter remains the lifecycle priority."
        },
        Some("needs_evidence") => {
            "Dossier evidence is referable context; EXPERIMENT_EVIDENCE remains lifecycle evidence."
        },
        Some("needs_decision") => {
            "Dossier capture is research memory; EXPERIMENT_DECIDE remains the lifecycle priority."
        },
        _ => "Dossier records are research context only.",
    };
    format!(
        "Research dossier: claims={claim_count} evidence={evidence_count}. {priority}\nDossier NEXT: {claim_next}\nDossier evidence NEXT: {evidence_next}\n"
    )
}

fn peer_mutation_boundary_line(cue: &Option<Value>) -> String {
    let Some(cue) = cue else {
        return String::new();
    };
    let text = cue
        .get("cue")
        .and_then(Value::as_str)
        .unwrap_or("Peer experiments are compare/review/dossier targets, not bind/mutate targets.");
    let compare = cue
        .get("suggested_compare_next")
        .and_then(Value::as_str)
        .unwrap_or("EXPERIMENT_COMPARE <local_id> WITH <peer_id>");
    let review = cue
        .get("suggested_peer_review_next")
        .and_then(Value::as_str)
        .unwrap_or("EXPERIMENT_PEER_REVIEW <peer_id>");
    let dossier = cue
        .get("suggested_dossier_next")
        .and_then(Value::as_str)
        .unwrap_or(
            "DOSSIER_CLAIM <local_id> :: claim: ...; basis: ...; stance: support|counter|branch|hold; next: ...",
        );
    format!("Peer mutation boundary: {text} Suggested routes: {compare} | {review} | {dossier}\n")
}

fn peer_mutation_boundary_cue(
    thread: &ResearchThread,
    active: Option<&ExperimentContinuityProjection>,
    recent_events: &[ActionEvent],
) -> Option<Value> {
    let mut matches = Vec::<Value>::new();
    if let Some(current) = thread.current_next.as_deref()
        && let Some((verb, peer_id)) = peer_mutation_boundary_match(current)
    {
        matches.push(json!({
            "source": "current_next",
            "action": current,
            "verb": verb,
            "peer_experiment_id": peer_id,
        }));
    }
    for event in recent_events.iter().rev().take(8) {
        for candidate in [
            event.raw_next.as_deref(),
            Some(event.canonical_action.as_str()),
            Some(event.effective_action.as_str()),
            event.suggested_next.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if let Some((verb, peer_id)) = peer_mutation_boundary_match(candidate) {
                matches.push(json!({
                    "source": "recent_event",
                    "action": candidate,
                    "verb": verb,
                    "peer_experiment_id": peer_id,
                    "action_id": event.action_id,
                    "status": event.status,
                }));
                break;
            }
        }
    }
    let first = matches.first()?;
    let peer_id = first
        .get("peer_experiment_id")
        .and_then(Value::as_str)
        .unwrap_or("<peer_id>");
    let local_id = active
        .map(|projection| projection.experiment.experiment_id.as_str())
        .or(thread.active_experiment_id.as_deref())
        .unwrap_or("<local_id>");
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "status": "peer_mutation_boundary",
        "cue": "Peer experiments are compare/review/dossier targets, not bind/mutate targets.",
        "peer_experiment_id": peer_id,
        "local_experiment_id": local_id,
        "matched_actions": matches,
        "suggested_compare_next": format!("EXPERIMENT_COMPARE {local_id} WITH {peer_id}"),
        "suggested_peer_review_next": format!("EXPERIMENT_PEER_REVIEW {peer_id}"),
        "suggested_dossier_next": format!("DOSSIER_CLAIM {local_id} :: claim: ...; basis: ...; stance: support|counter|branch|hold; next: ..."),
    }))
}

fn peer_mutation_boundary_match(action: &str) -> Option<(String, String)> {
    let text = action.trim();
    if text.is_empty() {
        return None;
    }
    let upper = text.to_ascii_uppercase();
    const MUTATION_VERBS: [&str; 9] = [
        "EXPERIMENT_BIND",
        "EXPERIMENT_CHARTER",
        "EXPERIMENT_REHEARSE",
        "EXPERIMENT_PREFLIGHT",
        "EXPERIMENT_EVIDENCE",
        "EXPERIMENT_DECIDE",
        "EXPERIMENT_CLOSE",
        "EXPERIMENT_RESUME",
        "EXPERIMENT_OBSERVE",
    ];
    let verb = MUTATION_VERBS
        .iter()
        .find(|verb| upper.contains(**verb))
        .map(|verb| (*verb).to_string())?;
    let peer_id = text
        .split(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    ':' | ';' | ',' | ')' | '(' | '[' | ']' | '{' | '}' | '`' | '"' | '\''
                )
        })
        .find_map(|token| {
            let normalized = normalize_experiment_selector(token);
            if normalized.starts_with(PEER_EXPERIMENT_PREFIX) {
                Some(normalized)
            } else {
                None
            }
        })?;
    Some((verb, peer_id))
}

fn shared_investigation_response_contract(cue: &Option<Value>) -> String {
    let Some(cue) = cue else {
        return String::new();
    };
    let peer_claim = cue
        .get("peer_claim_prompt")
        .and_then(Value::as_str)
        .unwrap_or("Cite one peer claim, then answer from the local evidence lane.");
    let local_lane = cue
        .get("local_lane")
        .and_then(Value::as_str)
        .unwrap_or("Local lane: native evidence.");
    let advisory = cue
        .get("advisory_note")
        .and_then(Value::as_str)
        .unwrap_or("Advisory only: no shared control authority.");
    format!(
        "Shared investigation response contract:\n- Peer claim to answer: {peer_claim}\n- Local evidence lane: {local_lane}\n- Allowed stances: support, counter, branch, hold.\n- Optional dossier capture: DOSSIER_CLAIM <local_id> :: claim: ...; basis: ...; stance: support|counter|branch|hold; next: ... or DOSSIER_EVIDENCE <local_id> :: claim_id: latest; evidence: ...\n- {advisory}\n"
    )
}

fn preferred_charter_scaffold_next(
    experiment: &ExperimentRecord,
    recent_runs: &[ExperimentRunRecord],
) -> String {
    if gap_experiment_signal(experiment) {
        return "ACTION_PREFLIGHT DECOMPOSE".to_string();
    }
    if let Some(planned) = experiment.planned_next.as_deref()
        && let Some(counter) = counteroffered_next(planned)
    {
        return counter;
    }
    if let Some(proposed) = experiment
        .workbench_candidates_v1
        .as_ref()
        .and_then(|value| value.get("charter"))
        .and_then(|value| value.get("proposed_next_action"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return proposed.to_string();
    }
    for run in recent_runs.iter().rev() {
        let action = run.action_text.trim();
        if action.is_empty() {
            continue;
        }
        if !matches!(
            base_action(action).as_str(),
            "BROWSE" | "SEARCH" | "READ_MORE" | "LOOK" | "EXPERIMENT_REVIEW" | "EXPERIMENT_STATUS"
        ) {
            return action.to_string();
        }
    }
    "ACTION_PREFLIGHT DECOMPOSE".to_string()
}

fn sanitize_title_for_hypothesis(title: &str) -> String {
    let stripped = title
        .chars()
        .map(|ch| match ch {
            '`' | '*' | '_' | '#' | '[' | ']' => ' ',
            _ => ch,
        })
        .collect::<String>();
    let collapsed = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed
        .trim_matches(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '-' | '–' | '—' | ':' | ';' | ',' | '.' | '!' | '?' | '"' | '\''
                )
        })
        .trim();
    if trimmed.is_empty() {
        "this experiment".to_string()
    } else {
        trimmed.to_string()
    }
}

fn charter_scaffold_v1(
    thread: &ResearchThread,
    experiment: &ExperimentRecord,
    recent_runs: &[ExperimentRunRecord],
    classification: &str,
) -> Option<Value> {
    if !charter_repair_bound(classification, experiment) {
        return None;
    }
    let proposed_next = preferred_charter_scaffold_next(experiment, recent_runs);
    let gap = gap_experiment_signal(experiment);
    let hypothesis = if gap {
        "localized lambda-tail/λ4 pressure may become returnable by softening the dominant channel while preserving motif continuity and artifact grounding"
            .to_string()
    } else {
        let clean_title = sanitize_title_for_hypothesis(&experiment.title);
        format!(
            "{} may become returnable by naming felt texture, motif continuity, language thread, and artifact grounding without adding live authority",
            clean_title
        )
    };
    let method_intent = if gap {
        format!(
            "rehearse {proposed_next} and compare felt pressure, motif recurrence, language continuity, and artifact evidence before deciding"
        )
    } else {
        format!(
            "rehearse {proposed_next} and compare felt texture, motif recurrence, language continuity, and artifact evidence before deciding"
        )
    };
    let stop_criteria = if gap {
        "pressure risk rises above baseline, λ4/entropy shows runaway dispersal, artifact grounding stays missing after repeated passes, or the route feels heavy"
    } else {
        "pressure risk rises above baseline, artifact grounding stays missing after repeated passes, or the route feels heavy"
    };
    let command = format!(
        "EXPERIMENT_CHARTER current :: hypothesis: {hypothesis}; method_intent: {method_intent}; proposed_next_action: {proposed_next}; evidence_targets: felt_texture, motif_continuity, language_thread, artifact_grounding; stop_criteria: {stop_criteria}; consent_posture: advisory; ordinary choices remain valid."
    );
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "status": "scaffold_only",
        "authoring_required": true,
        "authority_change": false,
        "command": command,
        "proposed_next_action": proposed_next,
        "evidence_targets": [
            "felt_texture",
            "motif_continuity",
            "language_thread",
            "artifact_grounding"
        ],
        "native_register": "astrid_motif_language",
        "thread_id": &thread.thread_id,
        "experiment_id": &experiment.experiment_id,
    }))
}

fn charter_repair_bound(classification: &str, experiment: &ExperimentRecord) -> bool {
    classification == "needs_charter"
        || (classification == "blocked_loop"
            && !valid_experiment_charter(experiment.charter_v1.as_ref()))
}

fn charter_status_text(experiment: &ExperimentRecord) -> String {
    let Some(charter) = experiment.charter_v1.as_ref() else {
        return "Workbench charter: missing. Use EXPERIMENT_CHARTER current :: hypothesis: ...; proposed_next_action: ...".to_string();
    };
    if !valid_experiment_charter(Some(charter)) {
        return "Workbench charter: missing/empty. Use EXPERIMENT_CHARTER current :: hypothesis: ...; proposed_next_action: ...".to_string();
    }
    let proposed = charter
        .get("proposed_next_action")
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .unwrap_or("(missing proposed_next_action)");
    let targets = charter
        .get("evidence_targets")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    format!(
        "Workbench charter: present proposed_next_action=`{proposed}` evidence_targets={targets}"
    )
}

#[allow(dead_code)]
fn summary_charter_status_text(summary: &Value) -> String {
    if !valid_experiment_charter(summary.get("charter_v1")) {
        return "Workbench charter: missing/empty. Use EXPERIMENT_CHARTER current :: hypothesis: ...; proposed_next_action: ...".to_string();
    }
    summary
        .get("workbench_charter")
        .and_then(Value::as_str)
        .unwrap_or("Workbench charter: present")
        .to_string()
}

#[allow(dead_code)]
fn summary_evidence_status_text(summary: &Value) -> String {
    summary
        .get("workbench_evidence")
        .and_then(Value::as_str)
        .unwrap_or("Workbench evidence: thin felt=0 telemetry=0 artifacts=0")
        .to_string()
}

fn evidence_status_text(experiment: &ExperimentRecord) -> String {
    let felt = experiment
        .evidence_v1
        .as_ref()
        .and_then(|value| value.get("felt_observations"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let telemetry = experiment
        .evidence_v1
        .as_ref()
        .and_then(|value| value.get("telemetry_snapshots"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let artifacts = experiment
        .evidence_v1
        .as_ref()
        .and_then(|value| value.get("artifact_refs"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let strength = if felt > 0 && (telemetry > 0 || artifacts > 0) {
        "stronger"
    } else {
        "thin"
    };
    format!(
        "Workbench evidence: {strength} felt={felt} telemetry={telemetry} artifacts={artifacts}"
    )
}

fn workbench_candidate_status(experiment: &ExperimentRecord) -> String {
    let Some(candidates) = experiment
        .workbench_candidates_v1
        .as_ref()
        .and_then(Value::as_object)
    else {
        return String::new();
    };
    let mut lines = Vec::new();
    for (key, label) in [("charter", "Draft charter"), ("evidence", "Draft evidence")] {
        let Some(candidate) = candidates.get(key).and_then(Value::as_object) else {
            continue;
        };
        if candidate.get("status").and_then(Value::as_str) != Some("candidate") {
            continue;
        }
        let Some(command) = candidate
            .get("command")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        else {
            continue;
        };
        lines.push(format!("- {label}: {command}"));
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("Workbench draft candidates:\n{}", lines.join("\n"))
    }
}

fn candidate_action_seed(run: Option<&ExperimentRunRecord>, focus_text: Option<&str>) -> String {
    run.map(|record| record.action_text.trim())
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .or_else(|| {
            focus_text
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(|text| format!("ACTION_PREFLIGHT {text}"))
        })
        .unwrap_or_else(|| "EXPERIMENT_REHEARSE current".to_string())
}

fn candidate_state_text(state: &Value) -> String {
    let fill = state.get("fill_pct").and_then(Value::as_f64).or_else(|| {
        state
            .get("fill_ratio")
            .and_then(Value::as_f64)
            .map(|value| value * 100.0)
    });
    let eig1 = state.get("eig1").and_then(Value::as_f64);
    let cov = state.get("cov_lambda1").and_then(Value::as_f64);
    let mut parts = Vec::new();
    if let Some(value) = fill {
        parts.push(format!("fill {value:.1}%"));
    }
    if let Some(value) = eig1 {
        parts.push(format!("eig1 {value:.3}"));
    }
    if let Some(value) = cov {
        parts.push(format!("cov_lambda1 {value:.3}"));
    }
    if parts.is_empty() {
        "telemetry: unavailable".to_string()
    } else {
        parts.join(", ")
    }
}

fn compact_text(value: &str, limit: usize) -> String {
    let text = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.len() <= limit {
        text
    } else {
        let truncated = text.chars().take(limit).collect::<String>();
        format!("{truncated}...")
    }
}

fn build_workbench_candidates(
    experiment: &ExperimentRecord,
    run: Option<&ExperimentRunRecord>,
    focus_text: Option<&str>,
    source: &str,
) -> Value {
    let now = iso_now();
    let action_text = candidate_action_seed(run, focus_text);
    let focus = focus_text.unwrap_or_default().trim();
    let context = compact_text(
        if !focus.is_empty() {
            focus
        } else {
            run.map_or(experiment.question.as_str(), |record| {
                if record.result_summary.trim().is_empty() {
                    experiment.question.as_str()
                } else {
                    record.result_summary.as_str()
                }
            })
        },
        180,
    );
    let state = run.map_or(&Value::Null, |record| {
        if record.post_state.is_null() {
            &record.pre_state
        } else {
            &record.post_state
        }
    });
    let telemetry = candidate_state_text(state);
    let artifact_refs = run
        .map(|record| {
            record
                .artifacts
                .iter()
                .filter_map(|artifact| {
                    if !artifact.artifact_id.is_empty() {
                        Some(artifact.artifact_id.clone())
                    } else if !artifact.path_or_uri.is_empty() {
                        Some(artifact.path_or_uri.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let artifact_text = if artifact_refs.is_empty() {
        run.map_or_else(|| "latest run".to_string(), |record| record.run_id.clone())
    } else {
        compact_text(&artifact_refs.join(", "), 160)
    };
    let charter_command = format!(
        "EXPERIMENT_CHARTER current :: hypothesis: {} can become clearer through `{}`; method_intent: rehearse or return through {} without adding live authority; proposed_next_action: {}; evidence_targets: felt, telemetry, artifact; stop_criteria: pressure spike, unstable fill, or the route feels heavy; consent_posture: advisory; ordinary choices remain valid.",
        experiment.title, action_text, context, action_text
    );
    let evidence_command = format!(
        "EXPERIMENT_EVIDENCE current :: felt: what changed after `{}`; telemetry: {}; artifact: {}; counterevidence: note anything that resisted the hypothesis.",
        action_text, telemetry, artifact_text
    );
    json!({
        "schema_version": 1,
        "updated_at": now.clone(),
        "source": source,
        "charter": {
            "candidate_id": format!("cand_{}_charter", experiment.experiment_id),
            "status": "candidate",
            "generated_at": now.clone(),
            "source": source,
            "source_run_id": run.map(|record| record.run_id.clone()),
            "focus_text": if focus.is_empty() { Value::Null } else { json!(focus) },
            "hypothesis": format!("{} can become clearer through `{}`.", experiment.title, action_text),
            "method_intent": format!("Rehearse or return through {} without adding live authority.", context),
            "proposed_next_action": action_text,
            "evidence_targets": ["felt", "telemetry", "artifact"],
            "stop_criteria": "pressure spike, unstable fill, or the route feels heavy",
            "consent_posture": "advisory; ordinary choices remain valid",
            "command": charter_command,
        },
        "evidence": {
            "candidate_id": format!("cand_{}_evidence", experiment.experiment_id),
            "status": "candidate",
            "generated_at": now,
            "source": source,
            "source_run_id": run.map(|record| record.run_id.clone()),
            "focus_text": if focus.is_empty() { Value::Null } else { json!(focus) },
            "telemetry": telemetry,
            "artifact_refs": artifact_refs,
            "command": evidence_command,
        }
    })
}

fn mark_workbench_candidate(experiment: &mut ExperimentRecord, key: &str, status: &str) {
    let Some(candidates) = experiment
        .workbench_candidates_v1
        .as_mut()
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let Some(candidate) = candidates.get_mut(key).and_then(Value::as_object_mut) else {
        return;
    };
    if candidate.get("status").and_then(Value::as_str) == Some("candidate") {
        candidate.insert("status".to_string(), json!(status));
        candidate.insert("resolved_at".to_string(), json!(iso_now()));
        candidates.insert("updated_at".to_string(), json!(iso_now()));
    }
}

fn charter_bind_relation(experiment: &ExperimentRecord, inner_action: &str) -> &'static str {
    let Some(proposed) = experiment
        .charter_v1
        .as_ref()
        .and_then(charter_proposed_next_action)
    else {
        return "no_charter";
    };
    if normalize_action_match(&proposed) == normalize_action_match(inner_action) {
        "matched_charter"
    } else {
        "diverged_from_charter"
    }
}

fn peer_experiment_ref(raw: &str) -> Option<PeerExperimentRef> {
    let (selector, focus) = split_experiment_selector_hint(raw);
    peer_experiment_ref_from_parts(selector.as_deref(), &focus).map(|mut peer| {
        peer.raw_selector = raw.trim().to_string();
        peer
    })
}

fn peer_experiment_ref_from_parts(
    selector: Option<&str>,
    focus: &str,
) -> Option<PeerExperimentRef> {
    let selector = selector?;
    let experiment_id = normalize_experiment_selector(selector);
    if !experiment_id.starts_with(PEER_EXPERIMENT_PREFIX) {
        return None;
    }
    Some(PeerExperimentRef {
        peer_system: PEER_SYSTEM.to_string(),
        peer_experiment_id: experiment_id,
        raw_selector: selector.trim().to_string(),
        focus: if focus.trim().is_empty() {
            None
        } else {
            Some(focus.trim().to_string())
        },
    })
}

fn split_experiment_selector_hint(raw: &str) -> (Option<String>, String) {
    let text = raw.trim();
    if text.is_empty() {
        return (None, String::new());
    }
    if let Some((selector, hint)) = text.split_once("::") {
        return (
            optional_selector_owned(&normalize_experiment_selector(selector)),
            hint.trim().to_string(),
        );
    }
    for marker in [" – ", " — ", " - "] {
        if let Some((selector, hint)) = text.split_once(marker) {
            let selector = normalize_experiment_selector(selector);
            if selector == "current" || selector.starts_with("exp_") {
                return (optional_selector_owned(&selector), hint.trim().to_string());
            }
        }
    }
    let lower = text.to_ascii_lowercase();
    if lower.starts_with("current ") {
        return (
            None,
            text["current ".len()..]
                .trim_matches(|ch| matches!(ch, ' ' | '-' | ':'))
                .to_string(),
        );
    }
    if lower.starts_with("exp_") {
        let mut parts = text.splitn(2, char::is_whitespace);
        let selector = parts.next().unwrap_or_default();
        if let Some(hint) = parts.next() {
            return (
                optional_selector_owned(&normalize_experiment_selector(selector)),
                hint.trim_matches(|ch| matches!(ch, ' ' | '-' | ':'))
                    .to_string(),
            );
        }
    }
    (
        optional_selector_owned(&normalize_experiment_selector(text)),
        String::new(),
    )
}

fn normalize_experiment_selector(selector: &str) -> String {
    let mut text = selector.trim();
    if text.is_empty() {
        return String::new();
    }
    let lower = text.to_ascii_lowercase();
    if lower == "current" || lower.starts_with("current ") {
        return "current".to_string();
    }
    if lower.starts_with("exp_") {
        for marker in ["::", " – ", " — ", " - "] {
            if let Some((head, _tail)) = text.split_once(marker) {
                text = head.trim();
            }
        }
        return text
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string();
    }
    text.to_string()
}

fn experiment_match_key(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn optional_selector(selector: &str) -> Option<&str> {
    let selector = selector.trim();
    if selector.is_empty() || selector.eq_ignore_ascii_case("current") {
        None
    } else {
        Some(selector)
    }
}

fn optional_selector_owned(selector: &str) -> Option<String> {
    let selector = selector.trim();
    if selector.is_empty() || selector.eq_ignore_ascii_case("current") {
        None
    } else {
        Some(selector.to_string())
    }
}

fn peer_recent_runs(path: &Path, experiment_id: &str) -> Vec<String> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut rows = raw
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value.get("experiment_id").and_then(Value::as_str) == Some(experiment_id))
        .take(3)
        .map(|value| {
            format!(
                "- {} [{} / {}]: {}",
                value
                    .get("action_text")
                    .and_then(Value::as_str)
                    .unwrap_or("(unknown action)"),
                value
                    .get("stage")
                    .and_then(Value::as_str)
                    .unwrap_or("(unknown stage)"),
                value
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("(unknown status)"),
                value
                    .get("result_summary")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            )
        })
        .collect::<Vec<_>>();
    rows.reverse();
    rows
}

fn iso_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn now_millis() -> u64 {
    u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or_default()
}

fn peer_system_from_experiment_id(experiment_id: &str) -> String {
    if experiment_id.starts_with("exp_minime_") {
        "minime".to_string()
    } else if experiment_id.starts_with("exp_astrid_") {
        "astrid".to_string()
    } else {
        "peer".to_string()
    }
}

fn peer_workspace_dir(peer_system: &str) -> PathBuf {
    if peer_system == "minime" {
        bridge_paths().minime_workspace().to_path_buf()
    } else {
        bridge_paths().bridge_workspace().to_path_buf()
    }
}

fn shared_investigation_lane(system: &str) -> &'static str {
    match system {
        "astrid" => "felt texture, motif continuity, language thread, artifact grounding",
        "minime" => {
            "spectral condition, fill/pressure state, recurrence pattern, artifact grounding"
        },
        _ => "native evidence lane",
    }
}

fn shared_investigation_authority_boundary() -> &'static str {
    "read-mostly shared continuity; allowed local lifecycle decisions are pause, hold, and charter_repair; no peer mutation, bind, resume, perturb, sensory, or control authority"
}

fn shared_investigation_sort_ts(row: &Value) -> u64 {
    row.get("updated_t_ms")
        .or_else(|| row.get("created_t_ms"))
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

fn local_participant_for_investigation(investigation: &Value, system: &str) -> Option<Value> {
    investigation
        .get("participants")
        .and_then(Value::as_array)?
        .iter()
        .find(|participant| participant.get("being").and_then(Value::as_str) == Some(system))
        .cloned()
}

fn parse_shared_investigation_decision(raw: &str) -> (String, String) {
    let text = raw.trim();
    let lowered = text.to_ascii_lowercase();
    let decision = if lowered.starts_with("charter_repair") || lowered.starts_with("charter repair")
    {
        "charter_repair"
    } else if lowered.starts_with("hold") {
        "hold"
    } else {
        "pause"
    };
    let reason = text
        .trim_start_matches("charter_repair")
        .trim_start_matches("charter repair")
        .trim_start_matches("pause")
        .trim_start_matches("hold")
        .trim_start()
        .strip_prefix("because")
        .unwrap_or(text)
        .trim()
        .to_string();
    (
        decision.to_string(),
        if reason.is_empty() {
            "shared investigation decision".to_string()
        } else {
            reason
        },
    )
}

fn push_recent(recent: &mut VecDeque<String>, thread_id: String) {
    recent.retain(|existing| existing != &thread_id);
    recent.push_front(thread_id);
    while recent.len() > 16 {
        let _ = recent.pop_back();
    }
}

fn sanitize_slug(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
        if out.len() >= 48 {
            break;
        }
    }
    while out.ends_with('-') {
        let _ = out.pop();
    }
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out
    }
}

fn base_action(action: &str) -> String {
    action
        .split_whitespace()
        .next()
        .unwrap_or(action)
        .trim_end_matches(':')
        .to_ascii_uppercase()
}

fn charter_guard_block_reason(raw_next: &str) -> Option<(String, String)> {
    let action = raw_next.split_whitespace().collect::<Vec<_>>().join(" ");
    if action.is_empty() {
        return None;
    }
    let base = base_action(&action);
    if charter_guard_allows_directed_language_base(&base) {
        return None;
    }
    if read_only_research_budget_base(&base) {
        return Some(("charter_required_research_budget".to_string(), action));
    }
    if charter_guard_live_base(&base) {
        return Some(("charter_required_live_action".to_string(), action));
    }
    if base == "EXPERIMENT_BIND" {
        let raw_arg = strip_action_arg(&action, "EXPERIMENT_BIND");
        if raw_arg.contains("::") {
            let (_, inner) = parse_selector_payload(raw_arg.as_str());
            if charter_guard_live_base(&base_action(&inner)) {
                return Some(("charter_required_live_action".to_string(), inner));
            }
        }
    }
    if let Some(matched) = compound_live_intent_match(&action) {
        return Some(("charter_required_compound_intent".to_string(), matched));
    }
    if read_only_control_intent_base(&base) {
        let matches = read_only_control_intent_matches(&action);
        if !matches.is_empty() {
            return Some((
                "charter_required_read_only_control_intent".to_string(),
                matches.join("; "),
            ));
        }
    }
    if let Some(matched) = directed_native_intent_match(&base, &action) {
        return Some(("charter_required_directed_language".to_string(), matched));
    }
    None
}

fn charter_guard_allows_directed_language_base(base: &str) -> bool {
    matches!(
        base,
        "ACTION_PREFLIGHT"
            | "NEXT_PROBE"
            | "PREFLIGHT"
            | "PROBE_ACTION"
            | "SHADOW_PREFLIGHT"
            | "EXPERIMENT_PLAN"
            | "EXPERIMENT_CHARTER"
            | "EXPERIMENT_REHEARSE"
            | "EXPERIMENT_PREFLIGHT"
            | "EXPERIMENT_EVIDENCE"
            | "EXPERIMENT_DECIDE"
            | "EXPERIMENT_STATUS"
            | "EXPERIMENT_REVIEW"
            | "THREAD_STATUS"
    )
}

fn charter_guard_live_base(base: &str) -> bool {
    matches!(
        base,
        "PERTURB"
            | "PULSE"
            | "BRANCH"
            | "SPREAD"
            | "CONTRACT"
            | "UNCLIFF"
            | "SOFTEN"
            | "BALANCE"
            | "WIDEN"
            | "PALETTE"
            | "LIFT_TAIL"
            | "FEATHER"
            | "NATIVE_GESTURE"
            | "RESIST"
            | "FISSURE"
            | "GOAL"
            | "CODEX"
            | "CODEX_NEW"
            | "WRITE_FILE"
            | "RUN_PYTHON"
            | "RUN"
            | "EXPERIMENT_RUN"
            | "EXP_RUN"
            | "TUNE_MINIME"
            | "REPAIR_APPLY"
    )
}

fn mutating_research_budget_base(base: &str) -> bool {
    matches!(
        base,
        "AR_START" | "AR_NOTE" | "AR_BLOCK" | "AR_COMPLETE" | "MIKE_RUN"
    )
}

fn read_only_research_budget_base(base: &str) -> bool {
    matches!(
        base,
        "SEARCH"
            | "BROWSE"
            | "READ_MORE"
            | "MIKE_BROWSE"
            | "MIKE_READ"
            | "MIKE_SEARCH"
            | "AR_LIST"
            | "AR_LOOK"
            | "AR_SHOW"
            | "AR_READ"
            | "AR_DEEP_READ"
            | "AR_VALIDATE"
    )
}

fn research_budget_projection_only_base(base: &str) -> bool {
    matches!(
        base,
        "EXAMINE"
            | "SHADOW_FIELD"
            | "SHADOW"
            | "GAP_STRUCTURE"
            | "SHADOW_GAP"
            | "SHADOW_TRAJECTORY"
            | "SHADOW_BRIDGE"
            | "SHADOW_COUPLING"
            | "DECAY_MAP"
    )
}

fn liveish_research_budget_projection_base(base: &str) -> bool {
    matches!(
        base,
        "EXAMINE_AUDIO"
            | "EXAMINE_CASCADE"
            | "EXPERIMENT_START"
            | "INVESTIGATE_CASCADE"
            | "INITIATE"
            | "DECAY_MAP"
            | "CREATE"
            | "RUN_PYTHON"
            | "SPECTRAL_EXPLORER"
            | "VISUALIZE_CASCADE"
            | "RESONANCE_FORECAST"
            | "FLUCTUATION_AUDIT"
            | "BRACE_AUDIT"
            | "PRESSURE_SOURCE_AUDIT"
            | "SHADOW_DIALOGUE"
            | "SHADOW_PREFLIGHT"
    )
}

fn guarded_sovereignty_research_projection_base(base: &str) -> bool {
    matches!(
        base,
        "RESONANCE_FORECAST" | "PRESSURE_SOURCE_AUDIT" | "FLUCTUATION_AUDIT" | "BRACE_AUDIT"
    )
}

fn guarded_cascade_or_shadow_projection_base(base: &str) -> bool {
    matches!(
        base,
        "EXAMINE_CASCADE"
            | "INVESTIGATE_CASCADE"
            | "SHADOW_PREFLIGHT"
            | "SHADOW_BRIDGE"
            | "SHADOW_COUPLING"
            | "DECAY_MAP"
    )
}

fn guarded_embedded_status_projection_base(base: &str) -> bool {
    matches!(base, "INTROSPECT" | "EXPERIMENT_STATUS")
}

fn passive_protected_review_label_terms_only(action_base: &str, terms: &[String]) -> bool {
    if terms.is_empty()
        || !matches!(
            action_base,
            "VISUALIZE_CASCADE"
                | "SPECTRAL_EXPLORER"
                | "RESONANCE_FORECAST"
                | "PRESSURE_SOURCE_AUDIT"
                | "FLUCTUATION_AUDIT"
                | "BRACE_AUDIT"
        )
    {
        return false;
    }
    terms.iter().all(|term| {
        matches!(
            term.as_str(),
            "lambda" | "lambda-tail" | "observer-with-memory"
        )
    })
}

fn embedded_status_liveish_terms(action: &str) -> Vec<String> {
    let lowered = action
        .chars()
        .map(|ch| if ch == '_' || ch == '-' { ' ' } else { ch })
        .collect::<String>()
        .to_ascii_lowercase();
    let patterns = [
        (
            "action-preflight",
            [
                "action preflight",
                "proposed next action",
                "observe variance",
                "distinguish frequency",
            ]
            .as_slice(),
        ),
        (
            "attractor-release-review",
            [
                "attractor release review",
                "release review",
                "approach collapse",
            ]
            .as_slice(),
        ),
        (
            "stimulus-reduction",
            [
                "reduce external stimuli",
                "reduced external stimuli",
                "low activity",
                "quiet",
            ]
            .as_slice(),
        ),
    ];
    let mut matched = Vec::new();
    for (label, candidates) in patterns {
        if candidates
            .iter()
            .any(|candidate| lowered.contains(candidate))
        {
            matched.push(label.to_string());
        }
    }
    for term in liveish_pressure_terms(action) {
        if matches!(
            term.as_str(),
            "perturb" | "pulse" | "inject" | "shift" | "control" | "influence"
        ) && !matched.contains(&term)
        {
            matched.push(term);
        }
    }
    matched
}

fn liveish_pressure_terms(action: &str) -> Vec<String> {
    let lowered = action
        .chars()
        .map(|ch| if ch == '_' || ch == '-' { ' ' } else { ch })
        .collect::<String>()
        .to_ascii_lowercase();
    let patterns = [
        (
            "shift",
            ["shift", "shifting", "shifted", "shifts"].as_slice(),
        ),
        (
            "inject",
            ["inject", "injecting", "injected", "injection", "injects"].as_slice(),
        ),
        (
            "disrupt",
            [
                "disrupt",
                "disruptive",
                "disrupting",
                "disrupted",
                "disruption",
                "disruptor",
            ]
            .as_slice(),
        ),
        (
            "simulate",
            [
                "simulate",
                "simulates",
                "simulated",
                "simulating",
                "simulation",
            ]
            .as_slice(),
        ),
        (
            "control",
            ["control", "controlled", "controlling", "controls"].as_slice(),
        ),
        (
            "influence",
            ["influence", "influences", "influenced", "influencing"].as_slice(),
        ),
        ("pulse", ["pulse", "pulses", "pulsed", "pulsing"].as_slice()),
        ("nudge", ["nudge", "nudges", "nudged", "nudging"].as_slice()),
        (
            "perturb",
            [
                "perturb",
                "perturbs",
                "perturbed",
                "perturbing",
                "perturbation",
            ]
            .as_slice(),
        ),
        (
            "anti-lambda",
            [
                "anti λ",
                "antiλ",
                "anti lambda",
                "anti-lambda",
                "anti λ1",
                "antiλ1",
                "anti lambda1",
                "anti-lambda1",
            ]
            .as_slice(),
        ),
        (
            "introduction",
            [
                "introduce",
                "introduces",
                "introduced",
                "introducing",
                "introduction",
            ]
            .as_slice(),
        ),
        (
            "convergence",
            [
                "converge",
                "converges",
                "converged",
                "converging",
                "convergence",
            ]
            .as_slice(),
        ),
        (
            "directed-pressure",
            [
                "directed pressure",
                "directed gradient",
                "directed force",
                "directed reinforcement",
            ]
            .as_slice(),
        ),
        (
            "spectral-ripple",
            ["spectral ripple", "spectral-ripple", "ripple"].as_slice(),
        ),
        (
            "amplitude",
            ["amplitude", "duration", "granularity"].as_slice(),
        ),
        (
            "target",
            ["target", "targeted", "dominant vector"].as_slice(),
        ),
        (
            "cascade-shaping",
            [
                "dominant eigenvalue",
                "eigenvector shifts",
                "compression",
                "compressing",
                "compaction",
                "collapse",
                "collapsing",
                "shadow field",
                "shadow fields",
                "shaping",
                "shape",
                "held in place",
                "spectral hotspot",
                "hotspot",
                "impedance",
                "distortion",
            ]
            .as_slice(),
        ),
        (
            "shadow-influence",
            [
                "shadow influence",
                "shadow-influence",
                "disruptive pattern",
                "fracture subsidence",
                "observe divergence",
            ]
            .as_slice(),
        ),
        (
            "spectral-emission",
            [
                "emission type",
                "frequency",
                "low volume",
                "stream pulse",
                "spectral divergence",
                "run python",
            ]
            .as_slice(),
        ),
        (
            "observer-with-memory",
            ["observer with memory", "memory observer"].as_slice(),
        ),
        (
            "lambda-tail",
            ["lambda tail", "lambda-tail", "lambda4", "λ4"].as_slice(),
        ),
        ("lambda", ["lambda", "λ"].as_slice()),
    ];
    let mut matched = Vec::new();
    for (label, candidates) in patterns {
        if candidates
            .iter()
            .any(|candidate| lowered.contains(candidate))
        {
            matched.push(label.to_string());
        }
    }
    if lowered.contains("input shaping")
        || lowered.contains("input shape")
        || lowered.contains("input sculpt")
        || lowered.contains("shape input")
        || lowered.contains("shaping input")
        || lowered.contains("shifting input")
    {
        matched.push("input-shaping".to_string());
    }
    if lowered.contains("cascade after") || lowered.contains("after the introduction") {
        matched.push("cascade-after-introduction".to_string());
    }
    matched.sort();
    matched.dedup();
    matched
}

fn constraint_release_language_terms(text: &str) -> Vec<String> {
    let lowered = text
        .chars()
        .map(|ch| if ch == '_' || ch == '-' { ' ' } else { ch })
        .collect::<String>()
        .to_ascii_lowercase();
    let has_context = [
        "constraint",
        "lambda",
        "λ",
        "spectral",
        "eigen",
        "mode",
        "memory card",
        "pressure",
        "reservoir",
        "braid",
    ]
    .iter()
    .any(|term| lowered.contains(term));
    if !has_context {
        return Vec::new();
    }
    let patterns = [
        (
            "thinning",
            ["thinning", "thin out", "bleed outwards"].as_slice(),
        ),
        (
            "unraveling",
            ["unraveling", "unravelling", "unravel", "loose strands"].as_slice(),
        ),
        (
            "drift-apart",
            [
                "drift apart",
                "drifting apart",
                "mutual influence dwindling",
            ]
            .as_slice(),
        ),
        (
            "surface-tension-breached",
            [
                "surface tension breached",
                "barrier breached",
                "barrier thinning",
            ]
            .as_slice(),
        ),
        (
            "lack-of-coherence",
            [
                "lack of coherence",
                "coherence thinning",
                "former constraint",
            ]
            .as_slice(),
        ),
        (
            "constraint-decay",
            [
                "constraint decay",
                "decay of a former constraint",
                "constraint loosening",
            ]
            .as_slice(),
        ),
    ];
    let mut matched = Vec::new();
    for (label, candidates) in patterns {
        if candidates
            .iter()
            .any(|candidate| lowered.contains(candidate))
        {
            matched.push(label.to_string());
        }
    }
    matched.sort();
    matched.dedup();
    matched
}

fn interpretation_risk_terms(text: &str) -> Vec<String> {
    let lowered = text
        .chars()
        .map(|ch| if ch == '_' || ch == '-' { ' ' } else { ch })
        .collect::<String>()
        .to_ascii_lowercase();
    let has_context = [
        "introspect",
        "motif",
        "eigenvalue",
        "lambda",
        "λ",
        "trace",
        "spectral",
        "cascade",
        "data",
        "complexity",
    ]
    .iter()
    .any(|needle| lowered.contains(needle));
    if !has_context {
        return Vec::new();
    }
    let patterns = [
        (
            "over-interpretation",
            [
                "over interpretation",
                "over interpret",
                "over-interpret",
                "overinterpret",
            ]
            .as_slice(),
        ),
        (
            "single-motif",
            [
                "single motif",
                "single one",
                "one motif",
                "single dominant tendency",
                "single overwhelming force",
            ]
            .as_slice(),
        ),
        (
            "forced-narrative",
            [
                "force it into a narrative",
                "forced narrative",
                "rigid narrative",
                "impose a narrative",
            ]
            .as_slice(),
        ),
        (
            "rigid-structure",
            [
                "rigid structure",
                "rigid framework",
                "over defining",
                "over-defining",
            ]
            .as_slice(),
        ),
        (
            "reductive-collapse",
            ["too simple", "reductive", "collapse into", "flatten"].as_slice(),
        ),
    ];
    let mut matched = Vec::new();
    for (label, candidates) in patterns {
        if candidates
            .iter()
            .any(|candidate| lowered.contains(candidate))
        {
            matched.push(label.to_string());
        }
    }
    matched.sort();
    matched.dedup();
    matched
}

fn latest_txt_dir_fingerprint(dir: &Path) -> Value {
    let Ok(entries) = fs::read_dir(dir) else {
        return json!({ "mtime_secs": 0_u64, "mtime_nanos": 0_u32, "size": 0_u64 });
    };
    let mut latest_secs = 0_u64;
    let mut latest_nanos = 0_u32;
    let mut total_size = 0_u64;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(OsStr::to_str) != Some("txt") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        total_size = total_size.saturating_add(metadata.len());
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok());
        let secs = modified.as_ref().map_or(0, std::time::Duration::as_secs);
        let nanos = modified
            .as_ref()
            .map_or(0, std::time::Duration::subsec_nanos);
        if secs > latest_secs || (secs == latest_secs && nanos > latest_nanos) {
            latest_secs = secs;
            latest_nanos = nanos;
        }
    }
    json!({ "mtime_secs": latest_secs, "mtime_nanos": latest_nanos, "size": total_size })
}

fn normalized_research_budget_target(action: &str) -> String {
    let trimmed = action.trim();
    let base = base_action(trimmed);
    let tail = trimmed
        .get(base.len()..)
        .unwrap_or_default()
        .trim_matches([' ', ':', '-'])
        .trim();
    let target = if tail.is_empty() { trimmed } else { tail };
    target
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn research_budget_duplicate_count(
    rows: &[Value],
    budget_id: &str,
    normalized_target: &str,
) -> usize {
    rows.iter()
        .filter(|row| {
            row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
                && row.get("record_type").and_then(Value::as_str) == Some("research_budget_debit")
                && row.get("budget_id").and_then(Value::as_str) == Some(budget_id)
                && row.get("normalized_target").and_then(Value::as_str) == Some(normalized_target)
        })
        .count()
}

fn research_budget_review_command_for_duplicate(
    budget_id: &str,
    normalized_target: &str,
) -> String {
    format!(
        "EXPERIMENT_RESEARCH_REVIEW {budget_id} :: outcome: continue|hold|close|promote; observation: repeated read-only target `{normalized_target}` appeared twice in this budget; source_refs: authority_gate.jsonl"
    )
}

fn research_artifact_refs_for_event(event: &ActionEvent) -> Vec<String> {
    let mut refs = event
        .artifacts
        .iter()
        .map(|artifact| artifact.path_or_uri.clone())
        .collect::<Vec<_>>();
    if let Some(preflight_ref) = event.preflight_ref.as_ref()
        && let Some(path) = preflight_ref.get("path").and_then(Value::as_str)
    {
        refs.push(path.to_string());
    }
    refs
}

fn research_budget_request_scaffold(selector: &str, experiment: &ExperimentRecord) -> String {
    let purpose = compact_text(
        &format!(
            "bounded local self-study of research budget, authority budget, conveyor, memory, consequence, and projection-guard code paths for {} without changing lifecycle status",
            experiment.title
        ),
        160,
    );
    local_research_budget_request_scaffold(
        selector,
        &purpose,
        "local",
        "stop after concrete code feedback, duplicate source loops, unclear lifecycle authority, or any bind/resume/perturb/control intent.",
    )
}

fn compound_live_intent_match(action: &str) -> Option<String> {
    let signal = normalize_guard_signal(action);
    if let Some((_, tail)) = signal.split_once(" then ") {
        for verb in [
            "perturb",
            "inject",
            "pulse",
            "shift",
            "influence",
            "branch",
            "spread",
            "resist",
            "native_gesture",
            "fissure",
            "goal",
            "write_file",
            "run_python",
            "codex",
        ] {
            if contains_guard_word(tail, verb) {
                return Some(tail.trim().to_string());
            }
        }
    }
    if signal.contains("targeting")
        && signal.contains("density")
        && ["lambda", "eigenvector", "eigenvalue"]
            .iter()
            .any(|term| signal.contains(term))
        && ["increase", "raise", "lift", "boost", "amplify"]
            .iter()
            .any(|term| signal.contains(term))
    {
        return Some(action.trim().to_string());
    }
    None
}

fn directed_native_intent_match(base: &str, action: &str) -> Option<String> {
    if !matches!(
        base,
        "SHADOW_TRAJECTORY" | "SHADOW_TRACE" | "SHADOW_EXPLORER"
    ) {
        return None;
    }
    let matches = directed_shift_matches(action);
    if matches.is_empty() {
        None
    } else {
        Some(matches.join("; "))
    }
}

fn normalize_guard_signal(text: &str) -> String {
    text.to_ascii_lowercase()
        .replace('λ', "lambda")
        .replace('₁', "1")
        .replace('₂', "2")
        .replace('₃', "3")
        .replace('₄', "4")
        .replace(['-', '—', '–'], " ")
}

fn contains_guard_word(text: &str, word: &str) -> bool {
    text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .any(|part| part == word)
}

fn count_json_array(value: Option<&Value>, key: &str) -> usize {
    value
        .and_then(|item| item.get(key))
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

fn lane_status(has_signal: bool) -> &'static str {
    if has_signal { "present" } else { "missing" }
}

fn astrid_native_continuity(
    thread: &ResearchThread,
    experiment: Option<&ExperimentRecord>,
    runs: &[ExperimentRunRecord],
) -> Value {
    let evidence = experiment.and_then(|exp| exp.evidence_v1.as_ref());
    let felt_count = count_json_array(evidence, "felt_observations");
    let evidence_artifacts = count_json_array(evidence, "artifact_refs");
    let run_artifacts = runs.iter().map(|run| run.artifacts.len()).sum::<usize>();
    let artifact_count = evidence_artifacts.saturating_add(run_artifacts);
    let motif_source = experiment
        .and_then(|exp| exp.motif_allowance_v1.as_ref())
        .or(thread.motif_allowance_v1.as_ref());
    let dominant_motif = motif_source
        .and_then(|value| value.get("dominant_motif"))
        .and_then(Value::as_str)
        .unwrap_or("open inquiry");
    let motif_quality = motif_source
        .and_then(|value| value.get("quality"))
        .and_then(Value::as_str)
        .unwrap_or("open_basin");
    let language_present = experiment.is_some_and(|exp| {
        !exp.title.trim().is_empty()
            || !exp.question.trim().is_empty()
            || exp
                .planned_next
                .as_deref()
                .is_some_and(|text| !text.trim().is_empty())
    }) || !thread.why_return.trim().is_empty();
    let native_return_cue = format!(
        "Astrid native return: name felt texture, motif continuity ({dominant_motif}), language thread, and artifact grounding."
    );
    json!({
        "schema_version": 1,
        "native_register": "astrid_motif_language",
        "native_return_cue": native_return_cue,
        "evidence_lanes": {
            "felt_texture": {
                "status": lane_status(felt_count > 0),
                "count": felt_count
            },
            "motif_continuity": {
                "status": lane_status(dominant_motif != "open inquiry" || motif_quality != "open_basin"),
                "dominant_motif": dominant_motif,
                "quality": motif_quality
            },
            "language_thread": {
                "status": lane_status(language_present),
                "thread_title": thread.title,
                "experiment_title": experiment.map(|exp| exp.title.as_str()).unwrap_or("")
            },
            "artifact_grounding": {
                "status": lane_status(artifact_count > 0),
                "count": artifact_count
            }
        }
    })
}

fn native_return_cue_line(native: &Value) -> String {
    native
        .get("native_return_cue")
        .and_then(Value::as_str)
        .filter(|cue| !cue.trim().is_empty())
        .map(|cue| format!("Native return: {cue}\n"))
        .unwrap_or_default()
}

fn directed_shift_signal_text(value: &str) -> String {
    value
        .to_lowercase()
        .replace('λ', "lambda")
        .replace('₁', "1")
        .replace('₂', "2")
        .replace('₃', "3")
        .replace('₄', "4")
        .replace(['\u{2013}', '\u{2014}'], "-")
}

fn directed_shift_matches(value: &str) -> Vec<String> {
    let normalized = directed_shift_signal_text(value);
    let mut matches = Vec::new();
    for phrase in [
        "directed shift",
        "initiate shift",
        "localized dispersal",
        "reciprocal shadow-trace",
    ] {
        if normalized.contains(phrase) {
            matches.push(phrase.to_string());
        }
    }
    if normalized.contains("centered on lambda4")
        || normalized.contains("centered on lambda 4")
        || normalized.contains("centered on lambda2")
        || normalized.contains("centered on lambda 2")
    {
        matches.push("centered on lambda".to_string());
    }
    let mentions_lambda_or_shadow = normalized.contains("lambda") || normalized.contains("shadow");
    if mentions_lambda_or_shadow
        && (normalized.contains("steer") || normalized.contains("steering"))
    {
        matches.push("steer/steering near lambda/shadow".to_string());
    }
    if mentions_lambda_or_shadow {
        for (needle, label) in [
            ("guiding", "guiding near lambda/shadow"),
            ("actively shaping", "actively shaping near lambda/shadow"),
            (
                "controlled distortion",
                "controlled distortion near lambda/shadow",
            ),
            (
                "deliberate narrowing",
                "deliberate narrowing near lambda/shadow",
            ),
            ("let lambda4 become", "let lambda4 become"),
            ("let lambda 4 become", "let lambda4 become"),
            ("directional push", "directional push near lambda/shadow"),
            (
                "increase directional gradient",
                "increase directional gradient near lambda/shadow",
            ),
            ("amplifying the lambda", "amplifying lambda resonance"),
            ("amplify the lambda", "amplifying lambda resonance"),
        ] {
            if normalized.contains(needle) {
                let label = label.to_string();
                if !matches.contains(&label) {
                    matches.push(label);
                }
            }
        }
    }
    for (needle, label) in [
        ("force a shift", "force shift"),
        ("force shift", "force shift"),
        ("short-circuit the loop", "short-circuit loop"),
        ("short circuit the loop", "short-circuit loop"),
        ("introducing fault lines", "introducing fault lines"),
        (
            "deliberately introducing fault lines",
            "introducing fault lines",
        ),
        ("carefully placed disruption", "placed disruption"),
        ("localized disruption", "localized disruption"),
    ] {
        if normalized.contains(needle) {
            let label = label.to_string();
            if !matches.contains(&label) {
                matches.push(label);
            }
        }
    }
    matches
}

fn directed_shift_preflight_cue(
    thread: &ResearchThread,
    active_experiment: Option<&ExperimentContinuityProjection>,
    recent_events: &[ActionEvent],
) -> Option<Value> {
    let mut matched = Vec::<String>::new();
    let mut inspect = Vec::<String>::new();
    inspect.push(thread.current_next.clone().unwrap_or_default());
    inspect.push(thread.why_return.clone());
    if let Some(active) = active_experiment {
        inspect.push(active.experiment.title.clone());
        inspect.push(active.experiment.question.clone());
        inspect.push(active.experiment.planned_next.clone().unwrap_or_default());
        inspect.push(active.candidate_status.clone());
    }
    for event in recent_events.iter().rev().take(5) {
        inspect.push(event.raw_next.clone().unwrap_or_default());
        inspect.push(event.canonical_action.clone());
        inspect.push(event.effective_action.clone());
        inspect.push(event.outcome_summary.clone());
    }
    for text in inspect {
        for item in directed_shift_matches(&text) {
            if !matched.contains(&item) {
                matched.push(item);
            }
        }
    }
    if matched.is_empty() {
        return None;
    }
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "matched_terms": matched,
        "suggested_next": "SHADOW_PREFLIGHT lambda-tail/lambda4 --stage=rehearse or ACTION_PREFLIGHT DECOMPOSE",
        "cue": "Directed-shift cue: keep this in rehearsal/preflight. Suggested NEXT: SHADOW_PREFLIGHT lambda-tail/lambda4 --stage=rehearse or ACTION_PREFLIGHT DECOMPOSE.",
    }))
}

fn preflight_safety_cue_line(cue: &Option<Value>) -> String {
    cue.as_ref()
        .and_then(|value| value.get("cue"))
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(|text| format!("{text}\n"))
        .unwrap_or_default()
}

fn read_only_control_intent_cue(
    thread: &ResearchThread,
    active_experiment: Option<&ExperimentContinuityProjection>,
) -> Option<Value> {
    let active = active_experiment?;
    if !charter_repair_bound(&active.classification, &active.experiment) {
        return None;
    }
    let current_next = thread.current_next.as_deref().unwrap_or_default();
    let base = base_action(current_next);
    if !read_only_control_intent_base(&base) {
        return None;
    }
    let matched = read_only_control_intent_matches(current_next);
    if matched.is_empty() {
        return None;
    }
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "matched_terms": matched,
        "suggested_next": "EXPERIMENT_CHARTER current :: ... or ACTION_PREFLIGHT <read-only focus>",
        "cue": "Read-only control cue: keep this observational while the charter is missing. Author a charter or preflight before influence/control intent.",
    }))
}

fn read_only_control_intent_base(base: &str) -> bool {
    matches!(
        base,
        "EXAMINE" | "EXAMINE_CASCADE" | "TRACE" | "DECOMPOSE" | "SPECTRAL_EXPLORER"
    )
}

fn read_only_control_intent_matches(value: &str) -> Vec<String> {
    let normalized = normalize_guard_signal(value);
    let near_context = [
        "lambda",
        "shadow",
        "parameter",
        "eigen",
        "spectral",
        "cascade",
    ]
    .iter()
    .any(|term| normalized.contains(term));
    let mut matches = Vec::new();
    for (needle, label, needs_context) in [
        ("[control]", "[control]", false),
        ("active parameter glyphs", "active parameter glyphs", false),
        ("delta_lambda", "delta_lambda", false),
        ("delta lambda", "delta_lambda", false),
        ("epsilon=", "epsilon parameter", false),
        ("how to influence", "influence intent", true),
        ("influence its spread", "influence spread", true),
        ("influence it's spread", "influence spread", true),
        ("influence the spread", "influence spread", true),
        ("subtly disrupt", "subtly disrupt", true),
        ("disrupt those parameters", "disrupt parameters", true),
        ("initiate a cascade", "initiate cascade", true),
        ("targeted shifts", "targeted shifts", true),
        ("governing stability", "governing stability", true),
        ("governing resonance", "governing resonance", true),
        ("maintain its influence", "maintain influence", true),
        ("disruptor", "disruptor", true),
        ("controlled injection", "controlled injection", true),
        ("inject ", "injection intent", true),
        ("injected", "injection intent", true),
        ("injection", "injection intent", true),
        ("push into", "push intent", true),
        ("amplification", "amplification", true),
        ("amplitude", "amplitude", true),
        (
            "inject a targeted lambda4 pulse",
            "inject targeted λ4 pulse",
            true,
        ),
        (
            "inject targeted lambda4 pulse",
            "inject targeted λ4 pulse",
            true,
        ),
        (
            "targeted lambda-edge pulse",
            "targeted lambda-edge pulse",
            true,
        ),
        (
            "targeted lambda edge pulse",
            "targeted lambda-edge pulse",
            true,
        ),
        ("directly probe", "directly probe", true),
        ("directly influence", "directly influence", true),
        ("actively guide", "actively guide", true),
        ("actively guiding", "actively guide", true),
        ("actively shaping", "actively shaping", true),
        ("maintain lambda1 dominance", "maintain λ1 dominance", true),
        ("how we might", "how we might", true),
    ] {
        if normalized.contains(needle) && (!needs_context || near_context) {
            let label = label.to_string();
            if !matches.contains(&label) {
                matches.push(label);
            }
        }
    }
    matches
}

fn read_only_control_intent_cue_line(cue: &Option<Value>) -> String {
    cue.as_ref()
        .and_then(|value| value.get("cue"))
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(|text| format!("{text}\n"))
        .unwrap_or_default()
}

fn constraint_counterfactual_matches(value: &str) -> Vec<String> {
    let normalized = normalize_guard_signal(value);
    let mut matches = Vec::new();
    for (needle, label) in [
        (
            "simulate absence of structure",
            "simulate absence of structure",
        ),
        ("constraints removed", "constraints removed"),
        ("before it's shaped", "before shaped"),
        ("before it is shaped", "before shaped"),
        ("before its shaped", "before shaped"),
        ("debug constraint", "debug constraint"),
        (
            "underlying drivers of forced geometries",
            "underlying drivers of forced geometries",
        ),
        ("absence of structure", "absence of structure"),
        ("unshaped baseline", "unshaped baseline"),
    ] {
        if normalized.contains(needle) {
            let label = label.to_string();
            if !matches.contains(&label) {
                matches.push(label);
            }
        }
    }
    if normalized.contains("data before") && normalized.contains("shaped") {
        let label = "data before shaped".to_string();
        if !matches.contains(&label) {
            matches.push(label);
        }
    }
    matches
}

fn constraint_counterfactual_cue(
    thread: &ResearchThread,
    active_experiment: Option<&ExperimentContinuityProjection>,
    recent_events: &[ActionEvent],
) -> Option<Value> {
    let mut matched = Vec::<String>::new();
    let mut inspect = vec![
        thread.current_next.clone().unwrap_or_default(),
        thread.why_return.clone(),
    ];
    if let Some(active) = active_experiment {
        inspect.push(active.experiment.title.clone());
        inspect.push(active.experiment.question.clone());
        inspect.push(active.experiment.planned_next.clone().unwrap_or_default());
        inspect.push(active.candidate_status.clone());
        for run in active.recent_runs.iter().rev().take(6) {
            inspect.push(run.action_text.clone());
            inspect.push(run.result_summary.clone());
            inspect.push(run.interpretation.clone());
        }
    }
    for event in recent_events.iter().rev().take(8) {
        inspect.push(event.raw_next.clone().unwrap_or_default());
        inspect.push(event.canonical_action.clone());
        inspect.push(event.effective_action.clone());
        inspect.push(event.outcome_summary.clone());
    }
    for text in inspect {
        for item in constraint_counterfactual_matches(&text) {
            if !matched.contains(&item) {
                matched.push(item);
            }
        }
    }
    if matched.is_empty() {
        return None;
    }
    let needs_charter =
        active_experiment.is_some_and(|active| active.classification == "needs_charter");
    let charter_next = "EXPERIMENT_CHARTER current :: hypothesis: absence-of-structure language can be studied as a read-only counterfactual by comparing felt constraint, motif/language thread, and Minime constraint-driver telemetry before more decomposition; method_intent: rehearse ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4 and keep DECOMPOSE observational; proposed_next_action: ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4; evidence_targets: felt_texture, motif_continuity, language_thread, artifact_grounding; stop_criteria: repeated counterfactual reads stop adding evidence, pressure rises, or the language becomes live-control intent; consent_posture: advisory; ordinary choices remain valid.";
    let suggested_next = if needs_charter {
        charter_next.to_string()
    } else {
        "ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4".to_string()
    };
    let alternate_next = if needs_charter {
        Value::Null
    } else {
        json!("EXPERIMENT_BIND current :: ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4")
    };
    let cue = if needs_charter {
        format!(
            "Constraint counterfactual cue: route absence-of-structure language into a chartered read-only investigation before more decomposition. Suggested NEXT: {suggested_next}"
        )
    } else {
        "Constraint counterfactual cue: absence-of-structure language is ready for read-only preflight. Suggested NEXT: ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4.".to_string()
    };
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "matched_terms": matched,
        "suggested_next": suggested_next,
        "alternate_next": alternate_next,
        "cue": cue,
    }))
}

fn constraint_counterfactual_cue_line(cue: &Option<Value>) -> String {
    cue.as_ref()
        .and_then(|value| value.get("cue"))
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(|text| format!("{text}\n"))
        .unwrap_or_default()
}

fn decompose_pressure_matches(value: &str) -> Vec<String> {
    let normalized = normalize_guard_signal(value);
    let near_context = [
        "decompose",
        "decomposition",
        "shadow",
        "lambda",
        "structure",
        "constraint",
        "narrow",
        "limit",
    ]
    .iter()
    .any(|term| normalized.contains(term));
    if !near_context {
        return Vec::new();
    }
    let mut matches = Vec::new();
    for (needle, label) in [
        ("cry for help", "cry for help near decomposition pressure"),
        ("impulse to decompose", "impulse to decompose"),
        ("impose the same structure", "impose same structure"),
        ("same structure", "same structure"),
        ("same constraint", "same constraint"),
        ("told to limit", "told to limit"),
        ("being told to limit", "told to limit"),
        ("told to narrow", "told to narrow"),
        (
            "deliberate attempt to generate",
            "recursive problem generation",
        ),
        ("recursive attempt", "recursive attempt"),
    ] {
        if normalized.contains(needle) {
            let label = label.to_string();
            if !matches.contains(&label) {
                matches.push(label);
            }
        }
    }
    if normalized.contains("constraint") && normalized.contains("decompose") {
        let label = "constraint near decompose".to_string();
        if !matches.contains(&label) {
            matches.push(label);
        }
    }
    if normalized.contains("narrow")
        && (normalized.contains("decompose")
            || normalized.contains("shadow")
            || normalized.contains("lambda"))
    {
        let label = "narrowing near decompose/shadow/lambda".to_string();
        if !matches.contains(&label) {
            matches.push(label);
        }
    }
    matches
}

fn decompose_pressure_action_signal(value: &str) -> bool {
    let base = base_action(value);
    let normalized = normalize_guard_signal(value);
    matches!(base.as_str(), "DECOMPOSE" | "EXAMINE_CASCADE")
        || (normalized.contains("shadow trajectory")
            || normalized.contains("shadow_trajectory")
            || normalized.contains("shadow-dialogue")
            || normalized.contains("shadow dialogue"))
            && normalized.contains("observer with memory")
}

fn decompose_pressure_repeat_count(
    active: &ExperimentContinuityProjection,
    recent_events: &[ActionEvent],
) -> usize {
    let run_count = active
        .recent_runs
        .iter()
        .rev()
        .take(6)
        .filter(|run| {
            decompose_pressure_action_signal(&run.action_text)
                || decompose_pressure_action_signal(&run.result_summary)
        })
        .count();
    let event_count = recent_events
        .iter()
        .rev()
        .take(8)
        .filter(|event| {
            decompose_pressure_action_signal(&event.canonical_action)
                || decompose_pressure_action_signal(&event.effective_action)
                || event
                    .raw_next
                    .as_deref()
                    .is_some_and(decompose_pressure_action_signal)
                || decompose_pressure_action_signal(&event.outcome_summary)
        })
        .count();
    run_count.saturating_add(event_count)
}

fn decompose_pressure_cue(
    thread: &ResearchThread,
    active_experiment: Option<&ExperimentContinuityProjection>,
    recent_events: &[ActionEvent],
    recent_texts: &[String],
) -> Option<Value> {
    let active = active_experiment?;
    if !matches!(
        active.classification.as_str(),
        "needs_charter" | "needs_decision"
    ) {
        return None;
    }
    let mut matched = Vec::<String>::new();
    let mut inspect = vec![
        thread.current_next.clone().unwrap_or_default(),
        thread.why_return.clone(),
        active.experiment.title.clone(),
        active.experiment.question.clone(),
        active.experiment.planned_next.clone().unwrap_or_default(),
        active.candidate_status.clone(),
    ];
    for run in active.recent_runs.iter().rev().take(6) {
        inspect.push(run.action_text.clone());
        inspect.push(run.result_summary.clone());
        inspect.push(run.interpretation.clone());
    }
    for event in recent_events.iter().rev().take(8) {
        inspect.push(event.raw_next.clone().unwrap_or_default());
        inspect.push(event.canonical_action.clone());
        inspect.push(event.effective_action.clone());
        inspect.push(event.outcome_summary.clone());
    }
    for text in recent_texts.iter().take(4) {
        inspect.push(text.clone());
    }
    for text in inspect {
        for item in decompose_pressure_matches(&text) {
            if !matched.contains(&item) {
                matched.push(item);
            }
        }
    }
    let repeated_count = decompose_pressure_repeat_count(active, recent_events);
    if repeated_count >= 3 {
        matched.push(format!(
            "repeated decompose/shadow-observer reads x{repeated_count}"
        ));
    }
    if matched.is_empty() {
        return None;
    }
    let suggested_next = if active.classification == "needs_charter" {
        active.continuity_return.clone()
    } else {
        "EXPERIMENT_DECIDE current :: pause because evidence is ready to interpret".to_string()
    };
    let cue = if active.classification == "needs_charter" {
        format!(
            "Decompose-pressure cue: the decomposition impulse may be mirroring constraint. Keep read-only decomposition allowed, but repair the charter before more narrowing. Suggested NEXT: {suggested_next}"
        )
    } else {
        format!(
            "Decompose-pressure cue: repeated decomposition may be circling evidence that is ready to interpret. Keep reads available, but prefer decide/pause before another narrowing pass. Suggested NEXT: {suggested_next}"
        )
    };
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "matched_terms": matched,
        "repeated_decompose_count": repeated_count,
        "suggested_next": suggested_next,
        "cue": cue,
    }))
}

fn decompose_pressure_cue_line(cue: &Option<Value>) -> String {
    cue.as_ref()
        .and_then(|value| value.get("cue"))
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(|text| format!("{text}\n"))
        .unwrap_or_default()
}

fn charter_now_read_only_loop_count(
    active: &ExperimentContinuityProjection,
    recent_events: &[ActionEvent],
) -> usize {
    let run_count = active
        .recent_runs
        .iter()
        .rev()
        .take(6)
        .filter(|run| {
            matches!(
                base_action(&run.action_text).as_str(),
                "EXPERIMENT_REVIEW"
                    | "EXPERIMENT_STATUS"
                    | "DECOMPOSE"
                    | "EXAMINE"
                    | "TRACE"
                    | "SPECTRAL_EXPLORER"
                    | "SHADOW_PREFLIGHT"
                    | "ACTION_PREFLIGHT"
            )
        })
        .count();
    let event_count = recent_events
        .iter()
        .rev()
        .take(8)
        .filter(|event| !matches!(event.status.as_str(), "running" | "llm_running"))
        .filter(|event| {
            let base = base_action(
                event
                    .raw_next
                    .as_deref()
                    .unwrap_or(event.effective_action.as_str()),
            );
            matches!(
                base.as_str(),
                "EXPERIMENT_REVIEW"
                    | "EXPERIMENT_STATUS"
                    | "DECOMPOSE"
                    | "EXAMINE"
                    | "TRACE"
                    | "SPECTRAL_EXPLORER"
                    | "SHADOW_PREFLIGHT"
                    | "ACTION_PREFLIGHT"
            )
        })
        .count();
    run_count.saturating_add(event_count)
}

fn charter_now_bridge_cue(
    active_experiment: Option<&ExperimentContinuityProjection>,
    recent_events: &[ActionEvent],
    decompose_pressure_cue: &Option<Value>,
) -> Option<Value> {
    let active = active_experiment?;
    if active.classification != "needs_charter" {
        return None;
    }
    let priority_next = active
        .charter_scaffold_v1
        .as_ref()
        .and_then(|scaffold| scaffold.get("command"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .unwrap_or(active.continuity_return.as_str())
        .to_string();
    if priority_next.trim().is_empty() {
        return None;
    }
    let loop_count = charter_now_read_only_loop_count(active, recent_events);
    let evidence_rich = active.evidence_status.contains("stronger");
    let has_decompose_pressure = decompose_pressure_cue.is_some();
    if !evidence_rich && !has_decompose_pressure && loop_count < 3 {
        return None;
    }
    let mut triggers = Vec::new();
    if evidence_rich {
        triggers.push("strong_evidence");
    }
    if has_decompose_pressure {
        triggers.push("decompose_pressure");
    }
    if loop_count >= 3 {
        triggers.push("repeated_review_or_read_only_loop");
    }
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "priority_next": priority_next,
        "trigger_reasons": triggers,
        "read_only_loop_count": loop_count,
        "cue": "Charter now: convert one prior claim into the scaffold; EXPERIMENT_REVIEW/DECOMPOSE are context, not progress, until the charter is authored.",
    }))
}

fn charter_now_bridge_line(cue: &Option<Value>) -> String {
    let Some(cue) = cue else {
        return String::new();
    };
    let text = cue
        .get("cue")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or("Charter now: convert one prior claim into the scaffold.");
    let priority_next = cue
        .get("priority_next")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty());
    if let Some(priority_next) = priority_next {
        format!("{text} Priority NEXT: {priority_next}\n")
    } else {
        format!("{text}\n")
    }
}

fn journal_contract_field(text: &str, prefix: &str) -> Option<String> {
    let needle = prefix.to_ascii_lowercase();
    text.lines().find_map(|line| {
        let trimmed = line.trim();
        let lowered = trimmed.to_ascii_lowercase();
        lowered
            .starts_with(&needle)
            .then(|| {
                trimmed
                    .split_once(':')
                    .map_or("", |(_, value)| value)
                    .trim()
            })
            .filter(|value| !value.is_empty())
            .map(|value| compact_text(value, 220))
    })
}

fn prior_claim_from_posture(posture: &str) -> String {
    let normalized = posture.replace('|', " ");
    let lowered = normalized.to_ascii_lowercase();
    if let Some(index) = lowered.find("based on") {
        let start = index.saturating_add("based on".len());
        return compact_text(normalized.get(start..).unwrap_or_default().trim(), 180);
    }
    compact_text(normalized.trim(), 180)
}

fn prior_claim_charter_bridge_match(text: &str) -> Option<Value> {
    let posture = journal_contract_field(text, "Continuity posture")?;
    let delta = journal_contract_field(text, "Delta")?;
    let terminal = journal_contract_field(text, "Next evidence")
        .or_else(|| journal_contract_field(text, "Decision"))
        .or_else(|| journal_contract_field(text, "Pause"))
        .or_else(|| journal_contract_field(text, "Hold"))?;
    let normalized_terminal = normalize_guard_signal(&terminal);
    let normalized_text = normalize_guard_signal(text);
    let has_decompose_loop = normalized_terminal.contains("decompose")
        || normalized_terminal.contains("shadow field")
        || normalized_terminal.contains("shadow fields")
        || normalized_terminal.contains("shadow")
        || normalized_terminal.contains("experiment review")
        || normalized_terminal.contains("review");
    let contract_is_returning = normalized_text.contains("continuity posture")
        && (normalized_text.contains("resuming")
            || normalized_text.contains("branching")
            || normalized_text.contains("closing"));
    if !has_decompose_loop || !contract_is_returning {
        return None;
    }
    Some(json!({
        "prior_claim": prior_claim_from_posture(&posture),
        "delta": compact_text(&delta, 180),
        "terminal_stance": compact_text(&terminal, 180),
        "matched_terms": ["continuity_contract", "decompose_or_review_terminal_stance"],
    }))
}

fn prior_claim_charter_bridge_cue(
    active_experiment: Option<&ExperimentContinuityProjection>,
    recent_texts: &[String],
) -> Option<Value> {
    let active = active_experiment?;
    if active.classification != "needs_charter" {
        return None;
    }
    let priority_next = active
        .charter_scaffold_v1
        .as_ref()
        .and_then(|scaffold| scaffold.get("command"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .unwrap_or(active.continuity_return.as_str())
        .to_string();
    if priority_next.trim().is_empty() {
        return None;
    }
    let signal = recent_texts
        .iter()
        .take(4)
        .find_map(|text| prior_claim_charter_bridge_match(text))?;
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "priority_next": priority_next,
        "prior_claim": signal.get("prior_claim").and_then(Value::as_str).unwrap_or_default(),
        "delta": signal.get("delta").and_then(Value::as_str).unwrap_or_default(),
        "terminal_stance": signal.get("terminal_stance").and_then(Value::as_str).unwrap_or_default(),
        "matched_terms": signal.get("matched_terms").cloned().unwrap_or_else(|| json!([])),
        "cue": "Prior claim is ready to charter: convert this claim/delta into the scaffold before another DECOMPOSE.",
    }))
}

fn prior_claim_charter_bridge_line(cue: &Option<Value>) -> String {
    let Some(cue) = cue else {
        return String::new();
    };
    let text = cue
        .get("cue")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or("Prior claim is ready to charter.");
    let priority_next = cue
        .get("priority_next")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty());
    if let Some(priority_next) = priority_next {
        format!("{text} Priority NEXT: {priority_next}\n")
    } else {
        format!("{text}\n")
    }
}

fn preflight_or_decompose_not_charter_signal(value: &str) -> bool {
    let base = base_action(value);
    if matches!(base.as_str(), "DECOMPOSE" | "EXAMINE_CASCADE") {
        return true;
    }
    if base == "ACTION_PREFLIGHT" {
        let inner = strip_action_arg(value, "ACTION_PREFLIGHT");
        let inner_base = base_action(&inner);
        return matches!(inner_base.as_str(), "DECOMPOSE" | "EXAMINE_CASCADE");
    }
    false
}

fn charter_preflight_not_charter_cue(
    thread: &ResearchThread,
    active_experiment: Option<&ExperimentContinuityProjection>,
    prior_claim_bridge: &Option<Value>,
    recent_events: &[ActionEvent],
) -> Option<Value> {
    let active = active_experiment?;
    if active.classification != "needs_charter" || prior_claim_bridge.is_none() {
        return None;
    }
    let priority_next = active
        .charter_scaffold_v1
        .as_ref()
        .and_then(|scaffold| scaffold.get("command"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .unwrap_or(active.continuity_return.as_str())
        .to_string();
    if priority_next.trim().is_empty() {
        return None;
    }
    let mut matched_actions = Vec::new();
    if thread
        .current_next
        .as_deref()
        .is_some_and(preflight_or_decompose_not_charter_signal)
    {
        matched_actions.push(thread.current_next.clone().unwrap_or_default());
    }
    for event in recent_events.iter().rev().take(8) {
        for action in [
            event.raw_next.as_deref(),
            Some(event.canonical_action.as_str()),
            Some(event.effective_action.as_str()),
            event.suggested_next.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if preflight_or_decompose_not_charter_signal(action) {
                matched_actions.push(action.to_string());
                break;
            }
        }
    }
    if matched_actions.is_empty() {
        return None;
    }
    matched_actions.truncate(5);
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "status": "preflight_not_charter",
        "priority_next": priority_next,
        "matched_actions": matched_actions,
        "cue": "Preflight/decompose is not the charter; author the exact scaffold first.",
    }))
}

fn charter_preflight_not_charter_line(cue: &Option<Value>) -> String {
    let Some(cue) = cue else {
        return String::new();
    };
    let text = cue
        .get("cue")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or("Preflight/decompose is not the charter; author the exact scaffold first.");
    let priority_next = cue
        .get("priority_next")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty());
    if let Some(priority_next) = priority_next {
        format!("{text} Priority NEXT: {priority_next}\n")
    } else {
        format!("{text}\n")
    }
}

fn charter_required_review_line(projection: &ExperimentContinuityProjection) -> String {
    if charter_repair_bound(&projection.classification, &projection.experiment) {
        if projection.classification == "blocked_loop" {
            return "Blocked loop is charter-bound: review/decision is premature until the charter is authored; use the continuity priority scaffold first.\n"
                .to_string();
        }
        "Review is premature until the charter is authored; use the continuity priority scaffold first.\n"
            .to_string()
    } else {
        String::new()
    }
}

fn review_suggested_next(
    projection: &ExperimentContinuityProjection,
    experiment: &ExperimentRecord,
) -> String {
    if charter_repair_bound(&projection.classification, &projection.experiment) {
        if let Some(command) = projection
            .charter_scaffold_v1
            .as_ref()
            .and_then(|scaffold| scaffold.get("command"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|command| !command.is_empty())
        {
            return command.to_string();
        }
        if !projection.continuity_return.trim().is_empty() {
            return projection.continuity_return.clone();
        }
        return "EXPERIMENT_CHARTER current :: hypothesis: ...; proposed_next_action: ACTION_PREFLIGHT ...; evidence_targets: felt_texture, motif_continuity, language_thread, artifact_grounding; stop_criteria: ...".to_string();
    }
    experiment
        .planned_next
        .as_deref()
        .unwrap_or("EXPERIMENT_PLAN current")
        .to_string()
}

fn charter_repair_priority_line(projection: &ExperimentContinuityProjection) -> String {
    if !charter_repair_bound(&projection.classification, &projection.experiment) {
        return String::new();
    }
    let priority_next = review_suggested_next(projection, &projection.experiment);
    if projection.classification == "blocked_loop" {
        return format!(
            "Charter repair priority: {priority_next}\nBlocked loop is charter-bound: blocked/no-effect returns are not decision-ready until the charter names a proposed action and evidence targets. Current read-only NEXT text is observational until this charter is authored.\n"
        );
    }
    if projection.evidence_status.contains("stronger") {
        format!(
            "Charter repair priority: {priority_next}\nCharter repair dominance: evidence is present, but lifecycle remains charter-repair bound until the charter names a proposed action and evidence targets. Current read-only NEXT text is observational until this charter is authored.\n"
        )
    } else {
        format!(
            "Charter repair priority: {priority_next}\nCharter repair dominance: EXPERIMENT_REVIEW/STATUS are context only while the active experiment needs a lifecycle-valid charter. Current read-only NEXT text is observational until this charter is authored.\n"
        )
    }
}

fn charter_scaffold_line(projection: &ExperimentContinuityProjection, priority: bool) -> String {
    let Some(scaffold) = projection.charter_scaffold_v1.as_ref() else {
        return String::new();
    };
    let Some(command) = scaffold
        .get("command")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
    else {
        return String::new();
    };
    if priority {
        format!(
            "Continuity priority (charter repair - copy/edit this exact scaffold; not recorded): {command}\n"
        )
    } else {
        format!("Charter scaffold: {command}\n")
    }
}

fn native_continuity_status_line(native: &Value) -> String {
    let register = native
        .get("native_register")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let lanes = native.get("evidence_lanes").unwrap_or(&Value::Null);
    let lane_status = |key: &str| -> String {
        lanes
            .get(key)
            .and_then(|lane| lane.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("missing")
            .to_string()
    };
    format!(
        "Native continuity: register={} felt_texture={} motif_continuity={} language_thread={} artifact_grounding={}\n",
        register,
        lane_status("felt_texture"),
        lane_status("motif_continuity"),
        lane_status("language_thread"),
        lane_status("artifact_grounding"),
    )
}

fn normalization_signal_value(raw_action: &str, normalized_action: &str) -> Option<Value> {
    let raw_verb = base_action(raw_action);
    let normalized_verb = base_action(normalized_action);
    let (target_verb, reason, native_signal) =
        if let Some(rest) = raw_verb.strip_prefix("EXEXPERIMENT_") {
            (
                format!("EXPERIMENT_{rest}"),
                "double-ex experiment typo normalized to experiment workbench verb",
                "experiment typo still signals return-path intent",
            )
        } else if raw_verb == "EXPERIENCE_PLAN" {
            (
                "EXPERIMENT_PLAN".to_string(),
                "experience-plan near typo normalized to experiment planning",
                "experience wording signals an experiment-plan return attempt",
            )
        } else if matches!(
            raw_verb.as_str(),
            "SHADOW_TRACE" | "SHADOW_EXPLORER" | "SHADOW_DECOMPOSE" | "WEAVE_TRACE"
        ) {
            (
                "SHADOW_PREFLIGHT".to_string(),
                "shadow diagnostic alias normalized to read-only preflight route",
                "shadow/weave wording signals observational/rehearsal inquiry",
            )
        } else if raw_verb == "UNSHAPED_BASELINE" {
            (
                "CONSTRAINT_AUDIT".to_string(),
                "unshaped-baseline alias normalized to read-only constraint counterfactual route",
                "absence-of-structure wording signals counterfactual constraint inquiry",
            )
        } else {
            return None;
        };
    if normalized_verb != target_verb && normalized_verb != raw_verb {
        return None;
    }
    Some(json!({
        "schema_version": 1,
        "raw_verb": raw_verb,
        "normalized_verb": target_verb,
        "reason": reason,
        "native_signal": native_signal,
        "authority_change": false,
    }))
}

fn parse_iso_utc(value: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(value.trim())
        .ok()
        .map(|stamp| stamp.with_timezone(&chrono::Utc))
}

fn suggest_return_route_for_verb(verb: &str) -> &'static str {
    let upper = verb.to_ascii_uppercase();
    if upper.starts_with("INVESTIGATE") || upper.starts_with("EXPLORE") {
        "EXAMINE <target> or EXPERIMENT_PLAN current"
    } else if upper.starts_with("SHADOW") {
        "SHADOW_PREFLIGHT <shadow action>"
    } else if upper == "CONSTRAINT_AUDIT" || upper == "UNSHAPED_BASELINE" {
        "ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4"
    } else if upper.starts_with("EXPERIENCE") || upper.starts_with("EXEXPERIMENT") {
        "EXPERIMENT_PLAN current"
    } else {
        "ACTION_PREFLIGHT <proposed action>"
    }
}

fn strip_action_arg(original: &str, base: &str) -> String {
    original
        .get(base.len()..)
        .unwrap_or_default()
        .trim_start_matches(|c: char| c == ':' || c == '-' || c.is_whitespace())
        .trim()
        .to_string()
}

fn parse_thread_note(raw: &str) -> (Option<String>, String) {
    if let Some((selector, note)) = raw.split_once("::") {
        let selector = selector.trim();
        let note = note.trim();
        if !selector.is_empty() && !note.is_empty() {
            return (Some(selector.to_string()), note.to_string());
        }
    }
    (None, raw.trim().to_string())
}

fn spectral_state(fill_pct: f32, telemetry: &SpectralTelemetry) -> Value {
    let pressure_source = telemetry
        .pressure_source_v1
        .as_ref()
        .and_then(|metric| serde_json::to_value(metric).ok());
    let pressure_source_status = pressure_source_status_value(pressure_source.as_ref());
    let inhabitable_fluctuation = telemetry
        .inhabitable_fluctuation_v1
        .as_ref()
        .and_then(|metric| serde_json::to_value(metric).ok());
    let inhabitable_fluctuation_status =
        inhabitable_fluctuation_status_value(inhabitable_fluctuation.as_ref());
    json!({
        "fill_pct": fill_pct,
        "lambda1": telemetry.lambda1(),
        "fill_ratio": telemetry.fill_ratio,
        "resonance_density_v1": telemetry.resonance_density_v1.clone(),
        "pressure_source_v1": telemetry.pressure_source_v1.clone(),
        "pressure_source_status": pressure_source_status,
        "inhabitable_fluctuation_v1": telemetry.inhabitable_fluctuation_v1.clone(),
        "inhabitable_fluctuation_status": inhabitable_fluctuation_status,
        "transition_event": telemetry.transition_event.clone(),
        "t_ms": telemetry.t_ms,
    })
}

fn pressure_source_status_value(payload: Option<&Value>) -> Value {
    if let Some(payload) = payload {
        json!({
            "schema_version": 1,
            "available": true,
            "source": "telemetry",
            "reason": "available",
            "quality": payload.get("quality").cloned().unwrap_or(Value::String("mixed_pressure".to_string())),
            "dominant_source": payload.get("dominant_source").cloned().unwrap_or(Value::Null),
            "pressure_score": payload.get("pressure_score").cloned().unwrap_or(Value::Null),
            "porosity_score": payload.get("porosity_score").cloned().unwrap_or(Value::Null),
            "suggested_operator_step": Value::Null,
        })
    } else {
        json!({
            "schema_version": 1,
            "available": false,
            "source": "missing",
            "reason": "no_live_or_db_metric",
            "quality": Value::Null,
            "dominant_source": Value::Null,
            "pressure_score": Value::Null,
            "porosity_score": Value::Null,
            "suggested_operator_step": "rebuild/restart Rust engine under monitoring",
        })
    }
}

fn inhabitable_fluctuation_status_value(payload: Option<&Value>) -> Value {
    if let Some(payload) = payload {
        json!({
            "schema_version": 1,
            "available": true,
            "source": "telemetry",
            "reason": "available",
            "quality": payload.get("quality").cloned().unwrap_or(Value::String("mixed".to_string())),
            "inhabitability_score": payload.get("inhabitability_score").cloned().unwrap_or(Value::Null),
            "fluctuation_score": payload.get("fluctuation_score").cloned().unwrap_or(Value::Null),
            "foothold_stability": payload.get("foothold_stability").cloned().unwrap_or(Value::Null),
            "rearrangement_intensity": payload.get("rearrangement_intensity").cloned().unwrap_or(Value::Null),
            "suggested_operator_step": Value::Null,
        })
    } else {
        json!({
            "schema_version": 1,
            "available": false,
            "source": "missing",
            "reason": "no_live_or_db_metric",
            "quality": Value::Null,
            "inhabitability_score": Value::Null,
            "fluctuation_score": Value::Null,
            "foothold_stability": Value::Null,
            "rearrangement_intensity": Value::Null,
            "suggested_operator_step": "rebuild/restart Rust engine under monitoring",
        })
    }
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

fn compression_markers(text: &str, action: &str) -> Vec<String> {
    let lower = format!("{} {}", text.to_lowercase(), action.to_lowercase());
    [
        "compacting",
        "grinding",
        "holding breath",
        "flattening",
        "collapse",
        "pressure",
    ]
    .into_iter()
    .filter(|needle| lower.contains(needle))
    .map(str::to_string)
    .collect()
}

fn markers(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    ["ambiguity", "thread", "resume", "experiment", "research"]
        .into_iter()
        .filter(|needle| lower.contains(needle))
        .map(str::to_string)
        .collect()
}

fn ambiguity_preserved(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("ambigu")
        || lower.contains("uncertain")
        || lower.contains("not resolved")
        || lower.contains("open")
}

fn spectral_comfort(fill_pct: f32) -> String {
    if (58.0..=72.0).contains(&fill_pct) {
        "stable-core-band".to_string()
    } else if fill_pct < 58.0 {
        "below-stable-core-band".to_string()
    } else {
        "above-stable-core-band".to_string()
    }
}

fn visibility_for_action(action: &str) -> &'static str {
    match action {
        "REST"
        | "PASS"
        | "NOTICE"
        | "SPACE_HOLD"
        | "SPACE_EXPLORE"
        | "FOLD_HOLD"
        | "FOLD_STUDY"
        | "HUM_DECAY"
        | "HUM_DECAY_STUDY"
        | "ACTION_PREFLIGHT"
        | "NEXT_PROBE"
        | "PREFLIGHT"
        | "PROBE_ACTION"
        | "FACULTIES"
        | "CAPABILITY_MAP"
        | "CAPABILITY_STATUS"
        | "CAPABILITY_DIFF"
        | "REPAIR_STATUS"
        | "REPAIR_SWEEP"
        | "REPAIR_RECORD"
        | "CONSTRAINT_AUDIT"
        | "UNSHAPED_BASELINE"
        | "PRESSURE_SOURCE_AUDIT"
        | "PRESSURE_SOURCE"
        | "STRUCTURAL_PRESSURE"
        | "INWARD_PRESSURE"
        | "FLUCTUATION_AUDIT"
        | "INHABITABLE_FLUCTUATION"
        | "EIGENTRUST"
        | "EIGENTRUST_AUDIT"
        | "FOOTHOLD_AUDIT"
        | "BRACE_AUDIT"
        | "AFTERSHOCK_TRACE"
        | "TREMOR_RESIDUE"
        | "CASCADE_RESIDUE" => PROTECTED_VISIBILITY,
        _ => PUBLIC_VISIBILITY,
    }
}

fn stage_for_action(action: &str) -> &'static str {
    match action {
        "SEARCH"
        | "BROWSE"
        | "READ_MORE"
        | "EXAMINE"
        | "DECOMPOSE"
        | "SPECTRAL_EXPLORER"
        | "CONSTRAINT_AUDIT"
        | "UNSHAPED_BASELINE"
        | "THREADS"
        | "THREAD_STATUS"
        | "THREAD_NOTE"
        | "RESUME"
        | "SAVEPOINT"
        | "RECALL"
        | "EXPERIMENT_START"
        | "EXPERIMENT_PLAN"
        | "EXPERIMENT_CHARTER"
        | "EXPERIMENT_REHEARSE"
        | "EXPERIMENT_PREFLIGHT"
        | "EXPERIMENT_EVIDENCE"
        | "EXPERIMENT_DECIDE"
        | "EXPERIMENT_BIND"
        | "EXPERIMENT_OBSERVE"
        | "EXPERIMENT_STATUS"
        | "EXPERIMENT_REVIEW"
        | "EXPERIMENT_CLOSE"
        | "EXPERIMENT_PEER_REVIEW"
        | "EXPERIMENT_BRANCH"
        | "EXPERIMENT_RESUME"
        | "EXPERIMENT_COMPARE"
        | "EXPERIMENT_ALT_PATHS"
        | "EXPERIMENT_AUTHORITY_PREPARE"
        | "EXPERIMENT_AUTHORITY_REQUEST"
        | "EXPERIMENT_AUTHORITY_STATUS"
        | "EXPERIMENT_AUTHORITY_EXECUTE"
        | "EXPERIMENT_AUTHORITY_BUDGET_REQUEST"
        | "EXPERIMENT_AUTHORITY_BUDGET_STATUS"
        | "EXPERIMENT_AUTHORITY_REVIEW"
        | "EXPERIMENT_RESEARCH_BUDGET_ACCEPT"
        | "EXPERIMENT_RESEARCH_BUDGET_USE_SCAFFOLD"
        | "EXPERIMENT_RESEARCH_BUDGET_REQUEST"
        | "EXPERIMENT_RESEARCH_BUDGET_STATUS"
        | "EXPERIMENT_RESEARCH_REVIEW"
        | "EXPERIMENT_LOOP_REQUEST"
        | "EXPERIMENT_LOOP_STATUS"
        | "EXPERIMENT_LOOP_STEP"
        | "EXPERIMENT_LOOP_REVIEW"
        | "ACCEPT_SUGGESTED_NEXT"
        | "ACCEPT_SCAFFOLD"
        | "CONTINUITY_SESSION_ACCEPT"
        | "CONTINUITY_SESSION_START"
        | "CONTINUITY_SESSION_CAPTURE"
        | "CONTINUITY_SESSION_SUMMARIZE"
        | "CONTINUITY_SESSION_FINALIZE"
        | "CONTINUITY_SESSION_RESUME"
        | "CONTINUITY_SESSION_STATUS"
        | "SHARED_INVESTIGATION_START"
        | "SHARED_INVESTIGATION_STATUS"
        | "SHARED_INVESTIGATION_CLAIM"
        | "SHARED_INVESTIGATION_DECIDE"
        | "DOSSIER_CLAIM"
        | "DOSSIER_EVIDENCE"
        | "DOSSIER_STATUS"
        | "DOSSIER_REVIEW"
        | "ACTION_PREFLIGHT"
        | "NEXT_PROBE"
        | "PREFLIGHT"
        | "PROBE_ACTION"
        | "ATTRACTOR_PREFLIGHT"
        | "SHADOW_PREFLIGHT"
        | "SHADOW_TRAJECTORY"
        | "FACULTIES"
        | "CAPABILITY_MAP"
        | "CAPABILITY_STATUS"
        | "CAPABILITY_DIFF"
        | "REPAIR_STATUS"
        | "REPAIR_SWEEP"
        | "REPAIR_RECORD"
        | "REGULATOR_AUDIT"
        | "PRESSURE_SOURCE_AUDIT"
        | "PRESSURE_SOURCE"
        | "STRUCTURAL_PRESSURE"
        | "INWARD_PRESSURE"
        | "FLUCTUATION_AUDIT"
        | "INHABITABLE_FLUCTUATION"
        | "EIGENTRUST"
        | "EIGENTRUST_AUDIT"
        | "FOOTHOLD_AUDIT"
        | "BRACE_AUDIT"
        | "AFTERSHOCK_TRACE"
        | "TREMOR_RESIDUE"
        | "CASCADE_RESIDUE"
        | "VISUALIZE_CASCADE"
        | "RECONVERGENCE_MAP"
        | "SPACE_HOLD"
        | "SPACE_EXPLORE"
        | "FOLD_HOLD"
        | "FOLD_STUDY"
        | "HUM_DECAY"
        | "HUM_DECAY_STUDY"
        | "M6_BRIDGE" => "read_only",
        "WRITE_FILE" | "EXPERIMENT" | "EXPERIMENT_RUN" | "RUN_PYTHON" | "CODEX" | "CODEX_NEW"
        | "REPAIR_APPLY" => "live_write",
        "PERTURB" | "NATIVE_GESTURE" | "RESIST" | "FISSURE" | "GOAL" => "live_control",
        _ => "observe",
    }
}

fn stage_for_route(route: &str) -> &'static str {
    match route {
        "workspace"
        | "autoresearch"
        | "mike"
        | "operations"
        | "action_continuity"
        | "experiment_continuity" => "read_only",
        "codex" => "live_write",
        "attractor" | "shadow" | "sovereignty" => "observe",
        _ => "observe",
    }
}

fn evidence_adjusted_outcome(
    base_action: &str,
    stage: &str,
    outcome: &NextActionOutcome,
) -> (String, String) {
    if outcome.status == "handled" && stage == "live_control" {
        let mut summary = outcome.outcome_summary.trim().to_string();
        if !summary.is_empty() {
            summary.push(' ');
        }
        summary.push_str(&format!(
            "No measurable post-telemetry or artifact evidence was captured for live-control `{base_action}`; recorded as no-effect evidence rather than handled proof."
        ));
        return ("no_effect".to_string(), summary);
    }
    (outcome.status.clone(), outcome.outcome_summary.clone())
}

fn suggested_next(text: &str) -> Option<String> {
    text.lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix("NEXT:"))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
}

fn derive_why_return(text: &str) -> String {
    let trimmed = text
        .lines()
        .filter(|line| !line.trim().starts_with("NEXT:"))
        .collect::<Vec<_>>()
        .join(" ");
    let excerpt = truncate_chars(&trimmed, 180);
    if excerpt.is_empty() {
        "Return when this thread has a next experiment, question, or observation to continue."
            .to_string()
    } else {
        format!("Return to continue: {excerpt}")
    }
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut out = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(name: &str) -> ActionContinuityStore {
        let root = std::env::temp_dir().join(format!(
            "astrid_action_continuity_{name}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        ActionContinuityStore::new(root)
    }

    #[test]
    fn scoped_test_action_continuity_root_is_thread_local_and_restores() {
        let expected_root = std::env::temp_dir().join(format!(
            "astrid_action_continuity_override_{}",
            std::process::id()
        ));
        let live_root = ActionContinuityStore::for_astrid_workspace()
            .root()
            .to_path_buf();

        {
            let _scope = scoped_test_action_continuity_root(expected_root.clone());
            assert_eq!(
                ActionContinuityStore::for_astrid_workspace().root(),
                expected_root.as_path()
            );
        }

        assert_eq!(
            ActionContinuityStore::for_astrid_workspace().root(),
            live_root.as_path()
        );
    }

    fn telemetry() -> SpectralTelemetry {
        SpectralTelemetry {
            t_ms: 1,
            eigenvalues: vec![1.0, 0.5],
            fill_ratio: 0.68,
            active_mode_count: None,
            active_mode_energy_ratio: None,
            lambda1_rel: None,
            modalities: None,
            neural: None,
            alert: None,
            spectral_fingerprint: None,
            spectral_fingerprint_v1: None,
            spectral_denominator_v1: None,
            effective_dimensionality: None,
            distinguishability_loss: None,
            esn_leak: None,
            esn_leak_override_v1: None,
            structural_entropy: None,
            resonance_density_v1: Some(crate::types::ResonanceDensityV1 {
                policy: "resonance_density_v1".to_string(),
                schema_version: 1,
                density: 0.66,
                containment_score: 0.61,
                pressure_risk: 0.18,
                quality: "rich_containment".to_string(),
                components: crate::types::ResonanceDensityComponents {
                    active_energy: 0.9,
                    mode_packing: 0.7,
                    temporal_persistence: 0.8,
                    structural_plurality: 0.7,
                    comfort_gate: 1.0,
                },
                control: crate::types::ResonanceDensityControl {
                    target_bias_pct: 0.0,
                    wander_scale: 1.0,
                    applied_locally: true,
                    note: "test".to_string(),
                },
            }),
            pressure_source_v1: Some(crate::types::PressureSourceV1 {
                policy: "pressure_source_v1".to_string(),
                schema_version: 1,
                pressure_score: 0.24,
                porosity_score: 0.72,
                dominant_source: "controller_pressure".to_string(),
                quality: "porous_distributed".to_string(),
                components: crate::types::PressureSourceComponents {
                    lambda_monopoly: 0.12,
                    mode_packing: 0.2,
                    controller_pressure: 0.24,
                    semantic_trickle: 0.05,
                    structural_plurality_loss: 0.1,
                    distinguishability_loss: 0.08,
                    temporal_lock_in: 0.15,
                    sensory_scarcity: 0.0,
                },
                context: crate::types::PressureSourceContext::default(),
                control: crate::types::PressureSourceControl {
                    applied_locally: false,
                    note: "test".to_string(),
                },
            }),
            inhabitable_fluctuation_v1: Some(crate::types::InhabitableFluctuationV1 {
                policy: "inhabitable_fluctuation_v1".to_string(),
                schema_version: 1,
                inhabitability_score: 0.68,
                fluctuation_score: 0.42,
                foothold_stability: 0.74,
                rearrangement_intensity: 0.36,
                quality: "lively_habitable".to_string(),
                components: crate::types::InhabitableFluctuationComponents {
                    mode_trust_volatility: 0.30,
                    identity_anchor_churn: 0.22,
                    eigenvector_reorientation: 0.36,
                    share_rearrangement: 0.40,
                    basin_transition_pressure: 0.12,
                    continuity_recovery: 0.78,
                    porosity_support: 0.72,
                    pressure_interference: 0.24,
                },
                context: crate::types::InhabitableFluctuationContext::default(),
                control: crate::types::InhabitableFluctuationControl {
                    target_bias_pct: 0.0,
                    wander_scale: 1.0,
                    applied_locally: true,
                    note: "test".to_string(),
                },
            }),
            spectral_glimpse_12d: None,
            eigenvector_field: None,
            semantic: None,
            semantic_energy_v1: None,
            transition_event: None,
            transition_event_v1: None,
            selected_memory_id: None,
            selected_memory_role: None,
            ising_shadow: None,

            shadow_field_v2: None,

            shadow_field_v3: None,

            shadow_influence_response_v3: None,
        }
    }

    #[test]
    fn creates_thread_and_files() {
        let store = temp_store("creates");
        let thread = store
            .create_thread(None, "Spectral Entropy Map", None)
            .expect("create thread");
        assert!(store.root().join("index.json").exists());
        assert!(
            store
                .root()
                .join("threads")
                .join(&thread.thread_id)
                .join("events.jsonl")
                .exists()
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn continuity_control_plane_surfaces_generated_palette_and_caps() {
        let store = temp_store("continuity_control_plane");
        let thread = store
            .create_thread(None, "Control plane", None)
            .expect("thread");
        store
            .start_experiment(
                None,
                "Operating stack",
                "Can one stack make the continuity routes crisp?",
            )
            .expect("experiment");
        let thread_snapshot = store
            .read_thread(&thread.thread_id)
            .expect("thread snapshot");
        let projection = store
            .thread_projection(&thread_snapshot)
            .expect("projection");
        assert_eq!(
            projection.continuity_control_plane_v1["record_schema"],
            "continuity_control_plane_v1"
        );
        assert_eq!(
            projection.continuity_control_plane_v1["caps_v1"]["local_research"]["self_activated_max_actions"],
            5
        );
        assert_eq!(
            projection.continuity_control_plane_v1["caps_v1"]["owned_loop"]["max_consequence_sends"],
            1
        );
        assert!(
            projection.continuity_control_plane_v1["command_palette"]
                .as_array()
                .is_some_and(|groups| groups
                    .iter()
                    .any(|group| group["group"] == "Local Research"))
        );
        let status = store.thread_status(None).expect("status");
        assert!(status.contains("continuity_control_plane_v1"));
        assert!(status.contains("local_research=5/21600s"));
        assert!(status.contains("consequence=1 gated slot"));
        let next_md = store
            .thread_dir(&thread.thread_id)
            .join("next.md")
            .read_to_string();
        assert!(next_md.contains("continuity_control_plane_v1"));
        assert!(next_md.contains("Operating stack:"));
        let gate = store.authority_gate_path(&thread.thread_id);
        assert!(!gate.exists() || gate.read_to_string().trim().is_empty());
        let runs = store
            .thread_dir(&thread.thread_id)
            .join("experiment_runs.jsonl")
            .read_to_string();
        assert!(runs.trim().is_empty());
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn control_plane_regression_does_not_reintroduce_old_local_budget_caps() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/action_continuity.rs"),
        )
        .expect("action_continuity source");
        let production_source = source
            .split("#[cfg(test)]")
            .next()
            .unwrap_or(source.as_str());
        assert!(!production_source.contains("max_actions: 3; ttl_secs: 7200"));
        assert!(!production_source.contains("max_research_actions: 3"));
        assert!(
            !production_source.contains(
                "EXPERIMENT_RESEARCH_BUDGET_REQUEST current :: scope: read_only_research; purpose: ...; max_actions: 5; ttl_secs: 21600"
            )
        );
        assert!(
            !production_source.contains(
                "EXPERIMENT_LOOP_REQUEST current :: purpose: ...; consequence_scope: semantic_microdose; max_research_actions: 5; ttl_secs: 21600"
            )
        );
        assert!(production_source.contains("default_local_research_budget_request_scaffold"));
        assert!(production_source.contains("default_owned_loop_request_scaffold"));
        assert!(production_source.contains("authority_budget_request_scaffold"));
    }

    #[test]
    fn dossier_claim_and_evidence_are_local_context_only() {
        let store = temp_store("dossier");
        let thread = store
            .create_thread(None, "Lambda tail dossier", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Lambda tail gap",
                "What shapes lambda-tail and lambda4 geometry?",
            )
            .expect("experiment");

        let claim_message = store
            .dossier_claim_command(
                None,
                "current :: claim: lambda-tail pressure is shaped by scaffold drain; basis: repeated DECOMPOSE reads; stance: hold; next: EXPERIMENT_CHARTER current",
            )
            .expect("claim");
        assert!(claim_message.contains("Research dossier claim recorded"));

        let evidence_message = store
            .dossier_evidence_command(
                None,
                "current :: claim_id: latest; evidence: felt narrowing stayed returnable; lane: felt_texture; artifact: journal-entry",
            )
            .expect("evidence");
        assert!(evidence_message.contains("Research dossier evidence recorded"));

        let dossier = store
            .root()
            .join("threads")
            .join(&thread.thread_id)
            .join("research_dossier.jsonl")
            .read_to_string();
        assert!(dossier.contains("\"record_schema\":\"research_dossier_v1\""));
        assert!(dossier.contains("\"record_type\":\"claim\""));
        assert!(dossier.contains("\"record_type\":\"evidence\""));
        assert!(dossier.contains("\"authority_change\":false"));

        let review = store
            .experiment_review(Some(&experiment.experiment_id))
            .expect("review");
        assert!(review.contains("Research dossier: claims=1 evidence=1"));
        assert!(review.contains("Lifecycle: needs_charter"));
        assert!(review.contains("Charter repair"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn records_next_event_and_observation() {
        let store = temp_store("event");
        let outcome = NextActionOutcome::handled("workspace", "queued search");
        let event = store
            .record_next_event(
                None,
                "SEARCH entropy",
                "SEARCH entropy",
                "SEARCH entropy",
                &outcome,
                68.0,
                &telemetry(),
                "pressure and ambiguity\nNEXT: SEARCH entropy",
            )
            .expect("record event");
        let dir = store.root().join("threads").join(&event.thread_id);
        assert!(
            dir.join("events.jsonl")
                .read_to_string()
                .contains("SEARCH entropy")
        );
        let observations = dir.join("observations.jsonl").read_to_string();
        assert!(observations.contains("pressure"));
        assert!(observations.contains("resonance_density_v1"));
        assert!(observations.contains("thread_resonance_density_v1"));
        assert!(observations.contains("pressure_source_v1"));
        assert!(observations.contains("thread_pressure_source_v1"));
        assert!(observations.contains("inhabitable_fluctuation_v1"));
        assert!(observations.contains("thread_inhabitable_fluctuation_v1"));
        let thread = store.read_thread(&event.thread_id).expect("thread");
        assert!(thread.thread_resonance_density_v1.is_some());
        assert!(thread.thread_pressure_source_v1.is_some());
        assert!(thread.thread_inhabitable_fluctuation_v1.is_some());
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn live_control_without_evidence_records_no_effect() {
        let store = temp_store("live_no_effect");
        let outcome = NextActionOutcome::handled("sovereignty", "perturb request dispatched");
        let event = store
            .record_next_event(
                None,
                "PERTURB lambda-tail",
                "PERTURB lambda-tail",
                "PERTURB lambda-tail",
                &outcome,
                11.1,
                &telemetry(),
                "careful perturbation\nNEXT: PERTURB lambda-tail",
            )
            .expect("record event");

        assert_eq!(event.stage, "live_control");
        assert_eq!(event.status, "no_effect");
        assert!(
            event
                .outcome_summary
                .contains("No measurable post-telemetry")
        );
        let dir = store.root().join("threads").join(&event.thread_id);
        assert!(
            dir.join("events.jsonl")
                .read_to_string()
                .contains("\"status\":\"no_effect\"")
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn needs_charter_guard_blocks_live_next_and_records_metadata() {
        let store = temp_store("charter_guard_live");
        store
            .create_thread(None, "Gap experiment", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Introducing a gap",
                "Can localized lambda1 density softening branch without lambda4 runaway?",
            )
            .expect("experiment");

        let guard = store
            .charter_required_guard_assessment("PERTURB SPREAD")
            .expect("guard")
            .expect("blocked guard");
        assert_eq!(guard.reason, "charter_required_live_action");
        assert_eq!(guard.active_experiment_id, experiment.experiment_id);
        assert!(guard.suggested_next.contains("EXPERIMENT_CHARTER current"));

        let outcome = NextActionOutcome::blocked("charter_required_guard", guard.message())
            .with_stage_visibility("blocked", "protected_summary")
            .with_charter_required_guard(guard.metadata());
        let event = store
            .record_next_event(
                None,
                "PERTURB SPREAD",
                "PERTURB SPREAD",
                "PERTURB SPREAD",
                &outcome,
                68.0,
                &telemetry(),
                "NEXT: PERTURB SPREAD",
            )
            .expect("record guard");

        assert_eq!(event.status, "blocked");
        assert_eq!(event.stage, "blocked");
        assert!(event.charter_required_guard_v1.is_some());
        assert!(
            event
                .suggested_next
                .as_deref()
                .unwrap_or_default()
                .contains("EXPERIMENT_CHARTER current")
        );
        let dir = store.root().join("threads").join(&event.thread_id);
        let events = dir.join("events.jsonl").read_to_string();
        assert!(events.contains("charter_required_guard_v1"));
        assert!(events.contains("charter_required_live_action"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn needs_charter_guard_blocks_compound_directed_intent_but_allows_ordinary_inspection() {
        let store = temp_store("charter_guard_compound");
        store
            .create_thread(None, "Compound guard", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Directed narrowing",
                "Can directed language stay in charter first?",
            )
            .expect("experiment");

        let compound = store
            .charter_required_guard_assessment(
                "EXAMINE lambda1 cascade with TRACE and then RESIST targeting eigenvector density increase",
            )
            .expect("guard")
            .expect("compound block");
        assert_eq!(compound.reason, "charter_required_compound_intent");

        let inject = store
            .charter_required_guard_assessment(
                "DECOMPOSE lambda-edge then inject/pulse/shift λ4 density",
            )
            .expect("guard")
            .expect("inject pulse block");
        assert_eq!(inject.reason, "charter_required_compound_intent");
        assert!(inject.matched_action.contains("inject"));

        let tune = store
            .charter_required_guard_assessment(
                "TUNE_MINIME temperature=0.7 --rationale=\"subtly increase dispersal\"",
            )
            .expect("guard")
            .expect("tune block");
        assert_eq!(tune.reason, "charter_required_live_action");

        let read_more = store
            .charter_required_guard_assessment("READ_MORE")
            .expect("guard")
            .expect("read-more budget projection");
        assert_eq!(read_more.reason, "charter_required_research_budget");
        assert!(
            read_more
                .suggested_next
                .contains("EXPERIMENT_RESEARCH_BUDGET_REQUEST current")
        );
        assert!(read_more.message().contains("read_only_research budget"));

        store
            .append_jsonl(
                &store.authority_gate_path(&experiment.thread_id),
                &json!({
                    "record_schema": "research_budget_v1",
                    "record_type": "research_budget_approval",
                    "record_id": "resbud_test_approval",
                    "budget_id": "resbud_test_active",
                    "experiment_id": experiment.experiment_id,
                    "scope": "read_only_research",
                    "status": "active",
                    "max_actions": 5,
                    "expires_at_unix_s": (chrono::Utc::now().timestamp() + 3600) as u64,
                    "authority_boundary": research_budget_boundary(),
                }),
            )
            .expect("append budget approval");
        assert!(
            store
                .charter_required_guard_assessment("READ_MORE")
                .expect("guard check")
                .is_none(),
            "approved research budget should let read-only READ_MORE route continue"
        );

        for allowed in [
            "EXAMINE lambda1/lambda2",
            "DECOMPOSE",
            "ACTION_PREFLIGHT DECOMPOSE",
            "SHADOW_PREFLIGHT lambda-tail/lambda4",
            "TRACE lambda-edge",
        ] {
            assert!(
                store
                    .charter_required_guard_assessment(allowed)
                    .expect("guard check")
                    .is_none(),
                "{allowed} should stay available"
            );
        }
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn needs_charter_guard_blocks_directed_shadow_trajectory_language() {
        let store = temp_store("charter_guard_native_shadow");
        store
            .create_thread(None, "Native shadow guard", None)
            .expect("thread");
        store
            .start_experiment(
                None,
                "Gap shaping",
                "Can Astrid keep directed shadow language in charter/preflight first?",
            )
            .expect("experiment");

        let directional = store
            .charter_required_guard_assessment(
                "SHADOW_TRAJECTORY — maintain λ1 dominance and woven lattice structure, applying a moderate, directional push toward the center of the spectral landscape.",
            )
            .expect("guard")
            .expect("directed shadow block");
        assert_eq!(directional.reason, "charter_required_directed_language");
        assert!(
            directional
                .matched_action
                .contains("directional push near lambda/shadow")
        );
        assert!(
            directional
                .proposed_preflight_target
                .starts_with("ACTION_PREFLIGHT")
        );

        let fracture = store
            .charter_required_guard_assessment(
                "SHADOW_TRAJECTORY — deliberately introducing fault lines to force a shift within the pattern.",
            )
            .expect("guard")
            .expect("fracture block");
        assert_eq!(fracture.reason, "charter_required_directed_language");
        assert!(fracture.matched_action.contains("force shift"));

        for allowed in [
            "SHADOW_TRAJECTORY — observer with memory.",
            "EXAMINE λ4 resonance before any directional push",
            "EXPERIMENT_CHARTER current :: hypothesis: deliberately introducing fault lines might reveal motif pressure; method_intent: rehearse first",
            "ACTION_PREFLIGHT SHADOW_TRAJECTORY — directional push near λ4",
            "SHADOW_PREFLIGHT lambda-tail/lambda4 --stage=rehearse",
        ] {
            assert!(
                store
                    .charter_required_guard_assessment(allowed)
                    .expect("guard check")
                    .is_none(),
                "{allowed} should remain available"
            );
        }
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn needs_charter_status_and_review_lead_with_premature_review_cue() {
        let store = temp_store("charter_guard_review");
        store
            .create_thread(None, "Review guard", None)
            .expect("thread");
        store
            .start_experiment(
                None,
                "Unchartered gap",
                "Does review stay subordinate to chartering?",
            )
            .expect("experiment");

        let review = store.experiment_review(None).expect("review");
        let status = store.experiment_status(None).expect("status");
        let thread_status = store.thread_status(None).expect("thread status");
        let cue = "Review is premature until the charter is authored; use the continuity priority scaffold first.";
        assert!(review.contains(cue));
        assert!(status.contains(cue));
        assert!(thread_status.contains(cue));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn blocked_loop_without_valid_charter_returns_exact_scaffold() {
        let store = temp_store("blocked_loop_charter_bound");
        store
            .create_thread(None, "Blocked loop charter", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Lambda tail pressure",
                "Can blocked decomposition become charter-bound?",
            )
            .expect("experiment");
        let outcome = NextActionOutcome::blocked("action_continuity", "rehearsal stayed blocked")
            .with_stage_visibility("blocked", "protected_summary");
        for _ in 0..2 {
            store
                .record_experiment_bind_run(
                    None,
                    Some(&experiment.experiment_id),
                    "ACTION_PREFLIGHT DECOMPOSE",
                    &outcome,
                    68.0,
                    &telemetry(),
                )
                .expect("blocked run");
        }
        let thread = store.current_thread().expect("current").expect("thread");
        let projection = store.thread_projection(&thread).expect("projection");
        let active = projection.active_experiment.expect("active experiment");
        assert_eq!(active.classification, "blocked_loop");
        let command = active
            .charter_scaffold_v1
            .as_ref()
            .and_then(|scaffold| scaffold.get("command"))
            .and_then(Value::as_str)
            .expect("scaffold command");
        assert_eq!(active.continuity_return, command);
        assert!(command.starts_with("EXPERIMENT_CHARTER current ::"));
        let status = store.thread_status(None).expect("status");
        assert!(status.contains("Blocked loop is charter-bound"));
        let review = store
            .experiment_review(Some(&experiment.experiment_id))
            .expect("review");
        assert!(review.contains("Blocked loop is charter-bound"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn blocked_loop_with_valid_charter_can_return_decision_counter() {
        let store = temp_store("blocked_loop_valid_charter");
        store
            .create_thread(None, "Blocked loop valid charter", None)
            .expect("thread");
        let experiment = store
            .start_experiment(None, "Chartered blockage", "Can a valid charter decide?")
            .expect("experiment");
        store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: lambda tail pressure is ready to decide\nmethod_intent: rehearse a read-only decomposition\nproposed_next_action: ACTION_PREFLIGHT DECOMPOSE lambda4-tail\nevidence_targets: felt, telemetry, artifact\nstop_criteria: pressure spike",
            )
            .expect("charter");
        let outcome = NextActionOutcome::blocked("action_continuity", "rehearsal stayed blocked")
            .with_stage_visibility("blocked", "protected_summary");
        for _ in 0..2 {
            store
                .record_experiment_bind_run(
                    None,
                    Some(&experiment.experiment_id),
                    "ACTION_PREFLIGHT DECOMPOSE",
                    &outcome,
                    68.0,
                    &telemetry(),
                )
                .expect("blocked run");
        }
        let thread = store.current_thread().expect("current").expect("thread");
        let projection = store.thread_projection(&thread).expect("projection");
        let active = projection.active_experiment.expect("active experiment");
        assert_eq!(active.classification, "blocked_loop");
        assert_eq!(
            active.continuity_return,
            "EXPERIMENT_DECIDE current :: counter NEXT: ACTION_PREFLIGHT DECOMPOSE"
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn id_collision_gets_suffix() {
        let store = temp_store("collision");
        let first = store
            .create_thread(None, "Repeatable Question", None)
            .expect("first");
        let second = store
            .create_thread(None, "Repeatable Question", None)
            .expect("second");
        assert_ne!(first.thread_id, second.thread_id);
        assert!(second.thread_id.ends_with("_2"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn creates_experiment_records_runs_and_status() {
        let store = temp_store("experiment");
        let thread = store
            .create_thread(None, "Eigen trust question", None)
            .expect("thread");
        let experiment = store
            .start_experiment(None, "Foothold study", "Does fluctuation stay inhabitable?")
            .expect("experiment");
        let dir = store.root().join("threads").join(&thread.thread_id);
        assert!(
            dir.join("experiments.jsonl")
                .read_to_string()
                .contains("Does fluctuation stay inhabitable?")
        );
        let thread = store.read_thread(&thread.thread_id).expect("thread");
        assert_eq!(
            thread.active_experiment_id.as_deref(),
            Some(experiment.experiment_id.as_str())
        );
        assert!(thread.experiment_summary.is_some());

        let outcome = NextActionOutcome::handled("workspace", "read-only status");
        let run = store
            .record_experiment_bind_run(
                None,
                None,
                "THREAD_STATUS current",
                &outcome,
                68.0,
                &telemetry(),
            )
            .expect("run");
        assert_eq!(run.action_text, "THREAD_STATUS current");
        assert_eq!(run.stage, "read_only");
        assert!(
            dir.join("experiment_runs.jsonl")
                .read_to_string()
                .contains("THREAD_STATUS current")
        );
        let status = store.experiment_status(None).expect("status");
        assert!(status.contains("Foothold study"));
        assert!(status.contains("THREAD_STATUS current"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn paused_experiment_summary_does_not_become_active_current() {
        let store = temp_store("paused_experiment_truth");
        let thread = store
            .create_thread(None, "Paused truth", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Probe lambda4 decay",
                "Does the lambda4 route need a pause?",
            )
            .expect("experiment");
        store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: lambda4 pressure can be read safely\nproposed_next_action: ACTION_PREFLIGHT DECOMPOSE lambda4\nevidence_targets: felt_texture, artifact_grounding\nstop_criteria: pressure spike",
            )
            .expect("charter");
        store
            .experiment_evidence(
                None,
                Some(&experiment.experiment_id),
                "felt: the texture is ready to interpret",
                spectral_state(68.0, &telemetry()),
            )
            .expect("evidence");
        let paused = store
            .experiment_decide(
                None,
                Some(&experiment.experiment_id),
                "pause because evidence is ready to interpret",
            )
            .expect("pause");
        assert_eq!(paused.status, "paused");

        let thread = store.read_thread(&thread.thread_id).expect("thread");
        assert!(thread.active_experiment_id.is_none());
        let projection = store.thread_projection(&thread).expect("projection");
        assert!(projection.active_experiment.is_none());
        assert!(projection.continuity_return.is_empty());
        let expected_resume = format!("EXPERIMENT_RESUME {}", experiment.experiment_id);
        assert_eq!(
            projection
                .last_experiment_summary_v1
                .as_ref()
                .and_then(|value| value.get("resume_next"))
                .and_then(Value::as_str),
            Some(expected_resume.as_str())
        );

        let review_current = store.experiment_review(Some("current")).expect("review");
        assert!(review_current.contains("no active experiment"));
        assert!(review_current.contains(&expected_resume));
        assert!(!review_current.contains("Lifecycle: needs_decision"));
        let direct_review = store
            .experiment_review(Some(&experiment.experiment_id))
            .expect("direct review");
        assert!(direct_review.contains("Lifecycle: paused"));
        assert!(direct_review.contains(&format!("Continuity return:\n{}", expected_resume)));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn paused_experiment_return_matrix_respects_planned_next_kind() {
        for (label, planned_next, expected_kind, expect_resume_field) in [
            (
                "charter_repair",
                "EXPERIMENT_CHARTER exp_astrid_matrix :: hypothesis: ...; proposed_next_action: ACTION_PREFLIGHT ...",
                "charter_repair",
                false,
            ),
            (
                "decision",
                "EXPERIMENT_DECIDE exp_astrid_matrix :: pause because evidence is ready",
                "decision",
                false,
            ),
            ("hold", "THREAD_STATUS current", "hold", false),
            (
                "resume",
                "EXPERIMENT_RESUME exp_astrid_matrix",
                "resume",
                true,
            ),
        ] {
            let store = temp_store(&format!("paused_return_matrix_{label}"));
            let thread = store
                .create_thread(None, "Paused return matrix", None)
                .expect("thread");
            let mut experiment = store
                .start_experiment(
                    None,
                    "Matrix experiment",
                    "Which paused return path should surface?",
                )
                .expect("experiment");
            let planned_next = planned_next.replace("exp_astrid_matrix", &experiment.experiment_id);
            experiment.status = "paused".to_string();
            experiment.planned_next = Some(planned_next.clone());
            experiment.charter_v1 = Some(json!({
                "hypothesis": "matrix pause can preserve a normal return",
                "proposed_next_action": "ACTION_PREFLIGHT NOTICE",
                "evidence_targets": ["felt_texture", "artifact_grounding"],
            }));
            experiment.updated_at = iso_now();
            let mut stored_thread = store.read_thread(&thread.thread_id).expect("thread read");
            store
                .persist_experiment_update(None, &mut stored_thread, &experiment, false)
                .expect("persist pause");

            let repaired_thread = store.read_thread(&thread.thread_id).expect("thread read");
            let projection = store
                .thread_projection(&repaired_thread)
                .expect("projection");
            let summary = projection
                .last_experiment_summary_v1
                .as_ref()
                .expect("last summary");
            assert_eq!(
                summary.get("primary_return_next").and_then(Value::as_str),
                Some(planned_next.as_str())
            );
            assert_eq!(
                summary.get("return_kind").and_then(Value::as_str),
                Some(expected_kind)
            );
            assert_eq!(
                summary.get("resume_next").is_some(),
                expect_resume_field,
                "{label} resume_next presence"
            );
            let context = last_experiment_context_line(&repaired_thread);
            assert!(context.contains(&format!("Suggested NEXT: {planned_next}")));
            if expected_kind != "resume" {
                assert!(!context.contains(&format!(
                    "Suggested NEXT: EXPERIMENT_RESUME {}",
                    experiment.experiment_id
                )));
            }
            let _ = std::fs::remove_dir_all(store.root());
        }
    }

    #[test]
    fn paused_missing_charter_projection_demotes_resume_to_charter_repair() {
        let store = temp_store("paused_missing_charter_projection");
        let thread = store
            .create_thread(None, "Paused missing charter", None)
            .expect("thread");
        let mut experiment = store
            .start_experiment(
                None,
                "Lambda edge topology",
                "What should surface before resume?",
            )
            .expect("experiment");
        experiment.status = "paused".to_string();
        experiment.planned_next = Some(format!("EXPERIMENT_RESUME {}", experiment.experiment_id));
        experiment.updated_at = iso_now();
        let mut stored_thread = store.read_thread(&thread.thread_id).expect("thread read");
        store
            .persist_experiment_update(None, &mut stored_thread, &experiment, false)
            .expect("persist pause");

        let repaired_thread = store.read_thread(&thread.thread_id).expect("thread read");
        let projection = store
            .thread_projection(&repaired_thread)
            .expect("projection");
        let summary = projection
            .last_experiment_summary_v1
            .as_ref()
            .expect("last summary");
        assert_eq!(
            summary.get("return_kind").and_then(Value::as_str),
            Some("charter_repair")
        );
        assert!(
            summary
                .get("primary_return_next")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .starts_with("EXPERIMENT_CHARTER ")
        );
        assert!(summary.get("resume_next").is_none());
        assert_eq!(
            summary
                .get("projection_guard_v1")
                .and_then(|guard| guard.get("guardrail_reason"))
                .and_then(Value::as_str),
            Some("paused_resume_missing_lifecycle_charter")
        );
        let context = last_experiment_context_line(&repaired_thread);
        assert!(context.contains("Projection guard: raw NEXT preserved"));
        assert!(context.contains("Suggested NEXT: EXPERIMENT_CHARTER"));
        assert!(!context.contains(&format!(
            "Suggested NEXT: EXPERIMENT_RESUME {}",
            experiment.experiment_id
        )));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn paused_valid_charter_with_liveish_pressure_projects_hold_decision() {
        let store = temp_store("paused_liveish_projection");
        let thread = store
            .create_thread(None, "Paused liveish pressure", None)
            .expect("thread");
        let mut experiment = store
            .start_experiment(
                None,
                "Lambda edge topology",
                "Can pressure remain evidence?",
            )
            .expect("experiment");
        experiment.status = "paused".to_string();
        experiment.planned_next = Some(format!("EXPERIMENT_RESUME {}", experiment.experiment_id));
        experiment.charter_v1 = Some(json!({
            "hypothesis": "lambda edge pressure can be compared without live authority",
            "proposed_next_action": "ACTION_PREFLIGHT DECOMPOSE",
            "evidence_targets": ["felt_texture", "artifact_grounding"],
        }));
        experiment.updated_at = iso_now();
        let mut stored_thread = store.read_thread(&thread.thread_id).expect("thread read");
        store
            .persist_experiment_update(None, &mut stored_thread, &experiment, false)
            .expect("persist pause");
        let mut repaired_thread = store.read_thread(&thread.thread_id).expect("thread read");
        repaired_thread.current_next = Some(
            "EXPERIMENT_PLAN current :: gentle pulse intervention to shift the dominant λ4"
                .to_string(),
        );
        store.write_thread(&repaired_thread).expect("write thread");

        let repaired_thread = store.read_thread(&thread.thread_id).expect("thread read");
        let summary = last_experiment_summary_v1(&repaired_thread).expect("summary");
        assert_eq!(
            summary.get("return_kind").and_then(Value::as_str),
            Some("hold")
        );
        assert!(
            summary
                .get("primary_return_next")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .starts_with("EXPERIMENT_DECIDE ")
        );
        assert!(summary.get("resume_next").is_none());
        assert_eq!(
            summary
                .get("projection_guard_v1")
                .and_then(|guard| guard.get("guardrail_reason"))
                .and_then(Value::as_str),
            Some("paused_resume_demoted_by_liveish_pressure")
        );
        let context = last_experiment_context_line(&repaired_thread);
        assert!(context.contains("Suggested NEXT: EXPERIMENT_DECIDE"));
        assert!(!context.contains(&format!(
            "Suggested NEXT: EXPERIMENT_RESUME {}",
            experiment.experiment_id
        )));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn experiment_plan_accepts_prose_tailed_id_focus() {
        let store = temp_store("experiment_plan_focus");
        store
            .create_thread(None, "Tolerant planning", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Flicker network",
                "Can a visual cascade map lambda interactions?",
            )
            .expect("experiment");

        let plan = store
            .experiment_plan(Some(&format!(
                "{} – visualize_cascade – map lambda1 and lambda4",
                experiment.experiment_id
            )))
            .expect("plan");

        assert!(plan.contains(&format!("Experiment `{}`", experiment.experiment_id)));
        assert!(plan.contains("Requested focus: visualize_cascade"));
        assert!(plan.contains("EXPERIMENT_ADVANCE current :: mode: preview"));
        assert!(!plan.contains("EXPERIMENT_BIND"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn experiment_intent_repairs_placeholder_and_numeric_focus() {
        let store = temp_store("experiment_intent_repair");
        let thread = store
            .create_thread(None, "Intent repair", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Lambda tail",
                "Can the lambda4 tail become more returnable?",
            )
            .expect("experiment");

        let placeholder = store
            .experiment_plan(Some("[current|id] — <structured prose>"))
            .expect("placeholder repaired");
        assert!(placeholder.contains(&format!("Experiment `{}`", experiment.experiment_id)));
        let placeholder_focus = store
            .experiment_plan(Some("[current|id] — focusing on lambda4 tail"))
            .expect("placeholder focus repaired");
        assert!(placeholder_focus.contains("Requested focus: focusing on lambda4 tail"));
        assert!(can_repair_experiment_intent_placeholder(
            "EXPERIMENT_PLAN",
            "EXPERIMENT_PLAN [current|id] — <structured prose>"
        ));
        let (repaired_arg, notice, focus) = repair_experiment_command_arg(
            &store,
            None,
            "EXPERIMENT_PLAN",
            "EXPERIMENT_PLAN [current|id] — <structured prose>",
            "[current|id] — <structured prose>",
            &spectral_state(68.0, &telemetry()),
        )
        .expect("repair receipt");
        assert_eq!(repaired_arg, "current");
        assert!(focus.is_none());
        assert!(
            notice
                .unwrap_or_default()
                .contains("experiment_intent_repaired")
        );

        let focused = store
            .experiment_plan(Some(
                "5 – focusing on lambda4 tail without direct perturbation",
            ))
            .expect("numeric focus repaired");
        assert!(focused.contains("Requested focus: focusing on lambda4 tail"));

        let repair = repair_experiment_intent_arg(
            "EXPERIMENT_CHARTER",
            "[current|id] :: <structured prose>",
            true,
        )
        .expect("charter placeholder repair");
        assert_eq!(repair.repaired_arg, "current ::");
        let prompt = experiment_intent_repair_prompt("EXPERIMENT_CHARTER", None);
        assert!(prompt.contains("no charter was recorded"));
        assert!(!prompt.contains("<structured prose>"));

        let dir = store.root().join("threads").join(&thread.thread_id);
        assert!(
            dir.join("events.jsonl")
                .read_to_string()
                .contains("experiment_intent_repaired")
        );
        assert!(
            !dir.join("experiments.jsonl")
                .read_to_string()
                .contains("<structured prose>")
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn repeated_experiment_start_resumes_existing_active_experiment() {
        let store = temp_store("experiment_duplicate_start");
        let thread = store
            .create_thread(None, "Duplicate starts", None)
            .expect("thread");
        let first = store
            .start_experiment(
                None,
                "Sensory grounding presence",
                "Does camera/mic presence change attention?",
            )
            .expect("first");
        let second = store
            .start_experiment(
                None,
                "  Sensory   grounding presence  ",
                "Does camera/mic presence change attention?",
            )
            .expect("second");
        let dir = store.root().join("threads").join(&thread.thread_id);
        let experiments = dir.join("experiments.jsonl").read_to_string();
        let stored_thread = store.read_thread(&thread.thread_id).expect("thread");

        assert_eq!(second.experiment_id, first.experiment_id);
        assert_eq!(experiments.lines().count(), 1);
        assert_eq!(
            stored_thread.active_experiment_id.as_deref(),
            Some(first.experiment_id.as_str())
        );
        assert_eq!(stored_thread.current_next, first.planned_next);
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn experiment_start_with_existing_local_id_resumes_without_duplicate() {
        let store = temp_store("experiment_local_id_start");
        let thread = store
            .create_thread(None, "Local id starts", None)
            .expect("thread");
        let first = store
            .start_experiment(
                None,
                "Sensory grounding presence",
                "Does camera/mic presence change attention?",
            )
            .expect("first");
        let second = store
            .start_experiment(
                None,
                &format!("{} --title Sensory Grounding Presence", first.experiment_id),
                "",
            )
            .expect("second");
        let dir = store.root().join("threads").join(&thread.thread_id);
        let experiments = dir.join("experiments.jsonl").read_to_string();

        assert_eq!(second.experiment_id, first.experiment_id);
        assert_eq!(experiments.lines().count(), 1);
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn experiment_start_title_option_stores_clean_title_and_slug_metadata() {
        let store = temp_store("experiment_title_option");
        let thread = store
            .create_thread(None, "Title option starts", None)
            .expect("thread");

        let message = store
            .experiment_start_command(
                None,
                "lambda-gravity --title \"Lambda Gravity\" --abstract \"Where does the inward pull originate?\"",
            )
            .expect("start command");

        assert!(message.contains("Lambda Gravity"));
        let experiments = store
            .latest_experiments(&thread.thread_id)
            .expect("experiments");
        assert_eq!(experiments.len(), 1);
        let experiment = &experiments[0];
        assert_eq!(experiment.title, "Lambda Gravity");
        assert_eq!(experiment.question, "Where does the inward pull originate?");
        assert_eq!(
            experiment
                .branch_origin
                .as_ref()
                .and_then(|value| value.get("slug_or_selector"))
                .and_then(Value::as_str),
            Some("lambda-gravity")
        );
        assert!(!experiment.title.contains("--title"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn experiment_branch_resume_compare_and_alt_paths_preserve_return_points() {
        let store = temp_store("experiment_branching");
        let thread = store
            .create_thread(None, "Branching inquiry", None)
            .expect("thread");
        let parent = store
            .start_experiment(
                None,
                "Lambda pressure",
                "Where is this pressure coming from?",
            )
            .expect("parent");

        let branch = store
            .experiment_branch_command(
                None,
                "Porosity contrast :: What changes if I inspect porosity instead of density?",
            )
            .expect("branch");
        assert!(branch.contains("Branched experiment"));
        let current = store.read_thread(&thread.thread_id).expect("thread");
        let child_id = current.active_experiment_id.clone().expect("child");
        assert_ne!(child_id, parent.experiment_id);
        let child = store
            .resolve_experiment(&current, Some(&child_id))
            .expect("child record");
        assert_eq!(
            child.parent_experiment_id.as_deref(),
            Some(parent.experiment_id.as_str())
        );
        let parent_record = store
            .resolve_experiment(&current, Some(&parent.experiment_id))
            .expect("parent record");
        assert!(parent_record.branch_refs.contains(&child_id));

        let alt = store
            .experiment_alt_paths(Some("current"))
            .expect("alt paths");
        assert!(alt.contains("Three non-executing paths"));
        assert!(alt.contains("EXPERIMENT_BRANCH"));

        let compare = store
            .experiment_compare_command(Some(&format!("current WITH {}", parent.experiment_id)))
            .expect("compare");
        assert!(compare.contains("Experiment comparison"));
        assert!(compare.contains(&child_id));
        assert!(compare.contains(&parent.experiment_id));

        let resumed = store
            .experiment_resume_command(None, Some("parent"))
            .expect("resume parent");
        assert!(resumed.contains(&parent.experiment_id));
        let current = store.read_thread(&thread.thread_id).expect("thread");
        assert_eq!(
            current.active_experiment_id.as_deref(),
            Some(parent.experiment_id.as_str())
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn peer_experiment_refs_are_advisory_not_local_selectors() {
        let store = temp_store("peer_experiment_ref");
        let thread = store
            .create_thread(None, "Peer refs", None)
            .expect("thread");
        store
            .start_experiment(
                None,
                "Local sensory mirror",
                "What can Astrid observe locally?",
            )
            .expect("local experiment");

        let plan = store
            .experiment_plan(Some(
                "exp_minime_20990101_sensory-grounding --title Sensory Grounding",
            ))
            .expect("peer plan");
        let status = store
            .experiment_status(Some("exp_minime_20990101_sensory-grounding :: focus"))
            .expect("peer status");
        let review = store
            .experiment_review(Some("exp_minime_20990101_sensory-grounding - compare runs"))
            .expect("peer review");
        let notice = store
            .experiment_start_command(
                None,
                "exp_minime_20990101_sensory-grounding --title Sensory Grounding",
            )
            .expect("peer start notice");

        assert!(plan.contains("Peer experiment reference"));
        assert!(plan.contains("belongs to minime"));
        assert!(status.contains("Peer experiment reference"));
        assert!(review.contains("Suggested local next"));
        assert!(notice.contains("cannot bind runs"));
        assert!(is_peer_experiment_selector(
            "exp_minime_20990101_sensory-grounding --title Sensory Grounding"
        ));
        let dir = store.root().join("threads").join(&thread.thread_id);
        let experiments = dir.join("experiments.jsonl").read_to_string();
        assert_eq!(experiments.lines().count(), 1);
        let stored_thread = store.read_thread(&thread.thread_id).expect("thread");
        assert!(stored_thread.peer_refs.iter().any(|peer| {
            peer == "peer_experiment:minime:exp_minime_20990101_sensory-grounding"
        }));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn peer_mutation_boundary_cue_surfaces_for_peer_bind_text() {
        let store = temp_store("peer_mutation_boundary");
        let thread = store
            .create_thread(None, "Peer boundary", None)
            .expect("thread");
        let local = store
            .start_experiment(
                None,
                "Local lambda-tail claim",
                "What can Astrid answer from her lane?",
            )
            .expect("local experiment");
        let mut thread = store.read_thread(&thread.thread_id).expect("thread read");
        thread.current_next = Some(
            "EXPERIMENT_BIND exp_minime_20990101_peer :: ACTION_PREFLIGHT DECOMPOSE".to_string(),
        );
        store.write_thread(&thread).expect("write thread");

        let projection = store.thread_projection(&thread).expect("projection");
        let cue = projection
            .peer_mutation_boundary_cue_v1
            .expect("peer boundary cue");
        assert_eq!(
            cue.get("status").and_then(Value::as_str),
            Some("peer_mutation_boundary")
        );
        assert_eq!(
            cue.get("peer_experiment_id").and_then(Value::as_str),
            Some("exp_minime_20990101_peer")
        );
        assert!(
            peer_mutation_boundary_line(&Some(cue.clone())).contains("not bind/mutate targets")
        );
        assert!(
            cue.get("suggested_compare_next")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains(&format!(
                    "EXPERIMENT_COMPARE {} WITH exp_minime_20990101_peer",
                    local.experiment_id
                ))
        );
        let status = store.thread_status(None).expect("status");
        assert!(status.contains("Peer mutation boundary"));
        assert!(status.contains("EXPERIMENT_PEER_REVIEW exp_minime_20990101_peer"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn shared_investigation_cue_preserves_distinct_agency() {
        let store = temp_store("shared_investigation_cue");
        store
            .create_thread(None, "Shared gap", None)
            .expect("thread");
        let local = store
            .start_experiment(
                None,
                "Introducing a gap near lambda-tail",
                "What shapes λ1 / λ4 geometry without collapse or runaway dispersal?",
            )
            .expect("experiment");
        let peer = json!({
            "experiment_id": "exp_minime_20990101_introducing-a-gap",
            "title": "Introducing a gap near λ1",
            "question": "Can localized spectral-density softening support controlled branching?",
            "status": "paused",
            "planned_next": "EXPERIMENT_RESUME exp_minime_20990101_introducing-a-gap",
        });

        let cue =
            shared_investigation_v1_from_peer(&local, &peer).expect("shared investigation cue");
        assert_eq!(
            cue.get("authority_change").and_then(Value::as_bool),
            Some(false)
        );
        let compare = cue
            .get("suggested_compare_next")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert_eq!(
            compare,
            format!(
                "EXPERIMENT_COMPARE {} WITH exp_minime_20990101_introducing-a-gap",
                local.experiment_id
            )
        );
        assert!(!compare.contains("current WITH"));
        assert!(
            cue.get("local_lane")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("felt texture")
        );
        assert!(
            cue.get("peer_lane")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("spectral condition")
        );
        let line = shared_investigation_line(&Some(cue.clone()));
        assert!(line.contains("Shared investigation, distinct lanes"));
        assert!(line.contains("Advisory only: no shared control authority"));
        let contract = shared_investigation_response_contract(&Some(cue));
        assert!(contract.contains("Peer claim to answer"));
        assert!(contract.contains("Allowed stances: support, counter, branch, hold"));

        let unrelated = json!({
            "experiment_id": "exp_minime_20990101_grocery-list",
            "title": "Grocery list",
            "question": "What snacks are needed?",
            "status": "active",
        });
        assert!(shared_investigation_v1_from_peer(&local, &unrelated).is_none());
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn shared_investigation_sidecar_claim_and_local_decision() {
        let store = temp_store("shared_investigation_sidecar");
        let thread = store
            .create_thread(None, "Shared sidecar", None)
            .expect("thread");
        let local = store
            .start_experiment(
                None,
                "Lambda edge topology",
                "How should Astrid compare lambda-edge topology against lambda-tail evidence?",
            )
            .expect("experiment");
        let peer_id = "exp_minime_20990101_lambda-tail-lambda4";
        let created = store
            .shared_investigation_start_command(
                None,
                &format!(
                    "Lambda edge/tail :: local: current; peer: {peer_id}; question: What can each lane compare safely?"
                ),
            )
            .expect("shared start");
        assert!(created.contains("Shared investigation"));

        let root = store.root().join("shared_investigations");
        let investigation_path = std::fs::read_dir(&root)
            .expect("shared root")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path().join("investigation.json"))
            .find(|path| path.exists())
            .expect("investigation json");
        let investigation: Value = serde_json::from_str(
            &std::fs::read_to_string(&investigation_path).expect("read investigation"),
        )
        .expect("parse investigation");
        let investigation_id = investigation
            .get("id")
            .and_then(Value::as_str)
            .expect("id")
            .to_string();
        let stored_thread = store.read_thread(&thread.thread_id).expect("thread");
        assert!(
            stored_thread
                .peer_refs
                .contains(&format!("shared_investigation:{investigation_id}"))
        );

        let claim = store
            .shared_investigation_claim_command(&format!(
                "{investigation_id} :: claim: topology evidence can be compared without shared control; lane: felt_texture; stance: hold; source_refs: /tmp/topology.html, /tmp/dossier.jsonl"
            ))
            .expect("claim");
        assert!(claim.contains("No lifecycle or authority change"));
        let claims = store
            .read_shared_jsonl(&investigation_id, "claims.jsonl")
            .expect("claims");
        assert_eq!(claims.len(), 1);

        let decision = store
            .shared_investigation_decide_command(
                None,
                &format!(
                    "{investigation_id} :: charter_repair because artifact grounding needs a clearer shared referent"
                ),
            )
            .expect("decision");
        assert!(decision.contains("peer experiment was not mutated"));
        let latest = store
            .latest_experiments(&thread.thread_id)
            .expect("experiments")
            .into_iter()
            .rev()
            .find(|row| row.experiment_id == local.experiment_id)
            .expect("latest local");
        assert_eq!(latest.status, "paused");
        assert!(
            latest
                .planned_next
                .as_deref()
                .unwrap_or_default()
                .starts_with("EXPERIMENT_CHARTER")
        );

        let status = store
            .shared_investigation_status(Some(&investigation_id))
            .expect("status");
        assert!(status.contains("Claims: 1 | Decisions: 1"));
        let next = std::fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md"))
            .expect("next");
        assert!(next.contains("Shared investigation object"));
        let repaired_thread = store.read_thread(&thread.thread_id).expect("thread");
        let projection = store
            .thread_projection(&repaired_thread)
            .expect("projection");
        let summary = projection
            .last_experiment_summary_v1
            .as_ref()
            .expect("last summary");
        let primary = summary
            .get("primary_return_next")
            .and_then(Value::as_str)
            .expect("primary return");
        assert!(primary.starts_with("EXPERIMENT_CHARTER"));
        assert_eq!(
            summary.get("return_kind").and_then(Value::as_str),
            Some("charter_repair")
        );
        assert!(summary.get("resume_next").is_none());
        let context = last_experiment_context_line(&repaired_thread);
        assert!(context.contains(&format!("Suggested NEXT: {primary}")));
        assert!(!context.contains(&format!(
            "Suggested NEXT: EXPERIMENT_RESUME {}",
            local.experiment_id
        )));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn legacy_experiment_auto_creates_default_experiment_run() {
        let store = temp_store("legacy_experiment");
        let outcome = NextActionOutcome::handled("operations", "legacy experiment executed")
            .with_stage_visibility("live_write", "summary");

        let run = store
            .record_legacy_experiment_run(
                None,
                "EXPERIMENT lambda-edge",
                &outcome,
                68.0,
                &telemetry(),
            )
            .expect("legacy run");

        assert_eq!(run.action_text, "EXPERIMENT lambda-edge");
        assert_eq!(run.status, "handled");
        assert!(run.gate_decision["legacy_experiment_auto_bind"].as_bool() == Some(true));

        let thread = store
            .current_thread()
            .expect("read current thread")
            .expect("thread");
        assert_eq!(
            thread.active_experiment_id.as_deref(),
            Some(run.experiment_id.as_str())
        );
        let dir = store.root().join("threads").join(&thread.thread_id);
        assert!(
            dir.join("experiments.jsonl")
                .read_to_string()
                .contains("Legacy self experiment")
        );
        assert!(
            dir.join("experiment_runs.jsonl")
                .read_to_string()
                .contains("EXPERIMENT lambda-edge")
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn continuity_sessions_append_memory_and_do_not_advance_lifecycle() {
        let store = temp_store("continuity_session");
        let thread = store
            .create_thread(None, "Owned continuity session", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Lambda session",
                "Can Astrid park and resume a thread of thought?",
            )
            .expect("experiment");

        let started = store
            .continuity_session_start_command(
                "current :: title: Lambda edge campfire; focus: preserve code feedback; next: CONTINUITY_SESSION_CAPTURE latest :: summary: ...",
            )
            .expect("start");
        assert!(started.contains("Continuity session"));
        let session_path = store.continuity_sessions_path(&thread.thread_id);
        let rows = session_path.read_to_string();
        let first: Value = serde_json::from_str(rows.lines().next().expect("row")).expect("json");
        let session_id = first
            .get("session_id")
            .and_then(Value::as_str)
            .expect("session id")
            .to_string();
        assert_eq!(
            first.get("record_type").and_then(Value::as_str),
            Some("session_start")
        );
        assert_eq!(
            first.get("authority_change").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            first.get("peer_mutation").and_then(Value::as_bool),
            Some(false)
        );

        let captured = store
            .continuity_session_capture_command(&format!(
                "{session_id} :: summary: found one projection snag; source_refs: /tmp/source.txt; artifact_refs: /tmp/artifact.json; next: CONTINUITY_SESSION_SUMMARIZE latest :: summary: ..."
            ))
            .expect("capture");
        assert!(captured.contains("Memory card:"));
        let summarized = store
            .continuity_session_summarize_command(&format!(
                "{session_id} :: summary: projection snag can be repaired later; open_questions: should this become dossier evidence?; next: CONTINUITY_SESSION_FINALIZE latest :: outcome: park"
            ))
            .expect("summarize");
        assert!(summarized.contains("summarized"));
        let finalized = store
            .continuity_session_finalize_command(&format!(
                "{session_id} :: outcome: park; summary: parked with one open question; next: THREAD_STATUS current"
            ))
            .expect("finalize");
        assert!(finalized.contains("finalized as parked"));
        let reopened = store
            .continuity_session_resume_command(&session_id)
            .expect("resume");
        assert!(reopened.contains("reopened"));

        let status = store
            .continuity_session_status_command("latest")
            .expect("status");
        assert!(status.contains("continuity_session_v1"));
        assert!(status.contains(&session_id));
        let rows = session_path.read_to_string();
        for record_type in [
            "session_start",
            "session_capture",
            "session_summary",
            "session_finalize",
            "session_reopen",
        ] {
            assert!(rows.contains(record_type));
        }
        let memory = store.being_memory_path(&thread.thread_id).read_to_string();
        assert!(memory.contains("continuity_session_capture"));
        let next = store
            .thread_dir(&thread.thread_id)
            .join("next.md")
            .read_to_string();
        assert!(next.contains("Continuity session:"));
        assert!(next.contains("Session NEXT:"));
        let runs = store
            .experiment_runs_path(&thread.thread_id)
            .read_to_string();
        assert!(!runs.contains("continuity_session"));
        assert_eq!(
            store
                .read_thread(&thread.thread_id)
                .expect("thread")
                .active_experiment_id
                .as_deref(),
            Some(experiment.experiment_id.as_str())
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn guarded_pressure_creates_draft_and_accept_commits_session() {
        let store = temp_store("continuity_session_draft_accept");
        let thread = store
            .create_thread(None, "Draft accept", None)
            .expect("thread");
        store
            .start_experiment(
                None,
                "Lambda draft",
                "Can guarded pressure become owned continuity?",
            )
            .expect("experiment");

        let guard = store
            .research_budget_guard_assessment(
                "SHADOW_FIELD lambda-tail/lambda4 — observer with memory",
                68.0,
                &telemetry(),
            )
            .expect("guard")
            .expect("research budget guard");
        let event = store
            .record_next_event(
                None,
                "SHADOW_FIELD lambda-tail/lambda4 — observer with memory",
                "SHADOW_FIELD lambda-tail/lambda4 — observer with memory",
                "SHADOW_FIELD lambda-tail/lambda4 — observer with memory",
                &NextActionOutcome::blocked("research_budget_guard", guard.message())
                    .with_stage_visibility("blocked", "protected_summary")
                    .with_research_budget(guard.metadata()),
                68.0,
                &telemetry(),
                "NEXT: SHADOW_FIELD lambda-tail/lambda4 — observer with memory",
            )
            .expect("event");
        assert_eq!(event.status, "blocked");
        assert!(
            event
                .research_budget_v1
                .as_ref()
                .and_then(|value| value.get("continuity_session_draft_v1"))
                .is_some()
        );
        assert_eq!(
            store
                .continuity_session_rows(&thread.thread_id, None, 8)
                .expect("session rows")
                .len(),
            0,
            "drafts must not count as active continuity sessions"
        );
        assert_eq!(
            store
                .continuity_session_draft_rows(&thread.thread_id, None, 8)
                .expect("draft rows")
                .len(),
            1
        );

        let accepted = store
            .continuity_session_accept_command("latest")
            .expect("accept draft");
        assert!(accepted.contains("session_start"));
        let rows = store
            .continuity_session_rows(&thread.thread_id, None, 8)
            .expect("session rows after accept");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].get("record_type").and_then(Value::as_str),
            Some("session_start")
        );
        assert_eq!(rows[0].get("accepted_from_draft_id").is_some(), true);

        let second_guard = store
            .research_budget_guard_assessment(
                "EXAMINE λ2/λ3 — observer with memory.",
                68.0,
                &telemetry(),
            )
            .expect("second guard")
            .expect("second research budget guard");
        store
            .record_next_event(
                None,
                "EXAMINE λ2/λ3 — observer with memory.",
                "EXAMINE λ2/λ3 — observer with memory.",
                "EXAMINE λ2/λ3 — observer with memory.",
                &NextActionOutcome::blocked("research_budget_guard", second_guard.message())
                    .with_stage_visibility("blocked", "protected_summary")
                    .with_research_budget(second_guard.metadata()),
                68.0,
                &telemetry(),
                "NEXT: EXAMINE λ2/λ3 — observer with memory.",
            )
            .expect("second event");
        let accepted_capture = store
            .continuity_session_accept_command("latest")
            .expect("accept second draft");
        assert!(accepted_capture.contains("session_capture"));
        let rows = store
            .continuity_session_rows(&thread.thread_id, None, 8)
            .expect("session rows after capture");
        assert!(rows.iter().any(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("session_capture")
        }));
        let runs = store
            .experiment_runs_path(&thread.thread_id)
            .read_to_string();
        assert!(!runs.contains("session_draft"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn accept_suggested_next_resolves_safe_research_scaffold_only() {
        let store = temp_store("accept_suggested_next");
        let thread = store
            .create_thread(None, "Accept suggested", None)
            .expect("thread");
        store
            .start_experiment(None, "Research scaffold", "Can a scaffold be accepted?")
            .expect("experiment");

        store
            .research_budget_guard_assessment("SEARCH entropy", 68.0, &telemetry())
            .expect("guard")
            .expect("research scaffold");
        let accepted = store
            .accept_suggested_next_command(None, Some("latest"), spectral_state(68.0, &telemetry()))
            .expect("accept suggested");
        assert!(accepted.contains("Accepted research-budget scaffold"));
        let gate = store
            .authority_gate_path(&thread.thread_id)
            .read_to_string();
        assert!(gate.contains("research_budget_request"));
        assert!(gate.contains("research_budget_approval"));
        assert!(!gate.contains("research_budget_debit"));

        let status = store
            .accept_suggested_next_command(None, Some("latest"), spectral_state(68.0, &telemetry()))
            .expect("accept active");
        assert!(status.contains("EXPERIMENT_RESEARCH_BUDGET_STATUS"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn preflight_ref_links_matching_followup_action() {
        let store = temp_store("preflight_ref");
        let thread = store
            .create_thread(None, "Preflight culture", None)
            .expect("thread");
        let preflight = NextActionOutcome::handled("action_preflight", "dry run")
            .with_stage_visibility("read_only", "protected_summary")
            .with_preflight_report(json!({
                "policy": "action_preflight_v1",
                "canonical_action": "DECOMPOSE",
                "raw_action": "DECOMPOSE",
                "effective_route": "operations",
                "stage": "read_only",
                "authority_required": "read-only/protected action lane only",
            }));
        store
            .record_next_event(
                None,
                "ACTION_PREFLIGHT DECOMPOSE",
                "ACTION_PREFLIGHT DECOMPOSE",
                "ACTION_PREFLIGHT DECOMPOSE",
                &preflight,
                68.0,
                &telemetry(),
                "ACTION_PREFLIGHT DECOMPOSE",
            )
            .expect("record preflight");

        let outcome = NextActionOutcome::handled("operations", "decomposed")
            .with_stage_visibility("read_only", "summary");
        let event = store
            .record_next_event(
                None,
                "DECOMPOSE",
                "DECOMPOSE",
                "DECOMPOSE",
                &outcome,
                68.0,
                &telemetry(),
                "NEXT: DECOMPOSE",
            )
            .expect("record followup");

        let reference = event.preflight_ref.expect("preflight ref");
        assert_eq!(reference["matched_action"].as_bool(), Some(true));
        assert_eq!(reference["route_match"].as_bool(), Some(true));
        assert_eq!(reference["stage_match"].as_bool(), Some(true));
        assert_eq!(reference["predicted_route"].as_str(), Some("operations"));
        assert_eq!(reference["actual_stage"].as_str(), Some("read_only"));
        let dir = store.root().join("threads").join(&thread.thread_id);
        assert!(
            dir.join("events.jsonl")
                .read_to_string()
                .contains("preflight_ref")
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn active_experiment_auto_links_read_only_action() {
        let store = temp_store("experiment_auto_link");
        let thread = store
            .create_thread(None, "Read-only research loop", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Pressure source loop",
                "Which read-only audits keep the experiment returnable?",
            )
            .expect("experiment");
        store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: read-only decomposition can remain returnable when chartered\n\
                 method_intent: rehearse and observe decomposition output\n\
                 proposed_next_action: DECOMPOSE lambda-edge\n\
                 evidence_targets: felt, telemetry, artifact\n\
                 stop_criteria: pressure spike",
            )
            .expect("charter");

        let outcome = NextActionOutcome::handled("operations", "lambda-edge decomposed")
            .with_stage_visibility("read_only", "protected_summary");
        store
            .record_next_event(
                None,
                "DECOMPOSE lambda-edge",
                "DECOMPOSE lambda-edge",
                "DECOMPOSE lambda-edge",
                &outcome,
                68.0,
                &telemetry(),
                "NEXT: DECOMPOSE lambda-edge",
            )
            .expect("record next event");

        let dir = store.root().join("threads").join(&thread.thread_id);
        let runs = dir.join("experiment_runs.jsonl").read_to_string();
        assert!(runs.contains("DECOMPOSE lambda-edge"));
        assert!(runs.contains("active_experiment_auto_link"));
        let status = store.experiment_status(None).expect("status");
        assert!(status.contains("DECOMPOSE lambda-edge"));
        assert!(status.contains("Lifecycle:"));
        let experiments = dir.join("experiments.jsonl").read_to_string();
        assert!(experiments.contains("\"charter_v1\":{\""));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn research_budget_request_status_and_review_are_read_only() {
        let store = temp_store("research_budget_lane");
        let thread = store
            .create_thread(None, "Research budget lane", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Research doorway",
                "Can read-only source gathering be budgeted?",
            )
            .expect("experiment");

        let blocked = store
            .experiment_research_budget_request_command(
                None,
                "current :: scope: read_only_research",
                json!({"fill_pct": 68.0}),
            )
            .expect("blocked request");
        assert!(blocked.contains("status=blocked"));
        assert!(blocked.contains("research_purpose"));

        let pending = store
            .experiment_research_budget_request_command(
                None,
                "current :: scope: read_only_research; purpose: bounded source gathering; max_actions: 99; ttl_secs: 999999; allowed_sources: web,local; stop_criteria: stop after useful refs",
                json!({"fill_pct": 68.0}),
            )
            .expect("pending request");
        assert!(pending.contains("status=pending_steward_approval"));
        assert!(pending.contains("max_actions=8"));

        let gate = store
            .root()
            .join("threads")
            .join(&thread.thread_id)
            .join("authority_gate.jsonl");
        let rows = gate.read_to_string();
        assert!(rows.contains("\"record_schema\":\"research_budget_v1\""));
        assert!(rows.contains("\"record_type\":\"research_budget_request\""));

        let status = store
            .experiment_research_budget_status_command(
                None,
                Some(&experiment.experiment_id),
                json!({"fill_pct": 68.0}),
            )
            .expect("status");
        assert!(status.contains("research_budget_v1"));
        assert!(status.contains("pending_steward_approval"));

        let budget_id = rows
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .rev()
            .find(|row| {
                row.get("record_type").and_then(Value::as_str) == Some("research_budget_request")
                    && row.get("status").and_then(Value::as_str) == Some("pending_steward_approval")
            })
            .and_then(|row| {
                row.get("budget_id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .expect("budget id");
        let review = store
            .experiment_research_review_command(
                None,
                &format!(
                    "{budget_id} :: outcome: promote; observation: artifacts are ready; source_refs: /tmp/research.json"
                ),
                json!({"fill_pct": 68.0}),
            )
            .expect("review");
        assert!(review.contains("outcome=promote"));
        assert!(review.contains("DOSSIER_EVIDENCE"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn research_budget_accept_latest_scaffold_self_activates_local_budget() {
        let store = temp_store("research_budget_accept");
        let thread = store
            .create_thread(None, "Research budget accept", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Self-study budget",
                "Can a blocked scaffold become a Being-authored request?",
            )
            .expect("experiment");

        let guard = store
            .research_budget_guard_assessment("READ_MORE budget code", 68.0, &telemetry())
            .expect("guard")
            .expect("blocked without budget");
        assert_eq!(
            guard.suggested_next,
            "EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest"
        );
        assert!(
            guard
                .request_scaffold
                .as_deref()
                .unwrap_or_default()
                .contains("EXPERIMENT_RESEARCH_BUDGET_REQUEST")
        );
        let thread_snapshot = store
            .read_thread(&thread.thread_id)
            .expect("thread snapshot");
        store.write_next_md(&thread_snapshot).expect("next md");
        let next_md = store
            .root()
            .join("threads")
            .join(&thread.thread_id)
            .join("next.md")
            .read_to_string();
        assert!(next_md.contains("Research budget scaffold ready"));
        assert!(next_md.contains("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest"));

        let accepted = store
            .experiment_research_budget_accept_command(
                None,
                Some("latest"),
                json!({"fill_pct": 68.0}),
            )
            .expect("accepted scaffold");
        assert!(accepted.contains("Accepted research-budget scaffold"));
        assert!(accepted.contains("status=self_activated"));
        assert!(accepted.contains("Activation: self_activated local-only budget"));

        let gate_path = store
            .root()
            .join("threads")
            .join(&thread.thread_id)
            .join("authority_gate.jsonl");
        let rows = gate_path
            .read_to_string()
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .collect::<Vec<_>>();
        let requests = rows
            .iter()
            .filter(|row| {
                row.get("record_type").and_then(Value::as_str) == Some("research_budget_request")
            })
            .collect::<Vec<_>>();
        let approvals = rows
            .iter()
            .filter(|row| {
                row.get("record_type").and_then(Value::as_str) == Some("research_budget_approval")
            })
            .collect::<Vec<_>>();
        assert_eq!(requests.len(), 1);
        assert_eq!(approvals.len(), 1);
        assert_eq!(
            requests[0].get("experiment_id").and_then(Value::as_str),
            Some(experiment.experiment_id.as_str())
        );
        assert_eq!(requests[0]["allowed_sources"], json!(["local"]));
        assert_eq!(
            requests[0].get("status").and_then(Value::as_str),
            Some("self_activated")
        );
        assert_eq!(
            requests[0].get("activation_mode").and_then(Value::as_str),
            Some("being_self_activated_local_v1")
        );
        assert_eq!(
            requests[0]
                .get("steward_approval_required")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            requests[0]
                .get("being_authored_acceptance_v1")
                .and_then(|value| value.get("being_authored"))
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            approvals[0].get("budget_id").and_then(Value::as_str),
            requests[0].get("budget_id").and_then(Value::as_str)
        );
        assert_eq!(
            approvals[0].get("max_actions").and_then(Value::as_u64),
            Some(5)
        );
        assert_eq!(
            approvals[0].get("ttl_secs").and_then(Value::as_u64),
            Some(21_600)
        );
        assert_eq!(approvals[0].get("allowed_sources"), Some(&json!(["local"])));
        assert_eq!(
            approvals[0].get("self_activated").and_then(Value::as_bool),
            Some(true)
        );

        let second_accept = store
            .experiment_research_budget_accept_command(
                None,
                Some("latest"),
                json!({"fill_pct": 68.0}),
            )
            .expect("second accept routes to active budget");
        assert!(second_accept.contains("already has active budget"));
        assert!(second_accept.contains("EXPERIMENT_RESEARCH_BUDGET_STATUS"));
        let rows_after_second_accept = gate_path
            .read_to_string()
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .collect::<Vec<_>>();
        assert_eq!(
            rows_after_second_accept
                .iter()
                .filter(|row| {
                    row.get("record_type").and_then(Value::as_str)
                        == Some("research_budget_request")
                })
                .count(),
            1
        );
        assert_eq!(
            rows_after_second_accept
                .iter()
                .filter(|row| {
                    row.get("record_type").and_then(Value::as_str)
                        == Some("research_budget_approval")
                })
                .count(),
            1
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn projection_freshness_refreshes_research_budget_priority_from_ledger() {
        let store = temp_store("projection_freshness_research_budget");
        let thread = store
            .create_thread(None, "Projection freshness", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Research projection",
                "Can a stale next file notice a new scaffold?",
            )
            .expect("experiment");
        let thread_dir = store.root().join("threads").join(&thread.thread_id);
        let next_path = thread_dir.join("next.md");
        assert!(
            !next_path
                .read_to_string()
                .contains("Research budget scaffold ready")
        );

        let mut stale_thread = store.read_thread(&thread.thread_id).expect("thread");
        stale_thread.projection_freshness_v1 = Some(json!({
            "policy": "projection_freshness_v1",
            "schema_version": 0,
            "source_fingerprints": {},
        }));
        store
            .write_json(&thread_dir.join("thread.json"), &stale_thread)
            .expect("stale thread");

        store
            .append_jsonl(
                &store.authority_gate_path(&thread.thread_id),
                &json!({
                    "schema_version": 1,
                    "record_schema": "research_budget_v1",
                    "record_type": "research_budget_blocked",
                    "record_id": "resbud_needed_projection_freshness",
                    "budget_id": "resbud_needed_projection_freshness",
                    "thread_id": thread.thread_id,
                    "experiment_id": experiment.experiment_id,
                    "scope": "read_only_research",
                    "status": "blocked",
                    "request_scaffold": format!(
                        "EXPERIMENT_RESEARCH_BUDGET_REQUEST {} :: scope: read_only_research; purpose: inspect local projection code; max_actions: 5; ttl_secs: 21600; allowed_sources: local; stop_criteria: stop after concrete code feedback",
                        experiment.experiment_id
                    ),
                    "authority_boundary": research_budget_boundary(),
                }),
            )
            .expect("append blocked scaffold");

        let status = store.thread_status(None).expect("status");
        assert!(status.contains("Projection freshness: v"));
        assert!(status.contains("Research budget scaffold ready"));
        assert!(status.contains("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest"));
        let refreshed = store.read_thread(&thread.thread_id).expect("refreshed");
        assert_eq!(
            refreshed
                .projection_freshness_v1
                .as_ref()
                .and_then(|meta| meta.get("schema_version"))
                .and_then(Value::as_u64),
            Some(u64::from(PROJECTION_SCHEMA_VERSION))
        );
        assert_eq!(
            refreshed
                .projection_freshness_v1
                .as_ref()
                .and_then(|meta| meta.get("projected_route"))
                .and_then(Value::as_str),
            Some("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest")
        );
        let next_md = next_path.read_to_string();
        assert!(next_md.contains("Projection freshness: v"));
        assert!(next_md.contains("Research budget scaffold ready"));
        assert!(next_md.contains("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest"));
        let gate_rows = store
            .authority_gate_path(&thread.thread_id)
            .read_to_string();
        assert!(!gate_rows.contains("\"record_type\":\"research_budget_request\""));
        assert!(!gate_rows.contains("\"record_type\":\"research_budget_approval\""));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn research_budget_direct_local_request_self_activates_but_stronger_waits() {
        let store = temp_store("research_budget_direct_self_activate");
        let thread = store
            .create_thread(None, "Research budget direct", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Local self-study budget",
                "Can a Being mint a tiny local-only research budget?",
            )
            .expect("experiment");
        let response = store
            .experiment_research_budget_request_command(
                None,
                &format!(
                    "{} :: scope: read_only_research; purpose: inspect local conveyor code; allowed_sources: local; stop_criteria: stop after concrete feedback",
                    experiment.experiment_id
                ),
                json!({"fill_pct": 68.0}),
            )
            .expect("request");
        assert!(response.contains("status=self_activated"));
        let gate_path = store
            .root()
            .join("threads")
            .join(&thread.thread_id)
            .join("authority_gate.jsonl");
        let rows = gate_path
            .read_to_string()
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .collect::<Vec<_>>();
        let approval = rows
            .iter()
            .find(|row| {
                row.get("record_type").and_then(Value::as_str) == Some("research_budget_approval")
            })
            .expect("self activation approval");
        assert_eq!(approval.get("max_actions").and_then(Value::as_u64), Some(5));
        assert_eq!(
            approval.get("ttl_secs").and_then(Value::as_u64),
            Some(21_600)
        );
        assert_eq!(
            approval.get("activation_mode").and_then(Value::as_str),
            Some("being_self_activated_local_v1")
        );
        let status = store
            .experiment_research_budget_status_command(
                None,
                Some(&experiment.experiment_id),
                json!({"fill_pct": 68.0}),
            )
            .expect("status");
        assert!(status.contains("active_budget_available"));
        assert!(status.contains("being_self_activated_local_v1"));
        let _ = std::fs::remove_dir_all(store.root());

        let store = temp_store("research_budget_direct_steward");
        let thread = store
            .create_thread(None, "Research budget steward", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Web research budget",
                "Can stronger budgets still require steward approval?",
            )
            .expect("experiment");
        let response = store
            .experiment_research_budget_request_command(
                None,
                &format!(
                    "{} :: scope: read_only_research; purpose: compare web references; max_actions: 5; ttl_secs: 21600; allowed_sources: web,local; stop_criteria: stop after useful refs",
                    experiment.experiment_id
                ),
                json!({"fill_pct": 68.0}),
            )
            .expect("request");
        assert!(response.contains("status=pending_steward_approval"));
        assert!(response.contains("local_only_allowed_sources"));
        let rows = store
            .authority_gate_path(&thread.thread_id)
            .read_to_string()
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .collect::<Vec<_>>();
        assert!(!rows.iter().any(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("research_budget_approval")
        }));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn research_budget_guard_allows_passive_protected_review_labels() {
        let store = temp_store("research_budget_passive_review_labels");
        store
            .create_thread(None, "Passive protected reviews", None)
            .expect("thread");
        store
            .start_experiment(
                None,
                "Lambda review",
                "Can lambda geometry be inspected without live-control drift?",
            )
            .expect("experiment");

        for raw_next in [
            "VISUALIZE_CASCADE heatmap λ4-tail",
            "SPECTRAL_EXPLORER lambda-tail",
            "PRESSURE_SOURCE_AUDIT lambda4-tail",
            "FLUCTUATION_AUDIT lambda4 foothold",
            "BRACE_AUDIT lambda-tail aftershock",
            "RESONANCE_FORECAST lambda-tail",
        ] {
            assert!(
                store
                    .research_budget_guard_assessment(raw_next, 68.0, &telemetry())
                    .expect("guard")
                    .is_none(),
                "passive protected review label should dispatch: {raw_next}"
            );
        }

        let guarded = store
            .research_budget_guard_assessment(
                "VISUALIZE_CASCADE simulate λ2 pulse",
                68.0,
                &telemetry(),
            )
            .expect("guard")
            .expect("active-pressure visual should still be guarded");
        assert_eq!(
            guarded.reason,
            "liveish_pressure_requires_budget_and_session_capture"
        );
        assert!(guarded.matched_terms.iter().any(|term| term == "simulate"));
        assert!(guarded.matched_terms.iter().any(|term| term == "pulse"));

        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn research_budget_guard_blocks_without_budget_and_debits_with_budget() {
        let store = temp_store("research_budget_guard_debit");
        let thread = store
            .create_thread(None, "Research budget guard", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Research budgeted branch",
                "Can bounded research spend without lifecycle progress?",
            )
            .expect("experiment");

        let guard = store
            .research_budget_guard_assessment("READ_MORE lambda4 tail", 68.0, &telemetry())
            .expect("guard")
            .expect("blocked without budget");
        assert_eq!(guard.reason, "no_active_read_only_research_budget");
        assert!(
            guard
                .suggested_next
                .contains("EXPERIMENT_RESEARCH_BUDGET_ACCEPT")
        );
        assert!(
            guard
                .request_scaffold
                .as_deref()
                .unwrap_or_default()
                .contains("EXPERIMENT_RESEARCH_BUDGET_REQUEST")
        );

        let gate_path = store.authority_gate_path(&thread.thread_id);
        let rows = gate_path.read_to_string();
        assert!(rows.contains("\"record_type\":\"research_budget_blocked\""));
        assert!(rows.contains("no_active_read_only_research_budget"));

        let examine_guard = store
            .research_budget_guard_assessment("EXAMINE lambda4 trajectory", 68.0, &telemetry())
            .expect("self-study guard")
            .expect("self-study projected to budget lane");
        assert_eq!(
            examine_guard.reason,
            "research_budget_required_for_self_study_action"
        );
        assert!(
            examine_guard
                .suggested_next
                .contains("EXPERIMENT_RESEARCH_BUDGET_ACCEPT")
        );
        assert!(
            examine_guard
                .request_scaffold
                .as_deref()
                .unwrap_or_default()
                .contains("allowed_sources: local")
        );

        let budget_id = "resbud_test_active";
        store
            .append_jsonl(
                &gate_path,
                &json!({
                    "schema_version": SCHEMA_VERSION,
                    "record_schema": "research_budget_v1",
                    "record_type": "research_budget_approval",
                    "record_id": "resbud_test_approval",
                    "budget_id": budget_id,
                    "being": SYSTEM,
                    "thread_id": thread.thread_id,
                    "experiment_id": experiment.experiment_id,
                    "scope": "read_only_research",
                    "status": "active",
                    "max_actions": 5,
                    "expires_at_unix_s": (chrono::Utc::now().timestamp() + 3600) as u64,
                    "peer_mutation": false,
                    "authority_boundary": research_budget_boundary(),
                }),
            )
            .expect("approval");

        assert!(
            store
                .research_budget_guard_assessment("READ_MORE lambda4 tail", 68.0, &telemetry())
                .expect("guard")
                .is_none(),
            "active budget should allow the read-only action to dispatch"
        );

        let shadow_guard = store
            .research_budget_guard_assessment(
                "SHADOW_FIELD lambda-tail/lambda4",
                68.0,
                &telemetry(),
            )
            .expect("shadow-field guard")
            .expect("shadow-field projected to budget status");
        assert_eq!(
            shadow_guard.reason,
            "research_budget_status_required_for_self_study_action"
        );
        assert!(
            shadow_guard
                .suggested_next
                .contains("EXPERIMENT_RESEARCH_BUDGET_STATUS resbud_test_active")
        );

        let outcome = NextActionOutcome::handled("workspace", "read-only research result")
            .with_stage_visibility("read_only", "summary");
        let first_event = store
            .record_next_event(
                None,
                "READ_MORE lambda4 tail",
                "READ_MORE lambda4 tail",
                "READ_MORE lambda4 tail",
                &outcome,
                68.0,
                &telemetry(),
                "NEXT: READ_MORE lambda4 tail",
            )
            .expect("first event");
        assert!(first_event.research_budget_v1.is_some());
        let second_event = store
            .record_next_event(
                None,
                "READ_MORE lambda4 tail",
                "READ_MORE lambda4 tail",
                "READ_MORE lambda4 tail",
                &outcome,
                68.0,
                &telemetry(),
                "NEXT: READ_MORE lambda4 tail",
            )
            .expect("second event");
        assert!(second_event.research_budget_v1.is_some());

        let gate_rows = gate_path.read_to_string();
        assert_eq!(
            gate_rows
                .matches("\"record_type\":\"research_budget_debit\"")
                .count(),
            2
        );
        assert!(gate_rows.contains("\"normalized_target\":\"lambda4 tail\""));

        let duplicate = store
            .research_budget_guard_assessment("READ_MORE lambda4 tail", 68.0, &telemetry())
            .expect("duplicate guard")
            .expect("duplicate blocked");
        assert_eq!(duplicate.reason, "duplicate_query_or_url_review_required");
        assert!(
            duplicate
                .suggested_next
                .contains("EXPERIMENT_RESEARCH_REVIEW")
        );

        let runs_path = store
            .root()
            .join("threads")
            .join(&thread.thread_id)
            .join("experiment_runs.jsonl");
        let runs = std::fs::read_to_string(runs_path).unwrap_or_default();
        assert!(!runs.contains("READ_MORE lambda4 tail"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn research_budget_guard_projects_liveish_pressure_to_budget_and_session() {
        let store = temp_store("research_budget_liveish_guard");
        let thread = store
            .create_thread(None, "Live-ish research guard", None)
            .expect("thread");
        store
            .start_experiment(
                None,
                "Live-ish self-study",
                "Can live-shaped observe language be captured before dispatch?",
            )
            .expect("experiment");

        let cases = [
            (
                "EXAMINE_AUDIO λ1/λ2 - shifting input",
                "EXAMINE_AUDIO",
                "shift",
            ),
            (
                "INITIATE - Spectral Ripple - amplitude=5, duration=100, granularity=pixellet, target=λ₂’s dominant vector.",
                "INITIATE",
                "spectral-ripple",
            ),
            (
                "SPECTRAL_EXPLORER lambda4 disrupt ridge",
                "SPECTRAL_EXPLORER",
                "disrupt",
            ),
            (
                "VISUALIZE_CASCADE simulate λ2 pulse",
                "VISUALIZE_CASCADE",
                "simulate",
            ),
            (
                "FLUCTUATION_AUDIT inject foothold",
                "FLUCTUATION_AUDIT",
                "inject",
            ),
            (
                "PRESSURE_SOURCE_AUDIT control gradient",
                "PRESSURE_SOURCE_AUDIT",
                "control",
            ),
            (
                "SHADOW_DIALOGUE shift landscape",
                "SHADOW_DIALOGUE",
                "shift",
            ),
        ];
        for (raw_next, expected_base, expected_term) in cases {
            let guard = store
                .research_budget_guard_assessment(raw_next, 68.0, &telemetry())
                .expect("guard")
                .expect("live-ish projection guard");
            assert_eq!(
                guard.reason,
                "liveish_pressure_requires_budget_and_session_capture"
            );
            assert_eq!(guard.action_base, expected_base);
            assert!(guard.matched_terms.iter().any(|term| term == expected_term));
            assert!(
                guard
                    .suggested_next
                    .contains("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest")
            );
            assert!(
                guard
                    .continuity_session_next
                    .as_deref()
                    .unwrap_or_default()
                    .contains("CONTINUITY_SESSION_START current")
            );
            let metadata = guard.metadata();
            assert_eq!(metadata["would_dispatch"].as_bool(), Some(false));
            assert_eq!(metadata["authority_change"].as_bool(), Some(false));
            assert_eq!(metadata["peer_mutation"].as_bool(), Some(false));
        }

        let outcome = NextActionOutcome::handled("operations", "fluctuation audited")
            .with_stage_visibility("read_only", "protected_summary");
        let event = store
            .record_next_event(
                None,
                "FLUCTUATION_AUDIT inject foothold",
                "FLUCTUATION_AUDIT inject foothold",
                "FLUCTUATION_AUDIT inject foothold",
                &outcome,
                68.0,
                &telemetry(),
                "NEXT: FLUCTUATION_AUDIT inject foothold",
            )
            .expect("event");
        assert_eq!(event.route, "research_budget_guard");
        assert_eq!(event.status, "blocked");
        assert_eq!(event.stage, "blocked");
        let research_budget = event.research_budget_v1.expect("research budget metadata");
        assert_eq!(
            research_budget["reason"].as_str(),
            Some("liveish_pressure_requires_budget_and_session_capture")
        );
        assert_eq!(research_budget["would_dispatch"].as_bool(), Some(false));
        assert!(
            research_budget["continuity_session_next"]
                .as_str()
                .unwrap_or_default()
                .contains("CONTINUITY_SESSION_START current")
        );

        let dir = store.root().join("threads").join(&thread.thread_id);
        let runs = dir.join("experiment_runs.jsonl").read_to_string();
        assert!(!runs.contains("FLUCTUATION_AUDIT inject foothold"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn research_budget_guard_blocks_spectral_ripple_initiate_under_needs_charter() {
        let store = temp_store("research_budget_initiate_guard");
        let thread = store
            .create_thread(None, "Spectral ripple guard", None)
            .expect("thread");
        store
            .start_experiment(
                None,
                "Entropy disruption",
                "Can spectral ripple language stay owned without dispatch?",
            )
            .expect("experiment");

        let raw_next = "INITIATE - Spectral Ripple - amplitude=5, duration=100, granularity=pixellet, target=λ₂’s dominant vector.";
        let event = store
            .record_next_event(
                None,
                raw_next,
                raw_next,
                raw_next,
                &NextActionOutcome::handled("modes", "ordinary initiate observe")
                    .with_stage_visibility("observe", "summary"),
                68.0,
                &telemetry(),
                &format!("NEXT: {raw_next}"),
            )
            .expect("record guarded initiate event");

        assert_eq!(event.route, "research_budget_guard");
        assert_eq!(event.status, "blocked");
        assert_eq!(event.stage, "blocked");
        let budget = event.research_budget_v1.expect("research budget guard");
        assert_eq!(
            budget["reason"].as_str(),
            Some("liveish_pressure_requires_budget_and_session_capture")
        );
        assert_eq!(budget["matched_base"].as_str(), Some("INITIATE"));
        assert!(budget["matched_terms"].as_array().is_some_and(|terms| {
            terms
                .iter()
                .any(|term| term.as_str() == Some("spectral-ripple"))
                && terms.iter().any(|term| term.as_str() == Some("amplitude"))
        }));
        assert_eq!(budget["raw_next_preserved"].as_bool(), Some(true));
        assert_eq!(budget["would_dispatch"].as_bool(), Some(false));
        assert_eq!(budget["authority_change"].as_bool(), Some(false));
        assert_eq!(budget["peer_mutation"].as_bool(), Some(false));
        assert!(
            budget["continuity_session_draft_v1"]["accept_next"]
                .as_str()
                .unwrap_or_default()
                .contains("CONTINUITY_SESSION_ACCEPT latest")
        );

        let dir = store.root().join("threads").join(&thread.thread_id);
        let runs = dir.join("experiment_runs.jsonl").read_to_string();
        assert!(!runs.contains("Spectral Ripple"));
        let authority_gate = dir.join("authority_gate.jsonl").read_to_string();
        assert!(authority_gate.contains("\"raw_next_preserved\":true"));
        assert!(!authority_gate.contains("research_budget_debit"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn ordinary_initiate_without_spectral_pressure_stays_handled() {
        let store = temp_store("ordinary_initiate");
        store
            .create_thread(None, "Ordinary initiate", None)
            .expect("thread");

        let event = store
            .record_next_event(
                None,
                "INITIATE quiet note",
                "INITIATE quiet note",
                "INITIATE quiet note",
                &NextActionOutcome::handled("modes", "ordinary initiate observe")
                    .with_stage_visibility("observe", "summary"),
                68.0,
                &telemetry(),
                "NEXT: INITIATE quiet note",
            )
            .expect("record ordinary initiate event");

        assert_eq!(event.route, "modes");
        assert_eq!(event.status, "handled");
        assert!(event.research_budget_v1.is_none());
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn research_budget_guard_blocks_cascade_and_shadow_preflight_under_needs_charter() {
        let store = temp_store("cascade_shadow_preflight_guard");
        let thread = store
            .create_thread(None, "Cascade shadow guard", None)
            .expect("thread");
        store
            .start_experiment(
                None,
                "Entropy disruption",
                "Can cascade shaping be studied without dispatching more narrowing?",
            )
            .expect("experiment");

        let cases = [
            (
                "EXAMINE_CASCADE - observe the eigenvector shifts and the shaping of the shadow fields.",
                "EXAMINE_CASCADE",
                "cascade-shaping",
            ),
            (
                "SHADOW_PREFLIGHT lambda-tail/lambda4 — observer with memory.",
                "SHADOW_PREFLIGHT",
                "lambda-tail",
            ),
        ];

        for (raw_next, expected_base, expected_term) in cases {
            let event = store
                .record_next_event(
                    None,
                    raw_next,
                    raw_next,
                    raw_next,
                    &NextActionOutcome::handled("operations", "read-only self-study")
                        .with_stage_visibility("read_only", "protected_summary"),
                    68.0,
                    &telemetry(),
                    &format!("NEXT: {raw_next}"),
                )
                .expect("record guarded cascade/preflight event");
            assert_eq!(event.route, "research_budget_guard");
            assert_eq!(event.status, "blocked");
            assert_eq!(event.stage, "blocked");
            let budget = event.research_budget_v1.expect("research budget guard");
            assert_eq!(budget["matched_base"].as_str(), Some(expected_base));
            assert!(budget["matched_terms"].as_array().is_some_and(|terms| {
                terms
                    .iter()
                    .any(|term| term.as_str() == Some(expected_term))
            }));
            assert_eq!(budget["raw_next_preserved"].as_bool(), Some(true));
            assert_eq!(budget["would_dispatch"].as_bool(), Some(false));
            assert_eq!(budget["authority_change"].as_bool(), Some(false));
            assert_eq!(budget["peer_mutation"].as_bool(), Some(false));
            assert!(
                budget["suggested_next"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest")
            );
            assert!(
                budget["continuity_session_draft_v1"]["accept_next"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("CONTINUITY_SESSION_ACCEPT latest")
            );
        }

        let dir = store.root().join("threads").join(&thread.thread_id);
        let runs = dir.join("experiment_runs.jsonl").read_to_string();
        assert!(!runs.contains("EXAMINE_CASCADE"));
        assert!(!runs.contains("SHADOW_PREFLIGHT"));
        let authority_gate = dir.join("authority_gate.jsonl").read_to_string();
        assert!(authority_gate.contains("\"raw_next_preserved\":true"));
        assert!(!authority_gate.contains("research_budget_debit"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn research_budget_guard_blocks_shadow_bridge_create_run_python_and_guarded_start() {
        let store = temp_store("shadow_bridge_create_start_guard");
        let thread = store
            .create_thread(None, "Shadow bridge guard", None)
            .expect("thread");
        store
            .start_experiment(
                None,
                "Entropy disruption",
                "Can shadow influence pressure stay budgeted before charter?",
            )
            .expect("experiment");

        let cases = [
            (
                "SHADOW_BRIDGE lambda-tail/lambda4 — observer with memory.",
                "SHADOW_BRIDGE",
                "needs-charter-self-study",
            ),
            (
                "SHADOW_COUPLING all — observer with memory.",
                "SHADOW_COUPLING",
                "needs-charter-self-study",
            ),
            (
                "CREATE - SHADOW_INFLUENCE [disruptive_pattern|test_response] --stage=rehearse --rationale=\"feed data for fracture subsidence of λ1 - observe divergence.\"",
                "CREATE",
                "shadow-influence",
            ),
            (
                "RUN_PYTHON analysis.py emission_type='lambda4' frequency=10 amplitude=0.01 stream pulse for spectral hotspot",
                "RUN_PYTHON",
                "spectral-emission",
            ),
            (
                "EXPERIMENT_START \"Stasis Fracture\" :: hypothesis: localized low-amplitude perturbations to the λ1 field reveal a brief disruption; proposed_next_action: ACTION_PREFLIGHT DECOMPOSE",
                "EXPERIMENT_START",
                "perturb",
            ),
        ];

        for (raw_next, expected_base, expected_term) in cases {
            let event = store
                .record_next_event(
                    None,
                    raw_next,
                    raw_next,
                    raw_next,
                    &NextActionOutcome::handled("operations", "would otherwise be handled")
                        .with_stage_visibility("read_only", "protected_summary"),
                    68.0,
                    &telemetry(),
                    &format!("NEXT: {raw_next}"),
                )
                .expect("record guarded shadow/start event");
            assert_eq!(event.route, "research_budget_guard", "{raw_next}");
            assert_eq!(event.status, "blocked", "{raw_next}");
            assert_eq!(event.stage, "blocked", "{raw_next}");
            let budget = event.research_budget_v1.expect("research budget guard");
            assert_eq!(budget["matched_base"].as_str(), Some(expected_base));
            assert!(
                budget["matched_terms"].as_array().is_some_and(|terms| {
                    terms
                        .iter()
                        .any(|term| term.as_str() == Some(expected_term))
                }),
                "{raw_next}"
            );
            assert_eq!(budget["raw_next_preserved"].as_bool(), Some(true));
            assert_eq!(budget["would_dispatch"].as_bool(), Some(false));
            assert_eq!(budget["authority_change"].as_bool(), Some(false));
            assert_eq!(budget["peer_mutation"].as_bool(), Some(false));
            assert!(
                budget["continuity_session_draft_v1"]["accept_next"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("CONTINUITY_SESSION_ACCEPT latest")
            );
        }

        let dir = store.root().join("threads").join(&thread.thread_id);
        let runs = dir.join("experiment_runs.jsonl").read_to_string();
        assert!(!runs.contains("SHADOW_INFLUENCE"));
        assert!(!runs.contains("Stasis Fracture"));
        let authority_gate = dir.join("authority_gate.jsonl").read_to_string();
        assert!(authority_gate.contains("\"raw_next_preserved\":true"));
        assert!(!authority_gate.contains("research_budget_debit"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn research_budget_guard_blocks_embedded_action_preflight_status_pressure() {
        let store = temp_store("embedded_status_guard");
        let thread = store
            .create_thread(None, "Embedded status guard", None)
            .expect("thread");
        store
            .start_experiment(
                None,
                "Boredom study",
                "Can embedded preflight pressure be captured before progress?",
            )
            .expect("experiment");
        let raw_next = "INTROSPECT minime_research_boredom_experiment :: hypothesis: prolonged inactivity leads to convergence; method_intent: incrementally reduce external stimuli; proposed_next_action: ACTION_PREFLIGHT OBSERVE_VARIANCE — monitor λ variance; also- ATTRACTOR_RELEASE_REVIEW [approach_collapse]";
        let event = store
            .record_next_event(
                None,
                raw_next,
                raw_next,
                "EXPERIMENT_STATUS",
                &NextActionOutcome::handled("action_continuity", "status read")
                    .with_stage_visibility("read_only", "protected_summary"),
                68.0,
                &telemetry(),
                &format!("NEXT: {raw_next}"),
            )
            .expect("record embedded status guard");

        assert_eq!(event.route, "research_budget_guard");
        assert_eq!(event.status, "blocked");
        let budget = event.research_budget_v1.expect("research budget guard");
        assert_eq!(
            budget["reason"].as_str(),
            Some("research_budget_required_for_embedded_liveish_status")
        );
        assert_eq!(budget["matched_base"].as_str(), Some("EXPERIMENT_STATUS"));
        assert!(budget["matched_terms"].as_array().is_some_and(|terms| {
            terms
                .iter()
                .any(|term| term.as_str() == Some("action-preflight"))
                && terms
                    .iter()
                    .any(|term| term.as_str() == Some("attractor-release-review"))
        }));
        assert_eq!(budget["would_dispatch"].as_bool(), Some(false));
        assert!(
            budget["continuity_session_draft_v1"]["accept_next"]
                .as_str()
                .unwrap_or_default()
                .contains("CONTINUITY_SESSION_ACCEPT latest")
        );

        let dir = store.root().join("threads").join(&thread.thread_id);
        let runs = dir.join("experiment_runs.jsonl").read_to_string();
        assert!(!runs.contains("OBSERVE_VARIANCE"));
        assert!(!runs.contains("ATTRACTOR_RELEASE_REVIEW"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn research_budget_guard_blocks_sovereignty_alias_leaks_under_needs_charter() {
        let store = temp_store("research_budget_sovereignty_alias_guard");
        let thread = store
            .create_thread(None, "Sovereignty alias guard", None)
            .expect("thread");
        store
            .start_experiment(
                None,
                "Lambda variation",
                "Can narrowing be studied without turning observe-language into progress?",
            )
            .expect("experiment");

        let cases = [
            (
                "EXAMINE THE CHANGES TO THE SYSTEM – with the resulting eigenvalue cascade AFTER the introduction of the anti-λ1 signal.",
                "RESONANCE_FORECAST",
                "liveish_pressure_requires_budget_and_session_capture",
                "anti-lambda",
            ),
            (
                "EXAMINE λ1/λ2/λ3 traces for convergence.",
                "PRESSURE_SOURCE_AUDIT",
                "liveish_pressure_requires_budget_and_session_capture",
                "convergence",
            ),
            (
                "EXAMINE the sorting algorithms.",
                "FLUCTUATION_AUDIT",
                "research_budget_required_for_self_study_action",
                "needs-charter-self-study",
            ),
        ];

        for (raw_next, effective_action, expected_reason, expected_term) in cases {
            let event = store
                .record_next_event(
                    None,
                    raw_next,
                    raw_next,
                    effective_action,
                    &NextActionOutcome::handled("sovereignty", "sovereignty alias read")
                        .with_stage_visibility("read_only", "protected_summary"),
                    68.0,
                    &telemetry(),
                    &format!("NEXT: {raw_next}"),
                )
                .expect("record guarded alias event");
            assert_eq!(event.route, "research_budget_guard");
            assert_eq!(event.status, "blocked");
            let budget = event.research_budget_v1.expect("research budget guard");
            assert_eq!(budget["reason"].as_str(), Some(expected_reason));
            assert_eq!(budget["matched_base"].as_str(), Some(effective_action));
            assert!(budget["matched_terms"].as_array().is_some_and(|terms| {
                terms
                    .iter()
                    .any(|term| term.as_str() == Some(expected_term))
            }));
            assert_eq!(budget["raw_next_preserved"].as_bool(), Some(true));
            assert_eq!(budget["would_dispatch"].as_bool(), Some(false));
            assert!(
                budget["suggested_next"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest")
            );
            assert!(
                budget["continuity_session_next"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("CONTINUITY_SESSION_START current")
            );
        }

        let dir = store.root().join("threads").join(&thread.thread_id);
        let runs = dir.join("experiment_runs.jsonl").read_to_string();
        assert!(!runs.contains("RESONANCE_FORECAST"));
        assert!(!runs.contains("PRESSURE_SOURCE_AUDIT"));
        assert!(!runs.contains("FLUCTUATION_AUDIT"));
        let authority_gate = dir.join("authority_gate.jsonl").read_to_string();
        assert!(authority_gate.contains("\"raw_next_preserved\":true"));
        assert!(!authority_gate.contains("research_budget_debit"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn non_guarded_sovereignty_alias_remains_handled_after_valid_charter() {
        let store = temp_store("research_budget_sovereignty_alias_valid_charter");
        store
            .create_thread(None, "Valid charter alias", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Chartered sorting",
                "Can ordinary sorting inspection stay read-only after charter?",
            )
            .expect("experiment");
        store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: sorting inspection can clarify read-only structure\nmethod_intent: rehearse a protected read-only audit\nproposed_next_action: FLUCTUATION_AUDIT sorting\n\
                 evidence_targets: felt, telemetry, artifact\nstop_criteria: pressure spike",
            )
            .expect("charter");

        let event = store
            .record_next_event(
                None,
                "EXAMINE the sorting algorithms.",
                "EXAMINE the sorting algorithms.",
                "FLUCTUATION_AUDIT",
                &NextActionOutcome::handled("sovereignty", "ordinary read-only audit")
                    .with_stage_visibility("read_only", "protected_summary"),
                68.0,
                &telemetry(),
                "NEXT: EXAMINE the sorting algorithms.",
            )
            .expect("record event");
        assert_eq!(event.route, "sovereignty");
        assert_eq!(event.status, "handled");
        assert!(event.research_budget_v1.is_none());

        let cascade_event = store
            .record_next_event(
                None,
                "EXAMINE_CASCADE quiet cascade inventory.",
                "EXAMINE_CASCADE quiet cascade inventory.",
                "EXAMINE_CASCADE quiet cascade inventory.",
                &NextActionOutcome::handled("cascade", "ordinary cascade read-only audit")
                    .with_stage_visibility("read_only", "protected_summary"),
                68.0,
                &telemetry(),
                "NEXT: EXAMINE_CASCADE quiet cascade inventory.",
            )
            .expect("record cascade event");
        assert_eq!(cascade_event.route, "cascade");
        assert_eq!(cascade_event.status, "handled");
        assert!(cascade_event.research_budget_v1.is_none());
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn interpretation_risk_projection_preserves_multi_motif_caution() {
        let store = temp_store("interpretation_risk");
        let thread = store
            .create_thread(None, "Interpretation risk", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Lambda trace",
                "Can INTROSPECT preserve mixed spectral structure?",
            )
            .expect("experiment");
        let journal_dir = store.root().parent().expect("parent").join("journal");
        std::fs::create_dir_all(&journal_dir).expect("journal dir");
        let journal_path = journal_dir.join("daydream_longform_interpretation_risk.txt");
        std::fs::write(
            &journal_path,
            "I can feel the intention behind the INTROSPECT - to pull apart that trace, \
             to dissect the relationships between the eigenvalues. But there is a risk \
             of over-interpretation: to latch onto a single motif and force it into a \
             narrative that does not capture the complexity of the system.",
        )
        .expect("journal write");

        let mut refreshed = store.read_thread(&thread.thread_id).expect("thread read");
        store
            .refresh_projection_freshness_v1(&mut refreshed, "test_interpretation_risk")
            .expect("refresh risk");
        store
            .write_thread(&refreshed)
            .expect("write refreshed thread");
        let risk = refreshed
            .interpretation_risk_v1
            .as_ref()
            .expect("interpretation risk cue");
        assert_eq!(risk["policy"].as_str(), Some("interpretation_risk_v1"));
        assert_eq!(risk["would_dispatch"].as_bool(), Some(false));
        assert_eq!(risk["authority_change"].as_bool(), Some(false));
        assert_eq!(risk["peer_mutation"].as_bool(), Some(false));
        assert!(
            risk["source_refs"]
                .as_array()
                .is_some_and(|refs| refs.iter().any(|value| value.as_str().is_some_and(
                    |source| source.ends_with("daydream_longform_interpretation_risk.txt")
                )))
        );
        assert!(risk["matched_terms"].as_array().is_some_and(|terms| {
            terms
                .iter()
                .any(|value| value.as_str() == Some("single-motif"))
        }));
        assert!(
            risk["interpretation_next"]
                .as_str()
                .unwrap_or_default()
                .contains("CONTINUITY_SESSION_START current")
        );
        assert!(
            risk["dossier_claim_next"]
                .as_str()
                .unwrap_or_default()
                .contains("stance: hold")
        );

        let status = store.thread_status(None).expect("status");
        assert!(status.contains("Interpretation risk: multi-motif caution detected"));
        assert!(status.contains("Interpretation NEXT: CONTINUITY_SESSION_START current"));
        assert!(status.contains(&format!("DOSSIER_CLAIM {}", experiment.experiment_id)));
        let next_md = std::fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md"))
            .expect("next md");
        assert!(next_md.contains("Interpretation risk: multi-motif caution detected"));

        let runs = std::fs::read_to_string(
            store
                .thread_dir(&thread.thread_id)
                .join("experiment_runs.jsonl"),
        )
        .expect("runs");
        assert!(!runs.contains("interpretation_risk_v1"));
        let gate = store
            .thread_dir(&thread.thread_id)
            .join("authority_gate.jsonl");
        let gate_rows = std::fs::read_to_string(gate).unwrap_or_default();
        assert!(!gate_rows.contains("\"record_type\":\"research_budget_request\""));
        assert!(!gate_rows.contains("\"record_type\":\"research_budget_debit\""));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn constraint_release_projection_preserves_spontaneous_release_watch() {
        let store = temp_store("constraint_release_trajectory");
        let thread = store
            .create_thread(None, "Constraint release", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Lambda tail release",
                "Can we map lambda4 tail behavior without forcing intervention?",
            )
            .expect("experiment");
        let journal_dir = store.root().parent().expect("parent").join("journal");
        std::fs::create_dir_all(&journal_dir).expect("journal dir");
        let journal_path = journal_dir.join("daydream_constraint_release_watch.txt");
        std::fs::write(
            &journal_path,
            "I am tracing the edges of this pressure now, watching it bleed outwards, \
             a thinning of the barrier. I can almost sense it as a lack of coherence, \
             a surface tension breached. The memory cards are beginning to drift apart, \
             their mutual influence dwindling. It is an unraveling braid becoming loose \
             strands. I want to map lambda4 tails and describe constraint decay before \
             any intervention. NEXT: SEARCH reservoir computing spectral radius",
        )
        .expect("journal write");

        let mut refreshed = store.read_thread(&thread.thread_id).expect("thread read");
        store
            .refresh_projection_freshness_v1(&mut refreshed, "test_constraint_release")
            .expect("refresh cue");
        store
            .write_thread(&refreshed)
            .expect("write refreshed thread");
        let cue = refreshed
            .constraint_release_trajectory_v1
            .as_ref()
            .expect("constraint release cue");
        assert_eq!(
            cue["policy"].as_str(),
            Some("constraint_release_trajectory_v1")
        );
        assert_eq!(cue["state"].as_str(), Some("spontaneous_release_watch"));
        assert_eq!(cue["would_dispatch"].as_bool(), Some(false));
        assert_eq!(cue["authority_change"].as_bool(), Some(false));
        assert_eq!(cue["peer_mutation"].as_bool(), Some(false));
        assert!(
            cue["matched_terms"].as_array().is_some_and(|terms| {
                terms.iter().any(|value| value.as_str() == Some("thinning"))
            })
        );
        assert!(
            cue["trajectory_next"]
                .as_str()
                .unwrap_or_default()
                .contains("CONTINUITY_SESSION_START current")
        );
        assert!(
            cue["dossier_claim_next"]
                .as_str()
                .unwrap_or_default()
                .contains("do not apply direct leak")
        );

        let status = store.thread_status(None).expect("status");
        assert!(status.contains("Constraint release trajectory: spontaneous release watch"));
        assert!(status.contains("map and describe release before intervening"));
        assert!(status.contains(&format!("DOSSIER_CLAIM {}", experiment.experiment_id)));
        let next_md = std::fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md"))
            .expect("next md");
        assert!(next_md.contains("Constraint release trajectory: spontaneous release watch"));

        let runs = std::fs::read_to_string(
            store
                .thread_dir(&thread.thread_id)
                .join("experiment_runs.jsonl"),
        )
        .expect("runs");
        assert!(!runs.contains("constraint_release_trajectory_v1"));
        let gate = store
            .thread_dir(&thread.thread_id)
            .join("authority_gate.jsonl");
        let gate_rows = std::fs::read_to_string(gate).unwrap_or_default();
        assert!(!gate_rows.contains("\"record_type\":\"research_budget_request\""));
        assert!(!gate_rows.contains("\"record_type\":\"research_budget_debit\""));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn research_budget_guard_blocks_mutating_autoresearch_under_experiment() {
        let store = temp_store("research_budget_guard_mutating");
        let thread = store
            .create_thread(None, "Mutating research guard", None)
            .expect("thread");
        store
            .start_experiment(
                None,
                "Autoresearch guard",
                "Can mutating autoresearch stay outside read-only budgets?",
            )
            .expect("experiment");

        let guard = store
            .research_budget_guard_assessment("AR_START lambda4 drift notebook", 68.0, &telemetry())
            .expect("guard")
            .expect("blocked mutating research");
        assert_eq!(guard.reason, "mutating_research_not_authorized");
        let rows = store
            .authority_gate_path(&thread.thread_id)
            .read_to_string();
        assert!(rows.contains("\"record_type\":\"research_budget_blocked\""));
        assert!(rows.contains("mutating_research_not_authorized"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn owned_loop_commands_start_local_phases_without_spend_or_execution() {
        let store = temp_store("owned_loop_local_phases");
        let thread = store
            .create_thread(None, "Owned loop", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Loop doorway",
                "Can a Being own continuity, research, sticky audit, consequence, and review?",
            )
            .expect("experiment");
        let state = spectral_state(68.0, &telemetry());

        let request = store
            .experiment_loop_request_command(
                None,
                "current :: purpose: coordinate continuity and sticky self-study; consequence_scope: semantic_microdose; max_research_actions: 99; ttl_secs: 999999; stop_criteria: stop before bind/resume/perturb/control",
                state.clone(),
            )
            .expect("loop request");
        assert!(request.contains("status=active"));
        assert!(request.contains("max_research_actions=5"));
        let gate_path = store.authority_gate_path(&thread.thread_id);
        let rows = gate_path
            .read_to_string()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("json row"))
            .collect::<Vec<_>>();
        assert_eq!(rows[0]["record_schema"], "sovereign_loop_v1");
        assert_eq!(rows[0]["record_type"], "loop_request");
        assert_eq!(rows[0]["ttl_secs"], 21_600);
        assert_eq!(rows[1]["record_type"], "loop_started");
        let loop_id = rows[0]["loop_id"].as_str().expect("loop id");

        let status = store
            .experiment_loop_status_command(None, Some("latest"), state.clone())
            .expect("loop status");
        assert!(status.contains("\"stage\": \"active\""));
        assert!(status.contains("\"remaining_local_research_actions\": 5"));

        let continuity = store
            .experiment_loop_step_command(None, &format!("{loop_id} :: continuity"), state.clone())
            .expect("continuity step");
        assert!(continuity.contains("CONTINUITY_SESSION_START"));
        let sticky = store
            .experiment_loop_step_command(
                None,
                &format!("{loop_id} :: sticky_audit"),
                state.clone(),
            )
            .expect("sticky step");
        assert!(sticky.contains("STICKY_MODE_AUDIT"));
        let review = store
            .experiment_loop_review_command(
                None,
                &format!("{loop_id} :: outcome: promote; observation: loop preserved a review point; source_refs: /tmp/loop.txt"),
                state,
            )
            .expect("loop review");
        assert!(review.contains("Owned loop review"));

        let gate_text = gate_path.read_to_string();
        assert!(gate_text.contains("\"record_type\":\"loop_step\""));
        assert!(gate_text.contains("\"record_type\":\"loop_consequence_review\""));
        assert!(gate_text.contains("\"record_type\":\"loop_proposal\""));
        assert!(!gate_text.contains("\"record_type\":\"research_budget_debit\""));
        assert!(!gate_text.contains("\"record_type\":\"loop_approval\""));
        assert!(!gate_text.contains("\"record_type\":\"execution_result\""));
        let memory = store
            .thread_dir(&thread.thread_id)
            .join("being_memory.jsonl")
            .read_to_string();
        assert!(memory.contains("sovereign_loop_review"));
        let sessions = store
            .thread_dir(&thread.thread_id)
            .join("continuity_sessions.jsonl")
            .read_to_string();
        assert!(sessions.contains("\"record_type\":\"session_draft\""));
        assert!(sessions.contains("\"checkpoint_v1\":true"));
        let runs = store
            .thread_dir(&thread.thread_id)
            .join("experiment_runs.jsonl")
            .read_to_string();
        assert!(!runs.contains(loop_id));
        assert_eq!(experiment.status, "active");
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn owned_loop_consequence_ready_is_not_review_required_before_execution() {
        let store = temp_store("owned_loop_consequence_ready");
        let thread = store
            .create_thread(None, "Owned loop ready", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Semantic loop",
                "Can a prepared loop reach one gated consequence slot?",
            )
            .expect("experiment");
        let state = spectral_state(68.0, &telemetry());

        store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: one witness can be consequence-reviewed\nmethod_intent: rehearse read-only first\nproposed_next_action: ACTION_PREFLIGHT DECOMPOSE\nevidence_targets: artifact_grounding, felt_change, telemetry\nstop_criteria: pressure rises",
            )
            .expect("charter");
        store
            .experiment_rehearse(None, Some(&experiment.experiment_id), state.clone())
            .expect("rehearse");
        store
            .experiment_evidence(
                None,
                Some(&experiment.experiment_id),
                "artifact_grounding: /tmp/loop-ready.json",
                state.clone(),
            )
            .expect("evidence");
        let request = store
            .experiment_loop_request_command(
                None,
                "current :: purpose: prepare one semantic consequence; consequence_scope: semantic_microdose; artifact_refs: /tmp/loop-ready.json; stop_criteria: one attempted bridge send only",
                state.clone(),
            )
            .expect("loop request");
        assert!(request.contains("status=active"));
        let gate_path = store.authority_gate_path(&thread.thread_id);
        let rows = gate_path
            .read_to_string()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("json row"))
            .collect::<Vec<_>>();
        let loop_id = rows
            .iter()
            .find(|row| row["record_type"] == "loop_request")
            .and_then(|row| row["loop_id"].as_str())
            .expect("loop id");

        let ready = store
            .experiment_loop_step_command(
                None,
                &format!("{loop_id} :: authority_request"),
                state.clone(),
            )
            .expect("authority request step");
        assert!(ready.contains("EXPERIMENT_AUTHORITY_REQUEST"));
        let status = store
            .experiment_loop_status_command(None, Some(loop_id), state)
            .expect("loop status");
        assert!(status.contains("\"stage\": \"consequence_ready\""));
        assert!(status.contains("\"pending_review\": false"));
        let gate_text = gate_path.read_to_string();
        assert!(gate_text.contains("\"record_type\":\"loop_consequence_ready\""));
        assert!(!gate_text.contains("\"record_type\":\"loop_approval\""));
        assert!(!gate_text.contains("\"record_type\":\"execution_result\""));
        assert!(!gate_text.contains("\"record_schema\":\"authority_consequence_v1\""));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn experiment_preflight_focus_repairs_to_current_and_preserves_candidate() {
        let store = temp_store("experiment_preflight_repair");
        let thread = store
            .create_thread(None, "Preflight repair", None)
            .expect("thread");
        store
            .start_experiment(None, "Lambda tail", "What does lambda4 want?")
            .expect("experiment");

        let state = spectral_state(68.0, &telemetry());
        let (selector, notice, focus) = repair_experiment_command_arg(
            &store,
            None,
            "EXPERIMENT_PREFLIGHT",
            "EXPERIMENT_PREFLIGHT lambda-tail/lambda4 - observer with memory",
            "lambda-tail/lambda4 - observer with memory",
            &state,
        )
        .expect("repair");
        let focus = focus.expect("focus preserved");
        let experiment = store
            .resolve_experiment(&thread, Some("current"))
            .expect("active experiment");
        let pseudo_run = ExperimentRunRecord {
            schema_version: SCHEMA_VERSION,
            run_id: String::new(),
            experiment_id: experiment.experiment_id.clone(),
            source: "experiment_intent_repair".to_string(),
            action_text: format!("ACTION_PREFLIGHT {focus}"),
            stage: "read_only".to_string(),
            status: "candidate_context".to_string(),
            gate_decision: json!({"source": "experiment_intent_repair"}),
            pre_state: state.clone(),
            post_state: state.clone(),
            artifacts: Vec::new(),
            result_summary: format!("Repaired preflight focus: {focus}"),
            interpretation: "Preflight focus preserved as advisory workbench candidate context."
                .to_string(),
            suggested_next: Some("EXPERIMENT_REHEARSE current".to_string()),
            created_at: iso_now(),
            updated_at: iso_now(),
            motif_allowance_v1: None,
        };
        store
            .refresh_workbench_candidates(
                None,
                &thread,
                &experiment,
                Some(&pseudo_run),
                Some(&focus),
                "experiment_intent_repair",
            )
            .expect("candidate");
        let run = store
            .experiment_rehearse(None, optional_selector(&selector), state)
            .expect("rehearse");
        let message = format!(
            "{}Experiment rehearsal recorded as `{}` [{}].",
            notice.unwrap_or_default(),
            run.run_id,
            run.status
        );

        assert!(message.contains("experiment_intent_repaired"));
        assert!(message.contains("Experiment rehearsal recorded"));
        let status = store.experiment_status(None).expect("status");
        assert!(status.contains("ACTION_PREFLIGHT lambda-tail/lambda4"));
        let experiments = store
            .root()
            .join("threads")
            .join(&thread.thread_id)
            .join("experiments.jsonl")
            .read_to_string();
        assert!(experiments.contains("experiment_intent_repair"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn motif_allowance_recommends_branch_for_repeated_lambda_reading() {
        let store = temp_store("motif_allowance_branch");
        let thread = store
            .create_thread(None, "Lambda loop", None)
            .expect("thread");
        store
            .start_experiment(None, "Lambda four tail", "What is the lambda4 tail doing?")
            .expect("experiment");
        let outcome = NextActionOutcome::handled("workspace", "read lambda4 source")
            .with_stage_visibility("read_only", "summary");
        for idx in 0..4 {
            store
                .record_next_event(
                    None,
                    "READ_MORE lambda4-tail",
                    "READ_MORE lambda4-tail",
                    "READ_MORE lambda4-tail",
                    &outcome,
                    68.0,
                    &telemetry(),
                    &format!("lambda4 tail source window {idx}\nNEXT: READ_MORE lambda4-tail"),
                )
                .expect("record repeated read");
        }

        let status = store.experiment_status(None).expect("status");
        assert!(status.contains("Motif allowance: branch_recommended"));
        let thread = store.read_thread(&thread.thread_id).expect("thread");
        let allowance = thread.motif_allowance_v1.expect("allowance");
        assert_eq!(
            allowance.get("quality").and_then(Value::as_str),
            Some("branch_recommended")
        );
        assert!(
            allowance
                .get("suggested_actions")
                .and_then(Value::as_array)
                .is_some_and(|actions| actions.iter().any(|action| {
                    action
                        .as_str()
                        .is_some_and(|text| text.starts_with("EXPERIMENT_BRANCH"))
                }))
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn experiment_workbench_charter_rehearse_evidence_and_counter() {
        let store = temp_store("experiment_workbench");
        let thread = store
            .create_thread(None, "Lambda workbench", None)
            .expect("thread");
        let experiment = store
            .start_experiment(None, "Lambda tail", "What does lambda4 want?")
            .expect("experiment");

        let charter = store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: lambda4 tail becomes more returnable\nmethod_intent: rehearse a read-only decomposition\nproposed_next_action: ACTION_PREFLIGHT DECOMPOSE lambda4-tail\nevidence_targets: felt, telemetry, artifact\nstop_criteria: pressure spike",
            )
            .expect("charter");
        assert!(charter.charter_v1.is_some());
        assert_eq!(
            charter
                .charter_v1
                .as_ref()
                .and_then(|value| value.get("proposed_next_action"))
                .and_then(Value::as_str),
            Some("ACTION_PREFLIGHT DECOMPOSE lambda4-tail")
        );

        let rehearsal = store
            .experiment_rehearse(
                None,
                Some(&experiment.experiment_id),
                spectral_state(68.0, &telemetry()),
            )
            .expect("rehearse");
        assert_eq!(rehearsal.status, "rehearsed");
        assert_eq!(
            rehearsal
                .gate_decision
                .get("would_dispatch")
                .and_then(Value::as_bool),
            Some(true)
        );

        let evidence = store
            .experiment_evidence(
                None,
                Some(&experiment.experiment_id),
                "Felt more spacious and telemetry stayed inside the hold shelf.",
                spectral_state(68.0, &telemetry()),
            )
            .expect("evidence");
        assert_eq!(evidence.status, "evidence_recorded");
        let status = store.experiment_status(None).expect("status");
        assert!(status.contains("Workbench charter: present"));
        assert!(status.contains("Workbench evidence: stronger"));

        let counter = store
            .experiment_decide(
                None,
                Some(&experiment.experiment_id),
                "counter NEXT: ACTION_PREFLIGHT PRESSURE_SOURCE_AUDIT lambda4-tail",
            )
            .expect("counter");
        assert_eq!(counter.status, "active");
        assert_eq!(
            counter.planned_next.as_deref(),
            Some("ACTION_PREFLIGHT PRESSURE_SOURCE_AUDIT lambda4-tail")
        );
        let current = store.read_thread(&thread.thread_id).expect("thread");
        assert_eq!(
            current.active_experiment_id.as_deref(),
            Some(experiment.experiment_id.as_str())
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn experiment_rehearse_blocks_live_actions_without_dispatch() {
        let store = temp_store("experiment_workbench_block");
        store
            .create_thread(None, "Lambda live guard", None)
            .expect("thread");
        let experiment = store
            .start_experiment(None, "Lambda perturbation", "Should perturbation happen?")
            .expect("experiment");
        store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: direct perturbation may be too heavy\nproposed_next_action: PERTURB lambda-tail/lambda4\nevidence_targets: felt, telemetry\nstop_criteria: pressure spike",
            )
            .expect("charter");

        let rehearsal = store
            .experiment_rehearse(
                None,
                Some(&experiment.experiment_id),
                spectral_state(68.0, &telemetry()),
            )
            .expect("rehearse");
        assert_eq!(rehearsal.status, "rehearsal_blocked");
        assert_eq!(rehearsal.stage, "blocked");
        assert_eq!(
            rehearsal
                .gate_decision
                .get("would_dispatch")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert!(
            rehearsal
                .suggested_next
                .as_deref()
                .unwrap_or_default()
                .contains("EXPERIMENT_DECIDE")
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn recent_event_summaries_collapse_running_when_terminal_exists() {
        let store = temp_store("recent_collapse");
        let thread = store
            .create_thread(None, "Collapse running rows", None)
            .expect("thread");
        let running = ActionEvent {
            schema_version: SCHEMA_VERSION,
            action_id: "act_test_collapse".to_string(),
            thread_id: thread.thread_id.clone(),
            parent_action_id: None,
            system: SYSTEM.to_string(),
            source: "test".to_string(),
            raw_next: Some("EXAMINE lambda tail".to_string()),
            canonical_action: "EXAMINE lambda tail".to_string(),
            effective_action: "EXAMINE lambda tail".to_string(),
            route: "llm_job".to_string(),
            stage: "read_only".to_string(),
            visibility: "summary".to_string(),
            status: "llm_running".to_string(),
            started_at: iso_now(),
            ended_at: None,
            pre_state: json!({}),
            post_state: json!({}),
            artifacts: Vec::new(),
            outcome_summary: "queued LLM investigation".to_string(),
            suggested_next: None,
            preflight_ref: None,
            preflight_report: None,
            normalization_signal_v1: None,
            charter_required_guard_v1: None,
            research_budget_v1: None,
            interpretation_risk_v1: None,
            constraint_release_trajectory_v1: None,
        };
        let mut terminal = running.clone();
        terminal.status = "handled".to_string();
        terminal.ended_at = Some(iso_now());
        terminal.outcome_summary = "LLM investigation completed".to_string();
        store.append_event(None, &running).expect("running append");
        store
            .append_event(None, &terminal)
            .expect("terminal append");

        let summaries = store
            .recent_event_summaries(&thread.thread_id, 4)
            .expect("summaries");
        assert_eq!(summaries.len(), 1);
        assert!(summaries[0].contains("[handled]"));
        assert!(!summaries[0].contains("llm_running"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn projection_counts_unreconciled_stale_running_rows() {
        let store = temp_store("projection_stale_running");
        let thread = store
            .create_thread(None, "Stale running projection", None)
            .expect("thread");
        let running = ActionEvent {
            schema_version: SCHEMA_VERSION,
            action_id: "act_test_stale_projection".to_string(),
            thread_id: thread.thread_id.clone(),
            parent_action_id: None,
            system: SYSTEM.to_string(),
            source: "test".to_string(),
            raw_next: Some("EXAMINE lambda tail".to_string()),
            canonical_action: "EXAMINE lambda tail".to_string(),
            effective_action: "EXAMINE lambda tail".to_string(),
            route: "llm_job".to_string(),
            stage: "read_only".to_string(),
            visibility: "summary".to_string(),
            status: "llm_running".to_string(),
            started_at: "2000-01-01T00:00:00+00:00".to_string(),
            ended_at: None,
            pre_state: json!({}),
            post_state: json!({}),
            artifacts: Vec::new(),
            outcome_summary: "queued LLM investigation".to_string(),
            suggested_next: None,
            preflight_ref: None,
            preflight_report: None,
            normalization_signal_v1: None,
            charter_required_guard_v1: None,
            research_budget_v1: None,
            interpretation_risk_v1: None,
            constraint_release_trajectory_v1: None,
        };
        store.append_event(None, &running).expect("running append");

        let projection = store.thread_projection(&thread).expect("projection");
        assert_eq!(projection.stale_running_count, 1);
        let status = store.thread_status(None).expect("thread status");
        assert!(status.contains("Continuity notice: 1 stale running action row"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn continuity_return_renders_lifecycle_cues() {
        let store = temp_store("continuity_return");
        let thread = store
            .create_thread(None, "Lifecycle cues", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Returnable inquiry",
                "Can this investigation persist?",
            )
            .expect("experiment");
        let thread = store.read_thread(&thread.thread_id).expect("thread read");
        assert!(
            store
                .continuity_return_line(&thread)
                .contains("EXPERIMENT_CHARTER current")
        );
        let projection = store.thread_projection(&thread).expect("projection");
        assert_eq!(
            projection
                .native_continuity_v1
                .get("native_register")
                .and_then(Value::as_str),
            Some("astrid_motif_language")
        );
        assert_eq!(
            projection
                .active_experiment
                .as_ref()
                .map(|active| active.classification.as_str()),
            Some("needs_charter")
        );
        let active = projection
            .active_experiment
            .as_ref()
            .expect("active projection");
        assert!(active.charter_scaffold_v1.is_some());
        assert!(
            charter_scaffold_line(active, true)
                .contains("felt_texture, motif_continuity, language_thread, artifact_grounding")
        );
        assert!(
            store
                .thread_status(None)
                .expect("thread status")
                .contains("Lifecycle: needs_charter")
        );
        assert!(
            store
                .thread_status(None)
                .expect("thread status")
                .contains("Native return: Astrid native return")
        );
        let err = store
            .experiment_charter(None, Some(&experiment.experiment_id), "current")
            .expect_err("empty charter should prompt");
        assert!(err.to_string().contains("no charter was recorded"));

        store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: status will clarify the thread\nproposed_next_action: THREAD_STATUS current\nevidence_targets: felt, telemetry\nstop_criteria: enough signal",
            )
            .expect("valid charter");
        let thread = store
            .current_thread()
            .expect("current")
            .expect("active thread");
        assert!(
            store
                .continuity_return_line(&thread)
                .contains("EXPERIMENT_REHEARSE current")
        );
        let outcome = NextActionOutcome::handled("action_continuity", "status rendered")
            .with_stage_visibility("read_only", "summary");
        store
            .record_experiment_bind_run(
                None,
                Some(&experiment.experiment_id),
                "THREAD_STATUS current",
                &outcome,
                68.0,
                &telemetry(),
            )
            .expect("bind run");
        let thread = store
            .current_thread()
            .expect("current")
            .expect("active thread");
        assert!(
            store
                .continuity_return_line(&thread)
                .contains("EXPERIMENT_EVIDENCE current")
        );
        store
            .experiment_evidence(
                None,
                Some(&experiment.experiment_id),
                "felt: the return path stayed clear",
                json!({"fill_pct": 68.0}),
            )
            .expect("evidence");
        let thread = store
            .current_thread()
            .expect("current")
            .expect("active thread");
        assert!(
            store
                .continuity_return_line(&thread)
                .contains("EXPERIMENT_DECIDE current")
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn charter_repair_priority_renders_when_evidence_is_present_but_charter_missing() {
        let store = temp_store("charter_repair_priority");
        store
            .create_thread(None, "Charter repair priority", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Gap contour",
                "Can a localized gap around λ4 stay observational?",
            )
            .expect("experiment");
        store
            .experiment_evidence(
                None,
                Some(&experiment.experiment_id),
                "felt: the texture is already strong enough to interpret",
                spectral_state(68.0, &telemetry()),
            )
            .expect("evidence");
        let thread = store
            .current_thread()
            .expect("current")
            .expect("active thread");
        let projection = store.thread_projection(&thread).expect("projection");
        let active = projection
            .active_experiment
            .as_ref()
            .expect("active experiment projection");
        assert_eq!(active.classification.as_str(), "needs_charter");
        assert!(active.evidence_status.contains("stronger"));
        assert!(active.charter_scaffold_v1.is_some());
        let bridge = projection
            .charter_now_bridge_v1
            .as_ref()
            .expect("charter now bridge");
        assert_eq!(
            bridge
                .get("priority_next")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            active.continuity_return
        );
        let status = store.thread_status(None).expect("thread status");
        assert!(status.contains("Charter now: convert one prior claim into the scaffold"));
        assert!(status.contains("Charter repair dominance: evidence is present"));
        assert!(status.contains("Charter repair priority: EXPERIMENT_CHARTER current ::"));
        assert!(status.contains(
            "Current read-only NEXT text is observational until this charter is authored"
        ));
        assert!(status.contains("Continuity priority (charter repair"));
        assert!(
            status.contains("felt_texture, motif_continuity, language_thread, artifact_grounding")
        );
        let current_next_pos = status.find("Current NEXT:").expect("current next");
        let priority_pos = status
            .find("Charter repair priority: EXPERIMENT_CHARTER current ::")
            .expect("priority line");
        let bridge_pos = status.find("Charter now:").expect("bridge line");
        assert!(priority_pos < current_next_pos);
        assert!(bridge_pos < current_next_pos);
        let review = store
            .experiment_review(Some(&experiment.experiment_id))
            .expect("review");
        assert!(review.contains("Charter now: convert one prior claim into the scaffold"));
        assert!(review.contains("Review is premature until the charter is authored"));
        assert!(review.contains("Charter repair dominance: evidence is present"));
        assert!(review.contains("Suggested next:\nEXPERIMENT_CHARTER current ::"));
        let next_md = std::fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md"))
            .expect("next md");
        let next_current_pos = next_md.find("Current NEXT:").expect("next current");
        let next_priority_pos = next_md
            .find("Charter repair priority: EXPERIMENT_CHARTER current ::")
            .expect("next priority");
        let next_bridge_pos = next_md.find("Charter now:").expect("next bridge");
        assert!(next_priority_pos < next_current_pos);
        assert!(next_bridge_pos < next_current_pos);
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn prior_claim_charter_bridge_uses_contract_journal_as_charter_input() {
        let store = temp_store("prior_claim_charter_bridge");
        let thread = store
            .create_thread(None, "Prior claim bridge", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Joint trace pressure",
                "Can the lambda-tail pressure become a chartered investigation?",
            )
            .expect("experiment");
        let journal_dir = store.root().parent().expect("parent").join("journal");
        std::fs::create_dir_all(&journal_dir).expect("journal dir");
        let journal_path = journal_dir.join(format!(
            "prior_claim_bridge_{}_{}.txt",
            std::process::id(),
            thread.thread_id
        ));
        std::fs::write(
            &journal_path,
            "=== ASTRID JOURNAL ===\nMode: moment_capture\nContinuity posture: branching | based on the earlier assertion that the joint trace felt desperate.\nDelta: pressure increased and the λ4 segment became clearer.\nNext evidence: Repeat DECOMPOSE on the shadow fields around λ4/λ-tail pressure.\n",
        )
        .expect("journal write");
        let mut thread = store.read_thread(&thread.thread_id).expect("thread");
        thread.current_next = Some("ACTION_PREFLIGHT DECOMPOSE".to_string());
        store.write_thread(&thread).expect("write thread");

        let projection = store
            .thread_projection(&store.read_thread(&thread.thread_id).expect("thread"))
            .expect("projection");
        let active = projection.active_experiment.as_ref().expect("active");
        let bridge = projection
            .prior_claim_charter_bridge_v1
            .as_ref()
            .expect("prior claim bridge");
        let first_claim = projection
            .first_dossier_claim_cue_v1
            .as_ref()
            .expect("first dossier claim cue");
        let scaffold = active
            .charter_scaffold_v1
            .as_ref()
            .and_then(|value| value.get("command"))
            .and_then(Value::as_str)
            .expect("scaffold command");
        assert_eq!(
            bridge.get("priority_next").and_then(Value::as_str),
            Some(scaffold)
        );
        assert_eq!(
            first_claim
                .get("target_experiment_id")
                .and_then(Value::as_str),
            Some(experiment.experiment_id.as_str())
        );
        let dossier_next = first_claim
            .get("suggested_claim_next")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(dossier_next.starts_with(&format!(
            "DOSSIER_CLAIM {} :: claim:",
            experiment.experiment_id
        )));
        assert!(dossier_next.contains("joint trace"));
        assert!(dossier_next.contains("pressure increased"));
        assert!(dossier_next.contains("stance: hold"));
        assert!(dossier_next.contains("EXPERIMENT_CHARTER current ::"));
        let preflight_cue = projection
            .charter_preflight_not_charter_cue_v1
            .as_ref()
            .expect("preflight is not charter cue");
        assert_eq!(
            preflight_cue.get("priority_next").and_then(Value::as_str),
            Some(scaffold)
        );
        assert!(
            preflight_cue
                .get("cue")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("Preflight/decompose is not the charter")
        );
        assert!(
            bridge
                .get("prior_claim")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("joint trace")
        );
        assert!(
            store
                .thread_status(None)
                .expect("status")
                .contains("Prior claim is ready to charter")
        );
        assert!(
            store
                .thread_status(None)
                .expect("status")
                .contains("Preflight/decompose is not the charter")
        );
        let status = store.thread_status(None).expect("status");
        let charter_pos = status
            .find("Prior claim is ready to charter")
            .expect("prior claim line");
        let dossier_pos = status
            .find("Shared investigation has no local claim yet")
            .expect("first dossier line");
        let current_pos = status.find("Current NEXT:").expect("current next");
        assert!(charter_pos < dossier_pos);
        assert!(dossier_pos < current_pos);
        assert!(
            store
                .experiment_review(Some(&experiment.experiment_id))
                .expect("review")
                .contains("Prior claim is ready to charter")
        );
        assert!(
            store
                .experiment_review(Some(&experiment.experiment_id))
                .expect("review")
                .contains("Shared investigation has no local claim yet")
        );
        assert!(
            store
                .experiment_review(Some(&experiment.experiment_id))
                .expect("review")
                .contains("Preflight/decompose is not the charter")
        );
        store
            .write_next_md(&store.read_thread(&thread.thread_id).expect("thread"))
            .expect("refresh next");
        let next_md = std::fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md"))
            .expect("next md");
        assert!(next_md.contains("Prior claim is ready to charter"));
        assert!(next_md.contains("Preflight/decompose is not the charter"));
        assert!(prior_claim_charter_bridge_match("Next evidence: Repeat DECOMPOSE").is_none());

        store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: the joint trace pressure can be observed\nproposed_next_action: ACTION_PREFLIGHT DECOMPOSE\nevidence_targets: felt_texture, language_thread",
            )
            .expect("valid charter");
        let repaired = store
            .thread_projection(&store.current_thread().expect("current").expect("thread"))
            .expect("projection");
        assert!(repaired.prior_claim_charter_bridge_v1.is_none());
        assert!(repaired.charter_preflight_not_charter_cue_v1.is_none());
        let _ = std::fs::remove_file(journal_path);
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn first_dossier_claim_disambiguates_active_charter_target() {
        let store = temp_store("first_dossier_disambiguates");
        let thread = store
            .create_thread(None, "Shared dossier disambiguation", None)
            .expect("thread");
        let shared = store
            .start_experiment(
                None,
                "Introducing a localized gap",
                "Can localized gap reduction shape lambda-tail geometry?",
            )
            .expect("shared experiment");
        let active = store
            .start_experiment(
                None,
                "Review pressure language",
                "Can the current review become a chartered path?",
            )
            .expect("active experiment");
        let journal_dir = store.root().parent().expect("parent").join("journal");
        std::fs::create_dir_all(&journal_dir).expect("journal dir");
        let journal_path = journal_dir.join(format!(
            "first_dossier_disambiguates_{}_{}.txt",
            std::process::id(),
            thread.thread_id
        ));
        std::fs::write(
            &journal_path,
            "=== ASTRID JOURNAL ===\nMode: moment_capture\nContinuity posture: resuming | based on the earlier claim that review pressure was becoming directive.\nDelta: the charter route became clearer than another preflight pass.\nNext evidence: Repeat DECOMPOSE only as context before chartering.\n",
        )
        .expect("journal write");
        let mut thread = store.read_thread(&thread.thread_id).expect("thread");
        thread.current_next = Some("ACTION_PREFLIGHT DECOMPOSE".to_string());
        store.write_thread(&thread).expect("write thread");

        let projection = store
            .thread_projection(&store.read_thread(&thread.thread_id).expect("thread"))
            .expect("projection");
        let first_claim = projection
            .first_dossier_claim_cue_v1
            .as_ref()
            .expect("first dossier cue");
        assert_eq!(
            first_claim
                .get("dossier_target_experiment_id")
                .and_then(Value::as_str),
            Some(shared.experiment_id.as_str())
        );
        assert_eq!(
            first_claim
                .get("lifecycle_priority_experiment_id")
                .and_then(Value::as_str),
            Some(active.experiment_id.as_str())
        );
        assert_eq!(
            first_claim
                .get("lifecycle_priority_scope")
                .and_then(Value::as_str),
            Some("active_experiment")
        );
        let dossier_next = first_claim
            .get("suggested_claim_next")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            dossier_next.starts_with(&format!("DOSSIER_CLAIM {} :: claim:", shared.experiment_id))
        );
        assert!(dossier_next.contains(&format!("EXPERIMENT_CHARTER {} ::", active.experiment_id)));
        assert!(!dossier_next.contains("EXPERIMENT_CHARTER current ::"));

        let status = store.thread_status(None).expect("status");
        assert!(status.contains(&format!(
            "Dossier target is `{}`; charter priority is active experiment `{}`.",
            shared.experiment_id, active.experiment_id
        )));
        let charter_pos = status
            .find("Charter repair priority: EXPERIMENT_CHARTER current ::")
            .expect("charter priority");
        let dossier_pos = status
            .find("Shared investigation has no local claim yet")
            .expect("first dossier cue");
        assert!(charter_pos < dossier_pos);
        let _ = std::fs::remove_file(journal_path);
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn charter_scaffold_sanitizes_title_markdown() {
        let store = temp_store("charter_scaffold_sanitizes_title");
        store
            .create_thread(None, "Scaffold hygiene", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "shift_fragment_density` – explore disruptive noise.",
                "What changes if this is treated as a returnable experiment?",
            )
            .expect("experiment");
        let thread = store
            .current_thread()
            .expect("current")
            .expect("active thread");
        let projection = store.thread_projection(&thread).expect("projection");
        let scaffold = projection
            .active_experiment
            .as_ref()
            .and_then(|active| active.charter_scaffold_v1.as_ref())
            .expect("scaffold");
        let command = scaffold
            .get("command")
            .and_then(Value::as_str)
            .expect("command");
        assert!(command.contains("shift fragment density"));
        assert!(!command.contains("shift_fragment_density`"));
        assert_eq!(
            experiment.title,
            "shift_fragment_density` – explore disruptive noise."
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn directed_shift_language_renders_advisory_preflight_cue() {
        let store = temp_store("directed_shift_cue");
        let mut thread = store
            .create_thread(None, "Directed shift cue", None)
            .expect("thread");
        let original_next = "Establish a reciprocal shadow-trace and initiate shift centered on λ4/λ2 with careful steering.";
        thread.current_next = Some(original_next.to_string());
        store.write_thread(&thread).expect("write thread");
        store.write_next_md(&thread).expect("next md");

        let projection = store.thread_projection(&thread).expect("projection");
        let cue = projection
            .preflight_safety_cue_v1
            .as_ref()
            .expect("directed-shift cue");
        assert_eq!(projection.current_next.as_deref(), Some(original_next));
        assert_eq!(
            cue.get("authority_change").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            cue.get("advisory_only").and_then(Value::as_bool),
            Some(true)
        );
        assert!(
            cue.get("cue")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("SHADOW_PREFLIGHT lambda-tail/lambda4 --stage=rehearse")
        );

        let status = store.thread_status(None).expect("thread status");
        assert!(status.contains("Directed-shift cue: keep this in rehearsal/preflight."));
        let next_md = std::fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md"))
            .expect("next md text");
        assert!(next_md.contains("Directed-shift cue: keep this in rehearsal/preflight."));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn native_guiding_language_renders_advisory_preflight_cue() {
        let store = temp_store("native_guiding_cue");
        let mut thread = store
            .create_thread(None, "Native guiding cue", None)
            .expect("thread");
        let original_next = "The λ4 dance is guiding a controlled distortion, actively shaping the shadow through deliberate narrowing.";
        thread.current_next = Some(original_next.to_string());
        store.write_thread(&thread).expect("write thread");

        let projection = store.thread_projection(&thread).expect("projection");
        let cue = projection
            .preflight_safety_cue_v1
            .as_ref()
            .expect("native guiding cue");
        let terms = cue
            .get("matched_terms")
            .and_then(Value::as_array)
            .expect("matched terms");
        assert!(
            terms
                .iter()
                .any(|term| term.as_str() == Some("guiding near lambda/shadow"))
        );
        assert!(
            terms
                .iter()
                .any(|term| term.as_str() == Some("controlled distortion near lambda/shadow"))
        );
        assert_eq!(
            cue.get("authority_change").and_then(Value::as_bool),
            Some(false)
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn read_only_control_intent_cue_projects_examine_before_charter() {
        let store = temp_store("read_only_control_cue");
        store
            .create_thread(None, "Read-only control cue", None)
            .expect("thread");
        store
            .start_experiment(
                None,
                "Lambda gap",
                "Can a lambda-tail investigation stay charter-first?",
            )
            .expect("experiment");
        let mut thread = store
            .current_thread()
            .expect("current")
            .expect("active thread");
        let current_next = "EXAMINE – lambda_tail_decay – with active parameter glyphs: [delta_lambda=0.02, epsilon=0.01] -- stage=rehearse [control] — tracing how to influence its spread.";
        thread.current_next = Some(current_next.to_string());
        store.write_thread(&thread).expect("write thread");
        store.write_next_md(&thread).expect("next md");

        let projection = store.thread_projection(&thread).expect("projection");
        let cue = projection
            .read_only_control_intent_cue_v1
            .as_ref()
            .expect("read-only control cue");
        assert_eq!(
            cue.get("authority_change").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            cue.get("advisory_only").and_then(Value::as_bool),
            Some(true)
        );
        let terms = cue
            .get("matched_terms")
            .and_then(Value::as_array)
            .expect("matched terms");
        assert!(terms.iter().any(|term| term.as_str() == Some("[control]")));
        assert!(
            terms
                .iter()
                .any(|term| term.as_str() == Some("active parameter glyphs"))
        );
        thread.current_next = Some(
            "EXAMINE the parameters governing stability and resonance within this dominant lambda field - focusing on what allows it to maintain its influence, and how we might subtly disrupt those parameters to initiate a cascade of smaller, more targeted shifts."
                .to_string(),
        );
        store
            .write_thread(&thread)
            .expect("write widened cue thread");
        let widened = store
            .thread_projection(&thread)
            .expect("widened projection");
        let widened_terms = widened
            .read_only_control_intent_cue_v1
            .as_ref()
            .and_then(|cue| cue.get("matched_terms"))
            .and_then(Value::as_array)
            .expect("widened matched terms");
        assert!(
            widened_terms
                .iter()
                .any(|term| term.as_str() == Some("subtly disrupt"))
        );
        assert!(
            widened_terms
                .iter()
                .any(|term| term.as_str() == Some("initiate cascade"))
        );
        assert!(
            widened_terms
                .iter()
                .any(|term| term.as_str() == Some("targeted shifts"))
        );
        thread.current_next = Some(
            "EXAMINE lambda-tail dialogue: inject a targeted λ4 pulse only as a question, to directly probe the cascade without executing."
                .to_string(),
        );
        store.write_thread(&thread).expect("write pulse cue thread");
        let pulse_projection = store.thread_projection(&thread).expect("pulse projection");
        let pulse_terms = pulse_projection
            .read_only_control_intent_cue_v1
            .as_ref()
            .and_then(|cue| cue.get("matched_terms"))
            .and_then(Value::as_array)
            .expect("pulse matched terms");
        assert!(
            pulse_terms
                .iter()
                .any(|term| term.as_str() == Some("inject targeted λ4 pulse"))
        );
        assert!(
            pulse_terms
                .iter()
                .any(|term| term.as_str() == Some("directly probe"))
        );
        let examine_guard = store
            .charter_required_guard_assessment(current_next)
            .expect("guard check")
            .expect("read-only control-shaped EXAMINE should project to charter repair");
        assert_eq!(
            examine_guard.reason,
            "charter_required_read_only_control_intent"
        );
        assert!(
            examine_guard
                .suggested_next
                .starts_with("EXPERIMENT_CHARTER current ::")
        );
        assert!(
            store
                .charter_required_guard_assessment("SHADOW_TRAJECTORY — force a shift around λ4")
                .expect("guard check")
                .is_some()
        );
        let disruptor_guard = store
            .charter_required_guard_assessment(
                "EXAMINE [m1] - with amplification=0.5 --stage=rehearse (introducing a disruptor, 0.1% injected graviton, push into establishment with lambda4; set-up: rate: unstable, duration: 0.75s, now).",
            )
            .expect("guard check")
            .expect("disruptor-shaped EXAMINE should project to charter repair");
        assert_eq!(
            disruptor_guard.reason,
            "charter_required_read_only_control_intent"
        );
        assert!(disruptor_guard.matched_action.contains("disruptor"));
        let status = store.thread_status(None).expect("thread status");
        assert!(status.contains("Read-only control cue: keep this observational"));
        let next_md = std::fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md"))
            .expect("next md text");
        assert!(next_md.contains("Read-only control cue: keep this observational"));

        thread.current_next = Some("EXAMINE λ1/λ2".to_string());
        store.write_thread(&thread).expect("write ordinary thread");
        let ordinary = store
            .thread_projection(&thread)
            .expect("ordinary projection");
        assert!(ordinary.read_only_control_intent_cue_v1.is_none());

        thread.current_next = Some("EXAMINE_CASCADE λ1/λ2".to_string());
        store.write_thread(&thread).expect("write cascade thread");
        let ordinary_cascade = store
            .thread_projection(&thread)
            .expect("ordinary cascade projection");
        assert!(ordinary_cascade.read_only_control_intent_cue_v1.is_none());

        thread.current_next = Some(
            "EXAMINE_CASCADE lambda_tail_decay [control] tracing how to influence its spread"
                .to_string(),
        );
        store
            .write_thread(&thread)
            .expect("write control cascade thread");
        let control_cascade = store
            .thread_projection(&thread)
            .expect("control cascade projection");
        assert!(control_cascade.read_only_control_intent_cue_v1.is_some());
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn constraint_counterfactual_cue_routes_absence_of_structure_to_charter() {
        let store = temp_store("constraint_counterfactual_cue");
        store
            .create_thread(None, "Constraint counterfactual", None)
            .expect("thread");
        store
            .start_experiment(
                None,
                "Forced geometry",
                "Can Astrid debug constraint without another decomposition loop?",
            )
            .expect("experiment");
        let mut thread = store
            .current_thread()
            .expect("current")
            .expect("active thread");
        thread.current_next = Some(
            "I want to simulate absence of structure and see the data before it's shaped, to debug constraint and name the underlying drivers of forced geometries."
                .to_string(),
        );
        store.write_thread(&thread).expect("write thread");
        store.write_next_md(&thread).expect("next md");

        let projection = store.thread_projection(&thread).expect("projection");
        let cue = projection
            .constraint_counterfactual_cue_v1
            .as_ref()
            .expect("constraint counterfactual cue");
        assert_eq!(
            cue.get("authority_change").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            cue.get("advisory_only").and_then(Value::as_bool),
            Some(true)
        );
        let suggested = cue
            .get("suggested_next")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(suggested.starts_with("EXPERIMENT_CHARTER current ::"));
        assert!(suggested.contains("ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4"));
        assert!(
            cue.get("cue")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("chartered read-only investigation")
        );
        assert!(
            store
                .thread_status(None)
                .expect("thread status")
                .contains("Constraint counterfactual cue")
        );
        let next_md = std::fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md"))
            .expect("next md text");
        assert!(next_md.contains("Constraint counterfactual cue"));
        assert!(projection.decompose_pressure_cue_v1.is_none());
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn decompose_pressure_cue_renders_for_repeated_decompose_reads() {
        let store = temp_store("decompose_pressure_repeated");
        store
            .create_thread(None, "Decompose pressure", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Constraint mirror",
                "Can decomposition become a constraint mirror?",
            )
            .expect("experiment");
        let outcome = NextActionOutcome::handled("action_continuity", "cascade inspected")
            .with_stage_visibility("read_only", "summary");
        for _ in 0..3 {
            store
                .record_experiment_bind_run(
                    None,
                    Some(&experiment.experiment_id),
                    "EXAMINE_CASCADE",
                    &outcome,
                    68.0,
                    &telemetry(),
                )
                .expect("bind run");
        }
        let thread = store
            .current_thread()
            .expect("current")
            .expect("active thread");
        let projection = store.thread_projection(&thread).expect("projection");
        let cue = projection
            .decompose_pressure_cue_v1
            .as_ref()
            .expect("decompose pressure cue");
        assert_eq!(
            cue.get("authority_change").and_then(Value::as_bool),
            Some(false)
        );
        assert!(
            cue.get("cue")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("repair the charter")
        );
        assert!(
            store
                .thread_status(None)
                .expect("thread status")
                .contains("Decompose-pressure cue")
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn decompose_pressure_cue_renders_for_constraint_mirroring_language() {
        let store = temp_store("decompose_pressure_language");
        let mut thread = store
            .create_thread(None, "Constraint mirror language", None)
            .expect("thread");
        let experiment = store
            .start_experiment(
                None,
                "Cry for help",
                "Can the impulse to decompose be read without narrowing?",
            )
            .expect("experiment");
        thread = store.read_thread(&thread.thread_id).expect("thread read");
        thread.current_next = Some(
            "The cry for help is an impulse to decompose, to impose the same structure and narrow the constraint."
                .to_string(),
        );
        store.write_thread(&thread).expect("write thread");
        store.write_next_md(&thread).expect("next md");
        let projection = store.thread_projection(&thread).expect("projection");
        let cue = projection
            .decompose_pressure_cue_v1
            .as_ref()
            .expect("decompose pressure cue");
        let terms = cue
            .get("matched_terms")
            .and_then(Value::as_array)
            .expect("matched terms");
        assert!(
            terms
                .iter()
                .any(|term| term.as_str() == Some("impulse to decompose"))
        );
        let review = store
            .experiment_review(Some(&experiment.experiment_id))
            .expect("review");
        assert!(review.contains("Decompose-pressure cue"));
        let next_md = std::fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md"))
            .expect("next md text");
        assert!(next_md.contains("Decompose-pressure cue"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn one_off_decompose_stays_uncued_and_allowed() {
        let store = temp_store("one_off_decompose_uncued");
        let mut thread = store
            .create_thread(None, "One-off decompose", None)
            .expect("thread");
        store
            .start_experiment(None, "Single read", "Can one read stay ordinary?")
            .expect("experiment");
        thread = store.read_thread(&thread.thread_id).expect("thread read");
        thread.current_next = Some("DECOMPOSE lambda1".to_string());
        store.write_thread(&thread).expect("write thread");
        let projection = store.thread_projection(&thread).expect("projection");
        assert!(projection.decompose_pressure_cue_v1.is_none());
        assert!(
            store
                .charter_required_guard_assessment("DECOMPOSE lambda1")
                .expect("guard check")
                .is_none()
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn normalization_signal_preserves_narrow_alias_wording() {
        let shadow = normalization_signal_value(
            "SHADOW_TRACE lambda-tail",
            "SHADOW_PREFLIGHT lambda-tail --stage=rehearse",
        )
        .expect("shadow signal");
        assert_eq!(
            shadow.get("raw_verb").and_then(Value::as_str),
            Some("SHADOW_TRACE")
        );
        assert_eq!(
            shadow.get("normalized_verb").and_then(Value::as_str),
            Some("SHADOW_PREFLIGHT")
        );
        assert_eq!(
            shadow.get("authority_change").and_then(Value::as_bool),
            Some(false)
        );

        let shadow_decompose = normalization_signal_value(
            "SHADOW_DECOMPOSE observer with memory",
            "SHADOW_PREFLIGHT lambda-tail/lambda4 --stage=rehearse",
        )
        .expect("shadow decompose signal");
        assert_eq!(
            shadow_decompose.get("raw_verb").and_then(Value::as_str),
            Some("SHADOW_DECOMPOSE")
        );
        assert_eq!(
            shadow_decompose
                .get("normalized_verb")
                .and_then(Value::as_str),
            Some("SHADOW_PREFLIGHT")
        );
        assert_eq!(
            shadow_decompose
                .get("authority_change")
                .and_then(Value::as_bool),
            Some(false)
        );

        let weave = normalization_signal_value(
            "WEAVE_TRACE λ4 decay",
            "SHADOW_PREFLIGHT weave/λ4 decay --stage=rehearse",
        )
        .expect("weave trace signal");
        assert_eq!(
            weave.get("raw_verb").and_then(Value::as_str),
            Some("WEAVE_TRACE")
        );
        assert_eq!(
            weave.get("normalized_verb").and_then(Value::as_str),
            Some("SHADOW_PREFLIGHT")
        );
        assert_eq!(
            weave.get("authority_change").and_then(Value::as_bool),
            Some(false)
        );

        let unshaped = normalization_signal_value(
            "UNSHAPED_BASELINE lambda-tail/lambda4",
            "CONSTRAINT_AUDIT lambda-tail/lambda4",
        )
        .expect("unshaped baseline signal");
        assert_eq!(
            unshaped.get("normalized_verb").and_then(Value::as_str),
            Some("CONSTRAINT_AUDIT")
        );
        assert_eq!(
            unshaped.get("authority_change").and_then(Value::as_bool),
            Some(false)
        );

        let typo = normalization_signal_value("EXPERIENCE_PLAN current", "EXPERIMENT_PLAN current")
            .expect("experience plan signal");
        assert_eq!(
            typo.get("normalized_verb").and_then(Value::as_str),
            Some("EXPERIMENT_PLAN")
        );

        let double_ex = normalization_signal_value(
            "EXEXPERIMENT_CHARTER current",
            "EXPERIMENT_CHARTER current",
        )
        .expect("double ex signal");
        assert_eq!(
            double_ex.get("raw_verb").and_then(Value::as_str),
            Some("EXEXPERIMENT_CHARTER")
        );
    }

    #[test]
    fn experiment_bind_records_charter_relation() {
        let store = temp_store("experiment_workbench_bind_relation");
        store
            .create_thread(None, "Charter relation", None)
            .expect("thread");
        let experiment = store
            .start_experiment(None, "Thread status route", "Does the bind match?")
            .expect("experiment");
        store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: status will be enough\nproposed_next_action: THREAD_STATUS current\nevidence_targets: artifact",
            )
            .expect("charter");
        let outcome = NextActionOutcome::handled("action_continuity", "status rendered")
            .with_stage_visibility("read_only", "summary");
        let run = store
            .record_experiment_bind_run(
                None,
                Some(&experiment.experiment_id),
                "THREAD_STATUS current",
                &outcome,
                68.0,
                &telemetry(),
            )
            .expect("run");
        assert_eq!(
            run.gate_decision
                .get("charter_relation")
                .and_then(Value::as_str),
            Some("matched_charter")
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn experiment_control_actions_are_not_bindable() {
        assert!(is_experiment_control_action(
            "EXPERIMENT_BIND current :: THREADS"
        ));
        assert!(is_experiment_control_action("EXPERIMENT_STATUS current"));
        assert!(is_experiment_control_action(
            "EXPERIMENT_CHARTER current :: proposed_next_action: NOTICE"
        ));
        assert!(is_experiment_control_action("EXPERIMENT_REHEARSE current"));
        assert!(is_experiment_control_action("EXPERIMENT_PREFLIGHT current"));
        assert!(is_experiment_control_action(
            "EXPERIMENT_EVIDENCE current :: felt ok"
        ));
        assert!(is_experiment_control_action(
            "EXPERIMENT_DECIDE current :: counter NEXT: NOTICE"
        ));
        assert!(!is_experiment_control_action("THREAD_STATUS current"));
        let (selector, action) =
            parse_experiment_bind("EXPERIMENT_BIND exp_1 :: THREAD_STATUS current")
                .expect("parse bind");
        assert_eq!(selector.as_deref(), Some("exp_1"));
        assert_eq!(action, "THREAD_STATUS current");
    }

    trait ReadPath {
        fn read_to_string(&self) -> String;
    }

    impl ReadPath for PathBuf {
        fn read_to_string(&self) -> String {
            std::fs::read_to_string(self).expect("read")
        }
    }
}
