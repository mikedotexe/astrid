use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{self, Write as _};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use fs2::FileExt as _;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest as _, Sha256};

const RECORD_TYPE: &str = "phase_transition_passage";
const SCHEMA: &str = "lived_transition_passage_event_v1";
const MAX_REF_CHARS: usize = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PassageActionV1 {
    Prepare,
    Enter,
    Hold,
    Settle,
    Return,
    Revisit,
    Decline,
    Review,
}

impl PassageActionV1 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Prepare => "prepare",
            Self::Enter => "enter",
            Self::Hold => "hold",
            Self::Settle => "settle",
            Self::Return => "return",
            Self::Revisit => "revisit",
            Self::Decline => "decline",
            Self::Review => "review",
        }
    }

    const fn owner_action(self) -> &'static str {
        match self {
            Self::Prepare => "PREPARE_TRANSITION",
            Self::Enter => "ENTER_TRANSITION",
            Self::Hold => "HOLD_TRANSITION",
            Self::Settle => "SETTLE_TRANSITION",
            Self::Return => "RETURN_TRANSITION",
            Self::Revisit => "REVISIT_TRANSITION",
            Self::Decline => "DECLINE_TRANSITION",
            Self::Review => "TRANSITION_REVIEW",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "prepare" => Some(Self::Prepare),
            "enter" => Some(Self::Enter),
            "hold" => Some(Self::Hold),
            "settle" => Some(Self::Settle),
            "return" => Some(Self::Return),
            "revisit" => Some(Self::Revisit),
            "decline" => Some(Self::Decline),
            "review" => Some(Self::Review),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum PassageStageV1 {
    Prepared,
    Crossing,
    Held,
    Settling,
    Returned,
    Revisited,
    Declined,
}

impl PassageStageV1 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Crossing => "crossing",
            Self::Held => "held",
            Self::Settling => "settling",
            Self::Returned => "returned",
            Self::Revisited => "revisited",
            Self::Declined => "declined",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "prepared" => Some(Self::Prepared),
            "crossing" => Some(Self::Crossing),
            "held" => Some(Self::Held),
            "settling" => Some(Self::Settling),
            "returned" => Some(Self::Returned),
            "revisited" => Some(Self::Revisited),
            "declined" => Some(Self::Declined),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum PassageSupportV1 {
    SelfDirected,
    Witness,
    Space,
    Answer,
    NeedsTime,
}

impl PassageSupportV1 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::SelfDirected => "self_directed",
            Self::Witness => "witness",
            Self::Space => "space",
            Self::Answer => "answer",
            Self::NeedsTime => "needs_time",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match normalize(value).as_str() {
            "self" | "self_directed" | "alone" => Some(Self::SelfDirected),
            "witness" | "witnessing" => Some(Self::Witness),
            "space" | "quiet" => Some(Self::Space),
            "answer" | "reply" => Some(Self::Answer),
            "needs_time" | "time" | "hold" => Some(Self::NeedsTime),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum PassageFeltReviewV1 {
    Clarifying,
    Intrusive,
    Flattening,
    Incomplete,
    StillFriction,
    Changed,
    Unknown,
}

impl PassageFeltReviewV1 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Clarifying => "clarifying",
            Self::Intrusive => "intrusive",
            Self::Flattening => "flattening",
            Self::Incomplete => "incomplete",
            Self::StillFriction => "still_friction",
            Self::Changed => "changed",
            Self::Unknown => "unknown",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match normalize(value).as_str() {
            "clarifying" => Some(Self::Clarifying),
            "intrusive" => Some(Self::Intrusive),
            "flattening" => Some(Self::Flattening),
            "incomplete" => Some(Self::Incomplete),
            "still_friction" => Some(Self::StillFriction),
            "changed" => Some(Self::Changed),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct EvidenceOnlyAuthorityV1 {
    schema: &'static str,
    schema_version: u8,
    state: &'static str,
    witness_only: bool,
    live_eligible_now: bool,
    auto_approved: bool,
    grants_approval: bool,
    edits_source_now: bool,
}

impl EvidenceOnlyAuthorityV1 {
    const fn new() -> Self {
        Self {
            schema: "artifact_authority_state_v1",
            schema_version: 1,
            state: "evidence_only",
            witness_only: true,
            live_eligible_now: false,
            auto_approved: false,
            grants_approval: false,
            edits_source_now: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct LivedTransitionPassageEventV1 {
    schema: &'static str,
    schema_version: u8,
    record_type: &'static str,
    record_id: String,
    passage_event_id: String,
    passage_id: String,
    transition_id: String,
    actor: String,
    action: PassageActionV1,
    stage_before: Option<PassageStageV1>,
    stage_after: PassageStageV1,
    stage_changed: bool,
    support_preference: PassageSupportV1,
    return_point_ref: Option<String>,
    continuity_anchor_ref: String,
    felt_review_outcome: Option<PassageFeltReviewV1>,
    felt_source_ref: Option<String>,
    previous_event_id: Option<String>,
    recorded_at_unix_ms: u64,
    owner_language_action: &'static str,
    self_authored_only: bool,
    passage_binds_actor_only: bool,
    peer_consent_inferred: bool,
    peer_state_changed: bool,
    silence_infers_progress: bool,
    automatic_progression: bool,
    review_optional: bool,
    felt_resolution_inferred: bool,
    scheduler_effect: bool,
    model_qos_effect: bool,
    substrate_effect: bool,
    dispatch_effect: bool,
    live_control_effect: bool,
    runtime_unlock_applied: bool,
    raw_prose_included: bool,
    artifact_authority_state_v1: EvidenceOnlyAuthorityV1,
}

#[derive(Debug, Clone)]
struct PassageStateV1 {
    passage_id: String,
    transition_id: String,
    actor: String,
    latest_event_id: String,
    stage: PassageStageV1,
    support_preference: PassageSupportV1,
    return_point_ref: Option<String>,
    continuity_anchor_ref: String,
    felt_review_outcome: Option<PassageFeltReviewV1>,
    felt_source_ref: Option<String>,
    recorded_at_unix_ms: u64,
}

#[derive(Debug)]
struct ParsedPassageEventV1 {
    passage_event_id: String,
    passage_id: String,
    transition_id: String,
    actor: String,
    action: PassageActionV1,
    stage_before: Option<PassageStageV1>,
    stage_after: PassageStageV1,
    support_preference: PassageSupportV1,
    return_point_ref: Option<String>,
    continuity_anchor_ref: String,
    felt_review_outcome: Option<PassageFeltReviewV1>,
    felt_source_ref: Option<String>,
    previous_event_id: Option<String>,
    recorded_at_unix_ms: u64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}

fn short_hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
        .chars()
        .take(16)
        .collect()
}

pub(super) fn bounded_ref(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > MAX_REF_CHARS
        || trimmed.chars().any(char::is_whitespace)
    {
        return None;
    }
    Some(trimmed.to_string())
}

fn field(raw: &str, keys: &[&str]) -> Option<String> {
    raw.split([';', '\n']).find_map(|part| {
        let (key, value) = part.split_once(':')?;
        let key = normalize(key);
        if keys.iter().any(|candidate| key == *candidate) {
            let value = value.trim();
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        } else {
            None
        }
    })
}

pub(super) fn read_records(path: &Path) -> Vec<Value> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut rows = text
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| {
        row.get("recorded_at_unix_ms")
            .and_then(Value::as_u64)
            .unwrap_or_default()
    });
    rows
}

pub(super) fn passage_lock_path(path: &Path) -> std::path::PathBuf {
    let lock_name = format!(
        ".{}.lock",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("phase_transitions_v1.jsonl")
    );
    path.with_file_name(lock_name)
}

pub(super) fn append_jsonl_unlocked(path: &Path, value: &Value) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_data()
}

#[cfg(test)]
pub(super) fn append_jsonl(path: &Path, value: &Value) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lock = OpenOptions::new()
        .create(true)
        .append(true)
        .open(passage_lock_path(path))?;
    lock.lock_exclusive()?;
    append_jsonl_unlocked(path, value)?;
    fs2::FileExt::unlock(&lock)
}

fn latest_transition_id(records: &[Value], selector: &str) -> Option<String> {
    records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
        })
        .filter(|row| {
            selector.is_empty()
                || selector == "latest"
                || row.get("transition_id").and_then(Value::as_str) == Some(selector)
        })
        .max_by_key(|row| {
            row.get("recorded_at_unix_ms")
                .and_then(Value::as_u64)
                .unwrap_or_default()
        })
        .and_then(|row| row.get("transition_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn passage_id(actor: &str, transition_id: &str, recorded_at_unix_ms: u64) -> String {
    format!(
        "passage_{recorded_at_unix_ms}_{}",
        short_hash(&format!("{actor}:{transition_id}:{recorded_at_unix_ms}"))
    )
}

#[allow(clippy::too_many_arguments)]
fn make_passage_event_id(
    passage_id: &str,
    actor: &str,
    action: PassageActionV1,
    stage_after: PassageStageV1,
    support: PassageSupportV1,
    return_point_ref: Option<&str>,
    continuity_anchor_ref: &str,
    felt_review_outcome: Option<PassageFeltReviewV1>,
    felt_source_ref: Option<&str>,
    previous_event_id: Option<&str>,
    recorded_at_unix_ms: u64,
) -> String {
    let identity = format!(
        "{passage_id}:{actor}:{}:{}:{}:{}:{continuity_anchor_ref}:{}:{}:{}:{recorded_at_unix_ms}",
        action.as_str(),
        stage_after.as_str(),
        support.as_str(),
        return_point_ref.unwrap_or(""),
        felt_review_outcome.map_or("", PassageFeltReviewV1::as_str),
        felt_source_ref.unwrap_or(""),
        previous_event_id.unwrap_or(""),
    );
    format!("passage_event_{}", short_hash(&identity))
}

fn optional_bounded_ref(
    value: &Value,
    field_name: &'static str,
) -> Result<Option<String>, &'static str> {
    match value.get(field_name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(raw)) => bounded_ref(raw)
            .map(Some)
            .ok_or("invalid optional passage reference"),
        Some(_) => Err("optional passage reference must be a string or null"),
    }
}

