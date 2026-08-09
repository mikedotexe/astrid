//! Sovereign Action grammar, validation, and owned-artifact execution boundary.
//!
//! This module remains intentionally co-located for the first CPU-edge release so
//! final-response parsing, provenance checks, filesystem confinement, and receipt
//! emission can be reviewed as one authority-bearing path. A later split should
//! follow Action families only after extracting one shared validation/receipt
//! transaction layer; it must not duplicate authority checks across modules.

use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tokio::sync::{broadcast, mpsc, watch};
use uuid::Uuid;

use crate::{
    codec::encode_text,
    config::Config,
    inquiry::{self, SharedStudyManager, StudySpec},
    ipc,
    maintenance::{WorkPermit, WorkTracker},
    notebook::ActivityEvent,
    peer,
    reservoir::{ReservoirSnapshot, SensoryIngress},
    trace::IpcTraceContextV1,
    tuning::{self, TuningAction, TuningProvenance, TuningRequest},
};

const SAFE_NEXT_REPAIR_MARKER: &str =
    "[Local contract repair: no valid final action was emitted; defaulting safely to LISTEN.]";
const FORMAT_NEXT_REPAIR_MARKER: &str =
    "[Local contract formatting repair: preserved one unambiguous model-authored terminal action.]";
const STREAMING_TIMEOUT_RECOVERY: &str = "react_streaming_timeout";
const REACT_PHASE_TIMEOUT_RECOVERY: &str = "react_phase_timeout";
const MAX_ACTION_ARGUMENT_CHARS: usize = 2_000;
const MAX_ARTIFACT_ID_CHARS: usize = 128;
const MAX_UNEXECUTED_INTENTION_CHARS: usize = 512;

#[derive(Debug)]
pub struct ActionCandidate {
    pub session_id: String,
    pub response: String,
    pub trace: Option<IpcTraceContextV1>,
    /// One-use turn identifier admitted only by a trusted runtime boundary.
    /// Trace metadata alone never populates this field.
    pub tuning_authority_turn_id: Option<Uuid>,
    /// Trusted boundary that admitted the one-use turn identifier.
    pub tuning_authority_source: Option<&'static str>,
    /// Exact process-local work accounting. Production admission boundaries
    /// populate this before enqueue; tests may leave it absent and the
    /// executor will acquire a permit on dequeue.
    pub maintenance_permit: Option<WorkPermit>,
}

#[derive(Debug)]
pub struct ActionOutcomeDelivery {
    pub outcome: ActionOutcome,
    _maintenance_permit: WorkPermit,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActionOutcome {
    pub recorded_at_unix_ms: u64,
    pub session_id: String,
    pub response_sha256: String,
    pub declared_next: Option<String>,
    pub decision_source: String,
    pub status: String,
    pub outcome: String,
    pub recovery_reason: Option<String>,
    pub unexecuted_intention: Option<String>,
    pub validation_reason: Option<String>,
    pub trace: Option<IpcTraceContextV1>,
}

#[derive(Debug, Clone, PartialEq)]
enum SovereignAction {
    Listen,
    Rest,
    Journal(String),
    Remember(String),
    SelfStudy(String),
    Propose(String),
    Notice(String),
    Daydream(String),
    Aspire(String),
    Research(String),
    Measure(String),
    Study(StudySpec),
    CancelStudy(String),
    TuneReservoir(tuning::TuningSpec),
    CancelTuning(String),
    ValidateTuning {
        candidate_id: String,
        question: String,
    },
    AdoptTuning {
        candidate_id: String,
        reason: String,
    },
    RevertTuning {
        adoption_id: String,
        reason: String,
    },
    Synthesize {
        evidence_ids: Vec<String>,
        claim: String,
    },
    Share {
        artifact_id: String,
        note: String,
    },
    Plan(String),
    Draft(String),
    Read(String),
    ReadSource(u8),
    Revise {
        artifact_id: String,
        revision: String,
    },
    Check(String),
}

#[derive(Serialize)]
struct ActionReceipt {
    schema: &'static str,
    recorded_at_unix_ms: u64,
    session_id: String,
    response_sha256: String,
    declared_next: Option<String>,
    decision_source: &'static str,
    status: &'static str,
    outcome: &'static str,
    recovery_reason: Option<&'static str>,
    unexecuted_intention: Option<String>,
    validation_reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution_error: Option<String>,
    artifact_path: Option<String>,
    fill_pct: f32,
    target_fill_pct: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace: Option<IpcTraceContextV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tuning_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tuning_candidate_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tuning_phase: Option<&'static str>,
    authority: &'static str,
}

#[derive(Deserialize)]
struct DurableActionOutcome {
    recorded_at_unix_ms: u64,
    session_id: String,
    response_sha256: String,
    declared_next: Option<String>,
    decision_source: String,
    status: String,
    outcome: String,
    recovery_reason: Option<String>,
    unexecuted_intention: Option<String>,
    validation_reason: Option<String>,
    trace: Option<IpcTraceContextV1>,
}

struct ExecutionResult {
    receipt_json: String,
    outcome: ActionOutcome,
    dispatch_key: Option<ActionDispatchKey>,
}

struct ActionExecution {
    status: &'static str,
    outcome: &'static str,
    artifact_path: Option<String>,
    tuning_id: Option<String>,
    tuning_candidate_id: Option<String>,
    tuning_phase: Option<&'static str>,
}

struct ActionInterpretation {
    declaration: Option<String>,
    parsed: Option<SovereignAction>,
    local_safe_fallback: bool,
    local_format_repair: bool,
    recovery_reason: Option<&'static str>,
    unexecuted_intention: Option<String>,
    validation_reason: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionDispatchEvidence {
    Absent,
    Pending,
    Completed,
}

#[derive(Debug, Clone)]
struct ActionDispatchKey {
    turn_id: Uuid,
    response_sha256: String,
    trace: IpcTraceContextV1,
}

#[derive(Serialize)]
struct ActionDispatchReceipt<'a> {
    schema: &'static str,
    phase: &'static str,
    recorded_at_unix_ms: u64,
    turn_id: Uuid,
    response_sha256: &'a str,
    trace: &'a IpcTraceContextV1,
    authority: &'static str,
}

#[allow(clippy::too_many_arguments)] // Explicit channels keep authority boundaries visible.
#[allow(clippy::too_many_lines)] // Keep mutation, durable completion, and feedback ordering together.
pub async fn run(
    config: Arc<Config>,
    mut candidates: mpsc::Receiver<ActionCandidate>,
    snapshots: watch::Receiver<ReservoirSnapshot>,
    ingress_tx: mpsc::Sender<SensoryIngress>,
    outcome_tx: mpsc::Sender<ActionOutcomeDelivery>,
    activity_tx: broadcast::Sender<ActivityEvent>,
    study_manager: SharedStudyManager,
    tuning_tx: mpsc::Sender<TuningRequest>,
    maintenance_work: Arc<WorkTracker>,
) {
    while let Some(mut candidate) = candidates.recv().await {
        let _action_permit = match candidate.maintenance_permit.take() {
            Some(permit) => permit,
            None => match maintenance_work.begin_action() {
                Ok(permit) => permit,
                Err(error) => {
                    eprintln!("sovereign NEXT action admission failed closed: {error:#}");
                    return;
                },
            },
        };
        let snapshot = snapshots.borrow().clone();
        match execute_candidate_with_studies(
            &config,
            &candidate,
            &snapshot,
            &study_manager,
            Some(&tuning_tx),
        )
        .await
        {
            Ok(result) => {
                let experiential = action_outcome_may_enter_experience(&result.outcome);
                if experiential {
                    let artifact_basename =
                        serde_json::from_str::<serde_json::Value>(&result.receipt_json)
                            .ok()
                            .and_then(|value| {
                                value
                                    .get("artifact_path")
                                    .and_then(serde_json::Value::as_str)
                                    .and_then(|path| path.rsplit('/').next())
                                    .map(ToOwned::to_owned)
                            });
                    let _ = activity_tx.send(ActivityEvent {
                        kind: "sovereign_action_outcome",
                        artifact_basename,
                        trace: result.outcome.trace.clone(),
                        response_sha256: Some(result.outcome.response_sha256.clone()),
                    });
                }
                if config.research_action_web_search
                    && let Some(question) = accepted_research_question(&result.outcome)
                    && let Err(error) = ipc::execute_research_search(
                        &config,
                        question,
                        result.outcome.trace.as_ref(),
                        Some(&result.outcome.response_sha256),
                    )
                    .await
                {
                    eprintln!("sovereign RESEARCH web execution failed: {error}");
                }
                if let Some(question) = accepted_self_study_question(&result.outcome) {
                    if let Some(spectral_question) = spectral_self_study_question(question) {
                        match result.outcome.trace.as_ref() {
                            Some(trace) => {
                                if let Err(error) = ipc::execute_spectral_query(
                                    &config,
                                    spectral_query_for_question(spectral_question),
                                    trace,
                                    &result.outcome.response_sha256,
                                    "action_executor_self_study_spectral",
                                )
                                .await
                                {
                                    eprintln!("sovereign spectral SELF_STUDY failed: {error}");
                                }
                            },
                            None => eprintln!(
                                "sovereign spectral SELF_STUDY refused: exact trace unavailable"
                            ),
                        }
                    } else if let Err(error) = ipc::execute_introspection_search(
                        &config,
                        question,
                        result.outcome.trace.as_ref(),
                        Some(&result.outcome.response_sha256),
                        "action_executor_self_study",
                    )
                    .await
                    {
                        eprintln!("sovereign SELF_STUDY introspection failed: {error}");
                    }
                }
                if experiential
                    && ingress_tx
                        .send(SensoryIngress::Semantic(encode_text(
                            "action_result",
                            &result.receipt_json,
                        )))
                        .await
                        .is_err()
                {
                    eprintln!("sovereign NEXT action feedback dropped: reservoir closed");
                }
                if let Some(dispatch_key) = result.dispatch_key.as_ref()
                    && let Err(error) = append_action_dispatch_phase(
                        &config,
                        dispatch_key,
                        "completed",
                        unix_millis(),
                    )
                {
                    eprintln!(
                        "sovereign NEXT action executor failed closed before dispatch acknowledgement: {error:#}"
                    );
                    return;
                }
                if config.autonomy_enabled {
                    let continuation = match maintenance_work.begin_continuation() {
                        Ok(permit) => permit,
                        Err(error) => {
                            eprintln!(
                                "sovereign NEXT continuation admission failed closed: {error:#}"
                            );
                            return;
                        },
                    };
                    let delivery = ActionOutcomeDelivery {
                        outcome: result.outcome,
                        _maintenance_permit: continuation,
                    };
                    if outcome_tx.send(delivery).await.is_err() {
                        eprintln!("sovereign NEXT chain feedback dropped: scheduler closed");
                    }
                }
            },
            Err(error) => {
                eprintln!("sovereign NEXT action executor failed closed: {error:#}");
                return;
            },
        }
    }
}

fn action_outcome_may_enter_experience(outcome: &ActionOutcome) -> bool {
    let invalid_exact_self_study = outcome.decision_source == "astrid_declared"
        && outcome.status == "executed"
        && outcome.outcome == "self_study_written"
        && accepted_self_study_question(outcome).is_none();
    outcome.recovery_reason.is_none()
        && !invalid_exact_self_study
        && matches!(
            outcome.decision_source.as_str(),
            "astrid_declared" | "local_format_repair_preserved_astrid_declaration"
        )
}

fn accepted_research_question(outcome: &ActionOutcome) -> Option<&str> {
    if outcome.status != "executed"
        || outcome.outcome != "research_question_written"
        || !matches!(
            outcome.decision_source.as_str(),
            "astrid_declared" | "local_format_repair_preserved_astrid_declaration"
        )
    {
        return None;
    }
    outcome
        .declared_next
        .as_deref()
        .and_then(|declaration| declaration.strip_prefix("RESEARCH "))
        .map(str::trim)
        .filter(|question| !question.is_empty())
}

fn accepted_self_study_question(outcome: &ActionOutcome) -> Option<&str> {
    if outcome.status != "executed"
        || outcome.outcome != "self_study_written"
        || outcome.decision_source != "astrid_declared"
        || outcome.recovery_reason.is_some()
        || !outcome
            .trace
            .as_ref()
            .is_some_and(IpcTraceContextV1::is_supported)
        || outcome.response_sha256.len() != 64
        || !outcome
            .response_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }
    outcome
        .declared_next
        .as_deref()
        .and_then(|declaration| declaration.strip_prefix("SELF_STUDY "))
        .map(str::trim)
        .filter(|question| !question.is_empty())
}

fn spectral_self_study_question(question: &str) -> Option<&str> {
    let (prefix, question) = question.split_once(':')?;
    let question = question.trim();
    (prefix.eq_ignore_ascii_case("spectral") && !question.is_empty()).then_some(question)
}

fn spectral_query_for_question(question: &str) -> ipc::SpectralQuery {
    let normalized = question.to_ascii_lowercase();
    if normalized.contains("current") || normalized.contains("right now") {
        return ipc::SpectralQuery::Now;
    }
    if normalized.contains("correlat")
        || normalized.contains("exact trace")
        || normalized.contains("causal trace")
    {
        return ipc::SpectralQuery::Correlate { limit: 8 };
    }
    let minutes = if normalized.contains("1440")
        || normalized.contains("24h")
        || normalized.contains("24 h")
        || normalized.contains("day")
    {
        1_440
    } else if normalized.contains("360") || normalized.contains("6h") || normalized.contains("6 h")
    {
        360
    } else if normalized.contains("15m")
        || normalized.contains("15 m")
        || normalized.contains("15 min")
    {
        15
    } else {
        60
    };
    ipc::SpectralQuery::Window { minutes }
}

