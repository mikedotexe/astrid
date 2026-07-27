use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use astrid_minime_protocol::{DivisionCommandV1, DivisionLifecycleV1, DivisionStatusV1};
use fs2::FileExt as _;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest as _, Sha256};

const SCHEMA: &str = "division.ceremony_event.v1";
const RECORD_TYPE: &str = "division_ceremony_event";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) enum DivisionCeremonyActionV1 {
    #[serde(rename = "DIVISION_INTENT")]
    Intent,
    #[serde(rename = "DIVISION_ASSENT")]
    Assent,
    #[serde(rename = "DIVISION_WITHDRAW_ASSENT")]
    WithdrawAssent,
    #[serde(rename = "DIVISION_RETURN_REQUEST")]
    ReturnRequest,
    #[serde(rename = "DIVISION_REVIEW")]
    Review,
}

impl DivisionCeremonyActionV1 {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Intent => "DIVISION_INTENT",
            Self::Assent => "DIVISION_ASSENT",
            Self::WithdrawAssent => "DIVISION_WITHDRAW_ASSENT",
            Self::ReturnRequest => "DIVISION_RETURN_REQUEST",
            Self::Review => "DIVISION_REVIEW",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "DIVISION_INTENT" => Some(Self::Intent),
            "DIVISION_ASSENT" => Some(Self::Assent),
            "DIVISION_WITHDRAW_ASSENT" => Some(Self::WithdrawAssent),
            "DIVISION_RETURN_REQUEST" => Some(Self::ReturnRequest),
            "DIVISION_REVIEW" => Some(Self::Review),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct DivisionCandidateV1 {
    division_id: String,
    parent_generation: u64,
    plan_digest: String,
    selected_strategy: String,
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
struct DivisionCeremonyEventV1 {
    schema: &'static str,
    schema_version: u8,
    record_type: &'static str,
    record_id: String,
    ceremony_event_id: String,
    actor: String,
    action: DivisionCeremonyActionV1,
    candidate: DivisionCandidateV1,
    source_ref: String,
    recorded_at_unix_ms: u64,
    expires_at_unix_ms: Option<u64>,
    previous_actor_event_id: Option<String>,
    targets_event_id: Option<String>,
    native_status_hash: Option<String>,
    readiness_receipt_ref: Option<String>,
    readiness_receipt_hash: Option<String>,
    snapshot_refs: Vec<String>,
    current_tick: Option<u64>,
    rollback_deadline_tick: Option<u64>,
    review_outcome: Option<String>,
    owner_language_action: &'static str,
    self_authored_only: bool,
    response_revisable: bool,
    right_to_ignore: bool,
    presence_inferred: bool,
    peer_consent_inferred: bool,
    silence_infers_consent: bool,
    native_assent_changed: bool,
    division_stage_changed: bool,
    prepare_dispatched: bool,
    commit_recommended: bool,
    commit_dispatched: bool,
    rollback_dispatched: bool,
    return_transition_dispatched: bool,
    scheduler_effect: bool,
    model_qos_effect: bool,
    substrate_effect: bool,
    dispatch_effect: bool,
    live_control_effect: bool,
    raw_prose_included: bool,
    artifact_authority_state_v1: EvidenceOnlyAuthorityV1,
}

#[derive(Debug, Clone)]
struct ParsedEventV1 {
    event_id: String,
    actor: String,
    action: DivisionCeremonyActionV1,
    candidate: DivisionCandidateV1,
    source_ref: String,
    recorded_at_unix_ms: u64,
    expires_at_unix_ms: Option<u64>,
    targets_event_id: Option<String>,
    native_status_hash: Option<String>,
    snapshot_refs: Vec<String>,
    current_tick: Option<u64>,
    rollback_deadline_tick: Option<u64>,
    review_outcome: Option<String>,
}

struct EventDraftV1 {
    candidate: DivisionCandidateV1,
    expires_at_unix_ms: Option<u64>,
    targets_event_id: Option<String>,
    native_status_hash: Option<String>,
    readiness_receipt_ref: Option<String>,
    readiness_receipt_hash: Option<String>,
    snapshot_refs: Vec<String>,
    current_tick: Option<u64>,
    rollback_deadline_tick: Option<u64>,
    review_outcome: Option<String>,
}

impl EventDraftV1 {
    fn new(candidate: DivisionCandidateV1) -> Self {
        Self {
            candidate,
            expires_at_unix_ms: None,
            targets_event_id: None,
            native_status_hash: None,
            readiness_receipt_ref: None,
            readiness_receipt_hash: None,
            snapshot_refs: Vec::new(),
            current_tick: None,
            rollback_deadline_tick: None,
            review_outcome: None,
        }
    }
}

fn ceremony_dir(workspace: &Path) -> PathBuf {
    workspace.join("division")
}

fn ledger_path(workspace: &Path) -> PathBuf {
    ceremony_dir(workspace).join("ceremony_v1.jsonl")
}

fn lock_path(workspace: &Path) -> PathBuf {
    ceremony_dir(workspace).join("ceremony_v1.lock")
}

fn status_path(workspace: &Path) -> PathBuf {
    ceremony_dir(workspace).join("status.json")
}

#[cfg(unix)]
fn set_owner_only(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("cannot set owner-only permissions: {error}"))
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}

fn parse_fields(raw: &str) -> Result<BTreeMap<String, String>, String> {
    let mut fields = BTreeMap::new();
    for part in raw
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let (key, value) = part
            .split_once(':')
            .ok_or_else(|| "ceremony arguments use bounded `key: value; ...` fields".to_string())?;
        let key = normalize_key(key);
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            return Err("ceremony field names and values must be non-empty".to_string());
        }
        if fields.insert(key.clone(), value.to_string()).is_some() {
            return Err(format!("duplicate ceremony field: {key}"));
        }
    }
    Ok(fields)
}

