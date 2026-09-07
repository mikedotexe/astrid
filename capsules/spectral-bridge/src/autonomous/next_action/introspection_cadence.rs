use std::{
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use tracing::{info, warn};

use super::super::{introspect, state::IntrospectTargetV2};
use super::{ConversationState, NextActionOutcome};

#[path = "introspection_cadence_parser.rs"]
mod parser;
use parser::{CadenceCommandV1, parse_command};

#[path = "introspection_cadence_status.rs"]
mod status;
pub(in crate::autonomous) use status::{help_text, render_status};

#[path = "introspection_cadence_state.rs"]
mod state_helpers;

pub(in crate::autonomous) const MIN_CADENCE_EXCHANGES: u16 = 4;
pub(in crate::autonomous) const MAX_CADENCE_EXCHANGES: u16 = 256;
const CADENCE_SCHEMA_VERSION: u32 = 1;
const PILOT_MAX_STARTS: u16 = 8;
const PILOT_MAX_RUNTIME_MS: u64 = 259_200_000;
#[cfg(not(test))]
const CADENCE_SIGNAL: &str = "introspection_cadence_v1";
const CADENCE_EVENT_LEDGER: &str = "introspection_cadence_events_v1.jsonl";

const fn default_schema_version() -> u32 {
    CADENCE_SCHEMA_VERSION
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::autonomous) enum NextActionAuthorV1 {
    Astrid,
    Operator,
}

impl NextActionAuthorV1 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Astrid => "astrid",
            Self::Operator => "operator",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::autonomous) struct IntrospectionCadenceTargetV1 {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
}