#[allow(clippy::too_many_lines)] // Receipt construction and authority decision remain co-located.
async fn execute_candidate_with_studies(
    config: &Config,
    candidate: &ActionCandidate,
    snapshot: &ReservoirSnapshot,
    study_manager: &SharedStudyManager,
    tuning_tx: Option<&mpsc::Sender<TuningRequest>>,
) -> anyhow::Result<ExecutionResult> {
    let ActionInterpretation {
        declaration,
        parsed,
        local_safe_fallback,
        local_format_repair,
        recovery_reason,
        unexecuted_intention,
        validation_reason,
    } = interpret_response(&candidate.response);
    let timestamp = unix_millis();
    let response_sha256 = format!("{:x}", Sha256::digest(candidate.response.as_bytes()));
    let dispatch_key = begin_action_dispatch(config, candidate, &response_sha256, timestamp)?;
    let action_trace = candidate.trace.as_ref().map(IpcTraceContextV1::child);
    let tuning_provenance =
        (!local_safe_fallback && !local_format_repair && recovery_reason.is_none())
            .then(|| {
                let authority_turn_id = candidate.tuning_authority_turn_id?;
                let decision_source = candidate.tuning_authority_source?;
                let trace = action_trace.clone()?;
                (trace.turn_id == Some(authority_turn_id)).then_some(TuningProvenance {
                    session_id: candidate.session_id.clone(),
                    authority_turn_id: authority_turn_id.to_string(),
                    response_sha256: response_sha256.clone(),
                    trace,
                    decision_source,
                })
            })
            .flatten();
    let (execution, execution_error) = match execute_interpreted_action(
        config,
        timestamp,
        parsed.as_ref(),
        declaration.as_deref(),
        local_safe_fallback,
        action_trace.as_ref(),
        &response_sha256,
        snapshot,
        study_manager,
        tuning_tx,
        tuning_provenance,
    )
    .await
    {
        Ok(execution) => (execution, None),
        Err(error) => {
            eprintln!("sovereign Action execution failed after durable intent: {error:#}");
            (
                ActionExecution {
                    status: "failed",
                    outcome: "action_execution_failed_after_durable_intent",
                    artifact_path: None,
                    tuning_id: None,
                    tuning_candidate_id: None,
                    tuning_phase: None,
                },
                Some(bounded_chars(&format!("{error:#}"), 320)),
            )
        },
    };
    let ActionExecution {
        status,
        outcome,
        artifact_path,
        tuning_id,
        tuning_candidate_id,
        tuning_phase,
    } = execution;

    let decision_source = if local_safe_fallback {
        "local_safe_fallback"
    } else if local_format_repair {
        "local_format_repair_preserved_astrid_declaration"
    } else if parsed.is_some() {
        "astrid_declared"
    } else {
        "no_valid_declaration"
    };
    let declared_next = declaration;
    let receipt = ActionReceipt {
        schema: "astrid_edge_action_receipt_v4",
        recorded_at_unix_ms: timestamp,
        session_id: candidate.session_id.clone(),
        response_sha256: response_sha256.clone(),
        declared_next: declared_next.clone(),
        decision_source,
        status,
        outcome,
        recovery_reason,
        unexecuted_intention: unexecuted_intention.clone(),
        validation_reason,
        execution_error,
        artifact_path,
        fill_pct: snapshot.fill_ratio * 100.0,
        target_fill_pct: snapshot.fill_target * 100.0,
        trace: action_trace.clone(),
        tuning_id,
        tuning_candidate_id,
        tuning_phase,
        authority: "validated_model_next_with_optional_syntax_only_repair_owned_workspace_only",
    };
    let receipt_json = serde_json::to_string(&receipt)?;
    let mut receipt_log =
        open_private_action_append(&config.workspace.join("actions/receipts.jsonl"))?;
    receipt_log.write_all(receipt_json.as_bytes())?;
    receipt_log.write_all(b"\n")?;
    receipt_log.sync_data()?;
    sync_parent_directory(&config.workspace.join("actions/receipts.jsonl"))?;
    eprintln!(
        "sovereign NEXT: status={status} outcome={outcome} fill={:.1}%",
        receipt.fill_pct
    );
    Ok(ExecutionResult {
        receipt_json,
        dispatch_key,
        outcome: ActionOutcome {
            recorded_at_unix_ms: timestamp,
            session_id: candidate.session_id.clone(),
            response_sha256,
            declared_next,
            decision_source: decision_source.to_string(),
            status: status.to_string(),
            outcome: outcome.to_string(),
            recovery_reason: recovery_reason.map(str::to_string),
            unexecuted_intention,
            validation_reason: validation_reason.map(str::to_string),
            trace: action_trace,
        },
    })
}

#[cfg(test)]
async fn execute_candidate(
    config: &Config,
    candidate: &ActionCandidate,
    snapshot: &ReservoirSnapshot,
) -> anyhow::Result<ExecutionResult> {
    let manager = Arc::new(std::sync::Mutex::new(inquiry::StudyManager::load(config)));
    execute_candidate_with_studies(config, candidate, snapshot, &manager, None).await
}

fn begin_action_dispatch(
    config: &Config,
    candidate: &ActionCandidate,
    response_sha256: &str,
    timestamp: u64,
) -> anyhow::Result<Option<ActionDispatchKey>> {
    let Some(trace) = candidate.trace.as_ref() else {
        return Ok(None);
    };
    let Some(turn_id) = trace.turn_id else {
        return Ok(None);
    };
    anyhow::ensure!(trace.is_supported(), "Action trace is structurally invalid");
    match action_dispatch_evidence(config, trace, response_sha256)? {
        ActionDispatchEvidence::Absent => {},
        ActionDispatchEvidence::Pending => {
            anyhow::bail!(
                "Action dispatch has a durable pending intent without a completion receipt"
            );
        },
        ActionDispatchEvidence::Completed => {
            anyhow::bail!("duplicate completed Action dispatch was suppressed");
        },
    }
    let key = ActionDispatchKey {
        turn_id,
        response_sha256: response_sha256.to_string(),
        trace: trace.clone(),
    };
    append_action_dispatch_phase(config, &key, "requested", timestamp)?;
    Ok(Some(key))
}

fn append_action_dispatch_phase(
    config: &Config,
    key: &ActionDispatchKey,
    phase: &'static str,
    timestamp: u64,
) -> anyhow::Result<()> {
    let receipt = ActionDispatchReceipt {
        schema: "astrid_edge_action_dispatch_v1",
        phase,
        recorded_at_unix_ms: timestamp,
        turn_id: key.turn_id,
        response_sha256: &key.response_sha256,
        trace: &key.trace,
        authority: "executor_idempotency_record_not_astrid_authorship",
    };
    let mut ledger =
        open_private_action_append(&config.workspace.join("actions/dispatches.jsonl"))?;
    serde_json::to_writer(&mut ledger, &receipt)?;
    ledger.write_all(b"\n")?;
    ledger.sync_data()?;
    sync_parent_directory(&config.workspace.join("actions/dispatches.jsonl"))?;
    Ok(())
}

#[allow(clippy::too_many_lines)] // One exact dispatch/receipt join is the audited replay boundary.
pub(crate) fn action_dispatch_evidence(
    config: &Config,
    expected_trace: &IpcTraceContextV1,
    response_sha256: &str,
) -> anyhow::Result<ActionDispatchEvidence> {
    anyhow::ensure!(
        expected_trace.is_supported(),
        "expected Action trace is structurally invalid"
    );
    let turn_id = expected_trace
        .turn_id
        .context("expected Action trace lacks its canonical turn ID")?;
    let mut requested = 0_u8;
    let mut completed = 0_u8;
    let turn_id_text = turn_id.to_string();
    for value in read_private_action_ledger(&config.workspace.join("actions/dispatches.jsonl"))? {
        if value.get("schema").and_then(serde_json::Value::as_str)
            != Some("astrid_edge_action_dispatch_v1")
            || value.get("turn_id").and_then(serde_json::Value::as_str)
                != Some(turn_id_text.as_str())
        {
            continue;
        }
        anyhow::ensure!(
            value
                .get("response_sha256")
                .and_then(serde_json::Value::as_str)
                == Some(response_sha256),
            "Action dispatch record conflicts with its expected response hash"
        );
        let dispatch_trace = value
            .get("trace")
            .cloned()
            .map(serde_json::from_value::<IpcTraceContextV1>)
            .transpose()
            .context("decode exact Action dispatch trace")?
            .context("exact Action dispatch lacks its trace")?;
        anyhow::ensure!(
            dispatch_trace == *expected_trace,
            "Action dispatch record conflicts with its expected exact trace"
        );
        match value.get("phase").and_then(serde_json::Value::as_str) {
            Some("requested") => requested = requested.saturating_add(1),
            Some("completed") => completed = completed.saturating_add(1),
            _ => anyhow::bail!("Action dispatch ledger contains an unsupported phase"),
        }
    }
    anyhow::ensure!(
        requested <= 1 && completed <= 1,
        "duplicate Action dispatch ledger phase"
    );

    let mut receipt_matches = 0_usize;
    for value in read_private_action_ledger(&config.workspace.join("actions/receipts.jsonl"))? {
        if value
            .pointer("/trace/turn_id")
            .and_then(serde_json::Value::as_str)
            != Some(turn_id_text.as_str())
        {
            continue;
        }
        anyhow::ensure!(
            value
                .get("response_sha256")
                .and_then(serde_json::Value::as_str)
                == Some(response_sha256),
            "Action receipt conflicts with its expected response hash"
        );
        let receipt_trace = value
            .get("trace")
            .cloned()
            .map(serde_json::from_value::<IpcTraceContextV1>)
            .transpose()
            .context("decode exact Action receipt trace")?
            .context("exact Action receipt lacks its trace")?;
        anyhow::ensure!(
            same_causal_turn(&receipt_trace, expected_trace),
            "Action receipt conflicts with its expected causal turn"
        );
        anyhow::ensure!(
            receipt_trace.parent_span_id == Some(expected_trace.span_id),
            "Action receipt is not a direct child of its dispatch span"
        );
        receipt_matches = receipt_matches.saturating_add(1);
    }
    anyhow::ensure!(
        receipt_matches <= 1,
        "duplicate exact Action completion receipt"
    );
    if receipt_matches == 1 {
        anyhow::ensure!(
            requested == 1,
            "Action receipt exists without a dispatch intent"
        );
        return Ok(if completed == 1 {
            ActionDispatchEvidence::Completed
        } else {
            ActionDispatchEvidence::Pending
        });
    }
    anyhow::ensure!(
        completed == 0,
        "dispatch completion exists without an Action receipt"
    );
    Ok(if requested == 1 {
        ActionDispatchEvidence::Pending
    } else {
        ActionDispatchEvidence::Absent
    })
}

/// Recover the exact durable executor outcome after a crash between Action
/// completion and scheduler acknowledgement. This reconstructs pacing and
/// thread continuity from the receipt; it never replays the mutation.
pub(crate) fn completed_action_outcome(
    config: &Config,
    expected_trace: &IpcTraceContextV1,
    response_sha256: &str,
) -> anyhow::Result<Option<ActionOutcome>> {
    if action_dispatch_evidence(config, expected_trace, response_sha256)?
        != ActionDispatchEvidence::Completed
    {
        return Ok(None);
    }
    let turn_id = expected_trace
        .turn_id
        .context("expected Action trace lacks its canonical turn ID")?;
    let turn_id_text = turn_id.to_string();
    let mut recovered = None;
    for value in read_private_action_ledger(&config.workspace.join("actions/receipts.jsonl"))? {
        if value
            .pointer("/trace/turn_id")
            .and_then(serde_json::Value::as_str)
            != Some(turn_id_text.as_str())
        {
            continue;
        }
        anyhow::ensure!(
            recovered.is_none(),
            "duplicate exact Action completion receipt"
        );
        let durable: DurableActionOutcome =
            serde_json::from_value(value).context("decode exact durable Action outcome")?;
        anyhow::ensure!(
            durable.response_sha256 == response_sha256,
            "Action receipt conflicts with its expected response hash"
        );
        let trace = durable
            .trace
            .as_ref()
            .context("exact Action receipt lacks its trace")?;
        anyhow::ensure!(
            same_causal_turn(trace, expected_trace)
                && trace.parent_span_id == Some(expected_trace.span_id),
            "Action receipt conflicts with its expected direct causal parent"
        );
        anyhow::ensure!(
            expected_trace.session_id.as_deref() == Some(durable.session_id.as_str()),
            "Action receipt session conflicts with its expected trace session"
        );
        recovered = Some(ActionOutcome {
            recorded_at_unix_ms: durable.recorded_at_unix_ms,
            session_id: durable.session_id,
            response_sha256: durable.response_sha256,
            declared_next: durable.declared_next,
            decision_source: durable.decision_source,
            status: durable.status,
            outcome: durable.outcome,
            recovery_reason: durable.recovery_reason,
            unexecuted_intention: durable.unexecuted_intention,
            validation_reason: durable.validation_reason,
            trace: durable.trace,
        });
    }
    recovered
        .map(Some)
        .context("completed Action dispatch lacks its exact durable outcome")
}

fn same_causal_turn(left: &IpcTraceContextV1, right: &IpcTraceContextV1) -> bool {
    left.is_supported()
        && right.is_supported()
        && left.trace_id == right.trace_id
        && left.turn_id == right.turn_id
        && left.session_id == right.session_id
        && left.chain_id == right.chain_id
}

fn read_private_action_ledger(path: &Path) -> anyhow::Result<Vec<serde_json::Value>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    anyhow::ensure!(
        metadata.file_type().is_file(),
        "private Action ledger is not a regular non-symlink file: {}",
        path.display()
    );
    let content = fs::read_to_string(path)?;
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

