//! Verification and one-way admission of immutable-steward reflections.
//!
//! The immutable steward writes exact model output and a small hash-linked
//! projection into installer-created directories.  This module is deliberately
//! observational: it cannot create a reflection or authorize a candidate.  It
//! verifies the complete projection/artifact/sidecar join before exposing the
//! bounded summary to ordinary prompt continuity and the reservoir.

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{BufRead as _, BufReader, Read as _, Write as _},
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, bail, ensure};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use tokio::{sync::mpsc, time::MissedTickBehavior};
use uuid::Uuid;

use crate::{codec::encode_text, config::Config, reservoir::SensoryIngress};

const CONTINUITY_SCHEMA: &str = "astrid_edge_scheduled_introspection_continuity_v1";
const REFLECTION_SCHEMA: &str = "astrid.edge.scheduled_introspection.model_reflection.v1";
const ADMISSION_SCHEMA: &str = "astrid.edge.scheduled_introspection.admission.v1";
const PROVENANCE: &str = "model_authored_runtime_scheduled";
const AUTHORITY: &str = "bounded_continuity_projection_not_voluntary_journal";
const AUTHORSHIP_CORE_SCHEMA: &str = "astrid.edge.scheduled_authorship.attestation.v1";
const AUTHORSHIP_ENVELOPE_SCHEMA: &str = "astrid.edge.scheduled_authorship.attestation_envelope.v1";
const AUTHORSHIP_AUTHORITY: &str = "immutable_steward_signed_exact_authorship_join";
const POLL_SECONDS: u64 = 10;
const MAX_PROJECTION_BYTES: u64 = 16 * 1_024;
const MAX_REFLECTION_BYTES: u64 = 64 * 1_024;
const MAX_SUMMARY_CHARS: usize = 320;
const MAX_ATTESTATION_BYTES: u64 = 32 * 1_024;
const MAX_RECEIPT_LEDGER_BYTES: u64 = 256 * 1_024 * 1_024;
const MAX_RECEIPT_LINE_BYTES: usize = 32 * 1_024;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinuityProjection {
    schema: String,
    appliance_id: String,
    model: String,
    due_nonce: String,
    recorded_at_unix_ms: u64,
    summary: String,
    summary_sha256: String,
    response_sha256: String,
    prompt_sha256: String,
    reflection_path: String,
    trace: ProjectionTrace,
    provenance: String,
    authority: String,
    #[serde(default)]
    context_provenance: Option<Value>,
    #[serde(default)]
    context_provenance_sha256: Option<String>,
    #[serde(default)]
    candidate_authoring_eligible: Option<bool>,
    #[serde(default)]
    reflection_lane: Option<String>,
    #[serde(default)]
    taint_causes: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ProjectionTrace {
    schema_version: u8,
    trace_id: String,
    turn_id: String,
    span_id: String,
    session_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReflectionMetadata {
    schema: String,
    provenance: String,
    appliance_id: String,
    due_nonce: String,
    trace_id: String,
    session_id: String,
    turn_id: String,
    model: String,
    prompt_sha256: String,
    response_sha256: String,
    exact_response_path: String,
    #[serde(default)]
    context_provenance: Option<Value>,
    #[serde(default)]
    context_provenance_sha256: Option<String>,
    #[serde(default)]
    reflection_lane: Option<String>,
    #[serde(default)]
    taint_causes: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AuthorshipEnvelope {
    schema: String,
    core: AuthorshipCore,
    auth: AuthorshipAuthentication,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AuthorshipCore {
    schema: String,
    appliance_id: String,
    due_nonce: String,
    due_at_unix_ms: u64,
    started_at_unix_ms: u64,
    completed_at_unix_ms: u64,
    terminal_status: String,
    model: String,
    prompt_sha256: String,
    response_sha256: String,
    reflection_path: String,
    reflection_sha256: String,
    reflection_metadata_sha256: String,
    continuity_projection_sha256: String,
    state_projection_sha256: String,
    terminal_receipt_sha256: String,
    context_provenance_sha256: String,
    candidate_id: Option<String>,
    candidate_digest: Option<String>,
    trace: ProjectionTrace,
    provenance: String,
    authority: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AuthorshipAuthentication {
    algorithm: String,
    key_id: String,
    signature: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AdmissionState {
    schema: String,
    continuity_admitted: bool,
    admitted_at_unix_ms: Option<u64>,
    last_response_sha256: Option<String>,
    last_summary_sha256: Option<String>,
    last_trace_id: Option<String>,
    last_due_nonce: Option<String>,
    reservoir_delivery: Option<String>,
    provenance: Option<String>,
    authority: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct VerifiedProjection {
    pub(crate) summary: String,
    pub(crate) summary_sha256: String,
    pub(crate) response_sha256: String,
    pub(crate) trace_id: String,
    pub(crate) due_nonce: String,
    pub(crate) recorded_at_unix_ms: u64,
}

pub(crate) async fn run(config: Arc<Config>, ingress_tx: mpsc::Sender<SensoryIngress>) {
    let mut poll = tokio::time::interval(Duration::from_secs(POLL_SECONDS));
    poll.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        poll.tick().await;
        let projection = match verify_current(&config) {
            Ok(Some(projection)) => projection,
            Ok(None) => continue,
            Err(error) => {
                eprintln!("scheduled introspection projection rejected: {error:#}");
                continue;
            },
        };
        match mark_admitted(&config, &projection) {
            Ok(true) => {
                if ingress_tx
                    .send(SensoryIngress::Semantic(encode_text(
                        "scheduled_introspection",
                        &projection.summary,
                    )))
                    .await
                    .is_err()
                {
                    eprintln!(
                        "scheduled introspection reservoir admission dropped: reservoir closed"
                    );
                    return;
                }
            },
            Ok(false) => {},
            Err(error) => {
                eprintln!("scheduled introspection admission state rejected: {error:#}");
            },
        }
    }
}

/// Return only a fully verified, bounded projection for prompt continuity.
pub(crate) fn latest_verified_summary(config: &Config) -> Option<String> {
    verify_current(config)
        .ok()
        .flatten()
        .map(|projection| projection.summary)
}

/// Return the exact bounded metadata needed to merge scheduled authorship into
/// the ordinary working-thread view. The projection remains the authoritative
/// separately owned record; callers must not rewrite it or infer parentage by
/// timestamp.
pub(crate) fn latest_verified_projection(config: &Config) -> Option<VerifiedProjection> {
    verify_current(config).ok().flatten()
}

fn verify_current(config: &Config) -> anyhow::Result<Option<VerifiedProjection>> {
    if config.dedicated_steward_enabled {
        return verify_immutable_steward_current(config);
    }
    verify_legacy_runtime_current(config)
}

fn verify_legacy_runtime_current(config: &Config) -> anyhow::Result<Option<VerifiedProjection>> {
    let path = projection_path(config);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = read_stable_regular(&path, MAX_PROJECTION_BYTES)
        .context("read scheduled introspection continuity projection")?;
    let projection: ContinuityProjection =
        serde_json::from_slice(&bytes).context("decode scheduled introspection projection")?;
    validate_projection(&projection)?;

    let relative = validate_reflection_path(&projection.reflection_path)?;
    let reflection = config.workspace.join(&relative);
    let response = read_stable_regular(&reflection, MAX_REFLECTION_BYTES)
        .context("read exact scheduled reflection")?;
    ensure!(
        sha256_hex(&response) == projection.response_sha256,
        "scheduled reflection response hash mismatch"
    );
    ensure!(
        std::str::from_utf8(&response).is_ok(),
        "scheduled reflection is not valid UTF-8"
    );

    let sidecar = reflection.with_extension("json");
    let metadata_bytes = read_stable_regular(&sidecar, MAX_PROJECTION_BYTES)
        .context("read scheduled reflection metadata")?;
    let metadata: ReflectionMetadata =
        serde_json::from_slice(&metadata_bytes).context("decode scheduled reflection metadata")?;
    validate_metadata(&projection, &metadata, &reflection)?;

    Ok(Some(VerifiedProjection {
        summary: projection.summary,
        summary_sha256: projection.summary_sha256,
        response_sha256: projection.response_sha256,
        trace_id: projection.trace.trace_id,
        due_nonce: projection.due_nonce,
        recorded_at_unix_ms: projection.recorded_at_unix_ms,
    }))
}

#[allow(clippy::too_many_lines)] // One exact immutable-authorship evidence join.
fn verify_immutable_steward_current(config: &Config) -> anyhow::Result<Option<VerifiedProjection>> {
    let attestation_path = config
        .scheduled_authorship_attestation_path
        .as_deref()
        .context("dedicated steward has no immutable authorship attestation path")?;
    if !attestation_path.exists() {
        return Ok(None);
    }
    let steward_uid = config
        .scheduled_authorship_steward_uid
        .context("dedicated steward UID is absent")?;
    let runtime_gid = fs::symlink_metadata(&config.workspace)
        .context("inspect runtime workspace identity")?
        .gid();
    let bytes = read_stable_steward_file(
        attestation_path,
        MAX_ATTESTATION_BYTES,
        steward_uid,
        runtime_gid,
        "immutable scheduled-authorship attestation",
    )?;
    let envelope: AuthorshipEnvelope =
        serde_json::from_slice(&bytes).context("decode scheduled-authorship attestation")?;
    verify_attestation_signature(config, &envelope)?;
    validate_authorship_core(config, &envelope.core)?;
    let core = &envelope.core;

    let continuity_path = projection_path(config);
    let continuity_bytes = read_stable_steward_file(
        &continuity_path,
        MAX_PROJECTION_BYTES,
        steward_uid,
        runtime_gid,
        "scheduled continuity projection",
    )?;
    ensure!(
        sha256_hex(&continuity_bytes) == core.continuity_projection_sha256,
        "scheduled continuity projection is not the attested bytes"
    );
    let projection: ContinuityProjection = serde_json::from_slice(&continuity_bytes)
        .context("decode attested scheduled continuity projection")?;
    validate_projection(&projection)?;
    validate_attested_projection(core, &projection)?;

    let state_path = projection_state_path(config);
    let state_bytes = read_stable_steward_file(
        &state_path,
        MAX_PROJECTION_BYTES,
        steward_uid,
        runtime_gid,
        "scheduled state projection",
    )?;
    ensure!(
        sha256_hex(&state_bytes) == core.state_projection_sha256,
        "scheduled state projection is not the attested bytes"
    );
    validate_attested_state(core, &state_bytes)?;

    let relative = validate_reflection_path(&core.reflection_path)?;
    let reflection = config.workspace.join(relative);
    let response = read_stable_steward_file(
        &reflection,
        MAX_REFLECTION_BYTES,
        steward_uid,
        runtime_gid,
        "scheduled reflection",
    )?;
    ensure!(
        sha256_hex(&response) == core.reflection_sha256
            && core.reflection_sha256 == core.response_sha256,
        "scheduled reflection does not match immutable authorship attestation"
    );
    ensure!(
        std::str::from_utf8(&response).is_ok(),
        "scheduled reflection is not valid UTF-8"
    );
    let sidecar = reflection.with_extension("json");
    let metadata_bytes = read_stable_steward_file(
        &sidecar,
        MAX_PROJECTION_BYTES,
        steward_uid,
        runtime_gid,
        "scheduled reflection metadata",
    )?;
    ensure!(
        sha256_hex(&metadata_bytes) == core.reflection_metadata_sha256,
        "scheduled reflection metadata is not the attested bytes"
    );
    let metadata: ReflectionMetadata =
        serde_json::from_slice(&metadata_bytes).context("decode attested reflection metadata")?;
    validate_metadata(&projection, &metadata, &reflection)?;
    ensure!(
        metadata.context_provenance_sha256.as_deref()
            == Some(core.context_provenance_sha256.as_str()),
        "reflection context provenance is not the attested context"
    );
    ensure!(
        metadata.context_provenance == projection.context_provenance
            && metadata.reflection_lane == projection.reflection_lane
            && metadata.taint_causes == projection.taint_causes,
        "reflection and continuity context classifications do not exactly join"
    );

    verify_attested_terminal_receipt(config, core, &projection, steward_uid, runtime_gid)?;

    Ok(Some(VerifiedProjection {
        summary: projection.summary,
        summary_sha256: projection.summary_sha256,
        response_sha256: projection.response_sha256,
        trace_id: projection.trace.trace_id,
        due_nonce: projection.due_nonce,
        recorded_at_unix_ms: projection.recorded_at_unix_ms,
    }))
}

fn validate_projection(projection: &ContinuityProjection) -> anyhow::Result<()> {
    ensure!(
        projection.schema == CONTINUITY_SCHEMA,
        "unsupported projection schema"
    );
    ensure!(
        projection.provenance == PROVENANCE,
        "projection is not exact model-authored provenance"
    );
    ensure!(
        projection.authority == AUTHORITY,
        "projection authority is invalid"
    );
    validate_identifier(&projection.appliance_id, 96, "appliance id")?;
    validate_identifier(&projection.model, 160, "model")?;
    validate_due_nonce(&projection.due_nonce)?;
    ensure!(
        projection.recorded_at_unix_ms > 0,
        "projection has no recording time"
    );
    let summary_chars = projection.summary.chars().count();
    ensure!(
        summary_chars > 0 && summary_chars <= MAX_SUMMARY_CHARS,
        "projection summary exceeds bounds"
    );
    ensure!(
        sha256_hex(projection.summary.as_bytes()) == projection.summary_sha256,
        "projection summary hash mismatch"
    );
    validate_sha256(&projection.summary_sha256, "summary hash")?;
    validate_sha256(&projection.response_sha256, "response hash")?;
    validate_sha256(&projection.prompt_sha256, "prompt hash")?;
    ensure!(
        projection.trace.schema_version == 1,
        "unsupported trace schema"
    );
    validate_uuid(&projection.trace.trace_id, "trace id")?;
    validate_uuid(&projection.trace.turn_id, "turn id")?;
    validate_uuid(&projection.trace.span_id, "span id")?;
    validate_identifier(&projection.trace.session_id, 96, "session id")
}

fn validate_metadata(
    projection: &ContinuityProjection,
    metadata: &ReflectionMetadata,
    reflection: &Path,
) -> anyhow::Result<()> {
    ensure!(
        metadata.schema == REFLECTION_SCHEMA,
        "unsupported reflection metadata schema"
    );
    ensure!(
        metadata.provenance == PROVENANCE,
        "reflection metadata provenance mismatch"
    );
    ensure!(
        metadata.appliance_id == projection.appliance_id,
        "reflection appliance mismatch"
    );
    ensure!(
        metadata.due_nonce == projection.due_nonce,
        "reflection due nonce mismatch"
    );
    ensure!(
        metadata.trace_id == projection.trace.trace_id,
        "reflection trace mismatch"
    );
    ensure!(
        metadata.session_id == projection.trace.session_id,
        "reflection session mismatch"
    );
    ensure!(
        metadata.turn_id == projection.trace.turn_id,
        "reflection turn mismatch"
    );
    ensure!(
        metadata.model == projection.model,
        "reflection model mismatch"
    );
    ensure!(
        metadata.prompt_sha256 == projection.prompt_sha256,
        "reflection prompt hash mismatch"
    );
    ensure!(
        metadata.response_sha256 == projection.response_sha256,
        "reflection response hash mismatch"
    );
    ensure!(
        reflection.file_name().and_then(|name| name.to_str())
            == Some(metadata.exact_response_path.as_str()),
        "reflection basename mismatch"
    );
    Ok(())
}

fn verify_attestation_signature(
    config: &Config,
    envelope: &AuthorshipEnvelope,
) -> anyhow::Result<()> {
    ensure!(
        envelope.schema == AUTHORSHIP_ENVELOPE_SCHEMA,
        "unsupported scheduled-authorship envelope schema"
    );
    ensure!(
        envelope.auth.algorithm == "ed25519",
        "scheduled-authorship algorithm is not Ed25519"
    );
    let key_path = config
        .scheduled_authorship_verify_key_path
        .as_deref()
        .context("scheduled-authorship verify key path is absent")?;
    let expected_hash = config
        .scheduled_authorship_verify_key_sha256
        .as_deref()
        .context("scheduled-authorship verify key hash is absent")?;
    let key_bytes = read_stable_regular(key_path, 32)
        .context("read scheduled-authorship public key credential")?;
    ensure!(
        key_bytes.len() == 32,
        "scheduled-authorship key is not 32 bytes"
    );
    ensure!(
        sha256_hex(&key_bytes) == expected_hash,
        "scheduled-authorship public key identity mismatch"
    );
    let key: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("scheduled-authorship key length changed"))?;
    let verifying =
        VerifyingKey::from_bytes(&key).context("scheduled-authorship public key is malformed")?;
    let expected_key_id = format!("ed25519:{}", &expected_hash[..16]);
    ensure!(
        envelope.auth.key_id == expected_key_id,
        "scheduled-authorship key identifier mismatch"
    );
    let signature = Signature::from_bytes(&decode_hex_64(&envelope.auth.signature)?);
    let unsigned = serde_json::json!({
        "schema": AUTHORSHIP_ENVELOPE_SCHEMA,
        "core": envelope.core
    });
    verifying
        .verify_strict(&canonical_json(&unsigned)?, &signature)
        .context("scheduled-authorship signature verification failed")
}

fn validate_authorship_core(config: &Config, core: &AuthorshipCore) -> anyhow::Result<()> {
    ensure!(
        core.schema == AUTHORSHIP_CORE_SCHEMA
            && core.terminal_status == "authored_completed"
            && core.provenance == PROVENANCE
            && core.authority == AUTHORSHIP_AUTHORITY,
        "scheduled-authorship authority fields are invalid"
    );
    ensure!(
        core.appliance_id == config.appliance_id,
        "scheduled-authorship appliance identity mismatch"
    );
    ensure!(
        core.model == config.local_model_id,
        "scheduled-authorship model identity mismatch"
    );
    validate_due_nonce(&core.due_nonce)?;
    let due_at = due_slot_millis(&core.due_nonce)?;
    ensure!(
        core.due_at_unix_ms == due_at
            && core.started_at_unix_ms >= due_at
            && core.completed_at_unix_ms >= core.started_at_unix_ms,
        "scheduled-authorship due/start/completion ordering is invalid"
    );
    validate_uuid(&core.trace.trace_id, "trace id")?;
    validate_uuid(&core.trace.turn_id, "turn id")?;
    validate_uuid(&core.trace.span_id, "span id")?;
    ensure!(
        core.trace.schema_version == 1,
        "unsupported scheduled-authorship trace schema"
    );
    validate_identifier(&core.trace.session_id, 96, "session id")?;
    for (value, label) in [
        (&core.prompt_sha256, "prompt hash"),
        (&core.response_sha256, "response hash"),
        (&core.reflection_sha256, "reflection hash"),
        (&core.reflection_metadata_sha256, "reflection metadata hash"),
        (
            &core.continuity_projection_sha256,
            "continuity projection hash",
        ),
        (&core.state_projection_sha256, "state projection hash"),
        (&core.terminal_receipt_sha256, "terminal receipt hash"),
        (&core.context_provenance_sha256, "context provenance hash"),
    ] {
        validate_sha256(value, label)?;
    }
    validate_reflection_path(&core.reflection_path)?;
    ensure!(
        core.candidate_id.is_some() == core.candidate_digest.is_some(),
        "scheduled-authorship candidate linkage is partial"
    );
    if let Some(candidate_id) = core.candidate_id.as_deref() {
        validate_identifier(candidate_id, 128, "candidate id")?;
    }
    if let Some(candidate_digest) = core.candidate_digest.as_deref() {
        validate_sha256(candidate_digest, "candidate digest")?;
    }
    Ok(())
}

fn validate_attested_projection(
    core: &AuthorshipCore,
    projection: &ContinuityProjection,
) -> anyhow::Result<()> {
    ensure!(
        projection.appliance_id == core.appliance_id
            && projection.model == core.model
            && projection.due_nonce == core.due_nonce
            && projection.recorded_at_unix_ms == core.completed_at_unix_ms
            && projection.prompt_sha256 == core.prompt_sha256
            && projection.response_sha256 == core.response_sha256
            && projection.reflection_path == core.reflection_path
            && projection.trace == core.trace,
        "continuity projection does not exactly join its signed authorship core"
    );
    ensure!(
        projection.context_provenance_sha256.as_deref()
            == Some(core.context_provenance_sha256.as_str()),
        "continuity projection context is not the signed context"
    );
    let context = projection
        .context_provenance
        .as_ref()
        .context("attested continuity projection has no typed context provenance")?;
    ensure!(
        sha256_hex(&canonical_json(context)?) == core.context_provenance_sha256,
        "continuity projection context payload does not match its signed hash"
    );
    ensure!(
        projection.candidate_authoring_eligible.is_some()
            && projection.reflection_lane.as_deref().is_some()
            && projection
                .taint_causes
                .as_ref()
                .is_some_and(|causes| causes.len() <= 16),
        "continuity projection omits bounded context classification"
    );
    Ok(())
}

fn validate_attested_state(core: &AuthorshipCore, bytes: &[u8]) -> anyhow::Result<()> {
    let value: Value = serde_json::from_slice(bytes).context("decode attested scheduled state")?;
    ensure!(
        value.get("schema").and_then(Value::as_str)
            == Some("astrid_edge_scheduled_introspection_state_v1")
            && value.get("last_status").and_then(Value::as_str) == Some("authored_completed")
            && value.get("due_nonce").and_then(Value::as_str) == Some(core.due_nonce.as_str())
            && value
                .get("last_completed_at_unix_ms")
                .and_then(Value::as_u64)
                == Some(core.completed_at_unix_ms)
            && value.get("last_started_at_unix_ms").and_then(Value::as_u64)
                == Some(core.started_at_unix_ms)
            && value.get("last_response_sha256").and_then(Value::as_str)
                == Some(core.response_sha256.as_str())
            && value.get("last_trace") == Some(&serde_json::to_value(&core.trace)?),
        "scheduled state does not exactly join its signed authorship core"
    );
    Ok(())
}

fn verify_attested_terminal_receipt(
    config: &Config,
    core: &AuthorshipCore,
    projection: &ContinuityProjection,
    steward_uid: u32,
    runtime_gid: u32,
) -> anyhow::Result<()> {
    let path = config
        .workspace
        .join("introspections/scheduled/receipts.jsonl");
    let receipt = find_exact_receipt(
        &path,
        &core.terminal_receipt_sha256,
        steward_uid,
        runtime_gid,
    )?
    .context("signed terminal receipt is absent from the steward ledger")?;
    ensure!(
        receipt.get("schema").and_then(Value::as_str)
            == Some("astrid_edge_scheduled_introspection_v1")
            && receipt.get("appliance").and_then(Value::as_str) == Some(core.appliance_id.as_str())
            && receipt.get("due_nonce").and_then(Value::as_str) == Some(core.due_nonce.as_str())
            && receipt.get("due_at_unix_ms").and_then(Value::as_u64) == Some(core.due_at_unix_ms)
            && receipt.get("started_at_unix_ms").and_then(Value::as_u64)
                == Some(core.started_at_unix_ms)
            && receipt.get("completed_at_unix_ms").and_then(Value::as_u64)
                == Some(core.completed_at_unix_ms)
            && receipt.get("status").and_then(Value::as_str) == Some("authored_completed")
            && receipt.get("provenance").and_then(Value::as_str) == Some(PROVENANCE)
            && receipt.get("model_id").and_then(Value::as_str) == Some(core.model.as_str())
            && receipt.get("prompt_sha256").and_then(Value::as_str)
                == Some(core.prompt_sha256.as_str())
            && receipt.get("response_sha256").and_then(Value::as_str)
                == Some(core.response_sha256.as_str())
            && receipt.get("reflection_path").and_then(Value::as_str)
                == Some(core.reflection_path.as_str())
            && receipt
                .get("continuity_projection_written")
                .and_then(Value::as_bool)
                == Some(true)
            && receipt
                .get("introspection_result_sha256")
                .and_then(Value::as_str)
                == Some(projection.summary_sha256.as_str())
            && receipt
                .get("context_provenance_sha256")
                .and_then(Value::as_str)
                == Some(core.context_provenance_sha256.as_str())
            && receipt.get("trace") == Some(&serde_json::to_value(&core.trace)?)
            && receipt.get("candidate_id").and_then(Value::as_str) == core.candidate_id.as_deref()
            && receipt.get("candidate_digest").and_then(Value::as_str)
                == core.candidate_digest.as_deref(),
        "terminal receipt does not exactly join its signed authorship core"
    );
    Ok(())
}

fn find_exact_receipt(
    path: &Path,
    expected_sha256: &str,
    steward_uid: u32,
    runtime_gid: u32,
) -> anyhow::Result<Option<Value>> {
    let before = fs::symlink_metadata(path).context("inspect scheduled receipt ledger")?;
    validate_steward_metadata(
        &before,
        steward_uid,
        runtime_gid,
        MAX_RECEIPT_LEDGER_BYTES,
        "scheduled receipt ledger",
    )?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    options.custom_flags(0o00400000);
    let file = options
        .open(path)
        .context("open scheduled receipt ledger")?;
    let opened = file.metadata()?;
    ensure!(
        opened.dev() == before.dev() && opened.ino() == before.ino(),
        "scheduled receipt ledger changed before open"
    );
    let captured = opened.len();
    let mut reader = BufReader::new(file.take(captured));
    let mut line = Vec::new();
    let mut found = None;
    loop {
        line.clear();
        let count = reader.read_until(b'\n', &mut line)?;
        if count == 0 {
            break;
        }
        ensure!(
            count <= MAX_RECEIPT_LINE_BYTES && line.ends_with(b"\n"),
            "scheduled receipt ledger contains an oversized or incomplete record"
        );
        line.pop();
        if sha256_hex(&line) != expected_sha256 {
            continue;
        }
        ensure!(found.is_none(), "signed terminal receipt is duplicated");
        found = Some(serde_json::from_slice(&line).context("decode signed terminal receipt")?);
    }
    let after = fs::symlink_metadata(path)?;
    ensure!(
        after.file_type().is_file()
            && after.nlink() == 1
            && after.dev() == opened.dev()
            && after.ino() == opened.ino()
            && after.len() >= captured,
        "scheduled receipt ledger was replaced while reading"
    );
    Ok(found)
}

fn validate_reflection_path(value: &str) -> anyhow::Result<PathBuf> {
    let path = Path::new(value);
    let components = path.components().collect::<Vec<_>>();
    let [
        Component::Normal(first),
        Component::Normal(second),
        Component::Normal(filename),
    ] = components.as_slice()
    else {
        bail!("reflection path is not an exact owned relative path");
    };
    ensure!(
        *first == "introspections" && *second == "scheduled",
        "reflection path escaped scheduled artifacts"
    );
    let filename = filename
        .to_str()
        .context("reflection filename is not UTF-8")?;
    ensure!(
        filename.starts_with("reflection_due-")
            && Path::new(filename)
                .extension()
                .is_some_and(|extension| extension == "md")
            && filename.len() <= 240
            && filename
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')),
        "reflection filename is invalid"
    );
    Ok(path.to_path_buf())
}

fn validate_identifier(value: &str, maximum: usize, name: &str) -> anyhow::Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= maximum
            && value.bytes().all(|byte| byte.is_ascii_alphanumeric()
                || matches!(byte, b'.' | b'_' | b'-' | b':' | b'/')),
        "{name} is invalid"
    );
    Ok(())
}

fn validate_due_nonce(value: &str) -> anyhow::Result<()> {
    let digits = value
        .strip_prefix("due-")
        .context("due nonce prefix is invalid")?;
    ensure!(
        (5..=20).contains(&digits.len()) && digits.bytes().all(|byte| byte.is_ascii_digit()),
        "due nonce is invalid"
    );
    Ok(())
}

fn validate_uuid(value: &str, name: &str) -> anyhow::Result<()> {
    let parsed = Uuid::parse_str(value).with_context(|| format!("{name} is invalid"))?;
    ensure!(!parsed.is_nil(), "{name} must not be nil");
    Ok(())
}

fn validate_sha256(value: &str, name: &str) -> anyhow::Result<()> {
    ensure!(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
        "{name} is invalid"
    );
    Ok(())
}

fn due_slot_millis(due_nonce: &str) -> anyhow::Result<u64> {
    let seconds = due_nonce
        .strip_prefix("due-")
        .context("due nonce prefix is invalid")?
        .parse::<u64>()
        .context("due nonce seconds are invalid")?;
    seconds
        .checked_mul(1_000)
        .context("due nonce milliseconds overflow")
}

fn decode_hex_64(value: &str) -> anyhow::Result<[u8; 64]> {
    ensure!(
        value.len() == 128
            && value
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')),
        "scheduled-authorship signature is not canonical"
    );
    let mut decoded = [0_u8; 64];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(pair).context("signature hex is not UTF-8")?;
        decoded[index] = u8::from_str_radix(text, 16).context("signature hex is invalid")?;
    }
    Ok(decoded)
}

fn canonical_json(value: &Value) -> anyhow::Result<Vec<u8>> {
    fn normalize(value: Value) -> anyhow::Result<Value> {
        match value {
            Value::Object(values) => {
                let mut ordered = BTreeMap::new();
                for (key, value) in values {
                    ordered.insert(key, normalize(value)?);
                }
                Ok(serde_json::to_value(ordered)?)
            },
            Value::Array(values) => values
                .into_iter()
                .map(normalize)
                .collect::<anyhow::Result<Vec<_>>>()
                .map(Value::Array),
            Value::Number(number) if number.is_f64() => {
                bail!("floating-point value in signed authorship envelope")
            },
            other => Ok(other),
        }
    }

    Ok(serde_json::to_vec(&normalize(value.clone())?)?)
}

fn validate_steward_metadata(
    metadata: &fs::Metadata,
    steward_uid: u32,
    runtime_gid: u32,
    maximum_bytes: u64,
    label: &str,
) -> anyhow::Result<()> {
    ensure!(
        metadata.file_type().is_file()
            && !metadata.file_type().is_symlink()
            && metadata.nlink() == 1
            && metadata.uid() == steward_uid
            && metadata.gid() == runtime_gid
            && metadata.permissions().mode() & 0o777 == 0o640
            && metadata.len() <= maximum_bytes,
        "{label} is not an exact steward:runtime 0640 bounded regular file"
    );
    Ok(())
}

fn read_stable_steward_file(
    path: &Path,
    maximum_bytes: u64,
    steward_uid: u32,
    runtime_gid: u32,
    label: &str,
) -> anyhow::Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)
        .with_context(|| format!("inspect {label} {}", path.display()))?;
    validate_steward_metadata(&before, steward_uid, runtime_gid, maximum_bytes, label)?;
    let bytes = read_stable_regular(path, maximum_bytes)?;
    let after = fs::symlink_metadata(path)?;
    validate_steward_metadata(&after, steward_uid, runtime_gid, maximum_bytes, label)?;
    ensure!(
        after.dev() == before.dev() && after.ino() == before.ino() && after.len() == before.len(),
        "{label} changed during verified read"
    );
    Ok(bytes)
}