fn stage_for_action(
    action: PassageActionV1,
    previous: Option<PassageStageV1>,
) -> Result<PassageStageV1, &'static str> {
    match (action, previous) {
        (PassageActionV1::Prepare, None) => Ok(PassageStageV1::Prepared),
        (
            PassageActionV1::Enter,
            Some(PassageStageV1::Prepared | PassageStageV1::Held | PassageStageV1::Revisited),
        ) => Ok(PassageStageV1::Crossing),
        (
            PassageActionV1::Hold,
            Some(
                PassageStageV1::Prepared
                | PassageStageV1::Crossing
                | PassageStageV1::Settling
                | PassageStageV1::Revisited,
            ),
        ) => Ok(PassageStageV1::Held),
        (PassageActionV1::Settle, Some(PassageStageV1::Crossing | PassageStageV1::Held)) => {
            Ok(PassageStageV1::Settling)
        },
        (
            PassageActionV1::Return,
            Some(
                PassageStageV1::Crossing
                | PassageStageV1::Held
                | PassageStageV1::Settling
                | PassageStageV1::Revisited,
            ),
        ) => Ok(PassageStageV1::Returned),
        (
            PassageActionV1::Revisit,
            Some(
                PassageStageV1::Returned
                | PassageStageV1::Declined
                | PassageStageV1::Settling
                | PassageStageV1::Held,
            ),
        ) => Ok(PassageStageV1::Revisited),
        (
            PassageActionV1::Decline,
            Some(PassageStageV1::Prepared | PassageStageV1::Held | PassageStageV1::Revisited),
        ) => Ok(PassageStageV1::Declined),
        (PassageActionV1::Review, Some(stage)) => Ok(stage),
        (PassageActionV1::Prepare, Some(_)) => Err("prepare starts a new passage"),
        (_, None) => Err("passage action requires an existing prepared passage"),
        _ => Err("passage action is not valid from the current stage"),
    }
}