impl IntrospectionCadenceTargetV1 {
    fn render(&self) -> String {
        match self.offset {
            Some(offset) => format!("{} {offset}", self.label),
            None => self.label.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::autonomous) enum IntrospectionCadenceEventV1 {
    Configured,
    Disabled,
    Rejected,
    Due,
    Deferred,
    Attempted,
    Admitted,
    Failed,
}

impl IntrospectionCadenceEventV1 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Configured => "configured",
            Self::Disabled => "disabled",
            Self::Rejected => "rejected",
            Self::Due => "due",
            Self::Deferred => "deferred",
            Self::Attempted => "attempted",
            Self::Admitted => "admitted",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::autonomous) struct IntrospectionCadenceOutcomeV1 {
    pub event: IntrospectionCadenceEventV1,
    pub exchange: u64,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub attempt_id: Option<String>,
    #[serde(default)]
    pub artifact_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::autonomous) struct IntrospectionCadenceLifecycleDebtV1 {
    pub event: IntrospectionCadenceEventV1,
    pub exchange: u64,
    pub source: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub attempt_id: Option<String>,
    #[serde(default)]
    pub artifact_path: Option<String>,
    #[serde(default)]
    pub witness_outcome: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::autonomous) struct IntrospectionCadenceV1 {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub every_exchanges: u16,
    #[serde(default)]
    pub target: Option<IntrospectionCadenceTargetV1>,
    #[serde(default)]
    pub anchor_completed_exchange: Option<u64>,
    #[serde(default)]
    pub pending_since_exchange: Option<u64>,
    #[serde(default)]
    pub last_attempt_exchange: Option<u64>,
    #[serde(default)]
    pub last_admitted_exchange: Option<u64>,
    #[serde(default)]
    pub last_outcome: Option<IntrospectionCadenceOutcomeV1>,
    #[serde(default)]
    pub last_deferred_exchange: Option<u64>,
    #[serde(default)]
    pub last_deferred_reason: Option<String>,
    #[serde(default)]
    pub last_astrid_action_completed_exchange: Option<u64>,
    #[serde(default)]
    pub pilot_started_at_unix_ms: Option<u64>,
    #[serde(default)]
    pub pilot_runtime_ms: u64,
    #[serde(default)]
    pub pilot_last_observed_unix_ms: Option<u64>,
    #[serde(default)]
    pub pilot_cadence_starts: u16,
    #[serde(default)]
    pub pilot_hold_reason: Option<String>,
    #[serde(default)]
    pub lifecycle_receipt_debt: Option<IntrospectionCadenceLifecycleDebtV1>,
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

pub(in crate::autonomous) fn unix_now_ms_for_restore() -> u64 {
    unix_now_ms()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::autonomous) struct IntrospectionCadenceAttemptV1 {
    pub attempt_id: String,
    pub pending_since_exchange: u64,
    pub target: Option<IntrospectionCadenceTargetV1>,
    started: bool,
}

fn lifecycle_payload(
    cadence: &IntrospectionCadenceV1,
    event: IntrospectionCadenceEventV1,
    exchange: u64,
    source: &str,
    reason: Option<&str>,
    attempt_id: Option<&str>,
    artifact_path: Option<&str>,
    witness_outcome: Option<&str>,
) -> Value {
    let identity_bytes = serde_json::to_vec(&json!({
        "event": event.as_str(),
        "exchange": exchange,
        "source": source,
        "enabled": cadence.enabled,
        "every_exchanges": cadence.every_exchanges,
        "target": cadence.target_text(),
        "anchor_completed_exchange": cadence.anchor_completed_exchange,
        "pending_since_exchange": cadence.pending_since_exchange,
        "attempt_id": attempt_id,
        "reason": reason,
        "artifact_path": artifact_path,
        "witness_outcome": witness_outcome,
        "pilot_cadence_starts": cadence.pilot_cadence_starts,
        "pilot_hold_reason": cadence.pilot_hold_reason,
    }))
    .unwrap_or_default();
    let lifecycle_id = format!(
        "introspection-cadence-v1-{:x}",
        Sha256::digest(identity_bytes)
    );
    json!({
        "schema_version": CADENCE_SCHEMA_VERSION,
        "lifecycle_id": lifecycle_id,
        "event": event.as_str(),
        "exchange": exchange,
        "source": source,
        "enabled": cadence.enabled,
        "every_exchanges": cadence.enabled.then_some(cadence.every_exchanges),
        "target": cadence.enabled.then(|| cadence.target_text()),
        "anchor_completed_exchange": cadence.anchor_completed_exchange,
        "pending_since_exchange": cadence.pending_since_exchange,
        "attempt_id": attempt_id,
        "reason": reason,
        "artifact_path": artifact_path,
        "witness_outcome": witness_outcome,
        "pilot_started_at_unix_ms": cadence.pilot_started_at_unix_ms,
        "pilot_runtime_ms": cadence.pilot_runtime_ms,
        "pilot_cadence_starts": cadence.pilot_cadence_starts,
        "pilot_hold_reason": cadence.pilot_hold_reason,
        "authority": "astrid_authored_preference_only_no_operator_activation",
    })
}

fn append_lifecycle_event_at(path: &std::path::Path, event: &Value) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if let Some(lifecycle_id) = event.get("lifecycle_id").and_then(Value::as_str)
        && path.exists()
    {
        let existing = std::fs::read_to_string(path)?;
        if existing
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .any(|row| row.get("lifecycle_id").and_then(Value::as_str) == Some(lifecycle_id))
        {
            return Ok(());
        }
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    serde_json::to_writer(&mut file, event).map_err(std::io::Error::other)?;
    file.write_all(b"\n")?;
    file.sync_data()
}

#[cfg(not(test))]
fn persist_lifecycle_event(mut event: Value) -> std::io::Result<()> {
    if let Value::Object(fields) = &mut event {
        fields.insert(
            "recorded_at".to_string(),
            Value::String(chrono::Local::now().fixed_offset().to_rfc3339()),
        );
    }
    let ledger_path = crate::paths::bridge_paths()
        .bridge_workspace()
        .join(CADENCE_EVENT_LEDGER);
    append_lifecycle_event_at(&ledger_path, &event)?;
    if let Err(error) = crate::condition_metrics::record_bridge_signal(CADENCE_SIGNAL, event) {
        warn!(%error, "cadence lifecycle metric projection failed after durable ledger append");
    }
    Ok(())
}

#[cfg(test)]
thread_local! {
    static TEST_LIFECYCLE_PERSISTENCE_FAILURE: std::cell::Cell<bool> = const {
        std::cell::Cell::new(false)
    };
}

#[cfg(test)]
fn persist_lifecycle_event(_event: Value) -> std::io::Result<()> {
    TEST_LIFECYCLE_PERSISTENCE_FAILURE.with(|failure| {
        if failure.get() {
            Err(std::io::Error::other("injected cadence lifecycle failure"))
        } else {
            Ok(())
        }
    })
}

fn record_lifecycle(
    cadence: &IntrospectionCadenceV1,
    event: IntrospectionCadenceEventV1,
    exchange: u64,
    source: &str,
    reason: Option<&str>,
    attempt_id: Option<&str>,
    artifact_path: Option<&str>,
    witness_outcome: Option<&str>,
) -> std::io::Result<()> {
    let payload = lifecycle_payload(
        cadence,
        event,
        exchange,
        source,
        reason,
        attempt_id,
        artifact_path,
        witness_outcome,
    );
    if let Err(error) = persist_lifecycle_event(payload) {
        warn!(
            error = %error,
            cadence_event = event.as_str(),
            "failed to persist introspection cadence lifecycle event"
        );
        return Err(error);
    }
    info!(
        cadence_event = event.as_str(),
        exchange,
        source,
        reason = reason.unwrap_or("none"),
        "introspection cadence lifecycle"
    );
    Ok(())
}

fn lifecycle_debt(
    event: IntrospectionCadenceEventV1,
    exchange: u64,
    source: &str,
    reason: Option<&str>,
    attempt_id: Option<&str>,
    artifact_path: Option<&str>,
    witness_outcome: Option<&str>,
) -> IntrospectionCadenceLifecycleDebtV1 {
    IntrospectionCadenceLifecycleDebtV1 {
        event,
        exchange,
        source: source.to_string(),
        reason: reason.map(str::to_string),
        attempt_id: attempt_id.map(str::to_string),
        artifact_path: artifact_path.map(str::to_string),
        witness_outcome: witness_outcome.map(str::to_string),
    }
}

fn record_or_defer_lifecycle(
    cadence: &mut IntrospectionCadenceV1,
    debt: IntrospectionCadenceLifecycleDebtV1,
) -> bool {
    if record_lifecycle(
        cadence,
        debt.event,
        debt.exchange,
        &debt.source,
        debt.reason.as_deref(),
        debt.attempt_id.as_deref(),
        debt.artifact_path.as_deref(),
        debt.witness_outcome.as_deref(),
    )
    .is_ok()
    {
        cadence.lifecycle_receipt_debt = None;
        true
    } else {
        cadence.lifecycle_receipt_debt = Some(debt);
        false
    }
}

fn flush_lifecycle_debt(cadence: &mut IntrospectionCadenceV1) -> bool {
    let Some(debt) = cadence.lifecycle_receipt_debt.clone() else {
        return true;
    };
    record_or_defer_lifecycle(cadence, debt)
}

fn set_last_outcome(
    cadence: &mut IntrospectionCadenceV1,
    event: IntrospectionCadenceEventV1,
    exchange: u64,
    reason: Option<String>,
    attempt_id: Option<String>,
    artifact_path: Option<String>,
) {
    cadence.last_outcome = Some(IntrospectionCadenceOutcomeV1 {
        event,
        exchange,
        reason,
        attempt_id,
        artifact_path,
    });
}

fn rejection_outcome(
    conv: &mut ConversationState,
    author: NextActionAuthorV1,
    reason: String,
) -> NextActionOutcome {
    if flush_lifecycle_debt(&mut conv.introspection_cadence) {
        let debt = lifecycle_debt(
            IntrospectionCadenceEventV1::Rejected,
            conv.exchange_count,
            author.as_str(),
            Some(&reason),
            None,
            None,
            None,
        );
        record_or_defer_lifecycle(&mut conv.introspection_cadence, debt);
    }
    if author == NextActionAuthorV1::Astrid {
        conv.push_receipt(
            "INTROSPECTION_CADENCE",
            vec![format!("unchanged: {reason}")],
        );
        conv.emphasis = Some(format!(
            "Introspection cadence stayed unchanged: {reason}. Use NEXT: INTROSPECTION_CADENCE STATUS for the current factual state."
        ));
    }
    NextActionOutcome::blocked("introspection_cadence", reason)
        .with_stage_visibility("blocked", "protected_summary")
}

fn persistence_blocked_outcome(
    conv: &mut ConversationState,
    event: IntrospectionCadenceEventV1,
    error: &std::io::Error,
) -> NextActionOutcome {
    let reason = format!(
        "{} receipt could not be made durable; cadence state stayed unchanged: {error}",
        event.as_str()
    );
    let debt = lifecycle_debt(
        IntrospectionCadenceEventV1::Rejected,
        conv.exchange_count,
        NextActionAuthorV1::Astrid.as_str(),
        Some(&reason),
        None,
        None,
        None,
    );
    record_or_defer_lifecycle(&mut conv.introspection_cadence, debt);
    conv.push_receipt("INTROSPECTION_CADENCE", vec![reason.clone()]);
    conv.emphasis = Some(reason.clone());
    NextActionOutcome::blocked("introspection_cadence", reason)
        .with_stage_visibility("blocked", "protected_summary")
}

pub(in crate::autonomous) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    author: NextActionAuthorV1,
) -> Option<NextActionOutcome> {
    if base_action != "INTROSPECTION_CADENCE" {
        return None;
    }
    let command = match parse_command(original) {
        Ok(command) => command,
        Err(reason) => return Some(rejection_outcome(conv, author, reason)),
    };

    if command == CadenceCommandV1::Status {
        let status = render_status(&conv.introspection_cadence, conv.exchange_count);
        if author == NextActionAuthorV1::Astrid {
            conv.pending_file_listing = Some(status.clone());
        }
        return Some(
            NextActionOutcome::handled("introspection_cadence", status)
                .with_stage_visibility("read_only", "protected_summary"),
        );
    }

    if !flush_lifecycle_debt(&mut conv.introspection_cadence) {
        let reason = "a prior cadence lifecycle receipt is still pending durable append; configuration remains unchanged"
            .to_string();
        return Some(
            NextActionOutcome::blocked("introspection_cadence", reason)
                .with_stage_visibility("blocked", "protected_summary"),
        );
    }

    if author != NextActionAuthorV1::Astrid {
        return Some(rejection_outcome(
            conv,
            author,
            "operator actions may inspect cadence status but may not enable, change, or disable Astrid's preference"
                .to_string(),
        ));
    }

    match command {
        CadenceCommandV1::Status => unreachable!("STATUS returned above"),
        CadenceCommandV1::Off => {
            let mut cadence = conv.introspection_cadence.clone();
            cadence.observe_pilot_runtime_at(unix_now_ms());
            cadence.enabled = false;
            cadence.every_exchanges = 0;
            cadence.target = None;
            cadence.anchor_completed_exchange = None;
            cadence.pending_since_exchange = None;
            cadence.last_deferred_exchange = None;
            cadence.last_deferred_reason = None;
            cadence.pilot_last_observed_unix_ms = None;
            if cadence.pilot_started_at_unix_ms.is_some() {
                cadence.pilot_hold_reason = Some("astrid_authored_off".to_string());
            }
            set_last_outcome(
                &mut cadence,
                IntrospectionCadenceEventV1::Disabled,
                conv.exchange_count,
                Some("astrid_authored_off".to_string()),
                None,
                None,
            );
            if let Err(error) = record_lifecycle(
                &cadence,
                IntrospectionCadenceEventV1::Disabled,
                conv.exchange_count,
                author.as_str(),
                Some("astrid_authored_off"),
                None,
                None,
                None,
            ) {
                return Some(persistence_blocked_outcome(
                    conv,
                    IntrospectionCadenceEventV1::Disabled,
                    &error,
                ));
            }
            conv.introspection_cadence = cadence;
            conv.introspection_cadence_attempt = None;
            let message = "Introspection cadence is OFF. Any pending cadence-due attempt was cleared; one-shot INTROSPECT remains available.".to_string();
            conv.push_receipt("INTROSPECTION_CADENCE", vec![message.clone()]);
            conv.emphasis = Some(message.clone());
            Some(
                NextActionOutcome::handled("introspection_cadence", message)
                    .with_stage_visibility("self_direction", "protected_summary"),
            )
        },
        CadenceCommandV1::Every {
            every_exchanges,
            target,
        } => {
            if let Some(requested) = target.as_ref() {
                let sources = introspect::introspect_sources();
                if let Err(reason) =
                    introspect::resolve_introspect_target_result(&requested.label, &sources)
                {
                    return Some(rejection_outcome(
                        conv,
                        author,
                        format!("target `{}` is unavailable: {reason}", requested.label),
                    ));
                }
            }
            let now_ms = unix_now_ms();
            let anchor = conv.exchange_count.saturating_add(1);
            let mut cadence = conv.introspection_cadence.clone();
            cadence.observe_pilot_runtime_at(now_ms);
            if cadence.pilot_started_at_unix_ms.is_none() {
                cadence.pilot_started_at_unix_ms = Some(now_ms);
                cadence.pilot_runtime_ms = 0;
                cadence.pilot_cadence_starts = 0;
                cadence.pilot_hold_reason = None;
            }
            cadence.enabled = true;
            cadence.every_exchanges = every_exchanges;
            cadence.target = target;
            cadence.anchor_completed_exchange = Some(anchor);
            cadence.pending_since_exchange = None;
            cadence.last_deferred_exchange = None;
            cadence.last_deferred_reason = None;
            if cadence.pilot_hold_reason.is_none() {
                cadence.pilot_last_observed_unix_ms = Some(now_ms);
            }
            set_last_outcome(
                &mut cadence,
                IntrospectionCadenceEventV1::Configured,
                conv.exchange_count,
                Some("astrid_authored_every".to_string()),
                None,
                None,
            );
            if let Err(error) = record_lifecycle(
                &cadence,
                IntrospectionCadenceEventV1::Configured,
                conv.exchange_count,
                author.as_str(),
                Some("astrid_authored_every"),
                None,
                None,
                None,
            ) {
                return Some(persistence_blocked_outcome(
                    conv,
                    IntrospectionCadenceEventV1::Configured,
                    &error,
                ));
            }
            conv.introspection_cadence = cadence;
            conv.introspection_cadence_attempt = None;
            let pilot_note = conv
                .introspection_cadence
                .pilot_hold_reason
                .as_deref()
                .map_or("", |reason| {
                    if reason == "astrid_authored_off" {
                        " The bounded pilot remains stopped after authored OFF pending review."
                    } else {
                        " The bounded pilot is at its review hold; configuration is retained without new cadence starts."
                    }
                });
            let message = format!(
                "Introspection cadence is ON every {every_exchanges} completed exchanges; target={}. Explicit actions, correspondence, peer study, and safety remain ahead of it.{pilot_note}",
                conv.introspection_cadence.target_text(),
            );
            conv.push_receipt("INTROSPECTION_CADENCE", vec![message.clone()]);
            conv.emphasis = Some(message.clone());
            Some(
                NextActionOutcome::handled("introspection_cadence", message)
                    .with_stage_visibility("self_direction", "protected_summary"),
            )
        },
    }
}

pub(in crate::autonomous) fn note_astrid_action(conv: &mut ConversationState) {
    conv.introspection_cadence
        .last_astrid_action_completed_exchange = Some(conv.exchange_count.saturating_add(1));
}

pub(in crate::autonomous) fn observe_due(conv: &mut ConversationState) -> bool {
    if !flush_lifecycle_debt(&mut conv.introspection_cadence) {
        return false;
    }
    let exchange = conv.exchange_count;
    let mut observed = conv.introspection_cadence.clone();
    let new_hold_reason = observed.observe_pilot_runtime_at(unix_now_ms());
    if let Some(reason) = new_hold_reason {
        set_last_outcome(
            &mut observed,
            IntrospectionCadenceEventV1::Deferred,
            exchange,
            Some(reason.to_string()),
            None,
            None,
        );
        let debt = lifecycle_debt(
            IntrospectionCadenceEventV1::Deferred,
            exchange,
            "pilot_bound",
            Some(reason),
            None,
            None,
            None,
        );
        record_or_defer_lifecycle(&mut observed, debt);
        conv.introspection_cadence = observed;
        return false;
    }
    conv.introspection_cadence = observed;
    let due_exchange = match conv.introspection_cadence.next_due_exchange() {
        Some(due_exchange) => due_exchange,
        None => return false,
    };
    if conv.introspection_cadence.pending_since_exchange.is_some() {
        return true;
    }
    if exchange < due_exchange {
        return false;
    }
    let mut cadence = conv.introspection_cadence.clone();
    cadence.pending_since_exchange = Some(due_exchange);
    set_last_outcome(
        &mut cadence,
        IntrospectionCadenceEventV1::Due,
        exchange,
        Some("configured_interval_elapsed".to_string()),
        None,
        None,
    );
    if record_lifecycle(
        &cadence,
        IntrospectionCadenceEventV1::Due,
        exchange,
        "scheduler",
        Some("configured_interval_elapsed"),
        None,
        None,
        None,
    )
    .is_err()
    {
        return false;
    }
    conv.introspection_cadence = cadence;
    true
}

pub(in crate::autonomous) fn defer_pending(conv: &mut ConversationState, reason: &str) -> bool {
    if !flush_lifecycle_debt(&mut conv.introspection_cadence) {
        return true;
    }
    if conv.introspection_cadence.pending_since_exchange.is_none() {
        return false;
    }
    let exchange = conv.exchange_count;
    if conv.introspection_cadence.last_deferred_exchange == Some(exchange)
        && conv.introspection_cadence.last_deferred_reason.as_deref() == Some(reason)
    {
        return true;
    }
    let mut cadence = conv.introspection_cadence.clone();
    cadence.last_deferred_exchange = Some(exchange);
    cadence.last_deferred_reason = Some(reason.to_string());
    set_last_outcome(
        &mut cadence,
        IntrospectionCadenceEventV1::Deferred,
        exchange,
        Some(reason.to_string()),
        None,
        None,
    );
    if record_lifecycle(
        &cadence,
        IntrospectionCadenceEventV1::Deferred,
        exchange,
        "scheduler",
        Some(reason),
        None,
        None,
        None,
    )
    .is_ok()
    {
        conv.introspection_cadence = cadence;
    }
    true
}

fn explicit_action_deferral_reason(conv: &ConversationState) -> Option<&'static str> {
    if conv
        .introspection_cadence
        .last_astrid_action_completed_exchange
        == Some(conv.exchange_count)
    {
        return Some("newer_astrid_next_action");
    }
    if conv.wants_search || conv.search_topic.is_some() {
        return Some("pending_authored_search");
    }
    if conv.browse_url.is_some() {
        return Some("pending_authored_browse");
    }
    if conv.form_constraint.is_some() || conv.revise_keyword.is_some() {
        return Some("pending_authored_creation");
    }
    if conv.wants_decompose || conv.wants_spectral_explorer || conv.force_all_viz {
        return Some("pending_authored_spectral_read");
    }
    if conv.wants_deep_think {
        return Some("pending_authored_deep_think");
    }
    if conv.wants_look
        || conv.wants_compose_audio
        || conv.wants_analyze_audio
        || conv.wants_render_audio.is_some()
    {
        return Some("pending_authored_sensory_or_audio_action");
    }
    None
}