fn read_stable_regular(path: &Path, maximum_bytes: u64) -> anyhow::Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)
        .with_context(|| format!("inspect protected input {}", path.display()))?;
    ensure!(
        before.file_type().is_file() && before.nlink() == 1,
        "protected input is not a single-linked regular file"
    );
    ensure!(
        before.len() <= maximum_bytes,
        "protected input exceeds byte limit"
    );
    ensure!(
        before.permissions().mode() & 0o022 == 0,
        "protected input is group/world writable"
    );
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    options.custom_flags(0o00400000);
    let file = options
        .open(path)
        .with_context(|| format!("open protected input {}", path.display()))?;
    let opened = file.metadata()?;
    ensure!(
        opened.dev() == before.dev() && opened.ino() == before.ino(),
        "protected input changed before open"
    );
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
    file.take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    ensure!(
        u64::try_from(bytes.len()).unwrap_or(u64::MAX) <= maximum_bytes,
        "protected input exceeds byte limit"
    );
    let after = fs::symlink_metadata(path)?;
    ensure!(
        after.file_type().is_file()
            && after.nlink() == 1
            && after.dev() == opened.dev()
            && after.ino() == opened.ino()
            && after.len() == opened.len(),
        "protected input changed while reading"
    );
    Ok(bytes)
}

