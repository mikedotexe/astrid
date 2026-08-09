//! Exact signed steward-to-builder model-unload handoff verification.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::{Config, valid_hex64, valid_identifier};
use crate::fs_guard::{canonical_json, read_regular, sha256};
use crate::manifest::Candidate;
use crate::{Error, Result};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    schema: String,
    core: Core,
    core_sha256: String,
    auth: Auth,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct Core {
    schema: String,
    recorded_at: u64,
    appliance_id: String,
    envelope_id: String,
    intent_id: String,
    trace_id: String,
    session_id: String,
    turn_id: String,
    response_sha256: String,
    terminal_declaration_sha256: String,
    intent_envelope_sha256: String,
    candidate_id: String,
    candidate_sha256: String,
    model: String,
    origin: String,
    request_sha256: String,
    result_sha256: String,
    provider_result_sha256: Option<String>,
    elapsed_ms: Option<u64>,
    result_class: String,
    status: String,
    attempt_count: u64,
    automatic_retry: bool,
    build_ready: bool,
    authorship_unchanged: bool,
    response_body_retained: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Auth {
    algorithm: String,
    key_id: String,
    signature: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct IntentEnvelope {
    schema: String,
    core: IntentCore,
    auth: Auth,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CompletedIntentEnvelope {
    schema: String,
    intent_envelope: IntentEnvelope,
    authored_completion: CompletionEnvelope,
    auth: Auth,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CompletionEnvelope {
    schema: String,
    core: CompletionCore,
    core_sha256: String,
    auth: Auth,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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
    candidate_publication: CompletionCandidatePublication,
    status: String,
    provenance: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct IntentCore {
    candidate: Candidate,
    candidate_sha256: String,
    created_at: u64,
    envelope_id: String,
    intent: AuthoredIntent,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct AuthoredIntent {
    schema: String,
    intent_id: String,
    appliance_id: String,
    trace_id: String,
    session_id: String,
    turn_id: String,
    response_sha256: String,
    terminal_declaration_sha256: String,
    candidate_id: String,
    candidate_sha256: String,
    base_generation: String,
    current_generation: String,
    observed_at: u64,
    origin: String,
    authorship_status: String,
    transport_status: String,
    declaration_provenance: String,
    fallback: bool,
    executor_repair: bool,
    operator_harness: bool,
}

pub fn verify(
    config: &Config,
    path: &Path,
    intent_envelope_path: &Path,
    candidate: &Candidate,
) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if path.parent() != Some(&config.roots.model_handoff_root)
        || !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != config.identities.steward_uid
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new("model-unload handoff path or ownership failed"));
    }
    let bytes = read_regular(path, 64 * 1024)?;
    let envelope: Envelope = serde_json::from_slice(&bytes)?;
    if canonical_json(&envelope)? != bytes {
        return Err(Error::new(
            "model-unload handoff is not exact canonical JSON",
        ));
    }
    let core_bytes = canonical_json(&envelope.core)?;
    let key = read_regular(&config.source.intent_attestation_key, 4_096)?;
    if key.len() != 32 {
        return Err(Error::new(
            "intent attestation key must be exactly 32 bytes",
        ));
    }
    let key_id = format!("hmac-sha256:{}", &sha256(&key)[..16]);
    let intent = verify_intent_envelope(config, intent_envelope_path, candidate, &key, &key_id)?;
    let expected_request = sha256(&canonical_json(&serde_json::json!({
        "model": config.model,
        "keep_alive": 0
    }))?);
    if envelope.schema != "astrid.edge.steward_helper.model_unload_handoff_envelope.v2"
        || envelope.core.schema != "astrid.edge.steward_helper.model_unload_handoff.v2"
        || envelope.core_sha256 != sha256(&core_bytes)
        || envelope.auth.algorithm != "hmac-sha256"
        || envelope.auth.key_id != key_id
        || !valid_hex64(&envelope.auth.signature)
        || !constant_time_equal(
            hmac_sha256(&key, &core_bytes).as_bytes(),
            envelope.auth.signature.as_bytes(),
        )
        || path.file_name().and_then(|name| name.to_str())
            != Some(&format!("{}.json", envelope.core.envelope_id))
        || !valid_identifier(&envelope.core.envelope_id)
        || !valid_identifier(&envelope.core.intent_id)
        || !valid_identifier(&envelope.core.trace_id)
        || !valid_identifier(&envelope.core.session_id)
        || !valid_identifier(&envelope.core.turn_id)
        || envelope.core.appliance_id != config.appliance_id
        || envelope.core.envelope_id != intent.envelope_id
        || envelope.core.intent_id != intent.intent_id
        || envelope.core.trace_id != intent.trace_id
        || envelope.core.session_id != intent.session_id
        || envelope.core.turn_id != intent.turn_id
        || envelope.core.response_sha256 != intent.response_sha256
        || envelope.core.terminal_declaration_sha256 != intent.terminal_declaration_sha256
        || envelope.core.intent_envelope_sha256 != intent.envelope_sha256
        || envelope.core.candidate_id != candidate.candidate_id
        || envelope.core.candidate_sha256 != candidate.digest()?
        || envelope.core.model != config.model
        || envelope.core.origin != config.ollama_origin
        || envelope.core.request_sha256 != expected_request
        || !valid_hex64(&envelope.core.result_sha256)
        || envelope.core.provider_result_sha256.as_deref()
            != Some(envelope.core.result_sha256.as_str())
        || envelope.core.elapsed_ms.is_none()
        || envelope.core.result_class != "exact_http_200_done_reason_unload"
        || envelope.core.status != "unload_confirmed"
        || envelope.core.attempt_count != 1
        || envelope.core.automatic_retry
        || !envelope.core.build_ready
        || !envelope.core.authorship_unchanged
        || envelope.core.response_body_retained
    {
        return Err(Error::new(
            "model-unload handoff binding or confirmation failed",
        ));
    }
    require_identical_ledger_record(config, &bytes)
}

#[derive(Debug)]
struct VerifiedIntent {
    envelope_id: String,
    envelope_sha256: String,
    intent_id: String,
    trace_id: String,
    session_id: String,
    turn_id: String,
    response_sha256: String,
    terminal_declaration_sha256: String,
}

fn verify_intent_envelope(
    config: &Config,
    path: &Path,
    candidate: &Candidate,
    key: &[u8],
    key_id: &str,
) -> Result<VerifiedIntent> {
    let (envelope, canonical) = read_completed_intent(config, path)?;
    verify_completed_intent_signatures(&envelope, key, key_id)?;
    let candidate_bytes = canonical_json(candidate)?;
    let candidate_sha256 = sha256(&candidate_bytes);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    verify_authored_intent(
        config,
        path,
        &envelope.intent_envelope,
        candidate,
        &candidate_bytes,
        &candidate_sha256,
        now,
    )?;
    verify_authored_completion(
        config,
        &envelope.authored_completion,
        &envelope.intent_envelope,
        candidate,
        &candidate_sha256,
        now,
    )?;
    let intent = &envelope.intent_envelope.core.intent;
    Ok(VerifiedIntent {
        envelope_id: envelope.intent_envelope.core.envelope_id.clone(),
        envelope_sha256: sha256(&canonical),
        intent_id: intent.intent_id.clone(),
        trace_id: intent.trace_id.clone(),
        session_id: intent.session_id.clone(),
        turn_id: intent.turn_id.clone(),
        response_sha256: intent.response_sha256.clone(),
        terminal_declaration_sha256: intent.terminal_declaration_sha256.clone(),
    })
}

fn read_completed_intent(
    config: &Config,
    path: &Path,
) -> Result<(CompletedIntentEnvelope, Vec<u8>)> {
    let expected_parent = config
        .roots
        .supervisor_state
        .join("inbox")
        .join("processed");
    let metadata = fs::symlink_metadata(path)?;
    if path.parent() != Some(expected_parent.as_path())
        || !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != nix::unistd::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new(
            "scheduled intent envelope path or ownership failed",
        ));
    }
    let bytes = read_regular(path, 256 * 1024)?;
    let envelope: CompletedIntentEnvelope = serde_json::from_slice(&bytes)?;
    let canonical = canonical_json(&envelope)?;
    if bytes != canonical && bytes != [canonical.as_slice(), b"\n"].concat() {
        return Err(Error::new(
            "completed scheduled intent envelope is not canonical JSON",
        ));
    }
    Ok((envelope, canonical))
}

fn verify_completed_intent_signatures(
    envelope: &CompletedIntentEnvelope,
    key: &[u8],
    key_id: &str,
) -> Result<()> {
    let unsigned_wrapper = canonical_json(&serde_json::json!({
        "schema": envelope.schema,
        "intent_envelope": envelope.intent_envelope,
        "authored_completion": envelope.authored_completion,
    }))?;
    let nested = &envelope.intent_envelope;
    let unsigned_intent = canonical_json(&serde_json::json!({
        "schema": nested.schema,
        "core": nested.core,
    }))?;
    let completion = &envelope.authored_completion;
    let completion_core = canonical_json(&completion.core)?;
    if envelope.schema != "astrid.edge_self_change.completed_intent_envelope.v1"
        || !valid_hmac(&envelope.auth, key_id, key, &unsigned_wrapper)
        || nested.schema != "astrid.edge_self_change.intent_attestor_envelope.v1"
        || !valid_hmac(&nested.auth, key_id, key, &unsigned_intent)
        || completion.schema != "astrid.edge.steward_helper.authored_completion_envelope.v2"
        || completion.core.schema != "astrid.edge.steward_helper.authored_completion.v2"
        || completion.core_sha256 != sha256(&completion_core)
        || !valid_hmac(&completion.auth, key_id, key, &completion_core)
    {
        return Err(Error::new(
            "completed scheduled intent signature chain failed",
        ));
    }
    Ok(())
}

fn valid_hmac(auth: &Auth, key_id: &str, key: &[u8], payload: &[u8]) -> bool {
    auth.algorithm == "hmac-sha256"
        && auth.key_id == key_id
        && valid_hex64(&auth.signature)
        && constant_time_equal(
            hmac_sha256(key, payload).as_bytes(),
            auth.signature.as_bytes(),
        )
}

fn verify_authored_intent(
    config: &Config,
    path: &Path,
    nested: &IntentEnvelope,
    candidate: &Candidate,
    candidate_bytes: &[u8],
    candidate_sha256: &str,
    now: u64,
) -> Result<()> {
    let intent = &nested.core.intent;
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let expected_prefix = format!("candidate-intent-{}.", nested.core.envelope_id);
    if !filename.starts_with(&expected_prefix)
        || path.extension().and_then(std::ffi::OsStr::to_str) != Some("json")
        || !valid_identifier(&nested.core.envelope_id)
        || nested.core.candidate_sha256 != candidate_sha256
        || canonical_json(&nested.core.candidate)? != candidate_bytes
        || nested.core.created_at > now.saturating_add(60)
        || now.saturating_sub(nested.core.created_at) > config.policy.pipeline_timeout_seconds
        || intent.schema != "astrid.edge_self_change.scheduled_model_intent.v1"
        || !valid_identifier(&intent.intent_id)
        || !valid_identifier(&intent.trace_id)
        || !valid_identifier(&intent.session_id)
        || !valid_identifier(&intent.turn_id)
        || !valid_hex64(&intent.response_sha256)
        || !valid_hex64(&intent.terminal_declaration_sha256)
        || intent.appliance_id != config.appliance_id
        || intent.candidate_id != candidate.candidate_id
        || intent.candidate_sha256 != candidate_sha256
        || intent.base_generation != candidate.base_generation
        || intent.current_generation != candidate.base_generation
        || intent.observed_at != nested.core.created_at
        || intent.origin != "scheduled_autonomy"
        || intent.authorship_status != "genuinely_authored"
        || intent.transport_status != "authored_completed"
        || intent.declaration_provenance != "exact_terminal_model_declaration"
        || intent.fallback
        || intent.executor_repair
        || intent.operator_harness
    {
        return Err(Error::new(
            "scheduled model intent authority or binding failed",
        ));
    }
    Ok(())
}

fn verify_authored_completion(
    config: &Config,
    completion: &CompletionEnvelope,
    nested: &IntentEnvelope,
    candidate: &Candidate,
    candidate_sha256: &str,
    now: u64,
) -> Result<()> {
    let intent = &nested.core.intent;
    let core = &completion.core;
    let publication = &core.candidate_publication;
    let completion_seconds = core.completed_at_unix_ms / 1_000;
    if core.appliance_id != config.appliance_id
        || !valid_identifier(&core.due_nonce)
        || !valid_identifier(&core.trace_id)
        || !valid_identifier(&core.session_id)
        || !valid_identifier(&core.turn_id)
        || !valid_hex64(&core.response_sha256)
        || !valid_hex64(&core.transaction_sha256)
        || core.completed_at_unix_ms == 0
        || completion_seconds < nested.core.created_at
        || completion_seconds > now.saturating_add(60)
        || now.saturating_sub(completion_seconds) > config.policy.pipeline_timeout_seconds
        || core.status != "authored_completed"
        || core.provenance != "model_authored_runtime_scheduled"
        || core.trace_id != intent.trace_id
        || core.session_id != intent.session_id
        || core.turn_id != intent.turn_id
        || core.response_sha256 != intent.response_sha256
        || publication.intent_envelope_id != nested.core.envelope_id
        || publication.intent_envelope_sha256 != sha256(&canonical_json(nested)?)
        || publication.intent_id != intent.intent_id
        || publication.terminal_declaration_sha256 != intent.terminal_declaration_sha256
        || publication.candidate_id != candidate.candidate_id
        || publication.candidate_sha256 != candidate_sha256
        || publication.candidate_sha256 != intent.candidate_sha256
        || publication.base_generation != candidate.base_generation
        || publication.base_generation != intent.base_generation
    {
        return Err(Error::new(
            "scheduled authored completion authority or binding failed",
        ));
    }
    Ok(())
}
fn require_identical_ledger_record(config: &Config, expected: &[u8]) -> Result<()> {
    let metadata = fs::symlink_metadata(&config.roots.model_handoff_ledger)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != config.identities.steward_uid
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new("model-unload handoff ledger ownership failed"));
    }
    let mut file = File::open(&config.roots.model_handoff_ledger)?;
    let length = file.metadata()?.len();
    let maximum = 4 * 1024 * 1024_u64;
    let start = length.saturating_sub(maximum);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(maximum).read_to_end(&mut bytes)?;
    if start > 0 {
        if let Some(newline) = bytes.iter().position(|byte| *byte == b'\n') {
            bytes.drain(..=newline);
        } else {
            bytes.clear();
        }
    }
    if !bytes
        .split(|byte| *byte == b'\n')
        .any(|line| line == expected)
    {
        return Err(Error::new(
            "per-envelope model handoff lacks its identical signed ledger record",
        ));
    }
    Ok(())
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> String {
    let mut normalized = [0_u8; 64];
    if key.len() > 64 {
        normalized[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        normalized[..key.len()].copy_from_slice(key);
    }
    let mut inner_pad = [0x36_u8; 64];
    let mut outer_pad = [0x5c_u8; 64];
    for index in 0..64 {
        inner_pad[index] ^= normalized[index];
        outer_pad[index] ^= normalized[index];
    }
    let inner = Sha256::new()
        .chain_update(inner_pad)
        .chain_update(message)
        .finalize();
    format!(
        "{:x}",
        Sha256::new()
            .chain_update(outer_pad)
            .chain_update(inner)
            .finalize()
    )
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
            == 0
}