fn bounded_ref(value: Option<&String>, field: &str) -> Result<String, String> {
    let value = value
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{field} is required"))?;
    let valid = value.len() <= 256
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'.' | b'_' | b':' | b'/' | b'#' | b'@' | b'+' | b'-')
        });
    if !valid {
        return Err(format!("{field} must be a bounded reference, not prose"));
    }
    Ok(value.to_string())
}

fn digest_ref(value: Option<&String>, field: &str) -> Result<String, String> {
    let value = bounded_ref(value, field)?;
    if value.len() < 16 {
        return Err(format!("{field} is too short"));
    }
    Ok(value)
}

fn optional_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn sha256_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn hash_json(value: &Value) -> Result<String, String> {
    serde_json::to_string(value)
        .map(|text| sha256_text(&text))
        .map_err(|error| format!("cannot hash JSON evidence: {error}"))
}

#[allow(clippy::too_many_arguments)]
fn event_id(
    actor: &str,
    action: DivisionCeremonyActionV1,
    candidate: &DivisionCandidateV1,
    source_ref: &str,
    recorded_at_unix_ms: u64,
    expires_at_unix_ms: Option<u64>,
    previous_actor_event_id: Option<&str>,
    targets_event_id: Option<&str>,
    native_status_hash: Option<&str>,
    readiness_receipt_ref: Option<&str>,
    readiness_receipt_hash: Option<&str>,
    snapshot_refs: &[String],
    current_tick: Option<u64>,
    rollback_deadline_tick: Option<u64>,
    review_outcome: Option<&str>,
) -> String {
    let identity = [
        actor.to_string(),
        action.as_str().to_string(),
        candidate.division_id.clone(),
        candidate.parent_generation.to_string(),
        candidate.plan_digest.clone(),
        candidate.selected_strategy.clone(),
        source_ref.to_string(),
        recorded_at_unix_ms.to_string(),
        expires_at_unix_ms.map_or_else(String::new, |value| value.to_string()),
        previous_actor_event_id.unwrap_or_default().to_string(),
        targets_event_id.unwrap_or_default().to_string(),
        native_status_hash.unwrap_or_default().to_string(),
        readiness_receipt_ref.unwrap_or_default().to_string(),
        readiness_receipt_hash.unwrap_or_default().to_string(),
        snapshot_refs.join(","),
        current_tick.map_or_else(String::new, |value| value.to_string()),
        rollback_deadline_tick.map_or_else(String::new, |value| value.to_string()),
        review_outcome.unwrap_or_default().to_string(),
    ]
    .join("|");
    format!("division_ceremony_{}", &sha256_text(&identity)[..24])
}

