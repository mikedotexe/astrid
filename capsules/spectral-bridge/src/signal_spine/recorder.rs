use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::capture::{
    CaptureSubmitResultV1, CaptureWindowRequestV1, PendingVectorCaptureV1, try_submit_captures,
};
use super::types::{
    CausalSignalJourneyV1, CausalSignalStageV1, SignalCaptureFixtureRefV1, SignalEffectV1,
    SignalOwnershipDomainV1, SignalProcessIdentityV1, SignalRelationV1, SignalStageKindV1,
    SignalStageReceiptV1, SignalTemporalEnvelopeV1,
};
use crate::paths::bridge_paths;
use crate::witness::{ProvenanceInfluenceTypeV1, ProvenanceOriginV1, ProvenanceRefV1};

fn process_started() -> &'static Instant {
    static STARTED: OnceLock<Instant> = OnceLock::new();
    STARTED.get_or_init(Instant::now)
}

fn unix_ms() -> u64 {
    u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or(0)
}

fn monotonic_ns() -> u64 {
    u64::try_from(process_started().elapsed().as_nanos()).unwrap_or(u64::MAX)
}

pub(super) fn executable_name() -> &'static str {
    static EXECUTABLE: OnceLock<String> = OnceLock::new();
    EXECUTABLE
        .get_or_init(|| {
            std::env::current_exe()
                .ok()
                .and_then(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| "spectral-bridge-server".to_string())
        })
        .as_str()
}

#[derive(Debug)]
struct CaptureActivationCacheV1 {
    checked_at: Option<Instant>,
    capture_window_id: Option<String>,
}

fn active_capture_window_id(root: &Path, now_ms: u64) -> Option<String> {
    const REFRESH_INTERVAL: Duration = Duration::from_millis(100);
    static CACHE: OnceLock<Mutex<CaptureActivationCacheV1>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        Mutex::new(CaptureActivationCacheV1 {
            checked_at: None,
            capture_window_id: None,
        })
    });
    let mut cache = cache.lock().ok()?;
    let now = Instant::now();
    if cache.checked_at.is_some_and(|checked_at| {
        cache.capture_window_id.is_none() && now.duration_since(checked_at) < REFRESH_INTERVAL
    }) {
        return None;
    }
    cache.capture_window_id =
        CaptureWindowRequestV1::load(root, now_ms).map(|window| window.id().to_string());
    cache.checked_at = Some(now);
    cache.capture_window_id.clone()
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sha256_json<T: Serialize>(value: &T) -> String {
    serde_json::to_vec(value).map_or_else(
        |_| sha256_bytes(b"serialization_failed"),
        |bytes| sha256_bytes(&bytes),
    )
}