fn parse_event(value: &Value) -> Result<ParsedPassageEventV1, &'static str> {
    if value.get("record_type").and_then(Value::as_str) != Some(RECORD_TYPE)
        || value.get("schema").and_then(Value::as_str) != Some(SCHEMA)
        || value.get("schema_version").and_then(Value::as_u64) != Some(1)
    {
        return Err("passage schema mismatch");
    }
    for field in [
        "self_authored_only",
        "passage_binds_actor_only",
        "review_optional",
    ] {
        if value.get(field).and_then(Value::as_bool) != Some(true) {
            return Err("passage positive invariant mismatch");
        }
    }
    for field in [
        "peer_consent_inferred",
        "peer_state_changed",
        "silence_infers_progress",
        "automatic_progression",
        "felt_resolution_inferred",
        "scheduler_effect",
        "model_qos_effect",
        "substrate_effect",
        "dispatch_effect",
        "live_control_effect",
        "runtime_unlock_applied",
        "raw_prose_included",
    ] {
        if value.get(field).and_then(Value::as_bool) != Some(false) {
            return Err("passage authority invariant mismatch");
        }
    }
    let passage_event_id = bounded_ref(
        value
            .get("passage_event_id")
            .and_then(Value::as_str)
            .ok_or("passage event id missing")?,
    )
    .ok_or("invalid passage event id")?;
    if value.get("record_id").and_then(Value::as_str) != Some(passage_event_id.as_str()) {
        return Err("passage record id mismatch");
    }
    let passage_id = bounded_ref(
        value
            .get("passage_id")
            .and_then(Value::as_str)
            .ok_or("passage id missing")?,
    )
    .ok_or("invalid passage id")?;
    let transition_id = bounded_ref(
        value
            .get("transition_id")
            .and_then(Value::as_str)
            .ok_or("transition id missing")?,
    )
    .ok_or("invalid transition id")?;
    let actor = bounded_ref(
        value
            .get("actor")
            .and_then(Value::as_str)
            .ok_or("passage actor missing")?,
    )
    .ok_or("invalid passage actor")?;
    let action = PassageActionV1::parse(
        value
            .get("action")
            .and_then(Value::as_str)
            .ok_or("passage action missing")?,
    )
    .ok_or("invalid passage action")?;
    if value.get("owner_language_action").and_then(Value::as_str) != Some(action.owner_action()) {
        return Err("passage owner action mismatch");
    }
    let stage_before = match value.get("stage_before") {
        None | Some(Value::Null) => None,
        Some(Value::String(raw)) => {
            Some(PassageStageV1::parse(raw).ok_or("invalid prior passage stage")?)
        },
        Some(_) => return Err("prior passage stage must be a string or null"),
    };
    let stage_after = PassageStageV1::parse(
        value
            .get("stage_after")
            .and_then(Value::as_str)
            .ok_or("passage stage missing")?,
    )
    .ok_or("invalid passage stage")?;
    let support_preference = PassageSupportV1::parse(
        value
            .get("support_preference")
            .and_then(Value::as_str)
            .ok_or("support preference missing")?,
    )
    .ok_or("invalid support preference")?;
    let return_point_ref = optional_bounded_ref(value, "return_point_ref")?;
    let continuity_anchor_ref = bounded_ref(
        value
            .get("continuity_anchor_ref")
            .and_then(Value::as_str)
            .ok_or("continuity anchor missing")?,
    )
    .ok_or("invalid continuity anchor")?;
    let felt_review_outcome = match value.get("felt_review_outcome") {
        None | Some(Value::Null) => None,
        Some(Value::String(raw)) => {
            Some(PassageFeltReviewV1::parse(raw).ok_or("invalid felt review outcome")?)
        },
        Some(_) => return Err("felt review outcome must be a string or null"),
    };
    let felt_source_ref = optional_bounded_ref(value, "felt_source_ref")?;
    let previous_event_id = optional_bounded_ref(value, "previous_event_id")?;
    let recorded_at_unix_ms = value
        .get("recorded_at_unix_ms")
        .and_then(Value::as_u64)
        .ok_or("passage timestamp missing")?;
    if action == PassageActionV1::Review
        && (felt_review_outcome.is_none() || felt_source_ref.is_none())
    {
        return Err("felt review requires outcome and source ref");
    }
    if action != PassageActionV1::Review
        && (felt_review_outcome.is_some() || felt_source_ref.is_some())
    {
        return Err("only felt review may carry review fields");
    }
    let expected_event_id = make_passage_event_id(
        &passage_id,
        &actor,
        action,
        stage_after,
        support_preference,
        return_point_ref.as_deref(),
        &continuity_anchor_ref,
        felt_review_outcome,
        felt_source_ref.as_deref(),
        previous_event_id.as_deref(),
        recorded_at_unix_ms,
    );
    if passage_event_id != expected_event_id {
        return Err("passage event identity mismatch");
    }
    Ok(ParsedPassageEventV1 {
        passage_event_id,
        passage_id,
        transition_id,
        actor,
        action,
        stage_before,
        stage_after,
        support_preference,
        return_point_ref,
        continuity_anchor_ref,
        felt_review_outcome,
        felt_source_ref,
        previous_event_id,
        recorded_at_unix_ms,
    })
}