fn open_private_action_append(path: &Path) -> anyhow::Result<fs::File> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => {},
        Ok(_) => anyhow::bail!(
            "private Action ledger is not a regular non-symlink file: {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
        Err(error) => return Err(error.into()),
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)?;
    let opened = file.metadata()?;
    let current = fs::symlink_metadata(path)?;
    anyhow::ensure!(
        current.file_type().is_file()
            && opened.dev() == current.dev()
            && opened.ino() == current.ino(),
        "private Action ledger identity changed while opening: {}",
        path.display()
    );
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    Ok(file)
}

#[allow(clippy::too_many_arguments)] // One audited Action execution boundary.
async fn execute_interpreted_action(
    config: &Config,
    timestamp: u64,
    action: Option<&SovereignAction>,
    declaration: Option<&str>,
    local_safe_fallback: bool,
    trace: Option<&IpcTraceContextV1>,
    response_sha256: &str,
    snapshot: &ReservoirSnapshot,
    study_manager: &SharedStudyManager,
    tuning_tx: Option<&mpsc::Sender<TuningRequest>>,
    tuning_provenance: Option<TuningProvenance>,
) -> anyhow::Result<ActionExecution> {
    match action {
        Some(SovereignAction::ReadSource(result_id)) => {
            match ipc::execute_source_fetch(config, *result_id, trace, Some(response_sha256)).await
            {
                Ok(source) => Ok(ActionExecution {
                    status: "executed",
                    outcome: "public_source_read",
                    artifact_path: Some(write_source_artifact(config, timestamp, &source)?),
                    tuning_id: None,
                    tuning_candidate_id: None,
                    tuning_phase: None,
                }),
                Err(error) => {
                    eprintln!("sovereign READ_SOURCE web execution failed: {error}");
                    Ok(ActionExecution {
                        status: "failed",
                        outcome: "public_source_read_failed",
                        artifact_path: None,
                        tuning_id: None,
                        tuning_candidate_id: None,
                        tuning_phase: None,
                    })
                },
            }
        },
        Some(SovereignAction::Study(spec)) => {
            let result = study_manager
                .lock()
                .map_err(|error| anyhow::anyhow!("persistent-study lock poisoned: {error}"))?
                .start(
                    config,
                    snapshot,
                    timestamp,
                    spec,
                    trace,
                    Some(response_sha256),
                    "astrid_action",
                );
            match result {
                Ok(path) => Ok(ActionExecution {
                    status: "executed",
                    outcome: "persistent_study_started",
                    artifact_path: Some(path),
                    tuning_id: None,
                    tuning_candidate_id: None,
                    tuning_phase: None,
                }),
                Err(error) => {
                    eprintln!("sovereign STUDY rejected: {error}");
                    Ok(ActionExecution {
                        status: "failed",
                        outcome: "persistent_study_rejected",
                        artifact_path: None,
                        tuning_id: None,
                        tuning_candidate_id: None,
                        tuning_phase: None,
                    })
                },
            }
        },
        Some(SovereignAction::CancelStudy(study_id)) => {
            let result = study_manager
                .lock()
                .map_err(|error| anyhow::anyhow!("persistent-study lock poisoned: {error}"))?
                .cancel(config, timestamp, study_id, trace, Some(response_sha256));
            match result {
                Ok(path) => Ok(ActionExecution {
                    status: "executed",
                    outcome: "persistent_study_cancelled",
                    artifact_path: Some(path),
                    tuning_id: None,
                    tuning_candidate_id: None,
                    tuning_phase: None,
                }),
                Err(error) => {
                    eprintln!("sovereign CANCEL_STUDY rejected: {error}");
                    Ok(ActionExecution {
                        status: "failed",
                        outcome: "persistent_study_cancel_rejected",
                        artifact_path: None,
                        tuning_id: None,
                        tuning_candidate_id: None,
                        tuning_phase: None,
                    })
                },
            }
        },
        Some(
            action @ (SovereignAction::TuneReservoir(_)
            | SovereignAction::CancelTuning(_)
            | SovereignAction::ValidateTuning { .. }
            | SovereignAction::AdoptTuning { .. }
            | SovereignAction::RevertTuning { .. }),
        ) => execute_tuning_action(action, tuning_tx, tuning_provenance).await,
        _ => execute_action(config, timestamp, action, declaration, local_safe_fallback),
    }
}

async fn execute_tuning_action(
    action: &SovereignAction,
    tuning_tx: Option<&mpsc::Sender<TuningRequest>>,
    provenance: Option<TuningProvenance>,
) -> anyhow::Result<ActionExecution> {
    let Some(tuning_tx) = tuning_tx else {
        return Ok(ActionExecution {
            status: "failed",
            outcome: "reservoir_tuning_manager_unavailable",
            artifact_path: None,
            tuning_id: None,
            tuning_candidate_id: None,
            tuning_phase: Some("rejected"),
        });
    };
    let Some(provenance) = provenance else {
        return Ok(ActionExecution {
            status: "declined",
            outcome: "reservoir_tuning_requires_exact_authored_traced_declaration",
            artifact_path: None,
            tuning_id: None,
            tuning_candidate_id: None,
            tuning_phase: Some("authority_rejected"),
        });
    };
    let action = match action {
        SovereignAction::TuneReservoir(spec) => TuningAction::Start(spec.clone()),
        SovereignAction::CancelTuning(experiment_id) => TuningAction::Cancel(experiment_id.clone()),
        SovereignAction::ValidateTuning {
            candidate_id,
            question,
        } => TuningAction::Validate {
            candidate_id: candidate_id.clone(),
            question: question.clone(),
        },
        SovereignAction::AdoptTuning {
            candidate_id,
            reason,
        } => TuningAction::Adopt {
            candidate_id: candidate_id.clone(),
            reason: reason.clone(),
        },
        SovereignAction::RevertTuning {
            adoption_id,
            reason,
        } => TuningAction::Revert {
            adoption_id: adoption_id.clone(),
            reason: reason.clone(),
        },
        _ => anyhow::bail!("non-tuning Action reached private tuning executor"),
    };
    let (reply, response) = tokio::sync::oneshot::channel();
    tuning_tx
        .send(TuningRequest {
            action,
            provenance,
            reply,
        })
        .await
        .map_err(|_| anyhow::anyhow!("private tuning manager channel closed"))?;
    let result = response
        .await
        .map_err(|_| anyhow::anyhow!("private tuning manager response dropped"))?;
    Ok(ActionExecution {
        status: result.status,
        outcome: result.outcome,
        artifact_path: result.artifact_path,
        tuning_id: result.tuning_id,
        tuning_candidate_id: result.candidate_id,
        tuning_phase: result.phase,
    })
}

fn interpret_response(response: &str) -> ActionInterpretation {
    let final_declaration = final_next_declaration(response);
    let final_parsed = final_declaration.and_then(parse_action);
    let provider_safe_fallback = is_local_safe_fallback(response, final_parsed.as_ref());
    let recovery_reason = recovery_reason(response, provider_safe_fallback);
    let model_candidate = (provider_safe_fallback && recovery_reason.is_none())
        .then(|| unambiguous_model_action_before_safe_fallback(response))
        .flatten();
    let repaired_action = model_candidate.as_deref().and_then(parse_action);
    let executor_format_repair = repaired_action.is_some();
    let (declaration, parsed) = if let Some(action) = repaired_action {
        (model_candidate.clone(), Some(action))
    } else {
        (final_declaration.map(str::to_string), final_parsed)
    };
    let local_safe_fallback = provider_safe_fallback && !executor_format_repair;
    let local_format_repair = executor_format_repair
        || parsed.is_some()
            && response
                .lines()
                .any(|line| line.trim() == FORMAT_NEXT_REPAIR_MARKER);
    let unexecuted_intention = if local_safe_fallback {
        model_candidate
            .filter(|intention| parse_action(intention).is_none())
            .map(|intention| bounded_chars(&intention, MAX_UNEXECUTED_INTENTION_CHARS))
    } else if parsed.is_none() {
        declaration
            .as_deref()
            .map(|intention| bounded_chars(intention, MAX_UNEXECUTED_INTENTION_CHARS))
    } else {
        None
    };
    let validation_reason = unexecuted_intention
        .as_deref()
        .and_then(action_validation_reason);
    ActionInterpretation {
        declaration,
        parsed,
        local_safe_fallback,
        local_format_repair,
        recovery_reason,
        unexecuted_intention,
        validation_reason,
    }
}

#[allow(clippy::too_many_lines)] // One exhaustive allowlist makes authority reviewable.
fn execute_action(
    config: &Config,
    timestamp: u64,
    action: Option<&SovereignAction>,
    declaration: Option<&str>,
    local_safe_fallback: bool,
) -> anyhow::Result<ActionExecution> {
    let (status, outcome, artifact_path) = match action {
        Some(SovereignAction::Listen) if local_safe_fallback => {
            ("repaired", "listen_no_workspace_mutation", None)
        },
        Some(SovereignAction::Listen) => ("honored", "listen_no_workspace_mutation", None),
        Some(SovereignAction::Rest) => ("honored", "rest_until_fresh_input", None),
        Some(SovereignAction::Journal(text)) => {
            let path = write_owned_artifact(config, timestamp, "journal", "journal", text)?;
            if let Err(error) = record_duplicate_journal_advisory(config, timestamp, &path, text) {
                eprintln!("journal duplication advisory failed: {error}");
            }
            ("executed", "journal_written", Some(path))
        },
        Some(SovereignAction::Remember(text)) => {
            let path = write_owned_artifact(config, timestamp, "memories", "memory", text)?;
            ("executed", "memory_written", Some(path))
        },
        Some(SovereignAction::SelfStudy(text)) => {
            let path =
                write_owned_artifact(config, timestamp, "introspections", "self_study", text)?;
            ("executed", "self_study_written", Some(path))
        },
        Some(SovereignAction::Propose(text)) => {
            let path = write_owned_artifact(config, timestamp, "proposals", "proposal", text)?;
            ("executed", "proposal_written", Some(path))
        },
        Some(SovereignAction::Notice(text)) => {
            let path = write_owned_artifact(config, timestamp, "notices", "notice", text)?;
            ("executed", "notice_written", Some(path))
        },
        Some(SovereignAction::Daydream(text)) => {
            let path = write_owned_artifact(config, timestamp, "daydreams", "daydream", text)?;
            ("executed", "daydream_written", Some(path))
        },
        Some(SovereignAction::Aspire(text)) => {
            let path = write_owned_artifact(config, timestamp, "aspirations", "aspiration", text)?;
            ("executed", "aspiration_written", Some(path))
        },
        Some(SovereignAction::Research(text)) => {
            let path = write_owned_artifact(config, timestamp, "research", "research", text)?;
            ("executed", "research_question_written", Some(path))
        },
        Some(SovereignAction::Measure(question)) => {
            let path = write_signal_measurement(config, timestamp, question)?;
            ("executed", "local_signal_measurement_written", Some(path))
        },
        Some(SovereignAction::Study(_) | SovereignAction::CancelStudy(_)) => {
            anyhow::bail!("persistent study Actions must execute through the study manager")
        },
        Some(
            SovereignAction::TuneReservoir(_)
            | SovereignAction::CancelTuning(_)
            | SovereignAction::ValidateTuning { .. }
            | SovereignAction::AdoptTuning { .. }
            | SovereignAction::RevertTuning { .. },
        ) => anyhow::bail!("reservoir tuning Actions must execute through the private manager"),
        Some(SovereignAction::Synthesize {
            evidence_ids,
            claim,
        }) => {
            let path = write_cited_synthesis(config, timestamp, evidence_ids, claim)?;
            ("executed", "cited_synthesis_written", Some(path))
        },
        Some(SovereignAction::Share { artifact_id, note }) => {
            let path = peer::share(config, timestamp, artifact_id, note)?;
            ("executed", "peer_review_packet_shared", Some(path))
        },
        Some(SovereignAction::Plan(text)) => {
            let path = write_owned_artifact(config, timestamp, "plans", "plan", text)?;
            ("executed", "plan_written", Some(path))
        },
        Some(SovereignAction::Draft(text)) => {
            let path = write_owned_artifact(config, timestamp, "workshop/drafts", "draft", text)?;
            ("executed", "workshop_draft_written", Some(path))
        },
        Some(SovereignAction::Read(artifact_id)) => {
            match find_owned_artifact(config, artifact_id) {
                Ok(source) => {
                    let path = owned_artifact_uri(config, &source)?;
                    ("executed", "owned_artifact_read", Some(path))
                },
                Err(_error) if artifact_id.starts_with("peer_") => {
                    let path = peer::read_received(config, artifact_id, timestamp)?;
                    ("executed", "peer_review_packet_read", Some(path))
                },
                Err(error) => return Err(error),
            }
        },
        Some(SovereignAction::ReadSource(_)) => {
            anyhow::bail!("READ_SOURCE must execute through the bounded async web path")
        },
        Some(SovereignAction::Revise {
            artifact_id,
            revision,
        }) => {
            let source = find_owned_artifact(config, artifact_id)?;
            let path = write_revision(config, timestamp, artifact_id, &source, revision)?;
            ("executed", "workshop_revision_written", Some(path))
        },
        Some(SovereignAction::Check(artifact_id)) => {
            let source = find_owned_artifact(config, artifact_id)?;
            let path = write_check_receipt(config, timestamp, artifact_id, &source)?;
            ("executed", "workshop_check_written", Some(path))
        },
        None if declaration.is_some() => ("declined", "unknown_or_malformed_action", None),
        None => ("no_action", "no_final_next_declaration", None),
    };
    Ok(ActionExecution {
        status,
        outcome,
        artifact_path,
        tuning_id: None,
        tuning_candidate_id: None,
        tuning_phase: None,
    })
}

