//! Immutable evidence-only context for authored introspection reports.

mod identity;
#[path = "peer_snapshot.rs"]
mod peer_evidence_cache;
mod types;
mod writer;

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use sha2::{Digest, Sha256};

use types::{LivedStateBuildCandidateV1, LivedStateObservationKindV1};
pub use types::{
    LivedStateGapReceiptV1, LivedStateLlmResultV1, LivedStateModelRouteV1,
    LivedStateParameterObservationV1, LivedStateProcessIdentityV1, LivedStateSourceSnapshotV1,
    TemporalLivedStateWitnessV1,
};
pub(crate) use writer::WitnessSubmitResultV1;

use crate::paths::bridge_paths;
use crate::witness::{ProvenanceInfluenceTypeV1, ProvenanceOriginV1, ProvenanceRefV1};
use crate::ws::BridgeState;
use peer_evidence_cache::PeerEvidenceSourceStatusV1;

const MAX_PEER_STATE_AGE_MS: u64 = 30_000;
static WITNESS_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy)]
pub(crate) struct LivedStateClockSampleV1 {
    pub unix_ms: u64,
    pub monotonic_ns: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct LivedStateAuthorshipV1 {
    witness_id: String,
    authored_at: LivedStateClockSampleV1,
    authored_process_sequence: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LivedStateRuntimeContextV1 {
    pub parameter_observations: Vec<LivedStateParameterObservationV1>,
    pub peer_process_identity: Option<String>,
    pub peer_deployment_identity: Option<String>,
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn bounded_identity(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            if Path::new(value).is_absolute() {
                format!("sha256:{}", sha256_bytes(value.as_bytes()))
            } else {
                value.chars().take(160).collect()
            }
        })
}

fn next_witness_sequence() -> u64 {
    WITNESS_SEQUENCE
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map_or(u64::MAX, |previous| previous.saturating_add(1))
}

pub fn initialize_runtime_identity_v1() {
    let _ = identity::initialize();
    peer_evidence_cache::initialize(
        bridge_paths()
            .minime_workspace()
            .join("spectral_state.json"),
    );
}

pub(crate) fn clock_sample_v1() -> LivedStateClockSampleV1 {
    identity::clock_sample()
}

fn source_owner_and_relative_path(path: &Path) -> (String, String) {
    let paths = bridge_paths();
    for (owner, root) in [
        ("astrid", paths.astrid_root()),
        ("minime", paths.minime_root()),
    ] {
        if let Ok(relative) = path.strip_prefix(root) {
            return (owner.to_string(), relative.to_string_lossy().into_owned());
        }
    }
    if let Ok(relative) = path.strip_prefix(paths.bridge_workspace()) {
        return (
            "astrid_workspace".to_string(),
            format!("workspace/{}", relative.to_string_lossy()),
        );
    }
    if let Ok(relative) = path.strip_prefix(paths.minime_workspace()) {
        return (
            "minime_workspace".to_string(),
            format!("workspace/{}", relative.to_string_lossy()),
        );
    }
    ("unknown".to_string(), "redacted/unknown".to_string())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn source_snapshot_v1(
    canonical_path: &Path,
    file_content: &str,
    rendered_window: &str,
    start_line: usize,
    end_line: usize,
    total_lines: usize,
    source_read_at: LivedStateClockSampleV1,
) -> LivedStateSourceSnapshotV1 {
    let (source_owner, repository_relative_path) = source_owner_and_relative_path(canonical_path);
    let file_sha256 = sha256_bytes(file_content.as_bytes());
    let window_sha256 = sha256_bytes(rendered_window.as_bytes());
    let provenance = ProvenanceRefV1::new(
        if source_owner.starts_with("minime") {
            ProvenanceOriginV1::MinimeObservation
        } else {
            ProvenanceOriginV1::AstridInterpretation
        },
        format!("source_window:{source_owner}:{repository_relative_path}"),
        window_sha256.clone(),
        Vec::new(),
        source_read_at.unix_ms,
        vec![
            "source.file_sha256".to_string(),
            "source.window_sha256".to_string(),
            "source.window_lines".to_string(),
        ],
        vec![
            ProvenanceInfluenceTypeV1::Structural,
            ProvenanceInfluenceTypeV1::Authorship,
        ],
    );
    LivedStateSourceSnapshotV1::new(
        source_owner,
        repository_relative_path,
        start_line,
        end_line,
        total_lines,
        file_sha256,
        window_sha256,
        source_read_at.unix_ms,
        source_read_at.monotonic_ns,
        provenance,
    )
}

pub(crate) fn begin_authorship_v1(
    source: Option<&LivedStateSourceSnapshotV1>,
    model_routes: &[LivedStateModelRouteV1],
    artifact_kind: &str,
) -> LivedStateAuthorshipV1 {
    let authored_at = clock_sample_v1();
    let authored_process_sequence = next_witness_sequence();
    let startup = identity::snapshot();
    let mut hasher = Sha256::new();
    hasher.update(b"astrid-temporal-lived-state-witness-v1\0");
    hasher.update(startup.process.runtime_instance_id().as_bytes());
    hasher.update(authored_at.unix_ms.to_le_bytes());
    hasher.update(authored_at.monotonic_ns.to_le_bytes());
    hasher.update(artifact_kind.as_bytes());
    if let Some(source) = source {
        hasher.update(source.window_sha256().as_bytes());
    }
    for route in model_routes {
        hasher.update(route.call_id().as_bytes());
    }
    LivedStateAuthorshipV1::finish_witness_id(hasher, authored_at, authored_process_sequence)
}

impl LivedStateAuthorshipV1 {
    fn finish_witness_id(
        hasher: Sha256,
        authored_at: LivedStateClockSampleV1,
        authored_process_sequence: u64,
    ) -> LivedStateAuthorshipV1 {
        LivedStateAuthorshipV1 {
            witness_id: format!("lsw_{:x}", hasher.finalize()),
            authored_at,
            authored_process_sequence,
        }
    }

    pub(crate) fn witness_id(&self) -> &str {
        &self.witness_id
    }
}

fn parameter(
    name: &str,
    value: Option<f64>,
    unit: &str,
    kind: LivedStateObservationKindV1,
    observed_at_unix_ms: u64,
    age_ms: Option<u64>,
    fresh: Option<bool>,
    source_ref: &str,
) -> LivedStateParameterObservationV1 {
    let value_relation = match (kind, value, fresh) {
        (LivedStateObservationKindV1::PeerObserved, Some(_), Some(true)) => {
            "fresh_peer_scalar_observed"
        }
        (LivedStateObservationKindV1::Unknown, None, Some(false)) => {
            "peer_scalar_withheld_as_stale_temporal_context_only"
        }
        (LivedStateObservationKindV1::Unknown, None, _) => {
            "source_unavailable_or_value_unobserved"
        }
        (LivedStateObservationKindV1::CompiledConstant, Some(_), _) => {
            "compiled_value_observed_in_running_binary"
        }
        (LivedStateObservationKindV1::RuntimeObserved, Some(_), _) => {
            "runtime_scalar_observed"
        }
        (LivedStateObservationKindV1::SourceDeclared, Some(_), _) => {
            "source_declared_not_runtime_activation_proof"
        }
        _ => "bounded_observation_without_stronger_relation",
    };
    parameter_with_relation(
        name,
        value,
        unit,
        kind,
        observed_at_unix_ms,
        age_ms,
        fresh,
        source_ref,
        value_relation,
    )
}

#[allow(clippy::too_many_arguments)]
fn parameter_with_relation(
    name: &str,
    value: Option<f64>,
    unit: &str,
    kind: LivedStateObservationKindV1,
    observed_at_unix_ms: u64,
    age_ms: Option<u64>,
    fresh: Option<bool>,
    source_ref: &str,
    value_relation: &'static str,
) -> LivedStateParameterObservationV1 {
    LivedStateParameterObservationV1::new(
        name.to_string(),
        value.filter(|value| value.is_finite()),
        unit.to_string(),
        kind,
        observed_at_unix_ms,
        age_ms,
        fresh,
        source_ref.to_string(),
        value_relation,
    )
}

fn peer_scalar_observations_from_snapshot(
    value: Option<&Value>,
    age_ms: Option<u64>,
    now_ms: u64,
    status: PeerEvidenceSourceStatusV1,
) -> Vec<LivedStateParameterObservationV1> {
    let fresh = age_ms.is_some_and(|age| age <= MAX_PEER_STATE_AGE_MS);
    let fields = [
        ("minime.fill_pct", "fill_pct", "percent"),
        ("minime.spectral_entropy", "spectral_entropy", "ratio"),
        ("minime.structural_entropy", "structural_entropy", "ratio"),
    ];
    fields
        .into_iter()
        .map(|(name, field, unit)| {
            let observed_scalar = value.and_then(|value| value.get(field)).and_then(Value::as_f64);
            let scalar = fresh.then_some(observed_scalar).flatten();
            let value_relation = if scalar.is_some() {
                "fresh_peer_scalar_observed"
            } else if age_ms.is_some_and(|age| age > MAX_PEER_STATE_AGE_MS) {
                "peer_scalar_withheld_as_stale_temporal_context_only"
            } else if observed_scalar.is_some() && age_ms.is_none() {
                "source_timestamp_unavailable_scalar_withheld"
            } else {
                match status {
                    PeerEvidenceSourceStatusV1::Observed => "source_value_missing_or_non_scalar",
                    PeerEvidenceSourceStatusV1::FileMissing => "source_file_missing",
                    PeerEvidenceSourceStatusV1::FileUnreadable => "source_file_unreadable",
                    PeerEvidenceSourceStatusV1::JsonMalformed => "source_json_malformed",
                }
            };
            parameter_with_relation(
                name,
                scalar,
                unit,
                if scalar.is_some() {
                    LivedStateObservationKindV1::PeerObserved
                } else {
                    LivedStateObservationKindV1::Unknown
                },
                now_ms,
                age_ms,
                Some(fresh),
                "minime_workspace/spectral_state.json",
                value_relation,
            )
        })
        .collect()
}

fn peer_scalar_observations(now_ms: u64) -> Vec<LivedStateParameterObservationV1> {
    let snapshot = peer_evidence_cache::snapshot();
    let age_ms = snapshot
        .file_modified_unix_ms
        .map(|modified| now_ms.saturating_sub(modified));
    peer_scalar_observations_from_snapshot(
        snapshot.value.as_ref(),
        age_ms,
        now_ms,
        snapshot.status,
    )
}

pub(crate) fn runtime_context_v1(state: &BridgeState, fill_pct: f32) -> LivedStateRuntimeContextV1 {
    let now = clock_sample_v1();
    let (heartbeat_interval_s, heartbeat_intensity) =
        crate::autonomous::semantic_heartbeat_constants_v1();
    let telemetry_age_ms = state.latest_telemetry_arrival_unix_s.map(|arrival| {
        let arrival_ms = if arrival.is_sign_positive() {
            (arrival * 1_000.0) as u64
        } else {
            0
        };
        now.unix_ms.saturating_sub(arrival_ms)
    });
    let telemetry_fresh = telemetry_age_ms.map(|age| age <= MAX_PEER_STATE_AGE_MS);
    let spectral_entropy = state
        .latest_telemetry
        .as_ref()
        .and_then(|telemetry| telemetry.spectral_fingerprint_v1.as_ref())
        .map(|fingerprint| f64::from(fingerprint.spectral_entropy));
    let mut observations = vec![
        parameter(
            "bridge.fill_pct",
            Some(f64::from(fill_pct)),
            "percent",
            LivedStateObservationKindV1::RuntimeObserved,
            now.unix_ms,
            telemetry_age_ms,
            telemetry_fresh,
            "bridge_state.fill_pct",
        ),
        parameter(
            "bridge.spectral_entropy",
            spectral_entropy,
            "ratio",
            if spectral_entropy.is_some() {
                LivedStateObservationKindV1::RuntimeObserved
            } else {
                LivedStateObservationKindV1::Unknown
            },
            now.unix_ms,
            telemetry_age_ms,
            telemetry_fresh,
            "bridge_state.latest_telemetry.spectral_fingerprint_v1.spectral_entropy",
        ),
        parameter(
            "bridge.semantic_dimensions",
            Some(crate::codec::SEMANTIC_DIM as f64),
            "dimensions",
            LivedStateObservationKindV1::CompiledConstant,
            now.unix_ms,
            None,
            None,
            "codec::SEMANTIC_DIM",
        ),
        parameter(
            "bridge.semantic_heartbeat_interval",
            Some(heartbeat_interval_s as f64),
            "seconds",
            LivedStateObservationKindV1::CompiledConstant,
            now.unix_ms,
            None,
            None,
            "autonomous::SEMANTIC_HEARTBEAT_INTERVAL",
        ),
        parameter(
            "bridge.semantic_heartbeat_intensity",
            Some(f64::from(heartbeat_intensity)),
            "ratio",
            LivedStateObservationKindV1::CompiledConstant,
            now.unix_ms,
            None,
            None,
            "autonomous::SEMANTIC_HEARTBEAT_INTENSITY",
        ),
    ];
    observations.extend(peer_scalar_observations(now.unix_ms));
    LivedStateRuntimeContextV1 {
        parameter_observations: observations,
        peer_process_identity: bounded_identity(
            state
                .sensory_delivery_protocol_v1
                .server_process_identity
                .as_deref(),
        ),
        peer_deployment_identity: bounded_identity(
            state
                .sensory_delivery_protocol_v1
                .server_deployment_identity
                .as_deref(),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn finalize_and_submit_v1(
    authorship: &LivedStateAuthorshipV1,
    artifact_kind: &str,
    artifact_relative_path: &str,
    artifact_bytes: &[u8],
    source_snapshot: Option<LivedStateSourceSnapshotV1>,
    model_routes: Vec<LivedStateModelRouteV1>,
    runtime_context: LivedStateRuntimeContextV1,
) -> WitnessSubmitResultV1 {
    let startup = identity::snapshot();
    let source_provenance = source_snapshot
        .as_ref()
        .map(LivedStateSourceSnapshotV1::provenance_ref_v1);
    let process_provenance = ProvenanceRefV1::new(
        ProvenanceOriginV1::BridgeDerived,
        format!("process:{}", startup.process.runtime_instance_id()),
        startup.process.process_identity_sha256().to_string(),
        Vec::new(),
        authorship.authored_at.unix_ms,
        vec!["observed_process_v1".to_string()],
        vec![ProvenanceInfluenceTypeV1::Temporal],
    );
    let witness = TemporalLivedStateWitnessV1::new(
        authorship.witness_id.clone(),
        artifact_kind.chars().take(80).collect(),
        artifact_relative_path.chars().take(400).collect(),
        sha256_bytes(artifact_bytes),
        authorship.authored_at.unix_ms,
        authorship.authored_at.monotonic_ns,
        authorship.authored_process_sequence,
        source_snapshot,
        startup.process,
        startup.build_candidate,
        model_routes,
        runtime_context.parameter_observations,
        runtime_context.peer_process_identity,
        runtime_context.peer_deployment_identity,
        source_provenance,
        process_provenance,
    );
    writer::try_submit(
        &bridge_paths()
            .introspections_dir()
            .join("lived_state_witnesses"),
        witness,
    )
}

pub(crate) fn model_route_v1(
    job_id: Option<String>,
    qos_request_identity_sha256: Option<String>,
    request_content_anchor_sha256: Option<String>,
    provider_route: &str,
    model_profile: &str,
    started_at_unix_ms: u64,
    completed_at_unix_ms: u64,
    queue_wait_ms: Option<u64>,
    active_generation_and_reservoir_ms: Option<u64>,
    repair_parent_call_id: Option<String>,
    response_text: &str,
) -> LivedStateModelRouteV1 {
    let job_id = bounded_identity(job_id.as_deref());
    let provider_route: String = provider_route.chars().take(40).collect();
    let model_profile =
        bounded_identity(Some(model_profile)).unwrap_or_else(|| "unknown".to_string());
    let response_sha256 = sha256_bytes(response_text.as_bytes());
    let mut hasher = Sha256::new();
    hasher.update(b"astrid-lived-state-model-route-v1\0");
    if let Some(job_id) = job_id.as_deref() {
        hasher.update(job_id.as_bytes());
    }
    if let Some(qos) = qos_request_identity_sha256.as_deref() {
        hasher.update(qos.as_bytes());
    }
    hasher.update(provider_route.as_bytes());
    hasher.update(model_profile.as_bytes());
    hasher.update(started_at_unix_ms.to_le_bytes());
    hasher.update(response_sha256.as_bytes());
    let call_id = format!("lscall_{:x}", hasher.finalize());
    LivedStateModelRouteV1::observed(
        call_id,
        job_id,
        qos_request_identity_sha256,
        request_content_anchor_sha256,
        provider_route,
        model_profile,
        started_at_unix_ms,
        completed_at_unix_ms,
        completed_at_unix_ms.saturating_sub(started_at_unix_ms),
        queue_wait_ms,
        active_generation_and_reservoir_ms,
        repair_parent_call_id,
        response_sha256,
    )
}

#[cfg(test)]
mod tests;