pub(in crate::autonomous) fn try_select_due(conv: &mut ConversationState) -> bool {
    if !observe_due(conv) {
        return false;
    }
    if conv.introspection_cadence_attempt.is_some() {
        defer_pending(conv, "cadence_attempt_already_selected");
        return false;
    }
    if let Some(reason) = explicit_action_deferral_reason(conv) {
        defer_pending(conv, reason);
        return false;
    }
    let Some(pending_since_exchange) = conv.introspection_cadence.pending_since_exchange else {
        return false;
    };
    let target = conv.introspection_cadence.target.clone();
    let now_ms = unix_now_ms();
    let attempt_id = format!(
        "introspection-cadence-v1-{pending_since_exchange}-{}-{now_ms}",
        conv.exchange_count,
    );
    let mut cadence = conv.introspection_cadence.clone();
    cadence.last_attempt_exchange = Some(conv.exchange_count);
    cadence.pilot_cadence_starts = cadence.pilot_cadence_starts.saturating_add(1);
    if cadence.pilot_cadence_starts >= PILOT_MAX_STARTS {
        cadence.pilot_hold_reason = Some("pilot_eight_start_review_hold".to_string());
        cadence.pilot_last_observed_unix_ms = None;
    }
    set_last_outcome(
        &mut cadence,
        IntrospectionCadenceEventV1::Attempted,
        conv.exchange_count,
        Some("cadence_mode_execution_started".to_string()),
        Some(attempt_id.clone()),
        None,
    );
    if record_lifecycle(
        &cadence,
        IntrospectionCadenceEventV1::Attempted,
        conv.exchange_count,
        "scheduler",
        Some("cadence_mode_execution_started"),
        Some(&attempt_id),
        None,
        None,
    )
    .is_err()
    {
        return false;
    }
    conv.introspect_target = target.as_ref().map(|target| match target.offset {
        Some(offset) => IntrospectTargetV2::exact(target.label.clone(), offset),
        None => IntrospectTargetV2::auto(target.label.clone()),
    });
    conv.introspection_cadence_attempt = Some(IntrospectionCadenceAttemptV1 {
        attempt_id,
        pending_since_exchange,
        target,
        started: false,
    });
    conv.introspection_cadence = cadence;
    true
}