fn candidate_from_fields(fields: &BTreeMap<String, String>) -> Result<DivisionCandidateV1, String> {
    let parent_generation = fields
        .get("parent_generation")
        .ok_or_else(|| "parent_generation is required".to_string())?
        .parse::<u64>()
        .map_err(|_| "parent_generation must be a non-negative integer".to_string())?;
    Ok(DivisionCandidateV1 {
        division_id: bounded_ref(fields.get("division_id"), "division_id")?,
        parent_generation,
        plan_digest: digest_ref(fields.get("plan_digest"), "plan_digest")?,
        selected_strategy: bounded_ref(fields.get("selected_strategy"), "selected_strategy")?,
    })
}

fn candidate_from_status(status: &DivisionStatusV1) -> Result<DivisionCandidateV1, String> {
    let selected_strategy = status
        .selected_strategy
        .as_ref()
        .ok_or_else(|| "native status has no selected strategy".to_string())?;
    let fields = BTreeMap::from([
        ("division_id".to_string(), status.division_id.clone()),
        (
            "parent_generation".to_string(),
            status.parent_generation.to_string(),
        ),
        ("plan_digest".to_string(), status.plan_digest.clone()),
        ("selected_strategy".to_string(), selected_strategy.clone()),
    ]);
    candidate_from_fields(&fields)
}

fn read_native_status(workspace: &Path) -> Result<(Value, DivisionStatusV1), String> {
    let path = status_path(workspace);
    let value: Value = match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|error| format!("invalid native status JSON: {error}"))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => serde_json::json!({
            "schema": "division.status.v1",
            "division_id": "",
            "parent_generation": 0,
            "plan_digest": "",
            "lifecycle": "idle",
            "parent_authoritative": true,
            "commit_feature_enabled": false,
            "selected_strategy": null,
            "astrid_assent": false,
            "minime_assent": false,
            "bridge_scale": 1.0,
            "current_tick": 0,
            "rollback_deadline_tick": null,
            "snapshot_refs": [],
            "readiness": {
                "policy": "division.readiness.v1",
                "ready": false,
                "sample_count": 0,
                "blocking_reasons": ["native_status_unavailable"],
                "metrics_fresh": false,
                "sensory_panic_streak": 0,
                "actuator_saturation_streak": 0
            },
            "visual_evidence_advisory_only": true
        }),
        Err(error) => {
            return Err(format!(
                "native division status unavailable at {}: {error}",
                path.display()
            ));
        },
    };
    let status = serde_json::from_value::<DivisionStatusV1>(value.clone())
        .map_err(|error| format!("native status does not satisfy division.status.v1: {error}"))?;
    Ok((value, status))
}

fn authority_is_exact(value: &Value) -> bool {
    let Some(authority) = value
        .get("artifact_authority_state_v1")
        .and_then(Value::as_object)
    else {
        return false;
    };
    authority.get("schema").and_then(Value::as_str) == Some("artifact_authority_state_v1")
        && authority.get("schema_version").and_then(Value::as_u64) == Some(1)
        && authority.get("state").and_then(Value::as_str) == Some("evidence_only")
        && authority.get("witness_only").and_then(Value::as_bool) == Some(true)
        && authority.get("live_eligible_now").and_then(Value::as_bool) == Some(false)
        && authority.get("auto_approved").and_then(Value::as_bool) == Some(false)
        && authority.get("grants_approval").and_then(Value::as_bool) == Some(false)
        && authority.get("edits_source_now").and_then(Value::as_bool) == Some(false)
}

