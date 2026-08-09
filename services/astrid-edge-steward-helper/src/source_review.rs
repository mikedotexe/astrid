//! Crash-safe handoff between the rich scheduled reflection and an optional
//! fresh clean source-authoring pass.
//!
//! The rich response is sealed before a second provider request can begin. If
//! the process stops after that boundary, recovery finalizes the rich response
//! and records the clean pass as interrupted; it never calls either model pass
//! again for the same due slot.

use std::fs::{self, File};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::attestation::HmacSigner;
use crate::authored_transaction::AuthoredTransaction;
use crate::config::Config;
use crate::util::{atomic_private_write, canonical_json, read_stable_regular, sha256};
use crate::{Error, Result};

const CHECKPOINT_SCHEMA: &str = "astrid.edge.steward_helper.rich_checkpoint.v1";
const CHECKPOINT_ENVELOPE_SCHEMA: &str = "astrid.edge.steward_helper.rich_checkpoint_envelope.v1";
const CLEAN_START_SCHEMA: &str = "astrid.edge.steward_helper.clean_start.v1";
const CLEAN_START_ENVELOPE_SCHEMA: &str = "astrid.edge.steward_helper.clean_start_envelope.v1";
const MAX_CHECKPOINT_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointCore {
    schema: String,
    transaction: AuthoredTransaction,
    transaction_sha256: String,
    authority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<T> {
    schema: String,
    core: T,
    core_sha256: String,
    key_id: String,
    hmac_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CleanStart {
    pub schema: String,
    pub appliance_id: String,
    pub due_nonce: String,
    pub trace_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub span_id: String,
    pub prompt_sha256: String,
    pub prompt_chars: usize,
    pub started_at_unix_ms: u64,
    pub rich_transaction_sha256: String,
    pub authority: String,
}

impl CleanStart {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        config: &Config,
        rich: &AuthoredTransaction,
        turn_id: String,
        span_id: String,
        prompt_sha256: String,
        prompt_chars: usize,
        started_at_unix_ms: u64,
    ) -> Result<Self> {
        let value = Self {
            schema: CLEAN_START_SCHEMA.to_owned(),
            appliance_id: config.appliance_id.clone(),
            due_nonce: rich.due_nonce.clone(),
            trace_id: rich.trace_id.clone(),
            session_id: rich.session_id.clone(),
            turn_id,
            span_id,
            prompt_sha256,
            prompt_chars,
            started_at_unix_ms,
            rich_transaction_sha256: sha256(&canonical_json(rich)?),
            authority: "fresh_clean_context_started_once_no_automatic_model_retry_after_restart"
                .to_owned(),
        };
        validate_clean_start(config, &value)?;
        Ok(value)
    }
}

pub(crate) fn persist_rich(
    config: &Config,
    signer: &HmacSigner,
    transaction: &AuthoredTransaction,
) -> Result<()> {
    crate::authored_transaction::validate(config, transaction)?;
    if transaction.candidate.is_some()
        || transaction.unattested_proposal_binding.is_some()
        || transaction
            .source_review
            .as_ref()
            .is_none_or(|review| review.status != "requested_pending_clean")
    {
        return Err(Error::new(
            "rich checkpoint must contain one pending non-authorizing source review",
        ));
    }
    let transaction_sha256 = sha256(&canonical_json(transaction)?);
    let core = CheckpointCore {
        schema: CHECKPOINT_SCHEMA.to_owned(),
        transaction: transaction.clone(),
        transaction_sha256,
        authority: "exact_rich_response_recovery_never_calls_model_again".to_owned(),
    };
    write_exact(
        &checkpoint_path(config, &transaction.due_nonce),
        signer,
        CHECKPOINT_ENVELOPE_SCHEMA,
        &core,
    )
}

pub(crate) fn load_rich(
    config: &Config,
    signer: &HmacSigner,
    due_nonce: &str,
) -> Result<Option<AuthoredTransaction>> {
    let Some(core) = read_exact::<CheckpointCore>(
        &checkpoint_path(config, due_nonce),
        signer,
        CHECKPOINT_ENVELOPE_SCHEMA,
    )?
    else {
        return Ok(None);
    };
    if core.schema != CHECKPOINT_SCHEMA
        || core.authority != "exact_rich_response_recovery_never_calls_model_again"
        || core.transaction.due_nonce != due_nonce
        || core.transaction_sha256 != sha256(&canonical_json(&core.transaction)?)
    {
        return Err(Error::new("rich checkpoint binding is invalid"));
    }
    crate::authored_transaction::validate(config, &core.transaction)?;
    Ok(Some(core.transaction))
}

pub(crate) fn mark_clean_started(
    config: &Config,
    signer: &HmacSigner,
    start: &CleanStart,
) -> Result<()> {
    validate_clean_start(config, start)?;
    let checkpoint = load_rich(config, signer, &start.due_nonce)?
        .ok_or_else(|| Error::new("clean start lacks a rich checkpoint"))?;
    if start.trace_id != checkpoint.trace_id
        || start.session_id != checkpoint.session_id
        || start.rich_transaction_sha256 != sha256(&canonical_json(&checkpoint)?)
    {
        return Err(Error::new("clean start does not bind the rich checkpoint"));
    }
    write_exact(
        &clean_start_path(config, &start.due_nonce),
        signer,
        CLEAN_START_ENVELOPE_SCHEMA,
        start,
    )
}

pub(crate) fn load_clean_start(
    config: &Config,
    signer: &HmacSigner,
    due_nonce: &str,
) -> Result<Option<CleanStart>> {
    let value = read_exact::<CleanStart>(
        &clean_start_path(config, due_nonce),
        signer,
        CLEAN_START_ENVELOPE_SCHEMA,
    )?;
    if let Some(start) = &value {
        validate_clean_start(config, start)?;
        let checkpoint = load_rich(config, signer, due_nonce)?
            .ok_or_else(|| Error::new("clean start survived without its rich checkpoint"))?;
        if start.trace_id != checkpoint.trace_id
            || start.session_id != checkpoint.session_id
            || start.rich_transaction_sha256 != sha256(&canonical_json(&checkpoint)?)
        {
            return Err(Error::new("clean start checkpoint binding failed"));
        }
    }
    Ok(value)
}

pub(crate) fn retire(config: &Config, due_nonce: &str) -> Result<()> {
    let root = root(config);
    for path in [
        checkpoint_path(config, due_nonce),
        clean_start_path(config, due_nonce),
    ] {
        if path.exists() {
            fs::remove_file(&path)?;
        } else if path.is_symlink() {
            return Err(Error::new(
                "source-review recovery artifact became a symlink",
            ));
        }
    }
    if root.exists() {
        File::open(root)?.sync_all()?;
    }
    Ok(())
}

fn validate_clean_start(config: &Config, start: &CleanStart) -> Result<()> {
    if start.schema != CLEAN_START_SCHEMA
        || start.appliance_id != config.appliance_id
        || start.prompt_chars == 0
        || start.started_at_unix_ms == 0
        || start.authority
            != "fresh_clean_context_started_once_no_automatic_model_retry_after_restart"
    {
        return Err(Error::new("clean source-review start is invalid"));
    }
    for (value, label) in [
        (&start.due_nonce, "clean due nonce"),
        (&start.trace_id, "clean trace id"),
        (&start.session_id, "clean session id"),
        (&start.turn_id, "clean turn id"),
        (&start.span_id, "clean span id"),
    ] {
        crate::util::validate_identifier(value, label)?;
    }
    for (value, label) in [
        (&start.prompt_sha256, "clean prompt hash"),
        (&start.rich_transaction_sha256, "rich transaction hash"),
    ] {
        crate::util::validate_hex64(value, label)?;
    }
    Ok(())
}

fn write_exact<T: Serialize>(
    path: &Path,
    signer: &HmacSigner,
    schema: &str,
    core: &T,
) -> Result<()> {
    let core_bytes = canonical_json(core)?;
    let envelope = Envelope {
        schema: schema.to_owned(),
        core,
        core_sha256: sha256(&core_bytes),
        key_id: signer.key_id.clone(),
        hmac_sha256: signer.sign(&core_bytes),
    };
    let bytes = canonical_json(&envelope)?;
    if bytes.len() as u64 > MAX_CHECKPOINT_BYTES {
        return Err(Error::new("source-review checkpoint exceeds its bound"));
    }
    if path.exists() || path.is_symlink() {
        if read_stable_regular(path, MAX_CHECKPOINT_BYTES)? != bytes {
            return Err(Error::new("source-review checkpoint collision"));
        }
        return Ok(());
    }
    atomic_private_write(path, &bytes)
}

fn read_exact<T: for<'de> Deserialize<'de> + Serialize>(
    path: &Path,
    signer: &HmacSigner,
    schema: &str,
) -> Result<Option<T>> {
    if !path.exists() {
        if path.is_symlink() {
            return Err(Error::new("source-review checkpoint is a broken symlink"));
        }
        return Ok(None);
    }
    let bytes = read_stable_regular(path, MAX_CHECKPOINT_BYTES)?;
    let envelope: Envelope<T> = serde_json::from_slice(&bytes)?;
    let core_bytes = canonical_json(&envelope.core)?;
    if envelope.schema != schema
        || envelope.core_sha256 != sha256(&core_bytes)
        || envelope.key_id != signer.key_id
        || !signer.verify(&core_bytes, &envelope.hmac_sha256)
        || canonical_json(&envelope)? != bytes
    {
        return Err(Error::new("source-review checkpoint authentication failed"));
    }
    Ok(Some(envelope.core))
}

fn root(config: &Config) -> PathBuf {
    config.state_root.join("source-review-transactions")
}

fn checkpoint_path(config: &Config, due_nonce: &str) -> PathBuf {
    root(config).join(format!("{due_nonce}.rich.json"))
}

fn clean_start_path(config: &Config, due_nonce: &str) -> PathBuf {
    root(config).join(format!("{due_nonce}.clean-start.json"))
}