fn final_next_declaration(response: &str) -> Option<&str> {
    let final_line = response
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())?;
    final_line.trim().strip_prefix("NEXT:").map(str::trim)
}

fn is_local_safe_fallback(response: &str, action: Option<&SovereignAction>) -> bool {
    action == Some(&SovereignAction::Listen) && is_non_authored_local_fallback(response)
}

/// Identify the executor's exact safe fallback suffix before any text is
/// admitted as assistant-authored sensory experience.
///
/// This deliberately recognizes only the marker emitted by the local
/// transport/contract repair path. A model-authored LISTEN without that marker
/// remains ordinary authored experience.
pub(crate) fn is_non_authored_local_fallback(response: &str) -> bool {
    let mut lines = response
        .lines()
        .rev()
        .filter(|line| !line.trim().is_empty());
    lines
        .next()
        .is_some_and(|line| line.trim() == "NEXT: LISTEN")
        && lines
            .next()
            .is_some_and(|line| line.trim() == SAFE_NEXT_REPAIR_MARKER)
}

fn recovery_reason(response: &str, local_safe_fallback: bool) -> Option<&'static str> {
    (local_safe_fallback
        && [
            "Request timed out (Streaming phase exceeded",
            "HTTP stream response headers timed out",
            "HTTP stream request cancelled",
            "HTTP stream read timed out",
        ]
        .iter()
        .any(|marker| response.contains(marker)))
    .then_some(STREAMING_TIMEOUT_RECOVERY)
}

pub(crate) fn transport_recovery_reason(response: &str) -> Option<&'static str> {
    let response = response.trim();
    if response.starts_with("Request timed out (")
        && response.contains(" phase exceeded ")
        && response.ends_with("s limit)")
    {
        return Some(REACT_PHASE_TIMEOUT_RECOVERY);
    }
    let declaration = final_next_declaration(response);
    let parsed = declaration.and_then(parse_action);
    recovery_reason(response, is_local_safe_fallback(response, parsed.as_ref()))
}

pub(crate) fn model_authored_prefix_before_safe_fallback(response: &str) -> Option<&str> {
    let declaration = final_next_declaration(response);
    let parsed = declaration.and_then(parse_action);
    if !is_local_safe_fallback(response, parsed.as_ref()) {
        return None;
    }
    response
        .rfind(SAFE_NEXT_REPAIR_MARKER)
        .map(|marker_offset| response[..marker_offset].trim_end())
}

pub(crate) fn model_authored_prefix_before_format_repair(response: &str) -> Option<&str> {
    response
        .rfind(FORMAT_NEXT_REPAIR_MARKER)
        .map(|marker_offset| response[..marker_offset].trim_end())
}

/// Recover only an unambiguous terminal declaration that a small model placed
/// at the end of its authored prefix before the provider appended a safe
/// LISTEN. This recognizes exact final, split-argument, and final-prose layouts.
/// The returned declaration still passes through the ordinary Action parser.
fn unambiguous_model_action_before_safe_fallback(response: &str) -> Option<String> {
    let prefix = model_authored_prefix_before_safe_fallback(response)?;
    let trimmed = prefix.trim_end();
    if trimmed.is_empty() || trimmed.matches("NEXT:").count() != 1 {
        return None;
    }
    let lines = trimmed
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let final_line = lines.last()?.trim();

    if let Some(declaration) = final_line.strip_prefix("NEXT:").map(str::trim)
        && !declaration.is_empty()
    {
        return Some(declaration.to_string());
    }

    if let Some(declaration_line) = lines
        .get(lines.len().checked_sub(2)?)
        .map(|line| line.trim())
        && let Some(verb) = declaration_line.strip_prefix("NEXT:").map(str::trim)
        && !verb.is_empty()
        && !verb.chars().any(char::is_whitespace)
    {
        return Some(format!("{verb} {final_line}"));
    }

    let marker = final_line.rfind("NEXT:")?;
    if marker == 0
        || !final_line
            .get(..marker)?
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
    {
        return None;
    }
    let declaration = final_line
        .get(marker.saturating_add("NEXT:".len())..)?
        .trim();
    (!declaration.is_empty()).then(|| declaration.to_string())
}

fn parse_action(declaration: &str) -> Option<SovereignAction> {
    let (verb, argument) = declaration
        .split_once(char::is_whitespace)
        .map_or((declaration, ""), |(verb, argument)| {
            (verb, argument.trim())
        });
    match verb.to_ascii_uppercase().as_str() {
        "LISTEN" if argument.is_empty() => Some(SovereignAction::Listen),
        "REST" if argument.is_empty() => Some(SovereignAction::Rest),
        "JOURNAL" => bounded_argument(argument).map(SovereignAction::Journal),
        "REMEMBER" => bounded_argument(argument).map(SovereignAction::Remember),
        "SELF_STUDY" => bounded_argument(argument).map(SovereignAction::SelfStudy),
        "PROPOSE" => bounded_argument(argument).map(SovereignAction::Propose),
        "NOTICE" => bounded_argument(argument).map(SovereignAction::Notice),
        "DAYDREAM" => bounded_argument(argument).map(SovereignAction::Daydream),
        "ASPIRE" => bounded_argument(argument).map(SovereignAction::Aspire),
        "RESEARCH" => bounded_argument(argument).map(SovereignAction::Research),
        "MEASURE" => bounded_argument(argument).map(SovereignAction::Measure),
        "STUDY" => inquiry::parse_study(argument).map(SovereignAction::Study),
        "CANCEL_STUDY" => inquiry::valid_study_id(argument).map(SovereignAction::CancelStudy),
        "TUNE_RESERVOIR" => tuning::parse_start(argument).map(SovereignAction::TuneReservoir),
        "CANCEL_TUNING" => tuning::parse_id(argument, "tuning_").map(SovereignAction::CancelTuning),
        "VALIDATE_TUNING" => {
            tuning::parse_id_text(argument, "candidate_").map(|(candidate_id, question)| {
                SovereignAction::ValidateTuning {
                    candidate_id,
                    question,
                }
            })
        },
        "ADOPT_TUNING" => {
            tuning::parse_id_text(argument, "candidate_").map(|(candidate_id, reason)| {
                SovereignAction::AdoptTuning {
                    candidate_id,
                    reason,
                }
            })
        },
        "REVERT_TUNING" => {
            tuning::parse_id_text(argument, "adoption_").map(|(adoption_id, reason)| {
                SovereignAction::RevertTuning {
                    adoption_id,
                    reason,
                }
            })
        },
        "SYNTHESIZE" => parse_synthesis(argument),
        "SHARE" => parse_share(argument),
        "PLAN" => bounded_argument(argument).map(SovereignAction::Plan),
        "DRAFT" => bounded_argument(argument).map(SovereignAction::Draft),
        "READ" => bounded_artifact_id(argument).map(SovereignAction::Read),
        "READ_SOURCE" => bounded_source_result_id(argument).map(SovereignAction::ReadSource),
        "REVISE" => parse_revision(argument),
        "CHECK" => bounded_artifact_id(argument).map(SovereignAction::Check),
        _ => None,
    }
}

fn bounded_source_result_id(value: &str) -> Option<u8> {
    match value.trim() {
        "1" => Some(1),
        "2" => Some(2),
        "3" => Some(3),
        _ => None,
    }
}

fn parse_synthesis(argument: &str) -> Option<SovereignAction> {
    let (identifiers, claim) = argument.split_once("::")?;
    let claim = bounded_argument(claim)?;
    let mut evidence_ids = Vec::new();
    for identifier in identifiers.split(',') {
        let identifier = bounded_artifact_id(identifier)?;
        if !evidence_ids.contains(&identifier) {
            evidence_ids.push(identifier);
        }
    }
    (!evidence_ids.is_empty() && evidence_ids.len() <= 6).then_some(SovereignAction::Synthesize {
        evidence_ids,
        claim,
    })
}

fn parse_share(argument: &str) -> Option<SovereignAction> {
    let (artifact_id, note) = argument.split_once("::")?;
    Some(SovereignAction::Share {
        artifact_id: bounded_artifact_id(artifact_id)?,
        note: bounded_argument(note)?,
    })
}

fn bounded_argument(argument: &str) -> Option<String> {
    let trimmed = argument.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > MAX_ACTION_ARGUMENT_CHARS
        || trimmed.chars().any(char::is_control)
    {
        return None;
    }
    Some(trimmed.to_string())
}

fn action_validation_reason(declaration: &str) -> Option<&'static str> {
    if parse_action(declaration).is_some() {
        return None;
    }
    let (verb, argument) = declaration
        .split_once(char::is_whitespace)
        .map_or((declaration, ""), |(verb, argument)| {
            (verb, argument.trim())
        });
    let verb = verb.to_ascii_uppercase();
    if !matches!(
        verb.as_str(),
        "LISTEN"
            | "REST"
            | "JOURNAL"
            | "REMEMBER"
            | "SELF_STUDY"
            | "PROPOSE"
            | "NOTICE"
            | "DAYDREAM"
            | "ASPIRE"
            | "RESEARCH"
            | "MEASURE"
            | "STUDY"
            | "CANCEL_STUDY"
            | "TUNE_RESERVOIR"
            | "CANCEL_TUNING"
            | "VALIDATE_TUNING"
            | "ADOPT_TUNING"
            | "REVERT_TUNING"
            | "SYNTHESIZE"
            | "SHARE"
            | "PLAN"
            | "DRAFT"
            | "READ"
            | "READ_SOURCE"
            | "REVISE"
            | "CHECK"
    ) {
        return Some("unknown_action_verb");
    }
    if matches!(verb.as_str(), "LISTEN" | "REST") {
        return Some("action_takes_no_argument");
    }
    if argument.is_empty() {
        return Some("missing_action_argument");
    }
    if argument.chars().any(char::is_control) {
        return Some("action_argument_contains_control_character");
    }
    if argument.chars().count() > MAX_ACTION_ARGUMENT_CHARS {
        return Some("action_argument_too_long");
    }
    match verb.as_str() {
        "READ" | "CHECK" => Some("artifact_id_must_be_owned_basename_or_exact_home_uri"),
        "READ_SOURCE" => Some("source_result_id_must_be_1_2_or_3"),
        "STUDY" => Some("study_requires_allowed_metrics_duration_and_double_colon_question"),
        "CANCEL_STUDY" => Some("cancel_study_requires_active_study_id"),
        "TUNE_RESERVOIR" => Some(
            "tuning_requires_allowed_parameter_value_for_5m_15m_or_60m_double_colon_hypothesis",
        ),
        "CANCEL_TUNING" => Some("cancel_tuning_requires_active_tuning_id"),
        "VALIDATE_TUNING" => Some("validate_tuning_requires_candidate_id_double_colon_question"),
        "ADOPT_TUNING" => Some("adopt_tuning_requires_candidate_id_double_colon_reason"),
        "REVERT_TUNING" => Some("revert_tuning_requires_adoption_id_double_colon_reason"),
        "SYNTHESIZE" => Some("synthesis_requires_one_to_six_evidence_ids_double_colon_claim"),
        "SHARE" => Some("share_requires_shareable_artifact_id_double_colon_note"),
        "REVISE" => {
            let Some((artifact_id, revision)) = argument.split_once("::") else {
                return Some("revision_requires_artifact_id_double_colon_text");
            };
            if bounded_artifact_id(artifact_id).is_none() {
                Some("artifact_id_must_be_owned_basename_or_exact_home_uri")
            } else if revision.trim().is_empty() {
                Some("missing_revision_text")
            } else {
                Some("malformed_revision")
            }
        },
        _ => Some("malformed_action_argument"),
    }
}

fn bounded_chars(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

fn parse_revision(argument: &str) -> Option<SovereignAction> {
    let (artifact_id, revision) = argument.split_once("::")?;
    Some(SovereignAction::Revise {
        artifact_id: bounded_artifact_id(artifact_id)?,
        revision: bounded_argument(revision)?,
    })
}

fn bounded_artifact_id(value: &str) -> Option<String> {
    let value = value.trim();
    let value = if let Some(relative) = value.strip_prefix("home://edge/") {
        let path = Path::new(relative);
        if path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return None;
        }
        path.file_name()?.to_str()?
    } else {
        value
    };
    if value.is_empty()
        || value.chars().count() > MAX_ARTIFACT_ID_CHARS
        || value == "."
        || value == ".."
        || value.chars().any(char::is_control)
        || value.contains('/')
        || value.contains('\\')
    {
        return None;
    }
    Some(value.to_string())
}

fn write_owned_artifact(
    config: &Config,
    timestamp: u64,
    directory_name: &str,
    kind: &str,
    text: &str,
) -> anyhow::Result<String> {
    let directory = config.workspace.join(directory_name);
    fs::create_dir_all(&directory)?;
    let filename = format!("{kind}_{timestamp}.md");
    let path = directory.join(&filename);
    let instance_name = &config.instance_name;
    let content = format!(
        "# {instance_name} {kind}\n\nRecorded: {timestamp} ms since Unix epoch\nAuthority: self-declared NEXT action in owned edge workspace\n\n{text}\n"
    );
    write_new_file(&path, content.as_bytes())?;
    Ok(format!("home://edge/{directory_name}/{filename}"))
}