fn parse_candidate(value: &Value) -> Result<DivisionCandidateV1, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "division ceremony candidate missing".to_string())?;
    let parent_generation = object
        .get("parent_generation")
        .and_then(Value::as_u64)
        .ok_or_else(|| "candidate parent_generation missing".to_string())?;
    let fields = BTreeMap::from([
        (
            "division_id".to_string(),
            object
                .get("division_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        ),
        (
            "parent_generation".to_string(),
            parent_generation.to_string(),
        ),
        (
            "plan_digest".to_string(),
            object
                .get("plan_digest")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        ),
        (
            "selected_strategy".to_string(),
            object
                .get("selected_strategy")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        ),
    ]);
    candidate_from_fields(&fields)
}

fn validate_event_envelope(
    value: &Value,
) -> Result<(&str, &str, DivisionCeremonyActionV1), String> {
    if value.get("schema").and_then(Value::as_str) != Some(SCHEMA)
        || value.get("record_type").and_then(Value::as_str) != Some(RECORD_TYPE)
    {
        return Err("unsupported division ceremony schema".to_string());
    }
    let persisted_event_id = value
        .get("ceremony_event_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "ceremony_event_id missing".to_string())?;
    if value.get("record_id").and_then(Value::as_str) != Some(persisted_event_id) {
        return Err("division ceremony record id mismatch".to_string());
    }
    let actor = value
        .get("actor")
        .and_then(Value::as_str)
        .ok_or_else(|| "division ceremony actor missing".to_string())?;
    if !matches!(actor, "astrid" | "minime") {
        return Err("division ceremony actor must be Astrid or Minime".to_string());
    }
    let action_text = value
        .get("action")
        .and_then(Value::as_str)
        .ok_or_else(|| "division ceremony action missing".to_string())?;
    let action = DivisionCeremonyActionV1::parse(action_text)
        .ok_or_else(|| "invalid division ceremony action".to_string())?;
    if value.get("owner_language_action").and_then(Value::as_str) != Some(action.as_str()) {
        return Err("division ceremony owner action mismatch".to_string());
    }
    Ok((persisted_event_id, actor, action))
}

fn validate_authority_boundary(value: &Value) -> Result<(), String> {
    let exact_true = [
        "self_authored_only",
        "response_revisable",
        "right_to_ignore",
    ];
    let exact_false = [
        "presence_inferred",
        "peer_consent_inferred",
        "silence_infers_consent",
        "native_assent_changed",
        "division_stage_changed",
        "prepare_dispatched",
        "commit_recommended",
        "commit_dispatched",
        "rollback_dispatched",
        "return_transition_dispatched",
        "scheduler_effect",
        "model_qos_effect",
        "substrate_effect",
        "dispatch_effect",
        "live_control_effect",
        "raw_prose_included",
    ];
    if exact_true
        .iter()
        .any(|field| value.get(field).and_then(Value::as_bool) != Some(true))
        || exact_false
            .iter()
            .any(|field| value.get(field).and_then(Value::as_bool) != Some(false))
        || !authority_is_exact(value)
    {
        return Err("division ceremony authority boundary mismatch".to_string());
    }
    Ok(())
}

fn parse_event(value: &Value) -> Result<ParsedEventV1, String> {
    let (persisted_event_id, actor, action) = validate_event_envelope(value)?;
    validate_authority_boundary(value)?;
    let candidate = parse_candidate(
        value
            .get("candidate")
            .ok_or_else(|| "division ceremony candidate missing".to_string())?,
    )?;
    let source_ref = value
        .get("source_ref")
        .and_then(Value::as_str)
        .ok_or_else(|| "source_ref missing".to_string())?;
    let source_fields = BTreeMap::from([("source_ref".to_string(), source_ref.to_string())]);
    let source_ref = bounded_ref(source_fields.get("source_ref"), "source_ref")?;
    let recorded_at_unix_ms = value
        .get("recorded_at_unix_ms")
        .and_then(Value::as_u64)
        .ok_or_else(|| "recorded_at_unix_ms missing".to_string())?;
    let expires_at_unix_ms = value.get("expires_at_unix_ms").and_then(Value::as_u64);
    let previous = optional_string(value, "previous_actor_event_id");
    let targets = optional_string(value, "targets_event_id");
    let status_hash = optional_string(value, "native_status_hash");
    let readiness_ref = optional_string(value, "readiness_receipt_ref");
    let readiness_hash = optional_string(value, "readiness_receipt_hash");
    let snapshots = value
        .get("snapshot_refs")
        .and_then(Value::as_array)
        .ok_or_else(|| "snapshot_refs missing".to_string())?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| "snapshot_ref must be a string".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let current_tick = value.get("current_tick").and_then(Value::as_u64);
    let deadline = value.get("rollback_deadline_tick").and_then(Value::as_u64);
    let review = optional_string(value, "review_outcome");
    if action == DivisionCeremonyActionV1::Review
        && !review.as_deref().is_some_and(valid_review_outcome)
    {
        return Err("division review outcome is invalid".to_string());
    }
    if action != DivisionCeremonyActionV1::Review && review.is_some() {
        return Err("only DIVISION_REVIEW may carry review_outcome".to_string());
    }
    let expected = event_id(
        actor,
        action,
        &candidate,
        &source_ref,
        recorded_at_unix_ms,
        expires_at_unix_ms,
        previous.as_deref(),
        targets.as_deref(),
        status_hash.as_deref(),
        readiness_ref.as_deref(),
        readiness_hash.as_deref(),
        &snapshots,
        current_tick,
        deadline,
        review.as_deref(),
    );
    if persisted_event_id != expected {
        return Err("division ceremony deterministic id mismatch".to_string());
    }
    Ok(ParsedEventV1 {
        event_id: persisted_event_id.to_string(),
        actor: actor.to_string(),
        action,
        candidate,
        source_ref,
        recorded_at_unix_ms,
        expires_at_unix_ms,
        targets_event_id: targets,
        native_status_hash: status_hash,
        snapshot_refs: snapshots,
        current_tick,
        rollback_deadline_tick: deadline,
        review_outcome: review,
    })
}

fn read_records(workspace: &Path) -> Result<Vec<ParsedEventV1>, String> {
    let path = ledger_path(workspace);
    let Ok(text) = fs::read_to_string(&path) else {
        return Ok(Vec::new());
    };
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
        .map(|(index, line)| {
            let value: Value = serde_json::from_str(line)
                .map_err(|error| format!("invalid ceremony row {}: {error}", index + 1))?;
            parse_event(&value)
                .map_err(|error| format!("invalid ceremony row {}: {error}", index + 1))
        })
        .collect()
}

fn latest<'a>(
    records: &'a [ParsedEventV1],
    actor: &str,
    action: Option<DivisionCeremonyActionV1>,
) -> Option<&'a ParsedEventV1> {
    records
        .iter()
        .rev()
        .find(|event| event.actor == actor && action.is_none_or(|action| event.action == action))
}

