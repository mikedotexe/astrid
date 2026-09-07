use super::*;
use crate::learning_clock::LearningClock;
use std::time::Duration;

fn sample(t_ms: u64, at: Instant, scope: u128) -> LearningObservation {
    LearningObservation {
        scope,
        producer_t_ms: t_ms,
        received_at: at,
        target: Some(crate::learning_target::LearningTarget {
            fill_pct: 68.0,
            source: "test",
        }),
    }
}

fn snapshot(clock: &LearningClock, connection: u64, tick: u64) -> Option<LearningObservation> {
    clock
        .snapshot(true, Some(connection), tick)
        .map(|observed| sample(observed.producer_t_ms, observed.received_at, observed.scope))
}

fn arm(
    queue: &mut HebbianOutcomeQueue,
    exchange: u64,
    baseline: LearningObservation,
) -> Vec<OutcomeRetirement> {
    queue.arm(
        exchange,
        vec![0.2],
        71.0,
        Some(baseline),
        baseline.received_at,
    )
}

#[test]
fn fresh_learning_baseline_is_not_blocked_by_previous_process_watermark() {
    let now = Instant::now();
    let baseline = sample(529_076_440, now, rand::random::<u128>());
    let mut queue = HebbianOutcomeQueue::from_checkpoint(VecDeque::new(), Some(1_075_024_066));
    assert!(arm(&mut queue, 191_300, baseline).is_empty());
    let next = sample(529_078_800, now + Duration::from_secs(2), baseline.scope);
    assert_eq!(
        queue
            .take(Some(next), next.received_at)
            .ready
            .unwrap()
            .exchange_count,
        191_300
    );
}

#[test]
fn captured_legacy_backlog_is_retired_not_taught_after_reset() {
    let pending: VecDeque<PendingHebbianOutcome> = serde_json::from_value(serde_json::json!([
        {"exchange_count":189539,"signature":[0.2],"fill_before":71.0,"telemetry_t_ms_before":385872737},
        {"exchange_count":189541,"signature":[0.2],"fill_before":71.0,"telemetry_t_ms_before":386051893},
        {"exchange_count":189546,"signature":[0.2],"fill_before":71.0,"telemetry_t_ms_before":386515462},
        {"exchange_count":190494,"signature":[0.2],"fill_before":71.0,"telemetry_t_ms_before":463452539}
    ])).unwrap();
    let mut queue = HebbianOutcomeQueue::from_checkpoint(pending, Some(1_075_024_066));
    let now = Instant::now();
    let next = sample(529_076_440, now, rand::random::<u128>());
    let outcome = queue.take(Some(next), now);
    assert!(outcome.ready.is_none());
    assert_eq!(outcome.retired.len(), 4);
    assert!(
        outcome
            .retired
            .iter()
            .all(|item| item.reason == "restored_without_observation_clock")
    );
    assert!(queue.pending.is_empty());
    assert!(queue.last_consumed_t_ms.is_none());
    arm(&mut queue, 191_300, next);
    let later = sample(529_078_800, now + Duration::from_secs(2), next.scope);
    assert_eq!(
        queue
            .take(Some(later), later.received_at)
            .ready
            .unwrap()
            .exchange_count,
        191_300
    );
}

#[test]
fn pending_hebbian_outcomes_are_fifo_and_one_per_telemetry_tick() {
    let mut queue = HebbianOutcomeQueue::default();
    let now = Instant::now();
    let scope = rand::random::<u128>();
    arm(&mut queue, 3, sample(100, now, scope));
    arm(
        &mut queue,
        4,
        sample(101, now + Duration::from_secs(1), scope),
    );
    let next = sample(102, now + Duration::from_secs(2), scope);
    assert_eq!(
        queue
            .take(Some(next), next.received_at)
            .ready
            .unwrap()
            .exchange_count,
        3
    );
    assert!(queue.take(Some(next), next.received_at).ready.is_none());
    let later = sample(103, now + Duration::from_secs(3), scope);
    assert_eq!(
        queue
            .take(Some(later), later.received_at)
            .ready
            .unwrap()
            .exchange_count,
        4
    );
    assert!(queue.pending.is_empty());
}