pub(in crate::autonomous) fn begin_attempt(
    conv: &mut ConversationState,
) -> Option<IntrospectionCadenceAttemptV1> {
    let attempt = conv.introspection_cadence_attempt.as_mut()?;
    if attempt.started {
        return Some(attempt.clone());
    }
    attempt.started = true;
    Some(attempt.clone())
}

pub(in crate::autonomous) fn mark_failed(
    conv: &mut ConversationState,
    reason: impl Into<String>,
    artifact_path: Option<&std::path::Path>,
) {
    let Some(attempt) = conv.introspection_cadence_attempt.take() else {
        return;
    };
    let reason = reason.into();
    let artifact = artifact_path.map(|path| path.display().to_string());
    let mut cadence = conv.introspection_cadence.clone();
    cadence.pending_since_exchange = Some(attempt.pending_since_exchange);
    set_last_outcome(
        &mut cadence,
        IntrospectionCadenceEventV1::Failed,
        conv.exchange_count,
        Some(reason.clone()),
        Some(attempt.attempt_id.clone()),
        artifact.clone(),
    );
    let debt = lifecycle_debt(
        IntrospectionCadenceEventV1::Failed,
        conv.exchange_count,
        "scheduler",
        Some(&reason),
        Some(&attempt.attempt_id),
        artifact.as_deref(),
        None,
    );
    record_or_defer_lifecycle(&mut cadence, debt);
    conv.introspection_cadence = cadence;
}