fn vector_sha256(vector: &[f32]) -> String {
    let mut bytes = Vec::with_capacity(vector.len().saturating_mul(4));
    for value in vector {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    sha256_bytes(&bytes)
}

#[derive(Debug, Clone)]
pub(crate) struct SignalJourneyContextV1 {
    pub(crate) exchange: u64,
    pub(crate) source_time_ms: Option<u64>,
    pub(crate) connection_id: &'static str,
    pub(crate) connection_sequence: u64,
    pub(crate) deployment_identity: String,
}

#[derive(Debug, Clone)]
pub(crate) struct SignalStageHandleV1 {
    stage_id: String,
    output_sha256: String,
}

impl SignalStageHandleV1 {
    fn from_receipt(receipt: &SignalStageReceiptV1) -> Self {
        Self {
            stage_id: receipt.stage_id().to_string(),
            output_sha256: receipt.output_sha256().to_string(),
        }
    }

    pub(super) fn stage_id(&self) -> &str {
        &self.stage_id
    }

    pub(super) fn output_sha256(&self) -> &str {
        &self.output_sha256
    }
}

#[derive(Debug)]
pub(crate) struct ShadowSignalJourneyV1 {
    trusted: CausalSignalJourneyV1,
    context: SignalJourneyContextV1,
    arrival_time_unix_ms: u64,
    next_stage_index: u32,
    pending_captures: Vec<PendingVectorCaptureV1>,
    capture_window_id: Option<String>,
    parity_mismatch_count: u32,
}

impl ShadowSignalJourneyV1 {
    pub(crate) fn begin_authored(
        context: SignalJourneyContextV1,
        authored_text: &str,
    ) -> Result<(Self, SignalStageHandleV1), String> {
        let arrival = unix_ms();
        let authored_sha256 = sha256_bytes(authored_text.as_bytes());
        let journey_seed = json!({
            "exchange": context.exchange,
            "authored_sha256": authored_sha256,
            "connection_sequence": context.connection_sequence,
            "process_id": std::process::id(),
            "arrival_time_unix_ms": arrival,
        });
        let journey_digest = sha256_json(&journey_seed);
        let journey_id = format!("journey_{}", &journey_digest[..24]);
        let capture_root = default_signal_spine_root();
        let capture_window_id = active_capture_window_id(&capture_root, arrival);
        let mut shadow = Self {
            trusted: CausalSignalJourneyV1::new(journey_id),
            context,
            arrival_time_unix_ms: arrival,
            next_stage_index: 0,
            pending_captures: Vec::new(),
            capture_window_id,
            parity_mismatch_count: 0,
        };
        let authored = shadow.record_hashes(
            SignalStageKindV1::Authored,
            SignalRelationV1::Root,
            SignalEffectV1::Produced,
            SignalOwnershipDomainV1::AstridAuthored,
            &[],
            &authored_sha256,
            &authored_sha256,
            BTreeMap::from([
                ("text_bytes".to_string(), json!(authored_text.len())),
                ("raw_response_prose_persisted".to_string(), json!(false)),
            ]),
        )?;
        Ok((shadow, authored))
    }

    pub(crate) fn record_text(
        &mut self,
        kind: SignalStageKindV1,
        relation: SignalRelationV1,
        effect: SignalEffectV1,
        ownership: SignalOwnershipDomainV1,
        parents: &[&SignalStageHandleV1],
        text: &str,
        measurements: BTreeMap<String, Value>,
    ) -> Result<SignalStageHandleV1, String> {
        let output = sha256_bytes(text.as_bytes());
        let source = source_hash(parents, &output);
        self.record_hashes(
            kind,
            relation,
            effect,
            ownership,
            parents,
            &source,
            &output,
            measurements,
        )
    }

    pub(crate) fn record_vector(
        &mut self,
        kind: SignalStageKindV1,
        effect: SignalEffectV1,
        ownership: SignalOwnershipDomainV1,
        parent: &SignalStageHandleV1,
        vector: &[f32],
        mut measurements: BTreeMap<String, Value>,
    ) -> Result<SignalStageHandleV1, String> {
        let output = vector_sha256(vector);
        measurements.insert("vector_dimensions".to_string(), json!(vector.len()));
        measurements.insert("full_vector_persisted".to_string(), json!(false));
        let handle = self.record_hashes(
            kind,
            SignalRelationV1::ExactTransformation,
            effect,
            ownership,
            &[parent],
            &parent.output_sha256,
            &output,
            measurements,
        )?;
        if self.capture_window_id.is_some() {
            self.pending_captures
                .push(PendingVectorCaptureV1::new(handle.stage_id.clone(), vector));
        }
        Ok(handle)
    }

    pub(crate) fn record_json<T: Serialize>(
        &mut self,
        kind: SignalStageKindV1,
        relation: SignalRelationV1,
        effect: SignalEffectV1,
        ownership: SignalOwnershipDomainV1,
        parents: &[&SignalStageHandleV1],
        value: &T,
        measurements: BTreeMap<String, Value>,
    ) -> Result<SignalStageHandleV1, String> {
        let output = sha256_json(value);
        let source = source_hash(parents, &output);
        self.record_hashes(
            kind,
            relation,
            effect,
            ownership,
            parents,
            &source,
            &output,
            measurements,
        )
    }

    pub(crate) fn note_parity_mismatch(&mut self) {
        self.parity_mismatch_count = self.parity_mismatch_count.saturating_add(1);
    }

    pub(super) fn journey_id(&self) -> &str {
        self.trusted.journey_id()
    }

    #[cfg(test)]
    pub(super) fn parent_chain_valid(&self) -> bool {
        self.trusted.validate_parent_chain().is_ok()
    }

    #[cfg(test)]
    pub(super) fn trusted_receipts_for_test(&self) -> &[SignalStageReceiptV1] {
        self.trusted.receipts()
    }

    #[cfg(test)]
    pub(super) fn attach_capture_fixture_for_test(&mut self, stage_id: &str) {
        if let Some(receipt) = self
            .trusted
            .receipts_mut()
            .iter_mut()
            .find(|receipt| receipt.stage_id() == stage_id)
        {
            receipt.set_capture_fixture_ref(SignalCaptureFixtureRefV1::new(
                "capture_test".to_string(),
                "f".repeat(64),
                "captures/capture_test/fixtures/test.json".to_string(),
                48,
            ));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn record_hashes(
        &mut self,
        kind: SignalStageKindV1,
        relation: SignalRelationV1,
        effect: SignalEffectV1,
        ownership: SignalOwnershipDomainV1,
        parents: &[&SignalStageHandleV1],
        source_sha256: &str,
        output_sha256: &str,
        measurements: BTreeMap<String, Value>,
    ) -> Result<SignalStageHandleV1, String> {
        let stage_index = self.next_stage_index;
        self.next_stage_index = self.next_stage_index.saturating_add(1);
        let parent_stage_ids = parents
            .iter()
            .map(|parent| parent.stage_id.clone())
            .collect::<Vec<_>>();
        let stage_digest = sha256_json(&json!({
            "journey_id": self.trusted.journey_id(),
            "stage_index": stage_index,
            "kind": kind,
            "relation": relation,
            "effect": effect,
            "ownership": ownership.as_str(),
            "parents": parent_stage_ids,
            "source_sha256": source_sha256,
            "output_sha256": output_sha256,
        }));
        let stage_id = format!("stage_{}", &stage_digest[..24]);
        let stage_time = unix_ms();
        let provenance = ProvenanceRefV1::new(
            provenance_origin(ownership),
            stage_id.clone(),
            output_sha256.to_string(),
            parent_stage_ids.clone(),
            stage_time,
            vec![format!("signal_spine.{}", kind.as_str())],
            provenance_influences(relation, ownership),
        );
        let receipt = SignalStageReceiptV1::new(
            self.trusted.journey_id().to_string(),
            stage_id,
            stage_index,
            kind,
            relation,
            effect,
            ownership,
            parent_stage_ids,
            source_sha256.to_string(),
            output_sha256.to_string(),
            provenance,
            SignalProcessIdentityV1::current(self.context.deployment_identity.clone()),
            SignalTemporalEnvelopeV1::new(
                self.context.source_time_ms,
                self.arrival_time_unix_ms,
                stage_time,
                monotonic_ns(),
                self.context.connection_id.to_string(),
                self.context.connection_sequence,
                self.capture_window_id.clone(),
            ),
            measurements,
            None,
        )?;
        let trusted_stage = CausalSignalStageV1::new(receipt, ());
        let handle = SignalStageHandleV1::from_receipt(trusted_stage.receipt());
        self.trusted.push(trusted_stage.receipt().clone())?;
        Ok(handle)
    }
}

fn source_hash(parents: &[&SignalStageHandleV1], fallback: &str) -> String {
    match parents {
        [] => fallback.to_string(),
        [parent] => parent.output_sha256.clone(),
        _ => sha256_json(
            &parents
                .iter()
                .map(|parent| parent.output_sha256.as_str())
                .collect::<Vec<_>>(),
        ),
    }
}

const fn provenance_origin(ownership: SignalOwnershipDomainV1) -> ProvenanceOriginV1 {
    match ownership {
        SignalOwnershipDomainV1::AstridAuthored => ProvenanceOriginV1::AstridInterpretation,
        SignalOwnershipDomainV1::BridgeCodec
        | SignalOwnershipDomainV1::BridgeEvidence
        | SignalOwnershipDomainV1::BridgeSafety
        | SignalOwnershipDomainV1::BridgeDispatch => ProvenanceOriginV1::BridgeDerived,
        SignalOwnershipDomainV1::MinimeObserved => ProvenanceOriginV1::MinimeObservation,
    }
}

fn provenance_influences(
    relation: SignalRelationV1,
    ownership: SignalOwnershipDomainV1,
) -> Vec<ProvenanceInfluenceTypeV1> {
    let mut influences = match relation {
        SignalRelationV1::Root => vec![ProvenanceInfluenceTypeV1::Authorship],
        SignalRelationV1::TemporalAssociation => vec![ProvenanceInfluenceTypeV1::Temporal],
        SignalRelationV1::ExactReview
        | SignalRelationV1::SafetyDecision
        | SignalRelationV1::DeliveryEvidence => {
            vec![ProvenanceInfluenceTypeV1::Interpretive]
        },
        SignalRelationV1::ExactTransformation | SignalRelationV1::DispatchOutcome => {
            vec![ProvenanceInfluenceTypeV1::Structural]
        },
    };
    if ownership == SignalOwnershipDomainV1::AstridAuthored
        && !influences.contains(&ProvenanceInfluenceTypeV1::Authorship)
    {
        influences.push(ProvenanceInfluenceTypeV1::Authorship);
    }
    influences
}

#[derive(Debug, Serialize)]
struct PersistedSignalJourneyV1<'a> {
    schema: &'static str,
    schema_version: u8,
    journey_id: &'a str,
    stage_count: usize,
    parity_mismatch_count: u32,
    lineage_valid: bool,
    temporal_association_is_not_direct_causation: bool,
    sensory_protocol_changed: bool,
    raw_response_prose_included: bool,
    live_control_authority: bool,
    artifact_authority_state_v1: Value,
    receipts: &'a [SignalStageReceiptV1],
}

pub(crate) fn persist_shadow_signal_journey_v1(
    mut shadow: ShadowSignalJourneyV1,
) -> std::io::Result<PathBuf> {
    let root = default_signal_spine_root();
    let now = unix_ms();
    match try_submit_captures(
        &root,
        shadow.trusted.journey_id(),
        std::mem::take(&mut shadow.pending_captures),
        now,
    ) {
        CaptureSubmitResultV1::Accepted(references) => {
            for (stage_id, fixture_sha256, relative_path, dimensions) in references {
                if let Some(receipt) = shadow
                    .trusted
                    .receipts_mut()
                    .iter_mut()
                    .find(|receipt| receipt.stage_id() == stage_id)
                {
                    receipt.set_capture_fixture_ref(SignalCaptureFixtureRefV1::new(
                        shadow.capture_window_id.clone().unwrap_or_default(),
                        fixture_sha256,
                        relative_path,
                        dimensions,
                    ));
                }
            }
        },
        gap @ (CaptureSubmitResultV1::QueueFull
        | CaptureSubmitResultV1::InvalidVectorDimensions
        | CaptureSubmitResultV1::WindowUnavailable
        | CaptureSubmitResultV1::JourneyLimitReached) => {
            let reason = match gap {
                CaptureSubmitResultV1::QueueFull => "capture_queue_exhausted",
                CaptureSubmitResultV1::InvalidVectorDimensions => {
                    "capture_vector_dimension_mismatch"
                },
                CaptureSubmitResultV1::WindowUnavailable => {
                    "capture_window_unavailable_at_submission"
                },
                CaptureSubmitResultV1::JourneyLimitReached => {
                    "capture_journey_limit_reached_at_submission"
                },
                _ => unreachable!(),
            };
            let parent = shadow
                .trusted
                .receipts()
                .last()
                .map(SignalStageHandleV1::from_receipt);
            if let Some(parent) = parent {
                let _ = shadow.record_json(
                    SignalStageKindV1::CaptureGap,
                    SignalRelationV1::DeliveryEvidence,
                    SignalEffectV1::CaptureGap,
                    SignalOwnershipDomainV1::BridgeEvidence,
                    &[&parent],
                    &json!({"reason": reason, "dossier_sufficient": false}),
                    BTreeMap::from([("dossier_sufficient".to_string(), json!(false))]),
                );
            }
        },
        CaptureSubmitResultV1::NotArmed => {},
    }
    let lineage_valid = shadow.trusted.validate_parent_chain().is_ok();
    let persisted = PersistedSignalJourneyV1 {
        schema: "causal_signal_journey_v1",
        schema_version: 1,
        journey_id: shadow.trusted.journey_id(),
        stage_count: shadow.trusted.receipts().len(),
        parity_mismatch_count: shadow.parity_mismatch_count,
        lineage_valid,
        temporal_association_is_not_direct_causation: true,
        sensory_protocol_changed: false,
        raw_response_prose_included: false,
        live_control_authority: false,
        artifact_authority_state_v1: json!({
            "schema": "artifact_authority_state_v1",
            "schema_version": 1,
            "state": "evidence_only",
            "live_eligible_now": false,
            "auto_approved": false,
            "grants_approval": false,
            "edits_source_now": false,
        }),
        receipts: shadow.trusted.receipts(),
    };
    let bytes = serde_json::to_vec(&persisted).map_err(std::io::Error::other)?;
    let path = root
        .join("journeys")
        .join(format!("{}.json", shadow.trusted.journey_id()));
    write_owner_only(&path, &bytes)?;
    write_status(&root, &persisted)?;
    Ok(path)
}

fn default_signal_spine_root() -> PathBuf {
    bridge_paths()
        .bridge_workspace()
        .join("diagnostics/signal_spine_v1")
}

pub(crate) fn signal_deployment_identity_v1() -> String {
    let manifest = bridge_paths()
        .bridge_workspace()
        .join("deployment_manifests/spectral-bridge.json");
    let value = fs::read(&manifest)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let head = value
        .as_ref()
        .and_then(|item| item.pointer("/repository/head"))
        .and_then(Value::as_str)
        .unwrap_or("unknown_source");
    let binary = value
        .as_ref()
        .and_then(|item| item.pointer("/artifacts/spectral-bridge/sha256"))
        .and_then(Value::as_str)
        .unwrap_or("unknown_binary");
    format!("astrid:{head}:bridge:{binary}")
}

fn write_owner_only(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.write_all(bytes)?;
    file.flush()?;
    Ok(())
}

fn write_status(root: &Path, journey: &PersistedSignalJourneyV1<'_>) -> std::io::Result<()> {
    let status = serde_json::to_vec_pretty(&json!({
        "schema": "signal_spine_shadow_status_v1",
        "schema_version": 1,
        "updated_at_unix_ms": unix_ms(),
        "latest_journey_id": journey.journey_id,
        "latest_stage_count": journey.stage_count,
        "latest_lineage_valid": journey.lineage_valid,
        "latest_parity_mismatch_count": journey.parity_mismatch_count,
        "mode": "shadow",
        "projection_cutover": false,
        "sensory_protocol_changed": false,
        "temporal_association_is_not_direct_causation": true,
        "raw_response_prose_included": false,
        "artifact_authority_state_v1": journey.artifact_authority_state_v1,
    }))
    .map_err(std::io::Error::other)?;
    write_owner_only(&root.join("status.json"), &status)
}
