use super::*;
use crate::learning_clock::LearningObservation;

#[test]
fn production_preparation_uses_the_local_published_target_and_retires_on_change() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("health.json");
    let write_health = |target: f32| {
        std::fs::write(&path, serde_json::json!({"t_s":100.0,
            "stable_core":{"enabled":true,"structural_pi":{"active":true,"target_fill_pct":target}}}).to_string()).unwrap();
    };
    write_health(68.0);
    let db = BridgeDb::open(":memory:").unwrap();
    let mut conv = ConversationState::new(vec![], Some(dir.path().to_path_buf()));
    let raw = LearningObservation {
        scope: 1,
        producer_t_ms: 100_000,
        received_at: std::time::Instant::now(),
        target: None,
    };
    let baseline = prepare_hebbian_feedback(&mut conv, Some(raw), 70.0, &db).unwrap();
    assert_eq!(baseline.target.unwrap().fill_pct, 68.0);
    finalize_semantic_exchange(
        &mut conv,
        Some(vec![0.8; 48]),
        70.0,
        Some(baseline),
        true,
        &db,
    );
    write_health(63.0);
    let later = LearningObservation {
        producer_t_ms: 101_000,
        received_at: std::time::Instant::now(),
        ..raw
    };
    prepare_hebbian_feedback(&mut conv, Some(later), 69.0, &db);
    assert!(conv.hebbian_outcomes.pending.is_empty());
    let rows = db
        .query_messages(0.0, f64::MAX, Some(LEARNING_RETIREMENT_TOPIC), 10)
        .unwrap();
    let payload: serde_json::Value = serde_json::from_str(&rows[0].payload).unwrap();
    assert_eq!(payload["outcomes"][0]["reason"], "learning_target_changed");
    assert_eq!(payload["outcomes"][0]["baseline_target"]["fill_pct"], 68.0);
    assert_eq!(payload["outcomes"][0]["observed_target"]["fill_pct"], 63.0);
}

fn legacy_queue() -> learning_outcomes::HebbianOutcomeQueue {
    learning_outcomes::HebbianOutcomeQueue::from_checkpoint(
        serde_json::from_value(serde_json::json!([{
            "exchange_count":189539,"signature":[0.2],"fill_before":71.0,
            "telemetry_t_ms_before":385872737
        }]))
        .unwrap(),
        Some(1_075_024_066),
    )
}

#[test]
fn fresh_outcomes_resume_existing_scoring_without_resetting_preferences() {
    let db = BridgeDb::open(":memory:").unwrap();
    let mut conv = ConversationState::new(vec![], None);
    conv.hebbian_codec.set_learning_rate_scale(0.7);
    conv.codec_weights.insert("warmth".into(), 1.05);
    conv.hebbian_outcomes = legacy_queue();
    let mut expected = conv.hebbian_codec.clone();
    let now = std::time::Instant::now();
    let baseline = LearningObservation {
        scope: rand::random::<u128>(),
        producer_t_ms: 529_076_440,
        received_at: now - std::time::Duration::from_secs(2),
        target: Some(crate::learning_target::LearningTarget {
            fill_pct: 68.0,
            source: "test",
        }),
    };
    update_hebbian_feedback(&mut conv, Some(baseline), 71.0, &db);
    expected.decay_scores();
    assert_eq!(
        serde_json::to_value(&conv.hebbian_codec).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    let signature = vec![0.8; 48];
    finalize_semantic_exchange(
        &mut conv,
        Some(signature.clone()),
        71.0,
        Some(baseline),
        true,
        &db,
    );
    let later = LearningObservation {
        producer_t_ms: 529_078_800,
        received_at: now,
        ..baseline
    };
    update_hebbian_feedback(&mut conv, Some(later), 73.0, &db);
    expected.decay_scores();
    assert!(expected.observe_outcome(&signature, 71.0, 73.0, 68.0));
    assert_eq!(
        serde_json::to_value(&conv.hebbian_codec).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    assert_eq!(conv.codec_weights.get("warmth"), Some(&1.05));
    assert_eq!(conv.hebbian_codec.learning_rate_scale(), 0.7);
    assert!(conv.hebbian_outcomes.pending.is_empty());
}

#[test]
fn retirement_is_durable_and_does_not_claim_causal_success() {
    let db = BridgeDb::open(":memory:").unwrap();
    let mut conv = ConversationState::new(vec![], None);
    conv.hebbian_outcomes = legacy_queue();
    update_hebbian_feedback(&mut conv, None, 74.0, &db);
    assert!(conv.hebbian_outcomes.pending.is_empty());
    let rows = db
        .query_messages(0.0, f64::MAX, Some(LEARNING_RETIREMENT_TOPIC), 20)
        .unwrap();
    assert_eq!(rows.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&rows[0].payload).unwrap();
    assert_eq!(payload["retired_count"], 1);
    assert_eq!(payload["outcomes"][0]["exchange_count"], 189539);
    assert_eq!(
        payload["outcomes"][0]["reason"],
        "restored_without_observation_clock"
    );
    assert_eq!(payload["live_control_authority"], false);
    update_hebbian_feedback(&mut conv, None, 74.0, &db);
    assert_eq!(
        db.query_messages(0.0, f64::MAX, Some(LEARNING_RETIREMENT_TOPIC), 20)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn evidence_write_failure_keeps_queue_and_watermark_for_retry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bridge.db");
    drop(BridgeDb::open(&path).unwrap());
    let db = BridgeDb::open_read_only(&path).unwrap();
    let mut conv = ConversationState::new(vec![], None);
    conv.hebbian_outcomes = legacy_queue();
    let before = conv.hebbian_outcomes.pending.clone();
    let observation = LearningObservation {
        scope: rand::random::<u128>(),
        producer_t_ms: 529_076_440,
        received_at: std::time::Instant::now(),
        target: Some(crate::learning_target::LearningTarget {
            fill_pct: 68.0,
            source: "test",
        }),
    };
    update_hebbian_feedback(&mut conv, Some(observation), 74.0, &db);
    assert_eq!(conv.hebbian_outcomes.pending, before);
    assert_eq!(
        conv.hebbian_outcomes.last_consumed_t_ms,
        Some(1_075_024_066)
    );
    finalize_semantic_exchange(
        &mut conv,
        Some(vec![0.3]),
        71.0,
        Some(observation),
        true,
        &db,
    );
    assert_eq!(conv.hebbian_outcomes.pending, before);
    drop(db);
    let writable = BridgeDb::open(&path).unwrap();
    update_hebbian_feedback(&mut conv, Some(observation), 74.0, &writable);
    assert!(conv.hebbian_outcomes.pending.is_empty());
    assert_eq!(
        writable
            .query_messages(0.0, f64::MAX, Some(LEARNING_RETIREMENT_TOPIC), 20)
            .unwrap()
            .len(),
        1
    );
}
