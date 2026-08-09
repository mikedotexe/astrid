//! Durable boundary between an exact complete model response and its public effects.
//!
//! Once this signed transaction exists, recovery must finish this response and
//! must never call the model again for the same due nonce.

use std::fs::{self, File};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::attestation::HmacSigner;
use crate::config::Config;
use crate::context_provenance::ContextProvenance;
use crate::util::{atomic_private_write, canonical_json, read_stable_regular, sha256};
use crate::{Error, Result};

pub(crate) const CORE_SCHEMA: &str = "astrid.edge.steward_helper.authored_transaction.v1";
const ENVELOPE_SCHEMA: &str = "astrid.edge.steward_helper.authored_transaction_envelope.v1";
pub(crate) const COMPLETION_SCHEMA: &str = "astrid.edge.steward_helper.authored_completion.v2";
const COMPLETION_ENVELOPE_SCHEMA: &str =
    "astrid.edge.steward_helper.authored_completion_envelope.v2";
const MAX_TRANSACTION_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidatePublication {
    pub envelope: Value,
    pub binding: Value,
    pub envelope_id: String,
    pub envelope_sha256: String,
    pub intent_id: String,
    pub trace_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub response_sha256: String,
    #[serde(default)]
    pub context_provenance_sha256: String,
    pub terminal_declaration: String,
    pub terminal_declaration_sha256: String,
    pub candidate_id: String,
    pub candidate_sha256: String,
    pub source_id: String,
    pub base_generation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceReviewOutcome {
    pub schema: String,
    pub status: String,
    pub turn_id: Option<String>,
    pub span_id: Option<String>,
    pub prompt_sha256: Option<String>,
    pub prompt_chars: usize,
    pub response_sha256: Option<String>,
    pub exact_candidate_author_response: Option<String>,
    pub tools_used: Vec<String>,
    #[serde(default)]
    pub provider_calls: u8,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub provider_elapsed_ms: u64,
    pub failure_class: Option<String>,
    pub context_provenance: ContextProvenance,
    pub authority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredTransaction {
    pub schema: String,
    pub prepared_at_unix_ms: u64,
    pub completed_at_unix_ms: u64,
    pub started_at_unix_ms: u64,
    pub appliance_id: String,
    pub model: String,
    pub due_nonce: String,
    pub trace_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub span_id: String,
    pub prompt_sha256: String,
    pub prompt_chars: usize,
    pub response: String,
    pub response_sha256: String,
    pub summary: String,
    pub summary_sha256: String,
    pub tools_used: Vec<String>,
    #[serde(default)]
    pub provider_calls: u8,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub provider_elapsed_ms: u64,
    #[serde(default, skip_serializing_if = "ContextProvenance::is_legacy")]
    pub context_provenance: ContextProvenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_review: Option<SourceReviewOutcome>,
    pub candidate: Option<CandidatePublication>,
    pub unattested_proposal_binding: Option<String>,
    pub provenance: String,
    pub authority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionEnvelope {
    schema: String,
    core: AuthoredTransaction,
    core_sha256: String,
    key_id: String,
    hmac_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompletionCore {
    schema: String,
    appliance_id: String,
    due_nonce: String,
    trace_id: String,
    session_id: String,
    turn_id: String,
    response_sha256: String,
    transaction_sha256: String,
    completed_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_review_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_review_turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_review_response_sha256: Option<String>,
    candidate_publication: Option<CompletionCandidatePublication>,
    status: String,
    provenance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompletionCandidatePublication {
    intent_envelope_id: String,
    intent_envelope_sha256: String,
    intent_id: String,
    terminal_declaration_sha256: String,
    candidate_id: String,
    candidate_sha256: String,
    base_generation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompletionAuth {
    algorithm: String,
    key_id: String,
    signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompletionEnvelope {
    schema: String,
    core: CompletionCore,
    core_sha256: String,
    auth: CompletionAuth,
}

pub fn prepare(
    config: &Config,
    signer: &HmacSigner,
    transaction: &AuthoredTransaction,
) -> Result<()> {
    validate(config, transaction)?;
    let core_bytes = canonical_json(transaction)?;
    let envelope = TransactionEnvelope {
        schema: ENVELOPE_SCHEMA.to_owned(),
        core: transaction.clone(),
        core_sha256: sha256(&core_bytes),
        key_id: signer.key_id.clone(),
        hmac_sha256: signer.sign(&core_bytes),
    };
    let bytes = canonical_json(&envelope)?;
    if bytes.len() as u64 > MAX_TRANSACTION_BYTES {
        return Err(Error::new("authored transaction exceeds its private bound"));
    }
    let path = transaction_path(config, &transaction.due_nonce);
    if path.exists() || path.is_symlink() {
        let existing = read_stable_regular(&path, MAX_TRANSACTION_BYTES)?;
        if existing != bytes {
            return Err(Error::new(
                "due nonce already binds a different authored transaction",
            ));
        }
        return Ok(());
    }
    atomic_private_write(&path, &bytes)
}

pub fn load(
    config: &Config,
    signer: &HmacSigner,
    due_nonce: &str,
) -> Result<Option<AuthoredTransaction>> {
    let path = transaction_path(config, due_nonce);
    if !path.exists() {
        if path.is_symlink() {
            return Err(Error::new("authored transaction is a broken symlink"));
        }
        return Ok(None);
    }
    let bytes = read_stable_regular(&path, MAX_TRANSACTION_BYTES)?;
    let envelope: TransactionEnvelope = serde_json::from_slice(&bytes)?;
    let core_bytes = canonical_json(&envelope.core)?;
    if envelope.schema != ENVELOPE_SCHEMA
        || envelope.core.schema != CORE_SCHEMA
        || envelope.core.due_nonce != due_nonce
        || envelope.core_sha256 != sha256(&core_bytes)
        || envelope.key_id != signer.key_id
        || !signer.verify(&core_bytes, &envelope.hmac_sha256)
    {
        return Err(Error::new("authored transaction authentication failed"));
    }
    validate(config, &envelope.core)?;
    Ok(Some(envelope.core))
}

/// Persist and return the exact signed proof that this complete response crossed the durable
/// authorship boundary. A candidate intent may only become supervisor-visible after this call
/// succeeds. The returned value is suitable for inclusion in the public completed-intent wrapper.
pub fn write_completion(
    config: &Config,
    signer: &HmacSigner,
    transaction: &AuthoredTransaction,
) -> Result<Value> {
    validate(config, transaction)?;
    let transaction_bytes = canonical_json(transaction)?;
    let core = CompletionCore {
        schema: COMPLETION_SCHEMA.to_owned(),
        appliance_id: config.appliance_id.clone(),
        due_nonce: transaction.due_nonce.clone(),
        trace_id: transaction.trace_id.clone(),
        session_id: transaction.session_id.clone(),
        turn_id: transaction.turn_id.clone(),
        response_sha256: transaction.response_sha256.clone(),
        transaction_sha256: sha256(&transaction_bytes),
        completed_at_unix_ms: transaction.completed_at_unix_ms,
        source_review_status: transaction
            .source_review
            .as_ref()
            .map(|review| review.status.clone()),
        source_review_turn_id: transaction
            .source_review
            .as_ref()
            .and_then(|review| review.turn_id.clone()),
        source_review_response_sha256: transaction
            .source_review
            .as_ref()
            .and_then(|review| review.response_sha256.clone()),
        candidate_publication: transaction.candidate.as_ref().map(|candidate| {
            CompletionCandidatePublication {
                intent_envelope_id: candidate.envelope_id.clone(),
                intent_envelope_sha256: candidate.envelope_sha256.clone(),
                intent_id: candidate.intent_id.clone(),
                terminal_declaration_sha256: candidate.terminal_declaration_sha256.clone(),
                candidate_id: candidate.candidate_id.clone(),
                candidate_sha256: candidate.candidate_sha256.clone(),
                base_generation: candidate.base_generation.clone(),
            }
        }),
        status: "authored_completed".to_owned(),
        provenance: "model_authored_runtime_scheduled".to_owned(),
    };
    let core_bytes = canonical_json(&core)?;
    let envelope = CompletionEnvelope {
        schema: COMPLETION_ENVELOPE_SCHEMA.to_owned(),
        core,
        core_sha256: sha256(&core_bytes),
        auth: CompletionAuth {
            algorithm: "hmac-sha256".to_owned(),
            key_id: signer.key_id.clone(),
            signature: signer.sign(&core_bytes),
        },
    };
    let bytes = canonical_json(&envelope)?;
    let path = completion_path(config, &transaction.due_nonce);
    if path.exists() || path.is_symlink() {
        if read_stable_regular(&path, 64 * 1024)? != bytes {
            return Err(Error::new("authored completion marker collision"));
        }
        return Ok(serde_json::from_slice(&bytes)?);
    }
    atomic_private_write(&path, &bytes)?;
    File::open(
        path.parent()
            .ok_or_else(|| Error::new("completion marker has no parent"))?,
    )?
    .sync_all()?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn verify_completion(config: &Config, signer: &HmacSigner, due_nonce: &str) -> Result<bool> {
    let path = completion_path(config, due_nonce);
    if !path.exists() {
        if path.is_symlink() {
            return Err(Error::new("authored completion marker is a broken symlink"));
        }
        return Ok(false);
    }
    let bytes = read_stable_regular(&path, 64 * 1024)?;
    let envelope: CompletionEnvelope = serde_json::from_slice(&bytes)?;
    let core_bytes = canonical_json(&envelope.core)?;
    if envelope.schema != COMPLETION_ENVELOPE_SCHEMA
        || envelope.core.schema != COMPLETION_SCHEMA
        || envelope.core.appliance_id != config.appliance_id
        || envelope.core.due_nonce != due_nonce
        || envelope.core.status != "authored_completed"
        || envelope.core.provenance != "model_authored_runtime_scheduled"
        || envelope.core.completed_at_unix_ms == 0
        || envelope.core_sha256 != sha256(&core_bytes)
        || envelope.auth.algorithm != "hmac-sha256"
        || envelope.auth.key_id != signer.key_id
        || !signer.verify(&core_bytes, &envelope.auth.signature)
        || canonical_json(&envelope)? != bytes
    {
        return Err(Error::new(
            "authored completion marker authentication failed",
        ));
    }
    for (value, label) in [
        (&envelope.core.due_nonce, "completion due nonce"),
        (&envelope.core.trace_id, "completion trace id"),
        (&envelope.core.session_id, "completion session id"),
        (&envelope.core.turn_id, "completion turn id"),
    ] {
        crate::util::validate_identifier(value, label)?;
    }
    for (value, label) in [
        (&envelope.core.response_sha256, "completion response hash"),
        (
            &envelope.core.transaction_sha256,
            "completion transaction hash",
        ),
    ] {
        crate::util::validate_hex64(value, label)?;
    }
    if let Some(transaction) = load(config, signer, due_nonce)?
        && (envelope.core.trace_id != transaction.trace_id
            || envelope.core.session_id != transaction.session_id
            || envelope.core.turn_id != transaction.turn_id
            || envelope.core.response_sha256 != transaction.response_sha256
            || envelope.core.transaction_sha256 != sha256(&canonical_json(&transaction)?)
            || envelope.core.completed_at_unix_ms != transaction.completed_at_unix_ms
            || !completion_source_review_matches(
                &envelope.core,
                transaction.source_review.as_ref(),
            )
            || !completion_candidate_matches(
                envelope.core.candidate_publication.as_ref(),
                transaction.candidate.as_ref(),
            ))
    {
        return Err(Error::new(
            "authored completion marker does not bind the prepared transaction",
        ));
    }
    Ok(true)
}

/// Load the exact durable completion proof and require that it still binds this retained
/// transaction. This is used to construct or verify a public completed-intent wrapper during
/// crash recovery; it never manufactures a proof from an in-memory response.
pub fn completion_proof(
    config: &Config,
    signer: &HmacSigner,
    transaction: &AuthoredTransaction,
) -> Result<Value> {
    if !verify_completion(config, signer, &transaction.due_nonce)? {
        return Err(Error::new("authored completion proof is not durable"));
    }
    let path = completion_path(config, &transaction.due_nonce);
    let bytes = read_stable_regular(&path, 64 * 1024)?;
    let envelope: CompletionEnvelope = serde_json::from_slice(&bytes)?;
    if envelope.core.transaction_sha256 != sha256(&canonical_json(transaction)?)
        || !completion_candidate_matches(
            envelope.core.candidate_publication.as_ref(),
            transaction.candidate.as_ref(),
        )
    {
        return Err(Error::new(
            "authored completion proof does not bind retained transaction",
        ));
    }
    Ok(serde_json::from_slice(&bytes)?)
}

fn completion_source_review_matches(
    completion: &CompletionCore,
    review: Option<&SourceReviewOutcome>,
) -> bool {
    match review {
        None => {
            completion.source_review_status.is_none()
                && completion.source_review_turn_id.is_none()
                && completion.source_review_response_sha256.is_none()
        },
        Some(review) => {
            completion.source_review_status.as_deref() == Some(review.status.as_str())
                && completion.source_review_turn_id.as_deref() == review.turn_id.as_deref()
                && completion.source_review_response_sha256.as_deref()
                    == review.response_sha256.as_deref()
        },
    }
}

fn completion_candidate_matches(
    completion: Option<&CompletionCandidatePublication>,
    candidate: Option<&CandidatePublication>,
) -> bool {
    match (completion, candidate) {
        (None, None) => true,
        (Some(completion), Some(candidate)) => {
            completion.intent_envelope_id == candidate.envelope_id
                && completion.intent_envelope_sha256 == candidate.envelope_sha256
                && completion.intent_id == candidate.intent_id
                && completion.terminal_declaration_sha256 == candidate.terminal_declaration_sha256
                && completion.candidate_id == candidate.candidate_id
                && completion.candidate_sha256 == candidate.candidate_sha256
                && completion.base_generation == candidate.base_generation
        },
        (None, Some(_)) | (Some(_), None) => false,
    }
}

pub fn retire_prepared(config: &Config, due_nonce: &str) -> Result<()> {
    let path = transaction_path(config, due_nonce);
    if path.exists() {
        fs::remove_file(path)?;
        File::open(transaction_root(config))?.sync_all()?;
    } else if path.is_symlink() {
        return Err(Error::new("authored transaction became a broken symlink"));
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // One fail-closed validation surface for the signed transaction schema.
pub(crate) fn validate(config: &Config, transaction: &AuthoredTransaction) -> Result<()> {
    if transaction.schema != CORE_SCHEMA
        || transaction.appliance_id != config.appliance_id
        || transaction.model != config.model
        || transaction.prepared_at_unix_ms == 0
        || transaction.completed_at_unix_ms < transaction.prepared_at_unix_ms
        || transaction.started_at_unix_ms > transaction.prepared_at_unix_ms
        || transaction.response.is_empty()
        || transaction.response.chars().count() > 24_000
        || transaction.response_sha256 != sha256(transaction.response.as_bytes())
        || transaction.summary.is_empty()
        || transaction.summary.chars().count() > 320
        || transaction.summary_sha256 != sha256(transaction.summary.as_bytes())
        || transaction.prompt_chars == 0
        || transaction.tools_used.len() > 8
        || transaction.provider_calls > 8
        || transaction.provenance != "model_authored_runtime_scheduled"
        || transaction.authority
            != "rich_authored_response_with_optional_separate_clean_source_review_idempotent_publication"
    {
        return Err(Error::new("authored transaction content is invalid"));
    }
    for (value, label) in [
        (&transaction.due_nonce, "due nonce"),
        (&transaction.trace_id, "trace id"),
        (&transaction.session_id, "session id"),
        (&transaction.turn_id, "turn id"),
        (&transaction.span_id, "span id"),
    ] {
        crate::util::validate_identifier(value, label)?;
    }
    for tool in &transaction.tools_used {
        crate::util::validate_identifier(tool, "tool name")?;
    }
    for (value, label) in [
        (&transaction.prompt_sha256, "prompt hash"),
        (&transaction.response_sha256, "response hash"),
        (&transaction.summary_sha256, "summary hash"),
    ] {
        crate::util::validate_hex64(value, label)?;
    }
    if transaction.candidate.is_some() && transaction.unattested_proposal_binding.is_some() {
        return Err(Error::new(
            "authored transaction cannot both publish and reopen a candidate",
        ));
    }
    transaction.context_provenance.validate()?;
    validate_source_review(transaction)?;
    if let Some(proposal_binding) = &transaction.unattested_proposal_binding {
        crate::util::validate_hex64(proposal_binding, "unattested proposal binding")?;
    }
    if let Some(candidate) = &transaction.candidate {
        let review = transaction
            .source_review
            .as_ref()
            .ok_or_else(|| Error::new("candidate publication lacks a clean source review"))?;
        for (value, label) in [
            (&candidate.envelope_id, "envelope id"),
            (&candidate.intent_id, "intent id"),
            (&candidate.trace_id, "candidate trace id"),
            (&candidate.session_id, "candidate session id"),
            (&candidate.turn_id, "candidate turn id"),
            (&candidate.candidate_id, "candidate id"),
            (&candidate.base_generation, "candidate base generation"),
        ] {
            crate::util::validate_identifier(value, label)?;
        }
        let source_digest = candidate
            .source_id
            .strip_prefix("cpu-edge:")
            .ok_or_else(|| Error::new("authored candidate source identity is invalid"))?;
        crate::util::validate_hex64(source_digest, "authored candidate source id")?;
        for (value, label) in [
            (&candidate.envelope_sha256, "intent envelope hash"),
            (&candidate.response_sha256, "candidate response hash"),
            (
                &candidate.terminal_declaration_sha256,
                "terminal declaration hash",
            ),
            (&candidate.candidate_sha256, "candidate hash"),
            (
                &candidate.context_provenance_sha256,
                "candidate context provenance hash",
            ),
        ] {
            crate::util::validate_hex64(value, label)?;
        }
        if candidate.trace_id != transaction.trace_id
            || candidate.session_id != transaction.session_id
            || Some(candidate.turn_id.as_str()) != review.turn_id.as_deref()
            || Some(candidate.response_sha256.as_str()) != review.response_sha256.as_deref()
            || review
                .exact_candidate_author_response
                .as_deref()
                .is_none_or(|response| sha256(response.as_bytes()) != candidate.response_sha256)
            || candidate.context_provenance_sha256 != review.context_provenance.digest()?
            || candidate.envelope_sha256 != sha256(&canonical_json(&candidate.envelope)?)
            || candidate.binding.get("schema").and_then(Value::as_str)
                != Some("astrid.edge.steward_helper.intent_binding_receipt.v1")
            || candidate
                .binding
                .get("appliance_id")
                .and_then(Value::as_str)
                != Some(transaction.appliance_id.as_str())
            || candidate.binding.get("due_nonce").and_then(Value::as_str)
                != Some(transaction.due_nonce.as_str())
            || candidate.binding.get("trace_id").and_then(Value::as_str)
                != Some(transaction.trace_id.as_str())
            || candidate.binding.get("session_id").and_then(Value::as_str)
                != Some(transaction.session_id.as_str())
            || candidate.binding.get("turn_id").and_then(Value::as_str)
                != Some(candidate.turn_id.as_str())
            || candidate.binding.get("model").and_then(Value::as_str)
                != Some(transaction.model.as_str())
            || candidate
                .binding
                .get("response_sha256")
                .and_then(Value::as_str)
                != Some(candidate.response_sha256.as_str())
            || candidate
                .binding
                .get("terminal_declaration_sha256")
                .and_then(Value::as_str)
                != Some(candidate.terminal_declaration_sha256.as_str())
            || candidate
                .binding
                .get("candidate_sha256")
                .and_then(Value::as_str)
                != Some(candidate.candidate_sha256.as_str())
            || candidate.binding.get("source_id").and_then(Value::as_str)
                != Some(candidate.source_id.as_str())
            || candidate
                .binding
                .get("base_generation")
                .and_then(Value::as_str)
                != Some(candidate.base_generation.as_str())
            || candidate
                .binding
                .get("provider_provenance")
                .and_then(Value::as_str)
                != Some("ExactModel:direct_nonstreaming_loopback_no_retry")
            || candidate.terminal_declaration.is_empty()
            || candidate.terminal_declaration.contains(['\n', '\r', '\0'])
            || candidate.terminal_declaration_sha256
                != sha256(candidate.terminal_declaration.as_bytes())
        {
            return Err(Error::new(
                "authored candidate publication does not bind its response",
            ));
        }
    }
    Ok(())
}

fn validate_source_review(transaction: &AuthoredTransaction) -> Result<()> {
    let Some(review) = &transaction.source_review else {
        if transaction.candidate.is_some() || transaction.unattested_proposal_binding.is_some() {
            return Err(Error::new(
                "candidate state requires an explicit clean source-review outcome",
            ));
        }
        return Ok(());
    };
    validate_source_review_shape(review)?;
    validate_source_review_start(review)?;
    validate_source_review_budget(transaction, review)?;
    validate_source_review_terminal(transaction, review)
}

fn validate_source_review_shape(review: &SourceReviewOutcome) -> Result<()> {
    if review.schema != "astrid.edge.steward_helper.source_review_outcome.v1"
        || !matches!(
            review.status.as_str(),
            "requested_pending_clean"
                | "interrupted_by_restart_non_authored"
                | "failed_non_authored"
                | "completed_no_candidate"
                | "candidate_attested"
        )
        || review.tools_used.len() > 8
        || review.authority
            != "fresh_clean_context_no_rich_owned_or_web_content_candidate_authority_only_when_attested"
    {
        return Err(Error::new("source-review outcome is invalid"));
    }
    review.context_provenance.validate()?;
    if !review.context_provenance.candidate_authoring_eligible() {
        return Err(Error::new("source-review outcome is not clean"));
    }
    for tool in &review.tools_used {
        crate::util::validate_identifier(tool, "source-review tool")?;
    }
    for value in [&review.turn_id, &review.span_id].into_iter().flatten() {
        crate::util::validate_identifier(value, "source-review identifier")?;
    }
    for value in [&review.prompt_sha256, &review.response_sha256]
        .into_iter()
        .flatten()
    {
        crate::util::validate_hex64(value, "source-review hash")?;
    }
    Ok(())
}

fn validate_source_review_start(review: &SourceReviewOutcome) -> Result<()> {
    let started = review.turn_id.is_some()
        && review.span_id.is_some()
        && review.prompt_sha256.is_some()
        && review.prompt_chars > 0;
    let partially_started = review.turn_id.is_some()
        || review.span_id.is_some()
        || review.prompt_sha256.is_some()
        || review.prompt_chars > 0;
    if partially_started && !started {
        return Err(Error::new(
            "source review carries a partial model-start identity",
        ));
    }
    if matches!(review.status.as_str(), "requested_pending_clean") && partially_started {
        return Err(Error::new(
            "pending source review already carries model-start fields",
        ));
    }
    if matches!(
        review.status.as_str(),
        "completed_no_candidate" | "candidate_attested"
    ) && (!started || review.provider_calls == 0)
    {
        return Err(Error::new(
            "terminal source review lacks model-start fields",
        ));
    }
    Ok(())
}

fn validate_source_review_budget(
    transaction: &AuthoredTransaction,
    review: &SourceReviewOutcome,
) -> Result<()> {
    if review.provider_calls > 8
        || transaction
            .provider_calls
            .saturating_add(review.provider_calls)
            > 8
    {
        return Err(Error::new(
            "rich and clean source-review provider calls exceed the combined ceiling",
        ));
    }
    Ok(())
}

fn validate_source_review_terminal(
    transaction: &AuthoredTransaction,
    review: &SourceReviewOutcome,
) -> Result<()> {
    let exact_response = review.exact_candidate_author_response.as_deref();
    if review.status == "candidate_attested" {
        let response = exact_response
            .ok_or_else(|| Error::new("attested source review lacks exact author response"))?;
        let exact_response_sha256 = sha256(response.as_bytes());
        if response.is_empty()
            || response.chars().count() > 24_000
            || review.response_sha256.as_deref() != Some(exact_response_sha256.as_str())
            || review.failure_class.is_some()
            || transaction.candidate.is_none()
        {
            return Err(Error::new(
                "attested source-review response binding is invalid",
            ));
        }
    } else if exact_response.is_some() || transaction.candidate.is_some() {
        return Err(Error::new(
            "non-attested source review cannot retain author text or candidate authority",
        ));
    }
    if matches!(
        review.status.as_str(),
        "failed_non_authored" | "interrupted_by_restart_non_authored"
    ) != review.failure_class.is_some()
    {
        return Err(Error::new(
            "source-review failure provenance is inconsistent",
        ));
    }
    Ok(())
}

fn transaction_root(config: &Config) -> PathBuf {
    config.state_root.join("authored-transactions")
}

fn transaction_path(config: &Config, due_nonce: &str) -> PathBuf {
    transaction_root(config).join(format!("{due_nonce}.json"))
}

fn completion_path(config: &Config, due_nonce: &str) -> PathBuf {
    config.state_root.join("completed-nonces").join(due_nonce)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::symlink;

    use super::{AuthoredTransaction, load, prepare, verify_completion, write_completion};
    use crate::attestation::HmacSigner;

    #[test]
    fn transaction_reader_rejects_links_before_recovery() {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let key = temporary.path().join("key");
        fs::write(&key, [b'k'; 32]).unwrap();
        let signer = HmacSigner::from_file(&key).unwrap();
        let mut config = fixture_config(temporary.path());
        config.state_root = temporary.path().join("state");
        fs::create_dir(&config.state_root).unwrap();
        let root = config.state_root.join("authored-transactions");
        fs::create_dir(&root).unwrap();
        let outside = temporary.path().join("outside");
        fs::write(&outside, b"not a transaction").unwrap();
        symlink(&outside, root.join("due-10000.json")).unwrap();
        assert!(load(&config, &signer, "due-10000").is_err());
    }

    #[test]
    fn same_due_can_only_prepare_the_exact_same_complete_response() {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let key = temporary.path().join("key");
        fs::write(&key, [b'k'; 32]).unwrap();
        let signer = HmacSigner::from_file(&key).unwrap();
        let config = fixture_config(temporary.path());
        fs::create_dir(&config.state_root).unwrap();
        let transaction = fixture_transaction();
        prepare(&config, &signer, &transaction).unwrap();
        prepare(&config, &signer, &transaction).unwrap();
        let mut changed = transaction;
        changed.response = "different response".to_owned();
        changed.response_sha256 = crate::util::sha256(changed.response.as_bytes());
        assert!(prepare(&config, &signer, &changed).is_err());
    }

    #[test]
    fn completion_requires_exact_authenticated_transaction_binding() {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let key = temporary.path().join("key");
        fs::write(&key, [b'k'; 32]).unwrap();
        let signer = HmacSigner::from_file(&key).unwrap();
        let config = fixture_config(temporary.path());
        fs::create_dir(&config.state_root).unwrap();
        let transaction = fixture_transaction();
        write_completion(&config, &signer, &transaction).unwrap();
        assert!(verify_completion(&config, &signer, &transaction.due_nonce).unwrap());
        let completion = config
            .state_root
            .join("completed-nonces")
            .join(&transaction.due_nonce);
        fs::write(
            &completion,
            b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .unwrap();
        assert!(verify_completion(&config, &signer, &transaction.due_nonce).is_err());
    }

    #[test]
    fn completion_cannot_retire_a_different_prepared_transaction() {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let key = temporary.path().join("key");
        fs::write(&key, [b'k'; 32]).unwrap();
        let signer = HmacSigner::from_file(&key).unwrap();
        let config = fixture_config(temporary.path());
        fs::create_dir(&config.state_root).unwrap();
        let completed = fixture_transaction();
        write_completion(&config, &signer, &completed).unwrap();
        let mut prepared = completed;
        prepared.response = "A different complete reflection.".to_owned();
        prepared.response_sha256 = crate::util::sha256(prepared.response.as_bytes());
        prepare(&config, &signer, &prepared).unwrap();
        assert!(verify_completion(&config, &signer, &prepared.due_nonce).is_err());
    }

    #[test]
    fn legacy_recovery_can_finish_reflection_but_cannot_reopen_or_publish_a_draft() {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let config = fixture_config(temporary.path());
        let mut legacy = fixture_transaction();
        legacy.context_provenance =
            crate::context_provenance::ContextProvenance::legacy_unattributed();
        assert!(super::validate(&config, &legacy).is_ok());
        legacy.unattested_proposal_binding = Some("f".repeat(64));
        assert!(super::validate(&config, &legacy).is_err());
    }

    fn fixture_transaction() -> AuthoredTransaction {
        let response = "A complete reflection.".to_owned();
        let summary = response.clone();
        AuthoredTransaction {
            schema: super::CORE_SCHEMA.to_owned(),
            prepared_at_unix_ms: 2,
            completed_at_unix_ms: 2,
            started_at_unix_ms: 1,
            appliance_id: "avado-test".to_owned(),
            model: "model-test".to_owned(),
            due_nonce: "due-10000".to_owned(),
            trace_id: "11111111-1111-4111-8111-111111111111".to_owned(),
            session_id: "session-one".to_owned(),
            turn_id: "22222222-2222-4222-8222-222222222222".to_owned(),
            span_id: "33333333-3333-4333-8333-333333333333".to_owned(),
            prompt_sha256: "a".repeat(64),
            prompt_chars: 10,
            response_sha256: crate::util::sha256(response.as_bytes()),
            response,
            summary_sha256: crate::util::sha256(summary.as_bytes()),
            summary,
            tools_used: Vec::new(),
            provider_calls: 1,
            prompt_tokens: 1,
            completion_tokens: 1,
            provider_elapsed_ms: 1,
            context_provenance: crate::context_provenance::ContextProvenance::clean(),
            source_review: None,
            candidate: None,
            unattested_proposal_binding: None,
            provenance: "model_authored_runtime_scheduled".to_owned(),
            authority:
                "rich_authored_response_with_optional_separate_clean_source_review_idempotent_publication"
                    .to_owned(),
        }
    }

    fn fixture_config(root: &std::path::Path) -> crate::Config {
        let owned_inputs = vec![
            serde_json::json!({"kind":"continuity","path":root.join("workspace/autonomous/thread_state.json"),"maximum_files":1,"maximum_bytes_per_file":1024}),
            serde_json::json!({"kind":"self_profile","path":root.join("workspace/self/profile.json"),"maximum_files":1,"maximum_bytes_per_file":1024}),
            serde_json::json!({"kind":"verified_evidence","path":root.join("workspace/autonomous/thread_state.jsonl"),"maximum_files":1,"maximum_bytes_per_file":1024}),
            serde_json::json!({"kind":"machine_observation","path":root.join("workspace/perception/latest.json"),"maximum_files":1,"maximum_bytes_per_file":1024}),
            serde_json::json!({"kind":"spectral_host_state","path":root.join("workspace/runtime/spectral_state.json"),"maximum_files":1,"maximum_bytes_per_file":1024}),
        ];
        serde_json::from_value(serde_json::json!({
            "schema": crate::CONFIG_SCHEMA,
            "appliance_id": "avado-test",
            "target": "x86_64-unknown-linux-gnu",
            "model": "model-test",
            "ollama_origin": "http://127.0.0.1:11434",
            "connect_timeout_ms": 1000,
            "header_timeout_ms": 1000,
            "total_timeout_ms": 2000,
            "provider_broker": null,
            "web_broker": null,
            "context_tokens": 1024,
            "output_tokens": 64,
            "source_authoring_output_tokens": 128,
            "model_lock": root.join("model.lock"),
            "workspace_root": root.join("workspace"),
            "workspace_uid": 1,
            "workspace_gid": 1,
            "source_root": root.join("source"),
            "source_manifest": root.join("source/MANIFEST.json"),
            "source_manifest_sha256": "a".repeat(64),
            "source_signature": root.join("source/MANIFEST.signature.json"),
            "expected_source_id": format!("cpu-edge:{}", "b".repeat(64)),
            "active_generation_link": root.join("appliance/current"),
            "maintenance_lease": root.join("supervisor/maintenance.json"),
            "source_signing_key": root.join("source.key"),
            "source_signing_key_sha256": "c".repeat(64),
            "attestor_key": root.join("intent.key"),
            "attestor_key_sha256": "d".repeat(64),
            "state_root": root.join("state"),
            "supervisor_inbox": root.join("inbox"),
            "supervisor_status": root.join("status"),
            "current_generation": root.join("supervisor/current-generation"),
            "patch_export_root": root.join("workspace/self-change/patch-outbox"),
            "owned_inputs": owned_inputs,
            "gates": {"autonomy_state":root.join("workspace/autonomy.json"),"action_receipts":root.join("workspace/actions.jsonl"),"thermal_celsius":root.join("thermal"),"maximum_thermal_celsius":90}
        }))
        .unwrap()
    }
}