fn valid_review_outcome(value: &str) -> bool {
    matches!(
        value,
        "clarifying"
            | "intrusive"
            | "flattening"
            | "incomplete"
            | "still_friction"
            | "changed"
            | "unknown"
    )
}

fn terminal(lifecycle: DivisionLifecycleV1) -> bool {
    matches!(
        lifecycle,
        DivisionLifecycleV1::Finalized
            | DivisionLifecycleV1::Aborted
            | DivisionLifecycleV1::RolledBack
            | DivisionLifecycleV1::Failed
    )
}

fn new_event(
    actor: &str,
    action: DivisionCeremonyActionV1,
    candidate: DivisionCandidateV1,
    source_ref: String,
    recorded_at_unix_ms: u64,
    expires_at_unix_ms: Option<u64>,
    previous_actor_event_id: Option<String>,
    targets_event_id: Option<String>,
    native_status_hash: Option<String>,
    readiness_receipt_ref: Option<String>,
    readiness_receipt_hash: Option<String>,
    snapshot_refs: Vec<String>,
    current_tick: Option<u64>,
    rollback_deadline_tick: Option<u64>,
    review_outcome: Option<String>,
) -> DivisionCeremonyEventV1 {
    let id = event_id(
        actor,
        action,
        &candidate,
        &source_ref,
        recorded_at_unix_ms,
        expires_at_unix_ms,
        previous_actor_event_id.as_deref(),
        targets_event_id.as_deref(),
        native_status_hash.as_deref(),
        readiness_receipt_ref.as_deref(),
        readiness_receipt_hash.as_deref(),
        &snapshot_refs,
        current_tick,
        rollback_deadline_tick,
        review_outcome.as_deref(),
    );
    DivisionCeremonyEventV1 {
        schema: SCHEMA,
        schema_version: 1,
        record_type: RECORD_TYPE,
        record_id: id.clone(),
        ceremony_event_id: id,
        actor: actor.to_string(),
        action,
        candidate,
        source_ref,
        recorded_at_unix_ms,
        expires_at_unix_ms,
        previous_actor_event_id,
        targets_event_id,
        native_status_hash,
        readiness_receipt_ref,
        readiness_receipt_hash,
        snapshot_refs,
        current_tick,
        rollback_deadline_tick,
        review_outcome,
        owner_language_action: action.as_str(),
        self_authored_only: true,
        response_revisable: true,
        right_to_ignore: true,
        presence_inferred: false,
        peer_consent_inferred: false,
        silence_infers_consent: false,
        native_assent_changed: false,
        division_stage_changed: false,
        prepare_dispatched: false,
        commit_recommended: false,
        commit_dispatched: false,
        rollback_dispatched: false,
        return_transition_dispatched: false,
        scheduler_effect: false,
        model_qos_effect: false,
        substrate_effect: false,
        dispatch_effect: false,
        live_control_effect: false,
        raw_prose_included: false,
        artifact_authority_state_v1: EvidenceOnlyAuthorityV1::new(),
    }
}