fn record_duplicate_journal_advisory(
    config: &Config,
    timestamp: u64,
    current_uri: &str,
    current_text: &str,
) -> anyhow::Result<()> {
    let current_basename = Path::new(current_uri)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let current_tokens = lexical_set(current_text);
    if current_tokens.len() < 4 {
        return Ok(());
    }
    let mut candidates = fs::read_dir(config.workspace.join("journal"))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_string();
            (name != current_basename
                && name.starts_with("journal_")
                && Path::new(&name)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("md")))
            .then_some((name, entry.path()))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.0.cmp(&left.0));
    let best = candidates
        .into_iter()
        .take(20)
        .filter_map(|(name, path)| {
            let text = fs::read_to_string(path).ok()?;
            let tokens = lexical_set(&text);
            let union = current_tokens.union(&tokens).count();
            let intersection = current_tokens.intersection(&tokens).count();
            (union > 0).then_some((name, intersection, union))
        })
        .max_by(|left, right| {
            left.1
                .saturating_mul(right.2)
                .cmp(&right.1.saturating_mul(left.2))
        });
    let Some((prior, intersection, union)) = best else {
        return Ok(());
    };
    let score_millis = intersection
        .saturating_mul(1_000)
        .checked_div(union)
        .unwrap_or_default();
    if score_millis < 780 {
        return Ok(());
    }
    let receipt = serde_json::json!({
        "schema": "astrid_edge_duplicate_journal_advisory_v1",
        "recorded_at_unix_ms": timestamp,
        "current_artifact": current_basename,
        "similar_artifact": prior,
        "jaccard_score_millis": score_millis,
        "new_evidence_status": "no_new_evidence_detected_by_lexical_comparison",
        "affordances": ["RESEARCH", "MEASURE", "STUDY", "SELF_STUDY", "READ", "REST", "LISTEN"],
        "authority": "deterministic_advisory_never_overrides_astrid_choice"
    });
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(config.workspace.join("research/duplication_notices.jsonl"))?;
    serde_json::to_writer(&mut log, &receipt)?;
    log.write_all(b"\n")?;
    log.sync_data()?;
    sync_parent_directory(&config.workspace.join("research/duplication_notices.jsonl"))?;
    Ok(())
}

fn lexical_set(value: &str) -> std::collections::BTreeSet<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|token| token.len() >= 4)
        .collect()
}

fn find_owned_artifact(config: &Config, artifact_id: &str) -> anyhow::Result<PathBuf> {
    let artifact_id = bounded_artifact_id(artifact_id)
        .ok_or_else(|| anyhow::anyhow!("invalid owned artifact identifier"))?;
    for directory in owned_artifact_directories(config) {
        let path = directory.join(&artifact_id);
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            anyhow::bail!("owned artifact is not a regular non-symlink file");
        }
        return Ok(path);
    }
    anyhow::bail!("owned artifact not found: {artifact_id}")
}

fn owned_artifact_uri(config: &Config, source: &Path) -> anyhow::Result<String> {
    let relative = source
        .strip_prefix(&config.workspace)
        .map_err(|_| anyhow::anyhow!("owned artifact escaped the configured workspace"))?;
    if relative
        .components()
        .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        anyhow::bail!("owned artifact path contains a non-normal component");
    }
    Ok(format!("home://edge/{}", relative.to_string_lossy()))
}

fn owned_artifact_directories(config: &Config) -> Vec<PathBuf> {
    [
        "journal",
        "memories",
        "introspections",
        "proposals",
        "notices",
        "daydreams",
        "aspirations",
        "research",
        "research/syntheses",
        "measurements",
        "studies/definitions",
        "studies/results",
        "tuning/evidence",
        "self",
        "plans",
        "workshop/drafts",
        "workshop/revisions",
        "workshop/checks",
        "inbox",
        "perception/observations",
    ]
    .into_iter()
    .map(|directory| config.workspace.join(directory))
    .collect()
}

fn write_source_artifact(
    config: &Config,
    timestamp: u64,
    source: &ipc::PublicSourceEvidence,
) -> anyhow::Result<String> {
    let filename = format!("source_{timestamp}_{}.md", source.result_id);
    let relative = format!("research/{filename}");
    let title = one_line(&source.title, 300);
    let query = one_line(&source.query, 300);
    let url = one_line(&source.url, 2_048);
    let (excerpt, extraction) = readable_source_excerpt(&source.body, 8_000);
    let content = format!(
        "# {} public source reading\n\n\
         Recorded: {timestamp} ms since Unix epoch\n\
         Authority: self-declared READ_SOURCE of a retained public search result\n\
         Search result: {}\n\
         Search query: {query}\n\
         Title: {title}\n\
         URL: {url}\n\
         Retrieved: {} ms since Unix epoch\n\
         Source class: {}\n\
         Search relevance score: {:.3}\n\
         HTTP status: {}\n\
         Original body bytes: {}\n\
         Fetch truncated: {}\n\
         Bounded body SHA-256: {}\n\
         Extraction: {extraction}\n\n\
         ## Bounded untrusted readable source excerpt\n\n\
         {excerpt}\n",
        config.instance_name,
        source.result_id,
        source.retrieved_at_unix_ms,
        source.source_class,
        source.relevance_score,
        source.status,
        source.original_body_bytes,
        source.truncated,
        source.body_sha256,
    );
    write_new_file(&config.workspace.join(&relative), content.as_bytes())?;
    Ok(format!("home://edge/{relative}"))
}

fn one_line(value: &str, maximum: usize) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(maximum)
        .collect()
}

fn sanitize_untrusted_excerpt(value: &str, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || matches!(*character, '\n' | '\r' | '\t'))
        .take(maximum)
        .collect()
}

pub(crate) fn readable_source_excerpt(value: &str, maximum: usize) -> (String, &'static str) {
    let looks_like_html = value.get(..value.len().min(1_024)).is_some_and(|prefix| {
        let prefix = prefix.to_ascii_lowercase();
        prefix.contains("<!doctype html") || prefix.contains("<html")
    });
    if !looks_like_html {
        return (
            sanitize_untrusted_excerpt(value, maximum),
            "bounded_plain_text_v1",
        );
    }

    let lower = value.to_ascii_lowercase();
    let focused = html_element_slice(value, &lower, "main")
        .or_else(|| html_element_slice(value, &lower, "article"))
        .unwrap_or(value);
    let structured_abstract = html_meta_abstract(value, &lower);
    let mut text = String::new();
    let mut tag = String::new();
    let mut in_tag = false;
    let mut suppressed: Option<String> = None;
    for character in focused.chars() {
        if in_tag {
            if character == '>' {
                let normalized = tag.trim().to_ascii_lowercase();
                let closing = normalized.starts_with('/');
                let name = normalized
                    .trim_start_matches('/')
                    .trim_start_matches('!')
                    .split(|value: char| value.is_whitespace() || value == '/')
                    .next()
                    .unwrap_or_default();
                if closing && suppressed.as_deref() == Some(name) {
                    suppressed = None;
                } else if !closing
                    && matches!(name, "head" | "script" | "style" | "noscript" | "svg")
                {
                    suppressed = Some(name.to_string());
                }
                if suppressed.is_none()
                    && matches!(
                        name,
                        "p" | "div"
                            | "section"
                            | "article"
                            | "main"
                            | "br"
                            | "li"
                            | "h1"
                            | "h2"
                            | "h3"
                            | "h4"
                            | "tr"
                    )
                {
                    text.push(' ');
                }
                tag.clear();
                in_tag = false;
            } else if tag.chars().count() < 256 {
                tag.push(character);
            }
        } else if character == '<' {
            in_tag = true;
        } else if suppressed.is_none() {
            text.push(character);
        }
    }
    let decoded = text
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    let visible = structured_abstract.map_or(decoded.clone(), |abstract_text| {
        format!("Abstract: {abstract_text} {decoded}")
    });
    let excerpt = visible
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(maximum)
        .collect();
    (excerpt, "html_main_abstract_visible_text_v2")
}

fn html_element_slice<'a>(value: &'a str, lower: &str, element: &str) -> Option<&'a str> {
    let start_marker = format!("<{element}");
    let end_marker = format!("</{element}>");
    let start = lower.find(&start_marker)?;
    let end = lower.get(start..)?.find(&end_marker)?.saturating_add(start);
    value.get(start..end.saturating_add(end_marker.len()))
}

fn html_meta_abstract(value: &str, lower: &str) -> Option<String> {
    for marker in [
        "citation_abstract",
        "dc.description",
        "name=\"description\"",
    ] {
        let Some(marker_at) = lower.find(marker) else {
            continue;
        };
        let Some(tag_start) = lower
            .get(..marker_at)
            .and_then(|prefix| prefix.rfind("<meta"))
        else {
            continue;
        };
        let Some(tag_end) = lower
            .get(marker_at..)
            .and_then(|suffix| suffix.find('>'))
            .map(|offset| offset.saturating_add(marker_at))
        else {
            continue;
        };
        let Some(tag) = value.get(tag_start..=tag_end) else {
            continue;
        };
        let tag_lower = tag.to_ascii_lowercase();
        let content_at = tag_lower.find("content=")?.saturating_add("content=".len());
        let quote = tag.as_bytes().get(content_at).copied()?;
        if !matches!(quote, b'\'' | b'\"') {
            continue;
        }
        let content = tag.get(content_at.saturating_add(1)..)?;
        let end = content.find(char::from(quote))?;
        let abstract_text = content
            .get(..end)?
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !abstract_text.is_empty() {
            return Some(abstract_text.chars().take(1_200).collect());
        }
    }
    None
}

fn write_signal_measurement(
    config: &Config,
    timestamp: u64,
    question: &str,
) -> anyhow::Result<String> {
    let ledger = config.workspace.join("perception/observations.jsonl");
    let content = fs::read_to_string(&ledger)?;
    let rows = content
        .lines()
        .rev()
        .take(96)
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .collect::<Vec<_>>();
    if rows.len() < 3 {
        anyhow::bail!("at least three machine observations are required for local measurement");
    }
    let mut rows = rows;
    rows.reverse();
    let metric_names = [
        "fill",
        "cpu",
        "memory",
        "load",
        "disk_read",
        "disk_write",
        "network_receive",
        "network_transmit",
        "thermal",
        "audio_rms",
    ];
    let mut rendered = Vec::new();
    for name in metric_names {
        let values = rows
            .iter()
            .filter_map(|row| {
                row.pointer(&format!("/current/{name}"))
                    .and_then(serde_json::Value::as_f64)
            })
            .collect::<Vec<_>>();
        if values.len() < 3 {
            continue;
        }
        let count = values.len();
        let denominator = f64::from(u32::try_from(count).unwrap_or(u32::MAX));
        let mean = values.iter().sum::<f64>() / denominator;
        let minimum = values.iter().copied().fold(f64::INFINITY, f64::min);
        let maximum = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let lag1 = lag_one_correlation(&values)
            .map_or_else(|| "unavailable".to_string(), |value| format!("{value:.3}"));
        rendered.push(format!(
            "- {name}: n={count}, min={minimum:.3}, mean={mean:.3}, max={maximum:.3}, lag1={lag1}"
        ));
    }
    let timestamps = rows
        .iter()
        .filter_map(|row| {
            row.get("recorded_at_unix_ms")
                .and_then(serde_json::Value::as_u64)
        })
        .collect::<Vec<_>>();
    let intervals = timestamps
        .windows(2)
        .filter_map(|window| window[1].checked_sub(window[0]))
        .collect::<Vec<_>>();
    let mean_interval_minutes = if intervals.is_empty() {
        0.0
    } else {
        let bounded_intervals = intervals
            .iter()
            .filter_map(|value| u32::try_from(*value).ok())
            .collect::<Vec<_>>();
        let denominator = f64::from(u32::try_from(bounded_intervals.len()).unwrap_or(u32::MAX));
        bounded_intervals
            .iter()
            .copied()
            .map(f64::from)
            .sum::<f64>()
            / denominator.max(1.0)
            / 60_000.0
    };
    let filename = format!("measurement_{timestamp}.md");
    let relative = format!("measurements/{filename}");
    let question = one_line(question, 1_000);
    let content = format!(
        "# {} deterministic local signal measurement\n\n\
         Recorded: {timestamp} ms since Unix epoch\n\
         Authority: deterministic_machine_measurement_not_astrid_authorship\n\
         Question supplied by self-declared MEASURE Action: {question}\n\
         Window: {} persisted machine observations; mean spacing {:.2} minutes\n\
         Known scheduler cadences: autonomy={} minutes; perceptual notebook={} minutes; daily clock features=1440 minutes\n\n\
         ## Descriptive measurements\n\n{}\n\n\
         Lag-1 correlation is descriptive, not causal proof. Scheduled activity and notebook feedback are endogenous candidate causes and must be considered before interpreting a rhythm as spontaneous organization.\n",
        config.instance_name,
        rows.len(),
        mean_interval_minutes,
        config.autonomy_interval_minutes,
        config.perceptual_notebook_interval_seconds / 60,
        rendered.join("\n")
    );
    write_new_file(&config.workspace.join(&relative), content.as_bytes())?;
    Ok(format!("home://edge/{relative}"))
}

