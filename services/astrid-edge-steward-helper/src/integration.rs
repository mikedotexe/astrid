//! Steward-owned evidence-integration scheduling.
//!
//! The mutable runtime may announce exact v7 pending evidence, but that input
//! grants reflection only. The immutable helper derives the trigger nonce,
//! enforces cadence/quotas, and never treats timestamp proximity as identity or
//! causation.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::util::{
    atomic_private_write, bounded_text, canonical_json, read_stable_regular, sha256,
    validate_hex64, validate_identifier,
};
use crate::{Error, Result};

const STATE_SCHEMA: &str = "astrid.edge.steward_helper.evidence_integration_state.v1";
const SOURCE_SCHEMA: &str = "astrid_edge_thread_state_v7";
const QUIET_MS: u64 = 5 * 60 * 1_000;
const MINIMUM_INTERVAL_MS: u64 = 60 * 60 * 1_000;
const DAY_MS: u64 = 24 * 60 * 60 * 1_000;
const MAX_STARTS_PER_DAY: u8 = 12;
const MAX_PENDING_IDS: usize = 128;
const MAX_EVIDENCE_RECORDS: usize = 128;
const MAX_TRIGGER_EVIDENCE: usize = 6;
const PROMPT_REFERENCE_CHARS: usize = 192;
const PROMPT_SUMMARY_CHARS: usize = 240;
const MAX_CONSUMED: usize = 65_536;
const MAX_REJECTED: usize = 4_096;
const MAX_AMBIGUOUS: usize = 4_096;
const MAX_STATE_BYTES: u64 = 16 * 1_024 * 1_024;
const TRIGGER_DOMAIN: &[u8] = b"astrid.edge.evidence-integration.trigger.v1\0";

