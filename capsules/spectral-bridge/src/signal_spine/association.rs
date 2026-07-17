use std::collections::{BTreeMap, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use super::recorder::{ShadowSignalJourneyV1, SignalStageHandleV1, signal_deployment_identity_v1};
use super::types::{
    SignalEffectV1, SignalOwnershipDomainV1, SignalProcessIdentityV1, SignalRelationV1,
    SignalStageKindV1, SignalStageReceiptV1, SignalTemporalEnvelopeV1,
};
use crate::paths::bridge_paths;
use crate::witness::{
    MinimeObservationV1, ProvenanceInfluenceTypeV1, ProvenanceOriginV1, ProvenanceRefV1,
};

const TEMPORAL_WINDOW_MS: u64 = 30_000;
const MAX_PENDING_WINDOWS: usize = 256;

#[derive(Debug, Clone)]
struct PendingTemporalWindowV1 {
    journey_id: String,
    dispatched_stage_id: String,
    dispatched_output_sha256: String,
    opened_at_unix_ms: u64,
    expires_at_unix_ms: u64,
}

fn pending_windows() -> &'static Mutex<VecDeque<PendingTemporalWindowV1>> {
    static PENDING: OnceLock<Mutex<VecDeque<PendingTemporalWindowV1>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(VecDeque::new()))
}

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

fn sha256_json<T: Serialize>(value: &T) -> String {
    serde_json::to_vec(value).map_or_else(
        |_| format!("{:x}", Sha256::digest(b"serialization_failed")),
        |bytes| format!("{:x}", Sha256::digest(bytes)),
    )
}

pub(crate) fn register_delivery_temporal_window_v1(
    journey: &ShadowSignalJourneyV1,
    dispatched_stage: &SignalStageHandleV1,
) {
    let now = unix_ms();
    let pending = PendingTemporalWindowV1 {
        journey_id: journey.journey_id().to_string(),
        dispatched_stage_id: dispatched_stage.stage_id().to_string(),
        dispatched_output_sha256: dispatched_stage.output_sha256().to_string(),
        opened_at_unix_ms: now,
        expires_at_unix_ms: now.saturating_add(TEMPORAL_WINDOW_MS),
    };
    if let Ok(mut windows) = pending_windows().lock() {
        if windows.len() >= MAX_PENDING_WINDOWS {
            windows.pop_front();
        }
        windows.push_back(pending);
    }
}

pub(crate) fn record_minime_temporal_associations_v1(
    observation: &MinimeObservationV1,
    observed_at_unix_s: f64,
) {
    let observed_at_unix_ms = if observed_at_unix_s.is_finite() && observed_at_unix_s >= 0.0 {
        u64::try_from(std::time::Duration::from_secs_f64(observed_at_unix_s).as_millis())
            .unwrap_or_else(|_| unix_ms())
    } else {
        unix_ms()
    };
    let pending = {
        let Ok(mut windows) = pending_windows().lock() else {
            return;
        };
        while windows
            .front()
            .is_some_and(|window| window.expires_at_unix_ms < observed_at_unix_ms)
        {
            windows.pop_front();
        }
        let mut ready = Vec::new();
        while windows
            .front()
            .is_some_and(|window| window.opened_at_unix_ms <= observed_at_unix_ms)
        {
            if let Some(window) = windows.pop_front() {
                ready.push(window);
            }
        }
        ready
    };
    for window in pending {
        if let Err(error) = persist_temporal_association(
            &window,
            observation,
            observed_at_unix_ms,
            default_signal_spine_root().as_path(),
        ) {
            tracing::warn!(
                error = %error,
                journey_id = %window.journey_id,
                "signal spine temporal association persistence failed"
            );
        }
    }
}