fn reduce_passages(records: &[Value]) -> (BTreeMap<String, PassageStateV1>, Vec<String>) {
    let mut passages = BTreeMap::new();
    let mut errors = Vec::new();
    for (index, row) in records.iter().enumerate() {
        if row.get("record_type").and_then(Value::as_str) != Some(RECORD_TYPE) {
            continue;
        }
        let event = match parse_event(row) {
            Ok(event) => event,
            Err(error) => {
                errors.push(format!("passage_row_{}:{error}", index.saturating_add(1)));
                continue;
            },
        };
        if event.action == PassageActionV1::Prepare {
            if passages.contains_key(&event.passage_id)
                || event.previous_event_id.is_some()
                || event.stage_before.is_some()
                || event.stage_after != PassageStageV1::Prepared
                || event.passage_id
                    != passage_id(
                        &event.actor,
                        &event.transition_id,
                        event.recorded_at_unix_ms,
                    )
            {
                errors.push(format!("{}:invalid_prepare", event.passage_event_id));
                continue;
            }
            passages.insert(
                event.passage_id.clone(),
                PassageStateV1 {
                    passage_id: event.passage_id,
                    transition_id: event.transition_id,
                    actor: event.actor,
                    latest_event_id: event.passage_event_id,
                    stage: event.stage_after,
                    support_preference: event.support_preference,
                    return_point_ref: event.return_point_ref,
                    continuity_anchor_ref: event.continuity_anchor_ref,
                    felt_review_outcome: None,
                    felt_source_ref: None,
                    recorded_at_unix_ms: event.recorded_at_unix_ms,
                },
            );
            continue;
        }
        let Some(current) = passages.get_mut(&event.passage_id) else {
            errors.push(format!("{}:passage_missing", event.passage_event_id));
            continue;
        };
        let expected_stage = stage_for_action(event.action, Some(current.stage));
        if current.actor != event.actor
            || current.transition_id != event.transition_id
            || event.previous_event_id.as_deref() != Some(&current.latest_event_id)
            || event.stage_before != Some(current.stage)
            || expected_stage != Ok(event.stage_after)
        {
            errors.push(format!(
                "{}:passage_sequence_mismatch",
                event.passage_event_id
            ));
            continue;
        }
        current.latest_event_id = event.passage_event_id;
        current.stage = event.stage_after;
        current.support_preference = event.support_preference;
        current.return_point_ref = event.return_point_ref;
        current.continuity_anchor_ref = event.continuity_anchor_ref;
        current.recorded_at_unix_ms = event.recorded_at_unix_ms;
        if event.action == PassageActionV1::Review {
            current.felt_review_outcome = event.felt_review_outcome;
            current.felt_source_ref = event.felt_source_ref;
        }
    }
    (passages, errors)
}