#[test]
fn pending_hebbian_outcomes_require_newer_telemetry_and_arrival() {
    let mut queue = HebbianOutcomeQueue::default();
    let now = Instant::now();
    let baseline = sample(200, now, rand::random::<u128>());
    arm(&mut queue, 1, baseline);
    for next in [
        baseline,
        sample(199, now + Duration::from_secs(1), baseline.scope),
        sample(201, now, baseline.scope),
    ] {
        assert!(queue.take(Some(next), next.received_at).ready.is_none());
    }
    let next = sample(201, now + Duration::from_secs(1), baseline.scope);
    assert!(queue.take(Some(next), next.received_at).ready.is_some());
}

#[test]
fn pending_hebbian_outcomes_drop_oldest_when_fifo_is_full_with_reason() {
    let mut queue = HebbianOutcomeQueue::default();
    let now = Instant::now();
    let scope = rand::random::<u128>();
    for ix in 0..4 {
        assert!(arm(&mut queue, ix, sample(ix, now, scope)).is_empty());
    }
    let retired = arm(&mut queue, 4, sample(4, now, scope));
    assert_eq!(retired.len(), 1);
    assert_eq!(retired[0].reason, "fifo_capacity");
    assert_eq!(retired[0].exchange_count, 0);
    assert_eq!(queue.pending.len(), 4);
    assert_eq!(queue.pending.front().unwrap().exchange_count, 1);
}

#[test]
fn bridge_restart_does_not_rehydrate_an_instant_or_pair_old_outcomes() {
    let mut queue = HebbianOutcomeQueue::default();
    let now = Instant::now();
    let before = sample(100, now, rand::random::<u128>());
    arm(&mut queue, 1, before);
    let json = serde_json::to_value(&queue.pending).unwrap();
    assert!(json[0].get("observation").is_none());
    let restored = serde_json::from_value(json).unwrap();
    let mut queue = HebbianOutcomeQueue::from_checkpoint(restored, Some(90));
    let next = sample(102, now + Duration::from_secs(1), before.scope);
    let result = queue.take(Some(next), next.received_at);
    assert!(result.ready.is_none());
    assert_eq!(
        result.retired[0].reason,
        "restored_without_observation_clock"
    );
}

#[test]
fn reconnect_or_regression_retires_pairs_before_new_learning() {
    for reconnect in [false, true] {
        let mut clock = LearningClock::default();
        let mut queue = HebbianOutcomeQueue::default();
        let now = Instant::now();
        clock.observe(Some(1), 1000, now);
        let before = snapshot(&clock, 1, 1000).unwrap();
        arm(&mut queue, 1, before);
        let connection = if reconnect { 2 } else { 1 };
        clock.observe(Some(connection), 10, now + Duration::from_secs(1));
        clock.observe(Some(connection), 11, now + Duration::from_secs(2));
        let baseline = snapshot(&clock, connection, 11).unwrap();
        let result = queue.take(Some(baseline), baseline.received_at);
        assert!(result.ready.is_none());
        assert_eq!(result.retired[0].reason, "observation_continuity_changed");
        arm(&mut queue, 2, baseline);
        clock.observe(Some(connection), 12, now + Duration::from_secs(3));
        let after = snapshot(&clock, connection, 12).unwrap();
        assert_eq!(
            queue
                .take(Some(after), after.received_at)
                .ready
                .unwrap()
                .exchange_count,
            2
        );
    }
}