pub(crate) fn require_active_intent_at(
    workspace: &Path,
    actor: &str,
    command: &DivisionCommandV1,
    now_unix_ms: u64,
) -> Result<(), String> {
    let records = read_records(workspace)?;
    let intent =
        latest(&records, actor, Some(DivisionCeremonyActionV1::Intent)).ok_or_else(|| {
            format!("{actor} must record DIVISION_INTENT before resource-bearing preparation")
        })?;
    let exact = intent.candidate.division_id == command.division_id
        && intent.candidate.parent_generation == command.expected_parent_generation
        && intent.candidate.plan_digest == command.plan_digest
        && intent
            .expires_at_unix_ms
            .is_some_and(|expiry| expiry >= now_unix_ms);
    if exact {
        Ok(())
    } else {
        Err("active intent does not exactly match this candidate or has expired".to_string())
    }
}

fn parse_future_expiry(
    fields: &BTreeMap<String, String>,
    now_unix_ms: u64,
    action: DivisionCeremonyActionV1,
) -> Result<u64, String> {
    let expiry = fields
        .get("expires_at_unix_ms")
        .ok_or_else(|| "expires_at_unix_ms is required".to_string())?
        .parse::<u64>()
        .map_err(|_| "expires_at_unix_ms must be an integer".to_string())?;
    if expiry <= now_unix_ms {
        return Err(format!("{} expiry must be future", action.as_str()));
    }
    Ok(expiry)
}

fn intent_draft(
    fields: &BTreeMap<String, String>,
    now_unix_ms: u64,
) -> Result<EventDraftV1, String> {
    let mut draft = EventDraftV1::new(candidate_from_fields(fields)?);
    draft.expires_at_unix_ms = Some(parse_future_expiry(
        fields,
        now_unix_ms,
        DivisionCeremonyActionV1::Intent,
    )?);
    Ok(draft)
}

