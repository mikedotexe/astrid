//! Crash-safe publication of an exact model-authored candidate intent.
//!
//! A candidate patch is inert. Authority begins only when the attestor envelope is durably
//! visible in the immutable supervisor inbox. The prepared record below contains every byte
//! needed to finish that publication idempotently after a crash; when no authority artifact was
//! published, recovery restores the same candidate to editing instead.

use std::fs::File;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::attestation::HmacSigner;
use crate::authored_transaction::CandidatePublication;
use crate::candidate::{ActiveDraft, CandidateManager, SubmittedCandidate};
use crate::candidate_ledger::EventContext;
use crate::config::Config;
use crate::util::{
    atomic_private_write, canonical_json, read_stable_regular, sha256, unix_seconds,
    validate_hex64, validate_identifier,
};
use crate::{Error, Result};

const PREPARED_SCHEMA: &str = "astrid.edge.steward_helper.intent_publication_prepared.v1";
const COMMITTED_SCHEMA: &str = "astrid.edge.steward_helper.intent_publication_committed.v1";
const ABORTED_SCHEMA: &str = "astrid.edge.steward_helper.intent_publication_aborted.v1";
const ENVELOPE_SCHEMA: &str = "astrid.edge.steward_helper.intent_publication_envelope.v1";
const MAX_TRANSACTION_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone)]
pub struct PublicationInput<'a> {
    pub appliance_id: &'a str,
    pub due_nonce: &'a str,
    pub trace_id: &'a str,
    pub session_id: &'a str,
    pub turn_id: &'a str,
    pub model: &'a str,
    pub response_sha256: &'a str,
    pub context_provenance_sha256: &'a str,
    pub terminal_declaration: &'a str,
    pub source_id: &'a str,
    pub base_generation: &'a str,
    pub candidate: &'a SubmittedCandidate,
    pub intent_envelope_id: &'a str,
    pub intent_envelope: &'a Value,
    pub intent_binding: &'a Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorProjection {
    Idle,
    Matching,
    BusyOrDifferent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recovery {
    NoTransaction,
    RestoredEditing,
    FinalizedSubmitted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparedCore {
    schema: String,
    phase: String,
    transaction_id: String,
    prepared_at: u64,
    appliance_id: String,
    due_nonce: String,
    trace_id: String,
    session_id: String,
    turn_id: String,
    model: String,
    response_sha256: String,
    context_provenance_sha256: String,
    terminal_declaration: String,
    terminal_declaration_sha256: String,
    source_id: String,
    base_generation: String,
    candidate_id: String,
    candidate_sha256: String,
    patch_sha256: String,
    proposal_sha256: String,
    prepared_draft_sha256: String,
    intent_envelope_id: String,
    intent_filename: String,
    intent_envelope_sha256: String,
    intent_envelope: Value,
    intent_binding_filename: String,
    intent_binding_sha256: String,
    intent_binding: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TerminalCore {
    schema: String,
    phase: String,
    recorded_at: u64,
    transaction_id: String,
    prepared_record_sha256: String,
    candidate_id: String,
    candidate_sha256: String,
    intent_envelope_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SignedEnvelope<T> {
    schema: String,
    core: T,
    core_sha256: String,
    auth: Auth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Auth {
    algorithm: String,
    key_id: String,
    signature: String,
}

pub fn publish(
    config: &Config,
    manager: &CandidateManager<'_>,
    signer: &HmacSigner,
    input: &PublicationInput<'_>,
) -> Result<PathBuf> {
    let prepared = prepare_core(manager, input)?;
    let directory = transaction_directory(config, input.candidate)?;
    crate::util::ensure_private_dir(&directory)?;
    let prepared_path = directory.join("prepared.json");
    let prepared_bytes = signed(signer, &prepared)?;
    write_exact_or_verify(&prepared_path, &prepared_bytes)?;
    let prepared_sha256 = sha256(&prepared_bytes);

    manager.publish_patch(input.candidate)?;
    write_exact_or_verify(
        &config
            .state_root
            .join("intent-bindings")
            .join(&prepared.intent_binding_filename),
        &canonical_json(&prepared.intent_binding)?,
    )?;
    let intent_path = config.supervisor_inbox.join(&prepared.intent_filename);
    write_exact_or_verify(&intent_path, &canonical_json(&prepared.intent_envelope)?)?;
    manager.mark_submitted(
        input.candidate,
        &EventContext {
            due_nonce: input.due_nonce,
            trace_id: input.trace_id,
            session_id: input.session_id,
            turn_id: input.turn_id,
            response_sha256: input.response_sha256,
            declaration_sha256: &prepared.terminal_declaration_sha256,
            context_provenance_sha256: input.context_provenance_sha256,
        },
    )?;
    persist_committed(signer, &directory, &prepared, &prepared_sha256)?;
    Ok(intent_path)
}

/// Publish or finish publishing the exact transaction-bound candidate.
///
/// A restart can observe either the prepared or submitted draft state. Both converge on the
/// same create-or-verify authority artifact; editing or missing state is never inferred.
pub fn publish_idempotent(
    config: &Config,
    manager: &CandidateManager<'_>,
    signer: &HmacSigner,
    input: &PublicationInput<'_>,
) -> Result<PathBuf> {
    let intent_path = config.supervisor_inbox.join(format!(
        "candidate-intent-{}.json",
        input.intent_envelope_id
    ));
    match manager.active()? {
        Some(ActiveDraft::Prepared(candidate)) => {
            require_exact_candidate(&candidate, input.candidate)?;
            publish(config, manager, signer, input)
        },
        Some(ActiveDraft::Submitted(candidate)) => {
            require_exact_candidate(&candidate, input.candidate)?;
            let _ = recover_submitted(config, manager, signer, &candidate)?;
            if read_stable_regular(&intent_path, MAX_TRANSACTION_BYTES)?
                != canonical_json(input.intent_envelope)?
            {
                return Err(Error::new(
                    "recovered intent artifact differs from authored transaction",
                ));
            }
            Ok(intent_path)
        },
        Some(ActiveDraft::Editing) | None => Err(Error::new(
            "authored publication has no exact prepared or submitted candidate",
        )),
    }
}

/// Finish an already-visible completed-intent publication using only the retained authenticated
/// authored transaction and publication transaction. This path deliberately does not load the
/// active source generation: the immutable supervisor may have activated the candidate between
/// publishing the intent and the steward's next recovery pass.
pub fn finish_visible_from_retained(
    config: &Config,
    signer: &HmacSigner,
    candidate: &CandidatePublication,
    completed_intent_envelope: &Value,
) -> Result<Option<PathBuf>> {
    let directory =
        transaction_directory_parts(config, &candidate.candidate_id, &candidate.candidate_sha256)?;
    let prepared_path = directory.join("prepared.json");
    if !prepared_path.exists() {
        if prepared_path.is_symlink() {
            return Err(Error::new(
                "prepared intent transaction is a broken symlink",
            ));
        }
        return Ok(None);
    }
    let prepared_bytes = read_stable_regular(&prepared_path, MAX_TRANSACTION_BYTES)?;
    let prepared: PreparedCore = verify_signed(signer, &prepared_bytes, PREPARED_SCHEMA)?;
    validate_retained_prepared(config, candidate, completed_intent_envelope, &prepared)?;
    let prepared_sha256 = sha256(&prepared_bytes);
    validate_terminal_record_if_present(signer, &directory, &prepared, &prepared_sha256)?;
    let publication = find_published_intent(config, &prepared)?;
    if directory.join("aborted.json").exists() {
        if publication.is_some() {
            return Err(Error::new(
                "completed intent appeared after its publication transaction was aborted",
            ));
        }
        return Err(Error::new(
            "authored completion is bound to an aborted publication transaction",
        ));
    }
    let Some(path) = publication else {
        if directory.join("committed.json").exists() {
            return Err(Error::new(
                "committed publication lost its exact completed intent",
            ));
        }
        return Ok(None);
    };
    write_exact_or_verify(
        &config
            .state_root
            .join("intent-bindings")
            .join(&prepared.intent_binding_filename),
        &canonical_json(&prepared.intent_binding)?,
    )?;
    persist_committed(signer, &directory, &prepared, &prepared_sha256)?;
    Ok(Some(path))
}

fn require_exact_candidate(
    observed: &SubmittedCandidate,
    expected: &SubmittedCandidate,
) -> Result<()> {
    if observed.candidate_id != expected.candidate_id
        || observed.candidate_sha256 != expected.candidate_sha256
        || observed.patch_sha256 != expected.patch_sha256
        || observed.manifest.base_generation != expected.manifest.base_generation
    {
        return Err(Error::new(
            "candidate state differs from authored publication binding",
        ));
    }
    Ok(())
}

/// Recover a prepared publication before admitting another model turn.
///
/// An exact inbox/processed artifact proves that authority was published and must be completed.
/// With no such artifact and an idle supervisor, the prepared draft is safely reopened. Any
/// ambiguous artifact or conflicting supervisor projection fails closed for operator review.
pub fn recover(
    config: &Config,
    manager: &CandidateManager<'_>,
    signer: &HmacSigner,
    candidate: &SubmittedCandidate,
    projection: SupervisorProjection,
) -> Result<Recovery> {
    let directory = transaction_directory(config, candidate)?;
    let prepared_path = directory.join("prepared.json");
    if !prepared_path.exists() {
        if prepared_path.is_symlink() {
            return Err(Error::new(
                "prepared intent transaction is a broken symlink",
            ));
        }
        if projection != SupervisorProjection::Idle {
            return Err(Error::new(
                "prepared candidate has supervisor activity but no durable intent transaction",
            ));
        }
        if !manager.reopen_unattested(&candidate.manifest.proposal_sha256)? {
            return Err(Error::new(
                "unattested prepared candidate could not be restored",
            ));
        }
        return Ok(Recovery::RestoredEditing);
    }
    let prepared_bytes = read_stable_regular(&prepared_path, MAX_TRANSACTION_BYTES)?;
    let prepared: PreparedCore = verify_signed(signer, &prepared_bytes, PREPARED_SCHEMA)?;
    validate_prepared(config, manager, candidate, &prepared, true)?;
    let prepared_sha256 = sha256(&prepared_bytes);
    validate_terminal_record_if_present(signer, &directory, &prepared, &prepared_sha256)?;

    let publication = find_published_intent(config, &prepared)?;
    if directory.join("aborted.json").exists() && publication.is_some() {
        return Err(Error::new(
            "authority appeared after the publication transaction was durably aborted",
        ));
    }
    if directory.join("committed.json").exists() && publication.is_none() {
        return Err(Error::new(
            "committed publication lost its exact authority artifact",
        ));
    }
    if publication.is_some() {
        if projection == SupervisorProjection::BusyOrDifferent {
            return Err(Error::new(
                "published intent conflicts with immutable supervisor projection",
            ));
        }
        manager.publish_patch(candidate)?;
        write_exact_or_verify(
            &config
                .state_root
                .join("intent-bindings")
                .join(&prepared.intent_binding_filename),
            &canonical_json(&prepared.intent_binding)?,
        )?;
        manager.mark_submitted(candidate, &event_context(&prepared))?;
        persist_committed(signer, &directory, &prepared, &prepared_sha256)?;
        Ok(Recovery::FinalizedSubmitted)
    } else {
        if projection != SupervisorProjection::Idle {
            return Err(Error::new(
                "supervisor reports candidate authority but exact published intent is absent",
            ));
        }
        persist_aborted(signer, &directory, &prepared, &prepared_sha256)?;
        if !manager.reopen_unattested(&candidate.manifest.proposal_sha256)? {
            return Err(Error::new(
                "prepared candidate could not be safely restored",
            ));
        }
        Ok(Recovery::RestoredEditing)
    }
}

/// Complete the audit chain after a crash that happened after Draft became Submitted but before
/// the committed record was durable. Legacy submitted drafts without a transaction remain valid.
pub fn recover_submitted(
    config: &Config,
    manager: &CandidateManager<'_>,
    signer: &HmacSigner,
    candidate: &SubmittedCandidate,
) -> Result<Recovery> {
    let directory = transaction_directory(config, candidate)?;
    let prepared_path = directory.join("prepared.json");
    if !prepared_path.exists() {
        if prepared_path.is_symlink() {
            return Err(Error::new(
                "prepared intent transaction is a broken symlink",
            ));
        }
        return Ok(Recovery::NoTransaction);
    }
    let prepared_bytes = read_stable_regular(&prepared_path, MAX_TRANSACTION_BYTES)?;
    let prepared: PreparedCore = verify_signed(signer, &prepared_bytes, PREPARED_SCHEMA)?;
    validate_prepared(config, manager, candidate, &prepared, false)?;
    let prepared_sha256 = sha256(&prepared_bytes);
    validate_terminal_record_if_present(signer, &directory, &prepared, &prepared_sha256)?;
    if find_published_intent(config, &prepared)?.is_none() {
        return Err(Error::new(
            "submitted draft has no exact published authority artifact",
        ));
    }
    manager.publish_patch(candidate)?;
    write_exact_or_verify(
        &config
            .state_root
            .join("intent-bindings")
            .join(&prepared.intent_binding_filename),
        &canonical_json(&prepared.intent_binding)?,
    )?;
    manager.mark_submitted(candidate, &event_context(&prepared))?;
    persist_committed(signer, &directory, &prepared, &prepared_sha256)?;
    Ok(Recovery::FinalizedSubmitted)
}

fn event_context(prepared: &PreparedCore) -> EventContext<'_> {
    EventContext {
        due_nonce: &prepared.due_nonce,
        trace_id: &prepared.trace_id,
        session_id: &prepared.session_id,
        turn_id: &prepared.turn_id,
        response_sha256: &prepared.response_sha256,
        declaration_sha256: &prepared.terminal_declaration_sha256,
        context_provenance_sha256: &prepared.context_provenance_sha256,
    }
}

fn prepare_core(
    manager: &CandidateManager<'_>,
    input: &PublicationInput<'_>,
) -> Result<PreparedCore> {
    for (value, label) in [
        (input.appliance_id, "appliance_id"),
        (input.due_nonce, "due_nonce"),
        (input.trace_id, "trace_id"),
        (input.session_id, "session_id"),
        (input.turn_id, "turn_id"),
        (input.intent_envelope_id, "intent_envelope_id"),
    ] {
        validate_identifier(value, label)?;
    }
    for (value, label) in [
        (input.response_sha256, "response_sha256"),
        (input.context_provenance_sha256, "context_provenance_sha256"),
        (&input.candidate.candidate_sha256, "candidate_sha256"),
        (&input.candidate.patch_sha256, "patch_sha256"),
        (&input.candidate.manifest.proposal_sha256, "proposal_sha256"),
    ] {
        validate_hex64(value, label)?;
    }
    if input.terminal_declaration.is_empty()
        || input.terminal_declaration.chars().count() > 512
        || input.terminal_declaration.contains(['\n', '\r', '\0'])
        || input.base_generation != input.candidate.manifest.base_generation
        || input.model.is_empty()
        || input.model.len() > 128
        || input.model.chars().any(char::is_whitespace)
    {
        return Err(Error::new("intent publication binding is malformed"));
    }
    let source_digest = input
        .source_id
        .strip_prefix("cpu-edge:")
        .ok_or_else(|| Error::new("intent publication source identity is malformed"))?;
    validate_hex64(source_digest, "source_id")?;
    let prepared_draft_sha256 = manager.prepared_draft_sha256(input.candidate)?;
    let intent_filename = format!("candidate-intent-{}.json", input.intent_envelope_id);
    let intent_binding_filename = format!("{}.json", input.intent_envelope_id);
    let intent_envelope_sha256 = sha256(&canonical_json(input.intent_envelope)?);
    let intent_binding_sha256 = sha256(&canonical_json(input.intent_binding)?);
    let terminal_declaration_sha256 = sha256(input.terminal_declaration.as_bytes());
    let transaction_id = format!(
        "publication-{}",
        &sha256(&canonical_json(&serde_json::json!({
            "candidate_sha256": input.candidate.candidate_sha256,
            "response_sha256": input.response_sha256,
            "context_provenance_sha256": input.context_provenance_sha256,
            "terminal_declaration_sha256": terminal_declaration_sha256,
            "intent_envelope_sha256": intent_envelope_sha256
        }))?)[..32]
    );
    Ok(PreparedCore {
        schema: PREPARED_SCHEMA.to_owned(),
        phase: "prepared_before_publication".to_owned(),
        transaction_id,
        prepared_at: unix_seconds(),
        appliance_id: input.appliance_id.to_owned(),
        due_nonce: input.due_nonce.to_owned(),
        trace_id: input.trace_id.to_owned(),
        session_id: input.session_id.to_owned(),
        turn_id: input.turn_id.to_owned(),
        model: input.model.to_owned(),
        response_sha256: input.response_sha256.to_owned(),
        context_provenance_sha256: input.context_provenance_sha256.to_owned(),
        terminal_declaration: input.terminal_declaration.to_owned(),
        terminal_declaration_sha256,
        source_id: input.source_id.to_owned(),
        base_generation: input.base_generation.to_owned(),
        candidate_id: input.candidate.candidate_id.clone(),
        candidate_sha256: input.candidate.candidate_sha256.clone(),
        patch_sha256: input.candidate.patch_sha256.clone(),
        proposal_sha256: input.candidate.manifest.proposal_sha256.clone(),
        prepared_draft_sha256,
        intent_envelope_id: input.intent_envelope_id.to_owned(),
        intent_filename,
        intent_envelope_sha256,
        intent_envelope: input.intent_envelope.clone(),
        intent_binding_filename,
        intent_binding_sha256,
        intent_binding: input.intent_binding.clone(),
    })
}

fn validate_prepared(
    config: &Config,
    manager: &CandidateManager<'_>,
    candidate: &SubmittedCandidate,
    prepared: &PreparedCore,
    require_prepared_draft: bool,
) -> Result<()> {
    validate_prepared_common(config, prepared)?;
    if prepared.candidate_id != candidate.candidate_id
        || prepared.candidate_sha256 != candidate.candidate_sha256
        || prepared.patch_sha256 != candidate.patch_sha256
        || prepared.proposal_sha256 != candidate.manifest.proposal_sha256
        || prepared.base_generation != candidate.manifest.base_generation
    {
        return Err(Error::new("prepared intent candidate binding failed"));
    }
    if require_prepared_draft
        && prepared.prepared_draft_sha256 != manager.prepared_draft_sha256(candidate)?
    {
        return Err(Error::new("prepared candidate draft digest changed"));
    }
    Ok(())
}

fn validate_prepared_common(config: &Config, prepared: &PreparedCore) -> Result<()> {
    let now = unix_seconds();
    if prepared.schema != PREPARED_SCHEMA
        || prepared.phase != "prepared_before_publication"
        || prepared.prepared_at == 0
        || prepared.prepared_at > now.saturating_add(60)
        || prepared.appliance_id != config.appliance_id
        || prepared.model != config.model
        || prepared.intent_filename
            != format!("candidate-intent-{}.json", prepared.intent_envelope_id)
        || prepared.intent_binding_filename != format!("{}.json", prepared.intent_envelope_id)
        || prepared.intent_envelope_sha256 != sha256(&canonical_json(&prepared.intent_envelope)?)
        || prepared.intent_binding_sha256 != sha256(&canonical_json(&prepared.intent_binding)?)
        || prepared.terminal_declaration_sha256 != sha256(prepared.terminal_declaration.as_bytes())
    {
        return Err(Error::new("prepared intent transaction binding failed"));
    }
    for (value, label) in [
        (&prepared.response_sha256, "response_sha256"),
        (
            &prepared.context_provenance_sha256,
            "context_provenance_sha256",
        ),
        (
            &prepared.terminal_declaration_sha256,
            "terminal_declaration_sha256",
        ),
        (&prepared.candidate_sha256, "candidate_sha256"),
        (&prepared.patch_sha256, "patch_sha256"),
        (&prepared.proposal_sha256, "proposal_sha256"),
        (&prepared.prepared_draft_sha256, "prepared_draft_sha256"),
        (&prepared.intent_envelope_sha256, "intent_envelope_sha256"),
        (&prepared.intent_binding_sha256, "intent_binding_sha256"),
    ] {
        validate_hex64(value, label)?;
    }
    let source_digest = prepared
        .source_id
        .strip_prefix("cpu-edge:")
        .ok_or_else(|| Error::new("prepared source identity is malformed"))?;
    validate_hex64(source_digest, "source_id")?;
    if prepared.model.is_empty()
        || prepared.model.len() > 128
        || prepared.model.chars().any(char::is_whitespace)
        || prepared.terminal_declaration.is_empty()
        || prepared.terminal_declaration.chars().count() > 512
        || prepared.terminal_declaration.contains(['\n', '\r', '\0'])
    {
        return Err(Error::new("prepared intent text binding is malformed"));
    }
    let expected_transaction_id = format!(
        "publication-{}",
        &sha256(&canonical_json(&serde_json::json!({
            "candidate_sha256": prepared.candidate_sha256,
            "response_sha256": prepared.response_sha256,
            "context_provenance_sha256": prepared.context_provenance_sha256,
            "terminal_declaration_sha256": prepared.terminal_declaration_sha256,
            "intent_envelope_sha256": prepared.intent_envelope_sha256
        }))?)[..32]
    );
    if prepared.transaction_id != expected_transaction_id {
        return Err(Error::new("prepared transaction identity is not canonical"));
    }
    for (value, label) in [
        (&prepared.transaction_id, "transaction_id"),
        (&prepared.due_nonce, "due_nonce"),
        (&prepared.trace_id, "trace_id"),
        (&prepared.session_id, "session_id"),
        (&prepared.turn_id, "turn_id"),
        (&prepared.intent_envelope_id, "intent_envelope_id"),
    ] {
        validate_identifier(value, label)?;
    }
    Ok(())
}

fn validate_retained_prepared(
    config: &Config,
    candidate: &CandidatePublication,
    completed_intent_envelope: &Value,
    prepared: &PreparedCore,
) -> Result<()> {
    validate_prepared_common(config, prepared)?;
    let nested = completed_intent_envelope
        .get("intent_envelope")
        .ok_or_else(|| Error::new("completed intent wrapper has no nested intent"))?;
    if prepared.due_nonce
        != candidate
            .binding
            .get("due_nonce")
            .and_then(Value::as_str)
            .unwrap_or_default()
        || prepared.trace_id != candidate.trace_id
        || prepared.session_id != candidate.session_id
        || prepared.turn_id != candidate.turn_id
        || prepared.response_sha256 != candidate.response_sha256
        || prepared.context_provenance_sha256 != candidate.context_provenance_sha256
        || prepared.terminal_declaration != candidate.terminal_declaration
        || prepared.terminal_declaration_sha256 != candidate.terminal_declaration_sha256
        || prepared.source_id != candidate.source_id
        || prepared.base_generation != candidate.base_generation
        || prepared.candidate_id != candidate.candidate_id
        || prepared.candidate_sha256 != candidate.candidate_sha256
        || prepared.intent_envelope_id != candidate.envelope_id
        || nested != &candidate.envelope
        || prepared.intent_envelope != *completed_intent_envelope
        || prepared.intent_envelope_sha256 != sha256(&canonical_json(completed_intent_envelope)?)
        || prepared.intent_binding != candidate.binding
        || prepared.intent_binding_sha256 != sha256(&canonical_json(&candidate.binding)?)
    {
        return Err(Error::new(
            "retained authored transaction does not match publication transaction",
        ));
    }
    Ok(())
}

fn find_published_intent(config: &Config, prepared: &PreparedCore) -> Result<Option<PathBuf>> {
    let expected = canonical_json(&prepared.intent_envelope)?;
    let direct = config.supervisor_inbox.join(&prepared.intent_filename);
    let mut matches = Vec::new();
    if direct.exists() || direct.is_symlink() {
        if read_stable_regular(&direct, MAX_TRANSACTION_BYTES)? != expected {
            return Err(Error::new(
                "published intent inbox artifact mismatches transaction",
            ));
        }
        matches.push(direct);
    }
    let processed = config.supervisor_inbox.join("processed");
    if processed.exists() || processed.is_symlink() {
        let metadata = std::fs::symlink_metadata(&processed)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(Error::new(
                "processed intent path is linked or not a directory",
            ));
        }
        let prefix = format!("candidate-intent-{}.", prepared.intent_envelope_id);
        for entry in std::fs::read_dir(&processed)? {
            let path = entry?.path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if name.starts_with(&prefix)
                && Path::new(name).extension() == Some(std::ffi::OsStr::new("json"))
            {
                if read_stable_regular(&path, MAX_TRANSACTION_BYTES)? != expected {
                    return Err(Error::new(
                        "processed intent artifact mismatches prepared transaction",
                    ));
                }
                matches.push(path);
            }
        }
    }
    if matches.len() > 1 {
        return Err(Error::new("intent publication is duplicated or replayed"));
    }
    Ok(matches.pop())
}

fn persist_committed(
    signer: &HmacSigner,
    directory: &Path,
    prepared: &PreparedCore,
    prepared_record_sha256: &str,
) -> Result<()> {
    let core = TerminalCore {
        schema: COMMITTED_SCHEMA.to_owned(),
        phase: "committed_after_intent_and_draft".to_owned(),
        recorded_at: prepared.prepared_at,
        transaction_id: prepared.transaction_id.clone(),
        prepared_record_sha256: prepared_record_sha256.to_owned(),
        candidate_id: prepared.candidate_id.clone(),
        candidate_sha256: prepared.candidate_sha256.clone(),
        intent_envelope_sha256: prepared.intent_envelope_sha256.clone(),
    };
    write_exact_or_verify(&directory.join("committed.json"), &signed(signer, &core)?)
}

fn persist_aborted(
    signer: &HmacSigner,
    directory: &Path,
    prepared: &PreparedCore,
    prepared_record_sha256: &str,
) -> Result<()> {
    let core = TerminalCore {
        schema: ABORTED_SCHEMA.to_owned(),
        phase: "aborted_no_published_authority_restored_editing".to_owned(),
        recorded_at: prepared.prepared_at,
        transaction_id: prepared.transaction_id.clone(),
        prepared_record_sha256: prepared_record_sha256.to_owned(),
        candidate_id: prepared.candidate_id.clone(),
        candidate_sha256: prepared.candidate_sha256.clone(),
        intent_envelope_sha256: prepared.intent_envelope_sha256.clone(),
    };
    write_exact_or_verify(&directory.join("aborted.json"), &signed(signer, &core)?)
}

fn validate_terminal_record_if_present(
    signer: &HmacSigner,
    directory: &Path,
    prepared: &PreparedCore,
    prepared_record_sha256: &str,
) -> Result<()> {
    for (name, schema) in [
        ("committed.json", COMMITTED_SCHEMA),
        ("aborted.json", ABORTED_SCHEMA),
    ] {
        let path = directory.join(name);
        if !path.exists() {
            if path.is_symlink() {
                return Err(Error::new(
                    "publication terminal record is a broken symlink",
                ));
            }
            continue;
        }
        let bytes = read_stable_regular(&path, MAX_TRANSACTION_BYTES)?;
        let core: TerminalCore = verify_signed(signer, &bytes, schema)?;
        if core.transaction_id != prepared.transaction_id
            || core.prepared_record_sha256 != prepared_record_sha256
            || core.candidate_id != prepared.candidate_id
            || core.candidate_sha256 != prepared.candidate_sha256
            || core.intent_envelope_sha256 != prepared.intent_envelope_sha256
        {
            return Err(Error::new("publication terminal hash chain failed"));
        }
    }
    if directory.join("committed.json").exists() && directory.join("aborted.json").exists() {
        return Err(Error::new(
            "publication transaction has conflicting terminal records",
        ));
    }
    Ok(())
}

fn transaction_directory(config: &Config, candidate: &SubmittedCandidate) -> Result<PathBuf> {
    transaction_directory_parts(config, &candidate.candidate_id, &candidate.candidate_sha256)
}

fn transaction_directory_parts(
    config: &Config,
    candidate_id: &str,
    candidate_sha256: &str,
) -> Result<PathBuf> {
    validate_identifier(candidate_id, "candidate_id")?;
    validate_hex64(candidate_sha256, "candidate_sha256")?;
    Ok(config
        .state_root
        .join("intent-transactions")
        .join(format!("{candidate_id}-{candidate_sha256}")))
}

fn signed<T>(signer: &HmacSigner, core: &T) -> Result<Vec<u8>>
where
    T: Serialize + Clone,
{
    let core_bytes = canonical_json(core)?;
    canonical_json(&SignedEnvelope {
        schema: ENVELOPE_SCHEMA.to_owned(),
        core: core.clone(),
        core_sha256: sha256(&core_bytes),
        auth: Auth {
            algorithm: "hmac-sha256".to_owned(),
            key_id: signer.key_id.clone(),
            signature: signer.sign(&core_bytes),
        },
    })
}

fn verify_signed<T>(signer: &HmacSigner, bytes: &[u8], core_schema: &str) -> Result<T>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    let envelope: SignedEnvelope<T> = serde_json::from_slice(bytes)?;
    let core_bytes = canonical_json(&envelope.core)?;
    let core_value = serde_json::to_value(&envelope.core)?;
    if envelope.schema != ENVELOPE_SCHEMA
        || envelope.core_sha256 != sha256(&core_bytes)
        || envelope.auth.algorithm != "hmac-sha256"
        || envelope.auth.key_id != signer.key_id
        || !signer.verify(&core_bytes, &envelope.auth.signature)
        || core_value.get("schema").and_then(Value::as_str) != Some(core_schema)
        || canonical_json(&envelope)? != bytes
    {
        return Err(Error::new(
            "intent publication transaction authentication failed",
        ));
    }
    Ok(envelope.core)
}

fn write_exact_or_verify(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() || path.is_symlink() {
        if read_stable_regular(path, MAX_TRANSACTION_BYTES)? != bytes {
            return Err(Error::new("intent publication artifact collision"));
        }
        return Ok(());
    }
    atomic_private_write(path, bytes)?;
    File::open(
        path.parent()
            .ok_or_else(|| Error::new("artifact has no parent"))?,
    )?
    .sync_all()?;
    Ok(())
}