#[test]
fn isolated_out_of_order_packet_cannot_teach_from_prior_window() {
    let now = Instant::now();
    let mut clock = LearningClock::default();
    let mut queue = HebbianOutcomeQueue::default();
    clock.observe(Some(1), 100, now);
    arm(&mut queue, 1, snapshot(&clock, 1, 100).unwrap());
    clock.observe(Some(1), 99, now + Duration::from_secs(1));
    assert!(
        queue
            .take(snapshot(&clock, 1, 99), now + Duration::from_secs(1))
            .ready
            .is_none()
    );
    clock.observe(Some(1), 101, now + Duration::from_secs(2));
    let result = queue.take(snapshot(&clock, 1, 101), now + Duration::from_secs(2));
    assert!(result.ready.is_none());
    assert_eq!(result.retired[0].reason, "observation_continuity_changed");
}

#[test]
fn delayed_outcomes_expire_without_waiting_for_telemetry() {
    let now = Instant::now();
    let mut queue = HebbianOutcomeQueue::default();
    arm(&mut queue, 1, sample(100, now, rand::random::<u128>()));
    let result = queue.take(None, now + Duration::from_secs(301));
    assert!(result.ready.is_none());
    assert_eq!(result.retired[0].reason, "outcome_window_expired");
    assert!(queue.pending.is_empty());
}

#[test]
fn changed_or_missing_target_retires_pair_without_crediting_a_moved_reference() {
    let now = Instant::now();
    for target in [
        None,
        Some(crate::learning_target::LearningTarget {
            fill_pct: 63.0,
            source: "test",
        }),
        Some(crate::learning_target::LearningTarget {
            fill_pct: 68.0,
            source: "different_controller",
        }),
    ] {
        let mut queue = HebbianOutcomeQueue::default();
        let baseline = sample(100, now, 1);
        arm(&mut queue, 1, baseline);
        let mut later = sample(101, now + Duration::from_secs(1), 1);
        later.target = target;
        let result = queue.take(Some(later), later.received_at);
        assert!(result.ready.is_none());
        assert_eq!(result.retired.len(), 1);
        assert_eq!(
            result.retired[0].reason,
            if target.is_none() {
                "learning_target_unavailable"
            } else {
                "learning_target_changed"
            }
        );
        assert!(queue.last_consumed_t_ms.is_none());
        assert_eq!(result.retired[0].baseline_target, baseline.target);
    }
    let mut queue = HebbianOutcomeQueue::default();
    let mut baseline = sample(100, now, 1);
    baseline.target = None;
    assert_eq!(
        arm(&mut queue, 2, baseline)[0].reason,
        "learning_target_unavailable"
    );
    assert!(queue.pending.is_empty());
}

#[test]
fn stale_or_future_samples_cannot_consume_and_invalid_baselines_are_not_queued() {
    let now = Instant::now();
    let mut queue = HebbianOutcomeQueue::default();
    let baseline = sample(100, now, rand::random::<u128>());
    arm(&mut queue, 1, baseline);
    let next = sample(101, now + Duration::from_secs(1), baseline.scope);
    assert!(queue.take(Some(next), now).ready.is_none());
    assert!(
        queue
            .take(Some(next), now + Duration::from_secs(32))
            .ready
            .is_none()
    );
    assert_eq!(queue.pending.len(), 1);
    assert_eq!(
        queue.arm(2, vec![0.2], 71.0, None, now)[0].reason,
        "baseline_observation_unavailable"
    );
    assert_eq!(
        queue.arm(2, vec![f32::NAN], 71.0, Some(baseline), now)[0].reason,
        "invalid_baseline"
    );
    assert_eq!(
        queue.arm(2, vec![0.2], f32::NAN, Some(baseline), now)[0].reason,
        "invalid_baseline"
    );
    assert_eq!(
        queue.arm(
            2,
            vec![0.2],
            71.0,
            Some(baseline),
            now + Duration::from_secs(301)
        )[0]
        .reason,
        "outcome_window_expired"
    );
    assert_eq!(queue.pending.len(), 1);
}