const ELIGIBLE_KINDS: &[&str] = &[
    "verified_source",
    "deterministic_measurement",
    "deterministic_check",
    "completed_study",
    "spectral_observation",
    "reservoir_tuning_result",
    "owned_self_study_result",
    "owned_artifact_read",
    "cited_synthesis",
    "verified_peer_packet",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EvidenceRecord {
    pub evidence_id: String,
    pub kind: String,
    pub epistemic_status: String,
    pub reference: String,
    pub summary: String,
    pub source: String,
    pub captured_at_unix_ms: u64,
    pub sha256: String,
    pub eligible_for_belief_update: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveTrigger {
    trigger_nonce: String,
    due_nonce: String,
    generation: u64,
    created_at_unix_ms: u64,
    last_attempt_at_unix_ms: Option<u64>,
    evidence: Vec<EvidenceRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsumedEvidence {
    evidence_id: String,
    sha256: String,
    consumed_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RejectedEvidence {
    record_sha256: String,
    evidence_id: Option<String>,
    reason: String,
    source_revision: u64,
    first_seen_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AmbiguousTrigger {
    trigger_nonce: String,
    due_nonce: String,
    provider_started_at_unix_ms: u64,
    terminalized_at_unix_ms: u64,
    evidence: Vec<ConsumedEvidence>,
    status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScheduledAbsorption {
    scheduled_nonce: String,
    prepared_at_unix_ms: u64,
    evidence: Vec<EvidenceRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IntegrationState {
    schema: String,
    generation: u64,
    pending: Vec<EvidenceRecord>,
    quiet_until_unix_ms: u64,
    active: Option<ActiveTrigger>,
    consumed: Vec<ConsumedEvidence>,
    rejected: Vec<RejectedEvidence>,
    ambiguous: Vec<AmbiguousTrigger>,
    scheduled_absorption: Option<ScheduledAbsorption>,
    last_completed_at_unix_ms: Option<u64>,
    last_finished_trigger_nonce: Option<String>,
    last_finished_due_nonce: Option<String>,
    last_absorbed_scheduled_nonce: Option<String>,
    utc_day: u64,
    starts_today: u8,
    last_source_revision: Option<u64>,
    last_source_sha256: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(clippy::struct_field_names)] // Distinguishes steward trigger identity from the due-slot nonce.
pub(crate) struct Trigger {
    pub trigger_nonce: String,
    pub due_nonce: String,
    pub evidence: Vec<EvidenceRecord>,
}

impl Trigger {
    pub(crate) fn question(&self) -> String {
        format!(
            "What do these {} newly verified evidence records change in my current inquiry?",
            self.evidence.len()
        )
    }

    pub(crate) fn prompt_projection(&self) -> Value {
        serde_json::json!({
            "schema": "astrid.edge.evidence_integration.trigger_projection.v1",
            "trigger_nonce": self.trigger_nonce,
            "evidence": self.evidence.iter().map(prompt_record_projection).collect::<Vec<_>>(),
            "authority": "runtime_announces_exact_evidence_steward_grants_reflection_only"
        })
    }
}

/// Return the exact retained trigger regardless of its retry floor.
///
/// This is used only to resume an already-prepared authored transaction after
/// a restart. A new provider call must still pass [`active`] or [`consider`].
pub(crate) fn retained(config: &Config, now: u64) -> Result<Option<Trigger>> {
    let state = load(config, now)?;
    Ok(state.active.as_ref().map(|active| Trigger {
        trigger_nonce: active.trigger_nonce.clone(),
        due_nonce: active.due_nonce.clone(),
        evidence: active.evidence.clone(),
    }))
}

/// Recover the last finalized trigger identity when a crash left its signed
/// prepared transaction behind after integration state was advanced.
pub(crate) fn last_finished(config: &Config, now: u64) -> Result<Option<Trigger>> {
    let state = load(config, now)?;
    match (
        state.last_finished_trigger_nonce,
        state.last_finished_due_nonce,
    ) {
        (Some(trigger_nonce), Some(due_nonce)) => Ok(Some(Trigger {
            trigger_nonce,
            due_nonce,
            evidence: Vec::new(),
        })),
        (None, None) => Ok(None),
        _ => Err(Error::new(
            "last finished integration recovery binding is partial",
        )),
    }
}

pub(crate) fn provider_started(config: &Config, trigger_nonce: &str, now: u64) -> Result<bool> {
    let state = load(config, now)?;
    Ok(state.active.as_ref().is_some_and(|active| {
        active.trigger_nonce == trigger_nonce && active.last_attempt_at_unix_ms.is_some()
    }))
}

#[derive(Debug, Clone)]
pub(crate) enum Decision {
    None,
    Deferred {
        status: &'static str,
        until_unix_ms: u64,
        due_nonce: String,
    },
    Due(Trigger),
}

impl IntegrationState {
    fn initial(now: u64) -> Self {
        Self {
            schema: STATE_SCHEMA.to_owned(),
            generation: 0,
            pending: Vec::new(),
            quiet_until_unix_ms: 0,
            active: None,
            consumed: Vec::new(),
            rejected: Vec::new(),
            ambiguous: Vec::new(),
            scheduled_absorption: None,
            last_completed_at_unix_ms: None,
            last_finished_trigger_nonce: None,
            last_finished_due_nonce: None,
            last_absorbed_scheduled_nonce: None,
            utc_day: utc_day(now),
            starts_today: 0,
            last_source_revision: None,
            last_source_sha256: None,
        }
    }

    fn roll_day(&mut self, now: u64) {
        let day = utc_day(now);
        if self.utc_day != day {
            self.utc_day = day;
            self.starts_today = 0;
        }
    }

    #[allow(clippy::too_many_lines)] // One exact fail-closed validator for the durable state schema.
    fn validate(&self, config: &Config, now: u64) -> Result<()> {
        if self.schema != STATE_SCHEMA
            || self.pending.len() > MAX_PENDING_IDS
            || self.consumed.len() > MAX_CONSUMED
            || self.rejected.len() > MAX_REJECTED
            || self.ambiguous.len() > MAX_AMBIGUOUS
            || self.starts_today > MAX_STARTS_PER_DAY
            || self.utc_day > utc_day(now).saturating_add(1)
        {
            return Err(Error::new("evidence integration state bounds are invalid"));
        }
        let mut identities = BTreeMap::new();
        for record in &self.pending {
            validate_record(record, now)?;
            insert_identity(&mut identities, &record.evidence_id, &record.sha256)?;
        }
        if let Some(active) = &self.active {
            validate_identifier(&active.trigger_nonce, "integration trigger nonce")?;
            validate_due_nonce(&active.due_nonce)?;
            if active.evidence.is_empty()
                || active.evidence.len() > MAX_TRIGGER_EVIDENCE
                || active.created_at_unix_ms == 0
                || active.created_at_unix_ms > now.saturating_add(60_000)
                || active
                    .last_attempt_at_unix_ms
                    .is_some_and(|value| value > now.saturating_add(60_000))
            {
                return Err(Error::new("active evidence integration trigger is invalid"));
            }
            for record in &active.evidence {
                validate_record(record, now)?;
                insert_identity(&mut identities, &record.evidence_id, &record.sha256)?;
            }
            let expected = derive_trigger(config, active.generation, &active.evidence)?;
            if active.trigger_nonce != expected.trigger_nonce
                || active.due_nonce != expected.due_nonce
            {
                return Err(Error::new("active integration trigger derivation failed"));
            }
        }
        let mut consumed_ids = BTreeSet::new();
        for record in &self.consumed {
            validate_short_id(&record.evidence_id, "consumed evidence id")?;
            validate_hex64(&record.sha256, "consumed evidence hash")?;
            if record.consumed_at_unix_ms == 0
                || record.consumed_at_unix_ms > now.saturating_add(60_000)
                || !consumed_ids.insert(&record.evidence_id)
                || identities.contains_key(&record.evidence_id)
            {
                return Err(Error::new("consumed evidence state is invalid"));
            }
            insert_identity(&mut identities, &record.evidence_id, &record.sha256)?;
        }
        for record in &self.rejected {
            validate_hex64(&record.record_sha256, "rejected evidence record hash")?;
            if let Some(evidence_id) = &record.evidence_id {
                validate_short_id(evidence_id, "rejected evidence id")?;
            }
            if record.reason.is_empty()
                || record.reason.chars().count() > 160
                || record.reason.chars().any(char::is_control)
                || record.source_revision == 0
                || record.first_seen_at_unix_ms == 0
                || record.first_seen_at_unix_ms > now.saturating_add(60_000)
            {
                return Err(Error::new("rejected evidence state is invalid"));
            }
        }
        let mut ambiguous_triggers = BTreeSet::new();
        for record in &self.ambiguous {
            validate_identifier(&record.trigger_nonce, "ambiguous integration trigger nonce")?;
            validate_due_nonce(&record.due_nonce)?;
            if record.provider_started_at_unix_ms == 0
                || record.terminalized_at_unix_ms < record.provider_started_at_unix_ms
                || record.terminalized_at_unix_ms > now.saturating_add(60_000)
                || record.evidence.is_empty()
                || record.evidence.len() > MAX_TRIGGER_EVIDENCE
                || record.status != "provider_started_delivery_authorship_unknown_non_authored"
                || !ambiguous_triggers.insert(&record.trigger_nonce)
            {
                return Err(Error::new("ambiguous integration state is invalid"));
            }
            for evidence in &record.evidence {
                validate_short_id(&evidence.evidence_id, "ambiguous evidence id")?;
                validate_hex64(&evidence.sha256, "ambiguous evidence hash")?;
                if evidence.consumed_at_unix_ms != record.terminalized_at_unix_ms {
                    return Err(Error::new("ambiguous evidence timestamp is invalid"));
                }
                insert_identity(&mut identities, &evidence.evidence_id, &evidence.sha256)?;
            }
        }
        if let Some(snapshot) = &self.scheduled_absorption {
            validate_identifier(
                &snapshot.scheduled_nonce,
                "scheduled absorption snapshot nonce",
            )?;
            if snapshot.prepared_at_unix_ms == 0
                || snapshot.prepared_at_unix_ms > now.saturating_add(60_000)
                || snapshot.evidence.len() > MAX_TRIGGER_EVIDENCE
            {
                return Err(Error::new(
                    "scheduled absorption snapshot bounds are invalid",
                ));
            }
            for record in &snapshot.evidence {
                validate_record(record, now)?;
                if identities.get(&record.evidence_id).copied() != Some(&record.sha256) {
                    return Err(Error::new(
                        "scheduled absorption snapshot is not backed by pending evidence",
                    ));
                }
            }
        }
        for (value, label) in [
            (
                self.last_finished_trigger_nonce.as_deref(),
                "last finished integration trigger nonce",
            ),
            (
                self.last_absorbed_scheduled_nonce.as_deref(),
                "last absorbed scheduled nonce",
            ),
        ] {
            if let Some(value) = value {
                validate_identifier(value, label)?;
            }
        }
        if self.last_finished_trigger_nonce.is_some() != self.last_finished_due_nonce.is_some() {
            return Err(Error::new(
                "last finished integration trigger binding is partial",
            ));
        }
        if let Some(value) = &self.last_finished_due_nonce {
            validate_due_nonce(value)?;
        }
        if let Some(hash) = &self.last_source_sha256 {
            validate_hex64(hash, "integration source hash")?;
        }
        Ok(())
    }
}

/// Return an already-active trigger or its durable retry deferral.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn active(config: &Config, now: u64) -> Result<Option<Decision>> {
    let mut state = load(config, now)?;
    state.roll_day(now);
    let Some(trigger) = state.active.as_ref() else {
        save(config, &state, now)?;
        return Ok(None);
    };
    let decision = active_decision(&state, trigger, now);
    save(config, &state, now)?;
    Ok(Some(decision))
}

/// Refresh exact v7 evidence and promote at most one coalesced trigger.
pub(crate) fn consider(config: &Config, now: u64) -> Result<Decision> {
    let mut state = load(config, now)?;
    state.roll_day(now);
    refresh(config, &mut state, now)?;
    if let Some(trigger) = state.active.as_ref() {
        let decision = active_decision(&state, trigger, now);
        save(config, &state, now)?;
        return Ok(decision);
    }
    if state.pending.is_empty() {
        save(config, &state, now)?;
        return Ok(Decision::None);
    }
    if now < state.quiet_until_unix_ms {
        let due_nonce = preview_due_nonce(config, state.generation, &state.pending)?;
        let decision = Decision::Deferred {
            status: "evidence_integration_quiet_until",
            until_unix_ms: state.quiet_until_unix_ms,
            due_nonce,
        };
        save(config, &state, now)?;
        return Ok(decision);
    }
    if let Some(last) = state.last_completed_at_unix_ms {
        let eligible = last.saturating_add(MINIMUM_INTERVAL_MS);
        if now < eligible {
            let due_nonce = preview_due_nonce(config, state.generation, &state.pending)?;
            let decision = Decision::Deferred {
                status: "evidence_integration_minimum_interval_until",
                until_unix_ms: eligible,
                due_nonce,
            };
            save(config, &state, now)?;
            return Ok(decision);
        }
    }
    if state.starts_today >= MAX_STARTS_PER_DAY {
        let decision = Decision::Deferred {
            status: "evidence_integration_daily_limit_until",
            until_unix_ms: next_utc_day(now),
            due_nonce: preview_due_nonce(config, state.generation, &state.pending)?,
        };
        save(config, &state, now)?;
        return Ok(decision);
    }
    let take = state.pending.len().min(MAX_TRIGGER_EVIDENCE);
    let evidence = state.pending.drain(..take).collect::<Vec<_>>();
    let trigger = derive_trigger(config, state.generation, &evidence)?;
    state.active = Some(ActiveTrigger {
        trigger_nonce: trigger.trigger_nonce.clone(),
        due_nonce: trigger.due_nonce.clone(),
        generation: state.generation,
        created_at_unix_ms: now,
        last_attempt_at_unix_ms: None,
        evidence,
    });
    save(config, &state, now)?;
    Ok(Decision::Due(trigger))
}

/// Consume one quota slot immediately before the first provider request.
pub(crate) fn begin_attempt(config: &Config, trigger_nonce: &str, now: u64) -> Result<()> {
    let mut state = load(config, now)?;
    state.roll_day(now);
    let active = state
        .active
        .as_mut()
        .ok_or_else(|| Error::new("integration attempt has no active trigger"))?;
    if active.trigger_nonce != trigger_nonce
        || state.starts_today >= MAX_STARTS_PER_DAY
        || active
            .last_attempt_at_unix_ms
            .is_some_and(|last| now < last.saturating_add(MINIMUM_INTERVAL_MS))
    {
        return Err(Error::new("integration attempt cadence or identity failed"));
    }
    active.last_attempt_at_unix_ms = Some(now);
    state.starts_today = state.starts_today.saturating_add(1);
    save(config, &state, now)
}

/// Terminalize a provider-started trigger when no signed authored transaction
/// exists. The exact evidence fingerprints remain quarantined permanently, so
/// neither a later poll nor v7 projection churn can retry the ambiguous call.
pub(crate) fn terminalize_started_unknown(
    config: &Config,
    trigger_nonce: &str,
    now: u64,
) -> Result<bool> {
    let mut state = load(config, now)?;
    state.roll_day(now);
    if state
        .ambiguous
        .iter()
        .any(|record| record.trigger_nonce == trigger_nonce)
    {
        return Ok(true);
    }
    let Some(active) = state.active.as_ref() else {
        return Ok(false);
    };
    if active.trigger_nonce != trigger_nonce {
        return Err(Error::new("ambiguous integration trigger mismatch"));
    }
    let Some(provider_started_at_unix_ms) = active.last_attempt_at_unix_ms else {
        return Ok(false);
    };
    let active = state
        .active
        .take()
        .ok_or_else(|| Error::new("provider-started trigger disappeared"))?;
    let evidence = active
        .evidence
        .into_iter()
        .map(|record| ConsumedEvidence {
            evidence_id: record.evidence_id,
            sha256: record.sha256,
            consumed_at_unix_ms: now,
        })
        .collect();
    state.ambiguous.push(AmbiguousTrigger {
        trigger_nonce: active.trigger_nonce,
        due_nonce: active.due_nonce,
        provider_started_at_unix_ms,
        terminalized_at_unix_ms: now,
        evidence,
        status: "provider_started_delivery_authorship_unknown_non_authored".to_owned(),
    });
    state.last_completed_at_unix_ms = Some(now);
    state.generation = state.generation.saturating_add(1);
    save(config, &state, now)?;
    Ok(true)
}

/// Consume only evidence cited by a structured integration.
///
/// `None` denotes an unstructured/non-authored terminal path and requeues the
/// entire trigger. `Some` is the exact signed inquiry declaration: uncited
/// evidence, including an entirely empty citation set, remains pending.
pub(crate) fn finish(
    config: &Config,
    trigger_nonce: &str,
    cited_evidence_ids: Option<&[String]>,
    now: u64,
) -> Result<()> {
    let mut state = load(config, now)?;
    state.roll_day(now);
    if state.last_finished_trigger_nonce.as_deref() == Some(trigger_nonce) && state.active.is_none()
    {
        return Ok(());
    }
    let active = state
        .active
        .take()
        .ok_or_else(|| Error::new("integration completion has no active trigger"))?;
    if active.trigger_nonce != trigger_nonce {
        return Err(Error::new("integration completion trigger mismatch"));
    }
    state.last_completed_at_unix_ms = Some(now);
    state.last_finished_trigger_nonce = Some(trigger_nonce.to_owned());
    state.last_finished_due_nonce = Some(active.due_nonce.clone());
    state.generation = state.generation.saturating_add(1);
    let (consumed, mut requeued) = match cited_evidence_ids {
        Some(evidence_ids) => partition_cited_evidence(active.evidence, evidence_ids)?,
        None => (Vec::new(), active.evidence),
    };
    if !consumed.is_empty() {
        consume(&mut state, consumed, now)?;
    }
    if !requeued.is_empty() {
        requeued.append(&mut state.pending);
        if requeued.len() > MAX_PENDING_IDS {
            return Err(Error::new("requeued evidence exceeds pending capacity"));
        }
        state.pending = requeued;
    }
    if cited_evidence_ids.is_none() {
        state.quiet_until_unix_ms = now.saturating_add(MINIMUM_INTERVAL_MS);
    }
    save(config, &state, now)
}

/// Durably bind the exact evidence visible to a regular scheduled reflection.
///
/// An empty snapshot is significant: evidence arriving after this point was not
/// visible to the model and must remain pending for a later integration.
pub(crate) fn prepare_scheduled(config: &Config, scheduled_nonce: &str, now: u64) -> Result<Value> {
    let mut state = load(config, now)?;
    state.roll_day(now);
    validate_identifier(scheduled_nonce, "scheduled absorption nonce")?;
    if let Some(snapshot) = &state.scheduled_absorption {
        if snapshot.scheduled_nonce != scheduled_nonce {
            return Err(Error::new(
                "another scheduled absorption snapshot remains active",
            ));
        }
        return scheduled_projection(snapshot);
    }
    refresh(config, &mut state, now)?;
    let evidence = state.active.as_ref().map_or_else(
        || {
            state
                .pending
                .iter()
                .take(MAX_TRIGGER_EVIDENCE)
                .cloned()
                .collect()
        },
        |active| active.evidence.clone(),
    );
    state.scheduled_absorption = Some(ScheduledAbsorption {
        scheduled_nonce: scheduled_nonce.to_owned(),
        prepared_at_unix_ms: now,
        evidence,
    });
    let projection = scheduled_projection(
        state
            .scheduled_absorption
            .as_ref()
            .ok_or_else(|| Error::new("scheduled absorption snapshot disappeared"))?,
    )?;
    save(config, &state, now)?;
    Ok(projection)
}

/// A structured regular reflection absorbs only cited members of its exact
/// pre-prompt snapshot. Uncited members remain pending.
pub(crate) fn absorb_scheduled(
    config: &Config,
    scheduled_nonce: &str,
    cited_evidence_ids: &[String],
    now: u64,
) -> Result<usize> {
    let mut state = load(config, now)?;
    state.roll_day(now);
    validate_identifier(scheduled_nonce, "scheduled absorption nonce")?;
    if state.last_absorbed_scheduled_nonce.as_deref() == Some(scheduled_nonce) {
        return Ok(0);
    }
    let snapshot = state
        .scheduled_absorption
        .take()
        .ok_or_else(|| Error::new("scheduled absorption snapshot is absent"))?;
    if snapshot.scheduled_nonce != scheduled_nonce {
        return Err(Error::new("scheduled absorption snapshot nonce mismatch"));
    }
    let owner = snapshot_owner(&state, &snapshot.evidence)?;
    let (evidence, mut uncited) = partition_cited_evidence(snapshot.evidence, cited_evidence_ids)?;
    let had_uncited = !uncited.is_empty();
    match owner {
        SnapshotOwner::Empty => {},
        SnapshotOwner::Active => {
            state.active = None;
            uncited.append(&mut state.pending);
            if uncited.len() > MAX_PENDING_IDS {
                return Err(Error::new(
                    "requeued scheduled evidence exceeds pending capacity",
                ));
            }
            state.pending = uncited;
        },
        SnapshotOwner::Pending => {
            state.pending.retain(|record| {
                !evidence.iter().any(|cited| {
                    record.evidence_id == cited.evidence_id && record.sha256 == cited.sha256
                })
            });
        },
    }
    let count = evidence.len();
    if count > 0 {
        consume(&mut state, evidence, now)?;
        state.last_completed_at_unix_ms = Some(now);
        state.generation = state.generation.saturating_add(1);
        state.quiet_until_unix_ms = 0;
    } else if had_uncited || !state.pending.is_empty() {
        state.quiet_until_unix_ms = now.saturating_add(MINIMUM_INTERVAL_MS);
    }
    state.last_absorbed_scheduled_nonce = Some(scheduled_nonce.to_owned());
    save(config, &state, now)?;
    Ok(count)
}

/// Release a scheduled snapshot after exact but unstructured model authorship.
/// The evidence remains pending because no inquiry admission may occur.
pub(crate) fn release_scheduled(config: &Config, scheduled_nonce: &str, now: u64) -> Result<bool> {
    let mut state = load(config, now)?;
    state.roll_day(now);
    validate_identifier(scheduled_nonce, "scheduled absorption nonce")?;
    let Some(snapshot) = state.scheduled_absorption.as_ref() else {
        return Ok(false);
    };
    if snapshot.scheduled_nonce != scheduled_nonce {
        return Err(Error::new("scheduled absorption snapshot nonce mismatch"));
    }
    state.scheduled_absorption = None;
    save(config, &state, now)?;
    Ok(true)
}

fn scheduled_projection(snapshot: &ScheduledAbsorption) -> Result<Value> {
    let projection = serde_json::json!({
        "schema": "astrid.edge.evidence_integration.scheduled_projection.v1",
        "scheduled_nonce": snapshot.scheduled_nonce,
        "evidence": snapshot.evidence.iter().map(prompt_record_projection).collect::<Vec<_>>(),
        "authority": "pre_prompt_exact_evidence_snapshot_reflection_only"
    });
    // Keep the projection canonicalizable here so prompt binding cannot fail
    // only after the durable snapshot has been accepted.
    canonical_json(&projection)?;
    Ok(projection)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SnapshotOwner {
    Empty,
    Active,
    Pending,
}

fn snapshot_owner(state: &IntegrationState, snapshot: &[EvidenceRecord]) -> Result<SnapshotOwner> {
    let mut owner = None;
    for expected in snapshot {
        let active_match = state.active.as_ref().is_some_and(|active| {
            active.evidence.iter().any(|record| {
                record.evidence_id == expected.evidence_id && record.sha256 == expected.sha256
            })
        });
        let pending_match = state.pending.iter().any(|record| {
            record.evidence_id == expected.evidence_id && record.sha256 == expected.sha256
        });
        if active_match == pending_match {
            return Err(Error::new(
                "scheduled evidence is absent or has ambiguous ownership",
            ));
        }
        let record_owner = if active_match {
            SnapshotOwner::Active
        } else {
            SnapshotOwner::Pending
        };
        if owner.is_some_and(|existing| existing != record_owner) {
            return Err(Error::new(
                "scheduled evidence snapshot has mixed ownership",
            ));
        }
        owner = Some(record_owner);
    }
    if owner == Some(SnapshotOwner::Active) {
        let active = state
            .active
            .as_ref()
            .ok_or_else(|| Error::new("scheduled active evidence disappeared"))?;
        if active.evidence.len() != snapshot.len()
            || !active.evidence.iter().all(|record| {
                snapshot.iter().any(|expected| {
                    record.evidence_id == expected.evidence_id && record.sha256 == expected.sha256
                })
            })
        {
            return Err(Error::new(
                "scheduled absorption cannot partially consume an active trigger",
            ));
        }
    }
    Ok(owner.unwrap_or(SnapshotOwner::Empty))
}

fn partition_cited_evidence(
    evidence: Vec<EvidenceRecord>,
    cited_evidence_ids: &[String],
) -> Result<(Vec<EvidenceRecord>, Vec<EvidenceRecord>)> {
    if cited_evidence_ids.len() > MAX_TRIGGER_EVIDENCE {
        return Err(Error::new(
            "inquiry evidence citation count exceeds the bound",
        ));
    }
    let mut citations = BTreeSet::new();
    for evidence_id in cited_evidence_ids {
        validate_short_id(evidence_id, "inquiry evidence citation")?;
        if !citations.insert(evidence_id.as_str()) {
            return Err(Error::new("inquiry evidence citation is duplicated"));
        }
        if !evidence
            .iter()
            .any(|record| record.evidence_id == evidence_id.as_str())
        {
            return Err(Error::new(
                "inquiry evidence citation is outside the exact prompt snapshot",
            ));
        }
    }
    let (consumed, pending) = evidence
        .into_iter()
        .partition(|record| citations.contains(record.evidence_id.as_str()));
    Ok((consumed, pending))
}

fn prompt_record_projection(record: &EvidenceRecord) -> Value {
    serde_json::json!({
        "evidence_id": record.evidence_id,
        "kind": record.kind,
        "epistemic_status": record.epistemic_status,
        "captured_at_unix_ms": record.captured_at_unix_ms,
        "sha256": record.sha256,
        "source": record.source,
        "reference": bounded_text(&record.reference, PROMPT_REFERENCE_CHARS),
        "reference_truncated": record.reference.chars().count() > PROMPT_REFERENCE_CHARS,
        "summary": bounded_text(&record.summary, PROMPT_SUMMARY_CHARS),
        "summary_truncated": record.summary.chars().count() > PROMPT_SUMMARY_CHARS
    })
}

fn active_decision(state: &IntegrationState, active: &ActiveTrigger, now: u64) -> Decision {
    if let Some(last) = active.last_attempt_at_unix_ms {
        let eligible = last.saturating_add(MINIMUM_INTERVAL_MS);
        if now < eligible {
            return Decision::Deferred {
                status: "evidence_integration_retry_floor_until",
                until_unix_ms: eligible,
                due_nonce: active.due_nonce.clone(),
            };
        }
    }
    if state.starts_today >= MAX_STARTS_PER_DAY {
        return Decision::Deferred {
            status: "evidence_integration_daily_limit_until",
            until_unix_ms: next_utc_day(now),
            due_nonce: active.due_nonce.clone(),
        };
    }
    Decision::Due(Trigger {
        trigger_nonce: active.trigger_nonce.clone(),
        due_nonce: active.due_nonce.clone(),
        evidence: active.evidence.clone(),
    })
}

#[allow(clippy::too_many_lines)] // Independent record rejection and exact-ID reconciliation stay atomic.
fn refresh(config: &Config, state: &mut IntegrationState, now: u64) -> Result<bool> {
    let Some((source, source_sha256)) = load_source(config, now)? else {
        return Ok(false);
    };
    let mut raw_records = BTreeMap::new();
    let mut duplicate_ids = BTreeSet::new();
    for raw in source.evidence_records {
        let evidence_id = raw
            .get("evidence_id")
            .and_then(Value::as_str)
            .filter(|value| validate_short_id(value, "evidence id").is_ok())
            .map(str::to_owned);
        let Some(evidence_id) = evidence_id else {
            record_rejection(
                state,
                &raw,
                None,
                "malformed_or_missing_evidence_id",
                source.revision,
                now,
            )?;
            continue;
        };
        if raw_records.insert(evidence_id.clone(), raw).is_some() {
            duplicate_ids.insert(evidence_id);
        }
    }
    let mut records = BTreeMap::new();
    for (evidence_id, raw) in raw_records {
        if duplicate_ids.contains(&evidence_id) {
            record_rejection(
                state,
                &raw,
                Some(evidence_id),
                "duplicate_evidence_id",
                source.revision,
                now,
            )?;
            continue;
        }
        let Ok(record) = serde_json::from_value::<EvidenceRecord>(raw.clone()) else {
            record_rejection(
                state,
                &raw,
                Some(evidence_id),
                "malformed_evidence_record",
                source.revision,
                now,
            )?;
            continue;
        };
        if validate_record(&record, now).is_err() {
            record_rejection(
                state,
                &raw,
                Some(evidence_id),
                "invalid_evidence_record",
                source.revision,
                now,
            )?;
            continue;
        }
        records.insert(record.evidence_id.clone(), record);
    }
    let mut pending_ids = BTreeSet::new();
    let mut changed = false;
    for raw_evidence_id in source.pending_evidence_ids {
        let Some(evidence_id) = raw_evidence_id.as_str().map(str::to_owned) else {
            record_rejection(
                state,
                &serde_json::json!({"pending_evidence_id": raw_evidence_id}),
                None,
                "malformed_pending_evidence_id",
                source.revision,
                now,
            )?;
            continue;
        };
        if validate_short_id(&evidence_id, "pending evidence id").is_err() {
            record_rejection(
                state,
                &serde_json::json!({"pending_evidence_id": evidence_id}),
                None,
                "malformed_pending_evidence_id",
                source.revision,
                now,
            )?;
            continue;
        }
        if !pending_ids.insert(evidence_id.clone()) {
            record_rejection(
                state,
                &serde_json::json!({"pending_evidence_id": evidence_id}),
                Some(evidence_id),
                "duplicate_pending_evidence_id",
                source.revision,
                now,
            )?;
            continue;
        }
        let Some(record) = records.get(&evidence_id) else {
            record_rejection(
                state,
                &serde_json::json!({"pending_evidence_id": evidence_id}),
                Some(evidence_id),
                "pending_evidence_lacks_valid_exact_record",
                source.revision,
                now,
            )?;
            continue;
        };
        if !record.eligible_for_belief_update || !ELIGIBLE_KINDS.contains(&record.kind.as_str()) {
            record_rejection(
                state,
                &serde_json::to_value(record)?,
                Some(evidence_id),
                "pending_evidence_is_ineligible",
                source.revision,
                now,
            )?;
            continue;
        }
        match identity_known(state, record) {
            Ok(true) => continue,
            Ok(false) => {},
            Err(_) => {
                record_rejection(
                    state,
                    &serde_json::to_value(record)?,
                    Some(evidence_id),
                    "evidence_identity_hash_replacement",
                    source.revision,
                    now,
                )?;
                continue;
            },
        }
        state.pending.push(record.clone());
        state.quiet_until_unix_ms = state
            .quiet_until_unix_ms
            .max(record.captured_at_unix_ms.saturating_add(QUIET_MS));
        changed = true;
    }
    state.last_source_revision = Some(source.revision);
    state.last_source_sha256 = Some(source_sha256);
    Ok(changed)
}

fn record_rejection(
    state: &mut IntegrationState,
    raw: &Value,
    evidence_id: Option<String>,
    reason: &str,
    source_revision: u64,
    now: u64,
) -> Result<()> {
    let record_sha256 = sha256(&canonical_json(raw)?);
    if state
        .rejected
        .iter()
        .any(|record| record.record_sha256 == record_sha256 && record.reason == reason)
    {
        return Ok(());
    }
    if state.rejected.len() >= MAX_REJECTED {
        return Err(Error::new(
            "evidence integration rejection capacity is exhausted",
        ));
    }
    state.rejected.push(RejectedEvidence {
        record_sha256,
        evidence_id,
        reason: reason.to_owned(),
        source_revision,
        first_seen_at_unix_ms: now,
    });
    Ok(())
}

fn identity_known(state: &IntegrationState, record: &EvidenceRecord) -> Result<bool> {
    for (known_id, known_hash) in state
        .pending
        .iter()
        .map(|value| (&value.evidence_id, &value.sha256))
        .chain(state.active.iter().flat_map(|active| {
            active
                .evidence
                .iter()
                .map(|value| (&value.evidence_id, &value.sha256))
        }))
        .chain(
            state
                .consumed
                .iter()
                .map(|value| (&value.evidence_id, &value.sha256)),
        )
        .chain(state.ambiguous.iter().flat_map(|trigger| {
            trigger
                .evidence
                .iter()
                .map(|value| (&value.evidence_id, &value.sha256))
        }))
    {
        if known_id == &record.evidence_id {
            if known_hash != &record.sha256 {
                return Err(Error::new("evidence ID was replaced with a new hash"));
            }
            return Ok(true);
        }
    }
    Ok(false)
}

fn consume(state: &mut IntegrationState, evidence: Vec<EvidenceRecord>, now: u64) -> Result<()> {
    for record in evidence {
        if let Some(existing) = state
            .consumed
            .iter()
            .find(|value| value.evidence_id == record.evidence_id)
        {
            if existing.sha256 != record.sha256 {
                return Err(Error::new("consumed evidence identity collision"));
            }
            continue;
        }
        state.consumed.push(ConsumedEvidence {
            evidence_id: record.evidence_id,
            sha256: record.sha256,
            consumed_at_unix_ms: now,
        });
    }
    if state.consumed.len() > MAX_CONSUMED {
        return Err(Error::new(
            "evidence integration fingerprint capacity is exhausted",
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct TriggerSource {
    pending_evidence_ids: Vec<Value>,
    evidence_records: Vec<Value>,
    updated_at_unix_ms: u64,
    revision: u64,
    event: String,
}

fn load_source(config: &Config, now: u64) -> Result<Option<(TriggerSource, String)>> {
    let path = continuity_path(config)?;
    if !path.exists() {
        if path.is_symlink() {
            return Err(Error::new(
                "v7 continuity trigger source is a broken symlink",
            ));
        }
        return Ok(None);
    }
    let input = config
        .owned_inputs
        .iter()
        .find(|input| input.kind == "continuity")
        .ok_or_else(|| Error::new("continuity input is absent"))?;
    let bytes = read_stable_regular(&path, input.maximum_bytes_per_file)?;
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return Ok(None);
    };
    if value.get("schema").and_then(Value::as_str) != Some(SOURCE_SCHEMA) {
        return Ok(None);
    }
    let object = value
        .as_object()
        .ok_or_else(|| Error::new("v7 continuity trigger source is not an object"))?;
    let source: TriggerSource = serde_json::from_value(serde_json::json!({
        "pending_evidence_ids": object.get("pending_evidence_ids"),
        "evidence_records": object.get("evidence_records"),
        "updated_at_unix_ms": object.get("updated_at_unix_ms"),
        "revision": object.get("revision"),
        "event": object.get("event")
    }))?;
    if source.pending_evidence_ids.len() > MAX_PENDING_IDS
        || source.evidence_records.len() > MAX_EVIDENCE_RECORDS
        || source.updated_at_unix_ms == 0
        || source.updated_at_unix_ms > now.saturating_add(60_000)
        || source.revision == 0
        || source.event.is_empty()
        || source.event.chars().count() > 192
        || source.event.chars().any(char::is_control)
    {
        return Err(Error::new("v7 continuity trigger metadata is invalid"));
    }
    Ok(Some((source, sha256(&bytes))))
}

fn continuity_path(config: &Config) -> Result<PathBuf> {
    let input = config
        .owned_inputs
        .iter()
        .find(|input| input.kind == "continuity")
        .ok_or_else(|| Error::new("continuity input is absent"))?;
    if input.path != config.workspace_root.join("autonomous/thread_state.json") {
        return Err(Error::new("continuity trigger path is not canonical"));
    }
    Ok(input.path.clone())
}

fn validate_record(record: &EvidenceRecord, now: u64) -> Result<()> {
    validate_short_id(&record.evidence_id, "evidence id")?;
    validate_identifier(&record.kind, "evidence kind")?;
    validate_identifier(&record.epistemic_status, "evidence epistemic status")?;
    validate_hex64(&record.sha256, "evidence hash")?;
    for (value, maximum, label) in [
        (&record.reference, 512, "evidence reference"),
        (&record.summary, 480, "evidence summary"),
        (&record.source, 256, "evidence source"),
    ] {
        if value.is_empty()
            || value.trim() != value
            || value.chars().count() > maximum
            || value.chars().any(char::is_control)
        {
            return Err(Error::new(format!("{label} is invalid")));
        }
    }
    if record.captured_at_unix_ms == 0 || record.captured_at_unix_ms > now.saturating_add(60_000) {
        return Err(Error::new("evidence capture timestamp is invalid"));
    }
    Ok(())
}

fn validate_short_id(value: &str, label: &str) -> Result<()> {
    if value.len() > 96 {
        return Err(Error::new(format!("invalid {label}")));
    }
    validate_identifier(value, label)
}

fn insert_identity<'a>(
    identities: &mut BTreeMap<&'a String, &'a String>,
    evidence_id: &'a String,
    hash: &'a String,
) -> Result<()> {
    if let Some(previous) = identities.insert(evidence_id, hash)
        && previous != hash
    {
        return Err(Error::new("evidence identity hash collision"));
    }
    Ok(())
}

fn derive_trigger(
    config: &Config,
    generation: u64,
    evidence: &[EvidenceRecord],
) -> Result<Trigger> {
    if evidence.is_empty() || evidence.len() > MAX_TRIGGER_EVIDENCE {
        return Err(Error::new("integration trigger evidence count is invalid"));
    }
    let mut preimage = TRIGGER_DOMAIN.to_vec();
    preimage.extend_from_slice(config.appliance_id.as_bytes());
    preimage.push(0);
    preimage.extend_from_slice(&generation.to_be_bytes());
    preimage.push(0);
    preimage.extend_from_slice(&canonical_json(&evidence.to_vec())?);
    let digest = sha256(&preimage);
    let numeric = u64::from_str_radix(&digest[..16], 16)
        .map_err(|_| Error::new("integration trigger digest is invalid"))?
        | 0x8000_0000_0000_0000;
    Ok(Trigger {
        trigger_nonce: format!("evidence-integration-{digest}"),
        due_nonce: format!("due-{numeric}"),
        evidence: evidence.to_vec(),
    })
}

fn preview_due_nonce(
    config: &Config,
    generation: u64,
    pending: &[EvidenceRecord],
) -> Result<String> {
    let take = pending.len().min(MAX_TRIGGER_EVIDENCE);
    Ok(derive_trigger(config, generation, &pending[..take])?.due_nonce)
}

fn validate_due_nonce(value: &str) -> Result<()> {
    validate_identifier(value, "integration due nonce")?;
    let suffix = value
        .strip_prefix("due-")
        .ok_or_else(|| Error::new("integration due nonce prefix is invalid"))?;
    if suffix.len() < 5 || suffix.len() > 20 || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Error::new("integration due nonce is invalid"));
    }
    Ok(())
}

fn load(config: &Config, now: u64) -> Result<IntegrationState> {
    let path = state_path(config);
    if !path.exists() {
        if path.is_symlink() {
            return Err(Error::new("evidence integration state is a broken symlink"));
        }
        return Ok(IntegrationState::initial(now));
    }
    let bytes = read_stable_regular(&path, MAX_STATE_BYTES)?;
    let state: IntegrationState = serde_json::from_slice(&bytes)?;
    if canonical_json(&state)? != bytes {
        return Err(Error::new("evidence integration state is not canonical"));
    }
    state.validate(config, now)?;
    Ok(state)
}

fn save(config: &Config, state: &IntegrationState, now: u64) -> Result<()> {
    state.validate(config, now)?;
    let bytes = canonical_json(state)?;
    if bytes.len() as u64 > MAX_STATE_BYTES {
        return Err(Error::new("evidence integration state exceeds its bound"));
    }
    atomic_private_write(&state_path(config), &bytes)
}

fn state_path(config: &Config) -> PathBuf {
    config.state_root.join("evidence-integration.json")
}

fn utc_day(now: u64) -> u64 {
    now.checked_div(DAY_MS).unwrap_or(0)
}

fn next_utc_day(now: u64) -> u64 {
    utc_day(now).saturating_add(1).saturating_mul(DAY_MS)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

    use super::{Decision, MINIMUM_INTERVAL_MS, QUIET_MS};
    use crate::config::{Config, GateConfig, OwnedInput};
    use crate::util::sha256;

    struct Fixture {
        _temporary: tempfile::TempDir,
        config: Config,
        continuity: std::path::PathBuf,
    }

    impl Fixture {
        fn new(now: u64) -> Self {
            let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
            let root = temporary.path();
            let workspace = root.join("workspace");
            let state = root.join("state");
            fs::create_dir_all(workspace.join("autonomous")).unwrap();
            fs::create_dir(&state).unwrap();
            fs::set_permissions(&state, fs::Permissions::from_mode(0o700)).unwrap();
            let continuity = workspace.join("autonomous/thread_state.json");
            let config = Config {
                schema: crate::CONFIG_SCHEMA.to_owned(),
                appliance_id: "avado-integration-test".to_owned(),
                target: "x86_64-unknown-linux-gnu".to_owned(),
                model: "test-model".to_owned(),
                ollama_origin: "http://127.0.0.1:11434".to_owned(),
                connect_timeout_ms: 1_000,
                header_timeout_ms: 2_000,
                total_timeout_ms: 3_000,
                provider_broker: None,
                web_broker: None,
                context_tokens: 1_024,
                output_tokens: 64,
                source_authoring_output_tokens: 64,
                model_lock: root.join("model.lock"),
                workspace_root: workspace.clone(),
                workspace_uid: nix::unistd::geteuid().as_raw(),
                workspace_gid: nix::unistd::getegid().as_raw(),
                source_root: root.join("source"),
                source_manifest: root.join("source/MANIFEST.json"),
                source_manifest_sha256: "a".repeat(64),
                source_signature: root.join("source/MANIFEST.signature.json"),
                expected_source_id: format!("cpu-edge:{}", "b".repeat(64)),
                source_signing_key: root.join("source.key"),
                source_signing_key_sha256: "c".repeat(64),
                attestor_key: root.join("intent.key"),
                attestor_key_sha256: "d".repeat(64),
                state_root: state,
                inquiry_history_root: root.join("candidate/inquiry-history"),
                supervisor_inbox: root.join("inbox"),
                supervisor_status: root.join("status"),
                current_generation: root.join("supervisor/current-generation"),
                active_generation_link: root.join("appliance/current"),
                maintenance_lease: root.join("supervisor/maintenance.json"),
                patch_export_root: workspace.join("self-change/patch-outbox"),
                owned_inputs: vec![OwnedInput {
                    kind: "continuity".to_owned(),
                    path: continuity.clone(),
                    maximum_files: 1,
                    maximum_bytes_per_file: 65_536,
                }],
                gates: GateConfig {
                    autonomy_state: root.join("autonomy.json"),
                    action_receipts: root.join("actions.jsonl"),
                    thermal_celsius: root.join("thermal"),
                    maximum_thermal_celsius: 90,
                },
            };
            let fixture = Self {
                _temporary: temporary,
                config,
                continuity,
            };
            fixture.write(now, &["evidence-a"]);
            fixture
        }

        fn write(&self, now: u64, pending: &[&str]) {
            let records = ["evidence-a", "evidence-b", "evidence-c"]
                .into_iter()
                .map(|evidence_id| {
                    serde_json::json!({
                        "evidence_id": evidence_id,
                        "kind": "completed_study",
                        "epistemic_status": "verified_machine_evidence",
                        "reference": format!("studies/{evidence_id}.json"),
                        "summary": format!("Bounded result for {evidence_id}."),
                        "source": "exact_action_parent_and_artifact_hash",
                        "captured_at_unix_ms": now,
                        "sha256": sha256(evidence_id.as_bytes()),
                        "eligible_for_belief_update": true
                    })
                })
                .collect::<Vec<_>>();
            fs::write(
                &self.continuity,
                serde_json::to_vec(&serde_json::json!({
                    "schema": "astrid_edge_thread_state_v7",
                    "pending_evidence_ids": pending,
                    "evidence_records": records,
                    "last_admitted_inquiry_step_id": null,
                    "last_inquiry_ledger_hash": null,
                    "updated_at_unix_ms": now,
                    "revision": 1,
                    "event": "evidence_arrival_completed_study_test"
                }))
                .unwrap(),
            )
            .unwrap();
        }

        fn state(&self) -> serde_json::Value {
            serde_json::from_slice(
                &fs::read(self.config.state_root.join("evidence-integration.json")).unwrap(),
            )
            .unwrap()
        }
    }

    #[test]
    fn quiet_coalescing_floor_and_structured_completion_are_durable() {
        let now = 1_800_000_000_000_u64;
        let fixture = Fixture::new(now);
        assert!(matches!(
            super::consider(&fixture.config, now).unwrap(),
            Decision::Deferred {
                status: "evidence_integration_quiet_until",
                ..
            }
        ));
        fixture.write(now.saturating_add(60_000), &["evidence-a", "evidence-b"]);
        assert!(matches!(
            super::consider(&fixture.config, now.saturating_add(60_000)).unwrap(),
            Decision::Deferred { .. }
        ));
        let due = now.saturating_add(60_000).saturating_add(QUIET_MS);
        let Decision::Due(trigger) = super::consider(&fixture.config, due).unwrap() else {
            panic!("coalesced evidence should become due");
        };
        assert_eq!(trigger.evidence.len(), 2);
        super::begin_attempt(&fixture.config, &trigger.trigger_nonce, due).unwrap();
        assert!(matches!(
            super::active(&fixture.config, due.saturating_add(1)).unwrap(),
            Some(Decision::Deferred {
                status: "evidence_integration_retry_floor_until",
                ..
            })
        ));
        let cited = trigger
            .evidence
            .iter()
            .map(|record| record.evidence_id.clone())
            .collect::<Vec<_>>();
        super::finish(
            &fixture.config,
            &trigger.trigger_nonce,
            Some(&cited),
            due.saturating_add(2),
        )
        .unwrap();
        assert!(
            super::active(&fixture.config, due.saturating_add(3))
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            super::consider(&fixture.config, due.saturating_add(MINIMUM_INTERVAL_MS)).unwrap(),
            Decision::None
        ));
    }

    #[test]
    fn structured_integration_consumes_only_exact_citations() {
        let now = 1_800_000_000_000_u64;
        let fixture = Fixture::new(now.saturating_sub(QUIET_MS));
        fixture.write(now.saturating_sub(QUIET_MS), &["evidence-a", "evidence-b"]);
        let Decision::Due(trigger) = super::consider(&fixture.config, now).unwrap() else {
            panic!("evidence should be due");
        };
        super::begin_attempt(&fixture.config, &trigger.trigger_nonce, now).unwrap();
        super::finish(
            &fixture.config,
            &trigger.trigger_nonce,
            Some(&["evidence-a".to_owned()]),
            now.saturating_add(1),
        )
        .unwrap();

        let state = fixture.state();
        assert_eq!(state["consumed"].as_array().unwrap().len(), 1);
        assert_eq!(state["consumed"][0]["evidence_id"], "evidence-a");
        assert_eq!(state["pending"].as_array().unwrap().len(), 1);
        assert_eq!(state["pending"][0]["evidence_id"], "evidence-b");

        let Decision::Due(next) = super::consider(
            &fixture.config,
            now.saturating_add(MINIMUM_INTERVAL_MS).saturating_add(1),
        )
        .unwrap() else {
            panic!("uncited evidence must remain pending");
        };
        assert_eq!(next.evidence.len(), 1);
        assert_eq!(next.evidence[0].evidence_id, "evidence-b");
    }

    #[test]
    fn empty_structured_citations_consume_nothing_and_foreign_ids_fail_closed() {
        let now = 1_800_000_000_000_u64;
        let empty = Fixture::new(now.saturating_sub(QUIET_MS));
        let Decision::Due(trigger) = super::consider(&empty.config, now).unwrap() else {
            panic!("evidence should be due");
        };
        super::begin_attempt(&empty.config, &trigger.trigger_nonce, now).unwrap();
        super::finish(
            &empty.config,
            &trigger.trigger_nonce,
            Some(&[]),
            now.saturating_add(1),
        )
        .unwrap();
        let state = empty.state();
        assert!(state["consumed"].as_array().unwrap().is_empty());
        assert_eq!(state["pending"][0]["evidence_id"], "evidence-a");

        let foreign = Fixture::new(now.saturating_sub(QUIET_MS));
        let Decision::Due(trigger) = super::consider(&foreign.config, now).unwrap() else {
            panic!("evidence should be due");
        };
        super::begin_attempt(&foreign.config, &trigger.trigger_nonce, now).unwrap();
        assert!(
            super::finish(
                &foreign.config,
                &trigger.trigger_nonce,
                Some(&["evidence-foreign".to_owned()]),
                now.saturating_add(1),
            )
            .is_err()
        );
        let state = foreign.state();
        assert!(state["active"].is_object());
        assert!(state["consumed"].as_array().unwrap().is_empty());
    }

    #[test]
    fn unstructured_requeues_with_a_new_steward_nonce_after_the_hourly_floor() {
        let now = 1_800_000_000_000_u64;
        let fixture = Fixture::new(now.saturating_sub(QUIET_MS));
        let Decision::Due(first) = super::consider(&fixture.config, now).unwrap() else {
            panic!("evidence should be due");
        };
        super::begin_attempt(&fixture.config, &first.trigger_nonce, now).unwrap();
        super::finish(
            &fixture.config,
            &first.trigger_nonce,
            None,
            now.saturating_add(1),
        )
        .unwrap();
        assert!(matches!(
            super::consider(&fixture.config, now.saturating_add(2)).unwrap(),
            Decision::Deferred { .. }
        ));
        let Decision::Due(second) = super::consider(
            &fixture.config,
            now.saturating_add(1).saturating_add(MINIMUM_INTERVAL_MS),
        )
        .unwrap() else {
            panic!("requeued evidence should become due");
        };
        assert_ne!(first.trigger_nonce, second.trigger_nonce);
        assert_ne!(first.due_nonce, second.due_nonce);
    }

    #[test]
    fn scheduled_absorption_consumes_pending_without_an_integration_start() {
        let now = 1_800_000_000_000_u64;
        let fixture = Fixture::new(now);
        let projection = super::prepare_scheduled(&fixture.config, "due-12345", now).unwrap();
        assert_eq!(projection["evidence"].as_array().unwrap().len(), 1);
        let cited = vec!["evidence-a".to_owned()];
        assert_eq!(
            super::absorb_scheduled(&fixture.config, "due-12345", &cited, now).unwrap(),
            1
        );
        assert_eq!(
            super::absorb_scheduled(&fixture.config, "due-12345", &cited, now).unwrap(),
            0
        );
        assert!(matches!(
            super::consider(&fixture.config, now.saturating_add(QUIET_MS)).unwrap(),
            Decision::None
        ));
    }

    #[test]
    fn scheduled_absorption_never_consumes_evidence_that_arrived_after_its_prompt_snapshot() {
        let now = 1_800_000_000_000_u64;
        let fixture = Fixture::new(now.saturating_sub(QUIET_MS));
        let projection = super::prepare_scheduled(&fixture.config, "due-12345", now).unwrap();
        assert_eq!(projection["evidence"][0]["evidence_id"], "evidence-a");

        fixture.write(now, &["evidence-a", "evidence-b"]);
        let cited = vec!["evidence-a".to_owned()];
        assert_eq!(
            super::absorb_scheduled(&fixture.config, "due-12345", &cited, now.saturating_add(1))
                .unwrap(),
            1
        );

        assert!(matches!(
            super::consider(&fixture.config, now.saturating_add(1)).unwrap(),
            Decision::Deferred {
                status: "evidence_integration_quiet_until",
                ..
            }
        ));
        let Decision::Due(trigger) = super::consider(
            &fixture.config,
            now.saturating_add(MINIMUM_INTERVAL_MS).saturating_add(1),
        )
        .unwrap() else {
            panic!("post-snapshot evidence must remain pending");
        };
        assert_eq!(trigger.evidence.len(), 1);
        assert_eq!(trigger.evidence[0].evidence_id, "evidence-b");
    }

    #[test]
    fn scheduled_absorption_consumes_only_cited_snapshot_members() {
        let now = 1_800_000_000_000_u64;
        let fixture = Fixture::new(now.saturating_sub(QUIET_MS));
        fixture.write(now.saturating_sub(QUIET_MS), &["evidence-a", "evidence-b"]);
        super::prepare_scheduled(&fixture.config, "due-12345", now).unwrap();
        assert_eq!(
            super::absorb_scheduled(
                &fixture.config,
                "due-12345",
                &["evidence-a".to_owned()],
                now.saturating_add(1),
            )
            .unwrap(),
            1
        );
        let state = fixture.state();
        assert_eq!(state["consumed"].as_array().unwrap().len(), 1);
        assert_eq!(state["consumed"][0]["evidence_id"], "evidence-a");
        assert_eq!(state["pending"].as_array().unwrap().len(), 1);
        assert_eq!(state["pending"][0]["evidence_id"], "evidence-b");
    }

    #[test]
    fn empty_or_foreign_scheduled_citations_never_consume_the_snapshot() {
        let now = 1_800_000_000_000_u64;
        let empty = Fixture::new(now.saturating_sub(QUIET_MS));
        super::prepare_scheduled(&empty.config, "due-12345", now).unwrap();
        assert_eq!(
            super::absorb_scheduled(&empty.config, "due-12345", &[], now).unwrap(),
            0
        );
        let state = empty.state();
        assert!(state["consumed"].as_array().unwrap().is_empty());
        assert_eq!(state["pending"][0]["evidence_id"], "evidence-a");

        let foreign = Fixture::new(now.saturating_sub(QUIET_MS));
        super::prepare_scheduled(&foreign.config, "due-12345", now).unwrap();
        assert!(
            super::absorb_scheduled(
                &foreign.config,
                "due-12345",
                &["evidence-foreign".to_owned()],
                now,
            )
            .is_err()
        );
        let state = foreign.state();
        assert!(state["scheduled_absorption"].is_object());
        assert!(state["consumed"].as_array().unwrap().is_empty());
    }

    #[test]
    fn prompt_projection_bounds_summary_and_reference_with_explicit_truncation() {
        let record = super::EvidenceRecord {
            evidence_id: "evidence-a".to_owned(),
            kind: "completed_study".to_owned(),
            epistemic_status: "verified_machine_evidence".to_owned(),
            reference: "r".repeat(512),
            summary: "s".repeat(480),
            source: "exact_action_parent_and_artifact_hash".to_owned(),
            captured_at_unix_ms: 1_800_000_000_000,
            sha256: sha256(b"evidence-a"),
            eligible_for_belief_update: true,
        };
        let projected = super::prompt_record_projection(&record);
        assert_eq!(
            projected["reference"].as_str().unwrap().chars().count(),
            super::PROMPT_REFERENCE_CHARS
        );
        assert_eq!(
            projected["summary"].as_str().unwrap().chars().count(),
            super::PROMPT_SUMMARY_CHARS
        );
        assert_eq!(projected["reference_truncated"], true);
        assert_eq!(projected["summary_truncated"], true);
    }

    #[test]
    fn unstructured_scheduled_reflection_releases_snapshot_without_consuming_evidence() {
        let now = 1_800_000_000_000_u64;
        let fixture = Fixture::new(now.saturating_sub(QUIET_MS));
        super::prepare_scheduled(&fixture.config, "due-12345", now).unwrap();
        assert!(super::release_scheduled(&fixture.config, "due-12345", now).unwrap());
        assert!(!super::release_scheduled(&fixture.config, "due-12345", now).unwrap());

        let Decision::Due(trigger) = super::consider(&fixture.config, now).unwrap() else {
            panic!("unstructured scheduled authorship must leave evidence pending");
        };
        assert_eq!(trigger.evidence[0].evidence_id, "evidence-a");
    }

    #[test]
    fn ineligible_duplicate_replaced_and_legacy_sources_never_trigger() {
        let now = 1_800_000_000_000_u64;
        let fixture = Fixture::new(now);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&fixture.continuity).unwrap()).unwrap();
        value["evidence_records"][0]["eligible_for_belief_update"] = serde_json::json!(false);
        fs::write(&fixture.continuity, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            super::consider(&fixture.config, now).unwrap(),
            Decision::None
        ));
        assert_eq!(
            fixture.state()["rejected"][0]["reason"],
            "pending_evidence_is_ineligible"
        );

        let legacy = Fixture::new(now);
        fs::write(
            &legacy.continuity,
            br#"{"schema":"astrid_edge_thread_state_v6","pending_evidence_ids":["evidence-a"]}"#,
        )
        .unwrap();
        assert!(matches!(
            super::consider(&legacy.config, now).unwrap(),
            Decision::None
        ));

        let metadata = fs::metadata(&legacy.config.state_root).unwrap();
        assert_eq!(metadata.gid(), nix::unistd::getegid().as_raw());
    }

    #[test]
    fn malformed_hash_is_skipped_reported_and_does_not_wedge_valid_evidence() {
        let now = 1_800_000_000_000_u64;
        let fixture = Fixture::new(now.saturating_sub(QUIET_MS));
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&fixture.continuity).unwrap()).unwrap();
        value["pending_evidence_ids"] =
            serde_json::json!([null, 7, "evidence-a", "evidence-b", "evidence-c"]);
        value["evidence_records"][0]["sha256"] = serde_json::Value::Null;
        value["evidence_records"][1]["sha256"] = serde_json::json!("A".repeat(64));
        value["evidence_records"][1]["captured_at_unix_ms"] =
            serde_json::json!(now.saturating_sub(QUIET_MS));
        fs::write(&fixture.continuity, serde_json::to_vec(&value).unwrap()).unwrap();

        let Decision::Due(trigger) = super::consider(&fixture.config, now).unwrap() else {
            panic!("the valid sibling evidence should still become due");
        };
        assert_eq!(trigger.evidence.len(), 1);
        assert_eq!(trigger.evidence[0].evidence_id, "evidence-c");
        let state = fixture.state();
        let reasons = state["rejected"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value["reason"].as_str())
            .collect::<Vec<_>>();
        assert!(reasons.contains(&"malformed_evidence_record"));
        assert!(reasons.contains(&"invalid_evidence_record"));
        assert!(reasons.contains(&"malformed_pending_evidence_id"));
    }

    #[test]
    fn provider_started_without_prepared_authorship_is_terminal_and_never_retried() {
        let now = 1_800_000_000_000_u64;
        let fixture = Fixture::new(now.saturating_sub(QUIET_MS));
        let Decision::Due(trigger) = super::consider(&fixture.config, now).unwrap() else {
            panic!("evidence should be due");
        };
        super::begin_attempt(&fixture.config, &trigger.trigger_nonce, now).unwrap();
        assert!(super::provider_started(&fixture.config, &trigger.trigger_nonce, now).unwrap());
        assert!(
            super::terminalize_started_unknown(
                &fixture.config,
                &trigger.trigger_nonce,
                now.saturating_add(1)
            )
            .unwrap()
        );
        assert!(
            super::terminalize_started_unknown(
                &fixture.config,
                &trigger.trigger_nonce,
                now.saturating_add(2)
            )
            .unwrap()
        );
        assert!(matches!(
            super::consider(&fixture.config, now.saturating_add(2 * MINIMUM_INTERVAL_MS)).unwrap(),
            Decision::None
        ));
        let state = fixture.state();
        assert_eq!(state["ambiguous"].as_array().unwrap().len(), 1);
        assert_eq!(
            state["ambiguous"][0]["status"],
            "provider_started_delivery_authorship_unknown_non_authored"
        );
    }

    #[test]
    fn consumed_fingerprint_survives_v7_eviction_and_reappearance() {
        let now = 1_800_000_000_000_u64;
        let fixture = Fixture::new(now.saturating_sub(QUIET_MS));
        let Decision::Due(trigger) = super::consider(&fixture.config, now).unwrap() else {
            panic!("evidence should be due");
        };
        super::begin_attempt(&fixture.config, &trigger.trigger_nonce, now).unwrap();
        super::finish(
            &fixture.config,
            &trigger.trigger_nonce,
            Some(&["evidence-a".to_owned()]),
            now.saturating_add(1),
        )
        .unwrap();
        fixture.write(now.saturating_add(2), &[]);
        assert!(matches!(
            super::consider(&fixture.config, now.saturating_add(2)).unwrap(),
            Decision::None
        ));
        fixture.write(now.saturating_add(3), &["evidence-a"]);
        assert!(matches!(
            super::consider(
                &fixture.config,
                now.saturating_add(MINIMUM_INTERVAL_MS).saturating_add(3)
            )
            .unwrap(),
            Decision::None
        ));
        assert_eq!(fixture.state()["consumed"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn twelfth_daily_start_is_allowed_and_thirteenth_is_deferred() {
        let day_start = super::DAY_MS.saturating_mul(20_000);
        let fixture = Fixture::new(day_start);
        for index in 0_u64..12 {
            let now = day_start
                .saturating_add(6 * 60 * 1_000)
                .saturating_add(index.saturating_mul(MINIMUM_INTERVAL_MS));
            let evidence_id = format!("evidence-{index}");
            fs::write(
                &fixture.continuity,
                serde_json::to_vec(&serde_json::json!({
                    "schema": "astrid_edge_thread_state_v7",
                    "pending_evidence_ids": [&evidence_id],
                    "evidence_records": [{
                        "evidence_id": &evidence_id,
                        "kind": "completed_study",
                        "epistemic_status": "verified_machine_evidence",
                        "reference": format!("studies/{evidence_id}.json"),
                        "summary": "Bounded result.",
                        "source": "exact_action_parent_and_artifact_hash",
                        "captured_at_unix_ms": now.saturating_sub(QUIET_MS),
                        "sha256": sha256(evidence_id.as_bytes()),
                        "eligible_for_belief_update": true
                    }],
                    "updated_at_unix_ms": now.saturating_sub(QUIET_MS),
                    "revision": index.saturating_add(1),
                    "event": "evidence_arrival_completed_study"
                }))
                .unwrap(),
            )
            .unwrap();
            let Decision::Due(trigger) = super::consider(&fixture.config, now).unwrap() else {
                panic!("daily integration start {index} should be due");
            };
            super::begin_attempt(&fixture.config, &trigger.trigger_nonce, now).unwrap();
            super::finish(
                &fixture.config,
                &trigger.trigger_nonce,
                Some(&[evidence_id]),
                now,
            )
            .unwrap();
        }
        let now = day_start
            .saturating_add(6 * 60 * 1_000)
            .saturating_add(12 * MINIMUM_INTERVAL_MS);
        fs::write(
            &fixture.continuity,
            serde_json::to_vec(&serde_json::json!({
                "schema": "astrid_edge_thread_state_v7",
                "pending_evidence_ids": ["evidence-12"],
                "evidence_records": [{
                    "evidence_id": "evidence-12",
                    "kind": "completed_study",
                    "epistemic_status": "verified_machine_evidence",
                    "reference": "studies/evidence-12.json",
                    "summary": "Bounded result.",
                    "source": "exact_action_parent_and_artifact_hash",
                    "captured_at_unix_ms": now.saturating_sub(QUIET_MS),
                    "sha256": sha256(b"evidence-12"),
                    "eligible_for_belief_update": true
                }],
                "updated_at_unix_ms": now.saturating_sub(QUIET_MS),
                "revision": 13,
                "event": "evidence_arrival_completed_study"
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            super::consider(&fixture.config, now).unwrap(),
            Decision::Deferred {
                status: "evidence_integration_daily_limit_until",
                ..
            }
        ));
    }
}
