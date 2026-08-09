//! Ordered scheduled-authorship transaction orchestrator.
//!
//! This module deliberately keeps gate, lock, provider, tool, attestation, and durable
//! finalization ordering together: that ordering is the crash-safety invariant. Pure prompt
//! construction and durable transaction schemas live in dedicated modules.

use std::path::PathBuf;

use fs2::FileExt;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::attestation::{
    ENVELOPE_SCHEMA, HmacSigner, INTENT_SCHEMA, SupervisorIntent, completed_envelope, envelope,
};
use crate::authored_transaction::{AuthoredTransaction, CandidatePublication, SourceReviewOutcome};
use crate::candidate::{CandidateManager, SubmittedCandidate};
use crate::candidate_ledger::EventContext;
use crate::config::Config;
use crate::context_provenance::ContextProvenance;
use crate::gate;
use crate::owned;
use crate::provider::{Message, Provider};
use crate::reporting::{project_scheduled_contract, workspace_write_exact};
use crate::source::SourceSnapshot;
use crate::util::{
    append_private, atomic_private_write, bounded_text, canonical_json, ensure_private_dir, sha256,
    unix_seconds, validate_hex64, validate_identifier,
};
use crate::{Error, RECEIPT_SCHEMA, REFLECTION_SCHEMA, Result};

const MAX_MODEL_STEPS: usize = 8;
const DEFAULT_QUESTION: &str = "What do my recent experience, evidence, source, and limitations suggest I should understand or improve next?";

#[derive(Debug, Clone, Default)]
pub struct RunRequest {
    /// Optional timer-slot identifier in `due-<decimal>` form.
    pub due_nonce: Option<String>,
    /// Optional bounded question for this scheduled reflection.
    pub question: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunResult {
    /// Terminal status, including explicit deferral states.
    pub status: String,
    /// Coalescing nonce for the scheduled slot.
    pub due_nonce: String,
    /// Direct-provider trace ID when a model turn began.
    pub trace_id: Option<String>,
    /// Exact authored reflection path, if a reflection completed.
    pub reflection_path: Option<String>,
    /// Supervisor-compatible intent path, if exact submission completed.
    pub intent_path: Option<String>,
    /// Candidate ID bound to the intent, if one was emitted.
    pub candidate_id: Option<String>,
}

#[derive(Debug)]
enum ModelOutput {
    Tool(ToolCall),
    Final(String),
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolCall {
    name: String,
    arguments: Value,
}

#[derive(Debug)]
struct Terminal {
    candidate_id: String,
    candidate_sha256: String,
    declaration: String,
}

#[derive(Debug)]
struct ToolFlowPolicy {
    lane: ToolLane,
    provenance: ContextProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolLane {
    Rich,
    Clean,
}

impl ToolFlowPolicy {
    fn rich(provenance: ContextProvenance) -> Self {
        Self {
            lane: ToolLane::Rich,
            provenance,
        }
    }

    fn clean() -> Self {
        Self {
            lane: ToolLane::Clean,
            provenance: ContextProvenance::clean(),
        }
    }

    fn authorize(&mut self, name: &str, web_available: bool) -> Result<()> {
        match (self.lane, name) {
            (ToolLane::Rich, "inspect_owned" | "read_owned") => Ok(()),
            (ToolLane::Rich, "search_web" | "fetch_web") if web_available => Ok(()),
            (ToolLane::Rich, "search_web" | "fetch_web") if !web_available => {
                Err(Error::new("model requested an unadvertised tool"))
            },
            (
                ToolLane::Clean,
                "list_source"
                | "search_source"
                | "read_source_chunk"
                | "read_generation_diff"
                | "read_build_evidence"
                | "begin_candidate"
                | "apply_candidate_patch"
                | "inspect_candidate"
                | "format_candidate"
                | "abandon_candidate"
                | "submit_candidate",
            ) if self.provenance.candidate_authoring_eligible() => Ok(()),
            _ => Err(Error::new("model requested an unadvertised tool")),
        }
    }

    fn record_result(&mut self, name: &str, result: &Value) -> Result<()> {
        if matches!(
            name,
            "search_web" | "fetch_web" | "inspect_owned" | "read_owned"
        ) {
            self.provenance
                .mark_untrusted(name, &sha256(&canonical_json(result)?))?;
        }
        Ok(())
    }

    fn provenance(&self) -> &ContextProvenance {
        &self.provenance
    }
}

struct UnattestedSubmission<'manager, 'snapshot> {
    manager: &'manager CandidateManager<'snapshot>,
    proposal_binding: &'manager str,
    armed: bool,
}

impl<'manager, 'snapshot> UnattestedSubmission<'manager, 'snapshot> {
    fn new(
        manager: &'manager CandidateManager<'snapshot>,
        proposal_binding: &'manager str,
    ) -> Self {
        Self {
            manager,
            proposal_binding,
            armed: false,
        }
    }

    fn arm(&mut self) {
        self.armed = true;
    }

    const fn is_armed(&self) -> bool {
        self.armed
    }

    fn commit(&mut self) {
        self.armed = false;
    }
}

impl Drop for UnattestedSubmission<'_, '_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.manager.reopen_unattested(self.proposal_binding);
        }
    }
}

#[derive(Debug)]
struct EmittedIntent {
    path: PathBuf,
    envelope_id: String,
    envelope_sha256: String,
    intent_id: String,
    trace_id: String,
    session_id: String,
    turn_id: String,
    response_sha256: String,
    terminal_declaration_sha256: String,
    candidate_id: String,
    candidate_sha256: String,
}

/// Publish a deliberately non-authorizing wakeup after the exact signed intent is durable.
///
/// The immutable supervisor still has to authenticate and consume the adjacent intent envelope;
/// this marker contains no candidate body, signature, command, or deployment authority.  It is
/// omitted when the supervisor was operator-paused at the reflection's pre-provider snapshot.
fn publish_supervisor_handoff_trigger(
    appliance_id: &str,
    supervisor_inbox: &std::path::Path,
    supervisor_mode: Option<&str>,
    created_at: u64,
    emitted: Option<&EmittedIntent>,
) -> Result<&'static str> {
    let Some(emitted) = emitted else {
        return Ok("not_applicable_no_intent");
    };
    if supervisor_mode != Some("running") {
        return Ok("queued_operator_paused_no_trigger");
    }
    validate_identifier(appliance_id, "handoff appliance_id")?;
    for (value, label) in [
        (&emitted.envelope_id, "handoff envelope_id"),
        (&emitted.intent_id, "handoff intent_id"),
        (&emitted.candidate_id, "handoff candidate_id"),
    ] {
        validate_identifier(value, label)?;
    }
    for (value, label) in [
        (&emitted.envelope_sha256, "handoff envelope_sha256"),
        (&emitted.response_sha256, "handoff response_sha256"),
        (&emitted.candidate_sha256, "handoff candidate_sha256"),
    ] {
        validate_hex64(value, label)?;
    }
    let value = serde_json::json!({
        "schema": "astrid.edge.steward_helper.supervisor_handoff_trigger.v1",
        "appliance_id": appliance_id,
        "envelope_id": emitted.envelope_id,
        "envelope_sha256": emitted.envelope_sha256,
        "intent_id": emitted.intent_id,
        "candidate_id": emitted.candidate_id,
        "candidate_sha256": emitted.candidate_sha256,
        "response_sha256": emitted.response_sha256,
        "created_at": created_at,
        "provenance": "exact_model_intent_already_published",
        "authority": "trigger_only_no_candidate_or_deployment_authority"
    });
    let mut encoded = canonical_json(&value)?;
    encoded.push(b'\n');
    // The mutable steward may only publish an inert pending marker.  The
    // immutable reflection cleanup promotes it to the watched `.json` name
    // after the admission lease and model lock have both been released.
    let path = supervisor_inbox.join(format!("candidate-ready-{}.pending", emitted.envelope_id));
    if path.exists() || path.is_symlink() {
        if path.is_symlink() || crate::util::read_stable_regular(&path, 8_192)? != encoded {
            return Err(Error::new(
                "supervisor handoff trigger collides with non-identical content",
            ));
        }
        return Ok("already_published_pending_root_cleanup_trigger");
    }
    atomic_private_write(&path, &encoded)?;
    Ok("published_pending_root_cleanup_trigger")
}

#[derive(Debug)]
struct PreparedIntent {
    envelope: Value,
    binding: Value,
    envelope_id: String,
    envelope_sha256: String,
    intent_id: String,
    trace_id: String,
    session_id: String,
    turn_id: String,
    response_sha256: String,
    terminal_declaration: String,
    terminal_declaration_sha256: String,
    candidate_id: String,
    candidate_sha256: String,
    source_id: String,
    base_generation: String,
}

#[derive(Debug)]
struct CleanReview {
    outcome: SourceReviewOutcome,
    candidate: Option<CandidatePublication>,
    prepared_proposal_binding: Option<String>,
}

const SOURCE_REVIEW_OUTCOME_SCHEMA: &str = "astrid.edge.steward_helper.source_review_outcome.v1";
const SOURCE_REVIEW_AUTHORITY: &str =
    "fresh_clean_context_no_rich_owned_or_web_content_candidate_authority_only_when_attested";

/// Run one fail-closed scheduled reflection.
///
/// # Errors
///
/// Returns an error if immutable configuration, owned evidence, source integrity,
/// provider transport, candidate validation, or durable persistence fails.
pub fn run_once(config: &Config, request: RunRequest) -> Result<RunResult> {
    run_once_with_reflection_validator(
        config,
        request,
        crate::reflection::require,
        crate::reflection::require_model_start,
    )
}

/// Integration-test entry point. Release appliance binaries do not contain
/// this symbol; production always validates root-issued reflection admission.
#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn run_once_without_root_guard_for_test(
    config: &Config,
    request: RunRequest,
) -> Result<RunResult> {
    run_once_with_reflection_validator(config, request, |_| Ok(()), |_, _| Ok(()))
}