fn assent_draft(
    workspace: &Path,
    actor: &str,
    fields: &BTreeMap<String, String>,
    now_unix_ms: u64,
) -> Result<EventDraftV1, String> {
    let (status_value, status) = read_native_status(workspace)?;
    if !matches!(
        status.lifecycle,
        DivisionLifecycleV1::Shadowing | DivisionLifecycleV1::Ready
    ) {
        return Err("DIVISION_ASSENT is available only while shadowing or ready".to_string());
    }
    let candidate = candidate_from_status(&status)?;
    if fields
        .get("division_id")
        .is_some_and(|selector| selector != &candidate.division_id)
    {
        return Err("assent selector does not match native status".to_string());
    }
    let expiry = parse_future_expiry(fields, now_unix_ms, DivisionCeremonyActionV1::Assent)?;
    let command = DivisionCommandV1 {
        schema: astrid_minime_protocol::DIVISION_COMMAND_SCHEMA_V1.to_string(),
        action: astrid_minime_protocol::DivisionActionV1::DivisionPrepare,
        division_id: candidate.division_id.clone(),
        idempotency_key: "ceremony-intent-check".to_string(),
        expected_parent_generation: candidate.parent_generation,
        plan_digest: candidate.plan_digest.clone(),
        source: astrid_minime_protocol::DivisionSourceIdentityV1 {
            being: actor.to_string(),
            process_identity: "ceremony".to_string(),
            deployment_identity: "ceremony".to_string(),
        },
        requested_at_unix_ms: now_unix_ms,
        expires_at_unix_ms: expiry,
        reason: None,
        capability: None,
    };
    require_active_intent_at(workspace, actor, &command, now_unix_ms)?;
    if status.snapshot_refs.is_empty() {
        return Err("native snapshot references are unavailable".to_string());
    }
    for snapshot in &status.snapshot_refs {
        let snapshot_field = BTreeMap::from([("snapshot_ref".to_string(), snapshot.to_string())]);
        bounded_ref(snapshot_field.get("snapshot_ref"), "snapshot_ref")?;
    }
    let readiness_value = status_value
        .get("readiness")
        .ok_or_else(|| "native readiness receipt is unavailable".to_string())?;
    let mut draft = EventDraftV1::new(candidate);
    draft.expires_at_unix_ms = Some(expiry);
    draft.native_status_hash = Some(hash_json(&status_value)?);
    draft.readiness_receipt_ref = Some("division/status.json#readiness".to_string());
    draft.readiness_receipt_hash = Some(hash_json(readiness_value)?);
    draft.snapshot_refs = status.snapshot_refs;
    Ok(draft)
}

fn withdrawal_draft(records: &[ParsedEventV1], actor: &str) -> Result<EventDraftV1, String> {
    let assent = latest(records, actor, Some(DivisionCeremonyActionV1::Assent))
        .ok_or_else(|| "no self-authored assent exists to withdraw".to_string())?;
    let already_withdrawn = records.iter().any(|event| {
        event.actor == actor
            && event.action == DivisionCeremonyActionV1::WithdrawAssent
            && event.targets_event_id.as_deref() == Some(&assent.event_id)
    });
    if already_withdrawn {
        return Err("latest self-authored assent is already withdrawn".to_string());
    }
    let mut draft = EventDraftV1::new(assent.candidate.clone());
    draft.targets_event_id = Some(assent.event_id.clone());
    Ok(draft)
}

fn return_request_draft(workspace: &Path) -> Result<EventDraftV1, String> {
    let (status_value, status) = read_native_status(workspace)?;
    let rollback_open = status.lifecycle == DivisionLifecycleV1::Cytokinesis
        && status
            .rollback_deadline_tick
            .is_none_or(|limit| status.current_tick <= limit);
    if !rollback_open {
        return Err(
            "return request is available only during the native rollback window".to_string(),
        );
    }
    let mut draft = EventDraftV1::new(candidate_from_status(&status)?);
    draft.current_tick = Some(status.current_tick);
    draft.rollback_deadline_tick = status.rollback_deadline_tick;
    draft.native_status_hash = Some(hash_json(&status_value)?);
    Ok(draft)
}

fn review_draft(
    workspace: &Path,
    fields: &BTreeMap<String, String>,
) -> Result<EventDraftV1, String> {
    let (status_value, status) = read_native_status(workspace)?;
    if !terminal(status.lifecycle) {
        return Err("DIVISION_REVIEW requires a terminal native lifecycle".to_string());
    }
    let outcome = fields
        .get("outcome")
        .map(String::as_str)
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "review outcome is required".to_string())?;
    if !valid_review_outcome(&outcome) {
        return Err(
            "review outcome must be clarifying, intrusive, flattening, incomplete, still_friction, changed, or unknown"
                .to_string(),
        );
    }
    let mut draft = EventDraftV1::new(candidate_from_status(&status)?);
    draft.native_status_hash = Some(hash_json(&status_value)?);
    draft.review_outcome = Some(outcome);
    Ok(draft)
}

