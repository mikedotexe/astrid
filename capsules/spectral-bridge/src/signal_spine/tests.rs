use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

use serde_json::json;

use super::capture::{CaptureWindowRequestV1, PendingVectorCaptureV1, try_submit_captures};
use super::recorder::{ShadowSignalJourneyV1, SignalJourneyContextV1};
use super::types::{SignalEffectV1, SignalOwnershipDomainV1, SignalRelationV1, SignalStageKindV1};

fn context() -> SignalJourneyContextV1 {
    SignalJourneyContextV1 {
        exchange: 7,
        source_time_ms: Some(123),
        connection_id: "minime_sensory_7879",
        connection_sequence: 7001,
        deployment_identity: "test".to_string(),
    }
}

fn capture_window(now: u64, id: &str, expires_at: u64, journey_limit: u32) -> serde_json::Value {
    json!({
        "schema": "signal_spine_capture_window_v1",
        "schema_version": 1,
        "capture_window_id": id,
        "started_at_unix_ms": now.saturating_sub(1),
        "expires_at_unix_ms": expires_at,
        "journey_limit": journey_limit,
        "actor": "test",
        "acknowledgement": "bounded test capture",
        "full_vector_dimensions": 48,
        "raw_response_prose_included": false,
        "capture_can_delay_dispatch": false,
        "witness_only": true,
        "artifact_authority_state_v1": {
            "schema": "artifact_authority_state_v1",
            "schema_version": 1,
            "state": "evidence_only",
            "live_eligible_now": false,
            "auto_approved": false,
            "grants_approval": false,
            "edits_source_now": false,
        },
    })
}

#[test]
fn parent_chain_is_exact_and_forward_references_are_refused() {
    let (mut journey, authored) =
        ShadowSignalJourneyV1::begin_authored(context(), "bounded thought").unwrap();
    let chunk = journey
        .record_text(
            SignalStageKindV1::Chunked,
            SignalRelationV1::ExactTransformation,
            SignalEffectV1::Produced,
            SignalOwnershipDomainV1::BridgeCodec,
            &[&authored],
            "bounded",
            BTreeMap::new(),
        )
        .unwrap();
    let vector = vec![0.25_f32; 48];
    journey
        .record_vector(
            SignalStageKindV1::Encoded,
            SignalEffectV1::Produced,
            SignalOwnershipDomainV1::BridgeCodec,
            &chunk,
            &vector,
            BTreeMap::new(),
        )
        .unwrap();
    let encoded_stage = journey
        .trusted_receipts_for_test()
        .last()
        .unwrap()
        .stage_id()
        .to_string();
    journey.attach_capture_fixture_for_test(&encoded_stage);
    assert!(journey.parent_chain_valid());
    assert!(
        journey
            .trusted_receipts_for_test()
            .iter()
            .all(super::types::SignalStageReceiptV1::integrity_valid)
    );
}

#[test]
fn stage_hashes_are_deterministic_for_equal_inputs_and_context() {
    let (mut first, first_root) = ShadowSignalJourneyV1::begin_authored(context(), "same").unwrap();
    let (mut second, second_root) =
        ShadowSignalJourneyV1::begin_authored(context(), "same").unwrap();
    let vector = vec![0.5_f32; 48];
    let first_stage = first
        .record_vector(
            SignalStageKindV1::Encoded,
            SignalEffectV1::Produced,
            SignalOwnershipDomainV1::BridgeCodec,
            &first_root,
            &vector,
            BTreeMap::new(),
        )
        .unwrap();
    let second_stage = second
        .record_vector(
            SignalStageKindV1::Encoded,
            SignalEffectV1::Produced,
            SignalOwnershipDomainV1::BridgeCodec,
            &second_root,
            &vector,
            BTreeMap::new(),
        )
        .unwrap();
    assert_eq!(first_stage.output_sha256(), second_stage.output_sha256());
}

#[test]
fn floating_measurements_are_persisted_as_canonical_decimal_text() {
    let (mut journey, root) = ShadowSignalJourneyV1::begin_authored(context(), "same").unwrap();
    journey
        .record_text(
            SignalStageKindV1::Chunked,
            SignalRelationV1::ExactTransformation,
            SignalEffectV1::Produced,
            SignalOwnershipDomainV1::BridgeCodec,
            &[&root],
            "same",
            BTreeMap::from([("nested".to_string(), json!({"float": 0.125, "integer": 7}))]),
        )
        .unwrap();
    let value = serde_json::to_value(journey.trusted_receipts_for_test()).unwrap();
    assert_eq!(
        value.pointer("/1/measurements/nested/float"),
        Some(&json!("0.125"))
    );
    assert_eq!(
        value.pointer("/1/measurements/nested/integer"),
        Some(&json!(7))
    );
}

#[test]
fn temporal_association_is_labeled_as_noncausal() {
    assert_eq!(
        SignalRelationV1::TemporalAssociation.as_str(),
        "temporal_association_not_direct_causation"
    );
}