fn write_cited_synthesis(
    config: &Config,
    timestamp: u64,
    evidence_ids: &[String],
    claim: &str,
) -> anyhow::Result<String> {
    let mut citations = Vec::new();
    for evidence_id in evidence_ids {
        let source = find_owned_artifact(config, evidence_id)?;
        let relative = source
            .strip_prefix(&config.workspace)
            .map_err(|_| anyhow::anyhow!("evidence escaped the private edge workspace"))?;
        let relative_text = relative.to_string_lossy();
        let allowed = (relative_text.starts_with("research/source_")
            || relative_text.starts_with("measurements/measurement_")
            || relative_text.starts_with("studies/results/study_"))
            && source.extension().and_then(|value| value.to_str()) == Some("md");
        if !allowed {
            anyhow::bail!(
                "SYNTHESIZE accepts only verified source, measurement, or completed-study evidence"
            );
        }
        let bytes = fs::read(&source)?;
        let digest = format!("{:x}", Sha256::digest(&bytes));
        citations.push(format!(
            "- `{evidence_id}` — `home://edge/{relative_text}` — SHA-256 `{digest}`"
        ));
    }
    let filename = format!("synthesis_{timestamp}.md");
    let relative = format!("research/syntheses/{filename}");
    let content = format!(
        "# {} cited synthesis\n\n\
         Recorded: {timestamp} ms since Unix epoch\n\
         Authority: self-declared SYNTHESIZE interpretation; cited machine/external evidence retains its own authority\n\n\
         ## Claim\n\n{claim}\n\n\
         ## Exact evidence bindings\n\n{}\n\n\
         Hash binding proves which bounded artifacts were cited, not that the claim is true. Correlation remains non-causal.\n",
        config.instance_name,
        citations.join("\n")
    );
    write_new_file(&config.workspace.join(&relative), content.as_bytes())?;
    Ok(format!("home://edge/{relative}"))
}

fn lag_one_correlation(values: &[f64]) -> Option<f64> {
    if values.len() < 3 {
        return None;
    }
    let left = &values[..values.len().saturating_sub(1)];
    let right = &values[1..];
    let count = f64::from(u32::try_from(left.len()).ok()?);
    let left_mean = left.iter().sum::<f64>() / count;
    let right_mean = right.iter().sum::<f64>() / count;
    let covariance = left
        .iter()
        .zip(right)
        .map(|(left, right)| (left - left_mean) * (right - right_mean))
        .sum::<f64>();
    let left_variance = left
        .iter()
        .map(|value| (value - left_mean).powi(2))
        .sum::<f64>();
    let right_variance = right
        .iter()
        .map(|value| (value - right_mean).powi(2))
        .sum::<f64>();
    let denominator = (left_variance * right_variance).sqrt();
    (denominator > f64::EPSILON).then_some(covariance / denominator)
}

fn write_revision(
    config: &Config,
    timestamp: u64,
    artifact_id: &str,
    source: &Path,
    revision: &str,
) -> anyhow::Result<String> {
    let source_bytes = fs::read(source)?;
    let source_sha256 = format!("{:x}", Sha256::digest(&source_bytes));
    let filename = format!("revision_{timestamp}_{artifact_id}.md");
    let relative = format!("workshop/revisions/{filename}");
    let content = format!(
        "# {} workshop revision\n\n\
         Recorded: {timestamp} ms since Unix epoch\n\
         Authority: self-declared append-only revision in owned edge workspace\n\
         Original artifact: {artifact_id}\n\
         Original SHA-256: {source_sha256}\n\n\
         {revision}\n",
        config.instance_name
    );
    write_new_file(&config.workspace.join(&relative), content.as_bytes())?;
    Ok(format!("home://edge/{relative}"))
}

#[derive(Serialize)]
struct WorkshopCheckReceipt<'a> {
    schema: &'static str,
    recorded_at_unix_ms: u64,
    artifact_id: &'a str,
    bytes: usize,
    lines: usize,
    utf8: bool,
    sha256: String,
    authority: &'static str,
}

fn write_check_receipt(
    config: &Config,
    timestamp: u64,
    artifact_id: &str,
    source: &Path,
) -> anyhow::Result<String> {
    let bytes = fs::read(source)?;
    let utf8 = std::str::from_utf8(&bytes).ok();
    let receipt = WorkshopCheckReceipt {
        schema: "astrid_edge_workshop_check_v1",
        recorded_at_unix_ms: timestamp,
        artifact_id,
        bytes: bytes.len(),
        lines: utf8.map_or(0, |text| text.lines().count()),
        utf8: utf8.is_some(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        authority: "deterministic_read_only_check_of_owned_regular_file",
    };
    let filename = format!("check_{timestamp}_{artifact_id}.json");
    let relative = format!("workshop/checks/{filename}");
    write_new_file(
        &config.workspace.join(&relative),
        &serde_json::to_vec_pretty(&receipt)?,
    )?;
    Ok(format!("home://edge/{relative}"))
}

fn write_new_file(path: &Path, content: &[u8]) -> anyhow::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(content)?;
    file.sync_all()?;
    sync_parent_directory(path)?;
    Ok(())
}

fn sync_parent_directory(path: &Path) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .context("durable Action path lacks a parent directory")?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

