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
use tokio::{
    sync::{mpsc, oneshot},
    time::MissedTickBehavior,
};
use uuid::Uuid;

use crate::{
    autonomy,
    codec::encode_text,
    config::Config,
    inquiry::{InquiryBeliefOperation, InquiryThreadOperation, VerifiedInquiryStepInput},
    reservoir::SensoryIngress,
    semantic_envelope::{
        CODEC_VERSION, ENVELOPE_SCHEMA, SemanticAdmissionAckV1, SemanticEnvelopeV1,
        SemanticSourceClassV1, SemanticTraceV1, derive_admission_id,
    },
    trace::IpcTraceContextV1,
};

const CONTINUITY_SCHEMA: &str = "astrid_edge_scheduled_introspection_continuity_v1";
const CONTINUITY_SCHEMA_V2: &str = "astrid_edge_scheduled_introspection_continuity_v2";
const REFLECTION_SCHEMA: &str = "astrid.edge.scheduled_introspection.model_reflection.v1";
const REFLECTION_SCHEMA_V2: &str = "astrid.edge.scheduled_introspection.model_reflection.v2";
const LEGACY_ADMISSION_SCHEMA: &str = "astrid.edge.scheduled_introspection.admission.v1";
const ADMISSION_SCHEMA: &str = "astrid.edge.inquiry.semantic_admission.v2";
const ADMISSION_RECEIPT_SCHEMA: &str = "astrid.edge.inquiry.semantic_admission_receipt.v1";
const SCHEDULED_PROVENANCE: &str = "model_authored_runtime_scheduled";
const EVIDENCE_INTEGRATION_PROVENANCE: &str = "model_authored_runtime_evidence_integration";
const AUTHORITY: &str = "bounded_continuity_projection_not_voluntary_journal";
const AUTHORITY_V2: &str =
    "bounded_signed_inquiry_continuity_projection_not_code_or_action_authority";
const AUTHORSHIP_CORE_SCHEMA: &str = "astrid.edge.scheduled_authorship.attestation.v1";
const AUTHORSHIP_ENVELOPE_SCHEMA: &str = "astrid.edge.scheduled_authorship.attestation_envelope.v1";
const AUTHORSHIP_CORE_SCHEMA_V2: &str = "astrid.edge.scheduled_authorship.attestation.v2";
const AUTHORSHIP_ENVELOPE_SCHEMA_V2: &str =
    "astrid.edge.scheduled_authorship.attestation_envelope.v2";
const AUTHORSHIP_AUTHORITY: &str = "immutable_steward_signed_exact_authorship_join";
const INQUIRY_PROJECTION_SCHEMA: &str = "astrid.edge.inquiry.current.v1";
const INQUIRY_PROJECTION_AUTHORITY: &str =
    "immutable_steward_signed_bounded_inquiry_projection_observational_only";
const POLL_SECONDS: u64 = 10;
const MAX_PROJECTION_BYTES: u64 = 16 * 1_024;
const MAX_REFLECTION_BYTES: u64 = 64 * 1_024;
const MAX_SUMMARY_CHARS: usize = 320;
const MAX_ATTESTATION_BYTES: u64 = 32 * 1_024;
const MAX_RECEIPT_LEDGER_BYTES: u64 = 256 * 1_024 * 1_024;
const MAX_RECEIPT_LINE_BYTES: usize = 32 * 1_024;
const MAX_ADMISSION_LEDGER_BYTES: u64 = 256 * 1_024 * 1_024;
const RESERVOIR_ACK_TIMEOUT_SECONDS: u64 = 5;