fn persist_temporal_association(
    window: &PendingTemporalWindowV1,
    observation: &MinimeObservationV1,
    observed_at_unix_ms: u64,
    root: &Path,
) -> std::io::Result<()> {
    let observation_provenance = observation.provenance();
    let digest = sha256_json(&json!({
        "journey_id": window.journey_id,
        "dispatched_stage_id": window.dispatched_stage_id,
        "minime_observation_id": observation_provenance.source_id(),
        "relation": "temporal_association_not_direct_causation",
        "observed_at_unix_ms": observed_at_unix_ms,
    }));
    let stage_id = format!("stage_{}", &digest[..24]);
    let provenance = ProvenanceRefV1::new(
        ProvenanceOriginV1::MinimeObservation,
        stage_id.clone(),
        observation_provenance.canonical_sha256().to_string(),
        vec![
            window.dispatched_stage_id.clone(),
            observation_provenance.source_id().to_string(),
        ],
        observed_at_unix_ms,
        vec!["signal_spine.minime_telemetry_window".to_string()],
        vec![
            ProvenanceInfluenceTypeV1::RegulatoryStateObserved,
            ProvenanceInfluenceTypeV1::Temporal,
        ],
    );
    let measurements = BTreeMap::from([
        (
            "window_latency_ms".to_string(),
            json!(observed_at_unix_ms.saturating_sub(window.opened_at_unix_ms)),
        ),
        ("direct_causation_claimed".to_string(), json!(false)),
        ("wire_acknowledgement_available".to_string(), json!(false)),
        ("journey_id_on_wire".to_string(), json!(false)),
    ]);
    let receipt = SignalStageReceiptV1::new(
        window.journey_id.clone(),
        stage_id,
        u32::MAX,
        SignalStageKindV1::MinimeTelemetryWindow,
        SignalRelationV1::TemporalAssociation,
        SignalEffectV1::TemporallyAssociated,
        SignalOwnershipDomainV1::MinimeObserved,
        vec![window.dispatched_stage_id.clone()],
        window.dispatched_output_sha256.clone(),
        observation_provenance.canonical_sha256().to_string(),
        provenance,
        SignalProcessIdentityV1::current(signal_deployment_identity_v1()),
        SignalTemporalEnvelopeV1::new(
            Some(observation.packet().t_ms),
            observed_at_unix_ms,
            unix_ms(),
            monotonic_ns(),
            "minime_telemetry_7878".to_string(),
            observation.packet().t_ms,
            None,
        ),
        measurements,
        None,
    )
    .map_err(std::io::Error::other)?;
    let payload = serde_json::to_vec(&json!({
        "schema": "signal_temporal_association_v1",
        "schema_version": 1,
        "journey_id": window.journey_id,
        "relation": "temporal_association_not_direct_causation",
        "direct_causation_claimed": false,
        "wire_acknowledgement_available": false,
        "raw_response_prose_included": false,
        "live_control_authority": false,
        "artifact_authority_state_v1": {
            "schema": "artifact_authority_state_v1",
            "schema_version": 1,
            "state": "evidence_only",
            "live_eligible_now": false,
            "auto_approved": false,
            "grants_approval": false,
            "edits_source_now": false,
        },
        "receipt": receipt,
    }))
    .map_err(std::io::Error::other)?;
    write_owner_only(
        &root.join("temporal_associations").join(format!(
            "{}_{}.json",
            window.journey_id, window.dispatched_stage_id
        )),
        &payload,
    )
}

fn default_signal_spine_root() -> std::path::PathBuf {
    bridge_paths()
        .bridge_workspace()
        .join("diagnostics/signal_spine_v1")
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

#[cfg(test)]
mod tests {
    use astrid_minime_protocol::EigenPacketV1;
    use serde_json::Value;

    use super::*;
    use crate::witness::WireReceiptV1;

    #[test]
    fn later_telemetry_receipt_is_explicitly_associative_not_causal() {
        let temp = tempfile::tempdir().unwrap();
        let packet = EigenPacketV1 {
            t_ms: 42,
            eigenvalues: vec![0.7, 0.2],
            fill_ratio: 0.68,
            ..EigenPacketV1::default()
        };
        let packet_sha = sha256_json(&packet);
        let provenance = ProvenanceRefV1::new(
            ProvenanceOriginV1::MinimeObservation,
            "observation_test".to_string(),
            packet_sha.clone(),
            Vec::new(),
            42,
            vec!["packet".to_string()],
            vec![ProvenanceInfluenceTypeV1::RegulatoryStateObserved],
        );
        let observation = MinimeObservationV1::new(
            packet,
            provenance,
            WireReceiptV1::new(
                10,
                packet_sha.clone(),
                packet_sha,
                "legacy_accepted".to_string(),
            ),
        );
        let pending = PendingTemporalWindowV1 {
            journey_id: "journey_test".to_string(),
            dispatched_stage_id: "stage_dispatch".to_string(),
            dispatched_output_sha256: "a".repeat(64),
            opened_at_unix_ms: 1_000,
            expires_at_unix_ms: 31_000,
        };
        persist_temporal_association(&pending, &observation, 1_100, temp.path()).unwrap();
        let path = temp
            .path()
            .join("temporal_associations/journey_test_stage_dispatch.json");
        let value: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(
            value["relation"],
            "temporal_association_not_direct_causation"
        );
        assert_eq!(value["direct_causation_claimed"], false);
        assert_eq!(value["wire_acknowledgement_available"], false);
        assert_eq!(value["receipt"]["parent_stage_ids"][0], "stage_dispatch");
    }
}