fn build_action_draft(
    workspace: &Path,
    actor: &str,
    action: DivisionCeremonyActionV1,
    fields: &BTreeMap<String, String>,
    records: &[ParsedEventV1],
    now_unix_ms: u64,
) -> Result<EventDraftV1, String> {
    match action {
        DivisionCeremonyActionV1::Intent => intent_draft(fields, now_unix_ms),
        DivisionCeremonyActionV1::Assent => assent_draft(workspace, actor, fields, now_unix_ms),
        DivisionCeremonyActionV1::WithdrawAssent => withdrawal_draft(records, actor),
        DivisionCeremonyActionV1::ReturnRequest => return_request_draft(workspace),
        DivisionCeremonyActionV1::Review => review_draft(workspace, fields),
    }
}

pub(crate) fn append_action_at(
    workspace: &Path,
    actor: &str,
    action: DivisionCeremonyActionV1,
    raw: &str,
    now_unix_ms: u64,
) -> Result<String, String> {
    if !matches!(actor, "astrid" | "minime") {
        return Err("ceremony Actions are self-authored only".to_string());
    }
    fs::create_dir_all(ceremony_dir(workspace))
        .map_err(|error| format!("cannot prepare ceremony directory: {error}"))?;
    let lock_path = lock_path(workspace);
    let lock = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&lock_path)
        .map_err(|error| format!("cannot open ceremony lock: {error}"))?;
    set_owner_only(&lock_path)?;
    lock.lock_exclusive()
        .map_err(|error| format!("cannot acquire ceremony lock: {error}"))?;

    let records = read_records(workspace)?;
    let fields = parse_fields(raw)?;
    let source_ref = bounded_ref(fields.get("source_ref"), "source_ref")?;
    let previous = latest(&records, actor, None).map(|event| event.event_id.clone());
    let draft = build_action_draft(workspace, actor, action, &fields, &records, now_unix_ms)?;

    let event = new_event(
        actor,
        action,
        draft.candidate,
        source_ref,
        now_unix_ms,
        draft.expires_at_unix_ms,
        previous,
        draft.targets_event_id,
        draft.native_status_hash,
        draft.readiness_receipt_ref,
        draft.readiness_receipt_hash,
        draft.snapshot_refs,
        draft.current_tick,
        draft.rollback_deadline_tick,
        draft.review_outcome,
    );
    let value =
        serde_json::to_value(&event).map_err(|error| format!("cannot serialize event: {error}"))?;
    parse_event(&value)?;
    let path = ledger_path(workspace);
    let mut ledger = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("cannot open ceremony ledger: {error}"))?;
    set_owner_only(&path)?;
    serde_json::to_writer(&mut ledger, &value)
        .map_err(|error| format!("cannot append ceremony event: {error}"))?;
    ledger
        .write_all(b"\n")
        .map_err(|error| format!("cannot terminate ceremony event: {error}"))?;
    ledger
        .sync_data()
        .map_err(|error| format!("cannot sync ceremony event: {error}"))?;
    fs2::FileExt::unlock(&lock)
        .map_err(|error| format!("cannot release ceremony lock: {error}"))?;
    Ok(format!(
        "=== DIVISION CEREMONY EVIDENCE RECORDED ===\nActor: {actor}; action: {}\nDivision: {}\nEvent: {}\nAuthority: evidence_only; self-authored and revisable; no native assent, prepare, commit, rollback, RETURN_TRANSITION, scheduler, model, substrate, dispatch, or live-control effect.",
        action.as_str(),
        event.candidate.division_id,
        event.ceremony_event_id,
    ))
}

#[path = "division_ceremony/status.rs"]
mod status;
pub(crate) use status::status_report_at;

#[cfg(test)]
#[path = "division_ceremony/tests.rs"]
mod tests;