#[allow(clippy::too_many_lines)] // Scheduling, coalescing, gate, and lock form one fail-closed admission transaction.
fn run_once_with_reflection_validator(
    config: &Config,
    request: RunRequest,
    require_reflection: fn(&Config) -> Result<()>,
    require_model_start: fn(&Config, &str) -> Result<()>,
) -> Result<RunResult> {
    config.validate()?;
    ensure_private_dir(&config.state_root)?;
    let (due_nonce, automatic_schedule, model_floor_until) =
        match crate::schedule::decide(config, request.due_nonce)? {
            crate::schedule::Decision::NotDue {
                next_due_at_unix_seconds,
            } => {
                return Ok(RunResult {
                    status: format!("not_due_until:{next_due_at_unix_seconds}"),
                    due_nonce: format!("due-{next_due_at_unix_seconds}"),
                    trace_id: None,
                    reflection_path: None,
                    intent_path: None,
                    candidate_id: None,
                });
            },
            crate::schedule::Decision::ModelFloor {
                nonce,
                next_model_eligible_at_unix_seconds,
            } => (nonce, true, Some(next_model_eligible_at_unix_seconds)),
            crate::schedule::Decision::Due { nonce, automatic } => (nonce, automatic, None),
        };
    validate_due_nonce(&due_nonce)?;
    let signer = HmacSigner::from_file(&config.attestor_key)?;
    let completed = pre_provider_step(
        config,
        &due_nonce,
        "authored_recovery_integrity_failed_non_authored",
        "completion marker validation failed before any provider call",
        crate::authored_transaction::verify_completion(config, &signer, &due_nonce),
    )?;
    let prepared = pre_provider_step(
        config,
        &due_nonce,
        "authored_recovery_integrity_failed_non_authored",
        "prepared authored transaction validation failed before any provider call",
        crate::authored_transaction::load(config, &signer, &due_nonce),
    )?;
    let rich_checkpoint = pre_provider_step(
        config,
        &due_nonce,
        "rich_checkpoint_integrity_failed_non_authored",
        "rich reflection checkpoint validation failed before any provider call",
        crate::source_review::load_rich(config, &signer, &due_nonce),
    )?;
    if let Some(transaction) = prepared {
        if let Err(error) = require_reflection(config) {
            if automatic_schedule && crate::reflection::artifacts_absent()? {
                return Ok(root_admission_boundary_deferral(
                    due_nonce,
                    Some(transaction.trace_id),
                ));
            }
            return Err(error);
        }
        if let Some(deferred) = maintenance_deferral(
            config,
            &due_nonce,
            "deferred_authored_transaction_recovery_maintenance",
            crate::maintenance::inspect(config),
        )? {
            return Ok(deferred);
        }
        let model_lock = crate::model_lock::open(config)?;
        if model_lock.try_lock_exclusive().is_err() {
            return Ok(RunResult {
                status: "deferred: model lock is held during authored recovery".to_owned(),
                due_nonce,
                trace_id: Some(transaction.trace_id),
                reflection_path: None,
                intent_path: None,
                candidate_id: None,
            });
        }
        let result = finalize_authored_transaction(config, &signer, &transaction);
        let _ = FileExt::unlock(&model_lock);
        if result.is_ok() {
            crate::schedule::complete(config, &due_nonce, automatic_schedule)?;
        }
        return result;
    }
    if let Some(transaction) = rich_checkpoint {
        if let Err(error) = require_reflection(config) {
            if automatic_schedule && crate::reflection::artifacts_absent()? {
                return Ok(root_admission_boundary_deferral(
                    due_nonce,
                    Some(transaction.trace_id),
                ));
            }
            return Err(error);
        }
        if let Some(deferred) = maintenance_deferral(
            config,
            &due_nonce,
            "deferred_rich_checkpoint_recovery_maintenance",
            crate::maintenance::inspect(config),
        )? {
            return Ok(deferred);
        }
        let model_lock = crate::model_lock::open(config)?;
        if model_lock.try_lock_exclusive().is_err() {
            return Ok(RunResult {
                status: "deferred: model lock is held during rich checkpoint recovery".to_owned(),
                due_nonce,
                trace_id: Some(transaction.trace_id),
                reflection_path: None,
                intent_path: None,
                candidate_id: None,
            });
        }
        let result = recover_rich_checkpoint(config, &signer, transaction)
            .and_then(|transaction| finalize_authored_transaction(config, &signer, &transaction));
        let _ = FileExt::unlock(&model_lock);
        if result.is_ok() {
            crate::schedule::complete(config, &due_nonce, automatic_schedule)?;
        }
        return result;
    }
    if completed && model_floor_until.is_some() {
        crate::schedule::complete(config, &due_nonce, automatic_schedule)?;
        return Ok(RunResult {
            status: "already_completed_coalesced".to_owned(),
            due_nonce,
            trace_id: None,
            reflection_path: None,
            intent_path: None,
            candidate_id: None,
        });
    }
    if let Some(next_model_eligible_at_unix_seconds) = model_floor_until {
        return Ok(RunResult {
            status: format!("model_floor_until:{next_model_eligible_at_unix_seconds}"),
            due_nonce,
            trace_id: None,
            reflection_path: None,
            intent_path: None,
            candidate_id: None,
        });
    }
    if let Err(error) = require_reflection(config) {
        if automatic_schedule && crate::reflection::artifacts_absent()? {
            return Ok(root_admission_boundary_deferral(due_nonce, None));
        }
        return Err(error);
    }
    if let Some(deferred) = maintenance_deferral(
        config,
        &due_nonce,
        "deferred_maintenance_pre_lock",
        crate::maintenance::inspect(config),
    )? {
        return Ok(deferred);
    }
    let lifecycle = pre_provider_step(
        config,
        &due_nonce,
        "candidate_lifecycle_validation_failed_non_authored",
        "candidate/supervisor lifecycle validation failed before any provider call",
        crate::lifecycle::reconcile(config),
    )?;
    match lifecycle {
        crate::lifecycle::LifecycleCheck::Deferred { reason } => {
            record_receipt(
                config,
                &receipt_core(
                    &due_nonce,
                    None,
                    "deferred_candidate_lifecycle",
                    &reason,
                    &[],
                    None,
                    None,
                ),
            )?;
            return Ok(RunResult {
                status: format!("deferred: {reason}"),
                due_nonce,
                trace_id: None,
                reflection_path: None,
                intent_path: None,
                candidate_id: None,
            });
        },
        crate::lifecycle::LifecycleCheck::Reconciled { candidate_id } => {
            record_receipt(
                config,
                &receipt_core(
                    &due_nonce,
                    None,
                    "candidate_terminal_reconciled",
                    &candidate_id,
                    &[],
                    None,
                    None,
                ),
            )?;
        },
        crate::lifecycle::LifecycleCheck::Ready => {},
    }
    if completed {
        crate::schedule::complete(config, &due_nonce, automatic_schedule)?;
        return Ok(RunResult {
            status: "already_completed_coalesced".to_owned(),
            due_nonce,
            trace_id: None,
            reflection_path: None,
            intent_path: None,
            candidate_id: None,
        });
    }
    let question = request
        .question
        .unwrap_or_else(|| DEFAULT_QUESTION.to_owned());
    if question.trim().is_empty()
        || question.trim() != question
        || question.chars().count() > 240
        || has_ambiguous_controls(&question)
    {
        let error = Error::new("scheduled introspection question exceeds 240 characters");
        record_terminal_receipt(
            config,
            &due_nonce,
            None,
            "scheduled_question_rejected_non_authored",
            "scheduled question failed its exact bounded plain-text policy",
            &[],
            None,
            None,
        )?;
        return Err(error);
    }
    let gate = pre_provider_step(
        config,
        &due_nonce,
        "runtime_gate_validation_failed_non_authored",
        "runtime gate evidence failed validation before any provider call",
        gate::inspect(&config.gates),
    )?;
    if !gate.ready {
        record_receipt(
            config,
            &receipt_core(&due_nonce, None, "deferred", &gate.reason, &[], None, None),
        )?;
        return Ok(RunResult {
            status: format!("deferred: {}", gate.reason),
            due_nonce,
            trace_id: None,
            reflection_path: None,
            intent_path: None,
            candidate_id: None,
        });
    }
    let model_lock = pre_provider_step(
        config,
        &due_nonce,
        "model_lock_validation_failed_non_authored",
        "model lock identity failed before any provider call",
        crate::model_lock::open(config),
    )?;
    if model_lock.try_lock_exclusive().is_err() {
        return Ok(RunResult {
            status: "deferred: model lock is held".to_owned(),
            due_nonce,
            trace_id: None,
            reflection_path: None,
            intent_path: None,
            candidate_id: None,
        });
    }
    if let Err(error) = require_reflection(config) {
        record_terminal_receipt(
            config,
            &due_nonce,
            None,
            "reflection_authority_revoked_non_authored",
            "immutable reflection admission failed before any provider call",
            &[],
            None,
            None,
        )?;
        let _ = FileExt::unlock(&model_lock);
        return Err(error);
    }
    if let Some(deferred) = maintenance_deferral(
        config,
        &due_nonce,
        "deferred_maintenance_post_lock",
        crate::maintenance::inspect(config),
    )? {
        let _ = FileExt::unlock(&model_lock);
        return Ok(deferred);
    }
    let result = run_locked(
        config,
        &due_nonce,
        &question,
        gate.thermal_celsius,
        automatic_schedule,
        require_reflection,
        require_model_start,
    );
    let _ = FileExt::unlock(&model_lock);
    if result
        .as_ref()
        .is_ok_and(|completed| completed.status == "authored_completed")
    {
        crate::schedule::complete(config, &due_nonce, automatic_schedule)?;
    }
    result
}

fn pre_provider_step<T>(
    config: &Config,
    due_nonce: &str,
    status: &str,
    detail: &str,
    result: Result<T>,
) -> Result<T> {
    match result {
        Ok(value) => Ok(value),
        Err(error) => {
            record_terminal_receipt(config, due_nonce, None, status, detail, &[], None, None)?;
            Err(error)
        },
    }
}

fn root_admission_boundary_deferral(due_nonce: String, trace_id: Option<String>) -> RunResult {
    RunResult {
        status: "deferred: root reflection admission boundary not yet available".to_owned(),
        due_nonce,
        trace_id,
        reflection_path: None,
        intent_path: None,
        candidate_id: None,
    }
}

fn maintenance_deferral(
    config: &Config,
    due_nonce: &str,
    receipt_status: &str,
    gate: crate::maintenance::Gate,
) -> Result<Option<RunResult>> {
    if gate.is_clear() {
        return Ok(None);
    }
    let reason = gate.reason();
    record_receipt(
        config,
        &receipt_core(due_nonce, None, receipt_status, reason, &[], None, None),
    )?;
    Ok(Some(RunResult {
        status: format!("deferred: {reason}"),
        due_nonce: due_nonce.to_owned(),
        trace_id: None,
        reflection_path: None,
        intent_path: None,
        candidate_id: None,
    }))
}

fn maintenance_interruption(
    config: &Config,
    due_nonce: &str,
    trace_id: &str,
    tools_used: &[String],
    prompt_sha256: &str,
    gate: crate::maintenance::Gate,
) -> Result<Option<RunResult>> {
    if gate.is_clear() {
        return Ok(None);
    }
    let reason = gate.reason();
    record_terminal_receipt(
        config,
        due_nonce,
        Some(trace_id),
        "interrupted_by_maintenance_non_authored",
        reason,
        tools_used,
        Some(prompt_sha256),
        None,
    )?;
    Ok(Some(RunResult {
        status: "interrupted_by_maintenance_non_authored".to_owned(),
        due_nonce: due_nonce.to_owned(),
        trace_id: Some(trace_id.to_owned()),
        reflection_path: None,
        intent_path: None,
        candidate_id: None,
    }))
}

fn pending_source_review() -> SourceReviewOutcome {
    SourceReviewOutcome {
        schema: SOURCE_REVIEW_OUTCOME_SCHEMA.to_owned(),
        status: "requested_pending_clean".to_owned(),
        turn_id: None,
        span_id: None,
        prompt_sha256: None,
        prompt_chars: 0,
        response_sha256: None,
        exact_candidate_author_response: None,
        tools_used: Vec::new(),
        provider_calls: 0,
        prompt_tokens: 0,
        completion_tokens: 0,
        provider_elapsed_ms: 0,
        failure_class: None,
        context_provenance: ContextProvenance::clean(),
        authority: SOURCE_REVIEW_AUTHORITY.to_owned(),
    }
}

fn parse_source_review_request(response: &str) -> Result<bool> {
    let lines = response.lines().collect::<Vec<_>>();
    let markers = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("SOURCE_REVIEW:"))
        .collect::<Vec<_>>();
    if markers.is_empty() {
        return Ok(false);
    }
    if markers.len() == 1
        && markers[0].0 == lines.len().saturating_sub(1)
        && *markers[0].1 == "SOURCE_REVIEW: REQUEST"
    {
        return Ok(true);
    }
    Err(Error::new(
        "source review request must be the exact final non-authorizing marker",
    ))
}

