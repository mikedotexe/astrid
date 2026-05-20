//! File-first action/thread continuity for Astrid.
//!
//! The JSON/JSONL files under `workspace/action_threads/` are authoritative.
//! SQLite rows are mirrors for querying and dashboards.

use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::db::{BridgeDb, unix_now};
use crate::paths::bridge_paths;
use crate::types::SpectralTelemetry;

const SCHEMA_VERSION: u32 = 1;
const DEFAULT_PRIVACY: &str = "summary";
const PROTECTED_VISIBILITY: &str = "protected_summary";
const PUBLIC_VISIBILITY: &str = "summary";
const SYSTEM: &str = "astrid";
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

impl CharterRequiredGuardAssessment {
    #[must_use]
    pub fn message(&self) -> String {
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
            "proposed_preflight_target": self.proposed_preflight_target,
            "authority_change": false,
            "would_dispatch": false,
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
}

#[derive(Debug, Clone)]
pub struct ActionContinuityStore {
    root: PathBuf,
}

impl ActionContinuityStore {
    #[must_use]
    pub fn for_astrid_workspace() -> Self {
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
        let thread = ResearchThread {
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
        };

        let dir = self.thread_dir(&thread_id);
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        self.write_json(&dir.join("thread.json"), &thread)?;
        self.ensure_thread_files(&thread_id)?;
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
        let suggested_next = self.continuity_return_command_for_runs(&experiment, &recent_runs);
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
            .into_iter()
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
                    "Active experiment: {} ({})\n{}{}{}Question: {}\nPlanned NEXT: {}\nLifecycle: {}\n{}\n{}\n{}\n",
                    active.experiment.title,
                    active.experiment.experiment_id,
                    charter_required_review_line(active),
                    charter_repair_priority_line(active),
                    charter_scaffold_line(active, true),
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
        let shared_investigation = shared_investigation_line(&projection.shared_investigation_v1);
        let status_charter_priority = projection
            .active_experiment
            .as_ref()
            .map_or_else(String::new, charter_repair_priority_line);
        Ok(format!(
            "Action thread `{}`: {}\nStatus: {}\nWhy return: {}\n{}{}{}{}{}Current NEXT: {}\n{}{}{}{}{}{}{}{}{}{}{}{}{}Recent events:\n{}\n{}",
            thread.thread_id,
            thread.title,
            thread.status,
            thread.why_return,
            status_charter_priority,
            charter_now_bridge,
            prior_claim_bridge,
            charter_preflight_not_charter,
            shared_investigation,
            thread.current_next.as_deref().unwrap_or("(none)"),
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
        let base_action = base_action(effective_next);
        let action_id = self.unique_action_id(&base_action)?;
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
        let visibility = visibility_for_action(&base_action).to_string();
        let stage = if outcome.stage == "read_only" || outcome.stage == "blocked" {
            outcome.stage.clone()
        } else {
            stage_for_action(&base_action).to_string()
        };
        let (status, outcome_summary) = evidence_adjusted_outcome(&base_action, &stage, outcome);
        let preflight_ref = self.preflight_ref_for_action(
            &thread.thread_id,
            canonical_next,
            effective_next,
            &outcome.route,
            &stage,
        )?;
        let event = ActionEvent {
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
        };
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
            "Experiment `{}`: {}\nQuestion: {}\n\nPlan prompt:\n{}- Hypothesis: name the structural change you expect to observe.\n- Method: choose one gated NEXT action and why it fits.\n- Measures: name the artifacts/metrics that would count as evidence.\n- Stop criteria: say what would make the run complete, blocked, or too pressurized.\n- Concrete next action example: EXPERIMENT_BIND {} :: ACTION_PREFLIGHT DECOMPOSE",
            experiment.experiment_id,
            experiment.title,
            experiment.question,
            focus_line,
            experiment.experiment_id
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
        experiment.planned_next = Some(format!("EXPERIMENT_REHEARSE {}", experiment.experiment_id));
        experiment.updated_at = iso_now();
        self.persist_experiment_update(db, &mut thread, &experiment, true)?;
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
        let constraint_counterfactual_cue = constraint_counterfactual_cue_line(
            &constraint_counterfactual_cue(&thread, Some(&projection), &recent_events),
        );
        let shared_investigation = shared_investigation_line(&projection.shared_investigation_v1);
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
            "Experiment review `{}`: {}\n{}{}{}{}{}{}{}{}{}{}Question: {}\nLifecycle: {}\n{}\n{}\n{}\n{}Learned so far:\n{}\n\nReview lens: completion is strong when felt evidence and telemetry/artifact evidence both exist; otherwise classify it as thin rather than failed.\nAgency options: accept, refuse, counter, pause, or complete. Ordinary choices remain valid.\n\nContinuity return:\n{}\n\nSuggested next:\n{}",
            experiment.experiment_id,
            experiment.title,
            charter_now_bridge,
            prior_claim_bridge,
            charter_preflight_not_charter,
            charter_required_review_line(&projection),
            charter_repair_priority_line(&projection),
            charter_scaffold_line(&projection, true),
            read_only_control_cue,
            constraint_counterfactual_cue,
            decompose_pressure_cue,
            shared_investigation,
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
                    "\nActive experiment: {} ({})\nQuestion: {}\nPlanned NEXT: {}\nLifecycle: {}\n{}\n{}\n{}\nWorkbench reminder: author a charter, rehearse before live, record felt plus telemetry/artifact evidence, then accept/refuse/counter/pause/complete. Ordinary choices remain valid.\n",
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
        let shared_investigation = shared_investigation_line(&projection.shared_investigation_v1);
        let charter_priority = projection
            .active_experiment
            .as_ref()
            .map_or_else(String::new, charter_repair_priority_line);
        let charter_scaffold = projection
            .active_experiment
            .as_ref()
            .map_or_else(String::new, |active| charter_scaffold_line(active, true));
        let body = format!(
            "# {}\n\n{}{}{}{}{}Current NEXT: {}\n\nWhy return: {}\n{}{}{}{}{}{}{}{}{}{}{}\nProtected note: ambiguity and private reflection remain valid; this thread is a return path, not a demand for productivity.\n",
            thread.title,
            charter_priority,
            charter_now_bridge,
            prior_claim_bridge,
            charter_preflight_not_charter,
            shared_investigation,
            thread.current_next.as_deref().unwrap_or("(none yet)"),
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
        let active_experiment = active_id
            .and_then(|id| self.resolve_experiment(thread, Some(id)).ok())
            .map(|experiment| self.experiment_projection(thread, &experiment, None))
            .transpose()?;
        let continuity_return = active_experiment
            .as_ref()
            .map(|projection| projection.continuity_return.clone())
            .unwrap_or_default();
        let native_continuity_v1 = active_experiment
            .as_ref()
            .map(|projection| projection.native_continuity_v1.clone())
            .unwrap_or_else(|| astrid_native_continuity(thread, None, &[]));
        let shared_investigation_v1 = active_experiment
            .as_ref()
            .and_then(|projection| projection.shared_investigation_v1.clone());
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
        Ok(ExperimentContinuityProjection {
            experiment: experiment.clone(),
            continuity_return,
            classification,
            native_continuity_v1,
            shared_investigation_v1: self.shared_investigation_v1(experiment),
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
        let cutoff = chrono::Utc::now() - chrono::Duration::minutes(45);
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
                *counts.entry(base).or_default() += 1;
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
                *counts.entry(base).or_default() += 1;
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
            "paused" => format!("EXPERIMENT_RESUME {}", experiment.experiment_id),
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
        serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
    }

    fn write_thread(&self, thread: &ResearchThread) -> Result<()> {
        self.write_json(
            &self.thread_dir(&thread.thread_id).join("thread.json"),
            thread,
        )?;
        self.write_next_md(thread)?;
        let mut index = self.load_index()?;
        index.active_thread_id = Some(thread.thread_id.clone());
        push_recent(&mut index.recent_threads, thread.thread_id.clone());
        index.updated_at = iso_now();
        self.save_index(&index)
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
            *motif_counts.entry(motif.clone()).or_insert(0) += 1;
        }
        let (dominant_motif, motif_hits) = motif_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .unwrap_or_else(|| ("open inquiry".to_string(), 0));
        let mut action_counts = HashMap::<String, usize>::new();
        for event in &events {
            *action_counts
                .entry(base_action(&event.canonical_action))
                .or_insert(0) += 1;
        }
        for run in &runs {
            *action_counts
                .entry(base_action(&run.action_text))
                .or_insert(0) += 1;
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
            "Peer experiment reference ({command}) `{}` belongs to {}.\n{}{}This is advisory: Astrid cannot bind runs, close, or mutate the peer experiment.\nLocal active experiment: {}\n{}Suggested local next: EXPERIMENT_PLAN current; EXPERIMENT_STATUS current; EXPERIMENT_PEER_REVIEW current.",
            peer.peer_experiment_id,
            peer.peer_system,
            focus,
            note,
            thread
                .active_experiment_id
                .as_deref()
                .unwrap_or("(none selected)"),
            snapshot,
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
        let constraint_counterfactual_cue = constraint_counterfactual_cue_line(
            &constraint_counterfactual_cue(thread, Some(&projection), &recent_events),
        );
        let shared_investigation = shared_investigation_line(&projection.shared_investigation_v1);
        format!(
            "Experiment `{}`: {}\n{}{}{}{}{}{}{}{}{}{}Thread: {}\nStatus: {}\nLifecycle: {}\nQuestion: {}\nHypothesis: {}\nAuthority: {}\nPlanned NEXT: {}\nContinuity return: {}\n{}{}{}\n{}\n{}\nMotif allowance: {} dominant={} action_concentration={} returnability={}\nLatest runs:\n{}",
            experiment.experiment_id,
            experiment.title,
            charter_now_bridge,
            prior_claim_bridge,
            charter_preflight_not_charter,
            charter_required_review_line(&projection),
            charter_repair_priority_line(&projection),
            charter_scaffold_line(&projection, true),
            read_only_control_cue,
            constraint_counterfactual_cue,
            decompose_pressure_cue,
            shared_investigation,
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
                "Active experiment: {} ({}) question={} planned_next={}\nLifecycle: {}\n{}\n{}\n{}Workbench reminder: author a charter, rehearse before live, record felt plus telemetry/artifact evidence, then accept/refuse/counter/pause/complete. Ordinary choices remain valid.\n",
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
    let shared_investigation = shared_investigation_line(&projection.shared_investigation_v1);
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
        "Current action thread: {} ({})\nWhy return: {}\n{}{}{}{}Current NEXT: {}\n{}{}{}{}{}{}{}{}{}{}{}{}{}Recent thread events:\n{}\nThread actions available: THREAD_START, THREADS, THREAD_STATUS, THREAD_NOTE, RESUME, SAVEPOINT, RECALL.\nExperiment actions available: ACTION_PREFLIGHT <NEXT action>, EXPERIMENT_START, EXPERIMENT_PLAN, EXPERIMENT_CHARTER, EXPERIMENT_REHEARSE, EXPERIMENT_PREFLIGHT, EXPERIMENT_EVIDENCE, EXPERIMENT_DECIDE, EXPERIMENT_BIND, EXPERIMENT_OBSERVE, EXPERIMENT_STATUS, EXPERIMENT_REVIEW, EXPERIMENT_CLOSE, EXPERIMENT_PEER_REVIEW, EXPERIMENT_BRANCH, EXPERIMENT_RESUME, EXPERIMENT_COMPARE, EXPERIMENT_ALT_PATHS. Read-only research actions auto-link when an experiment is active.",
        thread.title,
        thread.thread_id,
        thread.why_return,
        charter_now_bridge,
        prior_claim_bridge,
        charter_preflight_not_charter,
        shared_investigation,
        thread.current_next.as_deref().unwrap_or("(none)"),
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
        }
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
        object
            .entry("resume_next".to_string())
            .or_insert_with(|| json!(format!("EXPERIMENT_RESUME {experiment_id}")));
    } else if matches!(status.as_str(), "complete" | "completed") && !experiment_id.is_empty() {
        object.entry("inspect_next".to_string()).or_insert_with(|| {
            json!(format!(
                "EXPERIMENT_STATUS {experiment_id} or EXPERIMENT_REVIEW {experiment_id}"
            ))
        });
    }
    Some(summary)
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
    let planned_next = summary
        .get("planned_next")
        .or_else(|| summary.get("resume_next"))
        .and_then(Value::as_str)
        .unwrap_or("(none)");
    let mut lines = format!(
        "Last experiment summary: {title} ({experiment_id}) status={status}\nLast planned NEXT: {planned_next}\n"
    );
    if status == "paused" && experiment_id != "unknown" {
        lines.push_str(&format!(
            "Suggested NEXT: EXPERIMENT_RESUME {experiment_id}\n"
        ));
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
    let mut rest = raw[start + option.len()..].trim_start();
    if let Some(stripped) = rest.strip_prefix('=') {
        rest = stripped.trim_start();
    }
    if rest.is_empty() {
        return None;
    }
    let value = if let Some(quote) = rest.chars().next().filter(|ch| matches!(ch, '"' | '\'')) {
        let close = rest[quote.len_utf8()..].find(quote)?;
        rest[quote.len_utf8()..quote.len_utf8() + close].to_string()
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
    let right = text[idx + " with ".len()..].trim();
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
    {
        if felt.last().is_some_and(|entry| {
            entry
                .get("note")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .is_empty()
        }) {
            felt.pop();
        }
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
            let value = text[idx + "NEXT:".len()..].trim();
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
    format!("{text}\nSuggested NEXT: {compare}\nAlternate NEXT: {review}\n{advisory}\n")
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
        "Shared investigation response contract:\n- Peer claim to answer: {peer_claim}\n- Local evidence lane: {local_lane}\n- Allowed stances: support, counter, branch, hold.\n- {advisory}\n"
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
        .replace('-', " ")
        .replace('—', " ")
        .replace('–', " ")
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
        .replace('\u{2013}', "-")
        .replace('\u{2014}', "-")
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
    run_count + event_count
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
    run_count + event_count
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
        return compact_text(normalized[index + "based on".len()..].trim(), 180);
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
        | "FOOTHOLD_AUDIT" => PROTECTED_VISIBILITY,
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
        | "VISUALIZE_CASCADE"
        | "RECONVERGENCE_MAP"
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
    fn needs_charter_guard_blocks_compound_directed_intent_but_allows_reads() {
        let store = temp_store("charter_guard_compound");
        store
            .create_thread(None, "Compound guard", None)
            .expect("thread");
        store
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
        assert!(plan.contains(&format!(
            "EXPERIMENT_BIND {} :: ACTION_PREFLIGHT DECOMPOSE",
            experiment.experiment_id
        )));
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
    fn active_experiment_auto_links_read_only_research_action() {
        let store = temp_store("experiment_auto_link");
        let thread = store
            .create_thread(None, "Read-only research loop", None)
            .expect("thread");
        store
            .start_experiment(
                None,
                "Pressure source loop",
                "Which read-only audits keep the experiment returnable?",
            )
            .expect("experiment");

        let outcome = NextActionOutcome::handled("operations", "pressure source audited")
            .with_stage_visibility("read_only", "protected_summary");
        store
            .record_next_event(
                None,
                "PRESSURE_SOURCE_AUDIT lambda-edge",
                "PRESSURE_SOURCE_AUDIT lambda-edge",
                "PRESSURE_SOURCE_AUDIT lambda-edge",
                &outcome,
                68.0,
                &telemetry(),
                "NEXT: PRESSURE_SOURCE_AUDIT lambda-edge",
            )
            .expect("record next event");

        let dir = store.root().join("threads").join(&thread.thread_id);
        let runs = dir.join("experiment_runs.jsonl").read_to_string();
        assert!(runs.contains("PRESSURE_SOURCE_AUDIT lambda-edge"));
        assert!(runs.contains("active_experiment_auto_link"));
        let status = store.experiment_status(None).expect("status");
        assert!(status.contains("PRESSURE_SOURCE_AUDIT lambda-edge"));
        assert!(status.contains("Workbench draft candidates"));
        assert!(status.contains("EXPERIMENT_CHARTER current ::"));
        assert!(status.contains("EXPERIMENT_EVIDENCE current ::"));
        let experiments = dir.join("experiments.jsonl").read_to_string();
        assert!(experiments.contains("workbench_candidates_v1"));
        assert!(!experiments.contains("\"charter_v1\":{\""));
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
    fn read_only_control_intent_cue_renders_without_blocking_examine() {
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
        assert!(
            store
                .charter_required_guard_assessment(current_next)
                .expect("guard check")
                .is_none()
        );
        assert!(
            store
                .charter_required_guard_assessment("SHADOW_TRAJECTORY — force a shift around λ4")
                .expect("guard check")
                .is_some()
        );
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
