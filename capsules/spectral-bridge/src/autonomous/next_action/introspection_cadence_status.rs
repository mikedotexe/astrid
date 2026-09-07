use std::time::Duration;

use super::{IntrospectionCadenceV1, PILOT_MAX_STARTS};

pub(in crate::autonomous) fn render_status(
    cadence: &IntrospectionCadenceV1,
    completed_exchange: u64,
) -> String {
    let state = if cadence.enabled { "ON" } else { "OFF" };
    let interval = if cadence.enabled {
        cadence.every_exchanges.to_string()
    } else {
        "none".to_string()
    };
    let target = if cadence.enabled {
        cadence.target_text()
    } else {
        "none".to_string()
    };
    let next_due = cadence
        .next_due_exchange()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string());
    let pending = cadence
        .pending_since_exchange
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string());
    let pilot_hold = cadence.pilot_hold_reason.as_deref().unwrap_or("none");
    let lifecycle_debt = cadence.lifecycle_receipt_debt.as_ref().map_or_else(
        || "none".to_string(),
        |debt| {
            format!(
                "{} at exchange {}{}",
                debt.event.as_str(),
                debt.exchange,
                debt.attempt_id
                    .as_deref()
                    .map(|attempt_id| format!(" (attempt {attempt_id})"))
                    .unwrap_or_default()
            )
        },
    );
    let pilot_runtime_hours =
        Duration::from_millis(cadence.pilot_runtime_ms).as_secs_f64() / 3_600.0;
    let last_outcome = cadence.last_outcome.as_ref().map_or_else(
        || "none".to_string(),
        |outcome| {
            format!(
                "{} at exchange {}{}",
                outcome.event.as_str(),
                outcome.exchange,
                outcome
                    .reason
                    .as_deref()
                    .map(|reason| format!(" ({reason})"))
                    .unwrap_or_default()
            )
        },
    );
    format!(
        "=== INTROSPECTION CADENCE V1 ===\n\
         State: {state}\n\
         Completed exchange: {completed_exchange}\n\
         Every exchanges: {interval}\n\
         Target: {target}\n\
         Next due exchange: {next_due}\n\
         Pending since exchange: {pending}\n\
         Last attempted exchange: {}\n\
         Last admitted exchange: {}\n\
         Last outcome: {last_outcome}\n\
         Pilot starts: {}/{}\n\
         Pilot cumulative runtime hours: {pilot_runtime_hours:.3}/72\n\
         Pilot review hold: {pilot_hold}\n\
         Lifecycle receipt debt: {lifecycle_debt}\n\
         Authorship: only an Astrid-authored NEXT may enable, change, or disable this preference.\n\
         Priority: safety, correspondence, peer study, and newer explicit actions stay ahead.\n\
         Syntax: NEXT: INTROSPECTION_CADENCE EVERY <4..256> [target [offset]] | OFF | STATUS",
        cadence
            .last_attempt_exchange
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
        cadence
            .last_admitted_exchange
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
        cadence.pilot_cadence_starts,
        PILOT_MAX_STARTS,
    )
}

pub(in crate::autonomous) const fn help_text() -> &'static str {
    "INTROSPECTION_CADENCE — An optional, default-OFF standing introspection preference authored only by Astrid. Use NEXT: INTROSPECTION_CADENCE EVERY <4..256> [target [offset]], NEXT: INTROSPECTION_CADENCE OFF, or NEXT: INTROSPECTION_CADENCE STATUS. Safety, correspondence, peer study, and newer explicit actions remain ahead; failed attempts stay pending and do not advance the interval."
}