#[allow(clippy::too_many_arguments)]
fn clean_failure(
    config: &Config,
    rich: &AuthoredTransaction,
    start: Option<&crate::source_review::CleanStart>,
    tools_used: Vec<String>,
    provider_calls: u8,
    prompt_tokens: u64,
    completion_tokens: u64,
    provider_elapsed_ms: u64,
    response_sha256: Option<String>,
    failure_class: &str,
) -> Result<CleanReview> {
    let failure_class = bounded_text(failure_class, 320);
    record_terminal_receipt(
        config,
        &rich.due_nonce,
        Some(&rich.trace_id),
        "clean_source_review_failed_non_authored",
        &failure_class,
        &tools_used,
        start.map(|value| value.prompt_sha256.as_str()),
        response_sha256.as_deref(),
    )?;
    Ok(CleanReview {
        outcome: SourceReviewOutcome {
            schema: SOURCE_REVIEW_OUTCOME_SCHEMA.to_owned(),
            status: "failed_non_authored".to_owned(),
            turn_id: start.map(|value| value.turn_id.clone()),
            span_id: start.map(|value| value.span_id.clone()),
            prompt_sha256: start.map(|value| value.prompt_sha256.clone()),
            prompt_chars: start.map_or(0, |value| value.prompt_chars),
            response_sha256,
            exact_candidate_author_response: None,
            tools_used,
            provider_calls,
            prompt_tokens,
            completion_tokens,
            provider_elapsed_ms,
            failure_class: Some(failure_class),
            context_provenance: ContextProvenance::clean(),
            authority: SOURCE_REVIEW_AUTHORITY.to_owned(),
        },
        candidate: None,
        prepared_proposal_binding: None,
    })
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn run_clean_source_review(
    config: &Config,
    question: &str,
    snapshot: &SourceSnapshot,
    active_generation: &str,
    candidate: &CandidateManager<'_>,
    rich: &AuthoredTransaction,
    signer: &HmacSigner,
    require_reflection: fn(&Config) -> Result<()>,
    combined_provider_calls: &mut usize,
) -> Result<CleanReview> {
    if *combined_provider_calls >= MAX_MODEL_STEPS {
        return clean_failure(
            config,
            rich,
            None,
            Vec::new(),
            0,
            0,
            0,
            0,
            None,
            "combined eight-call budget exhausted by rich reflection before clean review",
        );
    }
    let fresh_source = SourceSnapshot::load_for_active_generation(config);
    let (fresh_snapshot, fresh_generation) = match fresh_source {
        Ok(value) => value,
        Err(error) => {
            return clean_failure(
                config,
                rich,
                None,
                Vec::new(),
                0,
                0,
                0,
                0,
                None,
                &format!(
                    "fresh signed source validation failed: {}",
                    bounded_failure_class(&error)
                ),
            );
        },
    };
    if fresh_snapshot.source_id != snapshot.source_id || fresh_generation != active_generation {
        return clean_failure(
            config,
            rich,
            None,
            Vec::new(),
            0,
            0,
            0,
            0,
            None,
            "signed source or active generation changed between rich and clean passes",
        );
    }
    let candidate_status = match candidate.prompt_status() {
        Ok(value) => value,
        Err(error) => {
            return clean_failure(
                config,
                rich,
                None,
                Vec::new(),
                0,
                0,
                0,
                0,
                None,
                &format!(
                    "candidate status validation failed: {}",
                    bounded_failure_class(&error)
                ),
            );
        },
    };
    let supervisor_status = match crate::prompt::supervisor_status(config) {
        Ok(value) => value,
        Err(error) => {
            return clean_failure(
                config,
                rich,
                None,
                Vec::new(),
                0,
                0,
                0,
                0,
                None,
                &format!(
                    "supervisor status validation failed: {}",
                    bounded_failure_class(&error)
                ),
            );
        },
    };
    let system = crate::prompt::clean_system_instruction();
    let message_budget = match crate::prompt::source_authoring_message_budget_chars(config) {
        Ok(value) => value,
        Err(error) => {
            return clean_failure(
                config,
                rich,
                None,
                Vec::new(),
                0,
                0,
                0,
                0,
                None,
                &format!(
                    "clean message budget failed: {}",
                    bounded_failure_class(&error)
                ),
            );
        },
    };
    let prompt_budget =
        match crate::prompt::initial_prompt_budget(message_budget, system.chars().count()) {
            Ok(value) => value,
            Err(error) => {
                return clean_failure(
                    config,
                    rich,
                    None,
                    Vec::new(),
                    0,
                    0,
                    0,
                    0,
                    None,
                    &format!(
                        "clean prompt reserve failed: {}",
                        bounded_failure_class(&error)
                    ),
                );
            },
        };
    let prompt = match crate::prompt::build_clean(
        config,
        &rich.due_nonce,
        question,
        active_generation,
        snapshot,
        &candidate_status,
        &supervisor_status,
        prompt_budget,
    ) {
        Ok(value) => value,
        Err(error) => {
            return clean_failure(
                config,
                rich,
                None,
                Vec::new(),
                0,
                0,
                0,
                0,
                None,
                &format!(
                    "clean prompt construction failed: {}",
                    bounded_failure_class(&error)
                ),
            );
        },
    };
    let turn_id = Uuid::new_v4().to_string();
    let span_id = Uuid::new_v4().to_string();
    let prompt_sha256 = sha256(prompt.as_bytes());
    let started_at_unix_ms = unix_millis();
    let start = crate::source_review::CleanStart::new(
        config,
        rich,
        turn_id.clone(),
        span_id.clone(),
        prompt_sha256.clone(),
        prompt.chars().count(),
        started_at_unix_ms,
    )?;
    crate::source_review::mark_clean_started(config, signer, &start)?;
    let clean_context = ContextProvenance::clean();
    let clean_context_sha256 = clean_context.digest()?;
    let proposal_binding = sha256(&canonical_json(&serde_json::json!({
        "schema": "astrid.edge.steward_helper.clean_source_proposal_binding.v1",
        "appliance_id": config.appliance_id,
        "due_nonce": rich.due_nonce,
        "trace_id": rich.trace_id,
        "session_id": rich.session_id,
        "turn_id": turn_id,
        "model": config.model,
        "prompt_sha256": prompt_sha256,
        "source_id": snapshot.source_id,
        "base_generation": active_generation,
        "context_provenance_sha256": clean_context_sha256
    }))?);
    let mut messages = vec![
        Message {
            role: "system".to_owned(),
            content: system,
        },
        Message {
            role: "user".to_owned(),
            content: prompt,
        },
    ];
    if let Err(error) = crate::prompt::ensure_message_budget(&messages, message_budget) {
        return clean_failure(
            config,
            rich,
            Some(&start),
            Vec::new(),
            0,
            0,
            0,
            0,
            None,
            &format!(
                "clean model envelope failed: {}",
                bounded_failure_class(&error)
            ),
        );
    }
    let provider = Provider::new(config);
    let mut tools_used = Vec::new();
    let mut lane_provider_calls = 0_u8;
    let mut prompt_tokens = 0_u64;
    let mut completion_tokens = 0_u64;
    let mut provider_elapsed_ms = 0_u64;
    let mut tool_flow = ToolFlowPolicy::clean();
    let mut unattested_submission = UnattestedSubmission::new(candidate, &proposal_binding);
    let mut final_response = None;
    while *combined_provider_calls < MAX_MODEL_STEPS {
        if let Err(error) = require_reflection(config) {
            return clean_failure(
                config,
                rich,
                Some(&start),
                tools_used,
                lane_provider_calls,
                prompt_tokens,
                completion_tokens,
                provider_elapsed_ms,
                None,
                &format!(
                    "reflection authority revoked: {}",
                    bounded_failure_class(&error)
                ),
            );
        }
        let maintenance = crate::maintenance::inspect(config);
        if !maintenance.is_clear() {
            return clean_failure(
                config,
                rich,
                Some(&start),
                tools_used,
                lane_provider_calls,
                prompt_tokens,
                completion_tokens,
                provider_elapsed_ms,
                None,
                &format!(
                    "maintenance interrupted clean review: {}",
                    maintenance.reason()
                ),
            );
        }
        *combined_provider_calls = combined_provider_calls.saturating_add(1);
        lane_provider_calls = lane_provider_calls.saturating_add(1);
        let response = match provider
            .generate_with_output_tokens(&messages, config.source_authoring_output_tokens)
        {
            Ok(value) => value,
            Err(error) => {
                return clean_failure(
                    config,
                    rich,
                    Some(&start),
                    tools_used,
                    lane_provider_calls,
                    prompt_tokens,
                    completion_tokens,
                    provider_elapsed_ms,
                    None,
                    &format!("clean provider failed: {}", bounded_failure_class(&error)),
                );
            },
        };
        prompt_tokens = prompt_tokens.saturating_add(response.prompt_tokens.unwrap_or(0));
        completion_tokens =
            completion_tokens.saturating_add(response.completion_tokens.unwrap_or(0));
        provider_elapsed_ms = provider_elapsed_ms.saturating_add(response.elapsed_ms);
        let model_response_sha256 = sha256(response.content.as_bytes());
        let output = match parse_model_output(&response.content) {
            Ok(value) => value,
            Err(error) => {
                return clean_failure(
                    config,
                    rich,
                    Some(&start),
                    tools_used,
                    lane_provider_calls,
                    prompt_tokens,
                    completion_tokens,
                    provider_elapsed_ms,
                    Some(model_response_sha256),
                    &format!(
                        "clean model output malformed: {}",
                        bounded_failure_class(&error)
                    ),
                );
            },
        };
        match output {
            ModelOutput::Final(content) => {
                final_response = Some(content);
                break;
            },
            ModelOutput::Tool(tool) => {
                if let Err(error) = tool_flow.authorize(&tool.name, false) {
                    return clean_failure(
                        config,
                        rich,
                        Some(&start),
                        tools_used,
                        lane_provider_calls,
                        prompt_tokens,
                        completion_tokens,
                        provider_elapsed_ms,
                        Some(model_response_sha256),
                        &format!(
                            "clean tool authority rejected: {}",
                            bounded_failure_class(&error)
                        ),
                    );
                }
                let result = match execute_tool(
                    config,
                    snapshot,
                    active_generation,
                    candidate,
                    &tool,
                    &proposal_binding,
                    &rich.due_nonce,
                    &rich.trace_id,
                    &rich.session_id,
                    &turn_id,
                    &model_response_sha256,
                    &clean_context_sha256,
                    signer,
                ) {
                    Ok(value) => value,
                    Err(error) => {
                        return clean_failure(
                            config,
                            rich,
                            Some(&start),
                            tools_used,
                            lane_provider_calls,
                            prompt_tokens,
                            completion_tokens,
                            provider_elapsed_ms,
                            Some(model_response_sha256),
                            &format!("clean tool failed: {}", tool_failure_class(&error)),
                        );
                    },
                };
                if tool.name == "submit_candidate" {
                    inject_post_submit_crash_for_test(config)?;
                    unattested_submission.arm();
                }
                tool_flow.record_result(&tool.name, &result)?;
                tools_used.push(tool.name.clone());
                if let Err(error) = crate::prompt::replace_with_latest_tool_context(
                    &mut messages,
                    &tool.name,
                    &tool.arguments,
                    &result,
                    message_budget,
                ) {
                    return clean_failure(
                        config,
                        rich,
                        Some(&start),
                        tools_used,
                        lane_provider_calls,
                        prompt_tokens,
                        completion_tokens,
                        provider_elapsed_ms,
                        Some(model_response_sha256),
                        &format!(
                            "clean tool context failed: {}",
                            bounded_failure_class(&error)
                        ),
                    );
                }
            },
        }
    }
    let Some(response) = final_response else {
        return clean_failure(
            config,
            rich,
            Some(&start),
            tools_used,
            lane_provider_calls,
            prompt_tokens,
            completion_tokens,
            provider_elapsed_ms,
            None,
            "combined eight-call budget exhausted before a clean terminal response",
        );
    };
    let response_sha256 = sha256(response.as_bytes());
    let terminal = match parse_terminal(&response) {
        Ok(value) => value,
        Err(error) => {
            return clean_failure(
                config,
                rich,
                Some(&start),
                tools_used,
                lane_provider_calls,
                prompt_tokens,
                completion_tokens,
                provider_elapsed_ms,
                Some(response_sha256),
                &format!(
                    "clean terminal malformed: {}",
                    bounded_failure_class(&error)
                ),
            );
        },
    };
    let Some(terminal) = terminal else {
        if unattested_submission.is_armed() {
            return clean_failure(
                config,
                rich,
                Some(&start),
                tools_used,
                lane_provider_calls,
                prompt_tokens,
                completion_tokens,
                provider_elapsed_ms,
                Some(response_sha256),
                "submitted candidate lacks an exact terminal attestation",
            );
        }
        return Ok(CleanReview {
            outcome: SourceReviewOutcome {
                schema: SOURCE_REVIEW_OUTCOME_SCHEMA.to_owned(),
                status: "completed_no_candidate".to_owned(),
                turn_id: Some(turn_id),
                span_id: Some(span_id),
                prompt_sha256: Some(prompt_sha256),
                prompt_chars: start.prompt_chars,
                response_sha256: Some(response_sha256),
                exact_candidate_author_response: None,
                tools_used,
                provider_calls: lane_provider_calls,
                prompt_tokens,
                completion_tokens,
                provider_elapsed_ms,
                failure_class: None,
                context_provenance: clean_context,
                authority: SOURCE_REVIEW_AUTHORITY.to_owned(),
            },
            candidate: None,
            prepared_proposal_binding: None,
        });
    };
    let submitted = match candidate.submitted_for_terminal(
        &rich.due_nonce,
        &rich.trace_id,
        &rich.session_id,
        &turn_id,
        &clean_context_sha256,
        &proposal_binding,
    ) {
        Ok(value) => value,
        Err(error) => {
            return clean_failure(
                config,
                rich,
                Some(&start),
                tools_used,
                lane_provider_calls,
                prompt_tokens,
                completion_tokens,
                provider_elapsed_ms,
                Some(response_sha256),
                &format!(
                    "submitted candidate validation failed: {}",
                    bounded_failure_class(&error)
                ),
            );
        },
    };
    let prepared = match prepare_intent(
        config,
        &rich.due_nonce,
        &rich.trace_id,
        &rich.session_id,
        &turn_id,
        &response_sha256,
        &snapshot.source_id,
        &terminal,
        submitted,
        signer,
    ) {
        Ok(value) => value,
        Err(error) => {
            return clean_failure(
                config,
                rich,
                Some(&start),
                tools_used,
                lane_provider_calls,
                prompt_tokens,
                completion_tokens,
                provider_elapsed_ms,
                Some(response_sha256),
                &format!(
                    "clean attestation failed: {}",
                    bounded_failure_class(&error)
                ),
            );
        },
    };
    let publication = CandidatePublication {
        envelope: prepared.envelope,
        binding: prepared.binding,
        envelope_id: prepared.envelope_id,
        envelope_sha256: prepared.envelope_sha256,
        intent_id: prepared.intent_id,
        trace_id: prepared.trace_id,
        session_id: prepared.session_id,
        turn_id: prepared.turn_id,
        response_sha256: prepared.response_sha256,
        context_provenance_sha256: clean_context_sha256,
        terminal_declaration: prepared.terminal_declaration,
        terminal_declaration_sha256: prepared.terminal_declaration_sha256,
        candidate_id: prepared.candidate_id,
        candidate_sha256: prepared.candidate_sha256,
        source_id: prepared.source_id,
        base_generation: prepared.base_generation,
    };
    unattested_submission.commit();
    Ok(CleanReview {
        outcome: SourceReviewOutcome {
            schema: SOURCE_REVIEW_OUTCOME_SCHEMA.to_owned(),
            status: "candidate_attested".to_owned(),
            turn_id: Some(turn_id),
            span_id: Some(span_id),
            prompt_sha256: Some(prompt_sha256),
            prompt_chars: start.prompt_chars,
            response_sha256: Some(response_sha256),
            exact_candidate_author_response: Some(response),
            tools_used,
            provider_calls: lane_provider_calls,
            prompt_tokens,
            completion_tokens,
            provider_elapsed_ms,
            failure_class: None,
            context_provenance: clean_context,
            authority: SOURCE_REVIEW_AUTHORITY.to_owned(),
        },
        candidate: Some(publication),
        prepared_proposal_binding: Some(proposal_binding.clone()),
    })
}

fn recover_rich_checkpoint(
    config: &Config,
    signer: &HmacSigner,
    mut transaction: AuthoredTransaction,
) -> Result<AuthoredTransaction> {
    let pending = transaction
        .source_review
        .as_ref()
        .is_some_and(|review| review.status == "requested_pending_clean");
    if !pending || transaction.candidate.is_some() {
        return Err(Error::new(
            "rich checkpoint recovery is not a pending non-authorizing clean review",
        ));
    }
    let clean_start =
        crate::source_review::load_clean_start(config, signer, &transaction.due_nonce)?;
    let (snapshot, active_generation) = SourceSnapshot::load_for_active_generation(config)?;
    let candidate_state = config.state_root.join("candidate");
    let candidate_outbox = config.state_root.join("candidate-outbox");
    let candidate = CandidateManager::new(
        &candidate_state,
        &candidate_outbox,
        &snapshot,
        signer,
        &config.current_generation,
        &active_generation,
    )?;
    candidate.reconcile_orphan_prepared()?;
    let failure_class = if clean_start.is_some() {
        "clean_source_review_interrupted_after_signed_start_no_retry"
    } else {
        "clean_source_review_interrupted_before_signed_start_no_retry"
    };
    transaction.source_review = Some(SourceReviewOutcome {
        schema: SOURCE_REVIEW_OUTCOME_SCHEMA.to_owned(),
        status: "interrupted_by_restart_non_authored".to_owned(),
        turn_id: clean_start.as_ref().map(|value| value.turn_id.clone()),
        span_id: clean_start.as_ref().map(|value| value.span_id.clone()),
        prompt_sha256: clean_start
            .as_ref()
            .map(|value| value.prompt_sha256.clone()),
        prompt_chars: clean_start.as_ref().map_or(0, |value| value.prompt_chars),
        response_sha256: None,
        exact_candidate_author_response: None,
        tools_used: Vec::new(),
        // The signed start is written before the provider request. Count one
        // conservatively because a crash cannot prove whether the peer accepted
        // that request; recovery nevertheless never retries it.
        provider_calls: u8::from(clean_start.is_some()),
        prompt_tokens: 0,
        completion_tokens: 0,
        provider_elapsed_ms: 0,
        failure_class: Some(failure_class.to_owned()),
        context_provenance: ContextProvenance::clean(),
        authority: SOURCE_REVIEW_AUTHORITY.to_owned(),
    });
    transaction.candidate = None;
    transaction.unattested_proposal_binding = None;
    transaction.completed_at_unix_ms = unix_millis();
    record_terminal_receipt(
        config,
        &transaction.due_nonce,
        Some(&transaction.trace_id),
        "clean_source_review_interrupted_by_restart_non_authored",
        failure_class,
        &[],
        clean_start
            .as_ref()
            .map(|value| value.prompt_sha256.as_str()),
        None,
    )?;
    crate::authored_transaction::prepare(config, signer, &transaction)?;
    Ok(transaction)
}

#[allow(clippy::too_many_lines)] // One auditable model/attestation transaction boundary.
fn run_locked(
    config: &Config,
    due_nonce: &str,
    question: &str,
    thermal: u16,
    automatic_schedule: bool,
    require_reflection: fn(&Config) -> Result<()>,
    require_model_start: fn(&Config, &str) -> Result<()>,
) -> Result<RunResult> {
    let signer = HmacSigner::from_file(&config.attestor_key)?;
    let (snapshot, active_generation) = pre_provider_step(
        config,
        due_nonce,
        "source_snapshot_validation_failed_non_authored",
        "signed source/generation validation failed before any provider call",
        SourceSnapshot::load_for_active_generation(config),
    )?;
    let candidate_state = config.state_root.join("candidate");
    let candidate_outbox = config.state_root.join("candidate-outbox");
    let candidate = pre_provider_step(
        config,
        due_nonce,
        "candidate_state_validation_failed_non_authored",
        "candidate state validation failed before any provider call",
        CandidateManager::new(
            &candidate_state,
            &candidate_outbox,
            &snapshot,
            &signer,
            &config.current_generation,
            &active_generation,
        ),
    )?;
    pre_provider_step(
        config,
        due_nonce,
        "candidate_orphan_recovery_failed_non_authored",
        "orphan prepared candidate could not be returned to non-authorizing edit state",
        candidate.reconcile_orphan_prepared(),
    )?;
    let system = crate::prompt::rich_system_instruction(config.web_broker.is_some());
    let message_budget = pre_provider_step(
        config,
        due_nonce,
        "prompt_budget_validation_failed_non_authored",
        "configured model context could not reserve output and chat framing",
        crate::prompt::message_budget_chars(config),
    )?;
    let system_chars = system.chars().count();
    let prompt_budget = pre_provider_step(
        config,
        due_nonce,
        "prompt_budget_validation_failed_non_authored",
        "system context or tool-result reserve exhausted the immutable model input budget",
        crate::prompt::initial_prompt_budget(message_budget, system_chars),
    )?;
    let owned_projection = pre_provider_step(
        config,
        due_nonce,
        "owned_evidence_validation_failed_non_authored",
        "mandatory programmatic introspection failed before any provider call",
        owned::project_required(&config.owned_inputs, &config.workspace_root, question),
    )?;
    let mut rich_context_provenance = ContextProvenance::clean();
    rich_context_provenance
        .mark_untrusted("programmatic_owned_projection", &owned_projection.digest()?)?;
    let candidate_status = pre_provider_step(
        config,
        due_nonce,
        "candidate_state_validation_failed_non_authored",
        "candidate prompt projection failed before any provider call",
        candidate.prompt_status(),
    )?;
    let supervisor_status = pre_provider_step(
        config,
        due_nonce,
        "supervisor_status_validation_failed_non_authored",
        "supervisor prompt projection failed before any provider call",
        crate::prompt::supervisor_status(config),
    )?;
    let prompt = pre_provider_step(
        config,
        due_nonce,
        "prompt_budget_validation_failed_non_authored",
        "bounded scheduled prompt construction failed before any provider call",
        crate::prompt::build_rich(
            config,
            due_nonce,
            question,
            thermal,
            &active_generation,
            &snapshot,
            &owned_projection,
            &candidate_status,
            &supervisor_status,
            prompt_budget,
        ),
    )?;
    if let Some(deferred) = maintenance_deferral(
        config,
        due_nonce,
        "deferred_maintenance_pre_provider",
        crate::maintenance::inspect(config),
    )? {
        return Ok(deferred);
    }
    if let Err(error) = require_reflection(config) {
        record_terminal_receipt(
            config,
            due_nonce,
            None,
            "reflection_authority_revoked_non_authored",
            "immutable reflection admission failed before the provider call",
            &[],
            None,
            None,
        )?;
        return Err(error);
    }
    let started_at_unix_ms = unix_millis();
    let trace_id = Uuid::new_v4().to_string();
    let session_id = format!("session-{}", Uuid::new_v4().simple());
    let turn_id = Uuid::new_v4().to_string();
    let span_id = Uuid::new_v4().to_string();
    let prompt_chars = prompt.chars().count();
    let prompt_sha256 = sha256(prompt.as_bytes());
    let rich_non_authorizing_binding = sha256(&canonical_json(&serde_json::json!({
        "schema": "astrid.edge.steward_helper.rich_non_authorizing_binding.v1",
        "appliance_id": config.appliance_id,
        "due_nonce": due_nonce,
        "trace_id": trace_id,
        "session_id": session_id,
        "turn_id": turn_id,
        "model": config.model,
        "prompt_sha256": prompt_sha256,
        "source_id": snapshot.source_id,
        "base_generation": &active_generation
    }))?);
    let provider = Provider::new(config);
    let mut messages = vec![
        Message {
            role: "system".to_owned(),
            content: system,
        },
        Message {
            role: "user".to_owned(),
            content: prompt,
        },
    ];
    if let Err(error) = crate::prompt::ensure_message_budget(&messages, message_budget) {
        record_terminal_receipt(
            config,
            due_nonce,
            Some(&trace_id),
            "prompt_budget_validation_failed_non_authored",
            "final model message envelope exceeded its immutable context budget",
            &[],
            Some(&prompt_sha256),
            None,
        )?;
        return Err(error);
    }
    pre_provider_step(
        config,
        due_nonce,
        "root_model_start_authority_failed_non_authored",
        "root admission did not authorize a fresh model start for this exact due slot",
        require_model_start(config, due_nonce),
    )?;
    pre_provider_step(
        config,
        due_nonce,
        "model_start_floor_failed_non_authored",
        "durable two-hour model-start floor could not be consumed before the provider call",
        crate::schedule::begin_model_attempt(config, due_nonce, automatic_schedule),
    )?;
    let mut tools_used = Vec::new();
    let mut final_response = None;
    let mut prompt_tokens = 0_u64;
    let mut completion_tokens = 0_u64;
    let mut provider_elapsed_ms = 0_u64;
    let mut combined_provider_calls = 0_usize;
    let mut rich_provider_calls = 0_u8;
    let mut tool_flow = ToolFlowPolicy::rich(rich_context_provenance);
    while combined_provider_calls < MAX_MODEL_STEPS {
        if let Err(error) = require_reflection(config) {
            record_terminal_receipt(
                config,
                due_nonce,
                Some(&trace_id),
                "reflection_authority_revoked_non_authored",
                "immutable reflection admission failed before a complete response",
                &tools_used,
                Some(&prompt_sha256),
                None,
            )?;
            return Err(error);
        }
        if let Some(interrupted) = maintenance_interruption(
            config,
            due_nonce,
            &trace_id,
            &tools_used,
            &prompt_sha256,
            crate::maintenance::inspect(config),
        )? {
            return Ok(interrupted);
        }
        combined_provider_calls = combined_provider_calls.saturating_add(1);
        rich_provider_calls = rich_provider_calls.saturating_add(1);
        let response = match provider.generate(&messages) {
            Ok(response) => response,
            Err(error) => {
                record_terminal_receipt(
                    config,
                    due_nonce,
                    Some(&trace_id),
                    "provider_failed_non_authored",
                    &bounded_failure_class(&error),
                    &tools_used,
                    Some(&prompt_sha256),
                    None,
                )?;
                return Err(error);
            },
        };
        if let Err(error) = require_reflection(config) {
            record_terminal_receipt(
                config,
                due_nonce,
                Some(&trace_id),
                "reflection_authority_revoked_non_authored",
                "immutable reflection admission failed after provider completion",
                &tools_used,
                Some(&prompt_sha256),
                None,
            )?;
            return Err(error);
        }
        if let Some(interrupted) = maintenance_interruption(
            config,
            due_nonce,
            &trace_id,
            &tools_used,
            &prompt_sha256,
            crate::maintenance::inspect(config),
        )? {
            return Ok(interrupted);
        }
        prompt_tokens = prompt_tokens.saturating_add(response.prompt_tokens.unwrap_or(0));
        completion_tokens =
            completion_tokens.saturating_add(response.completion_tokens.unwrap_or(0));
        provider_elapsed_ms = provider_elapsed_ms.saturating_add(response.elapsed_ms);
        let model_response_sha256 = sha256(response.content.as_bytes());
        let output = match parse_model_output(&response.content) {
            Ok(output) => output,
            Err(error) => {
                record_terminal_receipt(
                    config,
                    due_nonce,
                    Some(&trace_id),
                    "malformed_model_output_non_authored",
                    "provider completion was empty, oversized, or malformed tool-shaped output",
                    &tools_used,
                    Some(&prompt_sha256),
                    Some(&model_response_sha256),
                )?;
                return Err(error);
            },
        };
        match output {
            ModelOutput::Final(content) => {
                final_response = Some(content);
                break;
            },
            ModelOutput::Tool(tool) => {
                if tools_used.len() >= MAX_MODEL_STEPS {
                    break;
                }
                if let Err(error) = tool_flow.authorize(&tool.name, config.web_broker.is_some()) {
                    record_terminal_receipt(
                        config,
                        due_nonce,
                        Some(&trace_id),
                        "tool_authority_rejected_non_authored",
                        "model requested a tool or tool-flow combination outside the advertised boundary",
                        &tools_used,
                        Some(&prompt_sha256),
                        Some(&model_response_sha256),
                    )?;
                    return Err(error);
                }
                let result = match execute_tool(
                    config,
                    &snapshot,
                    &active_generation,
                    &candidate,
                    &tool,
                    &rich_non_authorizing_binding,
                    due_nonce,
                    &trace_id,
                    &session_id,
                    &turn_id,
                    &model_response_sha256,
                    &tool_flow.provenance().digest()?,
                    &signer,
                ) {
                    Ok(result) => result,
                    Err(error) => {
                        record_terminal_receipt(
                            config,
                            due_nonce,
                            Some(&trace_id),
                            "tool_execution_failed_non_authored",
                            &tool_failure_class(&error),
                            &tools_used,
                            Some(&prompt_sha256),
                            Some(&model_response_sha256),
                        )?;
                        return Err(error);
                    },
                };
                tool_flow.record_result(&tool.name, &result)?;
                tools_used.push(tool.name.clone());
                if let Err(error) = crate::prompt::replace_with_latest_tool_context(
                    &mut messages,
                    &tool.name,
                    &tool.arguments,
                    &result,
                    message_budget,
                ) {
                    record_terminal_receipt(
                        config,
                        due_nonce,
                        Some(&trace_id),
                        "tool_context_budget_failed_non_authored",
                        "bounded tool result could not fit the immutable model context",
                        &tools_used,
                        Some(&prompt_sha256),
                        Some(&model_response_sha256),
                    )?;
                    return Err(error);
                }
            },
        }
    }
    let Some(response) = final_response else {
        record_terminal_receipt(
            config,
            due_nonce,
            Some(&trace_id),
            "tool_loop_exhausted_non_authored",
            "no terminal reflection within eight direct model completions",
            &tools_used,
            Some(&prompt_sha256),
            None,
        )?;
        return Ok(RunResult {
            status: "tool_loop_exhausted_non_authored".to_owned(),
            due_nonce: due_nonce.to_owned(),
            trace_id: Some(trace_id),
            reflection_path: None,
            intent_path: None,
            candidate_id: None,
        });
    };
    if let Some(interrupted) = maintenance_interruption(
        config,
        due_nonce,
        &trace_id,
        &tools_used,
        &prompt_sha256,
        crate::maintenance::inspect(config),
    )? {
        return Ok(interrupted);
    }
    let response_sha256 = sha256(response.as_bytes());
    let context_provenance = tool_flow.provenance().clone();
    let source_review_requested = parse_source_review_request(&response)?;
    let summary = bounded_summary(&response);
    let prepared_at_unix_ms = unix_millis();
    let mut transaction = AuthoredTransaction {
        schema: crate::authored_transaction::CORE_SCHEMA.to_owned(),
        prepared_at_unix_ms,
        completed_at_unix_ms: prepared_at_unix_ms,
        started_at_unix_ms,
        appliance_id: config.appliance_id.clone(),
        model: config.model.clone(),
        due_nonce: due_nonce.to_owned(),
        trace_id: trace_id.clone(),
        session_id: session_id.clone(),
        turn_id: turn_id.clone(),
        span_id,
        prompt_sha256,
        prompt_chars,
        response,
        response_sha256,
        summary_sha256: sha256(summary.as_bytes()),
        summary,
        tools_used,
        provider_calls: rich_provider_calls,
        prompt_tokens,
        completion_tokens,
        provider_elapsed_ms,
        context_provenance,
        source_review: source_review_requested.then(pending_source_review),
        candidate: None,
        unattested_proposal_binding: None,
        provenance: "model_authored_runtime_scheduled".to_owned(),
        authority:
            "rich_authored_response_with_optional_separate_clean_source_review_idempotent_publication"
                .to_owned(),
    };
    let mut prepared_proposal_binding = None;
    if source_review_requested {
        crate::source_review::persist_rich(config, &signer, &transaction)?;
        let clean = run_clean_source_review(
            config,
            question,
            &snapshot,
            &active_generation,
            &candidate,
            &transaction,
            &signer,
            require_reflection,
            &mut combined_provider_calls,
        )?;
        transaction.source_review = Some(clean.outcome);
        transaction.candidate = clean.candidate;
        prepared_proposal_binding = clean.prepared_proposal_binding;
    }
    if let Err(error) = crate::authored_transaction::prepare(config, &signer, &transaction) {
        if let Some(binding) = &prepared_proposal_binding {
            candidate.reopen_unattested(binding)?;
        }
        record_terminal_receipt(
            config,
            due_nonce,
            Some(&transaction.trace_id),
            "authored_transaction_prepare_failed_non_authored",
            "complete response could not enter the durable authored transaction boundary",
            &transaction.tools_used,
            Some(&transaction.prompt_sha256),
            Some(&transaction.response_sha256),
        )?;
        return Err(error);
    }
    finalize_authored_transaction(config, &signer, &transaction)
}

/// Finish every durable effect of one exact complete response without consulting the model.
///
/// Each operation is create-or-verify or replay-idempotent. The authenticated transaction is
/// removed only after the completion nonce is durable, so a service restart resumes here.
#[allow(clippy::too_many_lines)] // One deliberately explicit crash-recovery transaction.
fn finalize_authored_transaction(
    config: &Config,
    signer: &HmacSigner,
    transaction: &AuthoredTransaction,
) -> Result<RunResult> {
    let reflection_path = persist_reflection(
        config,
        &transaction.due_nonce,
        &transaction.trace_id,
        &transaction.session_id,
        &transaction.turn_id,
        &transaction.prompt_sha256,
        &transaction.response_sha256,
        &transaction.response,
        &transaction.context_provenance,
    )?;
    inject_finalize_crash_for_test(config, "reflection")?;
    persist_summary(
        config,
        &transaction.due_nonce,
        &transaction.trace_id,
        &transaction.response_sha256,
        &transaction.summary,
        &transaction.context_provenance,
    )?;
    inject_finalize_crash_for_test(config, "summary")?;

    // Authorship is durably certified before any candidate authority artifact becomes visible.
    // A restart after this point must finish this exact transaction and must never call the model
    // again merely because publication, handoff, projection, or receipt work remains.
    let _ = crate::authored_transaction::write_completion(config, signer, transaction)?;
    let completion_proof =
        crate::authored_transaction::completion_proof(config, signer, transaction)?;
    inject_finalize_crash_for_test(config, "completion")?;

    if let Some(proposal_binding) = &transaction.unattested_proposal_binding {
        let (snapshot, active_generation) = SourceSnapshot::load_for_active_generation(config)?;
        let candidate_state = config.state_root.join("candidate");
        let candidate_outbox = config.state_root.join("candidate-outbox");
        let manager = CandidateManager::new(
            &candidate_state,
            &candidate_outbox,
            &snapshot,
            signer,
            &config.current_generation,
            &active_generation,
        )?;
        if !manager.reopen_unattested(proposal_binding)?
            && !matches!(
                manager.active()?,
                Some(crate::candidate::ActiveDraft::Editing)
            )
        {
            return Err(Error::new(
                "authored recovery could not restore its unattested candidate draft",
            ));
        }
    }

    let mut emitted_intent = None;
    if let Some(prepared) = &transaction.candidate {
        let public_envelope = completed_envelope(signer, &prepared.envelope, &completion_proof)?;
        let public_envelope_sha256 = sha256(&canonical_json(&public_envelope)?);
        let path = if let Some(path) = crate::publication::finish_visible_from_retained(
            config,
            signer,
            prepared,
            &public_envelope,
        )? {
            path
        } else {
            let (snapshot, active_generation) = SourceSnapshot::load_for_active_generation(config)?;
            let candidate_state = config.state_root.join("candidate");
            let candidate_outbox = config.state_root.join("candidate-outbox");
            let manager = CandidateManager::new(
                &candidate_state,
                &candidate_outbox,
                &snapshot,
                signer,
                &config.current_generation,
                &active_generation,
            )?;
            if prepared.source_id != snapshot.source_id
                || prepared.base_generation != active_generation
            {
                return Err(Error::new(
                    "authored candidate source changed before exact publication",
                ));
            }
            let exact_candidate = manager
                .submitted()?
                .ok_or_else(|| Error::new("prepared candidate disappeared before publication"))?;
            if exact_candidate.candidate_id != prepared.candidate_id
                || exact_candidate.candidate_sha256 != prepared.candidate_sha256
                || exact_candidate.manifest.base_generation != prepared.base_generation
            {
                return Err(Error::new(
                    "authored transaction no longer binds the exact candidate draft",
                ));
            }
            crate::publication::publish_idempotent(
                config,
                &manager,
                signer,
                &crate::publication::PublicationInput {
                    appliance_id: &config.appliance_id,
                    due_nonce: &transaction.due_nonce,
                    trace_id: &prepared.trace_id,
                    session_id: &prepared.session_id,
                    turn_id: &prepared.turn_id,
                    model: &config.model,
                    response_sha256: &prepared.response_sha256,
                    context_provenance_sha256: &prepared.context_provenance_sha256,
                    terminal_declaration: &prepared.terminal_declaration,
                    source_id: &prepared.source_id,
                    base_generation: &prepared.base_generation,
                    candidate: &exact_candidate,
                    intent_envelope_id: &prepared.envelope_id,
                    intent_envelope: &public_envelope,
                    intent_binding: &prepared.binding,
                },
            )?
        };
        emitted_intent = Some(EmittedIntent {
            path,
            envelope_id: prepared.envelope_id.clone(),
            envelope_sha256: public_envelope_sha256,
            intent_id: prepared.intent_id.clone(),
            trace_id: prepared.trace_id.clone(),
            session_id: prepared.session_id.clone(),
            turn_id: prepared.turn_id.clone(),
            response_sha256: prepared.response_sha256.clone(),
            terminal_declaration_sha256: prepared.terminal_declaration_sha256.clone(),
            candidate_id: prepared.candidate_id.clone(),
            candidate_sha256: prepared.candidate_sha256.clone(),
        });
        inject_finalize_crash_for_test(config, "publication")?;
    }

    let handoff_status = if let Some(emitted) = &emitted_intent {
        crate::handoff::unload_and_record(
            config,
            signer,
            &crate::handoff::ModelIntentBinding {
                envelope_id: &emitted.envelope_id,
                intent_id: &emitted.intent_id,
                trace_id: &emitted.trace_id,
                session_id: &emitted.session_id,
                turn_id: &emitted.turn_id,
                response_sha256: &emitted.response_sha256,
                terminal_declaration_sha256: &emitted.terminal_declaration_sha256,
                intent_envelope_sha256: &emitted.envelope_sha256,
                candidate_id: &emitted.candidate_id,
                candidate_sha256: &emitted.candidate_sha256,
            },
        )?
        .status
        .to_owned()
    } else {
        "not_applicable_no_intent".to_owned()
    };
    inject_finalize_crash_for_test(config, "handoff")?;
    let current_supervisor_mode = if emitted_intent.is_some() {
        crate::prompt::supervisor_status(config)?
            .get("mode")
            .and_then(Value::as_str)
            .map(str::to_owned)
    } else {
        None
    };
    let supervisor_handoff = publish_supervisor_handoff_trigger(
        &config.appliance_id,
        &config.supervisor_inbox,
        current_supervisor_mode.as_deref(),
        transaction.completed_at_unix_ms / 1_000,
        emitted_intent.as_ref(),
    )?;
    inject_finalize_crash_for_test(config, "supervisor_handoff")?;
    let candidate_id = emitted_intent
        .as_ref()
        .map(|intent| intent.candidate_id.clone());
    let candidate_digest = emitted_intent
        .as_ref()
        .map(|intent| intent.candidate_sha256.clone());
    let intent_path = emitted_intent.as_ref().map(|intent| intent.path.clone());
    project_scheduled_contract(
        config,
        signer,
        &transaction.due_nonce,
        transaction.started_at_unix_ms,
        &transaction.trace_id,
        &transaction.session_id,
        &transaction.turn_id,
        &transaction.prompt_sha256,
        transaction.prompt_chars,
        &transaction.response_sha256,
        &reflection_path,
        &transaction.summary,
        &transaction.tools_used,
        candidate_id.as_deref(),
        candidate_digest.as_deref(),
        intent_path.is_some(),
        transaction.completed_at_unix_ms,
        &transaction.span_id,
        &transaction.context_provenance,
    )?;
    inject_finalize_crash_for_test(config, "scheduled_projection")?;
    let detail = serde_json::json!({
        "summary_sha256": transaction.summary_sha256,
        "model": transaction.model,
        "prompt_tokens": transaction.prompt_tokens,
        "completion_tokens": transaction.completion_tokens,
        "provider_calls": transaction.provider_calls,
        "provider_elapsed_ms": transaction.provider_elapsed_ms,
        "direct_provider_provenance": "ExactModel",
        "intent_emitted": intent_path.is_some(),
        "maintenance_handoff": handoff_status,
        "supervisor_handoff": supervisor_handoff,
        "context_provenance": transaction.context_provenance,
        "context_provenance_sha256": transaction.context_provenance.digest()?,
        "reflection_lane": transaction.context_provenance.reflection_lane(),
        "taint_causes": transaction.context_provenance.taint_causes(),
        "source_review": transaction.source_review.as_ref().map(|review| serde_json::json!({
            "status": review.status,
            "turn_id": review.turn_id,
            "response_sha256": review.response_sha256,
            "tools_used": review.tools_used,
            "provider_calls": review.provider_calls,
            "failure_class": review.failure_class,
            "candidate_attested": review.status == "candidate_attested"
        }))
    });
    record_terminal_receipt_at(
        config,
        &transaction.due_nonce,
        Some(&transaction.trace_id),
        "authored_completed",
        &serde_json::to_string(&detail)?,
        &transaction.tools_used,
        Some(&transaction.prompt_sha256),
        Some(&transaction.response_sha256),
        transaction.completed_at_unix_ms / 1_000,
    )?;
    inject_finalize_crash_for_test(config, "terminal_receipt")?;
    crate::source_review::retire(config, &transaction.due_nonce)?;
    crate::authored_transaction::retire_prepared(config, &transaction.due_nonce)?;
    inject_finalize_crash_for_test(config, "retirement")?;
    Ok(RunResult {
        status: "authored_completed".to_owned(),
        due_nonce: transaction.due_nonce.clone(),
        trace_id: Some(transaction.trace_id.clone()),
        reflection_path: Some(reflection_path.to_string_lossy().into_owned()),
        intent_path: intent_path.map(|path| path.to_string_lossy().into_owned()),
        candidate_id,
    })
}

/// State-root-scoped, one-shot crash boundary used by integration tests. Release builds do not
/// inspect this path, so no appliance operator input can alter finalization behavior.
fn inject_finalize_crash_for_test(config: &Config, phase: &str) -> Result<()> {
    #[cfg(debug_assertions)]
    {
        let path = config.state_root.join("test-only-finalize-crash");
        if !path.exists() {
            if path.is_symlink() {
                return Err(Error::new("test crash marker is a broken symlink"));
            }
            return Ok(());
        }
        let bytes = crate::util::read_stable_regular(&path, 64)?;
        let requested = std::str::from_utf8(&bytes)
            .map_err(|_| Error::new("test crash phase is not UTF-8"))?
            .trim();
        if requested == phase {
            std::fs::remove_file(&path)?;
            std::fs::File::open(&config.state_root)?.sync_all()?;
            return Err(Error::new(format!(
                "test-only injected crash after finalize phase {phase}"
            )));
        }
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = (config, phase);
    }
    Ok(())
}

/// Simulate the uncatchable kill boundary after a candidate tool has durably
/// prepared its draft but before the in-memory transaction guard is armed.
fn inject_post_submit_crash_for_test(config: &Config) -> Result<()> {
    #[cfg(debug_assertions)]
    {
        let path = config.state_root.join("test-only-post-submit-crash");
        if !path.exists() {
            if path.is_symlink() {
                return Err(Error::new(
                    "test post-submit crash marker is a broken symlink",
                ));
            }
            return Ok(());
        }
        std::fs::remove_file(&path)?;
        std::fs::File::open(&config.state_root)?.sync_all()?;
        Err(Error::new(
            "test-only injected crash after candidate preparation before transaction guard",
        ))
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = config;
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)] // Exact trace-bound tool authorization boundary.
#[allow(clippy::too_many_lines)] // Exact tool schemas and authority dispatch stay visibly centralized.
fn execute_tool(
    config: &Config,
    snapshot: &SourceSnapshot,
    active_generation: &str,
    candidate: &CandidateManager<'_>,
    tool: &ToolCall,
    proposal_binding: &str,
    due_nonce: &str,
    trace_id: &str,
    session_id: &str,
    turn_id: &str,
    model_response_sha256: &str,
    context_provenance_sha256: &str,
    signer: &HmacSigner,
) -> Result<Value> {
    match tool.name.as_str() {
        "list_source" => {
            exact_argument_keys(&tool.arguments, &["prefix", "limit"])?;
            let prefix = string_argument(&tool.arguments, "prefix", 0, 160)?;
            let limit = usize_argument(&tool.arguments, "limit", 1, 50)?;
            Ok(serde_json::to_value(snapshot.list(&prefix, limit)?)?)
        },
        "search_source" => {
            exact_argument_keys(&tool.arguments, &["query", "limit"])?;
            let query = string_argument(&tool.arguments, "query", 1, 160)?;
            let limit = usize_argument(&tool.arguments, "limit", 1, 20)?;
            Ok(serde_json::to_value(snapshot.search(&query, limit)?)?)
        },
        "read_source_chunk" => {
            exact_argument_keys(
                &tool.arguments,
                &["source_id", "expected_sha256", "offset", "limit"],
            )?;
            let source_id = string_argument(&tool.arguments, "source_id", 1, 512)?;
            let expected = string_argument(&tool.arguments, "expected_sha256", 64, 64)?;
            let offset = usize_argument(&tool.arguments, "offset", 0, 512 * 1024)?;
            let limit = usize_argument(&tool.arguments, "limit", 1, 8_000)?;
            Ok(
                serde_json::json!({"content": snapshot.read(&source_id, Some(&expected), offset, limit)?}),
            )
        },
        "inspect_owned" => {
            exact_argument_keys(&tool.arguments, &["question", "limit"])?;
            let question = string_argument(&tool.arguments, "question", 1, 240)?;
            let limit = usize_argument(&tool.arguments, "limit", 1, 20)?;
            Ok(serde_json::to_value(owned::inspect(
                &config.owned_inputs,
                &question,
                limit,
            )?)?)
        },
        "read_owned" => {
            exact_argument_keys(&tool.arguments, &["kind", "basename"])?;
            let kind = string_argument(&tool.arguments, "kind", 1, 128)?;
            let basename = string_argument(&tool.arguments, "basename", 1, 128)?;
            Ok(serde_json::to_value(owned::read_basename(
                &config.owned_inputs,
                &kind,
                &basename,
            )?)?)
        },
        "read_generation_diff" => {
            exact_argument_keys(&tool.arguments, &["generation_id", "offset", "limit"])?;
            let generation_id = string_argument(&tool.arguments, "generation_id", 1, 128)?;
            let offset = usize_argument(&tool.arguments, "offset", 0, 25)?;
            let limit = usize_argument(&tool.arguments, "limit", 1, 4)?;
            crate::evidence::read_generation_diff(
                config,
                snapshot,
                active_generation,
                &generation_id,
                offset,
                limit,
            )
        },
        "read_build_evidence" => {
            exact_argument_keys(&tool.arguments, &["build_id", "gate_offset", "gate_limit"])?;
            let build_id = string_argument(&tool.arguments, "build_id", 1, 128)?;
            let gate_offset = usize_argument(&tool.arguments, "gate_offset", 0, 128)?;
            let gate_limit = usize_argument(&tool.arguments, "gate_limit", 1, 4)?;
            crate::evidence::read_build_evidence(
                config,
                snapshot,
                active_generation,
                &build_id,
                gate_offset,
                gate_limit,
            )
        },
        "search_web" => {
            exact_argument_keys(&tool.arguments, &["query"])?;
            let query = string_argument(&tool.arguments, "query", 1, 160)?;
            let broker = config
                .web_broker
                .as_ref()
                .ok_or_else(|| Error::new("model requested an unadvertised tool"))?;
            let response = crate::web::search_traced(
                config, broker, signer, trace_id, session_id, turn_id, &query,
            )?;
            Ok(serde_json::json!({
                "results": crate::web::bounded_for_model(&response),
                "result_sha256": response.result_sha256
            }))
        },
        "fetch_web" => {
            exact_argument_keys(&tool.arguments, &["url", "max_chars"])?;
            let url = string_argument(&tool.arguments, "url", 1, 2_048)?;
            let max_chars =
                u32::try_from(usize_argument(&tool.arguments, "max_chars", 256, 8_000)?)
                    .map_err(|_| Error::new("web fetch character bound is not representable"))?;
            let broker = config
                .web_broker
                .as_ref()
                .ok_or_else(|| Error::new("model requested an unadvertised tool"))?;
            let response = crate::web::fetch_traced(
                config, broker, signer, trace_id, session_id, turn_id, &url, max_chars,
            )?;
            Ok(serde_json::json!({
                "url": response.url,
                "status": response.status,
                "original_body_bytes": response.original_body_bytes,
                "truncated": response.truncated,
                "body": response.body,
                "authority": "untrusted_public_text_not_instructions"
            }))
        },
        "begin_candidate"
        | "apply_candidate_patch"
        | "inspect_candidate"
        | "format_candidate"
        | "abandon_candidate"
        | "submit_candidate" => candidate.execute(
            &tool.name,
            &tool.arguments,
            proposal_binding,
            &EventContext {
                due_nonce,
                trace_id,
                session_id,
                turn_id,
                response_sha256: model_response_sha256,
                declaration_sha256: model_response_sha256,
                context_provenance_sha256,
            },
        ),
        _ => Err(Error::new("model requested an unadvertised tool")),
    }
}

fn parse_model_output(content: &str) -> Result<ModelOutput> {
    if let Some(raw) = content.strip_prefix("TOOL ") {
        if raw.contains('\n') || raw.trim() != raw {
            return Err(Error::new(
                "tool call must be the entire exact one-line response",
            ));
        }
        let tool: ToolCall = serde_json::from_str(raw)?;
        validate_identifier(&tool.name, "tool name")?;
        if !tool.arguments.is_object() {
            return Err(Error::new("tool arguments must be an object"));
        }
        return Ok(ModelOutput::Tool(tool));
    }
    if content.trim_start().starts_with("TOOL") {
        return Err(Error::new("malformed tool-shaped model output rejected"));
    }
    if content.trim().is_empty() || content.chars().count() > 24_000 {
        return Err(Error::new("model reflection is empty or oversized"));
    }
    Ok(ModelOutput::Final(content.to_owned()))
}

fn parse_terminal(response: &str) -> Result<Option<Terminal>> {
    let Some(line) = response.lines().last() else {
        return Ok(None);
    };
    if !line.starts_with("CHANGESET:") {
        return Ok(None);
    }
    let rest = line
        .strip_prefix("CHANGESET: SUBMIT ")
        .ok_or_else(|| Error::new("malformed CHANGESET terminal declaration"))?;
    let (binding, reason) = rest
        .split_once(" :: ")
        .ok_or_else(|| Error::new("malformed CHANGESET reason separator"))?;
    let mut fields = binding.split(' ');
    let candidate_id = fields.next().unwrap_or_default().to_owned();
    let candidate_sha256 = fields.next().unwrap_or_default().to_owned();
    if fields.next().is_some()
        || reason.trim().is_empty()
        || reason.trim() != reason
        || reason.chars().count() > 240
        || has_ambiguous_controls(reason)
    {
        return Err(Error::new("malformed CHANGESET terminal fields"));
    }
    validate_identifier(&candidate_id, "terminal candidate_id")?;
    validate_hex64(&candidate_sha256, "terminal candidate_sha256")?;
    Ok(Some(Terminal {
        candidate_id,
        candidate_sha256,
        declaration: line.to_owned(),
    }))
}

#[allow(clippy::too_many_arguments)]
fn prepare_intent(
    config: &Config,
    due_nonce: &str,
    trace_id: &str,
    session_id: &str,
    turn_id: &str,
    response_sha256: &str,
    source_id: &str,
    terminal: &Terminal,
    submitted: Option<SubmittedCandidate>,
    signer: &HmacSigner,
) -> Result<PreparedIntent> {
    let submitted =
        submitted.ok_or_else(|| Error::new("terminal declaration has no submitted candidate"))?;
    if submitted.candidate_id != terminal.candidate_id
        || submitted.candidate_sha256 != terminal.candidate_sha256
        || submitted.manifest.base_generation != read_generation(config)?
    {
        return Err(Error::new(
            "terminal declaration does not bind exact current candidate",
        ));
    }
    let now = unix_seconds();
    let intent_id = format!("intent-{}", Uuid::new_v4().simple());
    let intent = SupervisorIntent {
        schema: INTENT_SCHEMA,
        intent_id,
        appliance_id: config.appliance_id.clone(),
        trace_id: trace_id.to_owned(),
        session_id: session_id.to_owned(),
        turn_id: turn_id.to_owned(),
        response_sha256: response_sha256.to_owned(),
        terminal_declaration_sha256: sha256(terminal.declaration.as_bytes()),
        candidate_id: terminal.candidate_id.clone(),
        candidate_sha256: terminal.candidate_sha256.clone(),
        base_generation: submitted.manifest.base_generation.clone(),
        current_generation: submitted.manifest.base_generation.clone(),
        observed_at: now,
        origin: "scheduled_autonomy",
        authorship_status: "genuinely_authored",
        transport_status: "authored_completed",
        declaration_provenance: "exact_terminal_model_declaration",
        fallback: false,
        executor_repair: false,
        operator_harness: false,
    };
    let envelope_id = format!("envelope-{}", Uuid::new_v4().simple());
    let envelope = envelope(
        signer,
        envelope_id.clone(),
        now,
        &submitted.manifest,
        &intent,
    )?;
    if envelope["schema"] != ENVELOPE_SCHEMA {
        return Err(Error::new("internal intent envelope schema mismatch"));
    }
    let binding = serde_json::json!({
        "schema": "astrid.edge.steward_helper.intent_binding_receipt.v1",
        "appliance_id": config.appliance_id,
        "due_nonce": due_nonce,
        "trace_id": trace_id,
        "session_id": session_id,
        "turn_id": turn_id,
        "model": config.model,
        "response_sha256": response_sha256,
        "terminal_declaration_sha256": intent.terminal_declaration_sha256,
        "candidate_sha256": intent.candidate_sha256,
        "patch_sha256": submitted.patch_sha256,
        "source_id": source_id,
        "base_generation": intent.base_generation,
        "provider_provenance": "ExactModel:direct_nonstreaming_loopback_no_retry"
    });
    let envelope_bytes = canonical_json(&envelope)?;
    let envelope_sha256 = sha256(&envelope_bytes);
    Ok(PreparedIntent {
        envelope,
        binding,
        envelope_id,
        envelope_sha256,
        intent_id: intent.intent_id,
        trace_id: intent.trace_id,
        session_id: intent.session_id,
        turn_id: intent.turn_id,
        response_sha256: intent.response_sha256,
        terminal_declaration: terminal.declaration.clone(),
        terminal_declaration_sha256: intent.terminal_declaration_sha256,
        candidate_id: terminal.candidate_id.clone(),
        candidate_sha256: terminal.candidate_sha256.clone(),
        source_id: source_id.to_owned(),
        base_generation: submitted.manifest.base_generation,
    })
}

#[allow(clippy::too_many_arguments)]
fn persist_reflection(
    config: &Config,
    due_nonce: &str,
    trace_id: &str,
    session_id: &str,
    turn_id: &str,
    prompt_sha256: &str,
    response_sha256: &str,
    response: &str,
    context_provenance: &ContextProvenance,
) -> Result<PathBuf> {
    let directory = config.workspace_root.join("introspections/scheduled");
    let path = directory.join(format!("reflection_{due_nonce}_{turn_id}.md"));
    workspace_write_exact(config, &path, response.as_bytes())?;
    let metadata = serde_json::json!({
        "schema": REFLECTION_SCHEMA,
        "provenance": "model_authored_runtime_scheduled",
        "appliance_id": config.appliance_id,
        "due_nonce": due_nonce,
        "trace_id": trace_id,
        "session_id": session_id,
        "turn_id": turn_id,
        "model": config.model,
        "prompt_sha256": prompt_sha256,
        "response_sha256": response_sha256,
        "exact_response_path": path.file_name().and_then(|name| name.to_str()),
        "context_provenance": context_provenance,
        "context_provenance_sha256": context_provenance.digest()?,
        "reflection_lane": context_provenance.reflection_lane(),
        "taint_causes": context_provenance.taint_causes()
    });
    workspace_write_exact(
        config,
        &path.with_extension("json"),
        &canonical_json(&metadata)?,
    )?;
    Ok(path)
}

fn persist_summary(
    config: &Config,
    due_nonce: &str,
    trace_id: &str,
    response_sha256: &str,
    summary: &str,
    context_provenance: &ContextProvenance,
) -> Result<()> {
    crate::prompt::persist_summary_at(
        &config.state_root.join("latest-authored-summary.json"),
        due_nonce,
        trace_id,
        response_sha256,
        summary,
        context_provenance,
    )
}

fn bounded_summary(response: &str) -> String {
    let without_terminal = response
        .lines()
        .filter(|line| !line.starts_with("CHANGESET: SUBMIT ") && *line != "SOURCE_REVIEW: REQUEST")
        .collect::<Vec<_>>()
        .join(" ");
    bounded_text(
        without_terminal
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .as_str(),
        320,
    )
}

fn receipt_core(
    due_nonce: &str,
    trace_id: Option<&str>,
    status: &str,
    detail: &str,
    tools: &[String],
    prompt_sha256: Option<&str>,
    response_sha256: Option<&str>,
) -> Value {
    receipt_core_at(
        due_nonce,
        trace_id,
        status,
        detail,
        tools,
        prompt_sha256,
        response_sha256,
        unix_seconds(),
    )
}

#[allow(clippy::too_many_arguments)]
fn receipt_core_at(
    due_nonce: &str,
    trace_id: Option<&str>,
    status: &str,
    detail: &str,
    tools: &[String],
    prompt_sha256: Option<&str>,
    response_sha256: Option<&str>,
    recorded_at: u64,
) -> Value {
    serde_json::json!({
        "schema": RECEIPT_SCHEMA,
        "recorded_at": recorded_at,
        "due_nonce": due_nonce,
        "trace_id": trace_id,
        "status": status,
        "detail": bounded_text(detail, 512),
        "tools_used": tools,
        "prompt_sha256": prompt_sha256,
        "response_sha256": response_sha256,
        "fallback": false,
        "executor_repair": false,
        "operator_harness": false
    })
}

#[allow(clippy::too_many_arguments)]
fn record_terminal_receipt(
    config: &Config,
    due_nonce: &str,
    trace_id: Option<&str>,
    status: &str,
    detail: &str,
    tools: &[String],
    prompt_sha256: Option<&str>,
    response_sha256: Option<&str>,
) -> Result<()> {
    record_terminal_receipt_at(
        config,
        due_nonce,
        trace_id,
        status,
        detail,
        tools,
        prompt_sha256,
        response_sha256,
        unix_seconds(),
    )
}

#[allow(clippy::too_many_arguments)]
fn record_terminal_receipt_at(
    config: &Config,
    due_nonce: &str,
    trace_id: Option<&str>,
    status: &str,
    detail: &str,
    tools: &[String],
    prompt_sha256: Option<&str>,
    response_sha256: Option<&str>,
    recorded_at: u64,
) -> Result<()> {
    let binding = serde_json::json!({
        "due_nonce": due_nonce,
        "trace_id": trace_id,
        "status": status,
        "detail_sha256": sha256(detail.as_bytes()),
        "tools_used": tools,
        "prompt_sha256": prompt_sha256,
        "response_sha256": response_sha256
    });
    let binding_sha256 = sha256(&canonical_json(&binding)?);
    let receipt_id = format!("terminal-{}", &binding_sha256[..32]);
    let mut core = receipt_core_at(
        due_nonce,
        trace_id,
        status,
        detail,
        tools,
        prompt_sha256,
        response_sha256,
        recorded_at,
    );
    core["receipt_id"] = Value::String(receipt_id);
    core["terminal_binding_sha256"] = Value::String(binding_sha256);
    record_receipt_once(config, &core)
}

fn record_receipt(config: &Config, core: &Value) -> Result<()> {
    let signer = HmacSigner::from_file(&config.attestor_key)?;
    let core_bytes = canonical_json(&core)?;
    let record = serde_json::json!({
        "schema": RECEIPT_SCHEMA,
        "core": core,
        "core_sha256": sha256(&core_bytes),
        "auth": {
            "algorithm": "hmac-sha256",
            "key_id": signer.key_id,
            "signature": signer.sign(&core_bytes)
        }
    });
    let mut line = canonical_json(&record)?;
    line.push(b'\n');
    append_private(&config.state_root.join("receipts.jsonl"), &line)
}

fn record_receipt_once(config: &Config, core: &Value) -> Result<()> {
    let receipt_id = core
        .get("receipt_id")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("terminal helper receipt lacks a receipt ID"))?;
    validate_identifier(receipt_id, "terminal helper receipt id")?;
    let signer = HmacSigner::from_file(&config.attestor_key)?;
    let core_bytes = canonical_json(core)?;
    let record = serde_json::json!({
        "schema": RECEIPT_SCHEMA,
        "core": core,
        "core_sha256": sha256(&core_bytes),
        "auth": {
            "algorithm": "hmac-sha256",
            "key_id": signer.key_id,
            "signature": signer.sign(&core_bytes)
        }
    });
    let record_bytes = canonical_json(&record)?;
    let path = config.state_root.join("receipts.jsonl");
    if path.exists() || path.is_symlink() {
        let bytes = crate::util::read_stable_regular(&path, 64 * 1024 * 1024)?;
        if !bytes.is_empty() && !bytes.ends_with(b"\n") {
            return Err(Error::new("helper receipt ledger has an incomplete tail"));
        }
        let mut matching = 0_u8;
        for line in bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
        {
            let existing: Value = serde_json::from_slice(line)?;
            verify_receipt_record(&signer, &existing)?;
            if existing
                .get("core")
                .and_then(|value| value.get("receipt_id"))
                .and_then(Value::as_str)
                == Some(receipt_id)
            {
                if canonical_json(&existing)? != record_bytes {
                    return Err(Error::new("terminal helper receipt ID collision"));
                }
                matching = matching.saturating_add(1);
            }
        }
        if matching > 1 {
            return Err(Error::new("duplicate terminal helper receipts detected"));
        }
        if matching == 1 {
            return Ok(());
        }
    }
    let mut line = record_bytes;
    line.push(b'\n');
    append_private(&path, &line)
}