fn provenance_for_trigger(trigger_kind: &str) -> anyhow::Result<&'static str> {
    match trigger_kind {
        "scheduled" => Ok(SCHEDULED_PROVENANCE),
        "evidence_integration" => Ok(EVIDENCE_INTEGRATION_PROVENANCE),
        other => bail!("unsupported inquiry trigger kind {other}"),
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinuityProjection {
    schema: String,
    appliance_id: String,
    model: String,
    #[serde(default)]
    trigger_kind: Option<String>,
    #[serde(default)]
    trigger_nonce: Option<String>,
    due_nonce: String,
    recorded_at_unix_ms: u64,
    summary: String,
    summary_sha256: String,
    response_sha256: String,
    prompt_sha256: String,
    reflection_path: String,
    #[serde(default)]
    signed_entry_id: Option<String>,
    #[serde(default)]
    step_id: Option<String>,
    #[serde(default)]
    admission_id: Option<String>,
    #[serde(default)]
    inquiry_current_projection_sha256: Option<String>,
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
pub(crate) struct ProjectionTrace {
    pub(crate) schema_version: u8,
    pub(crate) trace_id: String,
    pub(crate) turn_id: String,
    pub(crate) span_id: String,
    pub(crate) session_id: String,
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
    #[serde(default)]
    authorship_status: Option<String>,
    #[serde(default)]
    inquiry_failure_class: Option<String>,
    #[serde(default)]
    structured_inquiry: Option<bool>,
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
    #[serde(default)]
    continuity_projection_sha256: Option<String>,
    state_projection_sha256: String,
    terminal_receipt_sha256: String,
    context_provenance_sha256: String,
    candidate_id: Option<String>,
    candidate_digest: Option<String>,
    #[serde(default)]
    inquiry_current_projection_sha256: Option<String>,
    #[serde(default)]
    signed_entry_id: Option<String>,
    #[serde(default)]
    admission_id: Option<String>,
    #[serde(default)]
    step_id: Option<String>,
    #[serde(default)]
    inquiry_step_sha256: Option<String>,
    #[serde(default)]
    inquiry_declaration_sha256: Option<String>,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InquiryCurrentProjection {
    schema: String,
    appliance_id: String,
    signed_entry_id: String,
    step_id: String,
    admission_id: String,
    summary: String,
    summary_sha256: String,
    inquiry_step: Value,
    inquiry_step_sha256: String,
    declaration_sha256: String,
    response_sha256: String,
    trace: ProjectionTrace,
    trigger_kind: String,
    due_nonce: String,
    trigger_nonce: String,
    recorded_at_unix_ms: u64,
    reflection_path: String,
    reflection_sha256: String,
    ledger: InquiryLedgerReference,
    provenance: String,
    authority: String,
    core_sha256: String,
    auth: AuthorshipAuthentication,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InquiryProjectionCore {
    schema: String,
    appliance_id: String,
    signed_entry_id: String,
    step_id: String,
    admission_id: String,
    summary: String,
    summary_sha256: String,
    inquiry_step: Value,
    inquiry_step_sha256: String,
    declaration_sha256: String,
    response_sha256: String,
    trace: ProjectionTrace,
    trigger_kind: String,
    due_nonce: String,
    trigger_nonce: String,
    recorded_at_unix_ms: u64,
    reflection_path: String,
    reflection_sha256: String,
    ledger: InquiryLedgerReference,
    provenance: String,
    authority: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InquiryLedgerReference {
    segment: u64,
    entry_index: u64,
    prior_entry_sha256: Option<String>,
    entry_sha256: String,
    key_id: String,
    signature_algorithm: String,
    signature: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeInquiryStep {
    schema: String,
    thread_operation: String,
    thread_id: String,
    parent_step_id: Option<String>,
    observation: String,
    interpretation: String,
    uncertainty: String,
    decision: String,
    counterpoint: Option<String>,
    next_test: Option<String>,
    #[serde(default)]
    evidence_ids: Vec<String>,
    confidence: String,
    belief_operation: Option<String>,
    belief_id: Option<String>,
    belief_claim: Option<String>,
}

impl InquiryCurrentProjection {
    fn core(&self) -> InquiryProjectionCore {
        InquiryProjectionCore {
            schema: self.schema.clone(),
            appliance_id: self.appliance_id.clone(),
            signed_entry_id: self.signed_entry_id.clone(),
            step_id: self.step_id.clone(),
            admission_id: self.admission_id.clone(),
            summary: self.summary.clone(),
            summary_sha256: self.summary_sha256.clone(),
            inquiry_step: self.inquiry_step.clone(),
            inquiry_step_sha256: self.inquiry_step_sha256.clone(),
            declaration_sha256: self.declaration_sha256.clone(),
            response_sha256: self.response_sha256.clone(),
            trace: self.trace.clone(),
            trigger_kind: self.trigger_kind.clone(),
            due_nonce: self.due_nonce.clone(),
            trigger_nonce: self.trigger_nonce.clone(),
            recorded_at_unix_ms: self.recorded_at_unix_ms,
            reflection_path: self.reflection_path.clone(),
            reflection_sha256: self.reflection_sha256.clone(),
            ledger: self.ledger.clone(),
            provenance: self.provenance.clone(),
            authority: self.authority.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdmissionState {
    schema: String,
    continuity_admitted: bool,
    admitted_at_unix_ms: Option<u64>,
    signed_entry_id: Option<String>,
    admission_id: Option<String>,
    last_response_sha256: Option<String>,
    last_summary_sha256: Option<String>,
    last_trace_id: Option<String>,
    last_due_nonce: Option<String>,
    reservoir_delivery: Option<String>,
    queued_at_unix_ms: Option<u64>,
    terminal_at_unix_ms: Option<u64>,
    reservoir_generation: Option<String>,
    reservoir_sequence: Option<u64>,
    vector_sha256: Option<String>,
    source_class: Option<String>,
    migrated_legacy_schema: Option<String>,
    provenance: Option<String>,
    authority: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdmissionReceiptRecord {
    schema: String,
    event: String,
    recorded_at_unix_ms: u64,
    state: AdmissionState,
}

#[derive(Debug, Clone)]
pub(crate) struct VerifiedInquiryProjection {
    pub(crate) signed_entry_id: String,
    pub(crate) admission_id: String,
    pub(crate) step_id: String,
    pub(crate) trigger_kind: String,
    pub(crate) trigger_nonce: String,
    pub(crate) declaration_sha256: String,
    pub(crate) inquiry_step: Value,
    pub(crate) reflection_sha256: String,
    pub(crate) ledger_segment: u64,
    pub(crate) ledger_entry_index: u64,
    pub(crate) ledger_prior_entry_sha256: Option<String>,
    pub(crate) ledger_entry_sha256: String,
}

#[derive(Debug, Clone)]
pub(crate) struct VerifiedProjection {
    pub(crate) summary: String,
    pub(crate) summary_sha256: String,
    pub(crate) response_sha256: String,
    pub(crate) trace_id: String,
    pub(crate) due_nonce: String,
    pub(crate) recorded_at_unix_ms: u64,
    pub(crate) trace: ProjectionTrace,
    pub(crate) inquiry: Option<VerifiedInquiryProjection>,
}

#[expect(
    clippy::too_many_lines,
    reason = "the polling transaction deliberately keeps verify, durable thread admission, queue, and exact ACK handling visibly ordered"
)]
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
        let Some(inquiry) = projection.inquiry.as_ref() else {
            // Legacy and model-authored-unstructured reflections remain exact
            // private prose, but are not continuity or reservoir experience.
            continue;
        };
        let source_class = match inquiry.trigger_kind.as_str() {
            "scheduled" => SemanticSourceClassV1::ScheduledInquiry,
            "evidence_integration" => SemanticSourceClassV1::EvidenceIntegration,
            other => {
                eprintln!("scheduled inquiry projection has invalid trigger kind: {other}");
                continue;
            },
        };
        let verified_step = match verified_thread_step(&projection, inquiry) {
            Ok(step) => step,
            Err(error) => {
                eprintln!("scheduled inquiry thread projection rejected: {error:#}");
                continue;
            },
        };
        let inquiry_outcome = match autonomy::ingest_verified_inquiry_step(&config, &verified_step)
        {
            Ok(outcome) => outcome,
            Err(error) => {
                eprintln!("scheduled inquiry durable thread admission rejected: {error:#}");
                continue;
            },
        };
        if !inquiry_outcome.reservoir_eligible() {
            eprintln!(
                "scheduled inquiry advanced verified-ledger continuity but was excluded from authored continuity and reservoir admission: {inquiry_outcome:?}"
            );
            continue;
        }
        let vector = encode_text(source_class.as_str(), &projection.summary);
        let envelope = SemanticEnvelopeV1 {
            schema: ENVELOPE_SCHEMA.to_owned(),
            source_class,
            signed_entry_id: inquiry.signed_entry_id.clone(),
            admission_id: inquiry.admission_id.clone(),
            summary_sha256: projection.summary_sha256.clone(),
            codec_version: CODEC_VERSION.to_owned(),
            trace: SemanticTraceV1 {
                schema_version: projection.trace.schema_version,
                trace_id: projection.trace.trace_id.clone(),
                turn_id: projection.trace.turn_id.clone(),
                span_id: projection.trace.span_id.clone(),
                session_id: projection.trace.session_id.clone(),
                chain_id: None,
            },
            vector,
        };
        if let Err(error) = envelope.validate(&config.appliance_id) {
            eprintln!("scheduled inquiry semantic envelope rejected: {error:#}");
            continue;
        }
        let expected_vector_sha256 = envelope.vector_sha256();
        match queue_admission(&config, &projection, &envelope) {
            Ok(true) => {},
            Ok(false) => continue,
            Err(error) => {
                eprintln!("scheduled inquiry admission queue rejected: {error:#}");
                continue;
            },
        }
        let (reply, receiver) = oneshot::channel();
        if ingress_tx
            .send(SensoryIngress::ScheduledSemantic {
                envelope: Box::new(envelope),
                reply,
            })
            .await
            .is_err()
        {
            let _ = finish_admission(&config, inquiry, "failed", None);
            eprintln!("scheduled inquiry reservoir admission failed: reservoir closed");
            return;
        }
        let outcome =
            tokio::time::timeout(Duration::from_secs(RESERVOIR_ACK_TIMEOUT_SECONDS), receiver)
                .await;
        match outcome {
            Ok(Ok(Ok(ack))) => {
                if let Err(error) = validate_ack(inquiry, &ack, &expected_vector_sha256)
                    .and_then(|()| finish_admission(&config, inquiry, "acknowledged", Some(&ack)))
                {
                    let _ = finish_admission(&config, inquiry, "failed", None);
                    eprintln!("scheduled inquiry reservoir acknowledgment rejected: {error:#}");
                }
            },
            Ok(Ok(Err(error))) => {
                let _ = finish_admission(&config, inquiry, "failed", None);
                eprintln!("scheduled inquiry reservoir rejected admission: {error:#}");
            },
            Ok(Err(_)) | Err(_) => {
                let _ = finish_admission(&config, inquiry, "delivery_unknown", None);
                eprintln!("scheduled inquiry reservoir admission acknowledgment is unknown");
            },
        }
    }
}

/// Return only a fully verified, bounded projection for prompt continuity.
pub(crate) fn latest_verified_summary(config: &Config) -> Option<String> {
    verify_current(config)
        .ok()
        .flatten()
        .filter(|projection| projection.inquiry.is_some())
        .map(|projection| projection.summary)
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
        trace_id: projection.trace.trace_id.clone(),
        due_nonce: projection.due_nonce,
        recorded_at_unix_ms: projection.recorded_at_unix_ms,
        trace: projection.trace,
        inquiry: None,
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
    verify_attestation_signature(config, &envelope, &bytes)?;
    validate_authorship_core(config, &envelope.core)?;
    let core = &envelope.core;
    if core.terminal_status == "model_authored_unstructured" {
        verify_unstructured_authorship(config, core, steward_uid, runtime_gid)?;
        return Ok(None);
    }

    let continuity_path = projection_path(config);
    let continuity_bytes = read_stable_steward_file(
        &continuity_path,
        MAX_PROJECTION_BYTES,
        steward_uid,
        runtime_gid,
        "scheduled continuity projection",
    )?;
    ensure!(
        core.continuity_projection_sha256.as_deref()
            == Some(sha256_hex(&continuity_bytes).as_str()),
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

    let inquiry = verify_inquiry_current(config, core, &projection, steward_uid, runtime_gid)?;
    verify_attested_terminal_receipt(
        config,
        core,
        Some(&projection),
        inquiry.as_ref(),
        steward_uid,
        runtime_gid,
    )?;

    Ok(Some(VerifiedProjection {
        summary: projection.summary,
        summary_sha256: projection.summary_sha256,
        response_sha256: projection.response_sha256,
        trace_id: projection.trace.trace_id.clone(),
        due_nonce: projection.due_nonce,
        recorded_at_unix_ms: projection.recorded_at_unix_ms,
        trace: projection.trace,
        inquiry,
    }))
}

fn validate_projection(projection: &ContinuityProjection) -> anyhow::Result<()> {
    ensure!(
        matches!(
            projection.schema.as_str(),
            CONTINUITY_SCHEMA | CONTINUITY_SCHEMA_V2
        ),
        "unsupported projection schema"
    );
    let v2 = projection.schema == CONTINUITY_SCHEMA_V2;
    let expected_provenance = if v2 {
        provenance_for_trigger(projection.trigger_kind.as_deref().unwrap_or_default())?
    } else {
        SCHEDULED_PROVENANCE
    };
    ensure!(
        projection.provenance == expected_provenance,
        "projection is not exact model-authored provenance"
    );
    ensure!(
        (!v2 && projection.authority == AUTHORITY) || (v2 && projection.authority == AUTHORITY_V2),
        "projection authority is invalid"
    );
    let inquiry_bindings = [
        projection.trigger_kind.as_ref(),
        projection.trigger_nonce.as_ref(),
        projection.signed_entry_id.as_ref(),
        projection.step_id.as_ref(),
        projection.admission_id.as_ref(),
        projection.inquiry_current_projection_sha256.as_ref(),
    ];
    let present = inquiry_bindings
        .iter()
        .filter(|value| value.is_some())
        .count();
    ensure!(
        (!v2 && present == 0) || (v2 && present == inquiry_bindings.len()),
        "continuity projection has a partial inquiry binding"
    );
    if v2 {
        ensure!(
            matches!(
                projection.trigger_kind.as_deref(),
                Some("scheduled" | "evidence_integration")
            ),
            "continuity projection trigger kind is invalid"
        );
        validate_identifier(
            projection.trigger_nonce.as_deref().unwrap_or_default(),
            96,
            "continuity trigger nonce",
        )?;
        validate_prefixed_sha256(
            projection.signed_entry_id.as_deref().unwrap_or_default(),
            "inquiry-entry-",
            "continuity signed entry id",
        )?;
        validate_prefixed_sha256(
            projection.step_id.as_deref().unwrap_or_default(),
            "inquiry-step-",
            "continuity step id",
        )?;
        validate_prefixed_sha256(
            projection.admission_id.as_deref().unwrap_or_default(),
            "inquiry-admission-",
            "continuity admission id",
        )?;
        validate_sha256(
            projection
                .inquiry_current_projection_sha256
                .as_deref()
                .unwrap_or_default(),
            "continuity inquiry-current projection hash",
        )?;
    }
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
        matches!(
            metadata.schema.as_str(),
            REFLECTION_SCHEMA | REFLECTION_SCHEMA_V2
        ),
        "unsupported reflection metadata schema"
    );
    ensure!(
        metadata.provenance == projection.provenance,
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
    if metadata.schema == REFLECTION_SCHEMA_V2 {
        ensure!(
            metadata.authorship_status.as_deref() == Some("model_authored_structured")
                && metadata.structured_inquiry == Some(true)
                && metadata.inquiry_failure_class.is_none(),
            "structured reflection metadata has invalid inquiry classification"
        );
    }
    Ok(())
}

fn verify_unstructured_authorship(
    config: &Config,
    core: &AuthorshipCore,
    steward_uid: u32,
    runtime_gid: u32,
) -> anyhow::Result<()> {
    ensure!(
        core.continuity_projection_sha256.is_none()
            && core.inquiry_current_projection_sha256.is_none()
            && core.signed_entry_id.is_none()
            && core.admission_id.is_none(),
        "unstructured authorship cannot bind continuity or inquiry"
    );
    let state_bytes = read_stable_steward_file(
        &projection_state_path(config),
        MAX_PROJECTION_BYTES,
        steward_uid,
        runtime_gid,
        "unstructured scheduled state projection",
    )?;
    ensure!(
        sha256_hex(&state_bytes) == core.state_projection_sha256,
        "unstructured scheduled state is not the attested bytes"
    );
    validate_attested_state(core, &state_bytes)?;

    let relative = validate_reflection_path(&core.reflection_path)?;
    let reflection = config.workspace.join(relative);
    let response = read_stable_steward_file(
        &reflection,
        MAX_REFLECTION_BYTES,
        steward_uid,
        runtime_gid,
        "unstructured scheduled reflection",
    )?;
    ensure!(
        sha256_hex(&response) == core.reflection_sha256
            && core.reflection_sha256 == core.response_sha256
            && std::str::from_utf8(&response).is_ok(),
        "unstructured reflection is not the exact attested UTF-8 response"
    );
    let metadata_bytes = read_stable_steward_file(
        &reflection.with_extension("json"),
        MAX_PROJECTION_BYTES,
        steward_uid,
        runtime_gid,
        "unstructured reflection metadata",
    )?;
    ensure!(
        sha256_hex(&metadata_bytes) == core.reflection_metadata_sha256,
        "unstructured reflection metadata is not the attested bytes"
    );
    let metadata: ReflectionMetadata = serde_json::from_slice(&metadata_bytes)?;
    ensure!(
        metadata.schema == REFLECTION_SCHEMA_V2
            && metadata.provenance == core.provenance
            && metadata.appliance_id == core.appliance_id
            && metadata.due_nonce == core.due_nonce
            && metadata.trace_id == core.trace.trace_id
            && metadata.session_id == core.trace.session_id
            && metadata.turn_id == core.trace.turn_id
            && metadata.model == core.model
            && metadata.prompt_sha256 == core.prompt_sha256
            && metadata.response_sha256 == core.response_sha256
            && metadata.context_provenance_sha256.as_deref()
                == Some(core.context_provenance_sha256.as_str())
            && metadata.authorship_status.as_deref() == Some("model_authored_unstructured")
            && metadata.structured_inquiry == Some(false)
            && metadata.inquiry_failure_class.is_some()
            && reflection.file_name().and_then(|name| name.to_str())
                == Some(metadata.exact_response_path.as_str()),
        "unstructured reflection metadata does not join authorship"
    );
    verify_attested_terminal_receipt(config, core, None, None, steward_uid, runtime_gid)
}

fn verify_attestation_signature(
    config: &Config,
    envelope: &AuthorshipEnvelope,
    exact_envelope_bytes: &[u8],
) -> anyhow::Result<()> {
    ensure!(
        matches!(
            envelope.schema.as_str(),
            AUTHORSHIP_ENVELOPE_SCHEMA | AUTHORSHIP_ENVELOPE_SCHEMA_V2
        ),
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
    let exact_value: Value = serde_json::from_slice(exact_envelope_bytes)
        .context("decode exact scheduled-authorship envelope for signature")?;
    let exact_core = exact_value
        .get("core")
        .cloned()
        .context("scheduled-authorship envelope has no exact core")?;
    let unsigned = serde_json::json!({"schema": envelope.schema, "core": exact_core});
    verifying
        .verify_strict(&canonical_json(&unsigned)?, &signature)
        .context("scheduled-authorship signature verification failed")
}

#[allow(clippy::too_many_arguments)]
fn verify_inquiry_current(
    config: &Config,
    authorship: &AuthorshipCore,
    continuity: &ContinuityProjection,
    steward_uid: u32,
    runtime_gid: u32,
) -> anyhow::Result<Option<VerifiedInquiryProjection>> {
    let Some(expected_projection_sha256) = authorship.inquiry_current_projection_sha256.as_deref()
    else {
        ensure!(
            authorship.signed_entry_id.is_none() && authorship.admission_id.is_none(),
            "unstructured authorship retained a partial inquiry binding"
        );
        return Ok(None);
    };
    let path = inquiry_projection_path(config);
    let bytes = read_stable_steward_file(
        &path,
        MAX_PROJECTION_BYTES,
        steward_uid,
        runtime_gid,
        "signed inquiry-current projection",
    )?;
    ensure!(
        sha256_hex(&bytes) == expected_projection_sha256,
        "inquiry-current projection is not the attested bytes"
    );
    let projection: InquiryCurrentProjection =
        serde_json::from_slice(&bytes).context("decode signed inquiry-current projection")?;
    validate_inquiry_projection(config, authorship, continuity, &projection)?;
    verify_inquiry_projection_signature(config, &projection)?;
    Ok(Some(VerifiedInquiryProjection {
        signed_entry_id: projection.signed_entry_id,
        admission_id: projection.admission_id,
        step_id: projection.step_id,
        trigger_kind: projection.trigger_kind,
        trigger_nonce: projection.trigger_nonce,
        declaration_sha256: projection.declaration_sha256,
        inquiry_step: projection.inquiry_step,
        reflection_sha256: projection.reflection_sha256,
        ledger_segment: projection.ledger.segment,
        ledger_entry_index: projection.ledger.entry_index,
        ledger_prior_entry_sha256: projection
            .ledger
            .prior_entry_sha256
            .filter(|digest| digest != &"0".repeat(64)),
        ledger_entry_sha256: projection.ledger.entry_sha256,
    }))
}

fn verified_thread_step(
    projection: &VerifiedProjection,
    inquiry: &VerifiedInquiryProjection,
) -> anyhow::Result<VerifiedInquiryStepInput> {
    let step: RuntimeInquiryStep = serde_json::from_value(inquiry.inquiry_step.clone())
        .context("decode strictly bounded inquiry step for working continuity")?;
    ensure!(
        step.schema == "astrid.edge.inquiry.step.v1",
        "unsupported inquiry step schema"
    );
    let thread_operation = match step.thread_operation.as_str() {
        "continue" => InquiryThreadOperation::Continue,
        "open" => InquiryThreadOperation::Open,
        "branch" => InquiryThreadOperation::Branch,
        "pause" => InquiryThreadOperation::Pause,
        "close" => InquiryThreadOperation::Close,
        other => bail!("unsupported inquiry thread operation {other}"),
    };
    let belief_operation = match step.belief_operation.as_deref() {
        None => None,
        Some("unchanged") => Some(InquiryBeliefOperation::Unchanged),
        Some("propose") => Some(InquiryBeliefOperation::Propose),
        Some("support") => Some(InquiryBeliefOperation::Support),
        Some("weaken") => Some(InquiryBeliefOperation::Weaken),
        Some("revise") => Some(InquiryBeliefOperation::Revise),
        Some("suspend") => Some(InquiryBeliefOperation::Suspend),
        Some("resolve") => Some(InquiryBeliefOperation::Resolve),
        Some(other) => bail!("unsupported inquiry belief operation {other}"),
    };
    let trace = IpcTraceContextV1 {
        schema_version: projection.trace.schema_version,
        trace_id: Uuid::parse_str(&projection.trace.trace_id).context("parse inquiry trace id")?,
        turn_id: Some(Uuid::parse_str(&projection.trace.turn_id).context("parse inquiry turn id")?),
        span_id: Uuid::parse_str(&projection.trace.span_id).context("parse inquiry span id")?,
        parent_span_id: None,
        session_id: Some(projection.trace.session_id.clone()),
        chain_id: None,
    };
    ensure!(trace.is_supported(), "inquiry trace is not supported");
    Ok(VerifiedInquiryStepInput {
        step_id: inquiry.step_id.clone(),
        entry_hash: inquiry.ledger_entry_sha256.clone(),
        mechanical_predecessor_hash: inquiry.ledger_prior_entry_sha256.clone(),
        ledger_segment: inquiry.ledger_segment,
        ledger_entry_index: inquiry.ledger_entry_index,
        parent_step_id: step.parent_step_id,
        thread_operation,
        thread_id: Some(step.thread_id),
        observation: step.observation,
        interpretation: step.interpretation,
        uncertainty: step.uncertainty,
        decision: step.decision,
        counterpoint: step.counterpoint,
        next_test: step.next_test,
        evidence_ids: step.evidence_ids,
        confidence: step.confidence,
        belief_operation,
        belief_id: step.belief_id,
        belief_claim: step.belief_claim,
        trigger: inquiry.trigger_kind.clone(),
        recorded_at_unix_ms: projection.recorded_at_unix_ms,
        trace,
        response_sha256: projection.response_sha256.clone(),
        reflection_sha256: inquiry.reflection_sha256.clone(),
        declaration_sha256: inquiry.declaration_sha256.clone(),
    })
}

fn validate_inquiry_projection(
    config: &Config,
    authorship: &AuthorshipCore,
    continuity: &ContinuityProjection,
    projection: &InquiryCurrentProjection,
) -> anyhow::Result<()> {
    ensure!(
        projection.schema == INQUIRY_PROJECTION_SCHEMA
            && projection.appliance_id == config.appliance_id
            && projection.provenance == provenance_for_trigger(&projection.trigger_kind)?
            && projection.provenance == continuity.provenance
            && projection.authority == INQUIRY_PROJECTION_AUTHORITY,
        "inquiry-current projection authority fields are invalid"
    );
    validate_prefixed_sha256(
        &projection.signed_entry_id,
        "inquiry-entry-",
        "signed inquiry entry id",
    )?;
    validate_prefixed_sha256(
        &projection.admission_id,
        "inquiry-admission-",
        "inquiry admission id",
    )?;
    validate_prefixed_sha256(&projection.step_id, "inquiry-step-", "inquiry step id")?;
    validate_identifier(&projection.trigger_nonce, 96, "inquiry trigger nonce")?;
    ensure!(
        matches!(
            projection.trigger_kind.as_str(),
            "scheduled" | "evidence_integration"
        ),
        "inquiry-current trigger kind is invalid"
    );
    ensure!(
        projection.recorded_at_unix_ms > 0
            && projection.recorded_at_unix_ms == continuity.recorded_at_unix_ms,
        "inquiry-current recording time does not join continuity"
    );
    ensure!(
        projection.due_nonce == continuity.due_nonce
            && continuity.trigger_kind.as_deref() == Some(projection.trigger_kind.as_str())
            && continuity.trigger_nonce.as_deref() == Some(projection.trigger_nonce.as_str())
            && continuity.signed_entry_id.as_deref() == Some(projection.signed_entry_id.as_str())
            && continuity.step_id.as_deref() == Some(projection.step_id.as_str())
            && continuity.admission_id.as_deref() == Some(projection.admission_id.as_str())
            && continuity.inquiry_current_projection_sha256.as_deref()
                == authorship.inquiry_current_projection_sha256.as_deref()
            && projection.response_sha256 == continuity.response_sha256
            && projection.summary == continuity.summary
            && projection.summary_sha256 == continuity.summary_sha256
            && projection.trace == continuity.trace
            && projection.reflection_path == continuity.reflection_path
            && projection.reflection_sha256 == continuity.response_sha256,
        "inquiry-current projection does not exactly join signed reflection continuity"
    );
    ensure!(
        authorship.signed_entry_id.as_deref() == Some(projection.signed_entry_id.as_str())
            && authorship.admission_id.as_deref() == Some(projection.admission_id.as_str())
            && authorship.step_id.as_deref() == Some(projection.step_id.as_str())
            && authorship.inquiry_step_sha256.as_deref()
                == Some(projection.inquiry_step_sha256.as_str())
            && authorship.inquiry_declaration_sha256.as_deref()
                == Some(projection.declaration_sha256.as_str()),
        "inquiry-current identifiers do not match immutable authorship"
    );
    ensure!(
        projection.admission_id
            == derive_admission_id(&projection.appliance_id, &projection.signed_entry_id),
        "inquiry-current admission identity derivation is invalid"
    );
    ensure!(
        sha256_hex(projection.summary.as_bytes()) == projection.summary_sha256,
        "inquiry-current summary hash mismatch"
    );
    validate_sha256(&projection.inquiry_step_sha256, "inquiry step hash")?;
    validate_sha256(&projection.declaration_sha256, "inquiry declaration hash")?;
    validate_sha256(&projection.response_sha256, "inquiry response hash")?;
    validate_sha256(&projection.reflection_sha256, "inquiry reflection hash")?;
    let step_bytes = canonical_json(&projection.inquiry_step)?;
    ensure!(
        step_bytes.len() <= 8 * 1_024
            && projection.inquiry_step.is_object()
            && sha256_hex(&step_bytes) == projection.inquiry_step_sha256,
        "inquiry step is not the exact bounded canonical JSON"
    );
    validate_sha256(&projection.core_sha256, "inquiry projection core hash")?;
    ensure!(
        projection.ledger.segment > 0,
        "inquiry ledger segment is zero"
    );
    validate_sha256(&projection.ledger.entry_sha256, "inquiry ledger entry hash")?;
    if let Some(prior) = projection.ledger.prior_entry_sha256.as_deref() {
        validate_sha256(prior, "prior inquiry ledger hash")?;
    }
    ensure!(
        projection.ledger.signature_algorithm == "ed25519"
            && projection.ledger.key_id == projection.auth.key_id,
        "inquiry ledger authentication metadata is invalid"
    );
    let _ = decode_hex_64(&projection.ledger.signature)?;
    Ok(())
}

fn verify_inquiry_projection_signature(
    config: &Config,
    projection: &InquiryCurrentProjection,
) -> anyhow::Result<()> {
    ensure!(
        projection.auth.algorithm == "ed25519",
        "inquiry projection signature algorithm is invalid"
    );
    let key_path = config
        .scheduled_authorship_verify_key_path
        .as_deref()
        .context("inquiry projection verify key path is absent")?;
    let expected_hash = config
        .scheduled_authorship_verify_key_sha256
        .as_deref()
        .context("inquiry projection verify key hash is absent")?;
    let key_bytes = read_stable_regular(key_path, 32)?;
    ensure!(
        key_bytes.len() == 32 && sha256_hex(&key_bytes) == expected_hash,
        "inquiry projection verifier identity mismatch"
    );
    ensure!(
        projection.auth.key_id == format!("ed25519:{}", &expected_hash[..16]),
        "inquiry projection key identifier mismatch"
    );
    let core_bytes = canonical_json(&serde_json::to_value(projection.core())?)?;
    ensure!(
        sha256_hex(&core_bytes) == projection.core_sha256,
        "inquiry projection core hash mismatch"
    );
    let key: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("inquiry projection key length changed"))?;
    VerifyingKey::from_bytes(&key)?
        .verify_strict(
            &core_bytes,
            &Signature::from_bytes(&decode_hex_64(&projection.auth.signature)?),
        )
        .context("inquiry projection signature verification failed")
}

#[expect(
    clippy::too_many_lines,
    reason = "all fields in the signed authorship core are validated together as one cryptographic trust boundary"
)]
fn validate_authorship_core(config: &Config, core: &AuthorshipCore) -> anyhow::Result<()> {
    let legacy = core.schema == AUTHORSHIP_CORE_SCHEMA;
    let structured = core.schema == AUTHORSHIP_CORE_SCHEMA_V2
        && core.terminal_status == "model_authored_structured";
    let unstructured = core.schema == AUTHORSHIP_CORE_SCHEMA_V2
        && core.terminal_status == "model_authored_unstructured";
    let provenance_is_valid = if legacy {
        core.provenance == SCHEDULED_PROVENANCE
    } else {
        matches!(
            core.provenance.as_str(),
            SCHEDULED_PROVENANCE | EVIDENCE_INTEGRATION_PROVENANCE
        )
    };
    ensure!(
        ((legacy && core.terminal_status == "authored_completed") || structured || unstructured)
            && provenance_is_valid
            && core.authority == AUTHORSHIP_AUTHORITY,
        "scheduled-authorship authority fields are invalid"
    );
    let inquiry_fields = [
        core.inquiry_current_projection_sha256.as_ref(),
        core.signed_entry_id.as_ref(),
        core.admission_id.as_ref(),
        core.step_id.as_ref(),
        core.inquiry_step_sha256.as_ref(),
        core.inquiry_declaration_sha256.as_ref(),
    ];
    let present = inquiry_fields
        .iter()
        .filter(|value| value.is_some())
        .count();
    ensure!(
        present == 0 || present == inquiry_fields.len(),
        "scheduled-authorship inquiry binding is partial"
    );
    if structured {
        ensure!(
            present == inquiry_fields.len(),
            "structured authorship has no complete inquiry binding"
        );
        let continuity_hash = core
            .continuity_projection_sha256
            .as_deref()
            .context("structured authorship has no continuity projection")?;
        validate_sha256(continuity_hash, "continuity projection hash")?;
        if let Some(hash) = core.inquiry_current_projection_sha256.as_deref() {
            validate_sha256(hash, "inquiry-current projection hash")?;
            validate_prefixed_sha256(
                core.signed_entry_id.as_deref().unwrap_or_default(),
                "inquiry-entry-",
                "signed inquiry entry id",
            )?;
            validate_prefixed_sha256(
                core.admission_id.as_deref().unwrap_or_default(),
                "inquiry-admission-",
                "inquiry admission id",
            )?;
            validate_prefixed_sha256(
                core.step_id.as_deref().unwrap_or_default(),
                "inquiry-step-",
                "inquiry step id",
            )?;
            validate_sha256(
                core.inquiry_step_sha256.as_deref().unwrap_or_default(),
                "inquiry step hash",
            )?;
            validate_sha256(
                core.inquiry_declaration_sha256
                    .as_deref()
                    .unwrap_or_default(),
                "inquiry declaration hash",
            )?;
        }
    } else if unstructured {
        ensure!(
            present == 0 && core.continuity_projection_sha256.is_none(),
            "unstructured authorship cannot bind continuity or inquiry"
        );
        ensure!(
            core.candidate_id.is_none() && core.candidate_digest.is_none(),
            "unstructured reflection cannot request source review or a candidate"
        );
    } else {
        ensure!(
            present == 0 && core.continuity_projection_sha256.is_some(),
            "v1 authorship binding is invalid"
        );
    }
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
        (&core.state_projection_sha256, "state projection hash"),
        (&core.terminal_receipt_sha256, "terminal receipt hash"),
        (&core.context_provenance_sha256, "context provenance hash"),
    ] {
        validate_sha256(value, label)?;
    }
    if let Some(hash) = core.continuity_projection_sha256.as_deref() {
        validate_sha256(hash, "continuity projection hash")?;
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
    let state_schema = value.get("schema").and_then(Value::as_str);
    ensure!(
        matches!(
            state_schema,
            Some(
                "astrid_edge_scheduled_introspection_state_v1"
                    | "astrid_edge_scheduled_introspection_state_v2"
            )
        ) && value.get("last_status").and_then(Value::as_str)
            == Some(core.terminal_status.as_str())
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

#[expect(
    clippy::too_many_lines,
    reason = "the terminal receipt is an exact field-by-field causal join whose structured and unstructured exclusions are reviewed together"
)]
fn verify_attested_terminal_receipt(
    config: &Config,
    core: &AuthorshipCore,
    projection: Option<&ContinuityProjection>,
    inquiry: Option<&VerifiedInquiryProjection>,
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
    let schema = receipt.get("schema").and_then(Value::as_str);
    let legacy = core.schema == AUTHORSHIP_CORE_SCHEMA;
    ensure!(
        ((legacy && schema == Some("astrid_edge_scheduled_introspection_v1"))
            || (!legacy && schema == Some("astrid_edge_scheduled_introspection_v2")))
            && receipt.get("appliance").and_then(Value::as_str) == Some(core.appliance_id.as_str())
            && receipt.get("due_nonce").and_then(Value::as_str) == Some(core.due_nonce.as_str())
            && receipt.get("due_at_unix_ms").and_then(Value::as_u64) == Some(core.due_at_unix_ms)
            && receipt.get("started_at_unix_ms").and_then(Value::as_u64)
                == Some(core.started_at_unix_ms)
            && receipt.get("completed_at_unix_ms").and_then(Value::as_u64)
                == Some(core.completed_at_unix_ms)
            && receipt.get("status").and_then(Value::as_str) == Some(core.terminal_status.as_str())
            && receipt.get("provenance").and_then(Value::as_str) == Some(core.provenance.as_str())
            && receipt.get("model_id").and_then(Value::as_str) == Some(core.model.as_str())
            && receipt.get("prompt_sha256").and_then(Value::as_str)
                == Some(core.prompt_sha256.as_str())
            && receipt.get("response_sha256").and_then(Value::as_str)
                == Some(core.response_sha256.as_str())
            && receipt.get("reflection_path").and_then(Value::as_str)
                == Some(core.reflection_path.as_str())
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
    if !legacy {
        let trigger_kind = receipt
            .get("trigger_kind")
            .and_then(Value::as_str)
            .context("v2 terminal receipt has no trigger kind")?;
        let trigger_nonce = receipt
            .get("trigger_nonce")
            .and_then(Value::as_str)
            .context("v2 terminal receipt has no trigger nonce")?;
        validate_identifier(trigger_nonce, 96, "terminal receipt trigger nonce")?;
        ensure!(
            provenance_for_trigger(trigger_kind)? == core.provenance,
            "terminal receipt trigger contradicts signed provenance"
        );
        if trigger_kind == "scheduled" {
            ensure!(
                trigger_nonce == core.due_nonce,
                "scheduled terminal receipt trigger nonce differs from its due slot"
            );
        }
    }
    if let Some(projection) = projection {
        ensure!(
            receipt
                .get("continuity_projection_written")
                .and_then(Value::as_bool)
                == Some(true)
                && receipt
                    .get("introspection_result_sha256")
                    .and_then(Value::as_str)
                    == Some(projection.summary_sha256.as_str()),
            "structured terminal receipt does not bind continuity"
        );
        if !legacy {
            let inquiry =
                inquiry.context("structured receipt has no verified inquiry projection")?;
            ensure!(
                receipt.get("trigger_kind").and_then(Value::as_str)
                    == Some(inquiry.trigger_kind.as_str())
                    && receipt.get("trigger_nonce").and_then(Value::as_str)
                        == Some(inquiry.trigger_nonce.as_str())
                    && receipt.get("inquiry_status").and_then(Value::as_str)
                        == Some("model_authored_structured")
                    && receipt
                        .get("inquiry_failure_class")
                        .is_none_or(Value::is_null)
                    && receipt
                        .get("continuity_admission_status")
                        .and_then(Value::as_str)
                        == Some("pending_runtime_verification")
                    && receipt
                        .get("reservoir_admission_status")
                        .and_then(Value::as_str)
                        == Some("pending_runtime_ack"),
                "structured receipt inquiry lifecycle is invalid"
            );
            for (field, expected) in [
                ("signed_entry_id", core.signed_entry_id.as_deref()),
                ("step_id", core.step_id.as_deref()),
                ("admission_id", core.admission_id.as_deref()),
                ("inquiry_step_sha256", core.inquiry_step_sha256.as_deref()),
                (
                    "inquiry_declaration_sha256",
                    core.inquiry_declaration_sha256.as_deref(),
                ),
                (
                    "inquiry_current_projection_sha256",
                    core.inquiry_current_projection_sha256.as_deref(),
                ),
                (
                    "continuity_projection_sha256",
                    core.continuity_projection_sha256.as_deref(),
                ),
            ] {
                ensure!(
                    receipt.get(field).and_then(Value::as_str) == expected,
                    "structured terminal receipt {field} does not join authorship"
                );
            }
            ensure!(
                receipt
                    .get("reservoir_admission_eligible")
                    .and_then(Value::as_bool)
                    == Some(true)
                    && receipt.get("continuity_admitted").and_then(Value::as_bool) == Some(false)
                    && matches!(
                        receipt
                            .get("source_review_relation")
                            .and_then(Value::as_str),
                        None | Some("separate_clean_source_review")
                    ),
                "structured receipt incorrectly withholds reservoir eligibility"
            );
        }
    } else {
        ensure!(
            !legacy
                && receipt
                    .get("continuity_projection_written")
                    .and_then(Value::as_bool)
                    == Some(false)
                && receipt
                    .get("reservoir_admission_eligible")
                    .and_then(Value::as_bool)
                    == Some(false)
                && receipt.get("continuity_admitted").and_then(Value::as_bool) == Some(false)
                && receipt.get("inquiry_status").and_then(Value::as_str)
                    == Some("model_authored_unstructured")
                && receipt
                    .get("continuity_admission_status")
                    .and_then(Value::as_str)
                    == Some("not_admitted_model_authored_unstructured")
                && receipt
                    .get("reservoir_admission_status")
                    .and_then(Value::as_str)
                    == Some("not_eligible_model_authored_unstructured")
                && receipt
                    .get("source_review_relation")
                    .is_none_or(Value::is_null)
                && [
                    "signed_entry_id",
                    "step_id",
                    "admission_id",
                    "inquiry_step_sha256",
                    "inquiry_declaration_sha256",
                    "inquiry_current_projection_sha256",
                    "continuity_projection_sha256",
                ]
                .iter()
                .all(|field| receipt.get(*field).is_none_or(Value::is_null)),
            "unstructured terminal receipt falsely claims continuity or admission"
        );
    }
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
    options.custom_flags(0o00_400_000);
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

fn validate_prefixed_sha256(value: &str, prefix: &str, name: &str) -> anyhow::Result<()> {
    let digest = value
        .strip_prefix(prefix)
        .with_context(|| format!("{name} has an invalid domain prefix"))?;
    validate_sha256(digest, name)
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
    options.custom_flags(0o00_400_000);
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

fn load_admission_state(path: &Path) -> anyhow::Result<AdmissionState> {
    if !path.exists() {
        return Ok(AdmissionState::default());
    }
    let bytes = read_stable_regular(path, MAX_PROJECTION_BYTES)?;
    let value: Value = serde_json::from_slice(&bytes).context("decode admission state")?;
    match value.get("schema").and_then(Value::as_str) {
        Some(ADMISSION_SCHEMA) => {
            serde_json::from_value(value).context("decode v2 admission state")
        },
        Some(LEGACY_ADMISSION_SCHEMA) => Ok(AdmissionState {
            schema: ADMISSION_SCHEMA.to_owned(),
            continuity_admitted: false,
            last_response_sha256: value
                .get("last_response_sha256")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            last_summary_sha256: value
                .get("last_summary_sha256")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            last_trace_id: value
                .get("last_trace_id")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            last_due_nonce: value
                .get("last_due_nonce")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            reservoir_delivery: Some("legacy_unacknowledged_not_replayed".to_owned()),
            migrated_legacy_schema: Some(LEGACY_ADMISSION_SCHEMA.to_owned()),
            provenance: Some("legacy_pre_inquiry_train".to_owned()),
            authority: Some("migration_record_no_delivery_or_authorship_claim".to_owned()),
            ..AdmissionState::default()
        }),
        Some(other) => bail!("unsupported admission state schema {other}"),
        None => bail!("admission state has no schema"),
    }
}

fn admission_event_status(event: &str) -> Option<&'static str> {
    match event {
        "queued" => Some("queued"),
        "acknowledged" => Some("acknowledged"),
        "delivery_unknown" | "interrupted_before_ack" | "superseded_queued_delivery_unknown" => {
            Some("delivery_unknown")
        },
        "failed" => Some("failed"),
        _ => None,
    }
}

fn validate_admission_receipt(record: &AdmissionReceiptRecord) -> anyhow::Result<()> {
    let status = record
        .state
        .reservoir_delivery
        .as_deref()
        .context("admission receipt state has no delivery status")?;
    ensure!(
        record.schema == ADMISSION_RECEIPT_SCHEMA
            && record.recorded_at_unix_ms > 0
            && admission_event_status(&record.event) == Some(status)
            && record.state.schema == ADMISSION_SCHEMA
            && record.state.continuity_admitted
            && record
                .state
                .admitted_at_unix_ms
                .is_some_and(|value| value > 0)
            && record
                .state
                .queued_at_unix_ms
                .is_some_and(|value| value > 0)
            && record
                .state
                .signed_entry_id
                .as_deref()
                .is_some_and(|value| !value.is_empty())
            && record
                .state
                .admission_id
                .as_deref()
                .is_some_and(|value| !value.is_empty())
            && record
                .state
                .last_response_sha256
                .as_deref()
                .is_some_and(|value| validate_sha256(value, "admission response hash").is_ok())
            && record
                .state
                .last_summary_sha256
                .as_deref()
                .is_some_and(|value| validate_sha256(value, "admission summary hash").is_ok())
            && record
                .state
                .last_trace_id
                .as_deref()
                .is_some_and(|value| validate_uuid(value, "admission trace").is_ok())
            && record
                .state
                .last_due_nonce
                .as_deref()
                .is_some_and(|value| validate_due_nonce(value).is_ok())
            && record
                .state
                .vector_sha256
                .as_deref()
                .is_some_and(|value| validate_sha256(value, "admission vector hash").is_ok())
            && matches!(
                record.state.source_class.as_deref(),
                Some("scheduled_inquiry" | "evidence_integration")
            )
            && matches!(
                record.state.provenance.as_deref(),
                Some(SCHEDULED_PROVENANCE | EVIDENCE_INTEGRATION_PROVENANCE)
            )
            && record.state.authority.as_deref()
                == Some("verified_signed_inquiry_observational_only"),
        "admission receipt is malformed or internally inconsistent"
    );
    match status {
        "queued" => ensure!(
            record.state.terminal_at_unix_ms.is_none()
                && record.state.reservoir_generation.is_none()
                && record.state.reservoir_sequence.is_none(),
            "queued admission receipt carries terminal acknowledgment state"
        ),
        "acknowledged" => ensure!(
            record
                .state
                .terminal_at_unix_ms
                .is_some_and(|value| value > 0)
                && record
                    .state
                    .reservoir_generation
                    .as_deref()
                    .is_some_and(|value| !value.is_empty())
                && record.state.reservoir_sequence.is_some(),
            "acknowledged admission receipt lacks exact ACK state"
        ),
        "delivery_unknown" | "failed" => ensure!(
            record
                .state
                .terminal_at_unix_ms
                .is_some_and(|value| value > 0)
                && record.state.reservoir_generation.is_none()
                && record.state.reservoir_sequence.is_none(),
            "non-acknowledged terminal receipt carries ACK state"
        ),
        _ => bail!("unsupported admission receipt status"),
    }
    Ok(())
}

fn read_admission_receipts(config: &Config) -> anyhow::Result<Vec<AdmissionReceiptRecord>> {
    let path = admission_receipts_path(config);
    let before = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).context("inspect semantic admission ledger"),
    };
    ensure!(
        before.file_type().is_file()
            && before.nlink() == 1
            && before.permissions().mode() & 0o777 == 0o600
            && before.len() <= MAX_ADMISSION_LEDGER_BYTES,
        "semantic admission ledger is not a bounded owner-private single-linked file"
    );
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    options.custom_flags(0o00_400_000);
    let file = options.open(&path)?;
    let opened = file.metadata()?;
    ensure!(
        opened.dev() == before.dev() && opened.ino() == before.ino(),
        "semantic admission ledger changed before open"
    );
    let captured = opened.len();
    let mut reader = BufReader::new(file.take(captured));
    let mut line = Vec::new();
    let mut records = Vec::new();
    let mut lifecycle = BTreeMap::<String, String>::new();
    loop {
        line.clear();
        let count = reader.read_until(b'\n', &mut line)?;
        if count == 0 {
            break;
        }
        ensure!(
            count <= MAX_RECEIPT_LINE_BYTES && line.ends_with(b"\n"),
            "semantic admission ledger contains an oversized or incomplete record"
        );
        line.pop();
        let record: AdmissionReceiptRecord =
            serde_json::from_slice(&line).context("decode semantic admission receipt")?;
        validate_admission_receipt(&record)?;
        let admission_id = record
            .state
            .admission_id
            .as_ref()
            .context("semantic admission receipt has no admission identity")?;
        let status = record
            .state
            .reservoir_delivery
            .as_ref()
            .context("semantic admission receipt has no delivery status")?;
        match lifecycle.get(admission_id).map(String::as_str) {
            None => ensure!(
                status == "queued" && record.event == "queued",
                "semantic admission terminal receipt lacks its queued predecessor"
            ),
            Some("queued") => ensure!(
                status != "queued",
                "semantic admission queued receipt is duplicated"
            ),
            Some(_) => bail!("semantic admission terminal receipt is duplicated"),
        }
        lifecycle.insert(admission_id.clone(), status.clone());
        records.push(record);
    }
    let after = fs::symlink_metadata(&path)?;
    ensure!(
        after.file_type().is_file()
            && after.nlink() == 1
            && after.dev() == opened.dev()
            && after.ino() == opened.ino()
            && after.len() >= captured,
        "semantic admission ledger was replaced while reading"
    );
    Ok(records)
}

fn queued_admission_state(state: &AdmissionState) -> AdmissionState {
    let mut queued = state.clone();
    queued.reservoir_delivery = Some("queued".to_owned());
    queued.terminal_at_unix_ms = None;
    queued.reservoir_generation = None;
    queued.reservoir_sequence = None;
    queued
}

fn reconcile_admission_receipts(config: &Config, state: &AdmissionState) -> anyhow::Result<()> {
    let Some(admission_id) = state.admission_id.as_deref() else {
        return Ok(());
    };
    let status = state
        .reservoir_delivery
        .as_deref()
        .context("persisted admission identity has no delivery status")?;
    ensure!(
        matches!(
            status,
            "queued" | "acknowledged" | "delivery_unknown" | "failed"
        ),
        "persisted admission has unsupported delivery status"
    );
    let queued = queued_admission_state(state);
    let records = read_admission_receipts(config)?;
    let mut queued_matches = 0_u8;
    let mut terminal_matches = 0_u8;
    for record in records
        .iter()
        .filter(|record| record.state.admission_id.as_deref() == Some(admission_id))
    {
        match record.state.reservoir_delivery.as_deref() {
            Some("queued") => {
                ensure!(
                    record.event == "queued" && record.state == queued,
                    "persisted admission queued predecessor conflicts with its receipt"
                );
                queued_matches = queued_matches.saturating_add(1);
            },
            Some(_) => {
                ensure!(
                    status != "queued" && record.state == *state,
                    "persisted admission terminal state conflicts with its receipt"
                );
                terminal_matches = terminal_matches.saturating_add(1);
            },
            None => bail!("semantic admission receipt has no delivery status"),
        }
    }
    ensure!(
        queued_matches <= 1 && terminal_matches <= 1,
        "semantic admission receipt lifecycle is duplicated"
    );
    if queued_matches == 0 {
        append_admission_event(config, "queued", &queued)?;
    }
    if status != "queued" && terminal_matches == 0 {
        append_admission_event(config, status, state)?;
    }
    Ok(())
}

fn queue_admission(
    config: &Config,
    projection: &VerifiedProjection,
    envelope: &SemanticEnvelopeV1,
) -> anyhow::Result<bool> {
    let inquiry = projection
        .inquiry
        .as_ref()
        .context("cannot queue an unstructured or legacy reflection")?;
    let path = admission_state_path(config);
    let mut state = load_admission_state(&path)?;
    reconcile_admission_receipts(config, &state)?;
    if state.admission_id.as_deref() == Some(inquiry.admission_id.as_str()) {
        if state.reservoir_delivery.as_deref() == Some("queued") {
            state.reservoir_delivery = Some("delivery_unknown".to_owned());
            state.terminal_at_unix_ms = Some(unix_millis());
            atomic_private_json(&path, &state)?;
            append_admission_event(config, "interrupted_before_ack", &state)?;
        }
        return Ok(false);
    }
    if state.reservoir_delivery.as_deref() == Some("queued") {
        state.reservoir_delivery = Some("delivery_unknown".to_owned());
        state.terminal_at_unix_ms = Some(unix_millis());
        atomic_private_json(&path, &state)?;
        append_admission_event(config, "superseded_queued_delivery_unknown", &state)?;
    }
    ensure!(
        state.schema.is_empty() || state.schema == ADMISSION_SCHEMA,
        "unsupported admission state schema"
    );
    ADMISSION_SCHEMA.clone_into(&mut state.schema);
    state.continuity_admitted = true;
    state.admitted_at_unix_ms = Some(unix_millis());
    state.signed_entry_id = Some(inquiry.signed_entry_id.clone());
    state.admission_id = Some(inquiry.admission_id.clone());
    state.last_response_sha256 = Some(projection.response_sha256.clone());
    state.last_summary_sha256 = Some(projection.summary_sha256.clone());
    state.last_trace_id = Some(projection.trace_id.clone());
    state.last_due_nonce = Some(projection.due_nonce.clone());
    state.queued_at_unix_ms = Some(unix_millis());
    state.terminal_at_unix_ms = None;
    state.reservoir_generation = None;
    state.reservoir_sequence = None;
    state.vector_sha256 = Some(envelope.vector_sha256());
    state.source_class = Some(envelope.source_class.as_str().to_owned());
    // Persist queued before touching the channel. A restart after this point is
    // honestly ambiguous and must never replay/amplify the authored step.
    state.reservoir_delivery = Some("queued".to_owned());
    state.provenance = Some(provenance_for_trigger(&inquiry.trigger_kind)?.to_owned());
    state.authority = Some("verified_signed_inquiry_observational_only".to_owned());
    atomic_private_json(&path, &state)?;
    append_admission_event(config, "queued", &state)?;
    Ok(true)
}

fn validate_ack(
    inquiry: &VerifiedInquiryProjection,
    ack: &SemanticAdmissionAckV1,
    expected_vector_sha256: &str,
) -> anyhow::Result<()> {
    let expected_source_class = match inquiry.trigger_kind.as_str() {
        "scheduled" => SemanticSourceClassV1::ScheduledInquiry,
        "evidence_integration" => SemanticSourceClassV1::EvidenceIntegration,
        other => bail!("unsupported inquiry trigger kind {other}"),
    };
    ensure!(
        ack.schema == crate::semantic_envelope::ACK_SCHEMA
            && ack.status == "accepted"
            && ack.admission_id == inquiry.admission_id
            && ack.signed_entry_id == inquiry.signed_entry_id
            && ack.source_class == expected_source_class
            && ack.vector_sha256 == expected_vector_sha256
            && !ack.reservoir_generation.is_empty()
            && ack.accepted_at_unix_ms > 0,
        "reservoir acknowledgment is not the exact queued inquiry admission"
    );
    Ok(())
}

fn finish_admission(
    config: &Config,
    inquiry: &VerifiedInquiryProjection,
    status: &str,
    ack: Option<&SemanticAdmissionAckV1>,
) -> anyhow::Result<()> {
    ensure!(
        matches!(status, "acknowledged" | "delivery_unknown" | "failed"),
        "invalid admission terminal status"
    );
    let path = admission_state_path(config);
    let mut state = load_admission_state(&path)?;
    reconcile_admission_receipts(config, &state)?;
    ensure!(
        state.admission_id.as_deref() == Some(inquiry.admission_id.as_str())
            && state.signed_entry_id.as_deref() == Some(inquiry.signed_entry_id.as_str())
            && state.reservoir_delivery.as_deref() == Some("queued"),
        "admission terminal update does not match the durably queued identity"
    );
    if status == "acknowledged" {
        let ack = ack.context("acknowledged admission has no acknowledgment")?;
        state.reservoir_generation = Some(ack.reservoir_generation.clone());
        state.reservoir_sequence = Some(ack.reservoir_sequence);
        state.vector_sha256 = Some(ack.vector_sha256.clone());
    } else {
        ensure!(
            ack.is_none(),
            "non-acknowledged terminal state carried an ACK"
        );
    }
    state.reservoir_delivery = Some(status.to_owned());
    state.terminal_at_unix_ms = Some(unix_millis());
    atomic_private_json(&path, &state)?;
    append_admission_event(config, status, &state)
}

fn projection_path(config: &Config) -> PathBuf {
    config.runtime_path("scheduled-introspection/projection/continuity.json")
}

fn projection_state_path(config: &Config) -> PathBuf {
    config.runtime_path("scheduled-introspection/projection/state.json")
}

fn inquiry_projection_path(config: &Config) -> PathBuf {
    config.runtime_path("scheduled-introspection/projection/inquiry-current.json")
}

fn admission_state_path(config: &Config) -> PathBuf {
    config.runtime_path("scheduled-introspection/admission/state.json")
}

fn admission_receipts_path(config: &Config) -> PathBuf {
    config.runtime_path("scheduled-introspection/admission/receipts.jsonl")
}

fn append_admission_event(
    config: &Config,
    event: &str,
    state: &AdmissionState,
) -> anyhow::Result<()> {
    let path = admission_receipts_path(config);
    let parent = path.parent().context("admission ledger has no parent")?;
    let parent_metadata = fs::symlink_metadata(parent)?;
    #[expect(
        clippy::verbose_bit_mask,
        reason = "the explicit POSIX group/world permission mask is the security invariant under review"
    )]
    let parent_is_owner_private = parent_metadata.permissions().mode() & 0o077 == 0;
    ensure!(
        parent_metadata.is_dir()
            && !parent_metadata.file_type().is_symlink()
            && parent_is_owner_private,
        "admission ledger parent is not owner-private"
    );
    let existing = fs::symlink_metadata(&path).ok();
    if let Some(metadata) = existing.as_ref() {
        ensure!(
            metadata.file_type().is_file()
                && metadata.nlink() == 1
                && metadata.permissions().mode() & 0o777 == 0o600
                && metadata.len() <= MAX_ADMISSION_LEDGER_BYTES,
            "admission ledger is not a bounded owner-private single-linked file"
        );
    }
    let record = serde_json::json!({
        "schema": ADMISSION_RECEIPT_SCHEMA,
        "event": event,
        "recorded_at_unix_ms": unix_millis(),
        "state": state,
    });
    let mut bytes = serde_json::to_vec(&record)?;
    ensure!(
        bytes.len() <= MAX_RECEIPT_LINE_BYTES,
        "admission receipt exceeds its record bound"
    );
    bytes.push(b'\n');
    let mut options = OpenOptions::new();
    options.write(true).append(true).create(true).mode(0o600);
    #[cfg(target_os = "linux")]
    options.custom_flags(0o00_400_000);
    let mut file = options.open(&path)?;
    let opened = file.metadata()?;
    ensure!(
        opened.file_type().is_file()
            && opened.nlink() == 1
            && opened.permissions().mode() & 0o777 == 0o600
            && existing.as_ref().is_none_or(|before| {
                before.dev() == opened.dev() && before.ino() == opened.ino()
            })
            && opened
                .len()
                .saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX))
                <= MAX_ADMISSION_LEDGER_BYTES,
        "admission ledger changed or exceeded its bound before append"
    );
    file.write_all(&bytes)?;
    file.sync_data()?;
    Ok(())
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
    use serde_json::{Value, json};
    use uuid::Uuid;

    use super::{
        ADMISSION_SCHEMA, AuthorshipCore, ContinuityProjection, InquiryCurrentProjection,
        ReflectionMetadata, VerifiedInquiryProjection, VerifiedProjection, finish_admission,
        queue_admission, sha256_hex, validate_authorship_core, validate_inquiry_projection,
        validate_metadata, validate_projection, validate_reflection_path, verified_thread_step,
    };
    use crate::{
        codec::encode_text,
        config::Config,
        inquiry::{InquiryBeliefOperation, InquiryThreadOperation},
        semantic_envelope::{
            ACK_SCHEMA, CODEC_VERSION, ENVELOPE_SCHEMA, SemanticAdmissionAckV1, SemanticEnvelopeV1,
            SemanticSourceClassV1, SemanticTraceV1, derive_admission_id,
        },
    };

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

    fn admission_fixture(
        root: &Path,
        seed: u8,
    ) -> (Config, VerifiedProjection, SemanticEnvelopeV1) {
        let config = test_config(root);
        let value = projection();
        let signed_entry_id = format!("inquiry-entry-{}", format!("{seed:x}").repeat(64));
        let admission_id = derive_admission_id(&config.appliance_id, &signed_entry_id);
        let inquiry = VerifiedInquiryProjection {
            signed_entry_id: signed_entry_id.clone(),
            admission_id: admission_id.clone(),
            step_id: format!("inquiry-step-{seed}"),
            trigger_kind: "scheduled".to_owned(),
            trigger_nonce: "due-12345".to_owned(),
            declaration_sha256: sha256_hex(format!("declaration-{seed}").as_bytes()),
            inquiry_step: json!({"thread_operation": "continue"}),
            reflection_sha256: value.response_sha256.clone(),
            ledger_segment: 1,
            ledger_entry_index: u64::from(seed),
            ledger_prior_entry_sha256: None,
            ledger_entry_sha256: sha256_hex(format!("entry-{seed}").as_bytes()),
        };
        let verified = VerifiedProjection {
            summary: value.summary.clone(),
            summary_sha256: value.summary_sha256.clone(),
            response_sha256: value.response_sha256.clone(),
            trace_id: value.trace.trace_id.clone(),
            due_nonce: value.due_nonce.clone(),
            recorded_at_unix_ms: value.recorded_at_unix_ms,
            trace: value.trace.clone(),
            inquiry: Some(inquiry),
        };
        let envelope = SemanticEnvelopeV1 {
            schema: ENVELOPE_SCHEMA.to_owned(),
            source_class: SemanticSourceClassV1::ScheduledInquiry,
            signed_entry_id,
            admission_id,
            summary_sha256: value.summary_sha256,
            codec_version: CODEC_VERSION.to_owned(),
            trace: SemanticTraceV1 {
                schema_version: value.trace.schema_version,
                trace_id: value.trace.trace_id,
                turn_id: value.trace.turn_id,
                span_id: value.trace.span_id,
                session_id: value.trace.session_id,
                chain_id: None,
            },
            vector: encode_text("scheduled_inquiry", &value.summary),
        };
        (config, verified, envelope)
    }

    #[test]
    fn exact_projection_validates_and_fallback_provenance_does_not() {
        let mut value = projection();
        validate_projection(&value).expect("exact projection");
        value.provenance = "local_safe_fallback".to_owned();
        assert!(validate_projection(&value).is_err());
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the helper-to-runtime wire fixture keeps every signed cross-contract field visible in one reviewable transaction"
    )]
    fn helper_v2_wire_contract_decodes_and_binds_exact_inquiry_fields() {
        let summary = "A signed observation remains distinct from its interpretation.";
        let summary_sha256 = sha256_hex(summary.as_bytes());
        let response_sha256 = sha256_hex(b"exact structured response");
        let prompt_sha256 = sha256_hex(b"rich prompt");
        let entry_id = format!("inquiry-entry-{}", "a".repeat(64));
        let step_id = format!("inquiry-step-{}", "b".repeat(64));
        let admission_id = derive_admission_id("avado-astrid", &entry_id);
        let inquiry_step = json!({
            "schema": "astrid.edge.inquiry.step.v1",
            "thread_operation": "open",
            "thread_id": "thread-contract",
            "parent_step_id": null,
            "observation": "A bounded observation.",
            "interpretation": "A bounded interpretation.",
            "uncertainty": "A bounded uncertainty.",
            "decision": "Open a bounded thread.",
            "counterpoint": null,
            "next_test": null,
            "evidence_ids": [],
            "confidence": "tentative",
            "belief_operation": null,
            "belief_id": null,
            "belief_claim": null
        });
        let inquiry_step_sha256 =
            sha256_hex(&super::canonical_json(&inquiry_step).expect("canonical inquiry step"));
        let declaration_sha256 = sha256_hex(b"INQUIRY_STEP: exact");
        let inquiry_current_sha256 = "c".repeat(64);
        let trace = json!({
            "schema_version": 1,
            "trace_id": Uuid::from_u128(31),
            "turn_id": Uuid::from_u128(32),
            "span_id": Uuid::from_u128(33),
            "session_id": "session-v2-contract"
        });
        let reflection_path = format!(
            "introspections/scheduled/reflection_due-12345_{}.md",
            Uuid::from_u128(32)
        );
        let mut continuity: ContinuityProjection = serde_json::from_value(json!({
            "schema": "astrid_edge_scheduled_introspection_continuity_v2",
            "appliance_id": "avado-astrid",
            "model": "qwen3.5:4b",
            "trigger_kind": "scheduled",
            "trigger_nonce": "due-12345",
            "due_nonce": "due-12345",
            "recorded_at_unix_ms": 12_347_000_u64,
            "summary": summary,
            "summary_sha256": summary_sha256,
            "response_sha256": response_sha256,
            "prompt_sha256": prompt_sha256,
            "reflection_path": reflection_path,
            "signed_entry_id": entry_id,
            "step_id": step_id,
            "admission_id": admission_id,
            "inquiry_current_projection_sha256": inquiry_current_sha256,
            "trace": trace,
            "provenance": "model_authored_runtime_scheduled",
            "authority": "bounded_signed_inquiry_continuity_projection_not_code_or_action_authority",
            "context_provenance": {},
            "context_provenance_sha256": "d".repeat(64),
            "candidate_authoring_eligible": false,
            "reflection_lane": "rich_owned_context",
            "taint_causes": []
        }))
        .expect("decode helper v2 continuity wire shape");
        validate_projection(&continuity).expect("validate helper v2 continuity");

        let mut core: AuthorshipCore = serde_json::from_value(json!({
            "schema": "astrid.edge.scheduled_authorship.attestation.v2",
            "appliance_id": "avado-astrid",
            "due_nonce": "due-12345",
            "due_at_unix_ms": 12_345_000_u64,
            "started_at_unix_ms": 12_346_000_u64,
            "completed_at_unix_ms": 12_347_000_u64,
            "terminal_status": "model_authored_structured",
            "model": "qwen3.5:4b",
            "prompt_sha256": prompt_sha256,
            "response_sha256": response_sha256,
            "reflection_path": continuity.reflection_path,
            "reflection_sha256": response_sha256,
            "reflection_metadata_sha256": "e".repeat(64),
            "continuity_projection_sha256": "f".repeat(64),
            "inquiry_current_projection_sha256": inquiry_current_sha256,
            "signed_entry_id": entry_id,
            "step_id": step_id,
            "admission_id": admission_id,
            "inquiry_step_sha256": inquiry_step_sha256,
            "inquiry_declaration_sha256": declaration_sha256,
            "state_projection_sha256": "1".repeat(64),
            "terminal_receipt_sha256": "2".repeat(64),
            "context_provenance_sha256": "d".repeat(64),
            "candidate_id": null,
            "candidate_digest": null,
            "trace": trace,
            "provenance": "model_authored_runtime_scheduled",
            "authority": "immutable_steward_signed_exact_authorship_join"
        }))
        .expect("decode helper v2 authorship core wire shape");
        let mut config = test_config(Path::new("/tmp/astrid-v2-contract"));
        config.appliance_id = "avado-astrid".to_owned();
        config.local_model_id = "qwen3.5:4b".to_owned();
        validate_authorship_core(&config, &core).expect("validate v2 authorship core");

        let mut current: InquiryCurrentProjection = serde_json::from_value(json!({
            "schema": "astrid.edge.inquiry.current.v1",
            "appliance_id": "avado-astrid",
            "signed_entry_id": entry_id,
            "step_id": step_id,
            "admission_id": admission_id,
            "summary": summary,
            "summary_sha256": summary_sha256,
            "inquiry_step": inquiry_step,
            "inquiry_step_sha256": inquiry_step_sha256,
            "declaration_sha256": declaration_sha256,
            "response_sha256": response_sha256,
            "trace": trace,
            "trigger_kind": "scheduled",
            "due_nonce": "due-12345",
            "trigger_nonce": "due-12345",
            "recorded_at_unix_ms": 12_347_000_u64,
            "reflection_path": continuity.reflection_path,
            "reflection_sha256": response_sha256,
            "ledger": {
                "segment": 1,
                "entry_index": 1,
                "prior_entry_sha256": "0".repeat(64),
                "entry_sha256": "3".repeat(64),
                "key_id": "ed25519:0123456789abcdef",
                "signature_algorithm": "ed25519",
                "signature": "11".repeat(64)
            },
            "provenance": "model_authored_runtime_scheduled",
            "authority": "immutable_steward_signed_bounded_inquiry_projection_observational_only",
            "core_sha256": "4".repeat(64),
            "auth": {
                "algorithm": "ed25519",
                "key_id": "ed25519:0123456789abcdef",
                "signature": "22".repeat(64)
            }
        }))
        .expect("decode helper inquiry-current wire shape");
        validate_inquiry_projection(&config, &core, &continuity, &current)
            .expect("bind helper v2 inquiry-current fields");

        continuity.trigger_kind = Some("evidence_integration".to_owned());
        continuity.trigger_nonce = Some("integration-0123456789abcdef".to_owned());
        continuity.provenance = "model_authored_runtime_evidence_integration".to_owned();
        core.provenance = "model_authored_runtime_evidence_integration".to_owned();
        current.trigger_kind = "evidence_integration".to_owned();
        current.trigger_nonce = "integration-0123456789abcdef".to_owned();
        current.provenance = "model_authored_runtime_evidence_integration".to_owned();
        validate_projection(&continuity).expect("validate integration continuity provenance");
        validate_authorship_core(&config, &core).expect("validate integration authorship core");
        validate_inquiry_projection(&config, &core, &continuity, &current)
            .expect("bind integration inquiry-current provenance");
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
    fn admission_reconciles_state_first_queue_crash_before_deduplication() {
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
        let signed_entry_id =
            "inquiry-entry-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let admission_id = derive_admission_id(&config.appliance_id, signed_entry_id);
        let inquiry = VerifiedInquiryProjection {
            signed_entry_id: signed_entry_id.to_owned(),
            admission_id: admission_id.clone(),
            step_id: "inquiry-step-1".to_owned(),
            trigger_kind: "scheduled".to_owned(),
            trigger_nonce: "due-12345".to_owned(),
            declaration_sha256: sha256_hex(b"declaration"),
            inquiry_step: json!({"thread_operation": "continue"}),
            reflection_sha256: value.response_sha256.clone(),
            ledger_segment: 1,
            ledger_entry_index: 1,
            ledger_prior_entry_sha256: None,
            ledger_entry_sha256: sha256_hex(b"entry"),
        };
        let verified = super::VerifiedProjection {
            summary: value.summary.clone(),
            summary_sha256: value.summary_sha256.clone(),
            response_sha256: value.response_sha256.clone(),
            trace_id: value.trace.trace_id.clone(),
            due_nonce: value.due_nonce.clone(),
            recorded_at_unix_ms: value.recorded_at_unix_ms,
            trace: value.trace.clone(),
            inquiry: Some(inquiry),
        };
        let envelope = SemanticEnvelopeV1 {
            schema: ENVELOPE_SCHEMA.to_owned(),
            source_class: SemanticSourceClassV1::ScheduledInquiry,
            signed_entry_id: signed_entry_id.to_owned(),
            admission_id,
            summary_sha256: value.summary_sha256,
            codec_version: CODEC_VERSION.to_owned(),
            trace: SemanticTraceV1 {
                schema_version: value.trace.schema_version,
                trace_id: value.trace.trace_id,
                turn_id: value.trace.turn_id,
                span_id: value.trace.span_id,
                session_id: value.trace.session_id,
                chain_id: None,
            },
            vector: encode_text("scheduled_inquiry", &value.summary),
        };
        assert!(queue_admission(&config, &verified, &envelope).expect("first"));
        fs::remove_file(root.join("runtime/scheduled-introspection/admission/receipts.jsonl"))
            .expect("simulate lost queued append after durable state");
        assert!(!queue_admission(&config, &verified, &envelope).expect("duplicate"));
        let state: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join("runtime/scheduled-introspection/admission/state.json"))
                .expect("state"),
        )
        .expect("json");
        assert_eq!(state["schema"], ADMISSION_SCHEMA);
        assert_eq!(state["reservoir_delivery"], "delivery_unknown");
        let receipts = fs::read_to_string(
            root.join("runtime/scheduled-introspection/admission/receipts.jsonl"),
        )
        .expect("admission receipts");
        let events = receipts
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("receipt"))
            .map(|receipt| receipt["event"].as_str().unwrap_or_default().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(events, ["queued", "interrupted_before_ack"]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn admission_reconciles_terminal_state_when_terminal_append_was_lost() {
        let root = std::env::temp_dir().join(format!(
            "astrid-scheduled-terminal-reconcile-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(root.join("runtime/scheduled-introspection/admission")).expect("dirs");
        fs::set_permissions(
            root.join("runtime/scheduled-introspection/admission"),
            fs::Permissions::from_mode(0o700),
        )
        .expect("mode");
        let (config, verified, envelope) = admission_fixture(&root, 1);
        let inquiry = verified.inquiry.as_ref().expect("inquiry");
        assert!(queue_admission(&config, &verified, &envelope).expect("queue"));
        let ack = SemanticAdmissionAckV1 {
            schema: ACK_SCHEMA.to_owned(),
            admission_id: envelope.admission_id.clone(),
            signed_entry_id: envelope.signed_entry_id.clone(),
            source_class: SemanticSourceClassV1::ScheduledInquiry,
            reservoir_generation: "reservoir-generation-one".to_owned(),
            reservoir_sequence: 17,
            vector_sha256: envelope.vector_sha256(),
            accepted_at_unix_ms: 10,
            status: "accepted".to_owned(),
        };
        finish_admission(&config, inquiry, "acknowledged", Some(&ack))
            .expect("terminal state and receipt");
        let ledger = root.join("runtime/scheduled-introspection/admission/receipts.jsonl");
        let first = fs::read_to_string(&ledger)
            .expect("ledger")
            .lines()
            .next()
            .expect("queued receipt")
            .to_owned();
        fs::write(&ledger, format!("{first}\n")).expect("simulate terminal receipt append loss");
        fs::set_permissions(&ledger, fs::Permissions::from_mode(0o600)).expect("mode");

        assert!(!queue_admission(&config, &verified, &envelope).expect("reconcile"));
        let receipts = fs::read_to_string(&ledger).expect("reconciled ledger");
        let events = receipts
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("receipt"))
            .map(|receipt| receipt["event"].as_str().unwrap_or_default().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(events, ["queued", "acknowledged"]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn superseded_queued_admission_retains_complete_prior_lifecycle() {
        let root = std::env::temp_dir().join(format!(
            "astrid-scheduled-superseded-history-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(root.join("runtime/scheduled-introspection/admission")).expect("dirs");
        fs::set_permissions(
            root.join("runtime/scheduled-introspection/admission"),
            fs::Permissions::from_mode(0o700),
        )
        .expect("mode");
        let (config, first, first_envelope) = admission_fixture(&root, 1);
        let (_same_config, second, second_envelope) = admission_fixture(&root, 2);
        assert!(queue_admission(&config, &first, &first_envelope).expect("first"));
        assert!(queue_admission(&config, &second, &second_envelope).expect("second"));

        let receipts = fs::read_to_string(
            root.join("runtime/scheduled-introspection/admission/receipts.jsonl"),
        )
        .expect("ledger");
        let values = receipts
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("receipt"))
            .collect::<Vec<_>>();
        assert_eq!(
            values
                .iter()
                .map(|value| value["event"].as_str().unwrap_or_default())
                .collect::<Vec<_>>(),
            ["queued", "superseded_queued_delivery_unknown", "queued"]
        );
        assert_eq!(
            values[0]["state"]["admission_id"],
            first_envelope.admission_id
        );
        assert_eq!(
            values[1]["state"]["admission_id"],
            first_envelope.admission_id
        );
        assert_eq!(
            values[2]["state"]["admission_id"],
            second_envelope.admission_id
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn signed_inquiry_maps_strictly_into_the_v7_thread_transaction() {
        let value = projection();
        let step = json!({
            "schema": "astrid.edge.inquiry.step.v1",
            "thread_operation": "open",
            "thread_id": "thread-local-question",
            "parent_step_id": null,
            "observation": "A bounded local observation.",
            "interpretation": "The observation supports a testable local question.",
            "uncertainty": "No local measurement has answered it yet.",
            "decision": "Open the question without claiming an answer.",
            "counterpoint": null,
            "next_test": "Measure one eligible local signal.",
            "evidence_ids": [],
            "confidence": "tentative",
            "belief_operation": "propose",
            "belief_id": "belief-local-question",
            "belief_claim": "The local signal may vary with activity."
        });
        let inquiry = VerifiedInquiryProjection {
            signed_entry_id:
                "inquiry-entry-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_owned(),
            admission_id:
                "inquiry-admission-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_owned(),
            step_id: "inquiry-step-local-question".to_owned(),
            trigger_kind: "scheduled".to_owned(),
            trigger_nonce: "due-12345".to_owned(),
            declaration_sha256: "c".repeat(64),
            inquiry_step: step,
            reflection_sha256: value.response_sha256.clone(),
            ledger_segment: 1,
            ledger_entry_index: 1,
            ledger_prior_entry_sha256: None,
            ledger_entry_sha256: "d".repeat(64),
        };
        let verified = VerifiedProjection {
            summary: value.summary,
            summary_sha256: value.summary_sha256,
            response_sha256: value.response_sha256,
            trace_id: value.trace.trace_id.clone(),
            due_nonce: value.due_nonce,
            recorded_at_unix_ms: value.recorded_at_unix_ms,
            trace: value.trace,
            inquiry: Some(inquiry.clone()),
        };
        let mapped = verified_thread_step(&verified, &inquiry).expect("strict mapping");
        assert_eq!(mapped.thread_operation, InquiryThreadOperation::Open);
        assert_eq!(mapped.thread_id.as_deref(), Some("thread-local-question"));
        assert_eq!(
            mapped.belief_operation,
            Some(InquiryBeliefOperation::Propose)
        );
        assert_eq!(mapped.belief_id.as_deref(), Some("belief-local-question"));
        assert!(mapped.mechanical_predecessor_hash.is_none());

        let mut unknown = inquiry;
        unknown.inquiry_step["invented"] = json!(true);
        assert!(verified_thread_step(&verified, &unknown).is_err());
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