fn mark_admitted(config: &Config, projection: &VerifiedProjection) -> anyhow::Result<bool> {
    let path = admission_state_path(config);
    let mut state = if path.exists() {
        let bytes = read_stable_regular(&path, MAX_PROJECTION_BYTES)?;
        serde_json::from_slice::<AdmissionState>(&bytes).context("decode admission state")?
    } else {
        AdmissionState::default()
    };
    if !state.schema.is_empty() {
        ensure!(
            state.schema == ADMISSION_SCHEMA,
            "unsupported admission state schema"
        );
    }
    if state.last_response_sha256.as_deref() == Some(&projection.response_sha256) {
        return Ok(false);
    }
    ADMISSION_SCHEMA.clone_into(&mut state.schema);
    state.continuity_admitted = true;
    state.admitted_at_unix_ms = Some(unix_millis());
    state.last_response_sha256 = Some(projection.response_sha256.clone());
    state.last_summary_sha256 = Some(projection.summary_sha256.clone());
    state.last_trace_id = Some(projection.trace_id.clone());
    state.last_due_nonce = Some(projection.due_nonce.clone());
    // Persist before the ephemeral reservoir send: a crash may omit one impulse,
    // but can never replay a reflection and amplify it after every restart.
    state.reservoir_delivery = Some("attempted_after_durable_deduplication".to_owned());
    state.provenance = Some(PROVENANCE.to_owned());
    state.authority = Some("runtime_verified_projection_observational_only".to_owned());
    atomic_private_json(&path, &state)?;
    Ok(true)
}