pub(in crate::autonomous) fn mark_admitted(
    conv: &mut ConversationState,
    artifact_path: &std::path::Path,
    witness_outcome: &str,
) {
    let Some(attempt) = conv.introspection_cadence_attempt.take() else {
        return;
    };
    let artifact = artifact_path.display().to_string();
    let mut cadence = conv.introspection_cadence.clone();
    cadence.pending_since_exchange = None;
    cadence.anchor_completed_exchange = Some(conv.exchange_count.saturating_add(1));
    cadence.last_admitted_exchange = Some(conv.exchange_count);
    cadence.last_deferred_exchange = None;
    cadence.last_deferred_reason = None;
    set_last_outcome(
        &mut cadence,
        IntrospectionCadenceEventV1::Admitted,
        conv.exchange_count,
        Some("canonical_introspection_persisted".to_string()),
        Some(attempt.attempt_id.clone()),
        Some(artifact.clone()),
    );
    let debt = lifecycle_debt(
        IntrospectionCadenceEventV1::Admitted,
        conv.exchange_count,
        "scheduler",
        Some("canonical_introspection_persisted"),
        Some(&attempt.attempt_id),
        Some(&artifact),
        Some(witness_outcome),
    );
    record_or_defer_lifecycle(&mut cadence, debt);
    conv.introspection_cadence = cadence;
}

#[cfg(test)]
#[path = "introspection_cadence_tests.rs"]
mod tests;