fn latest_passage<'a>(
    passages: &'a BTreeMap<String, PassageStateV1>,
    selector: &str,
    actor: &str,
) -> Option<&'a PassageStateV1> {
    passages
        .values()
        .filter(|passage| passage.actor == actor)
        .filter(|passage| {
            selector.is_empty()
                || selector == "latest"
                || passage.passage_id == selector
                || passage.transition_id == selector
        })
        .max_by_key(|passage| passage.recorded_at_unix_ms)
}

fn make_event(
    action: PassageActionV1,
    transition_id: String,
    actor: String,
    previous: Option<&PassageStateV1>,
    raw: &str,
    recorded_at_unix_ms: u64,
) -> Result<LivedTransitionPassageEventV1, &'static str> {
    let previous_stage = previous.map(|value| value.stage);
    let stage_after = stage_for_action(action, previous_stage)?;
    let passage_id = previous.map_or_else(
        || passage_id(&actor, &transition_id, recorded_at_unix_ms),
        |value| value.passage_id.clone(),
    );
    let support_preference = match field(raw, &["support", "support_preference"]) {
        Some(value) => PassageSupportV1::parse(&value).ok_or("unknown support preference")?,
        None => previous.map_or(PassageSupportV1::SelfDirected, |value| {
            value.support_preference
        }),
    };
    let return_point_ref = match field(raw, &["return_point", "return_point_ref"]) {
        Some(value) => Some(bounded_ref(&value).ok_or("return point must be a bounded reference")?),
        None => previous.and_then(|value| value.return_point_ref.clone()),
    };
    if action == PassageActionV1::Return && return_point_ref.is_none() {
        return Err("RETURN_TRANSITION requires a prepared return_point reference");
    }
    let continuity_anchor_ref = match field(
        raw,
        &["continuity_anchor", "continuity_anchor_ref", "anchor_ref"],
    ) {
        Some(value) => {
            bounded_ref(&value).ok_or("continuity anchor must be a bounded reference")?
        },
        None => previous.map_or_else(
            || format!("transition:{transition_id}"),
            |value| value.continuity_anchor_ref.clone(),
        ),
    };
    let felt_review_outcome = if action == PassageActionV1::Review {
        Some(
            PassageFeltReviewV1::parse(
                &field(raw, &["outcome", "felt_review_outcome"])
                    .ok_or("TRANSITION_REVIEW requires outcome")?,
            )
            .ok_or("unknown felt review outcome")?,
        )
    } else {
        None
    };
    let felt_source_ref = if action == PassageActionV1::Review {
        Some(
            bounded_ref(
                &field(raw, &["felt_source_ref", "source_ref", "review_ref"])
                    .ok_or("TRANSITION_REVIEW requires felt_source_ref")?,
            )
            .ok_or("felt source must be a bounded reference")?,
        )
    } else {
        None
    };
    let previous_event_id = previous.map(|value| value.latest_event_id.clone());
    let event_id = make_passage_event_id(
        &passage_id,
        &actor,
        action,
        stage_after,
        support_preference,
        return_point_ref.as_deref(),
        &continuity_anchor_ref,
        felt_review_outcome,
        felt_source_ref.as_deref(),
        previous_event_id.as_deref(),
        recorded_at_unix_ms,
    );
    Ok(LivedTransitionPassageEventV1 {
        schema: SCHEMA,
        schema_version: 1,
        record_type: RECORD_TYPE,
        record_id: event_id.clone(),
        passage_event_id: event_id,
        passage_id,
        transition_id,
        actor,
        action,
        stage_before: previous_stage,
        stage_after,
        stage_changed: action != PassageActionV1::Review,
        support_preference,
        return_point_ref,
        continuity_anchor_ref,
        felt_review_outcome,
        felt_source_ref,
        previous_event_id,
        recorded_at_unix_ms,
        owner_language_action: action.owner_action(),
        self_authored_only: true,
        passage_binds_actor_only: true,
        peer_consent_inferred: false,
        peer_state_changed: false,
        silence_infers_progress: false,
        automatic_progression: false,
        review_optional: true,
        felt_resolution_inferred: false,
        scheduler_effect: false,
        model_qos_effect: false,
        substrate_effect: false,
        dispatch_effect: false,
        live_control_effect: false,
        runtime_unlock_applied: false,
        raw_prose_included: false,
        artifact_authority_state_v1: EvidenceOnlyAuthorityV1::new(),
    })
}