fn projection_path(config: &Config) -> PathBuf {
    config.runtime_path("scheduled-introspection/projection/continuity.json")
}

fn projection_state_path(config: &Config) -> PathBuf {
    config.runtime_path("scheduled-introspection/projection/state.json")
}

fn admission_state_path(config: &Config) -> PathBuf {
    config.runtime_path("scheduled-introspection/admission/state.json")
}

fn atomic_private_json(path: &Path, value: &impl Serialize) -> anyhow::Result<()> {
    let parent = path.parent().context("admission state has no parent")?;
    fs::create_dir_all(parent)?;
    let metadata = fs::symlink_metadata(parent)?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "admission parent is invalid"
    );
    ensure!(
        metadata.permissions().mode() & 0o022 == 0,
        "admission parent is group/world writable"
    );
    if path.exists() {
        let existing = fs::symlink_metadata(path)?;
        ensure!(
            existing.file_type().is_file() && existing.nlink() == 1,
            "admission state is not a regular single-linked file"
        );
    }
    let temporary = parent.join(format!(".state.{}.tmp", Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    serde_json::to_writer(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
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
    use std::{
        fmt::Write as _,
        fs,
        os::unix::fs::{MetadataExt as _, PermissionsExt as _, symlink},
        path::Path,
    };

    use ed25519_dalek::{Signer as _, SigningKey};
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        ADMISSION_SCHEMA, ContinuityProjection, ReflectionMetadata, mark_admitted, sha256_hex,
        validate_metadata, validate_projection, validate_reflection_path,
    };
    use crate::config::Config;

    fn projection() -> ContinuityProjection {
        let summary = "I noticed a stable distinction between evidence and interpretation.";
        let trace_id = Uuid::from_u128(3);
        serde_json::from_value(json!({
            "schema": "astrid_edge_scheduled_introspection_continuity_v1",
            "appliance_id": "avado-astrid",
            "model": "qwen3.5:4b",
            "due_nonce": "due-12345",
            "recorded_at_unix_ms": 1,
            "summary": summary,
            "summary_sha256": sha256_hex(summary.as_bytes()),
            "response_sha256": sha256_hex(b"exact response"),
            "prompt_sha256": sha256_hex(b"prompt"),
            "reflection_path": format!("introspections/scheduled/reflection_due-12345_{trace_id}.md"),
            "trace": {
                "schema_version": 1,
                "trace_id": trace_id,
                "turn_id": Uuid::from_u128(1),
                "span_id": Uuid::from_u128(2),
                "session_id": "session-0123456789abcdef"
            },
            "provenance": "model_authored_runtime_scheduled",
            "authority": "bounded_continuity_projection_not_voluntary_journal"
        })).expect("projection")
    }

    #[test]
    fn exact_projection_validates_and_fallback_provenance_does_not() {
        let mut value = projection();
        validate_projection(&value).expect("exact projection");
        value.provenance = "local_safe_fallback".to_owned();
        assert!(validate_projection(&value).is_err());
    }

    #[test]
    fn projection_hash_and_path_tampering_fail_closed() {
        let mut value = projection();
        value.summary.push_str(" tampered");
        assert!(validate_projection(&value).is_err());
        assert!(validate_reflection_path("../../operator-home/key").is_err());
        assert!(validate_reflection_path("introspections/scheduled/link.md/extra").is_err());
    }

    #[test]
    fn nil_causal_identifiers_never_enter_continuity_or_reservoir_admission() {
        for field in ["trace_id", "turn_id", "span_id"] {
            let mut malformed = projection();
            match field {
                "trace_id" => malformed.trace.trace_id = Uuid::nil().to_string(),
                "turn_id" => malformed.trace.turn_id = Uuid::nil().to_string(),
                "span_id" => malformed.trace.span_id = Uuid::nil().to_string(),
                _ => unreachable!("bounded test fixture"),
            }
            assert!(
                validate_projection(&malformed).is_err(),
                "accepted nil {field}"
            );
        }
    }

    #[test]
    fn metadata_requires_the_complete_causal_join() {
        let value = projection();
        let reflection = Path::new(&value.reflection_path);
        let mut metadata: ReflectionMetadata = serde_json::from_value(json!({
            "schema": "astrid.edge.scheduled_introspection.model_reflection.v1",
            "provenance": "model_authored_runtime_scheduled",
            "appliance_id": value.appliance_id,
            "due_nonce": value.due_nonce,
            "trace_id": value.trace.trace_id,
            "session_id": value.trace.session_id,
            "turn_id": value.trace.turn_id,
            "model": value.model,
            "prompt_sha256": value.prompt_sha256,
            "response_sha256": value.response_sha256,
            "exact_response_path": reflection.file_name().unwrap().to_str().unwrap()
        }))
        .expect("metadata");
        validate_metadata(&value, &metadata, reflection).expect("exact join");
        metadata.turn_id = Uuid::new_v4().to_string();
        assert!(validate_metadata(&value, &metadata, reflection).is_err());
    }

    #[test]
    fn admission_deduplicates_before_reservoir_delivery() {
        let root =
            std::env::temp_dir().join(format!("astrid-scheduled-admission-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("runtime/scheduled-introspection/admission")).expect("dirs");
        fs::set_permissions(
            root.join("runtime/scheduled-introspection/admission"),
            fs::Permissions::from_mode(0o700),
        )
        .expect("mode");
        let config = test_config(&root);
        let value = projection();
        let verified = super::VerifiedProjection {
            summary: value.summary,
            summary_sha256: value.summary_sha256,
            response_sha256: value.response_sha256,
            trace_id: value.trace.trace_id,
            due_nonce: value.due_nonce,
            recorded_at_unix_ms: value.recorded_at_unix_ms,
        };
        assert!(mark_admitted(&config, &verified).expect("first"));
        assert!(!mark_admitted(&config, &verified).expect("duplicate"));
        let state: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join("runtime/scheduled-introspection/admission/state.json"))
                .expect("state"),
        )
        .expect("json");
        assert_eq!(state["schema"], ADMISSION_SCHEMA);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn symlinked_projection_is_not_read_as_authored() {
        let root =
            std::env::temp_dir().join(format!("astrid-scheduled-symlink-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("runtime/scheduled-introspection/projection")).expect("dirs");
        let target = root.join("target.json");
        fs::write(&target, b"{}").expect("target");
        symlink(
            &target,
            root.join("runtime/scheduled-introspection/projection/continuity.json"),
        )
        .expect("symlink");
        assert!(super::verify_current(&test_config(&root)).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn immutable_attestation_survives_presentation_replacement_and_rejects_parent_forgery() {
        let root = std::env::temp_dir().join(format!("astrid-scheduled-signed-{}", Uuid::new_v4()));
        let (config, projection_bytes) = signed_fixture(&root);
        let exact = super::verify_immutable_steward_current(&config)
            .expect("verify immutable attestation")
            .expect("authored projection");
        assert_eq!(exact.due_nonce, "due-10000");

        // The owner-visible workspace attestation is presentation only. A
        // mutable runtime may replace it, but it cannot affect the read-only
        // immutable-root attestation consumed by admission.
        let presentation = root
            .join("workspace/runtime/scheduled-introspection/projection/authorship.current.json");
        fs::write(&presentation, b"{\"forged\":true}").expect("replace presentation copy");
        set_mode(&presentation, 0o640);
        assert!(
            super::verify_immutable_steward_current(&config)
                .expect("presentation is non-authoritative")
                .is_some()
        );

        // Replacing the runtime-owned ancestor and recreating plausible files
        // cannot manufacture a hash/signature join.
        let projection_root = root.join("workspace/runtime/scheduled-introspection/projection");
        let retained = root.join("retained-projection");
        fs::rename(&projection_root, &retained).expect("rename presentation parent");
        fs::create_dir_all(&projection_root).expect("replacement parent");
        fs::write(projection_root.join("continuity.json"), &projection_bytes)
            .expect("forged continuity copy");
        set_mode(&projection_root.join("continuity.json"), 0o640);
        fs::write(projection_root.join("state.json"), b"{}\n").expect("forged state");
        set_mode(&projection_root.join("state.json"), 0o640);
        assert!(super::verify_immutable_steward_current(&config).is_err());

        fs::remove_dir_all(&projection_root).expect("remove forged parent");
        fs::rename(&retained, &projection_root).expect("restore projection parent");
        let ledger = root.join("workspace/introspections/scheduled/receipts.jsonl");
        fs::write(&ledger, b"{\"status\":\"authored_completed\"}\n")
            .expect("forge terminal receipt");
        set_mode(&ledger, 0o640);
        assert!(super::verify_immutable_steward_current(&config).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[allow(clippy::too_many_lines)]
    fn signed_fixture(root: &Path) -> (Config, Vec<u8>) {
        let workspace = root.join("workspace");
        let projection_root = workspace.join("runtime/scheduled-introspection/projection");
        let reflection_root = workspace.join("introspections/scheduled");
        let immutable_root = root.join("immutable/scheduled-authorship");
        for directory in [&projection_root, &reflection_root, &immutable_root] {
            fs::create_dir_all(directory).expect("fixture directory");
        }
        let uid = fs::metadata(root).expect("root metadata").uid();
        let gid = fs::metadata(&workspace).expect("workspace metadata").gid();
        let signing = SigningKey::from_bytes(&[7_u8; 32]);
        let public_key = signing.verifying_key().to_bytes();
        let public_key_path = root.join("scheduled-authorship.pub");
        fs::write(&public_key_path, public_key).expect("public key");
        let context = json!({
            "schema": "astrid.edge.scheduled_introspection.context_provenance.v1",
            "owned_artifact_content": false,
            "web_content": false,
            "legacy_or_unverified": false,
            "candidate_authoring_eligible": true
        });
        let context_sha256 = sha256_hex(&super::canonical_json(&context).expect("context"));
        let trace = json!({
            "schema_version": 1,
            "trace_id": Uuid::from_u128(1).to_string(),
            "turn_id": Uuid::from_u128(2).to_string(),
            "span_id": Uuid::from_u128(3).to_string(),
            "session_id": "scheduled-session-10000"
        });
        let response = b"I distinguished verified evidence from interpretation.";
        let response_sha256 = sha256_hex(response);
        let prompt_sha256 = sha256_hex(b"prompt");
        let summary = "Verified evidence remains distinct from interpretation.";
        let summary_sha256 = sha256_hex(summary.as_bytes());
        let relative_reflection = format!(
            "introspections/scheduled/reflection_due-10000_{}.md",
            Uuid::from_u128(2)
        );
        let reflection = workspace.join(&relative_reflection);
        fs::write(&reflection, response).expect("reflection");
        set_mode(&reflection, 0o640);
        let metadata = json!({
            "schema": "astrid.edge.scheduled_introspection.model_reflection.v1",
            "provenance": "model_authored_runtime_scheduled",
            "appliance_id": "avado-astrid",
            "due_nonce": "due-10000",
            "trace_id": trace["trace_id"],
            "session_id": trace["session_id"],
            "turn_id": trace["turn_id"],
            "model": "qwen3.5:4b",
            "prompt_sha256": prompt_sha256,
            "response_sha256": response_sha256,
            "exact_response_path": reflection.file_name().unwrap().to_str().unwrap(),
            "context_provenance": context,
            "context_provenance_sha256": context_sha256,
            "reflection_lane": "clean_owned_context",
            "taint_causes": []
        });
        let metadata_bytes = super::canonical_json(&metadata).expect("metadata");
        fs::write(reflection.with_extension("json"), &metadata_bytes).expect("metadata file");
        set_mode(&reflection.with_extension("json"), 0o640);
        let continuity = json!({
            "schema": "astrid_edge_scheduled_introspection_continuity_v1",
            "appliance_id": "avado-astrid",
            "model": "qwen3.5:4b",
            "due_nonce": "due-10000",
            "recorded_at_unix_ms": 10_002_000_u64,
            "summary": summary,
            "summary_sha256": summary_sha256,
            "response_sha256": response_sha256,
            "prompt_sha256": prompt_sha256,
            "reflection_path": relative_reflection,
            "trace": trace,
            "provenance": "model_authored_runtime_scheduled",
            "authority": "bounded_continuity_projection_not_voluntary_journal",
            "context_provenance": context,
            "context_provenance_sha256": context_sha256,
            "candidate_authoring_eligible": true,
            "reflection_lane": "clean_owned_context",
            "taint_causes": []
        });
        let continuity_bytes = super::canonical_json(&continuity).expect("continuity");
        fs::write(projection_root.join("continuity.json"), &continuity_bytes)
            .expect("continuity file");
        set_mode(&projection_root.join("continuity.json"), 0o640);
        let state = json!({
            "schema": "astrid_edge_scheduled_introspection_state_v1",
            "last_status": "authored_completed",
            "due_nonce": "due-10000",
            "last_started_at_unix_ms": 10_001_000_u64,
            "last_completed_at_unix_ms": 10_002_000_u64,
            "last_response_sha256": response_sha256,
            "last_trace": trace
        });
        let state_bytes = super::canonical_json(&state).expect("state");
        fs::write(projection_root.join("state.json"), &state_bytes).expect("state file");
        set_mode(&projection_root.join("state.json"), 0o640);
        let receipt = json!({
            "schema": "astrid_edge_scheduled_introspection_v1",
            "appliance": "avado-astrid",
            "due_nonce": "due-10000",
            "due_at_unix_ms": 10_000_000_u64,
            "started_at_unix_ms": 10_001_000_u64,
            "completed_at_unix_ms": 10_002_000_u64,
            "status": "authored_completed",
            "provenance": "model_authored_runtime_scheduled",
            "model_id": "qwen3.5:4b",
            "prompt_sha256": prompt_sha256,
            "response_sha256": response_sha256,
            "reflection_path": relative_reflection,
            "continuity_projection_written": true,
            "introspection_result_sha256": summary_sha256,
            "context_provenance_sha256": context_sha256,
            "candidate_id": null,
            "candidate_digest": null,
            "trace": trace
        });
        let receipt_bytes = super::canonical_json(&receipt).expect("receipt");
        let mut receipt_line = receipt_bytes.clone();
        receipt_line.push(b'\n');
        fs::write(reflection_root.join("receipts.jsonl"), receipt_line).expect("ledger");
        set_mode(&reflection_root.join("receipts.jsonl"), 0o640);
        let core = json!({
            "schema": "astrid.edge.scheduled_authorship.attestation.v1",
            "appliance_id": "avado-astrid",
            "due_nonce": "due-10000",
            "due_at_unix_ms": 10_000_000_u64,
            "started_at_unix_ms": 10_001_000_u64,
            "completed_at_unix_ms": 10_002_000_u64,
            "terminal_status": "authored_completed",
            "model": "qwen3.5:4b",
            "prompt_sha256": prompt_sha256,
            "response_sha256": response_sha256,
            "reflection_path": relative_reflection,
            "reflection_sha256": response_sha256,
            "reflection_metadata_sha256": sha256_hex(&metadata_bytes),
            "continuity_projection_sha256": sha256_hex(&continuity_bytes),
            "state_projection_sha256": sha256_hex(&state_bytes),
            "terminal_receipt_sha256": sha256_hex(&receipt_bytes),
            "context_provenance_sha256": context_sha256,
            "candidate_id": null,
            "candidate_digest": null,
            "trace": trace,
            "provenance": "model_authored_runtime_scheduled",
            "authority": "immutable_steward_signed_exact_authorship_join"
        });
        let unsigned = json!({
            "schema": "astrid.edge.scheduled_authorship.attestation_envelope.v1",
            "core": core
        });
        let signature = signing.sign(&super::canonical_json(&unsigned).expect("unsigned"));
        let mut signature_hex = String::with_capacity(128);
        for byte in signature.to_bytes() {
            write!(&mut signature_hex, "{byte:02x}").expect("write signature hex");
        }
        let envelope = json!({
            "schema": "astrid.edge.scheduled_authorship.attestation_envelope.v1",
            "core": unsigned["core"],
            "auth": {
                "algorithm": "ed25519",
                "key_id": format!("ed25519:{}", &sha256_hex(&public_key)[..16]),
                "signature": signature_hex
            }
        });
        let attestation = immutable_root.join("current.json");
        fs::write(
            &attestation,
            super::canonical_json(&envelope).expect("envelope"),
        )
        .expect("attestation");
        set_mode(&attestation, 0o640);
        fs::write(
            projection_root.join("authorship.current.json"),
            super::canonical_json(&envelope).expect("presentation envelope"),
        )
        .expect("presentation");
        set_mode(&projection_root.join("authorship.current.json"), 0o640);

        let mut config = test_config(&workspace);
        config.appliance_id = "avado-astrid".to_owned();
        config.local_model_id = "qwen3.5:4b".to_owned();
        config.dedicated_steward_enabled = true;
        config.scheduled_authorship_attestation_path = Some(attestation);
        config.scheduled_authorship_verify_key_path = Some(public_key_path);
        config.scheduled_authorship_verify_key_sha256 = Some(sha256_hex(&public_key));
        config.scheduled_authorship_steward_uid = Some(uid);
        assert_eq!(gid, fs::metadata(&workspace).unwrap().gid());
        (config, continuity_bytes)
    }

    fn set_mode(path: &Path, mode: u32) {
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("fixture mode");
    }

    fn test_config(workspace: &Path) -> Config {
        use clap::Parser as _;
        Config::parse_from([
            "astrid-edge-runtime",
            "--workspace",
            workspace.to_str().expect("workspace"),
            "--maintenance-lease-path",
            "/tmp/astrid-test-maintenance.lock",
        ])
    }
}
