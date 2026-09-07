//! Bounded action/outcome pairing inside one observed telemetry continuity window.

use std::collections::VecDeque;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::learning_clock::{
    LearningObservation, MAX_LEARNING_OUTCOME_AGE, MAX_LEARNING_SAMPLE_AGE,
};

const MAX_PENDING_OUTCOMES: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct PendingHebbianOutcome {
    pub exchange_count: u64,
    pub signature: Vec<f32>,
    pub fill_before: f32,
    pub telemetry_t_ms_before: Option<u64>,
    // A monotonic observation cannot be reconstituted from a saved wall clock.
    #[serde(skip)]
    pub observation: Option<LearningObservation>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(super) struct OutcomeRetirement {
    pub exchange_count: u64,
    pub reason: &'static str,
    pub baseline_t_ms: Option<u64>,
    pub observed_t_ms: Option<u64>,
    pub baseline_target: Option<crate::learning_target::LearningTarget>,
    pub observed_target: Option<crate::learning_target::LearningTarget>,
}

impl OutcomeRetirement {
    fn new(
        pending: &PendingHebbianOutcome,
        reason: &'static str,
        sample: Option<LearningObservation>,
    ) -> Self {
        Self {
            exchange_count: pending.exchange_count,
            reason,
            baseline_t_ms: pending.telemetry_t_ms_before,
            observed_t_ms: sample.map(|sample| sample.producer_t_ms),
            baseline_target: pending.observation.and_then(|sample| sample.target),
            observed_target: sample.and_then(|sample| sample.target),
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct HebbianOutcomeQueue {
    pub pending: VecDeque<PendingHebbianOutcome>,
    pub last_consumed_t_ms: Option<u64>,
    scope: Option<u128>,
}

#[derive(Default)]
pub(super) struct OutcomeResolution {
    pub ready: Option<PendingHebbianOutcome>,
    pub retired: Vec<OutcomeRetirement>,
}

impl HebbianOutcomeQueue {
    pub fn from_checkpoint(
        mut pending: VecDeque<PendingHebbianOutcome>,
        watermark: Option<u64>,
    ) -> Self {
        for outcome in &mut pending {
            outcome.observation = None;
        }
        Self {
            pending,
            last_consumed_t_ms: watermark,
            scope: None,
        }
    }

    fn retire_ineligible(
        &mut self,
        sample: Option<LearningObservation>,
        now: Instant,
    ) -> Vec<OutcomeRetirement> {
        let mut retired = Vec::new();
        self.pending.retain(|pending| {
            let reason = match pending.observation {
                None => Some("restored_without_observation_clock"),
                Some(baseline) if !baseline.within_age(now, MAX_LEARNING_OUTCOME_AGE) => {
                    Some("outcome_window_expired")
                },
                Some(baseline) if sample.is_some_and(|sample| sample.scope != baseline.scope) => {
                    Some("observation_continuity_changed")
                },
                Some(_) if sample.is_some_and(|sample| sample.target.is_none()) => {
                    Some("learning_target_unavailable")
                },
                Some(baseline) if sample.is_some_and(|sample| sample.target != baseline.target) => {
                    Some("learning_target_changed")
                },
                _ => None,
            };
            if let Some(reason) = reason {
                retired.push(OutcomeRetirement::new(pending, reason, sample));
                false
            } else {
                true
            }
        });
        if let Some(sample) = sample
            && self.scope != Some(sample.scope)
        {
            self.scope = Some(sample.scope);
            self.last_consumed_t_ms = None;
        }
        retired
    }

    pub(super) fn arm(
        &mut self,
        exchange_count: u64,
        signature: Vec<f32>,
        fill_before: f32,
        sample: Option<LearningObservation>,
        now: Instant,
    ) -> Vec<OutcomeRetirement> {
        let pending = PendingHebbianOutcome {
            exchange_count,
            signature,
            fill_before,
            telemetry_t_ms_before: sample.map(|sample| sample.producer_t_ms),
            observation: sample,
        };
        let reason = if pending.signature.is_empty()
            || !fill_before.is_finite()
            || pending.signature.iter().any(|value| !value.is_finite())
        {
            Some("invalid_baseline")
        } else {
            match sample {
                None => Some("baseline_observation_unavailable"),
                Some(sample) if !sample.within_age(now, MAX_LEARNING_OUTCOME_AGE) => {
                    Some("outcome_window_expired")
                },
                Some(sample) if sample.target.is_none() => Some("learning_target_unavailable"),
                Some(_) => None,
            }
        };
        if let Some(reason) = reason {
            return vec![OutcomeRetirement::new(&pending, reason, sample)];
        }
        let mut retired = self.retire_ineligible(sample, now);
        while self.pending.len() >= MAX_PENDING_OUTCOMES {
            if let Some(oldest) = self.pending.pop_front() {
                retired.push(OutcomeRetirement::new(&oldest, "fifo_capacity", sample));
            }
        }
        self.pending.push_back(pending);
        retired
    }

    pub(super) fn take(
        &mut self,
        sample: Option<LearningObservation>,
        now: Instant,
    ) -> OutcomeResolution {
        let sample = sample.filter(|sample| sample.within_age(now, MAX_LEARNING_SAMPLE_AGE));
        let mut result = OutcomeResolution {
            retired: self.retire_ineligible(sample, now),
            ..OutcomeResolution::default()
        };
        let Some(sample) = sample else {
            return result;
        };
        if self
            .last_consumed_t_ms
            .is_some_and(|last| sample.producer_t_ms <= last)
        {
            return result;
        }
        if self.pending.front().is_some_and(|pending| {
            pending.observation.is_some_and(|baseline| {
                baseline.scope == sample.scope
                    && baseline.received_at < sample.received_at
                    && baseline.producer_t_ms < sample.producer_t_ms
            })
        }) {
            self.last_consumed_t_ms = Some(sample.producer_t_ms);
            result.ready = self.pending.pop_front();
        }
        result
    }
}

#[cfg(test)]
#[path = "learning_outcomes/tests.rs"]
mod tests;
