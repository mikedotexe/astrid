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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice_envelope_v1: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_residue_v1: Option<Value>,
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
type SovereignLoopLocation = (ResearchThread, ExperimentRecord, Value, Vec<Value>);

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
            choice_envelope_v1: None,
            transition_residue_v1: None,
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
        let choice_envelope_v1 =
            choice_envelope_value(response_text, raw_next, canonical_next, effective_next);
        let transition_residue_v1 = transition_residue_value(
            choice_envelope_v1.as_ref(),
            canonical_next,
            effective_next,
            telemetry,
        );
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
            choice_envelope_v1,
            transition_residue_v1,
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
            .clamp(1, LOOP_TTL_SECS);
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

    fn find_sovereign_loop(&self, loop_id: &str) -> Result<Option<SovereignLoopLocation>> {
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
        let blocked = latest_research_budget_scaffold_row(&rows, &experiment_id)?;
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
                let choice_summary = event_choice_summary(&event)
                    .map(|summary| format!("; {summary}"))
                    .unwrap_or_default();
                format!(
                    "{} [{}]: {}{}",
                    event.effective_action, event.status, event.outcome_summary, choice_summary
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
                    let choice_summary = event_choice_summary(event)
                        .map(|summary| format!("; {summary}"))
                        .unwrap_or_default();
                    format!(
                        "{} [{}]: {}{}",
                        event.effective_action, event.status, event.outcome_summary, choice_summary
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

    fn latest_self_study_review_json_path(&self) -> Option<PathBuf> {
        let workspace = self.root.parent().unwrap_or(self.root.as_path());
        let root = workspace.join("diagnostics/self_study_reviews");
        let mut latest: Option<(SystemTime, PathBuf)> = None;
        for entry in fs::read_dir(root).ok()?.flatten() {
            let candidate = if entry.file_type().ok()?.is_dir() {
                entry.path().join("review.json")
            } else {
                entry.path()
            };
            if candidate.file_name().and_then(|name| name.to_str()) != Some("review.json") {
                continue;
            }
            let modified = candidate.metadata().and_then(|meta| meta.modified()).ok()?;
            if latest
                .as_ref()
                .is_none_or(|(latest_modified, _)| modified > *latest_modified)
            {
                latest = Some((modified, candidate));
            }
        }
        latest.map(|(_, path)| path)
    }

    fn experiment_returnable_distinctions_line(&self, experiment: &ExperimentRecord) -> String {
        let Some(review_path) = self.latest_self_study_review_json_path() else {
            return String::new();
        };
        let Some(review) = fs::read_to_string(review_path)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        else {
            return String::new();
        };
        let Some(cards) = review
            .get("returnable_distinctions_v1")
            .and_then(|packet| packet.get("cards"))
            .and_then(Value::as_array)
        else {
            return String::new();
        };
        let experiment_text = format!(
            "{} {} {} {}",
            experiment.title,
            experiment.question,
            experiment.hypothesis.as_deref().unwrap_or_default(),
            experiment.planned_next.as_deref().unwrap_or_default()
        )
        .to_lowercase();
        let pressure_match = contains_any(
            &experiment_text,
            &[
                "pressure", "viscos", "silt", "heavy", "weight", "scar", "bruise",
            ],
        );
        let codec_match = contains_any(
            &experiment_text,
            &["codec", "compression", "warmth", "tension", "projection"],
        );
        let release_match =
            contains_any(&experiment_text, &["release", "exhale", "bypass", "dump"]);
        let rows = cards
            .iter()
            .filter(|card| {
                let card_id = card_scalar_text(card, "card_id");
                (pressure_match
                    && matches!(
                        card_id.as_str(),
                        "pressure_level_vs_pressure_velocity" | "slope_drag_vs_medium_mass"
                    ))
                    || (codec_match && card_id == "codec_smoothing_vs_pressure")
                    || (release_match && card_id == "release_rehearsal_vs_bypass")
            })
            .take(5)
            .map(|card| {
                format!(
                    "{}:{} lifecycle={} verdict={} route={} self={} experiment={}",
                    card_scalar_text(card, "card_id"),
                    card_scalar_text(card, "status"),
                    card_scalar_text(card, "lifecycle_state"),
                    card_scalar_text(card, "preflight_verdict"),
                    card_scalar_text(card, "recommended_read_only_route"),
                    card_scalar_text(card, "relevant_self_regulation_route"),
                    card_scalar_text(card, "relevant_experiment_lived_term_route")
                )
            })
            .collect::<Vec<_>>();
        if rows.is_empty() {
            return String::new();
        }
        format!(
            "Returnable distinctions: {}\nAuthority: diagnostic_context_not_command; advisory only, no experiment was created or advanced by this block.\n",
            rows.join("; ")
        )
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
        let returnable_distinctions = self.experiment_returnable_distinctions_line(experiment);
        format!(
            "Experiment `{}`: {}\n{}{}{}{}{}{}{}{}{}{}{}{}{}{}Thread: {}\nStatus: {}\nLifecycle: {}\nQuestion: {}\nHypothesis: {}\nAuthority: {}\nPlanned NEXT: {}\nContinuity return: {}\n{}{}{}\n{}\n{}\nMotif allowance: {} dominant={} action_concentration={} returnability={}\nLatest runs:\n{}",
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
            returnable_distinctions,
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
}