#[test]
fn capture_window_enforces_hard_limits_and_owner_only_fixture_permissions() {
    let temp = tempfile::tempdir().unwrap();
    let now = 1_000_000_u64;
    fs::write(
        temp.path().join("capture_window.json"),
        serde_json::to_vec(&capture_window(now, "capture_test", now + 30_000, 32)).unwrap(),
    )
    .unwrap();
    assert!(CaptureWindowRequestV1::load(temp.path(), now).is_some());
    let vector = vec![0.25_f32; 48];
    let result = try_submit_captures(
        temp.path(),
        "journey_test",
        vec![PendingVectorCaptureV1::new(
            "stage_test".to_string(),
            &vector,
        )],
        now,
    );
    let super::capture::CaptureSubmitResultV1::Accepted(references) = result else {
        panic!("capture should be accepted");
    };
    assert_eq!(
        references[0].2,
        format!("captures/capture_test/fixtures/{}.json", references[0].1)
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let fixture_dir = temp.path().join("captures/capture_test/fixtures");
    while !fixture_dir.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let fixture = fs::read_dir(fixture_dir)
        .unwrap()
        .filter_map(Result::ok)
        .next()
        .unwrap()
        .path();
    assert_eq!(
        fs::metadata(fixture).unwrap().permissions().mode() & 0o777,
        0o600
    );

    fs::write(
        temp.path().join("capture_window.json"),
        serde_json::to_vec(&capture_window(
            now.saturating_add(1),
            "too_long",
            now + 2 * 60 * 60 * 1_000 + 1,
            257,
        ))
        .unwrap(),
    )
    .unwrap();
    assert!(CaptureWindowRequestV1::load(temp.path(), now).is_none());
}

#[test]
fn capture_window_journey_limit_includes_pending_async_writes() {
    let temp = tempfile::tempdir().unwrap();
    let now = 2_000_000_u64;
    fs::write(
        temp.path().join("capture_window.json"),
        serde_json::to_vec(&capture_window(now, "capture_limit_test", now + 30_000, 1)).unwrap(),
    )
    .unwrap();
    let first = try_submit_captures(
        temp.path(),
        "journey_first",
        vec![PendingVectorCaptureV1::new(
            "stage_first".to_string(),
            &[0.25_f32; 48],
        )],
        now,
    );
    assert!(matches!(
        first,
        super::capture::CaptureSubmitResultV1::Accepted(_)
    ));
    let second = try_submit_captures(
        temp.path(),
        "journey_second",
        vec![PendingVectorCaptureV1::new(
            "stage_second".to_string(),
            &[0.25_f32; 48],
        )],
        now,
    );
    assert!(matches!(
        second,
        super::capture::CaptureSubmitResultV1::JourneyLimitReached
            | super::capture::CaptureSubmitResultV1::WindowUnavailable
    ));
}

#[test]
fn armed_capture_reports_dimension_and_expiry_gaps() {
    let temp = tempfile::tempdir().unwrap();
    let now = 1_000_000_u64;
    fs::write(
        temp.path().join("capture_window.json"),
        serde_json::to_vec(&capture_window(now, "capture_test", now + 1, 32)).unwrap(),
    )
    .unwrap();
    let invalid = try_submit_captures(
        temp.path(),
        "journey_invalid",
        vec![PendingVectorCaptureV1::new(
            "stage_invalid".to_string(),
            &[0.25_f32; 47],
        )],
        now,
    );
    assert!(matches!(
        invalid,
        super::capture::CaptureSubmitResultV1::InvalidVectorDimensions
    ));
    let expired = try_submit_captures(
        temp.path(),
        "journey_expired",
        vec![PendingVectorCaptureV1::new(
            "stage_expired".to_string(),
            &[0.25_f32; 48],
        )],
        now + 1,
    );
    assert!(matches!(
        expired,
        super::capture::CaptureSubmitResultV1::WindowUnavailable
    ));
}

#[test]
fn capture_window_rejects_disk_artifacts_that_claim_live_authority() {
    let temp = tempfile::tempdir().unwrap();
    let now = 3_000_000_u64;
    let mut request = capture_window(now, "capture_forbidden", now + 30_000, 1);
    request["artifact_authority_state_v1"]["live_eligible_now"] = json!(true);
    fs::write(
        temp.path().join("capture_window.json"),
        serde_json::to_vec(&request).unwrap(),
    )
    .unwrap();
    assert!(CaptureWindowRequestV1::load(temp.path(), now).is_none());
}

#[test]
fn no_capture_shadow_instrumentation_stays_below_one_millisecond_p95() {
    let mut samples = Vec::with_capacity(400);
    let vector = vec![0.125_f32; 48];
    for exchange in 0..400 {
        let started = Instant::now();
        let mut local_context = context();
        local_context.exchange = exchange;
        local_context.connection_sequence = exchange;
        let (mut journey, root) =
            ShadowSignalJourneyV1::begin_authored(local_context, "benchmark").unwrap();
        let _ = journey
            .record_vector(
                SignalStageKindV1::Encoded,
                SignalEffectV1::Produced,
                SignalOwnershipDomainV1::BridgeCodec,
                &root,
                &vector,
                BTreeMap::new(),
            )
            .unwrap();
        samples.push(started.elapsed());
    }
    samples.sort_unstable();
    let p95 = samples[samples.len() * 95 / 100];
    assert!(p95 < Duration::from_millis(1), "p95 was {p95:?}");
}