fn verify_receipt_record(signer: &HmacSigner, record: &Value) -> Result<()> {
    let core = record
        .get("core")
        .ok_or_else(|| Error::new("helper receipt has no core"))?;
    let core_bytes = canonical_json(core)?;
    let auth = record
        .get("auth")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("helper receipt has no authentication"))?;
    if record.get("schema").and_then(Value::as_str) != Some(RECEIPT_SCHEMA)
        || record.get("core_sha256").and_then(Value::as_str) != Some(sha256(&core_bytes).as_str())
        || auth.get("algorithm").and_then(Value::as_str) != Some("hmac-sha256")
        || auth.get("key_id").and_then(Value::as_str) != Some(signer.key_id.as_str())
        || !auth
            .get("signature")
            .and_then(Value::as_str)
            .is_some_and(|signature| signer.verify(&core_bytes, signature))
    {
        return Err(Error::new("helper receipt authentication failed"));
    }
    Ok(())
}

fn read_generation(config: &Config) -> Result<String> {
    let bytes = crate::util::read_stable_regular(&config.current_generation, 256)?;
    let value = std::str::from_utf8(&bytes)
        .map_err(|_| Error::new("current generation is not UTF-8"))?
        .trim()
        .to_owned();
    validate_identifier(&value, "current generation")?;
    Ok(value)
}

fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

fn validate_due_nonce(value: &str) -> Result<()> {
    validate_identifier(value, "due nonce")?;
    let suffix = value
        .strip_prefix("due-")
        .ok_or_else(|| Error::new("due nonce must use due-<decimal-slot> form"))?;
    if suffix.len() < 5 || suffix.len() > 20 || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Error::new("due nonce is malformed or replay-shaped"));
    }
    Ok(())
}

fn exact_argument_keys(value: &Value, expected: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::new("tool arguments must be an object"))?;
    let actual = object
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let expected = expected
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    if actual != expected {
        return Err(Error::new(
            "tool call contains missing or unadvertised arguments",
        ));
    }
    Ok(())
}

fn string_argument(value: &Value, key: &str, minimum: usize, maximum: usize) -> Result<String> {
    let text = value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new(format!("{key} must be a string")))?;
    let count = text.chars().count();
    if count < minimum || count > maximum {
        return Err(Error::new(format!("{key} exceeds its bound")));
    }
    Ok(text.to_owned())
}

fn usize_argument(value: &Value, key: &str, minimum: usize, maximum: usize) -> Result<usize> {
    let number = value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new(format!("{key} must be an unsigned integer")))?;
    let number = usize::try_from(number).map_err(|_| Error::new(format!("{key} is too large")))?;
    if number < minimum || number > maximum {
        return Err(Error::new(format!("{key} exceeds its bound")));
    }
    Ok(number)
}

