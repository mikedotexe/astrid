//! Bridge-local observation continuity, not a claimed producer boot identity.

use std::time::{Duration, Instant};

pub(crate) const MAX_LEARNING_SAMPLE_AGE: Duration = Duration::from_secs(30);
pub(crate) const MAX_LEARNING_OUTCOME_AGE: Duration = Duration::from_secs(300);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LearningObservation {
    pub scope: u128,
    pub producer_t_ms: u64,
    pub received_at: Instant,
    pub target: Option<crate::learning_target::LearningTarget>,
}

impl LearningObservation {
    pub fn within_age(self, now: Instant, limit: Duration) -> bool {
        now.checked_duration_since(self.received_at)
            .is_some_and(|age| age <= limit)
    }
}

#[derive(Debug, Default)]
pub(crate) struct LearningClock {
    connection_id: Option<u64>,
    scope: u128,
    last_t_ms: Option<u64>,
    sample: Option<LearningObservation>,
}

impl LearningClock {
    pub fn observe(&mut self, connection_id: Option<u64>, t_ms: u64, now: Instant) {
        let Some(connection_id) = connection_id else {
            *self = Self::default();
            return;
        };
        let changed = self.connection_id != Some(connection_id);
        let regressed = !changed && self.last_t_ms.is_some_and(|last| t_ms < last);
        if changed || regressed {
            self.scope = rand::random::<u128>();
            self.sample = None;
        }
        let duplicate = !changed && self.last_t_ms == Some(t_ms);
        self.connection_id = Some(connection_id);
        self.last_t_ms = Some(t_ms);
        // A regressing packet opens an uncertain interval, not a new baseline.
        // The next advancing packet can establish a fresh local window.
        self.sample = if regressed || duplicate {
            None
        } else {
            Some(LearningObservation {
                scope: self.scope,
                producer_t_ms: t_ms,
                received_at: now,
                target: None,
            })
        };
    }

    pub fn snapshot(
        &self,
        connected: bool,
        connection_id: Option<u64>,
        current_t_ms: u64,
    ) -> Option<LearningObservation> {
        self.sample.filter(|sample| {
            connected && connection_id == self.connection_id && sample.producer_t_ms == current_t_ms
        })
    }
}

impl crate::ws::BridgeState {
    /// Install the packet and its learning stamp together under the caller's write guard.
    pub(crate) fn install_telemetry_observation(
        &mut self,
        telemetry: crate::types::SpectralTelemetry,
        received_at: Instant,
    ) {
        self.learning_clock.observe(
            self.telemetry_ws.active_connection_id,
            telemetry.t_ms,
            received_at,
        );
        self.latest_telemetry = Some(telemetry);
    }

    pub(crate) fn learning_observation(&self) -> Option<LearningObservation> {
        self.learning_clock.snapshot(
            self.telemetry_connected,
            self.telemetry_ws.active_connection_id,
            self.latest_telemetry.as_ref()?.t_ms,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_requires_a_new_packet_even_without_a_producer_restart() {
        let mut clock = LearningClock::default();
        let now = Instant::now();
        clock.observe(Some(1), 100, now);
        let before = clock.snapshot(true, Some(1), 100).unwrap();
        assert!(clock.snapshot(false, None, 100).is_none());
        assert!(clock.snapshot(true, Some(2), 100).is_none());
        clock.observe(Some(2), 101, now);
        assert_ne!(
            before.scope,
            clock.snapshot(true, Some(2), 101).unwrap().scope
        );
    }

    #[test]
    fn regression_is_not_a_baseline_and_does_not_strand_future_samples() {
        let mut clock = LearningClock::default();
        let now = Instant::now();
        clock.observe(Some(1), 1_075_024_066, now);
        let old = clock.snapshot(true, Some(1), 1_075_024_066).unwrap();
        clock.observe(Some(1), 529_076_440, now);
        assert!(clock.snapshot(true, Some(1), 529_076_440).is_none());
        clock.observe(Some(1), 529_078_800, now);
        assert_ne!(
            old.scope,
            clock.snapshot(true, Some(1), 529_078_800).unwrap().scope
        );
    }

    #[test]
    fn duplicate_does_not_refresh_sample_age_or_allow_stale_snapshot() {
        let mut clock = LearningClock::default();
        let now = Instant::now();
        clock.observe(Some(1), 100, now);
        assert!(clock.snapshot(true, Some(1), 99).is_none());
        clock.observe(Some(1), 100, now + Duration::from_secs(60));
        assert!(clock.snapshot(true, Some(1), 100).is_none());
        clock.observe(None, 100, now);
        assert!(clock.snapshot(true, None, 100).is_none());
    }

    #[test]
    fn age_uses_monotonic_time_and_rejects_future_observations() {
        let observation = LearningObservation {
            scope: rand::random::<u128>(),
            producer_t_ms: 12,
            received_at: Instant::now(),
            target: None,
        };
        assert!(observation.within_age(observation.received_at, MAX_LEARNING_SAMPLE_AGE));
        assert!(!observation.within_age(
            observation.received_at - Duration::from_secs(1),
            MAX_LEARNING_SAMPLE_AGE
        ));
        assert!(!observation.within_age(
            observation.received_at + Duration::from_secs(31),
            MAX_LEARNING_SAMPLE_AGE
        ));
    }
}
