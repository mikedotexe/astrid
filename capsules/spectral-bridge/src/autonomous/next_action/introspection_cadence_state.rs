use super::{
    CADENCE_SCHEMA_VERSION, IntrospectionCadenceTargetV1, IntrospectionCadenceV1,
    MAX_CADENCE_EXCHANGES, MIN_CADENCE_EXCHANGES, PILOT_MAX_RUNTIME_MS, PILOT_MAX_STARTS,
};

impl Default for IntrospectionCadenceV1 {
    fn default() -> Self {
        Self {
            schema_version: CADENCE_SCHEMA_VERSION,
            enabled: false,
            every_exchanges: 0,
            target: None,
            anchor_completed_exchange: None,
            pending_since_exchange: None,
            last_attempt_exchange: None,
            last_admitted_exchange: None,
            last_outcome: None,
            last_deferred_exchange: None,
            last_deferred_reason: None,
            last_astrid_action_completed_exchange: None,
            pilot_started_at_unix_ms: None,
            pilot_runtime_ms: 0,
            pilot_last_observed_unix_ms: None,
            pilot_cadence_starts: 0,
            pilot_hold_reason: None,
            lifecycle_receipt_debt: None,
        }
    }
}

impl IntrospectionCadenceV1 {
    pub(crate) fn repair_after_restore(&mut self, completed_exchange: u64, now_ms: u64) {
        self.schema_version = CADENCE_SCHEMA_VERSION;
        if !self.enabled
            || !(MIN_CADENCE_EXCHANGES..=MAX_CADENCE_EXCHANGES).contains(&self.every_exchanges)
        {
            self.enabled = false;
            self.every_exchanges = 0;
            self.target = None;
            self.anchor_completed_exchange = None;
            self.pending_since_exchange = None;
            self.last_deferred_exchange = None;
            self.last_deferred_reason = None;
            self.pilot_last_observed_unix_ms = None;
            return;
        }
        if self.anchor_completed_exchange.is_none() {
            self.anchor_completed_exchange = Some(completed_exchange);
        }
        if self
            .pending_since_exchange
            .is_some_and(|pending| pending > completed_exchange)
        {
            self.pending_since_exchange = None;
        }
        if self.pilot_started_at_unix_ms.is_none() {
            self.pilot_started_at_unix_ms = Some(now_ms);
        }
        self.pilot_last_observed_unix_ms = self.pilot_hold_reason.is_none().then_some(now_ms);
    }

    pub(super) fn target_text(&self) -> String {
        self.target.as_ref().map_or_else(
            || "next-in-rotation".to_string(),
            IntrospectionCadenceTargetV1::render,
        )
    }

    pub(super) fn next_due_exchange(&self) -> Option<u64> {
        self.enabled.then_some(())?;
        self.pilot_hold_reason.is_none().then_some(())?;
        self.lifecycle_receipt_debt.is_none().then_some(())?;
        let anchor = self.anchor_completed_exchange?;
        Some(anchor.saturating_add(u64::from(self.every_exchanges)))
    }

    pub(super) fn observe_pilot_runtime_at(&mut self, now_ms: u64) -> Option<&'static str> {
        if !self.enabled || self.pilot_hold_reason.is_some() {
            self.pilot_last_observed_unix_ms = None;
            return None;
        }
        if self.pilot_started_at_unix_ms.is_none() {
            self.pilot_started_at_unix_ms = Some(now_ms);
        }
        if let Some(previous) = self.pilot_last_observed_unix_ms {
            self.pilot_runtime_ms = self
                .pilot_runtime_ms
                .saturating_add(now_ms.saturating_sub(previous));
        }
        self.pilot_last_observed_unix_ms = Some(now_ms);
        let reason = if self.pilot_cadence_starts >= PILOT_MAX_STARTS {
            Some("pilot_eight_start_review_hold")
        } else if self.pilot_runtime_ms >= PILOT_MAX_RUNTIME_MS {
            Some("pilot_seventy_two_runtime_hour_review_hold")
        } else {
            None
        };
        if let Some(reason) = reason {
            self.pilot_hold_reason = Some(reason.to_string());
            self.pilot_last_observed_unix_ms = None;
        }
        reason
    }
}