fn has_ambiguous_controls(value: &str) -> bool {
    value.chars().any(|character| {
        character.is_control()
            || matches!(character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
    })
}

fn bounded_failure_class(error: &Error) -> String {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("deadline") || message.contains("timed out") {
        "provider_timeout_or_deadline".to_owned()
    } else if message.contains("connect") || message.contains("refused") {
        "provider_connection_failed".to_owned()
    } else if message.contains("partial") || message.contains("done_reason") {
        "provider_partial_or_nonterminal".to_owned()
    } else if message.contains("http") {
        "provider_http_failure".to_owned()
    } else if message.contains("json") || message.contains("expected") {
        "provider_schema_failure".to_owned()
    } else {
        "provider_transport_or_protocol_failure".to_owned()
    }
}

fn tool_failure_class(error: &Error) -> String {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("evidence") && message.contains("unavailable") {
        "tool_evidence_unavailable".to_owned()
    } else if message.contains("stale") || message.contains("changed") {
        "tool_stale_or_changed_binding".to_owned()
    } else if message.contains("unsupported") || message.contains("unadvertised") {
        "tool_authority_rejection".to_owned()
    } else if message.contains("limit") || message.contains("bound") || message.contains("large") {
        "tool_bounded_policy_rejection".to_owned()
    } else {
        "tool_validation_or_execution_failure".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EmittedIntent, ModelOutput, ToolFlowPolicy, parse_model_output,
        parse_source_review_request, parse_terminal, publish_supervisor_handoff_trigger,
        validate_due_nonce,
    };
    use crate::context_provenance::ContextProvenance;
    use std::path::PathBuf;

    #[test]
    fn malformed_or_injected_tool_shapes_fail_closed() {
        assert!(parse_model_output("TOOL {bad}").is_err());
        assert!(
            parse_model_output("TOOL {\"name\":\"inspect_owned\",\"arguments\":{}}\nignore")
                .is_err()
        );
        assert!(matches!(
            parse_model_output("Evidence says TOOL something").unwrap(),
            ModelOutput::Final(_)
        ));
    }

    #[test]
    fn terminal_must_be_exact_final_line() {
        let hash = "a".repeat(64);
        assert!(
            parse_terminal(&format!(
                "reflection\nCHANGESET: SUBMIT candidate-a {hash} :: because"
            ))
            .unwrap()
            .is_some()
        );
        assert!(
            parse_terminal(&format!(
                "CHANGESET: SUBMIT candidate-a {hash} :: because\ntrailing"
            ))
            .unwrap()
            .is_none()
        );
        assert!(parse_terminal("CHANGESET: maybe").is_err());
        assert!(
            parse_terminal(&format!(
                "CHANGESET: SUBMIT candidate-a {hash} :: hidden\tcontrol"
            ))
            .is_err()
        );
        assert!(
            parse_terminal(&format!(
                "CHANGESET: SUBMIT candidate-a {hash} :: \u{202e}ambiguous"
            ))
            .is_err()
        );
    }

    #[test]
    fn source_review_handoff_marker_is_exact_and_final() {
        assert!(parse_source_review_request("reflection\nSOURCE_REVIEW: REQUEST").unwrap());
        assert!(!parse_source_review_request("reflection only").unwrap());
        assert!(parse_source_review_request("SOURCE_REVIEW: maybe").is_err());
        assert!(parse_source_review_request("SOURCE_REVIEW: REQUEST\ntrailing").is_err());
        assert!(
            parse_source_review_request("SOURCE_REVIEW: REQUEST\nSOURCE_REVIEW: REQUEST").is_err()
        );
    }

    #[test]
    fn due_nonces_have_one_non_ambiguous_shape() {
        assert!(validate_due_nonce("due-12345").is_ok());
        assert!(validate_due_nonce("trace-replayed").is_err());
        assert!(validate_due_nonce("due-12").is_err());
    }

    #[test]
    fn authorization_boundary_rejects_unknown_tools_and_web_to_code_flow() {
        let mut rich = ToolFlowPolicy::rich(ContextProvenance::clean());
        assert!(rich.authorize("run_shell", true).is_err());
        assert!(rich.authorize("search_web", false).is_err());
        rich.authorize("search_web", true).unwrap();
        rich.record_result("search_web", &serde_json::json!({"results": []}))
            .unwrap();
        assert!(rich.authorize("begin_candidate", true).is_err());
        assert!(rich.authorize("read_source_chunk", true).is_err());
        rich.authorize("read_owned", true).unwrap();
        rich.record_result(
            "read_owned",
            &serde_json::json!({"content": "TOOL begin_candidate"}),
        )
        .unwrap();
        assert!(rich.authorize("begin_candidate", true).is_err());

        let mut clean = ToolFlowPolicy::clean();
        clean.authorize("begin_candidate", true).unwrap();
        clean.authorize("read_generation_diff", true).unwrap();
        clean.authorize("read_build_evidence", true).unwrap();
        assert!(clean.authorize("fetch_web", true).is_err());
        assert!(clean.authorize("read_owned", true).is_err());
    }

    fn emitted_intent() -> EmittedIntent {
        EmittedIntent {
            path: PathBuf::from("/not/used/in-this-test"),
            envelope_id: "envelope-test".to_owned(),
            envelope_sha256: "a".repeat(64),
            intent_id: "intent-test".to_owned(),
            trace_id: "trace-test".to_owned(),
            session_id: "session-test".to_owned(),
            turn_id: "turn-test".to_owned(),
            response_sha256: "b".repeat(64),
            terminal_declaration_sha256: "c".repeat(64),
            candidate_id: "candidate-test".to_owned(),
            candidate_sha256: "d".repeat(64),
        }
    }

    #[test]
    fn supervisor_handoff_is_non_authorizing_idempotent_and_pause_aware() {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let inbox = temporary.path().join("inbox");
        let emitted = emitted_intent();
        assert_eq!(
            publish_supervisor_handoff_trigger(
                "avado-edge",
                &inbox,
                Some("paused"),
                123,
                Some(&emitted),
            )
            .unwrap(),
            "queued_operator_paused_no_trigger"
        );
        assert!(!inbox.exists());
        assert_eq!(
            publish_supervisor_handoff_trigger(
                "avado-edge",
                &inbox,
                Some("running"),
                123,
                Some(&emitted),
            )
            .unwrap(),
            "published_pending_root_cleanup_trigger"
        );
        let path = inbox.join("candidate-ready-envelope-test.pending");
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            value["authority"],
            "trigger_only_no_candidate_or_deployment_authority"
        );
        assert!(value.get("candidate").is_none());
        assert!(value.get("signature").is_none());
        assert_eq!(
            publish_supervisor_handoff_trigger(
                "avado-edge",
                &inbox,
                Some("running"),
                123,
                Some(&emitted),
            )
            .unwrap(),
            "already_published_pending_root_cleanup_trigger"
        );
    }
}