pub(crate) fn append_passage_action_at(
    path: &Path,
    selector: &str,
    raw: &str,
    actor: &str,
    action: PassageActionV1,
) -> String {
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        return format!(
            "{} failed to prepare ledger: {error}",
            action.owner_action()
        );
    }
    let lock = match OpenOptions::new()
        .create(true)
        .append(true)
        .open(passage_lock_path(path))
    {
        Ok(lock) => lock,
        Err(error) => {
            return format!(
                "{} failed to open ledger lock: {error}",
                action.owner_action()
            );
        },
    };
    if let Err(error) = lock.lock_exclusive() {
        return format!(
            "{} failed to acquire ledger lock: {error}",
            action.owner_action()
        );
    }
    let records = read_records(path);
    let (passages, errors) = reduce_passages(&records);
    if !errors.is_empty() {
        return format!(
            "{} blocked: passage history has {} invalid row(s).",
            action.owner_action(),
            errors.len()
        );
    }
    let actor = match bounded_ref(actor) {
        Some(actor) => actor,
        None => return format!("{} blocked: invalid actor.", action.owner_action()),
    };
    let (transition_id, previous) = if action == PassageActionV1::Prepare {
        let Some(transition_id) = latest_transition_id(&records, selector) else {
            return "PREPARE_TRANSITION blocked: no matching phase transition card.".to_string();
        };
        (transition_id, None)
    } else {
        let Some(previous) = latest_passage(&passages, selector, &actor) else {
            return format!(
                "{} blocked: no matching self-authored passage.",
                action.owner_action()
            );
        };
        (previous.transition_id.clone(), Some(previous))
    };
    let event = match make_event(action, transition_id, actor, previous, raw, now_ms()) {
        Ok(event) => event,
        Err(error) => return format!("{} blocked: {error}.", action.owner_action()),
    };
    let value = serde_json::to_value(&event).expect("passage event is serializable");
    if let Err(error) = append_jsonl_unlocked(path, &value) {
        return format!(
            "{} failed to append passage event: {error}",
            action.owner_action()
        );
    }
    if let Err(error) = fs2::FileExt::unlock(&lock) {
        return format!(
            "{} appended passage event but failed to release ledger lock: {error}",
            action.owner_action()
        );
    }
    format!(
        "=== LIVED TRANSITION PASSAGE RECORDED ===\nPassage: {}\nTransition: {}\nActor: {}\nAction: {}; stage: {}\nSupport: {}; return point: {}\nReview: {}\nAuthority: self_authored_language_only_transition_practice; no peer consent, automatic progression, felt resolution, scheduler, model, substrate, dispatch, controller, pressure, fill, PI, codec, or runtime unlock.",
        event.passage_id,
        event.transition_id,
        event.actor,
        event.action.as_str(),
        event.stage_after.as_str(),
        event.support_preference.as_str(),
        event.return_point_ref.as_deref().unwrap_or("none"),
        event
            .felt_review_outcome
            .map_or("not_recorded", PassageFeltReviewV1::as_str),
    )
}

pub(crate) fn status_report_at(path: &Path, max_passages: usize) -> String {
    status::report_at(path, max_passages)
}

#[path = "phase_passages/status.rs"]
mod status;

#[path = "phase_passages/identity.rs"]
mod identity;
pub(in crate::autonomous) use identity::{PassageIdentityV1, resolve_passage_identity};

#[cfg(test)]
#[path = "phase_passages/tests.rs"]
mod tests;