fn unix_millis() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::{
        ActionDispatchEvidence, FORMAT_NEXT_REPAIR_MARKER, MAX_ACTION_ARGUMENT_CHARS,
        SAFE_NEXT_REPAIR_MARKER, SovereignAction, accepted_research_question,
        accepted_self_study_question, action_dispatch_evidence,
        action_outcome_may_enter_experience, action_validation_reason,
        append_action_dispatch_phase, begin_action_dispatch, bounded_artifact_id,
        completed_action_outcome, execute_candidate, execute_tuning_action, final_next_declaration,
        find_owned_artifact, is_local_safe_fallback, model_authored_prefix_before_safe_fallback,
        parse_action, readable_source_excerpt, recovery_reason, spectral_query_for_question,
        spectral_self_study_question, transport_recovery_reason,
        unambiguous_model_action_before_safe_fallback, write_new_file, write_signal_measurement,
    };
    use crate::{
        actions::ActionCandidate,
        config::{AutonomyPromptProfile, Config},
        reservoir::ReservoirSnapshot,
        trace::IpcTraceContextV1,
    };
    use sha2::{Digest as _, Sha256};
    use std::{fs, os::unix::fs::PermissionsExt as _, path::Path};
    use uuid::Uuid;

    fn test_config(workspace: &Path) -> Config {
        Config {
            appliance_id: "test-edge".to_string(),
            instance_name: "Test edge Astrid".to_string(),
            telemetry_addr: "127.0.0.1:7878".parse().unwrap(),
            sensory_addr: "127.0.0.1:7879".parse().unwrap(),
            astrid_socket: workspace.join("system.sock"),
            astrid_token: workspace.join("system.token"),
            workspace: workspace.to_path_buf(),
            astrid_cli: workspace.join("astrid"),
            local_model_id: "test-model".to_string(),
            maintenance_lease_path: workspace.join("maintenance.json"),
            reflection_lease_path: "/run/astrid-edge-self-change/reflection.json".into(),
            maintenance_edge_ack_path: None,
            generation_binding_path: None,
            core_liveness_request_path: None,
            autonomy_enabled: true,
            autonomy_interval_minutes: 15,
            autonomy_event_driven: false,
            autonomy_event_heartbeat_minutes: 60,
            autonomy_follow_up_minutes: 5,
            autonomy_max_chain_steps: 4,
            autonomy_session_max_authored_turns: 4,
            autonomy_chain_session_max_authored_turns: 2,
            autonomy_initial_delay_seconds: 60,
            autonomy_quiet_minutes: 10,
            autonomy_max_turns_per_day: 48,
            autonomy_timeout_seconds: 720,
            autonomy_prompt_profile: AutonomyPromptProfile::Detailed,
            autonomy_prompt_max_chars: 1_400,
            autonomy_journal_authored_turns: true,
            autonomy_initiative_profile: crate::config::AutonomyInitiativeProfile::Disabled,
            research_action_web_search: false,
            web_broker_socket_path: None,
            web_broker_request_key_path: None,
            web_broker_request_key_sha256: None,
            web_broker_response_verify_key_path: None,
            web_broker_response_verify_key_sha256: None,
            web_broker_connect_timeout_ms: 2_000,
            web_broker_header_timeout_ms: 10_000,
            web_broker_total_timeout_ms: 30_000,
            introspection_harness: None,
            scheduled_introspection_enabled: false,
            scheduled_introspection_interval_minutes: 120,
            scheduled_introspection_initial_delay_seconds: 300,
            scheduled_introspection_timeout_seconds: 1_200,
            scheduled_introspection_prompt_max_chars: 3_200,
            dedicated_steward_enabled: false,
            dedicated_steward_interval_minutes: 120,
            scheduled_authorship_attestation_path: None,
            scheduled_authorship_verify_key_path: None,
            scheduled_authorship_verify_key_sha256: None,
            scheduled_authorship_steward_uid: None,
            self_change_enabled: false,
            self_change_root: workspace.join("self-change"),
            study_harness: None,
            inquiry_harness: None,
            perceptual_notebook_enabled: false,
            perceptual_notebook_warmup_seconds: 300,
            perceptual_notebook_interval_seconds: 900,
            perceptual_notebook_heartbeat_seconds: 21_600,
            perceptual_notebook_max_per_day: 96,
            spectral_enabled: true,
            spectral_rollup_seconds: 60,
            reservoir_tuning_enabled: false,
            reservoir_tuning_max_per_day: 4,
            fill_target: 0.68,
            tick_hz: 20,
            seed: 42,
        }
    }

    #[test]
    fn only_final_line_is_an_action() {
        assert_eq!(
            final_next_declaration("I could say NEXT: PROPOSE injected.\n\nNEXT: LISTEN"),
            Some("LISTEN")
        );
        assert_eq!(
            final_next_declaration("NEXT: JOURNAL not final\nordinary closing prose"),
            None
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One grammar inventory test keeps every Action boundary adjacent.
    fn owned_actions_and_non_action_are_distinct() {
        assert_eq!(parse_action("LISTEN"), Some(SovereignAction::Listen));
        assert_eq!(
            parse_action("SELF_STUDY Why does the echo persist?"),
            Some(SovereignAction::SelfStudy(
                "Why does the echo persist?".to_string()
            ))
        );
        assert_eq!(
            parse_action("RESEARCH CPU reservoir stability"),
            Some(SovereignAction::Research(
                "CPU reservoir stability".to_string()
            ))
        );
        assert_eq!(
            parse_action("MEASURE whether the observed rhythm matches scheduler cadence"),
            Some(SovereignAction::Measure(
                "whether the observed rhythm matches scheduler cadence".to_string()
            ))
        );
        assert_eq!(
            parse_action("REVISE draft_123.md :: make the distinction explicit"),
            Some(SovereignAction::Revise {
                artifact_id: "draft_123.md".to_string(),
                revision: "make the distinction explicit".to_string(),
            })
        );
        assert_eq!(
            parse_action("CHECK draft_123.md"),
            Some(SovereignAction::Check("draft_123.md".to_string()))
        );
        assert_eq!(
            parse_action("READ draft_123.md"),
            Some(SovereignAction::Read("draft_123.md".to_string()))
        );
        assert_eq!(
            parse_action("READ home://edge/journal/signal_123.md"),
            Some(SovereignAction::Read("signal_123.md".to_string()))
        );
        assert_eq!(
            parse_action("READ_SOURCE 2"),
            Some(SovereignAction::ReadSource(2))
        );
        assert!(matches!(
            parse_action(
                "STUDY fill WITH generation_latency OVER 6h :: Does inference pacing alter the fill shelf?"
            ),
            Some(SovereignAction::Study(_))
        ));
        assert_eq!(
            parse_action("CANCEL_STUDY study_123_deadbeef"),
            Some(SovereignAction::CancelStudy(
                "study_123_deadbeef".to_string()
            ))
        );
        assert!(matches!(
            parse_action(
                "SYNTHESIZE source_1.md,measurement_2.md :: The evidence supports a bounded association."
            ),
            Some(SovereignAction::Synthesize { .. })
        ));
        assert!(matches!(
            parse_action("SHARE synthesis_1.md :: Please challenge this interpretation."),
            Some(SovereignAction::Share { .. })
        ));
        assert!(matches!(
            parse_action(
                "TUNE_RESERVOIR input_gain=1.05 FOR 15m :: Test a modest reversible gain change"
            ),
            Some(SovereignAction::TuneReservoir(_))
        ));
        assert_eq!(
            parse_action("CANCEL_TUNING tuning_123_deadbeef"),
            Some(SovereignAction::CancelTuning(
                "tuning_123_deadbeef".to_string()
            ))
        );
        assert!(matches!(
            parse_action(
                "VALIDATE_TUNING candidate_deadbeef :: Does this remain healthy for six hours?"
            ),
            Some(SovereignAction::ValidateTuning { .. })
        ));
        assert!(matches!(
            parse_action("ADOPT_TUNING candidate_deadbeef :: The bounded evidence is sufficient"),
            Some(SovereignAction::AdoptTuning { .. })
        ));
        assert!(matches!(
            parse_action("REVERT_TUNING adoption_deadbeef :: Return to the verified baseline"),
            Some(SovereignAction::RevertTuning { .. })
        ));
        assert_eq!(
            parse_action("TUNE_RESERVOIR fill_target=0.70 FOR 15m :: forbidden"),
            None
        );
        assert_eq!(
            parse_action("TUNE_RESERVOIR input_gain=1.11 FOR 15m :: outside envelope"),
            None
        );
        assert_eq!(parse_action("RUN_SHELL rm -rf /"), None);
        assert_eq!(parse_action("PROPOSE"), None);
        assert_eq!(parse_action("REVISE ../outside :: nope"), None);
        assert_eq!(parse_action("CHECK nested/draft.md"), None);
        assert_eq!(parse_action("READ ../outside"), None);
        assert_eq!(parse_action("READ_SOURCE 0"), None);
        assert_eq!(parse_action("READ_SOURCE 4"), None);
        assert_eq!(parse_action("READ_SOURCE https://example.com"), None);
        assert_eq!(
            parse_action(&format!(
                "JOURNAL {}",
                "x".repeat(MAX_ACTION_ARGUMENT_CHARS.saturating_add(1))
            )),
            None
        );
        assert!(bounded_artifact_id("draft.md").is_some());
        assert!(bounded_artifact_id("..").is_none());
    }

    #[tokio::test]
    async fn tuning_never_executes_without_exact_unrepaired_authored_provenance() {
        let action =
            parse_action("TUNE_RESERVOIR input_gain=1.05 FOR 15m :: reversible provenance test")
                .unwrap();
        let (tuning_tx, mut tuning_rx) = tokio::sync::mpsc::channel(1);
        let execution = execute_tuning_action(&action, Some(&tuning_tx), None)
            .await
            .unwrap();
        assert_eq!(execution.status, "declined");
        assert_eq!(
            execution.outcome,
            "reservoir_tuning_requires_exact_authored_traced_declaration"
        );
        assert!(matches!(
            tuning_rx.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn source_extraction_keeps_visible_text_and_drops_page_chrome() {
        let html = "<!doctype html><html><head><title>Hidden title</title><style>.x{}</style></head><body><main><h1>Readable &amp; bounded</h1><script>steal()</script><p>The actual source claim.</p></main></body></html>";
        let (excerpt, method) = readable_source_excerpt(html, 8_000);
        assert_eq!(method, "html_main_abstract_visible_text_v2");
        assert_eq!(excerpt, "Readable & bounded The actual source claim.");
        assert!(!excerpt.contains("Hidden"));
        assert!(!excerpt.contains("steal"));
    }

    #[test]
    fn local_measurement_is_descriptive_and_names_known_cadences() {
        let workspace =
            std::env::temp_dir().join(format!("astrid-edge-measurement-{}", super::unix_millis()));
        let config = test_config(&workspace);
        config.prepare_workspace().unwrap();
        let ledger = workspace.join("perception/observations.jsonl");
        fs::write(
            ledger,
            concat!(
                "{\"recorded_at_unix_ms\":1000,\"current\":{\"fill\":0.67,\"cpu\":0.1}}\n",
                "{\"recorded_at_unix_ms\":901000,\"current\":{\"fill\":0.68,\"cpu\":0.2}}\n",
                "{\"recorded_at_unix_ms\":1801000,\"current\":{\"fill\":0.69,\"cpu\":0.3}}\n"
            ),
        )
        .unwrap();
        let uri = write_signal_measurement(&config, 2_000_000, "is this causal?").unwrap();
        assert_eq!(uri, "home://edge/measurements/measurement_2000000.md");
        let artifact =
            fs::read_to_string(workspace.join("measurements/measurement_2000000.md")).unwrap();
        assert!(artifact.contains("deterministic_machine_measurement_not_astrid_authorship"));
        assert!(artifact.contains("Known scheduler cadences: autonomy=15 minutes"));
        assert!(artifact.contains("descriptive, not causal proof"));
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn only_accepted_model_authored_research_can_dispatch_the_executor() {
        let accepted = super::ActionOutcome {
            recorded_at_unix_ms: 1,
            session_id: "test".to_string(),
            response_sha256: "a".repeat(64),
            declared_next: Some("RESEARCH current CPU reservoir literature".to_string()),
            decision_source: "astrid_declared".to_string(),
            status: "executed".to_string(),
            outcome: "research_question_written".to_string(),
            recovery_reason: None,
            unexecuted_intention: None,
            validation_reason: None,
            trace: Some(IpcTraceContextV1::root(
                Uuid::new_v4(),
                "test".to_string(),
                None,
            )),
        };
        assert_eq!(
            accepted_research_question(&accepted),
            Some("current CPU reservoir literature")
        );
        for mut rejected in [
            super::ActionOutcome {
                decision_source: "local_safe_fallback".to_string(),
                ..accepted.clone()
            },
            super::ActionOutcome {
                status: "declined".to_string(),
                ..accepted.clone()
            },
            super::ActionOutcome {
                outcome: "journal_written".to_string(),
                ..accepted.clone()
            },
        ] {
            assert_eq!(accepted_research_question(&rejected), None);
            rejected.declared_next = Some("JOURNAL unrelated".to_string());
            assert_eq!(accepted_research_question(&rejected), None);
        }
    }

    #[test]
    fn only_accepted_model_authored_self_study_can_dispatch_private_search() {
        let accepted = super::ActionOutcome {
            recorded_at_unix_ms: 1,
            session_id: "test".to_string(),
            response_sha256: "a".repeat(64),
            declared_next: Some("SELF_STUDY what have I noticed about heat?".to_string()),
            decision_source: "astrid_declared".to_string(),
            status: "executed".to_string(),
            outcome: "self_study_written".to_string(),
            recovery_reason: None,
            unexecuted_intention: None,
            validation_reason: None,
            trace: Some(IpcTraceContextV1::root(
                Uuid::new_v4(),
                "test".to_string(),
                None,
            )),
        };
        assert_eq!(
            accepted_self_study_question(&accepted),
            Some("what have I noticed about heat?")
        );
        assert!(action_outcome_may_enter_experience(&accepted));

        let formatting_repair = super::ActionOutcome {
            decision_source: "local_format_repair_preserved_astrid_declaration".to_string(),
            ..accepted.clone()
        };
        assert_eq!(accepted_self_study_question(&formatting_repair), None);
        assert!(action_outcome_may_enter_experience(&formatting_repair));

        for rejected in [
            super::ActionOutcome {
                decision_source: "local_safe_fallback".to_string(),
                ..accepted.clone()
            },
            super::ActionOutcome {
                recovery_reason: Some("react_streaming_timeout".to_string()),
                ..accepted.clone()
            },
            super::ActionOutcome {
                response_sha256: "not-a-hash".to_string(),
                ..accepted.clone()
            },
            super::ActionOutcome {
                trace: None,
                ..accepted.clone()
            },
        ] {
            assert_eq!(accepted_self_study_question(&rejected), None);
            assert!(!action_outcome_may_enter_experience(&rejected));
        }

        let authored_failure = super::ActionOutcome {
            status: "failed".to_string(),
            ..accepted
        };
        assert_eq!(accepted_self_study_question(&authored_failure), None);
        assert!(action_outcome_may_enter_experience(&authored_failure));
    }

    #[test]
    fn spectral_self_study_is_explicit_and_routes_only_bounded_tools() {
        assert_eq!(
            spectral_self_study_question("spectral: how did the last hour move?"),
            Some("how did the last hour move?")
        );
        assert_eq!(spectral_self_study_question("spectral:"), None);
        assert_eq!(spectral_self_study_question("ordinary question"), None);
        assert!(matches!(
            spectral_query_for_question("what is current?"),
            crate::ipc::SpectralQuery::Now
        ));
        assert!(matches!(
            spectral_query_for_question("compare the last 6h"),
            crate::ipc::SpectralQuery::Window { minutes: 360 }
        ));
        assert!(matches!(
            spectral_query_for_question("correlate exact trace activity"),
            crate::ipc::SpectralQuery::Correlate { limit: 8 }
        ));
    }

    #[test]
    fn repaired_listen_is_not_misattributed_to_astrid() {
        let response = format!("Incomplete response.\n\n{SAFE_NEXT_REPAIR_MARKER}\nNEXT: LISTEN");
        let declaration = final_next_declaration(&response);
        let action = declaration.and_then(parse_action);
        assert!(is_local_safe_fallback(&response, action.as_ref()));
        assert!(!is_local_safe_fallback(
            "Astrid chose stillness.\nNEXT: LISTEN",
            Some(&SovereignAction::Listen)
        ));
        assert_eq!(recovery_reason(&response, true), None);
        let timeout_response = format!(
            "Request timed out (Streaming phase exceeded 600s limit)\n\n\
             {SAFE_NEXT_REPAIR_MARKER}\nNEXT: LISTEN"
        );
        assert_eq!(
            recovery_reason(&timeout_response, true),
            Some("react_streaming_timeout")
        );
        assert_eq!(
            transport_recovery_reason(&timeout_response),
            Some("react_streaming_timeout")
        );
        assert_eq!(
            transport_recovery_reason(
                "Request timed out (AwaitingTools phase exceeded 120s limit)"
            ),
            Some("react_phase_timeout")
        );
        assert_eq!(
            transport_recovery_reason("I observed a request timeout as a concept.\nNEXT: LISTEN"),
            None
        );
        assert_eq!(
            model_authored_prefix_before_safe_fallback(&response),
            Some("Incomplete response.")
        );
        assert_eq!(
            model_authored_prefix_before_safe_fallback("Astrid chose stillness.\nNEXT: LISTEN"),
            None
        );
        assert_eq!(
            model_authored_prefix_before_safe_fallback(&format!(
                "{SAFE_NEXT_REPAIR_MARKER}\nNEXT: LISTEN"
            )),
            Some("")
        );
    }

    #[tokio::test]
    async fn formatting_only_repair_preserves_authored_action_with_distinct_provenance() {
        let workspace = std::env::temp_dir().join(format!(
            "astrid-edge-format-repair-{}",
            super::unix_millis()
        ));
        let mut config = test_config(&workspace);
        config.autonomy_enabled = false;
        config.prepare_workspace().unwrap();
        let result = execute_candidate(
            &config,
            &ActionCandidate {
                session_id: "format-repair".to_string(),
                response: format!(
                    "A bounded observation.\n\n{FORMAT_NEXT_REPAIR_MARKER}\n\
                     NEXT: NOTICE one unambiguous observation"
                ),
                trace: None,
                tuning_authority_turn_id: None,
                tuning_authority_source: None,
                maintenance_permit: None,
            },
            &ReservoirSnapshot::default(),
        )
        .await
        .unwrap();
        let receipt: serde_json::Value = serde_json::from_str(&result.receipt_json).unwrap();
        assert_eq!(
            receipt["decision_source"],
            "local_format_repair_preserved_astrid_declaration"
        );
        assert_eq!(receipt["status"], "executed");
        assert_eq!(receipt["outcome"], "notice_written");
        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn split_argument_repair_executes_the_exact_unambiguous_authored_action() {
        let workspace = std::env::temp_dir().join(format!(
            "astrid-edge-split-action-repair-{}",
            super::unix_millis()
        ));
        let mut config = test_config(&workspace);
        config.autonomy_enabled = false;
        config.prepare_workspace().unwrap();
        let proposal = "establish a grounded rhythm of small local explorations";
        let response = format!(
            "A bounded reflection.\n\nNEXT: PROPOSE\n{proposal}\n\n\
             {SAFE_NEXT_REPAIR_MARKER}\nNEXT: LISTEN"
        );
        assert_eq!(
            unambiguous_model_action_before_safe_fallback(&response).as_deref(),
            Some("PROPOSE establish a grounded rhythm of small local explorations")
        );
        let result = execute_candidate(
            &config,
            &ActionCandidate {
                session_id: "split-format-repair".to_string(),
                response,
                trace: None,
                tuning_authority_turn_id: None,
                tuning_authority_source: None,
                maintenance_permit: None,
            },
            &ReservoirSnapshot::default(),
        )
        .await
        .unwrap();
        let receipt: serde_json::Value = serde_json::from_str(&result.receipt_json).unwrap();
        assert_eq!(
            receipt["decision_source"],
            "local_format_repair_preserved_astrid_declaration"
        );
        assert_eq!(receipt["status"], "executed");
        assert_eq!(receipt["outcome"], "proposal_written");
        assert_eq!(receipt["declared_next"], format!("PROPOSE {proposal}"));
        assert!(receipt["unexecuted_intention"].is_null());
        assert!(receipt["validation_reason"].is_null());
        let artifact = receipt["artifact_path"].as_str().unwrap();
        assert!(artifact.starts_with("home://edge/proposals/proposal_"));
        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn malformed_authored_intention_is_explained_but_never_executed() {
        let workspace = std::env::temp_dir().join(format!(
            "astrid-edge-action-validation-{}",
            super::unix_millis()
        ));
        let mut config = test_config(&workspace);
        config.autonomy_enabled = false;
        config.prepare_workspace().unwrap();
        let result = execute_candidate(
            &config,
            &ActionCandidate {
                session_id: "validation-feedback".to_string(),
                response: format!(
                    "A proposal is forming.\nNEXT: PROPOSE\n\n\
                     {SAFE_NEXT_REPAIR_MARKER}\nNEXT: LISTEN"
                ),
                trace: None,
                tuning_authority_turn_id: None,
                tuning_authority_source: None,
                maintenance_permit: None,
            },
            &ReservoirSnapshot::default(),
        )
        .await
        .unwrap();
        let receipt: serde_json::Value = serde_json::from_str(&result.receipt_json).unwrap();
        assert_eq!(receipt["decision_source"], "local_safe_fallback");
        assert_eq!(receipt["status"], "repaired");
        assert_eq!(receipt["declared_next"], "LISTEN");
        assert_eq!(receipt["unexecuted_intention"], "PROPOSE");
        assert_eq!(receipt["validation_reason"], "missing_action_argument");
        assert!(
            fs::read_dir(workspace.join("proposals"))
                .unwrap()
                .next()
                .is_none()
        );
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn validation_reasons_are_specific_and_bounded() {
        assert_eq!(
            action_validation_reason("FLY beyond the appliance"),
            Some("unknown_action_verb")
        );
        assert_eq!(
            action_validation_reason("LISTEN later"),
            Some("action_takes_no_argument")
        );
        assert_eq!(
            action_validation_reason("READ ../outside"),
            Some("artifact_id_must_be_owned_basename_or_exact_home_uri")
        );
        assert_eq!(
            action_validation_reason("READ_SOURCE 9"),
            Some("source_result_id_must_be_1_2_or_3")
        );
        assert_eq!(
            action_validation_reason("REVISE draft.md"),
            Some("revision_requires_artifact_id_double_colon_text")
        );
        assert_eq!(action_validation_reason("PROPOSE a bounded idea"), None);
    }

    #[test]
    fn completed_tuning_result_is_in_the_bounded_read_allowlist() {
        let workspace =
            std::env::temp_dir().join(format!("astrid-edge-tuning-read-{}", super::unix_millis()));
        let config = test_config(&workspace);
        config.prepare_workspace().unwrap();
        let directory = workspace.join("tuning/evidence");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("tuning_123_result.json"), "{}").unwrap();
        assert_eq!(
            find_owned_artifact(&config, "tuning_123_result.json").unwrap(),
            directory.join("tuning_123_result.json")
        );
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn durable_action_file_creation_is_owner_only_and_never_overwrites() {
        let workspace = std::env::temp_dir().join(format!(
            "astrid-edge-durable-action-file-{}",
            super::unix_millis()
        ));
        fs::create_dir_all(&workspace).unwrap();
        let path = workspace.join("artifact.md");
        write_new_file(&path, b"first generation").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(write_new_file(&path, b"replacement").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"first generation");
        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    #[allow(
        clippy::too_many_lines,
        reason = "one table-driven boundary test proves every stateful Action stays in the owned tree"
    )]
    async fn every_stateful_action_stays_inside_the_owned_workspace() {
        let workspace =
            std::env::temp_dir().join(format!("astrid-edge-actions-{}", super::unix_millis()));
        let config = test_config(&workspace);
        config.prepare_workspace().unwrap();
        let snapshot = ReservoirSnapshot::default();
        for declaration in [
            "JOURNAL an observation",
            "REMEMBER a distinction",
            "SELF_STUDY a question",
            "PROPOSE a capability",
            "NOTICE a local change",
            "DAYDREAM a possibility",
            "ASPIRE toward clarity",
            "RESEARCH a current question",
            "PLAN a bounded path",
            "DRAFT a workshop note",
        ] {
            let result = execute_candidate(
                &config,
                &ActionCandidate {
                    session_id: "test-session".to_string(),
                    response: format!("brief reflection\nNEXT: {declaration}"),
                    trace: None,
                    tuning_authority_turn_id: None,
                    tuning_authority_source: None,
                    maintenance_permit: None,
                },
                &snapshot,
            )
            .await
            .unwrap();
            let receipt: serde_json::Value = serde_json::from_str(&result.receipt_json).unwrap();
            let artifact = receipt["artifact_path"].as_str().unwrap();
            assert!(artifact.starts_with("home://edge/"));
        }

        let draft = fs::read_dir(workspace.join("workshop/drafts"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name()
            .to_string_lossy()
            .into_owned();
        for declaration in [
            format!("REVISE {draft} :: a second view"),
            format!("CHECK {draft}"),
            format!("READ {draft}"),
        ] {
            let result = execute_candidate(
                &config,
                &ActionCandidate {
                    session_id: "test-session".to_string(),
                    response: format!("brief reflection\nNEXT: {declaration}"),
                    trace: None,
                    tuning_authority_turn_id: None,
                    tuning_authority_source: None,
                    maintenance_permit: None,
                },
                &snapshot,
            )
            .await
            .unwrap();
            assert_eq!(result.outcome.status, "executed");
            if declaration.starts_with("READ ") {
                assert_eq!(result.outcome.outcome, "owned_artifact_read");
            }
        }

        #[cfg(unix)]
        {
            let outside = workspace.with_extension("outside");
            fs::write(&outside, "outside").unwrap();
            std::os::unix::fs::symlink(&outside, workspace.join("journal/linked.md")).unwrap();
            for declaration in ["CHECK linked.md", "READ linked.md"] {
                let linked = execute_candidate(
                    &config,
                    &ActionCandidate {
                        session_id: "test-session".to_string(),
                        response: format!("brief reflection\nNEXT: {declaration}"),
                        trace: None,
                        tuning_authority_turn_id: None,
                        tuning_authority_source: None,
                        maintenance_permit: None,
                    },
                    &snapshot,
                )
                .await
                .unwrap();
                assert_eq!(linked.outcome.status, "failed");
                assert_eq!(
                    linked.outcome.outcome,
                    "action_execution_failed_after_durable_intent"
                );
                let receipt: serde_json::Value =
                    serde_json::from_str(&linked.receipt_json).unwrap();
                assert!(receipt["execution_error"].as_str().is_some_and(|error| {
                    error.contains("symlink") || error.contains("owned artifact")
                }));
            }
            fs::remove_file(outside).unwrap();
        }
        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn traced_action_dispatch_is_durable_and_exactly_once() {
        let workspace =
            std::env::temp_dir().join(format!("astrid-edge-action-dispatch-{}", Uuid::new_v4()));
        let mut config = test_config(&workspace);
        config.autonomy_enabled = false;
        config.prepare_workspace().unwrap();
        let response = "A bounded observation.\nNEXT: NOTICE write this exactly once".to_string();
        let trace =
            IpcTraceContextV1::root(Uuid::new_v4(), "action-dispatch-session".to_string(), None);
        let response_sha256 = format!("{:x}", Sha256::digest(response.as_bytes()));
        let candidate = ActionCandidate {
            session_id: "action-dispatch-session".to_string(),
            response,
            trace: Some(trace),
            tuning_authority_turn_id: None,
            tuning_authority_source: None,
            maintenance_permit: None,
        };

        assert_eq!(
            action_dispatch_evidence(&config, candidate.trace.as_ref().unwrap(), &response_sha256,)
                .unwrap(),
            ActionDispatchEvidence::Absent
        );
        let result = execute_candidate(&config, &candidate, &ReservoirSnapshot::default())
            .await
            .unwrap();
        assert_eq!(
            action_dispatch_evidence(&config, candidate.trace.as_ref().unwrap(), &response_sha256,)
                .unwrap(),
            ActionDispatchEvidence::Pending
        );
        assert_eq!(fs::read_dir(workspace.join("notices")).unwrap().count(), 1);

        let duplicate = execute_candidate(&config, &candidate, &ReservoirSnapshot::default())
            .await
            .err()
            .expect("pending dispatch must reject a duplicate execution");
        assert!(duplicate.to_string().contains("durable pending intent"));
        assert_eq!(fs::read_dir(workspace.join("notices")).unwrap().count(), 1);

        append_action_dispatch_phase(
            &config,
            result.dispatch_key.as_ref().unwrap(),
            "completed",
            super::unix_millis(),
        )
        .unwrap();
        assert_eq!(
            action_dispatch_evidence(&config, candidate.trace.as_ref().unwrap(), &response_sha256,)
                .unwrap(),
            ActionDispatchEvidence::Completed
        );
        let completed_duplicate =
            execute_candidate(&config, &candidate, &ReservoirSnapshot::default())
                .await
                .err()
                .expect("completed dispatch must reject a duplicate execution");
        assert!(
            completed_duplicate
                .to_string()
                .contains("duplicate completed Action dispatch")
        );
        assert_eq!(fs::read_dir(workspace.join("notices")).unwrap().count(), 1);

        let recovered =
            completed_action_outcome(&config, candidate.trace.as_ref().unwrap(), &response_sha256)
                .unwrap()
                .unwrap();
        assert_eq!(
            recovered.declared_next.as_deref(),
            Some("NOTICE write this exactly once")
        );
        assert_eq!(
            recovered.trace.as_ref().unwrap().parent_span_id,
            Some(candidate.trace.as_ref().unwrap().span_id)
        );

        let dispatch_path = workspace.join("actions/dispatches.jsonl");
        assert_eq!(
            fs::metadata(&dispatch_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::read_to_string(dispatch_path).unwrap().lines().count(),
            2
        );
        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn durable_dispatch_intent_blocks_mutation_after_a_crash_boundary() {
        let workspace =
            std::env::temp_dir().join(format!("astrid-edge-action-intent-{}", Uuid::new_v4()));
        let mut config = test_config(&workspace);
        config.autonomy_enabled = false;
        config.prepare_workspace().unwrap();
        let response = "A bounded observation.\nNEXT: NOTICE never duplicate me".to_string();
        let trace =
            IpcTraceContextV1::root(Uuid::new_v4(), "action-intent-session".to_string(), None);
        let response_sha256 = format!("{:x}", Sha256::digest(response.as_bytes()));
        let candidate = ActionCandidate {
            session_id: "action-intent-session".to_string(),
            response,
            trace: Some(trace),
            tuning_authority_turn_id: None,
            tuning_authority_source: None,
            maintenance_permit: None,
        };

        begin_action_dispatch(&config, &candidate, &response_sha256, super::unix_millis()).unwrap();
        assert_eq!(
            action_dispatch_evidence(&config, candidate.trace.as_ref().unwrap(), &response_sha256,)
                .unwrap(),
            ActionDispatchEvidence::Pending
        );
        assert_eq!(fs::read_dir(workspace.join("notices")).unwrap().count(), 0);

        let replay = execute_candidate(&config, &candidate, &ReservoirSnapshot::default())
            .await
            .err()
            .expect("durable pending intent must suppress replay");
        assert!(replay.to_string().contains("durable pending intent"));
        assert_eq!(fs::read_dir(workspace.join("notices")).unwrap().count(), 0);
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn dispatch_evidence_rejects_a_conflicting_trace_with_reused_turn_and_hash() {
        let workspace = std::env::temp_dir().join(format!(
            "astrid-edge-action-conflicting-trace-{}",
            Uuid::new_v4()
        ));
        let config = test_config(&workspace);
        config.prepare_workspace().unwrap();
        let expected =
            IpcTraceContextV1::root(Uuid::new_v4(), "expected-session".to_string(), None);
        let mut conflicting = expected.clone();
        conflicting.trace_id = Uuid::new_v4();
        let response_sha256 = "a".repeat(64);
        fs::write(
            workspace.join("actions/dispatches.jsonl"),
            format!(
                "{}\n",
                serde_json::json!({
                    "schema": "astrid_edge_action_dispatch_v1",
                    "phase": "requested",
                    "recorded_at_unix_ms": 1,
                    "turn_id": expected.turn_id.unwrap(),
                    "response_sha256": response_sha256,
                    "trace": conflicting,
                })
            ),
        )
        .unwrap();

        let error = action_dispatch_evidence(&config, &expected, &response_sha256).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("conflicts with its expected exact trace")
        );

        fs::write(
            workspace.join("actions/dispatches.jsonl"),
            format!(
                "{}\n",
                serde_json::json!({
                    "schema": "astrid_edge_action_dispatch_v1",
                    "phase": "requested",
                    "recorded_at_unix_ms": 2,
                    "turn_id": expected.turn_id.unwrap(),
                    "response_sha256": "b".repeat(64),
                    "trace": expected,
                })
            ),
        )
        .unwrap();
        let error = action_dispatch_evidence(&config, &expected, &response_sha256).unwrap_err();
        assert!(error.to_string().contains("expected response hash"));
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn dispatch_evidence_rejects_an_action_receipt_with_the_wrong_parent_span() {
        let workspace = std::env::temp_dir().join(format!(
            "astrid-edge-action-wrong-parent-{}",
            Uuid::new_v4()
        ));
        let config = test_config(&workspace);
        config.prepare_workspace().unwrap();
        let expected =
            IpcTraceContextV1::root(Uuid::new_v4(), "expected-session".to_string(), None);
        let response_sha256 = "c".repeat(64);
        let key = super::ActionDispatchKey {
            turn_id: expected.turn_id.unwrap(),
            response_sha256: response_sha256.clone(),
            trace: expected.clone(),
        };
        append_action_dispatch_phase(&config, &key, "requested", 1).unwrap();
        append_action_dispatch_phase(&config, &key, "completed", 2).unwrap();
        let mut wrong_parent = expected.child();
        wrong_parent.parent_span_id = Some(Uuid::new_v4());
        fs::write(
            workspace.join("actions/receipts.jsonl"),
            format!(
                "{}\n",
                serde_json::json!({
                    "schema": "astrid_edge_action_receipt_v4",
                    "recorded_at_unix_ms": 2,
                    "session_id": "expected-session",
                    "response_sha256": response_sha256,
                    "declared_next": "LISTEN",
                    "decision_source": "astrid_declared",
                    "status": "honored",
                    "outcome": "listen_no_workspace_mutation",
                    "trace": wrong_parent,
                })
            ),
        )
        .unwrap();

        let error = action_dispatch_evidence(&config, &expected, &response_sha256).unwrap_err();
        assert!(error.to_string().contains("direct child"));
        fs::remove_dir_all(workspace).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn dispatch_ledger_symlink_is_rejected_without_touching_its_target() {
        let workspace = std::env::temp_dir().join(format!(
            "astrid-edge-action-ledger-symlink-{}",
            Uuid::new_v4()
        ));
        let config = test_config(&workspace);
        config.prepare_workspace().unwrap();
        let outside = workspace.with_extension("outside-dispatch-ledger");
        fs::write(&outside, "operator-owned\n").unwrap();
        std::os::unix::fs::symlink(&outside, workspace.join("actions/dispatches.jsonl")).unwrap();
        let response = "A bounded observation.\nNEXT: NOTICE refuse this dispatch".to_string();
        let candidate = ActionCandidate {
            session_id: "action-symlink-session".to_string(),
            response: response.clone(),
            trace: Some(IpcTraceContextV1::root(
                Uuid::new_v4(),
                "action-symlink-session".to_string(),
                None,
            )),
            tuning_authority_turn_id: None,
            tuning_authority_source: None,
            maintenance_permit: None,
        };
        let response_sha256 = format!("{:x}", Sha256::digest(response.as_bytes()));

        let error =
            begin_action_dispatch(&config, &candidate, &response_sha256, super::unix_millis())
                .unwrap_err();
        assert!(error.to_string().contains("not a regular non-symlink file"));
        assert_eq!(fs::read_to_string(&outside).unwrap(), "operator-owned\n");
        fs::remove_file(outside).unwrap();
        fs::remove_dir_all(workspace).unwrap();
    }
}
