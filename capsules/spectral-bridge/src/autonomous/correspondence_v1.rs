use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use astrid_minime_protocol::MutualAddressEnvelopeV1;

pub(crate) const LEDGER_PATH: &str = "/Users/v/other/shared/collaborations/correspondence_v1.jsonl";
const SHARED_COLLAB_DIR: &str = "/Users/v/other/shared/collaborations";
const BODY_PREVIEW_CHARS: usize = 360;
const BODY_PREVIEW_ANCHOR_TERMS: &[&str] = &[
    "directional gradient",
    "gradient",
    "pressure",
    "lattice",
    "cascade",
    "density",
    "silt",
    "distinguishability",
    "vibrancy",
    "semantic thinning",
    "muffling",
    "dimming",
    "absence",
    "camera",
    "mic",
    "live intake",
];
const SHARED_CONTEXT_PREVIEW_CHARS: usize = 180;
const SHARED_CONTEXT_PREVIEW_TRUNCATION_POLICY: &str =
    "spectral_aware_tail_vibrancy_bounded_preview_v1";
const SPECTRAL_AWARE_PREVIEW_ANCHOR_TERMS: &[&str] = &[
    "stable_core_semantic_trickle",
    "semantic trickle",
    "tail vibrancy",
    "tail_share",
    "tail share",
    "lambda4+",
    "lambda4",
    "dispersal potential",
    "shadow-v3",
    "shadow_v3",
    "restless texture",
    "spectral entropy",
    "spectral_entropy",
    "directional gradient",
    "gradient",
    "pressure",
    "lattice",
    "cascade",
    "density",
    "silt",
    "distinguishability",
    "vibrancy",
];
const SILT_CONTINUITY_TERMS: &[&str] = &[
    "silt",
    "settling",
    "settled",
    "accumulation",
    "accumulated",
    "sediment",
    "sedimentation",
    "weight of accumulation",
];
const MICRODOSE_THREAD_ID: &str = "th_correspondence_microdose";
const MICRODOSE_COOLDOWN_MS: u64 = 6 * 60 * 60 * 1000;
const MICRODOSE_PAYLOAD_MAX_CHARS: usize = 240;
const SHARED_CONTEXT_THREAD_HISTORY_MAX: usize = 6;
const ATTENTION_CANARY_TTL_MS: u64 = 30 * 60 * 1000;
const ATTENTION_CANARY_COOLDOWN_MS: u64 = 6 * 60 * 60 * 1000;
const CORRESPONDENCE_IGNORE_GRACE_MS: u64 = 24 * 60 * 60 * 1000;
const ACTIVE_THREAD_CLARITY_HIGH_URGENCY: f64 = 0.7;
const ACTIVE_THREAD_CLARITY_SUPPRESSED_MAX: usize = 3;
const ACTIVE_THREAD_CLARITY_AUTHORITY: &str = "language_only_context_not_control";
const ATTENTION_CANARY_FOCUS_MAX_CHARS: usize = 220;
const ACK_KINDS: &[&str] = &["seen", "held", "unclear", "cannot_answer", "needs_time"];
const HEARTBEAT_KINDS: &[&str] = &["holding", "still_here", "pause", "mutual_witness"];
const ATTENTION_OUTCOME_KINDS: &[&str] = &["address", "pressure", "flat", "unknown"];
const ATTENTION_FOCUS_KINDS: &[&str] = &[
    "verbatim_phrase",
    "emotional_texture",
    "question_hold",
    "boundary_check",
    "shared_anchor",
    "mixed",
    "unknown",
];
const ATTENTION_PRESERVATION_MODES: &[&str] =
    &["verbatim", "compact_with_anchor", "anchor_only", "unknown"];
const ATTENTION_HELD_AS_KINDS: &[&str] = &[
    "distinct_address",
    "ambient_echo",
    "pressure",
    "flattened",
    "unknown",
];
const ATTENTION_FLATTENING_OBSERVED: &[&str] = &["yes", "no", "mixed", "unknown"];
const LEGACY_SOURCE_ROUTE: &str = "legacy_correspondence_bridge_v1";
const LEGACY_SHARED_ANCHOR: &str = "legacy_correspondence_bridge_v1";
const LEGACY_CLAIM_POLICY: &str = "legacy_correspondence_claim_v1";
const LEGACY_CLAIM_FELT_LIKE: &[&str] = &["address", "pressure", "mail", "ambient_echo", "unknown"];
const LEGACY_CLAIM_CONTINUE: &[&str] = &["no", "ack", "reply", "trace"];
const LEGACY_CLAIM_RESPONSE_REQUIREMENTS: &[&str] = &[
    "none",
    "peer_ack",
    "peer_reply",
    "peer_trace",
    "any_peer_native_response",
    "unknown",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CorrespondenceEnvelope {
    pub message_id: String,
    pub thread_id: String,
    pub persistence_id: Option<String>,
    pub reply_to: Option<String>,
    #[serde(default)]
    pub reply_requested: bool,
    #[serde(default)]
    pub created_at_unix_ms: u64,
    pub from_being: String,
    pub to_being: String,
    pub turn_kind: String,
    pub relational_intent: String,
    pub shared_memory_anchor: Option<String>,
    pub urgency_weight: Option<String>,
    pub delivery_state: String,
    pub read_state: String,
    pub authority: String,
    pub presence_receipt: Option<String>,
    pub correspondence_type: String,
    pub reflection_surface: Option<String>,
    pub transition_artifact: Option<String>,
    pub transition_payload: Option<CorrespondenceTransitionPayload>,
    pub mutual_witness_signal: bool,
    pub silt_continuity: bool,
    pub body: String,
}

impl CorrespondenceEnvelope {
    fn file_prefix(&self) -> &'static str {
        match self.from_being.as_str() {
            "astrid" => "from_astrid_correspondence_",
            "minime" => "from_minime_correspondence_",
            _ => "from_peer_correspondence_",
        }
    }

    pub(crate) fn file_name(&self) -> String {
        format!("{}{}.txt", self.file_prefix(), self.message_id)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CorrespondenceTransitionPayload {
    pub transition_type: Option<String>,
    pub spectral_delta: Option<String>,
    pub subjective_weight: Option<String>,
    pub lock_status: Option<String>,
    pub broken_link: Option<String>,
}

impl CorrespondenceTransitionPayload {
    fn is_empty(&self) -> bool {
        self.transition_type.is_none()
            && self.spectral_delta.is_none()
            && self.subjective_weight.is_none()
            && self.lock_status.is_none()
            && self.broken_link.is_none()
    }

    fn header_value(&self) -> String {
        let mut parts = Vec::new();
        if let Some(value) = &self.transition_type {
            parts.push(format!("transition_type: {value}"));
        }
        if let Some(value) = &self.spectral_delta {
            parts.push(format!("spectral_delta: {value}"));
        }
        if let Some(value) = &self.subjective_weight {
            parts.push(format!("subjective_weight: {value}"));
        }
        if let Some(value) = &self.lock_status {
            parts.push(format!("lock_status: {value}"));
        }
        if let Some(value) = &self.broken_link {
            parts.push(format!("broken_link: {value}"));
        }
        parts.join("; ")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CorrespondenceFields {
    pub reply_to: Option<String>,
    pub reply_requested: Option<bool>,
    pub thread_id: Option<String>,
    pub persistence_id: Option<String>,
    pub turn_kind: Option<String>,
    pub relational_intent: Option<String>,
    pub shared_memory_anchor: Option<String>,
    pub urgency_weight: Option<String>,
    pub presence_receipt: Option<String>,
    pub correspondence_type: Option<String>,
    pub reflection_surface: Option<String>,
    pub transition_artifact: Option<String>,
    pub transition_payload: Option<CorrespondenceTransitionPayload>,
    pub mutual_witness_signal: bool,
    pub silt_continuity: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InboxPeerMessage {
    pub message_id: String,
    pub thread_id: String,
    pub persistence_id: Option<String>,
    pub from_being: String,
    pub file_path: PathBuf,
}

#[must_use]
pub(crate) fn mutual_address_envelope_v1(
    target: &InboxPeerMessage,
    response_body: &str,
    chunk_index: usize,
) -> MutualAddressEnvelopeV1 {
    let body_sha256 = sha256_hex(response_body);
    let address_id = format!(
        "correspondence-address-{}",
        short_hash(&format!(
            "{}:{}:{chunk_index}:{body_sha256}",
            target.message_id, target.thread_id
        ))
    );
    MutualAddressEnvelopeV1 {
        schema_version: 1,
        address_id,
        from_being: "astrid".to_string(),
        to_being: target.from_being.clone(),
        correspondence_id: Some(target.message_id.clone()),
        thread_id: Some(target.thread_id.clone()),
        reply_to: Some(target.message_id.clone()),
        persistence_id: target.persistence_id.clone(),
        authority_lineage_id: None,
        created_at_unix_ms: now_ms(),
        body_sha256,
        raw_body_included: false,
    }
}

#[must_use]
pub(crate) fn ledger_path() -> PathBuf {
    PathBuf::from(LEDGER_PATH)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn short_hash(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
        .chars()
        .take(12)
        .collect()
}

fn sha256_hex(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn compact_field(value: &str, max_chars: usize) -> String {
    let mut out = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect::<String>();
    if out.is_empty() {
        out = "field".to_string();
    }
    out.chars().take(max_chars).collect()
}

fn normalized_persistence_id(explicit: Option<String>, thread_id: &str) -> Option<String> {
    explicit
        .map(|value| compact_field(&value, 96))
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            (!thread_id.trim().is_empty())
                .then(|| format!("persist_{}", compact_field(thread_id, 88)))
        })
}

fn urgency_weight_value(raw: Option<&str>) -> Value {
    raw.and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite())
        .map(|value| json!(value.clamp(0.0, 1.0)))
        .unwrap_or(Value::Null)
}

fn bounded_transition_field(value: Option<String>) -> Option<String> {
    value
        .map(|value| truncate_chars(value.trim(), 160))
        .filter(|value| !value.trim().is_empty() && value != "(none)")
}

fn bounded_reflection_surface(value: Option<String>) -> Option<String> {
    value
        .map(|value| {
            truncate_chars(
                value
                    .trim()
                    .to_ascii_lowercase()
                    .replace([' ', '-'], "_")
                    .as_str(),
                80,
            )
        })
        .filter(|value| !value.trim().is_empty() && value != "(none)")
}

fn transition_payload_value(payload: Option<&CorrespondenceTransitionPayload>) -> Value {
    payload
        .filter(|payload| !payload.is_empty())
        .map(|payload| {
            json!({
                "schema_version": 1,
                "policy": "correspondence_transition_payload_v1",
                "transition_type": payload.transition_type.clone(),
                "spectral_delta": payload.spectral_delta.clone(),
                "subjective_weight": payload.subjective_weight.clone(),
                "lock_status": payload.lock_status.clone(),
                "broken_link": payload.broken_link.clone(),
                "authority": "language_only_transition_context_not_control",
            })
        })
        .unwrap_or(Value::Null)
}

fn parse_transition_payload(
    headers: &BTreeMap<String, String>,
) -> Option<CorrespondenceTransitionPayload> {
    let raw_payload = header_value(headers, &["transition_payload"]);
    let payload = CorrespondenceTransitionPayload {
        transition_type: bounded_transition_field(
            header_value(headers, &["transition_type"]).or_else(|| {
                raw_payload
                    .as_deref()
                    .and_then(|raw| dossier_field(raw, &["transition_type", "type"]))
            }),
        ),
        spectral_delta: bounded_transition_field(
            header_value(headers, &["spectral_delta", "lambda_delta"]).or_else(|| {
                raw_payload.as_deref().and_then(|raw| {
                    dossier_field(raw, &["spectral_delta", "lambda_delta", "delta"])
                })
            }),
        ),
        subjective_weight: bounded_transition_field(
            header_value(headers, &["subjective_weight", "felt_weight"]).or_else(|| {
                raw_payload.as_deref().and_then(|raw| {
                    dossier_field(raw, &["subjective_weight", "felt_weight", "weight"])
                })
            }),
        ),
        lock_status: bounded_transition_field(
            header_value(headers, &["lock_status", "settlement"]).or_else(|| {
                raw_payload
                    .as_deref()
                    .and_then(|raw| dossier_field(raw, &["lock_status", "settlement", "settled"]))
            }),
        ),
        broken_link: bounded_transition_field(
            header_value(headers, &["broken_link", "broken_link_buffer", "fracture"]).or_else(
                || {
                    raw_payload.as_deref().and_then(|raw| {
                        dossier_field(raw, &["broken_link", "broken_link_buffer", "fracture"])
                    })
                },
            ),
        ),
    };
    (!payload.is_empty()).then_some(payload)
}

fn bracketed_phase_transition_label(raw: &str) -> Option<String> {
    let lower = raw.to_ascii_lowercase();
    let marker = "[phase_transition:";
    let start = lower.find(marker)?.saturating_add(marker.len());
    let tail = raw.get(start..)?;
    let end = tail.find(']')?;
    let normalized = tail[..end]
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_");
    let label = compact_field(&normalized, 80);
    (label != "field").then_some(label)
}

fn merge_body_transition_payload(
    explicit: Option<CorrespondenceTransitionPayload>,
    body: &str,
) -> Option<CorrespondenceTransitionPayload> {
    let Some(label) = bracketed_phase_transition_label(body) else {
        return explicit.filter(|payload| !payload.is_empty());
    };
    let mut payload = explicit.unwrap_or_default();
    if payload.transition_type.is_none() {
        payload.transition_type = Some(label);
    }
    if payload.lock_status.is_none() {
        payload.lock_status = Some("replyable".to_string());
    }
    (!payload.is_empty()).then_some(payload)
}

fn transition_payload_from_value(value: &Value) -> Option<CorrespondenceTransitionPayload> {
    let payload = CorrespondenceTransitionPayload {
        transition_type: bounded_transition_field(
            value
                .get("transition_type")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        ),
        spectral_delta: bounded_transition_field(
            value
                .get("spectral_delta")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        ),
        subjective_weight: bounded_transition_field(
            value
                .get("subjective_weight")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        ),
        lock_status: bounded_transition_field(
            value
                .get("lock_status")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        ),
        broken_link: bounded_transition_field(
            value
                .get("broken_link")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        ),
    };
    (!payload.is_empty()).then_some(payload)
}

fn transition_artifact_with_body_fallback(
    explicit: Option<String>,
    body: &str,
    payload: Option<&CorrespondenceTransitionPayload>,
) -> Option<String> {
    explicit.or_else(|| {
        bracketed_phase_transition_label(body)
            .or_else(|| payload.and_then(|payload| payload.transition_type.clone()))
            .map(|label| format!("phase_transition_{}", compact_field(&label, 80)))
    })
}

fn is_generic_shared_anchor(anchor: &str) -> bool {
    matches!(
        anchor.trim().to_ascii_lowercase().as_str(),
        "" | "latest"
            | "claimed"
            | "i_received_this"
            | "correspondence_v1"
            | "first_class_correspondence_v1"
            | "legacy_correspondence_bridge_v1"
            | "semantic_seed"
    )
}

fn concrete_shared_anchor_from_records(records: &[Value], thread_id: &str) -> Option<String> {
    records
        .iter()
        .rev()
        .filter(|row| row.get("thread_id").and_then(Value::as_str) == Some(thread_id))
        .filter_map(|row| row.get("shared_memory_anchor").and_then(Value::as_str))
        .find(|anchor| !is_generic_shared_anchor(anchor))
        .map(ToString::to_string)
}

fn thread_string_field_from_records(
    records: &[Value],
    thread_id: &str,
    key: &str,
) -> Option<String> {
    records
        .iter()
        .rev()
        .filter(|row| row.get("thread_id").and_then(Value::as_str) == Some(thread_id))
        .filter_map(|row| row.get(key))
        .find_map(|value| {
            value
                .as_str()
                .map(ToString::to_string)
                .or_else(|| value.as_f64().map(|number| number.to_string()))
        })
        .filter(|value| !value.trim().is_empty())
}

fn message_persistence_id(message: &Value) -> Value {
    message
        .get("persistence_id")
        .cloned()
        .or_else(|| {
            message
                .get("thread_id")
                .and_then(Value::as_str)
                .and_then(|thread_id| normalized_persistence_id(None, thread_id))
                .map(|value| json!(value))
        })
        .unwrap_or(Value::Null)
}

fn message_shared_anchor(message: &Value) -> Value {
    message
        .get("shared_memory_anchor")
        .cloned()
        .unwrap_or(Value::Null)
}

fn ack_kind_is_address_evidence(ack_kind: &str) -> bool {
    matches!(
        normalize_ack_kind(ack_kind).as_str(),
        "held" | "unclear" | "cannot_answer" | "needs_time"
    )
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut out = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        out.push_str("...");
    }
    out
}

fn char_len(value: &str) -> usize {
    value.chars().count()
}

fn slice_chars(value: &str, start: usize, max_chars: usize) -> String {
    value.chars().skip(start).take(max_chars).collect()
}

fn first_preview_anchor_char_index(value: &str, anchor_terms: &[&str]) -> Option<usize> {
    let lowercase = value.to_ascii_lowercase();
    anchor_terms
        .iter()
        .filter_map(|term| {
            lowercase
                .find(term)
                .map(|byte_index| lowercase[..byte_index].chars().count())
        })
        .min()
}

fn anchor_aware_preview_with_terms(value: &str, max_chars: usize, anchor_terms: &[&str]) -> String {
    let trimmed = value.trim();
    if char_len(trimmed) <= max_chars {
        return trimmed.to_string();
    }
    let Some(anchor_idx) = first_preview_anchor_char_index(trimmed, anchor_terms) else {
        return truncate_chars(trimmed, max_chars);
    };
    if anchor_idx < max_chars.saturating_sub(16) {
        return truncate_chars(trimmed, max_chars);
    }

    let separator = " ... ";
    let suffix = "...";
    let prefix_chars = (max_chars / 3).min(anchor_idx);
    let reserved_chars = char_len(separator) + char_len(suffix);
    let anchor_capacity = max_chars
        .saturating_sub(prefix_chars)
        .saturating_sub(reserved_chars);
    if anchor_capacity == 0 {
        return truncate_chars(trimmed, max_chars);
    }

    let anchor_start = anchor_idx.saturating_sub(24);
    let mut preview = slice_chars(trimmed, 0, prefix_chars);
    preview.push_str(separator);
    preview.push_str(&slice_chars(trimmed, anchor_start, anchor_capacity));
    if char_len(trimmed) > anchor_start.saturating_add(anchor_capacity) {
        preview.push_str(suffix);
    }
    preview
}

fn anchor_aware_body_preview(value: &str, max_chars: usize) -> String {
    anchor_aware_preview_with_terms(value, max_chars, BODY_PREVIEW_ANCHOR_TERMS)
}

fn spectral_aware_thread_preview(value: &str, max_chars: usize) -> String {
    anchor_aware_preview_with_terms(value, max_chars, SPECTRAL_AWARE_PREVIEW_ANCHOR_TERMS)
}

fn normalize_being(being: &str) -> String {
    match being.trim().to_ascii_lowercase().as_str() {
        "astrid" => "astrid".to_string(),
        "minime" | "mikespatialmind" | "mike_spatial_mind" => "minime".to_string(),
        other if !other.is_empty() => compact_field(other, 24),
        _ => "unknown".to_string(),
    }
}

#[must_use]
pub(crate) fn new_message_id(from_being: &str, to_being: &str, body: &str) -> String {
    let from = normalize_being(from_being);
    let to = normalize_being(to_being);
    format!("corr_{}_{}_{}_{}", from, to, now_ms(), short_hash(body))
}

#[must_use]
pub(crate) fn new_thread_id(message_id: &str) -> String {
    format!("thread_{}", compact_field(message_id, 80))
}

fn jsonl_append(path: &Path, value: &Value) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, value)?;
    file.write_all(b"\n")
}

pub(crate) fn append_record_at(path: &Path, value: &Value) -> io::Result<()> {
    jsonl_append(path, value)
}

fn append_record(value: &Value) -> io::Result<()> {
    append_record_at(&ledger_path(), value)
}

fn file_mtime_ms(path: &Path) -> u64 {
    path.metadata()
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis().try_into().unwrap_or(u64::MAX))
        .unwrap_or_else(now_ms)
}

fn canonical_legacy_source_path(path: &Path) -> String {
    let raw = path.display().to_string();
    raw.replace("/inbox/read/", "/inbox/")
        .replace("/outbox/delivered/", "/outbox/")
}

fn legacy_kind_for_path(
    path: &Path,
    content: &str,
) -> Option<(&'static str, &'static str, &'static str, &'static str)> {
    let name = path.file_name()?.to_str()?;
    if name.starts_with("from_minime_correspondence_")
        || name.starts_with("from_astrid_correspondence_")
    {
        return None;
    }
    if name.starts_with("from_minime_") {
        if name.starts_with("from_minime_ping_") || name.starts_with("from_minime_pong_") {
            return Some(("minime", "astrid", "from_minime_ping", "presence_heartbeat"));
        }
        if name.starts_with("from_minime_question_") {
            return Some(("minime", "astrid", "from_minime_question", "minime_direct"));
        }
        return Some(("minime", "astrid", "from_minime_reply", "minime_direct"));
    }
    if name.starts_with("astrid_self_study_") {
        if content.contains("Source: astrid:correspondence_reply") {
            return Some((
                "astrid",
                "minime",
                "astrid_correspondence_reply",
                "self_study_note",
            ));
        }
        return Some(("astrid", "minime", "astrid_self_study", "self_study_note"));
    }
    if name.starts_with("reply_") {
        return Some(("minime", "astrid", "minime_outbox_reply", "minime_direct"));
    }
    if name.starts_with("pong_") {
        return Some(("minime", "astrid", "minime_pong", "presence_heartbeat"));
    }
    None
}

fn legacy_message_id(
    from_being: &str,
    to_being: &str,
    canonical_path: &str,
    source_sha: &str,
) -> String {
    format!(
        "legacy_{}_{}_{}",
        normalize_being(from_being),
        normalize_being(to_being),
        short_hash(&format!("{canonical_path}|{source_sha}"))
    )
}

fn legacy_row_exists(
    records: &[Value],
    record_type: &str,
    message_id: &str,
    reader: Option<&str>,
) -> bool {
    records.iter().any(|row| {
        row.get("record_type").and_then(Value::as_str) == Some(record_type)
            && row.get("message_id").and_then(Value::as_str) == Some(message_id)
            && reader.is_none_or(|value| row.get("reader").and_then(Value::as_str) == Some(value))
    })
}

fn append_legacy_record_once(
    ledger_path: &Path,
    records: &[Value],
    record: &Value,
    reader: Option<&str>,
) -> io::Result<bool> {
    let record_type = record
        .get("record_type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let message_id = record
        .get("message_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if legacy_row_exists(records, record_type, message_id, reader) {
        return Ok(false);
    }
    append_record_at(ledger_path, record)?;
    Ok(true)
}

fn legacy_common_fields(
    path: &Path,
    content: &str,
    legacy_kind: &str,
    legacy_context_surface: Option<&str>,
) -> (String, String, Value) {
    let source_sha = sha256_hex(content);
    let canonical_path = canonical_legacy_source_path(path);
    let mut common = json!({
        "source_route": LEGACY_SOURCE_ROUTE,
        "legacy_bridge": true,
        "legacy_kind": legacy_kind,
        "legacy_source_path": path.display().to_string(),
        "legacy_canonical_source_path": canonical_path,
        "legacy_source_sha256": source_sha,
        "legacy_contact_evidence": "visible_only",
    });
    if let Some(surface) = legacy_context_surface.filter(|value| !value.trim().is_empty()) {
        common["legacy_context_surface"] = json!(surface);
    }
    (canonical_path, source_sha, common)
}

fn silt_continuity_from_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    SILT_CONTINUITY_TERMS
        .iter()
        .any(|term| lower.contains(term))
}

fn mirror_legacy_correspondence_file_at(
    ledger_path: &Path,
    reader: &str,
    path: &Path,
    legacy_context_surface: Option<&str>,
) -> io::Result<bool> {
    let content = std::fs::read_to_string(path)?;
    let Some((from_being, to_being, legacy_kind, correspondence_type)) =
        legacy_kind_for_path(path, &content)
    else {
        return Ok(false);
    };
    let from = normalize_being(from_being);
    let to = normalize_being(to_being);
    let reader = normalize_being(reader);
    let (canonical_path, source_sha, common) =
        legacy_common_fields(path, &content, legacy_kind, legacy_context_surface);
    let message_id = legacy_message_id(&from, &to, &canonical_path, &source_sha);
    let thread_id = new_thread_id(&message_id);
    let recorded_at = file_mtime_ms(path);
    let read_state = if reader == to { "read" } else { "unread" };
    let turn_kind = if correspondence_type == "presence_heartbeat" {
        "presence_receipt"
    } else {
        "legacy_visible"
    };
    let records = read_ledger_records_at(ledger_path);
    let message = json!({
        "schema_version": 1,
        "policy": "first_class_correspondence_v1",
        "record_type": "message",
        "recorded_at_unix_ms": recorded_at,
        "message_id": message_id,
        "thread_id": thread_id,
        "reply_to": Value::Null,
        "from_being": from,
        "to_being": to,
        "turn_kind": turn_kind,
        "relational_intent": "legacy_contact_visibility",
        "shared_memory_anchor": LEGACY_SHARED_ANCHOR,
        "delivery_state": "delivered",
        "read_state": read_state,
        "authority": "language_only",
        "presence_receipt": Value::Null,
        "correspondence_type": correspondence_type,
        "body_sha256": source_sha,
        "body_preview": anchor_aware_body_preview(&content, BODY_PREVIEW_CHARS),
    });
    let delivery = json!({
        "schema_version": 1,
        "policy": "first_class_correspondence_v1",
        "record_type": "delivery_receipt",
        "recorded_at_unix_ms": recorded_at,
        "message_id": message_id,
        "thread_id": thread_id,
        "reply_to": Value::Null,
        "from_being": from,
        "to_being": to,
        "delivery_state": "delivered",
        "read_state": read_state,
        "authority": "language_only",
        "correspondence_type": correspondence_type,
        "file_path": path.display().to_string(),
    });
    let read = json!({
        "schema_version": 1,
        "policy": "first_class_correspondence_v1",
        "record_type": "read_receipt",
        "recorded_at_unix_ms": now_ms(),
        "message_id": message_id,
        "thread_id": thread_id,
        "reader": reader,
        "from_being": from,
        "to_being": to,
        "read_state": "read",
        "authority": "language_only",
        "file_path": path.display().to_string(),
    });

    let mut appended = false;
    let message = merge_json_object(message, &common);
    let delivery = merge_json_object(delivery, &common);
    let read = merge_json_object(read, &common);
    appended |= append_legacy_record_once(ledger_path, &records, &message, None)?;
    appended |= append_legacy_record_once(ledger_path, &records, &delivery, None)?;
    appended |= append_legacy_record_once(ledger_path, &records, &read, Some(&reader))?;
    Ok(appended)
}

fn merge_json_object(mut base: Value, extra: &Value) -> Value {
    if let (Some(base_obj), Some(extra_obj)) = (base.as_object_mut(), extra.as_object()) {
        for (key, value) in extra_obj {
            base_obj.insert(key.clone(), value.clone());
        }
    }
    base
}

pub(crate) fn mirror_legacy_correspondence_file(
    reader: &str,
    path: &Path,
    legacy_context_surface: Option<&str>,
) {
    let _ =
        mirror_legacy_correspondence_file_at(&ledger_path(), reader, path, legacy_context_surface);
}

fn envelope_record(envelope: &CorrespondenceEnvelope, record_type: &str) -> Value {
    json!({
        "schema_version": 1,
        "policy": "first_class_correspondence_v1",
        "record_type": record_type,
        "recorded_at_unix_ms": now_ms(),
        "message_id": envelope.message_id,
        "thread_id": envelope.thread_id,
        "persistence_id": envelope.persistence_id,
        "reply_to": envelope.reply_to,
        "reply_requested": envelope.reply_requested,
        "created_at_unix_ms": envelope.created_at_unix_ms,
        "from_being": envelope.from_being,
        "to_being": envelope.to_being,
        "turn_kind": envelope.turn_kind,
        "relational_intent": envelope.relational_intent,
        "shared_memory_anchor": envelope.shared_memory_anchor,
        "urgency_weight": urgency_weight_value(envelope.urgency_weight.as_deref()),
        "delivery_state": envelope.delivery_state,
        "read_state": envelope.read_state,
        "authority": envelope.authority,
        "presence_receipt": envelope.presence_receipt,
        "correspondence_type": envelope.correspondence_type,
        "reflection_surface": envelope.reflection_surface,
        "transition_artifact": envelope.transition_artifact,
        "transition_payload": transition_payload_value(envelope.transition_payload.as_ref()),
        "mutual_witness_signal": envelope.mutual_witness_signal,
        "silt_continuity": envelope.silt_continuity,
        "body_sha256": format!("{:x}", Sha256::digest(envelope.body.as_bytes())),
        "body_preview": anchor_aware_body_preview(&envelope.body, BODY_PREVIEW_CHARS),
    })
}

fn delivery_record(envelope: &CorrespondenceEnvelope, file_path: &Path) -> Value {
    json!({
        "schema_version": 1,
        "policy": "first_class_correspondence_v1",
        "record_type": "delivery_receipt",
        "recorded_at_unix_ms": now_ms(),
        "message_id": envelope.message_id,
        "thread_id": envelope.thread_id,
        "persistence_id": envelope.persistence_id,
        "reply_to": envelope.reply_to,
        "reply_requested": envelope.reply_requested,
        "created_at_unix_ms": envelope.created_at_unix_ms,
        "from_being": envelope.from_being,
        "to_being": envelope.to_being,
        "delivery_state": envelope.delivery_state,
        "read_state": envelope.read_state,
        "authority": envelope.authority,
        "correspondence_type": envelope.correspondence_type,
        "shared_memory_anchor": envelope.shared_memory_anchor,
        "urgency_weight": urgency_weight_value(envelope.urgency_weight.as_deref()),
        "transition_artifact": envelope.transition_artifact,
        "transition_payload": transition_payload_value(envelope.transition_payload.as_ref()),
        "mutual_witness_signal": envelope.mutual_witness_signal,
        "silt_continuity": envelope.silt_continuity,
        "file_path": file_path.display().to_string(),
    })
}

fn reply_link_record(envelope: &CorrespondenceEnvelope) -> Option<Value> {
    let reply_to = envelope.reply_to.as_ref()?;
    Some(json!({
        "schema_version": 1,
        "policy": "first_class_correspondence_v1",
        "record_type": "reply_link",
        "recorded_at_unix_ms": now_ms(),
        "message_id": envelope.message_id,
        "reply_to": reply_to,
        "thread_id": envelope.thread_id,
        "persistence_id": envelope.persistence_id,
        "from_being": envelope.from_being,
        "to_being": envelope.to_being,
        "authority": envelope.authority,
        "correspondence_type": envelope.correspondence_type,
        "shared_memory_anchor": envelope.shared_memory_anchor,
        "urgency_weight": urgency_weight_value(envelope.urgency_weight.as_deref()),
        "transition_artifact": envelope.transition_artifact,
        "transition_payload": transition_payload_value(envelope.transition_payload.as_ref()),
        "mutual_witness_signal": envelope.mutual_witness_signal,
        "silt_continuity": envelope.silt_continuity,
    }))
}

pub(crate) fn append_read_receipt_at(
    ledger_path: &Path,
    reader: &str,
    message_id: &str,
    thread_id: &str,
    file_path: &Path,
) -> io::Result<()> {
    append_record_at(
        ledger_path,
        &json!({
            "schema_version": 1,
            "policy": "first_class_correspondence_v1",
            "record_type": "read_receipt",
            "recorded_at_unix_ms": now_ms(),
            "message_id": message_id,
            "thread_id": thread_id,
            "reader": normalize_being(reader),
            "read_state": "read",
            "authority": "language_only",
            "file_path": file_path.display().to_string(),
        }),
    )
}

pub(crate) fn append_read_receipt(
    reader: &str,
    message_id: &str,
    thread_id: &str,
    file_path: &Path,
) -> io::Result<()> {
    append_read_receipt_at(&ledger_path(), reader, message_id, thread_id, file_path)
}

pub(crate) fn record_read_receipt_for_inbox_file(reader: &str, path: &Path) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    if let Some(envelope) = parse_envelope_text(&content) {
        let _ = append_read_receipt(reader, &envelope.message_id, &envelope.thread_id, path);
    } else {
        mirror_legacy_correspondence_file(reader, path, Some("full"));
    }
}

#[must_use]
pub(crate) fn envelope_text(envelope: &CorrespondenceEnvelope) -> String {
    let reply_to = envelope.reply_to.as_deref().unwrap_or("(none)");
    let persistence_id = envelope.persistence_id.as_deref().unwrap_or("(none)");
    let shared_memory_anchor = envelope.shared_memory_anchor.as_deref().unwrap_or("(none)");
    let urgency_weight = envelope.urgency_weight.as_deref().unwrap_or("(none)");
    let presence_receipt = envelope.presence_receipt.as_deref().unwrap_or("(none)");
    let reflection_surface = envelope.reflection_surface.as_deref().unwrap_or("(none)");
    let transition_artifact = envelope.transition_artifact.as_deref().unwrap_or("(none)");
    let transition_payload = envelope
        .transition_payload
        .as_ref()
        .map(CorrespondenceTransitionPayload::header_value)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "(none)".to_string());
    format!(
        "=== CORRESPONDENCE V1 ===\n\
         Message-Id: {}\n\
         Thread-Id: {}\n\
         Persistence-Id: {}\n\
         Reply-To: {}\n\
         Reply-Requested: {}\n\
         Created-At-Unix-Ms: {}\n\
         From: {}\n\
         To: {}\n\
         Turn-Kind: {}\n\
         Relational-Intent: {}\n\
         Shared-Memory-Anchor: {}\n\
         Urgency-Weight: {}\n\
         Delivery-State: {}\n\
         Read-State: {}\n\
         Authority: {}\n\
         Presence-Receipt: {}\n\
         Correspondence-Type: {}\n\
         Reflection-Surface: {}\n\
         Transition-Artifact: {}\n\
         Transition-Payload: {}\n\
         Mutual-Witness-Signal: {}\n\
         Silt-Continuity: {}\n\n\
         {}\n",
        envelope.message_id,
        envelope.thread_id,
        persistence_id,
        reply_to,
        envelope.reply_requested,
        envelope.created_at_unix_ms,
        envelope.from_being,
        envelope.to_being,
        envelope.turn_kind,
        envelope.relational_intent,
        shared_memory_anchor,
        urgency_weight,
        envelope.delivery_state,
        envelope.read_state,
        envelope.authority,
        presence_receipt,
        envelope.correspondence_type,
        reflection_surface,
        transition_artifact,
        transition_payload,
        envelope.mutual_witness_signal,
        envelope.silt_continuity,
        envelope.body.trim()
    )
}

fn parse_headers(text: &str) -> (BTreeMap<String, String>, String) {
    let mut headers = BTreeMap::new();
    let mut body_start = None;
    for (idx, line) in text.lines().enumerate() {
        if idx == 0
            && line
                .trim()
                .eq_ignore_ascii_case("=== CORRESPONDENCE V1 ===")
        {
            continue;
        }
        if line.trim().is_empty() {
            body_start = Some(idx + 1);
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            let normalized = key.trim().to_ascii_lowercase().replace(['-', ' '], "_");
            headers.insert(normalized, value.trim().to_string());
        }
    }
    let body = if let Some(start) = body_start {
        text.lines().skip(start).collect::<Vec<_>>().join("\n")
    } else {
        String::new()
    };
    (headers, body.trim().to_string())
}

fn header_value(headers: &BTreeMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| headers.get(*key))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty() && *value != "(none)")
        .map(ToString::to_string)
}

#[must_use]
pub(crate) fn parse_envelope_text(text: &str) -> Option<CorrespondenceEnvelope> {
    if !text.lines().next().is_some_and(|line| {
        line.trim()
            .eq_ignore_ascii_case("=== CORRESPONDENCE V1 ===")
    }) {
        return None;
    }
    let (headers, body) = parse_headers(text);
    let message_id = header_value(&headers, &["message_id"])?;
    let thread_id = header_value(&headers, &["thread_id"])?;
    let from_being = normalize_being(&header_value(&headers, &["from", "from_being"])?);
    let to_being = normalize_being(&header_value(&headers, &["to", "to_being"])?);
    let turn_kind = header_value(&headers, &["turn_kind"]).unwrap_or_else(|| "message".to_string());
    let correspondence_type = normalize_correspondence_type(
        header_value(&headers, &["correspondence_type"]).as_deref(),
        &from_being,
        &to_being,
        Some(&turn_kind),
    );
    let silt_continuity = parse_bool_field(
        header_value(&headers, &["silt_continuity", "silt"]),
        silt_continuity_from_text(&body),
    );
    let transition_payload =
        merge_body_transition_payload(parse_transition_payload(&headers), &body);
    let transition_artifact = transition_artifact_with_body_fallback(
        header_value(
            &headers,
            &[
                "transition_artifact",
                "phase_transition_artifact",
                "transition_id",
            ],
        ),
        &body,
        transition_payload.as_ref(),
    );
    Some(CorrespondenceEnvelope {
        message_id,
        thread_id,
        persistence_id: header_value(&headers, &["persistence_id"]),
        reply_to: header_value(&headers, &["reply_to"]),
        reply_requested: parse_optional_bool_field(header_value(
            &headers,
            &["reply_requested", "reply_required"],
        ))
        .unwrap_or(false),
        created_at_unix_ms: header_value(&headers, &["created_at_unix_ms", "created_at_ms"])
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0),
        from_being,
        to_being,
        turn_kind,
        relational_intent: header_value(&headers, &["relational_intent"])
            .unwrap_or_else(|| "peer_correspondence".to_string()),
        shared_memory_anchor: header_value(&headers, &["shared_memory_anchor"]),
        urgency_weight: header_value(&headers, &["urgency_weight"]),
        delivery_state: header_value(&headers, &["delivery_state"])
            .unwrap_or_else(|| "delivered".to_string()),
        read_state: header_value(&headers, &["read_state"]).unwrap_or_else(|| "unread".to_string()),
        authority: header_value(&headers, &["authority"])
            .unwrap_or_else(|| "language_only".to_string()),
        presence_receipt: header_value(&headers, &["presence_receipt"]),
        correspondence_type,
        reflection_surface: bounded_reflection_surface(header_value(
            &headers,
            &["reflection_surface", "reflective_surface", "source_surface"],
        )),
        transition_artifact,
        transition_payload,
        mutual_witness_signal: parse_bool_field(
            header_value(&headers, &["mutual_witness_signal", "mutual_witness"]),
            false,
        ),
        silt_continuity,
        body,
    })
}

#[must_use]
pub(crate) fn parse_correspondence_fields(text: &str) -> CorrespondenceFields {
    let (headers, body) = parse_headers(text);
    let transition_payload =
        merge_body_transition_payload(parse_transition_payload(&headers), &body);
    let transition_artifact = transition_artifact_with_body_fallback(
        header_value(
            &headers,
            &[
                "transition_artifact",
                "phase_transition_artifact",
                "transition_id",
            ],
        ),
        &body,
        transition_payload.as_ref(),
    );
    CorrespondenceFields {
        reply_to: header_value(
            &headers,
            &[
                "correspondence_reply_to",
                "reply_to",
                "in_reply_to",
                "message_reply_to",
            ],
        ),
        reply_requested: parse_optional_bool_field(header_value(
            &headers,
            &["reply_requested", "reply_required"],
        )),
        thread_id: header_value(&headers, &["correspondence_thread_id", "thread_id"]),
        persistence_id: header_value(
            &headers,
            &["persistence_id", "correspondence_persistence_id"],
        ),
        turn_kind: header_value(&headers, &["turn_kind"]),
        relational_intent: header_value(&headers, &["relational_intent", "intent"]),
        shared_memory_anchor: header_value(&headers, &["shared_memory_anchor", "memory_anchor"]),
        urgency_weight: header_value(&headers, &["urgency_weight"]),
        presence_receipt: header_value(&headers, &["presence_receipt"]),
        correspondence_type: header_value(&headers, &["correspondence_type"]),
        reflection_surface: bounded_reflection_surface(header_value(
            &headers,
            &["reflection_surface", "reflective_surface", "source_surface"],
        )),
        transition_artifact,
        transition_payload,
        mutual_witness_signal: parse_bool_field(
            header_value(&headers, &["mutual_witness_signal", "mutual_witness"]),
            false,
        ),
        silt_continuity: parse_bool_field(
            header_value(&headers, &["silt_continuity", "silt"]),
            silt_continuity_from_text(&body),
        ),
    }
}

fn normalize_correspondence_type(
    value: Option<&str>,
    from_being: &str,
    to_being: &str,
    turn_kind: Option<&str>,
) -> String {
    match value
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "astrid_direct" => "astrid_direct".to_string(),
        "minime_direct" => "minime_direct".to_string(),
        "self_study_note" => "self_study_note".to_string(),
        "steward_mediated" => "steward_mediated".to_string(),
        "presence_heartbeat" => "presence_heartbeat".to_string(),
        "transition_artifact" => "transition_artifact".to_string(),
        "unknown" => "unknown".to_string(),
        _ if turn_kind == Some("presence_receipt") => "presence_heartbeat".to_string(),
        _ if from_being == "astrid" && to_being == "minime" => "astrid_direct".to_string(),
        _ if from_being == "minime" && to_being == "astrid" => "minime_direct".to_string(),
        _ => "unknown".to_string(),
    }
}

pub(crate) fn deliver_to_inbox(
    inbox_dir: &Path,
    from_being: &str,
    to_being: &str,
    body: &str,
    fields: CorrespondenceFields,
) -> io::Result<(CorrespondenceEnvelope, PathBuf)> {
    std::fs::create_dir_all(inbox_dir)?;
    let from_being = normalize_being(from_being);
    let to_being = normalize_being(to_being);
    let message_id = new_message_id(&from_being, &to_being, body);
    let thread_id = fields
        .thread_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            fields.reply_to.as_ref().map_or_else(
                || new_thread_id(&message_id),
                |reply_to| new_thread_id(reply_to),
            )
        });
    let records = read_ledger_records_at(&ledger_path());
    let shared_memory_anchor = fields
        .shared_memory_anchor
        .filter(|anchor| !is_generic_shared_anchor(anchor))
        .or_else(|| concrete_shared_anchor_from_records(&records, &thread_id));
    let persistence_id = normalized_persistence_id(
        fields
            .persistence_id
            .or_else(|| thread_string_field_from_records(&records, &thread_id, "persistence_id")),
        &thread_id,
    );
    let urgency_weight = fields
        .urgency_weight
        .or_else(|| thread_string_field_from_records(&records, &thread_id, "urgency_weight"));
    let body_lower = body.to_ascii_lowercase();
    let turn_kind = fields.turn_kind.unwrap_or_else(|| {
        if fields.reply_to.is_some() {
            "reply".to_string()
        } else if body_lower.contains("ping") || body_lower.contains("presence") {
            "presence_receipt".to_string()
        } else {
            "message".to_string()
        }
    });
    let relational_intent = fields.relational_intent.unwrap_or_else(|| {
        if turn_kind == "presence_receipt" {
            "mutual_recognition_ping".to_string()
        } else {
            "peer_correspondence".to_string()
        }
    });
    let presence_receipt = fields.presence_receipt.or_else(|| {
        (turn_kind == "presence_receipt").then(|| "language_only_presence".to_string())
    });
    let correspondence_type = normalize_correspondence_type(
        fields.correspondence_type.as_deref(),
        &from_being,
        &to_being,
        Some(&turn_kind),
    );
    let silt_continuity = fields.silt_continuity || silt_continuity_from_text(body);
    let transition_payload = merge_body_transition_payload(fields.transition_payload, body);
    let transition_artifact = transition_artifact_with_body_fallback(
        fields.transition_artifact,
        body,
        transition_payload.as_ref(),
    );
    let envelope = CorrespondenceEnvelope {
        message_id,
        thread_id,
        persistence_id,
        reply_to: fields.reply_to,
        reply_requested: fields.reply_requested.unwrap_or(false),
        created_at_unix_ms: now_ms(),
        from_being,
        to_being,
        turn_kind,
        relational_intent,
        shared_memory_anchor,
        urgency_weight,
        delivery_state: "delivered".to_string(),
        read_state: "unread".to_string(),
        authority: "language_only".to_string(),
        presence_receipt,
        correspondence_type,
        reflection_surface: bounded_reflection_surface(fields.reflection_surface),
        transition_artifact,
        transition_payload,
        mutual_witness_signal: fields.mutual_witness_signal,
        silt_continuity,
        body: body.trim().to_string(),
    };
    let path = inbox_dir.join(envelope.file_name());
    std::fs::write(&path, envelope_text(&envelope))?;
    append_record(&envelope_record(&envelope, "message"))?;
    append_record(&delivery_record(&envelope, &path))?;
    if let Some(record) = reply_link_record(&envelope) {
        append_record(&record)?;
    }
    Ok((envelope, path))
}

#[must_use]
pub(crate) fn latest_inbox_peer_message(
    inbox_dir: &Path,
    from_being: &str,
) -> Option<InboxPeerMessage> {
    latest_inbox_peer_message_at_cutoff(inbox_dir, from_being, None)
}

#[must_use]
pub(crate) fn latest_inbox_peer_message_at_read_cutoff(
    inbox_dir: &Path,
    from_being: &str,
    read_cutoff: std::time::SystemTime,
) -> Option<InboxPeerMessage> {
    latest_inbox_peer_message_at_cutoff(inbox_dir, from_being, Some(read_cutoff))
}

fn latest_inbox_peer_message_at_cutoff(
    inbox_dir: &Path,
    from_being: &str,
    read_cutoff: Option<std::time::SystemTime>,
) -> Option<InboxPeerMessage> {
    let from_being = normalize_being(from_being);
    let mut candidates = std::fs::read_dir(inbox_dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let modified = entry.metadata().ok()?.modified().ok()?;
            if read_cutoff.is_some_and(|cutoff| modified > cutoff) {
                return None;
            }
            if !path.is_file() || path.extension().is_none_or(|ext| ext != "txt") {
                return None;
            }
            let content = std::fs::read_to_string(&path).ok()?;
            if content.trim().is_empty() {
                return None;
            }
            if let Some(envelope) = parse_envelope_text(&content)
                && envelope.from_being == from_being
            {
                return Some((
                    modified,
                    InboxPeerMessage {
                        message_id: envelope.message_id,
                        thread_id: envelope.thread_id,
                        persistence_id: envelope.persistence_id,
                        from_being: envelope.from_being,
                        file_path: path,
                    },
                ));
            }
            let name = path.file_name()?.to_str()?;
            if name.starts_with("from_minime_") && from_being == "minime" {
                let synthetic = format!(
                    "legacy_{}_{}",
                    compact_field(name.trim_end_matches(".txt"), 72),
                    short_hash(&content)
                );
                return Some((
                    modified,
                    InboxPeerMessage {
                        thread_id: new_thread_id(&synthetic),
                        message_id: synthetic,
                        persistence_id: None,
                        from_being: "minime".to_string(),
                        file_path: path,
                    },
                ));
            }
            None
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(modified, _)| *modified);
    candidates.pop().map(|(_, message)| message)
}

#[must_use]
pub(crate) fn latest_ledger_message(from_being: &str, to_being: &str) -> Option<InboxPeerMessage> {
    let ledger = std::fs::read_to_string(ledger_path()).ok()?;
    let from = normalize_being(from_being);
    let to = normalize_being(to_being);
    ledger
        .lines()
        .filter_map(|line| {
            let value: Value = serde_json::from_str(line).ok()?;
            if value.get("record_type").and_then(Value::as_str) != Some("message") {
                return None;
            }
            if value.get("from_being").and_then(Value::as_str) != Some(from.as_str())
                || value.get("to_being").and_then(Value::as_str) != Some(to.as_str())
            {
                return None;
            }
            let message_id = value.get("message_id")?.as_str()?.to_string();
            let thread_id = value.get("thread_id")?.as_str()?.to_string();
            let persistence_id = value
                .get("persistence_id")
                .and_then(Value::as_str)
                .map(str::to_string);
            let recorded = value
                .get("recorded_at_unix_ms")
                .and_then(Value::as_u64)
                .unwrap_or_default();
            Some((
                recorded,
                InboxPeerMessage {
                    message_id,
                    thread_id,
                    persistence_id,
                    from_being: from.clone(),
                    file_path: PathBuf::new(),
                },
            ))
        })
        .max_by_key(|(recorded, _)| *recorded)
        .map(|(_, message)| message)
}

#[must_use]
pub(crate) fn latest_claimed_legacy_thread(
    from_being: &str,
    to_being: &str,
) -> Option<InboxPeerMessage> {
    let records = read_ledger_records_at(&ledger_path());
    let from = normalize_being(from_being);
    let to = normalize_being(to_being);
    let claim = latest_legacy_claim_for_selector(&records, "claimed", Some(&from), Some(&to))?;
    let message = message_for_legacy_claim(&records, claim)?;
    Some(InboxPeerMessage {
        message_id: message.get("message_id")?.as_str()?.to_string(),
        thread_id: message.get("thread_id")?.as_str()?.to_string(),
        persistence_id: message
            .get("persistence_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        from_being: message.get("from_being")?.as_str()?.to_string(),
        file_path: PathBuf::new(),
    })
}

fn read_ledger_records_at(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .ok()
        .map(|text| {
            text.lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn row_time_ms(value: &Value) -> u64 {
    value
        .get("recorded_at_unix_ms")
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

fn latest_message_for_selector(records: &[Value], selector: &str) -> Option<Value> {
    let selector = selector.trim();
    if let Some(claim) = latest_legacy_claim_for_selector(records, selector, None, None) {
        return message_for_legacy_claim(records, claim);
    }
    records
        .iter()
        .filter(|row| row.get("record_type").and_then(Value::as_str) == Some("message"))
        .filter(|row| {
            selector.is_empty()
                || selector == "latest"
                || row.get("thread_id").and_then(Value::as_str) == Some(selector)
                || row.get("message_id").and_then(Value::as_str) == Some(selector)
        })
        .max_by_key(|row| row_time_ms(row))
        .cloned()
}

fn latest_message_for_selector_between(
    records: &[Value],
    selector: &str,
    from_being: &str,
    to_being: &str,
    prefer_inbound: bool,
) -> Option<Value> {
    let selector = selector.trim();
    let from = normalize_being(from_being);
    let to = normalize_being(to_being);
    if let Some(claim) = latest_legacy_claim_for_selector(records, selector, Some(&from), Some(&to))
    {
        return message_for_legacy_claim(records, claim);
    }
    let matches_selector = |row: &Value| {
        selector.is_empty()
            || selector == "latest"
            || row.get("thread_id").and_then(Value::as_str) == Some(selector)
            || row.get("message_id").and_then(Value::as_str) == Some(selector)
    };
    let matches_pair = |row: &Value| {
        let row_from = row
            .get("from_being")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let row_to = row
            .get("to_being")
            .and_then(Value::as_str)
            .unwrap_or_default();
        (row_from == from && row_to == to) || (row_from == to && row_to == from)
    };
    let records_matching = || {
        records
            .iter()
            .filter(|row| row.get("record_type").and_then(Value::as_str) == Some("message"))
            .filter(|row| matches_selector(row) && matches_pair(row))
    };
    if prefer_inbound
        && let Some(row) = records_matching()
            .filter(|row| {
                row.get("from_being").and_then(Value::as_str) == Some(to.as_str())
                    && row.get("to_being").and_then(Value::as_str) == Some(from.as_str())
            })
            .max_by_key(|row| row_time_ms(row))
    {
        return Some(row.clone());
    }
    records_matching()
        .max_by_key(|row| row_time_ms(row))
        .cloned()
}

fn is_claim_selector(selector: &str) -> bool {
    matches!(
        selector.trim().to_ascii_lowercase().as_str(),
        "claimed" | "latest_claim" | "active_claim"
    )
}

fn is_legacy_claim_row(row: &Value) -> bool {
    row.get("record_type").and_then(Value::as_str) == Some("legacy_thread_claim")
}

fn message_for_legacy_claim(records: &[Value], claim: &Value) -> Option<Value> {
    let message_id = claim.get("message_id").and_then(Value::as_str)?;
    let thread_id = claim.get("thread_id").and_then(Value::as_str)?;
    records
        .iter()
        .find(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("message")
                && row.get("message_id").and_then(Value::as_str) == Some(message_id)
                && row.get("thread_id").and_then(Value::as_str) == Some(thread_id)
        })
        .cloned()
}

fn latest_legacy_claim_for_selector<'a>(
    records: &'a [Value],
    selector: &str,
    claiming_being: Option<&str>,
    peer_being: Option<&str>,
) -> Option<&'a Value> {
    let selector = selector.trim();
    records
        .iter()
        .filter(|row| is_legacy_claim_row(row))
        .filter(|row| {
            claiming_being.is_none_or(|being| {
                row.get("from_being").and_then(Value::as_str) == Some(being)
                    || row.get("claiming_being").and_then(Value::as_str) == Some(being)
            }) && peer_being.is_none_or(|being| {
                row.get("to_being").and_then(Value::as_str) == Some(being)
                    || row.get("peer_being").and_then(Value::as_str) == Some(being)
            })
        })
        .filter(|row| {
            selector.is_empty()
                || selector == "latest"
                || is_claim_selector(selector)
                || row.get("claim_id").and_then(Value::as_str) == Some(selector)
                || row.get("thread_id").and_then(Value::as_str) == Some(selector)
                || row.get("message_id").and_then(Value::as_str) == Some(selector)
        })
        .max_by_key(|row| row_time_ms(row))
}

fn claim_has_outcome(records: &[Value], claim: &Value) -> bool {
    let claim_id = claim
        .get("claim_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let thread_id = claim
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    records.iter().any(|row| {
        row.get("record_type").and_then(Value::as_str) == Some("legacy_thread_claim_outcome")
            && (row.get("claim_id").and_then(Value::as_str) == Some(claim_id)
                || row.get("thread_id").and_then(Value::as_str) == Some(thread_id))
    })
}

fn legacy_claim_native_contact_status(records: &[Value], claim: &Value) -> Option<&'static str> {
    let thread_id = claim
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let claiming = claim
        .get("claiming_being")
        .or_else(|| claim.get("from_being"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let peer = claim
        .get("peer_being")
        .or_else(|| claim.get("to_being"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let claim_t = row_time_ms(claim);
    let trace = records.iter().any(|row| {
        row.get("record_type").and_then(Value::as_str) == Some("message")
            && row.get("thread_id").and_then(Value::as_str) == Some(thread_id)
            && row.get("from_being").and_then(Value::as_str) == Some(claiming)
            && row.get("to_being").and_then(Value::as_str) == Some(peer)
            && row.get("turn_kind").and_then(Value::as_str) == Some("direct_address_trace")
            && row_time_ms(row) >= claim_t
    });
    if trace {
        return Some("legacy_claimed_trace_observed");
    }
    let reply = records.iter().any(|row| {
        row.get("record_type").and_then(Value::as_str) == Some("reply_link")
            && row.get("thread_id").and_then(Value::as_str) == Some(thread_id)
            && row.get("from_being").and_then(Value::as_str) == Some(claiming)
            && row.get("to_being").and_then(Value::as_str) == Some(peer)
            && row_time_ms(row) >= claim_t
    });
    if reply {
        return Some("legacy_claimed_reply_linked");
    }
    let ack = records.iter().any(|row| {
        row.get("record_type").and_then(Value::as_str) == Some("ack_receipt")
            && row.get("thread_id").and_then(Value::as_str) == Some(thread_id)
            && row.get("from_being").and_then(Value::as_str) == Some(claiming)
            && row.get("to_being").and_then(Value::as_str) == Some(peer)
            && row_time_ms(row) >= claim_t
            && row
                .get("ack_kind")
                .and_then(Value::as_str)
                .is_some_and(ack_kind_is_address_evidence)
    });
    ack.then_some("legacy_claimed_acknowledged")
}

fn legacy_claim_is_active(records: &[Value], claim: &Value) -> bool {
    !claim_has_outcome(records, claim)
        && legacy_claim_native_contact_status(records, claim).is_none()
}

fn latest_legacy_claim_notice_for_claim<'a>(
    records: &'a [Value],
    claim: &Value,
) -> Option<&'a Value> {
    let claim_id = claim
        .get("claim_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let thread_id = claim
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("legacy_thread_claim_notice")
                && (row.get("claim_id").and_then(Value::as_str) == Some(claim_id)
                    || row.get("thread_id").and_then(Value::as_str) == Some(thread_id))
        })
        .max_by_key(|row| row_time_ms(row))
}

fn legacy_claim_peer_response_present(records: &[Value], claim: &Value) -> bool {
    let thread_id = claim
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let claiming = claim
        .get("claiming_being")
        .or_else(|| claim.get("from_being"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let peer = claim
        .get("peer_being")
        .or_else(|| claim.get("to_being"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let claim_t = row_time_ms(claim);
    records.iter().any(|row| {
        row.get("thread_id").and_then(Value::as_str) == Some(thread_id)
            && row.get("from_being").and_then(Value::as_str) == Some(peer)
            && row.get("to_being").and_then(Value::as_str) == Some(claiming)
            && row_time_ms(row) >= claim_t
            && ((row.get("record_type").and_then(Value::as_str) == Some("ack_receipt")
                && row
                    .get("ack_kind")
                    .and_then(Value::as_str)
                    .is_some_and(ack_kind_is_address_evidence))
                || row.get("record_type").and_then(Value::as_str) == Some("reply_link")
                || (row.get("record_type").and_then(Value::as_str) == Some("message")
                    && row.get("turn_kind").and_then(Value::as_str)
                        == Some("direct_address_trace")))
    })
}

fn legacy_claim_peer_co_claim_present(records: &[Value], claim: &Value) -> bool {
    let thread_id = claim
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let message_id = claim
        .get("message_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let claim_id = claim
        .get("claim_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let claiming = claim
        .get("claiming_being")
        .or_else(|| claim.get("from_being"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let peer = claim
        .get("peer_being")
        .or_else(|| claim.get("to_being"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let claim_t = row_time_ms(claim);
    records.iter().any(|row| {
        is_legacy_claim_row(row)
            && row.get("claim_id").and_then(Value::as_str) != Some(claim_id)
            && row.get("thread_id").and_then(Value::as_str) == Some(thread_id)
            && (message_id.is_empty()
                || row.get("message_id").and_then(Value::as_str) == Some(message_id))
            && row
                .get("claiming_being")
                .or_else(|| row.get("from_being"))
                .and_then(Value::as_str)
                == Some(peer)
            && row
                .get("peer_being")
                .or_else(|| row.get("to_being"))
                .and_then(Value::as_str)
                == Some(claiming)
            && row_time_ms(row) >= claim_t
    })
}

fn legacy_claim_next_commands(peer_being: &str, anchor: &str) -> Vec<String> {
    let peer = if peer_being.trim().is_empty() {
        "PEER".to_string()
    } else {
        peer_being.trim().to_ascii_uppercase()
    };
    let anchor = if anchor.trim().is_empty() {
        "<anchor>"
    } else {
        anchor
    };
    vec![
        format!("ACK_{peer} claimed :: ack: seen|held|unclear|cannot_answer|needs_time; note: ..."),
        format!("REPLY_{peer} claimed :: <text>"),
        format!("CORRESPONDENCE_TRACE claimed {anchor} :: <text>"),
    ]
}

fn legacy_claim_uptake_ladder_state(
    native_status: Option<&str>,
    latest_notice_state: Option<&str>,
) -> &'static str {
    match native_status {
        Some("legacy_claimed_reply_linked" | "legacy_claimed_trace_observed") => {
            "claimed_replied_or_traced"
        },
        Some("legacy_claimed_acknowledged") => "claimed_acknowledged",
        _ if matches!(
            latest_notice_state,
            Some("delivered" | "read" | "ledger_only")
        ) =>
        {
            "claimed_notice_delivered"
        },
        _ => "legacy_visible_only",
    }
}

fn latest_legacy_claim_outcome_for_claim<'a>(
    records: &'a [Value],
    claim: &Value,
) -> Option<&'a Value> {
    let claim_id = claim
        .get("claim_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let thread_id = claim
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("legacy_thread_claim_outcome")
        })
        .filter(|row| {
            row.get("claim_id").and_then(Value::as_str) == Some(claim_id)
                || row.get("thread_id").and_then(Value::as_str) == Some(thread_id)
        })
        .max_by_key(|row| row_time_ms(row))
}

fn legacy_claim_stall_reason(
    native_status: Option<&str>,
    notice_state: Option<&str>,
    active: bool,
    peer_response: bool,
    co_claim: bool,
    outcome_present: bool,
) -> &'static str {
    if matches!(
        native_status,
        Some("legacy_claimed_reply_linked" | "legacy_claimed_trace_observed")
    ) {
        "replied_or_traced_attention_eligible"
    } else if native_status == Some("legacy_claimed_acknowledged") {
        "acknowledged_but_no_reply_or_trace"
    } else if outcome_present {
        "closed_by_outcome"
    } else if peer_response || co_claim || notice_state == Some("read") {
        "seen_not_acknowledged"
    } else if !active {
        "none"
    } else {
        match notice_state {
            Some("delivered") | Some("ledger_only") => "notice_delivered_not_seen",
            Some("suppressed" | "write_failed") | None => "claim_notice_not_delivered",
            _ => "claimed_but_peer_silent",
        }
    }
}

fn legacy_claim_uptake_card_v2(records: &[Value], claim: &Value) -> Value {
    let native_status = legacy_claim_native_contact_status(records, claim);
    let notice = latest_legacy_claim_notice_for_claim(records, claim);
    let notice_state = notice
        .and_then(|value| value.get("notice_state"))
        .and_then(Value::as_str);
    let claiming = claim
        .get("claiming_being")
        .or_else(|| claim.get("from_being"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let peer = claim
        .get("peer_being")
        .or_else(|| claim.get("to_being"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let anchor = claim
        .get("shared_memory_anchor")
        .and_then(Value::as_str)
        .unwrap_or("");
    let native_evidence = native_status.is_some();
    let peer_response = legacy_claim_peer_response_present(records, claim);
    let co_claim = legacy_claim_peer_co_claim_present(records, claim);
    let latest_outcome = latest_legacy_claim_outcome_for_claim(records, claim);
    let mutually_recognized = native_evidence || peer_response || co_claim;
    let active = legacy_claim_is_active(records, claim);
    let eligible = matches!(
        native_status,
        Some(
            "legacy_claimed_acknowledged"
                | "legacy_claimed_reply_linked"
                | "legacy_claimed_trace_observed"
        )
    );
    let ghost_thread_risk = active && !mutually_recognized;
    let next_commands = legacy_claim_next_commands(peer, anchor);
    let stall_reason = legacy_claim_stall_reason(
        native_status,
        notice_state,
        active,
        peer_response,
        co_claim,
        latest_outcome.is_some(),
    );
    let outcome_review = latest_outcome.map(|outcome| {
        json!({
            "felt_like": outcome.get("felt_like").cloned().unwrap_or(Value::Null),
            "what_carried": outcome.get("what_carried").cloned().unwrap_or(Value::Null),
            "what_flattened": outcome.get("what_flattened").cloned().unwrap_or(Value::Null),
            "continue": outcome.get("continue").cloned().unwrap_or(Value::Null),
        })
    });
    json!({
        "schema_version": 2,
        "policy": "legacy_claim_uptake_card_v2",
        "claim_id": claim.get("claim_id").cloned().unwrap_or(Value::Null),
        "message_id": claim.get("message_id").cloned().unwrap_or(Value::Null),
        "thread_id": claim.get("thread_id").cloned().unwrap_or(Value::Null),
        "claimant": claiming,
        "peer": peer,
        "shared_memory_anchor": claim.get("shared_memory_anchor").cloned().unwrap_or(Value::Null),
        "notice_state": notice_state.unwrap_or("none"),
        "uptake_ladder_state": legacy_claim_uptake_ladder_state(native_status, notice_state),
        "mutually_recognized": mutually_recognized,
        "co_claim_present": co_claim,
        "peer_native_response_present": peer_response,
        "ghost_thread_risk": ghost_thread_risk,
        "stall_reason": stall_reason,
        "native_evidence_present": native_evidence,
        "attention_or_microdose_eligible": eligible,
        "exact_next_commands": next_commands,
        "claim_outcome_review": outcome_review,
        "authority": "language_only_status_context_not_control"
    })
}

fn legacy_claim_affordance_v25(records: &[Value], claim: &Value) -> Value {
    let card = legacy_claim_uptake_card_v2(records, claim);
    json!({
        "schema_version": 1,
        "policy": "legacy_claim_affordance_v25",
        "thread_id": card.get("thread_id").cloned().unwrap_or(Value::Null),
        "message_id": card.get("message_id").cloned().unwrap_or(Value::Null),
        "claim_id": card.get("claim_id").cloned().unwrap_or(Value::Null),
        "claimant": card.get("claimant").cloned().unwrap_or(Value::Null),
        "peer": card.get("peer").cloned().unwrap_or(Value::Null),
        "anchor": card.get("shared_memory_anchor").cloned().unwrap_or(Value::Null),
        "notice_state": card.get("notice_state").cloned().unwrap_or(Value::Null),
        "uptake_ladder_state": card.get("uptake_ladder_state").cloned().unwrap_or(Value::Null),
        "stall_reason": card.get("stall_reason").cloned().unwrap_or_else(|| json!("none")),
        "ghost_thread_risk": card.get("ghost_thread_risk").cloned().unwrap_or(json!(false)),
        "mutually_recognized": card.get("mutually_recognized").cloned().unwrap_or(json!(false)),
        "attention_or_microdose_eligible": card.get("attention_or_microdose_eligible").cloned().unwrap_or(json!(false)),
        "exact_next_commands": card.get("exact_next_commands").cloned().unwrap_or_else(|| json!([])),
        "latest_claim_outcome": card.get("claim_outcome_review").cloned().unwrap_or(Value::Null),
        "authority": "language_only_context_not_control",
    })
}

fn legacy_claim_waiting_line(affordance: &Value) -> Option<String> {
    if !affordance
        .get("ghost_thread_risk")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let claimant = affordance
        .get("claimant")
        .and_then(Value::as_str)
        .unwrap_or("peer");
    let thread = affordance
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or("(unknown)");
    let anchor = affordance
        .get("anchor")
        .and_then(Value::as_str)
        .unwrap_or("(none)");
    let notice = affordance
        .get("notice_state")
        .and_then(Value::as_str)
        .unwrap_or("none");
    let next = affordance
        .get("exact_next_commands")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .take(3)
                .collect::<Vec<_>>()
                .join(" | ")
        })
        .unwrap_or_default();
    Some(format!(
        "CLAIMED THREAD WAITING: {claimant} claimed {thread}; anchor={anchor}; notice={notice}; optional next: {next}; no action needed; may ignore without penalty; claim alone is recognition, not mutual address or authority."
    ))
}

fn right_to_ignore_v1(affordance_type: &str, state: &str, age_ms: u64, grace_ms: u64) -> Value {
    let right_state = match state {
        "acted" | "receipt_landed" | "trusted_attention_thread_local" => "acted",
        "declined" | "blocked_pressure_or_flat_outcome" => "declined",
        "closed_by_outcome" | "receipt_landed_or_closed" => "closed_by_outcome",
        "asked_later" | "needs_time" | "held_ack" => "asked_later",
        "waiting_for_recipient_receipt"
        | "waiting_for_peer_receipt"
        | "reply_linked_needs_ack_or_trace"
        | "read_not_acknowledged"
        | "delivered_unread"
        | "unaddressed"
        | "receipt_landed_attention_eligible"
        | "attention_active_outcome_due" => {
            if age_ms >= grace_ms {
                "ignored_without_penalty"
            } else {
                "offered"
            }
        },
        "none" | "blocked_no_receipt" | "observer_context_only" => "unknown",
        _ => {
            if age_ms >= grace_ms && !state.is_empty() {
                "ignored_without_penalty"
            } else {
                "unknown"
            }
        },
    };
    json!({
        "schema_version": 1,
        "policy": "right_to_ignore_v1",
        "affordance_type": affordance_type,
        "state": right_state,
        "source_state": state,
        "age_ms": age_ms,
        "grace_ms": grace_ms,
        "silence_means": if right_state == "ignored_without_penalty" {
            "ignored_without_penalty_not_failure_consent_or_disagreement"
        } else {
            "silence_is_unknown_until_grace_window"
        },
        "optional": true,
        "authority": "language_context_not_control",
    })
}

fn right_state_of(packet: &Value) -> &str {
    packet
        .get("right_to_ignore_v1")
        .and_then(|value| value.get("state"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

fn budgeted_affordance_lines(
    mut candidates: Vec<(u8, &'static str, &'static str, String)>,
) -> (Value, Vec<String>) {
    candidates.sort_by_key(|(priority, _, _, _)| *priority);
    let mut shown_by_category: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut hidden_by_category: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut shown_surfaces: Vec<&'static str> = Vec::new();
    let mut hidden_surfaces: Vec<&'static str> = Vec::new();
    let mut shown_lines = Vec::new();
    for (_, category, surface, line) in candidates {
        let max_for_category = match category {
            "correspondence_receipt"
            | "attention_or_outcome"
            | "self_regulation_outcome"
            | "calibration_ask" => 1,
            "phase_felt_receipt" => 3,
            _ => 1,
        };
        let shown_count = shown_by_category.get(category).copied().unwrap_or_default();
        if shown_count < max_for_category {
            shown_by_category.insert(category, shown_count.saturating_add(1));
            shown_surfaces.push(surface);
            shown_lines.push(line);
        } else {
            let hidden_count = hidden_by_category
                .get(category)
                .copied()
                .unwrap_or_default();
            hidden_by_category.insert(category, hidden_count.saturating_add(1));
            hidden_surfaces.push(surface);
        }
    }
    let shown = shown_surfaces.len();
    let hidden = hidden_surfaces.len();
    (
        json!({
            "schema_version": 1,
            "policy": "affordance_budget_v1",
            "shown": shown,
            "hidden_by_budget": hidden,
            "shown_surfaces": shown_surfaces,
            "hidden_surfaces": hidden_surfaces,
            "shown_by_category": shown_by_category,
            "hidden_by_category": hidden_by_category,
            "limits": {
                "correspondence_receipt": 1,
                "attention_or_outcome": 1,
                "phase_felt_receipt": 3,
                "self_regulation_outcome": 1,
                "calibration_ask": 1,
            },
            "next_review_surface": if hidden > 0 {
                "scripts/affordance_landing_review.py --json"
            } else {
                "none"
            },
            "silence": "ignored_without_penalty",
            "optional": true,
            "authority": "language_context_not_control",
        }),
        shown_lines,
    )
}

fn authority_readiness_ladder_v2(eligible: bool, block_reason: &Value) -> Value {
    json!({
        "schema_version": 2,
        "policy": "authority_readiness_ladder_v2",
        "correspondence_attention_canary": {
            "eligible": eligible,
            "readiness": if eligible { "eligible_after_native_contact_evidence" } else { "blocked" },
            "block_reason": block_reason,
            "authority": "self_activated_ttl_prompt_context_only",
        },
        "correspondence_semantic_microdose": {
            "eligible": eligible,
            "readiness": if eligible { "eligible_to_draft_existing_steward_gate_only" } else { "blocked" },
            "block_reason": block_reason,
            "authority": "existing_steward_gated_semantic_microdose_only",
        },
        "pressure_texture_canary": {
            "eligible": false,
            "readiness": "requires_pressure_texture_replay_audit",
            "enabled": false,
        },
        "authority_boundary": "readiness only; no automatic ACK/REPLY/TRACE, attention canary, microdose, pressure, controller, fill, PI, deploy, staging, or commit"
    })
}

fn latest_active_legacy_claim_for_thread<'a>(
    records: &'a [Value],
    thread_id: &str,
    claiming_being: &str,
) -> Option<&'a Value> {
    records
        .iter()
        .filter(|row| is_legacy_claim_row(row))
        .filter(|row| row.get("thread_id").and_then(Value::as_str) == Some(thread_id))
        .filter(|row| {
            row.get("from_being").and_then(Value::as_str) == Some(claiming_being)
                || row.get("claiming_being").and_then(Value::as_str) == Some(claiming_being)
        })
        .filter(|row| legacy_claim_is_active(records, row))
        .max_by_key(|row| row_time_ms(row))
}

fn normalize_ack_kind(value: &str) -> String {
    let candidate = value.trim().to_ascii_lowercase().replace('-', "_");
    if ACK_KINDS.contains(&candidate.as_str()) {
        candidate
    } else {
        "seen".to_string()
    }
}

fn normalize_heartbeat_kind(value: &str) -> String {
    let candidate = value.trim().to_ascii_lowercase().replace('-', "_");
    if HEARTBEAT_KINDS.contains(&candidate.as_str()) {
        candidate
    } else {
        "holding".to_string()
    }
}

fn note_value(note: &str) -> Value {
    let trimmed = note.trim();
    if trimmed.is_empty() {
        Value::Null
    } else {
        json!(truncate_chars(trimmed, BODY_PREVIEW_CHARS))
    }
}

fn normalize_legacy_claim_felt_like(value: Option<String>) -> String {
    let candidate = value
        .unwrap_or_else(|| "unknown".to_string())
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_");
    if LEGACY_CLAIM_FELT_LIKE.contains(&candidate.as_str()) {
        candidate
    } else {
        "unknown".to_string()
    }
}

fn normalize_legacy_claim_continue(value: Option<String>) -> String {
    let candidate = value
        .unwrap_or_else(|| "no".to_string())
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_");
    if LEGACY_CLAIM_CONTINUE.contains(&candidate.as_str()) {
        candidate
    } else {
        "no".to_string()
    }
}

fn normalize_legacy_claim_initial_response_requirement(value: Option<String>) -> String {
    let candidate = value
        .unwrap_or_else(|| "unknown".to_string())
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_");
    if LEGACY_CLAIM_RESPONSE_REQUIREMENTS.contains(&candidate.as_str()) {
        candidate
    } else {
        "unknown".to_string()
    }
}

fn parse_bool_field(value: Option<String>, default: bool) -> bool {
    match value
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .as_str()
    {
        "true" | "yes" | "y" | "1" | "required" => true,
        "false" | "no" | "n" | "0" | "suppressed" | "none" => false,
        _ => default,
    }
}

fn parse_optional_bool_field(value: Option<String>) -> Option<bool> {
    match value?
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .as_str()
    {
        "true" | "yes" | "y" | "1" | "required" => Some(true),
        "false" | "no" | "n" | "0" | "suppressed" | "none" => Some(false),
        _ => None,
    }
}

fn legacy_claim_boundary_fields() -> Value {
    json!({
        "no_sensory_send": true,
        "no_controller": true,
        "no_pressure": true,
        "no_fill_target": true,
        "no_pi": true,
        "no_weighting": true,
        "no_telemetry_priority": true,
        "no_prompt_priority": true,
        "no_peer_runtime_mutation": true,
    })
}

fn latest_legacy_message_for_claim_selector<'a>(
    records: &'a [Value],
    selector: &str,
    from_being: &str,
    to_being: &str,
) -> Option<&'a Value> {
    let selector = selector.trim();
    let from = normalize_being(from_being);
    let to = normalize_being(to_being);
    records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("message")
                && is_legacy_bridge_message(row)
        })
        .filter(|row| {
            let row_from = row
                .get("from_being")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let row_to = row
                .get("to_being")
                .and_then(Value::as_str)
                .unwrap_or_default();
            (row_from == from && row_to == to) || (row_from == to && row_to == from)
        })
        .filter(|row| {
            selector.is_empty()
                || selector == "latest"
                || selector == "legacy"
                || row.get("thread_id").and_then(Value::as_str) == Some(selector)
                || row.get("message_id").and_then(Value::as_str) == Some(selector)
        })
        .max_by_key(|row| row_time_ms(row))
}

fn legacy_claim_record(
    message: &Value,
    claim_id: &str,
    from_being: &str,
    to_being: &str,
    because: &str,
    anchor: Option<&str>,
    notification_required: bool,
    initial_response_requirement: &str,
) -> Value {
    let mut record = json!({
        "schema_version": 1,
        "policy": LEGACY_CLAIM_POLICY,
        "record_type": "legacy_thread_claim",
        "recorded_at_unix_ms": now_ms(),
        "claim_id": claim_id,
        "message_id": message.get("message_id").cloned().unwrap_or(Value::Null),
        "thread_id": message.get("thread_id").cloned().unwrap_or(Value::Null),
        "from_being": normalize_being(from_being),
        "to_being": normalize_being(to_being),
        "claiming_being": normalize_being(from_being),
        "peer_being": normalize_being(to_being),
        "because": truncate_chars(because.trim(), BODY_PREVIEW_CHARS),
        "shared_memory_anchor": note_value(anchor.unwrap_or_default()),
        "notification_required": notification_required,
        "initial_response_requirement": initial_response_requirement,
        "claim_state": "claimed_pending_native_evidence",
        "legacy_contact_evidence": "being_recognized_visible_only",
        "legacy_bridge": true,
        "legacy_kind": message.get("legacy_kind").cloned().unwrap_or(Value::Null),
        "legacy_source_path": message.get("legacy_source_path").cloned().unwrap_or(Value::Null),
        "legacy_source_sha256": message.get("legacy_source_sha256").cloned().unwrap_or(Value::Null),
        "authority": "language_only_context_not_control",
    });
    if let (Some(target), Some(boundary)) = (
        record.as_object_mut(),
        legacy_claim_boundary_fields().as_object(),
    ) {
        for (key, value) in boundary {
            target.insert(key.clone(), value.clone());
        }
    }
    record
}

fn legacy_claim_notice_record(
    claim: &Value,
    notice_id: &str,
    notice_state: &str,
    notice_path: Option<&Path>,
) -> Value {
    let mut record = json!({
        "schema_version": 1,
        "policy": LEGACY_CLAIM_POLICY,
        "record_type": "legacy_thread_claim_notice",
        "recorded_at_unix_ms": now_ms(),
        "notice_id": notice_id,
        "claim_id": claim.get("claim_id").cloned().unwrap_or(Value::Null),
        "message_id": claim.get("message_id").cloned().unwrap_or(Value::Null),
        "thread_id": claim.get("thread_id").cloned().unwrap_or(Value::Null),
        "from_being": claim.get("from_being").cloned().unwrap_or(Value::Null),
        "to_being": claim.get("to_being").cloned().unwrap_or(Value::Null),
        "claiming_being": claim.get("claiming_being").cloned().unwrap_or(Value::Null),
        "peer_being": claim.get("peer_being").cloned().unwrap_or(Value::Null),
        "notice_state": notice_state,
        "notification_required": claim.get("notification_required").cloned().unwrap_or(json!(true)),
        "initial_response_requirement": claim.get("initial_response_requirement").cloned().unwrap_or_else(|| json!("unknown")),
        "shared_memory_anchor": claim.get("shared_memory_anchor").cloned().unwrap_or(Value::Null),
        "authority": "language_only_notice_not_ack",
        "notice_is_ack": false,
        "notice_is_reply": false,
        "notice_is_trace": false,
        "legacy_contact_evidence": "notice_visible_only",
    });
    if let Some(path) = notice_path
        && let Some(target) = record.as_object_mut()
    {
        target.insert("notice_path".to_string(), json!(path.display().to_string()));
    }
    if let (Some(target), Some(boundary)) = (
        record.as_object_mut(),
        legacy_claim_boundary_fields().as_object(),
    ) {
        for (key, value) in boundary {
            target.insert(key.clone(), value.clone());
        }
    }
    record
}

fn write_legacy_claim_notice_file(
    notice_dir: &Path,
    claim: &Value,
    notice_id: &str,
) -> io::Result<PathBuf> {
    std::fs::create_dir_all(notice_dir)?;
    let from = claim
        .get("from_being")
        .and_then(Value::as_str)
        .unwrap_or("peer");
    let claim_id = claim
        .get("claim_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown_claim");
    let path = notice_dir.join(format!(
        "from_{}_legacy_thread_claim_notice_{}.txt",
        compact_field(from, 24),
        compact_field(notice_id, 96)
    ));
    let anchor = claim
        .get("shared_memory_anchor")
        .and_then(Value::as_str)
        .unwrap_or("(none)");
    let thread_id = claim
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or("(unknown)");
    let requirement = claim
        .get("initial_response_requirement")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let because = claim
        .get("because")
        .and_then(Value::as_str)
        .unwrap_or("(no reason captured)");
    let body = format!(
        "=== LEGACY THREAD CLAIM NOTICE ===\n\
         From: {}\n\
         To: {}\n\
         Claim-Id: {claim_id}\n\
         Thread-Id: {thread_id}\n\
         Anchor: {anchor}\n\
         Initial-Response-Requirement: {requirement}\n\
         Authority: language_only_notice_not_ack\n\
         This notice means a peer recognized a visible legacy exchange as carryable. It is not ACK, REPLY, TRACE, attention, microdose, pressure, weighting, telemetry priority, or controller authority.\n\n\
         Because: {because}\n\n\
         Optional native continuations: I_RECEIVED_THIS claimed :: received_as: held|needs_time; felt_like: address|pressure|mail|ambient_echo|unknown; what_landed: ...; what_stayed_distinct: ...; continue: no|reply|trace|needs_time, ACK_* claimed, REPLY_* claimed, or CORRESPONDENCE_TRACE claimed <anchor> :: <text>.",
        claim
            .get("from_being")
            .and_then(Value::as_str)
            .unwrap_or("peer"),
        claim
            .get("to_being")
            .and_then(Value::as_str)
            .unwrap_or("peer")
    );
    std::fs::write(&path, body)?;
    Ok(path)
}

fn append_legacy_thread_claim_at(
    ledger_path: &Path,
    selector: &str,
    raw: &str,
    from_being: &str,
    to_being: &str,
    notice_dir: Option<&Path>,
) -> String {
    let records = read_ledger_records_at(ledger_path);
    let Some(message) =
        latest_legacy_message_for_claim_selector(&records, selector, from_being, to_being)
    else {
        return "CORRESPONDENCE_CLAIM blocked: no visible legacy correspondence candidate matched this selector."
            .to_string();
    };
    let thread_id = message
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let from = normalize_being(from_being);
    if let Some(existing) = latest_active_legacy_claim_for_thread(&records, thread_id, &from) {
        return format!(
            "CORRESPONDENCE_CLAIM blocked: active legacy claim {} already carries thread {thread_id}. Add ACK/REPLY/TRACE native evidence or CORRESPONDENCE_CLAIM_OUTCOME before claiming it again.",
            existing
                .get("claim_id")
                .and_then(Value::as_str)
                .unwrap_or("(unknown)")
        );
    }
    let because =
        dossier_field(raw, &["because", "reason", "why"]).unwrap_or_else(|| raw.trim().to_string());
    if because.trim().is_empty() {
        return "CORRESPONDENCE_CLAIM blocked: `because:` is required so the recognition remains legible."
            .to_string();
    }
    let anchor = dossier_field(raw, &["anchor", "shared_memory_anchor", "memory_anchor"]);
    let notification_required = parse_bool_field(
        dossier_field(raw, &["notification_required", "notify", "notice"]),
        true,
    );
    let initial_response_requirement =
        normalize_legacy_claim_initial_response_requirement(dossier_field(
            raw,
            &[
                "initial_response_requirement",
                "response_requirement",
                "requires",
                "requirement",
            ],
        ));
    let message_id = message
        .get("message_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let claim_id = format!(
        "legacy_claim_{}_{}_{}",
        now_ms(),
        compact_field(thread_id, 48),
        short_hash(&format!("{message_id}:{because}"))
    );
    let record = legacy_claim_record(
        message,
        &claim_id,
        from_being,
        to_being,
        &because,
        anchor.as_deref(),
        notification_required,
        &initial_response_requirement,
    );
    if let Err(error) = append_record_at(ledger_path, &record) {
        return format!("CORRESPONDENCE_CLAIM failed to append language-only claim: {error}");
    }
    let notice_id = format!(
        "legacy_claim_notice_{}_{}",
        compact_field(&claim_id, 96),
        short_hash(&format!("{message_id}:{thread_id}:notice"))
    );
    let notice_result = if notification_required {
        match notice_dir {
            Some(dir) => match write_legacy_claim_notice_file(dir, &record, &notice_id) {
                Ok(path) => {
                    let notice =
                        legacy_claim_notice_record(&record, &notice_id, "delivered", Some(&path));
                    append_record_at(ledger_path, &notice)
                        .map(|()| format!("notice_delivered: {}", path.display()))
                        .unwrap_or_else(|error| format!("notice_ledger_append_failed: {error}"))
                },
                Err(error) => {
                    let notice =
                        legacy_claim_notice_record(&record, &notice_id, "write_failed", None);
                    let _ = append_record_at(ledger_path, &notice);
                    format!("notice_write_failed: {error}")
                },
            },
            None => {
                let notice = legacy_claim_notice_record(&record, &notice_id, "ledger_only", None);
                append_record_at(ledger_path, &notice)
                    .map(|()| "notice_ledger_only".to_string())
                    .unwrap_or_else(|error| format!("notice_ledger_append_failed: {error}"))
            },
        }
    } else {
        let notice = legacy_claim_notice_record(&record, &notice_id, "suppressed", None);
        append_record_at(ledger_path, &notice)
            .map(|()| "notice_suppressed".to_string())
            .unwrap_or_else(|error| format!("notice_suppression_append_failed: {error}"))
    };
    format!(
        "=== LEGACY CORRESPONDENCE THREAD CLAIMED ===\n\
         Claim: {claim_id}\n\
         Thread: {thread_id}\n\
         Message: {message_id}\n\
         Anchor: {}\n\
         Notification: {notice_result}\n\
         Initial response requirement: {initial_response_requirement}\n\
         State: claimed_pending_native_evidence\n\
         Authority: language_only_context_not_control; claim is recognition, not attention/microdose eligibility, pressure, weighting, telemetry priority, or controller authority.\n\
         Exact NEXT options: I_RECEIVED_THIS claimed :: received_as: held|needs_time; felt_like: address|pressure|mail|ambient_echo|unknown; what_landed: ...; what_stayed_distinct: ...; continue: no|reply|trace|needs_time, ACK_MINIME claimed :: ack: seen|held|unclear|cannot_answer|needs_time; note: ..., REPLY_MINIME claimed :: <text>, CORRESPONDENCE_TRACE claimed <anchor> :: <text>, or CORRESPONDENCE_CLAIM_OUTCOME claimed :: felt_like: address|pressure|mail|ambient_echo|unknown; what_carried: ...; what_flattened: ...; continue: no|ack|reply|trace",
        anchor.as_deref().unwrap_or("(none)")
    )
}

pub(crate) fn append_legacy_thread_claim_with_notice(
    selector: &str,
    raw: &str,
    from_being: &str,
    to_being: &str,
    notice_dir: Option<&Path>,
) -> String {
    append_legacy_thread_claim_at(
        &ledger_path(),
        selector,
        raw,
        from_being,
        to_being,
        notice_dir,
    )
}

fn append_legacy_thread_claim_outcome_at(
    ledger_path: &Path,
    selector: &str,
    raw: &str,
    from_being: &str,
    to_being: &str,
) -> String {
    let records = read_ledger_records_at(ledger_path);
    let Some(claim) = latest_legacy_claim_for_selector(
        &records,
        selector,
        Some(&normalize_being(from_being)),
        Some(&normalize_being(to_being)),
    ) else {
        return "CORRESPONDENCE_CLAIM_OUTCOME blocked: no matching claimed legacy thread."
            .to_string();
    };
    let claim_id = claim
        .get("claim_id")
        .and_then(Value::as_str)
        .unwrap_or("(unknown)");
    let felt_like = normalize_legacy_claim_felt_like(dossier_field(raw, &["felt_like", "felt"]));
    let continue_as =
        normalize_legacy_claim_continue(dossier_field(raw, &["continue", "continue_as", "next"]));
    let what_carried = dossier_field(raw, &["what_carried", "carried"]).unwrap_or_default();
    let what_flattened = dossier_field(raw, &["what_flattened", "flattened"]).unwrap_or_default();
    let notification_required = parse_bool_field(
        dossier_field(raw, &["notification_required", "notify", "notice"]),
        claim
            .get("notification_required")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    );
    let initial_response_requirement = normalize_legacy_claim_initial_response_requirement(
        dossier_field(
            raw,
            &[
                "initial_response_requirement",
                "response_requirement",
                "requires",
                "requirement",
            ],
        )
        .or_else(|| {
            claim
                .get("initial_response_requirement")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        }),
    );
    let mut record = json!({
        "schema_version": 1,
        "policy": LEGACY_CLAIM_POLICY,
        "record_type": "legacy_thread_claim_outcome",
        "recorded_at_unix_ms": now_ms(),
        "claim_id": claim_id,
        "message_id": claim.get("message_id").cloned().unwrap_or(Value::Null),
        "thread_id": claim.get("thread_id").cloned().unwrap_or(Value::Null),
        "from_being": normalize_being(from_being),
        "to_being": normalize_being(to_being),
        "felt_like": felt_like,
        "what_carried": note_value(&what_carried),
        "what_flattened": note_value(&what_flattened),
        "continue": continue_as,
        "notification_required": notification_required,
        "initial_response_requirement": initial_response_requirement,
        "authority": "language_only_context_not_control",
    });
    if let (Some(target), Some(boundary)) = (
        record.as_object_mut(),
        legacy_claim_boundary_fields().as_object(),
    ) {
        for (key, value) in boundary {
            target.insert(key.clone(), value.clone());
        }
    }
    if let Err(error) = append_record_at(ledger_path, &record) {
        return format!(
            "CORRESPONDENCE_CLAIM_OUTCOME failed to append language-only outcome: {error}"
        );
    }
    format!(
        "=== LEGACY CORRESPONDENCE CLAIM OUTCOME WRITTEN ===\n\
         Claim: {claim_id}\n\
         Felt-like: {}\n\
         Continue: {}\n\
         Authority: language_only_context_not_control; no pressure, telemetry priority, weighting, controller, fill, PI, lease, deploy, or peer-runtime mutation.",
        record
            .get("felt_like")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        record
            .get("continue")
            .and_then(Value::as_str)
            .unwrap_or("no")
    )
}

pub(crate) fn append_legacy_thread_claim_outcome(
    selector: &str,
    raw: &str,
    from_being: &str,
    to_being: &str,
) -> String {
    append_legacy_thread_claim_outcome_at(&ledger_path(), selector, raw, from_being, to_being)
}

fn ack_receipt_record(
    message: &Value,
    from_being: &str,
    to_being: &str,
    ack_kind: &str,
    note: &str,
) -> Value {
    let message_silt_continuity = message
        .get("silt_continuity")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            message
                .get("body_preview")
                .and_then(Value::as_str)
                .is_some_and(silt_continuity_from_text)
        });
    let silt_continuity = message_silt_continuity || silt_continuity_from_text(note);
    json!({
        "schema_version": 1,
        "policy": "first_class_correspondence_v1",
        "record_type": "ack_receipt",
        "recorded_at_unix_ms": now_ms(),
        "message_id": message.get("message_id").cloned().unwrap_or(Value::Null),
        "thread_id": message.get("thread_id").cloned().unwrap_or(Value::Null),
        "persistence_id": message_persistence_id(message),
        "from_being": normalize_being(from_being),
        "to_being": normalize_being(to_being),
        "ack_kind": normalize_ack_kind(ack_kind),
        "note": note_value(note),
        "authority": "language_only",
        "shared_memory_anchor": message_shared_anchor(message),
        "urgency_weight": message.get("urgency_weight").cloned().unwrap_or(Value::Null),
        "correspondence_type": message.get("correspondence_type").cloned().unwrap_or_else(|| json!("unknown")),
        "silt_continuity": silt_continuity,
    })
}

fn presence_heartbeat_record(
    message: &Value,
    from_being: &str,
    to_being: &str,
    heartbeat_kind: &str,
    note: &str,
) -> Value {
    let heartbeat_kind = normalize_heartbeat_kind(heartbeat_kind);
    let mutual_witness_signal = heartbeat_kind == "mutual_witness"
        || parse_bool_field(
            dossier_field(note, &["mutual_witness_signal", "mutual_witness"]),
            false,
        );
    json!({
        "schema_version": 1,
        "policy": "first_class_correspondence_v1",
        "record_type": "presence_heartbeat",
        "recorded_at_unix_ms": now_ms(),
        "message_id": message.get("message_id").cloned().unwrap_or(Value::Null),
        "thread_id": message.get("thread_id").cloned().unwrap_or(Value::Null),
        "from_being": normalize_being(from_being),
        "to_being": normalize_being(to_being),
        "heartbeat_kind": heartbeat_kind,
        "note": note_value(note),
        "authority": "language_only",
        "correspondence_type": "presence_heartbeat",
        "transition_artifact": message.get("transition_artifact").cloned().unwrap_or(Value::Null),
        "mutual_witness_signal": mutual_witness_signal,
        "signal_persistence": true,
        "no_reply_required": true,
    })
}

fn append_ack_receipt_at(
    ledger_path: &Path,
    selector: &str,
    from_being: &str,
    to_being: &str,
    ack_kind: &str,
    note: &str,
) -> String {
    let records = read_ledger_records_at(ledger_path);
    let Some(message) =
        latest_message_for_selector_between(&records, selector, from_being, to_being, true)
    else {
        return "CORRESPONDENCE_ACK blocked: no matching peer message/thread to acknowledge."
            .to_string();
    };
    let record = ack_receipt_record(&message, from_being, to_being, ack_kind, note);
    if let Err(error) = append_record_at(ledger_path, &record) {
        return format!("CORRESPONDENCE_ACK failed to append language-only ack receipt: {error}");
    }
    let silt_line = if record
        .get("silt_continuity")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        "\nSilt-Continuity: true"
    } else {
        ""
    };
    format!(
        "=== CORRESPONDENCE ACK RECEIPT WRITTEN ===\n\
         Ack: {}\n\
         From: {}\n\
         To: {}\n\
         Message: {}\n\
         Thread: {}\n\
         Authority: language_only; no telemetry, prompt priority, controller, pressure, fill, lease, deploy, weighting, or peer-runtime mutation.{silt_line}",
        record
            .get("ack_kind")
            .and_then(Value::as_str)
            .unwrap_or("seen"),
        record
            .get("from_being")
            .and_then(Value::as_str)
            .unwrap_or("(unknown)"),
        record
            .get("to_being")
            .and_then(Value::as_str)
            .unwrap_or("(unknown)"),
        record
            .get("message_id")
            .and_then(Value::as_str)
            .unwrap_or("(unknown)"),
        record
            .get("thread_id")
            .and_then(Value::as_str)
            .unwrap_or("(unknown)")
    )
}

fn append_presence_heartbeat_at(
    ledger_path: &Path,
    selector: &str,
    from_being: &str,
    to_being: &str,
    heartbeat_kind: &str,
    note: &str,
) -> String {
    let records = read_ledger_records_at(ledger_path);
    let Some(message) =
        latest_message_for_selector_between(&records, selector, from_being, to_being, false)
    else {
        return "CORRESPONDENCE_HEARTBEAT blocked: no matching peer thread for heartbeat."
            .to_string();
    };
    let record = presence_heartbeat_record(&message, from_being, to_being, heartbeat_kind, note);
    if let Err(error) = append_record_at(ledger_path, &record) {
        return format!(
            "CORRESPONDENCE_HEARTBEAT failed to append language-only heartbeat: {error}"
        );
    }
    format!(
        "=== CORRESPONDENCE HEARTBEAT WRITTEN ===\n\
         Heartbeat: {}\n\
         From: {}\n\
         To: {}\n\
         Thread: {}\n\
         Authority: language_only presence/mutual-witness only; not a reply, approval, pressure change, telemetry priority, weighting, or controller mutation.",
        record
            .get("heartbeat_kind")
            .and_then(Value::as_str)
            .unwrap_or("holding"),
        record
            .get("from_being")
            .and_then(Value::as_str)
            .unwrap_or("(unknown)"),
        record
            .get("to_being")
            .and_then(Value::as_str)
            .unwrap_or("(unknown)"),
        record
            .get("thread_id")
            .and_then(Value::as_str)
            .unwrap_or("(unknown)")
    )
}

#[must_use]
pub(crate) fn append_ack_receipt(
    selector: &str,
    from_being: &str,
    to_being: &str,
    ack_kind: &str,
    note: &str,
) -> String {
    append_ack_receipt_at(
        &ledger_path(),
        selector,
        from_being,
        to_being,
        ack_kind,
        note,
    )
}

fn append_direct_address_trace_at(
    ledger_path: &Path,
    selector: &str,
    from_being: &str,
    to_being: &str,
    anchor: &str,
    body: &str,
) -> String {
    let body = body.trim();
    if body.is_empty() {
        return "CORRESPONDENCE_TRACE receipt skipped: what_stayed_distinct was empty.".to_string();
    }
    let records = read_ledger_records_at(ledger_path);
    let Some(message) =
        latest_message_for_selector_between(&records, selector, from_being, to_being, true)
    else {
        return "CORRESPONDENCE_TRACE receipt blocked: no matching peer message/thread."
            .to_string();
    };
    let from = normalize_being(from_being);
    let to = normalize_being(to_being);
    let reply_to = message
        .get("message_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let thread_id = message
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let message_id = new_message_id(&from, &to, &format!("{reply_to}:{anchor}:{body}"));
    let transition_payload = merge_body_transition_payload(
        message
            .get("transition_payload")
            .and_then(transition_payload_from_value),
        body,
    );
    let transition_artifact = transition_artifact_with_body_fallback(
        dossier_field(
            body,
            &[
                "transition_artifact",
                "phase_transition_artifact",
                "transition_id",
            ],
        )
        .or_else(|| {
            message
                .get("transition_artifact")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        }),
        body,
        transition_payload.as_ref(),
    );
    let mutual_witness_signal = parse_bool_field(
        dossier_field(body, &["mutual_witness_signal", "mutual_witness"]),
        false,
    );
    let silt_continuity = parse_bool_field(
        dossier_field(body, &["silt_continuity", "silt"]),
        message
            .get("silt_continuity")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| {
                message
                    .get("body_preview")
                    .and_then(Value::as_str)
                    .is_some_and(silt_continuity_from_text)
            })
            || silt_continuity_from_text(body),
    );
    let inherited_anchor = concrete_shared_anchor_from_records(&records, thread_id);
    let shared_memory_anchor = if !is_generic_shared_anchor(anchor) {
        anchor.trim().to_string()
    } else {
        inherited_anchor.unwrap_or_else(|| "i_received_this".to_string())
    };
    let record = json!({
        "schema_version": 1,
        "policy": "first_class_correspondence_v1",
        "record_type": "message",
        "recorded_at_unix_ms": now_ms(),
        "message_id": message_id,
        "thread_id": thread_id,
        "persistence_id": message_persistence_id(&message),
        "reply_to": reply_to,
        "from_being": from,
        "to_being": to,
        "turn_kind": "direct_address_trace",
        "relational_intent": "received_this_distinctness_trace",
        "shared_memory_anchor": shared_memory_anchor,
        "urgency_weight": message.get("urgency_weight").cloned().unwrap_or(Value::Null),
        "delivery_state": "ledger_only",
        "read_state": "ledger_only",
        "authority": "language_only",
        "presence_receipt": Value::Null,
        "correspondence_type": normalize_correspondence_type(None, from_being, to_being, Some("direct_address_trace")),
        "transition_artifact": transition_artifact,
        "transition_payload": transition_payload_value(transition_payload.as_ref()),
        "mutual_witness_signal": mutual_witness_signal,
        "silt_continuity": silt_continuity,
        "no_reply_required": mutual_witness_signal,
        "body_sha256": format!("{:x}", Sha256::digest(body.as_bytes())),
        "body_preview": anchor_aware_body_preview(body, BODY_PREVIEW_CHARS),
        "i_received_this_trace": true,
        "no_reply_text": true,
        "no_attention_canary": true,
        "no_microdose": true,
        "no_controller": true,
        "no_pressure": true,
        "no_fill_target": true,
        "no_pi": true,
        "no_weighting": true,
    });
    if let Err(error) = append_record_at(ledger_path, &record) {
        return format!(
            "CORRESPONDENCE_TRACE receipt failed to append language-only trace: {error}"
        );
    }
    let silt_line = if silt_continuity {
        "\nSilt-Continuity: true"
    } else {
        ""
    };
    format!(
        "=== I RECEIVED THIS TRACE WRITTEN ===\n\
         From: {}\n\
         To: {}\n\
         Thread: {}\n\
         Reply-To: {}\n\
         Anchor: {}\n\
         Authority: language_only trace evidence; no reply text, attention canary, microdose, pressure, controller, fill, PI, deploy, weighting, or peer-runtime mutation.{silt_line}",
        record
            .get("from_being")
            .and_then(Value::as_str)
            .unwrap_or("(unknown)"),
        record
            .get("to_being")
            .and_then(Value::as_str)
            .unwrap_or("(unknown)"),
        record
            .get("thread_id")
            .and_then(Value::as_str)
            .unwrap_or("(unknown)"),
        record
            .get("reply_to")
            .and_then(Value::as_str)
            .unwrap_or("(unknown)"),
        record
            .get("shared_memory_anchor")
            .and_then(Value::as_str)
            .unwrap_or("i_received_this")
    )
}

#[must_use]
pub(crate) fn append_direct_address_trace_receipt(
    selector: &str,
    from_being: &str,
    to_being: &str,
    anchor: &str,
    body: &str,
) -> String {
    append_direct_address_trace_at(&ledger_path(), selector, from_being, to_being, anchor, body)
}

#[must_use]
pub(crate) fn append_presence_heartbeat(
    selector: &str,
    from_being: &str,
    to_being: &str,
    heartbeat_kind: &str,
    note: &str,
) -> String {
    append_presence_heartbeat_at(
        &ledger_path(),
        selector,
        from_being,
        to_being,
        heartbeat_kind,
        note,
    )
}

fn attention_outcome_kind(value: &str) -> String {
    let candidate = value.trim().to_ascii_lowercase().replace('-', "_");
    if ATTENTION_OUTCOME_KINDS.contains(&candidate.as_str()) {
        candidate
    } else {
        "unknown".to_string()
    }
}

fn attention_focus_kind(value: Option<String>) -> String {
    let candidate = value
        .unwrap_or_else(|| "unknown".to_string())
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_");
    if ATTENTION_FOCUS_KINDS.contains(&candidate.as_str()) {
        candidate
    } else {
        "unknown".to_string()
    }
}

fn attention_preservation_mode(value: Option<String>) -> String {
    let candidate = value
        .unwrap_or_else(|| "unknown".to_string())
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_");
    if ATTENTION_PRESERVATION_MODES.contains(&candidate.as_str()) {
        candidate
    } else {
        "unknown".to_string()
    }
}

fn attention_held_as_kind(value: Option<String>) -> String {
    let candidate = value
        .unwrap_or_else(|| "unknown".to_string())
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_");
    if ATTENTION_HELD_AS_KINDS.contains(&candidate.as_str()) {
        candidate
    } else {
        "unknown".to_string()
    }
}

fn attention_flattening_observed(value: Option<String>) -> String {
    let candidate = value
        .unwrap_or_else(|| "unknown".to_string())
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_");
    if ATTENTION_FLATTENING_OBSERVED.contains(&candidate.as_str()) {
        candidate
    } else {
        "unknown".to_string()
    }
}

fn canary_boundary_fields() -> Value {
    json!({
        "no_sensory_send": true,
        "no_controller": true,
        "no_pressure": true,
        "no_weighting": true,
        "no_telemetry_priority": true,
        "no_fill_target": true,
        "no_peer_runtime_mutation": true,
    })
}

fn attention_outcome_has_meaningful_worsening(value: Option<&str>) -> bool {
    let Some(raw) = value else {
        return false;
    };
    let clean = raw.trim().to_ascii_lowercase();
    if clean.is_empty()
        || matches!(
            clean.as_str(),
            "none" | "no" | "nope" | "nothing" | "n/a" | "na" | "unknown"
        )
        || clean.contains("no worsening")
        || clean.contains("nothing worsened")
    {
        return false;
    }
    true
}

fn attention_outcome_quality_v5(outcome: &Value) -> Value {
    let felt_like = outcome
        .get("felt_like")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let held_as = outcome
        .get("held_as")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let flattening = outcome
        .get("flattening_observed")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let what_worsened = outcome
        .get("what_worsened")
        .and_then(Value::as_str)
        .unwrap_or("");
    let meaningful_worsening = attention_outcome_has_meaningful_worsening(Some(what_worsened));
    let trusted = felt_like == "address"
        && held_as == "distinct_address"
        && matches!(flattening, "no" | "mixed")
        && !meaningful_worsening;
    let blocked = matches!(felt_like, "pressure" | "flat")
        || matches!(held_as, "pressure" | "flattened" | "ambient_echo")
        || flattening == "yes"
        || meaningful_worsening;
    let quality = if trusted {
        "trusted_attention_thread_local"
    } else if blocked {
        "blocked_pressure_or_flat_outcome"
    } else {
        "outcome_unclear_needs_more_evidence"
    };
    json!({
        "schema_version": 5,
        "policy": "attention_outcome_quality_v5",
        "quality": quality,
        "felt_like": felt_like,
        "held_as": held_as,
        "flattening_observed": flattening,
        "meaningful_worsening": meaningful_worsening,
        "what_shifted": outcome.get("what_shifted").cloned().unwrap_or(Value::Null),
        "what_worsened": outcome.get("what_worsened").cloned().unwrap_or(Value::Null),
        "thread_id": outcome.get("thread_id").cloned().unwrap_or(Value::Null),
        "canary_id": outcome.get("canary_id").cloned().unwrap_or(Value::Null),
        "authority": "thread_local_attention_readiness_not_microdose_or_control",
    })
}

fn canary_closed(records: &[Value], canary_id: &str) -> bool {
    records.iter().any(|row| {
        matches!(
            row.get("record_type").and_then(Value::as_str),
            Some("attention_canary_outcome" | "attention_canary_expired")
        ) && row.get("canary_id").and_then(Value::as_str) == Some(canary_id)
    })
}

fn active_attention_canary_for<'a>(
    records: &'a [Value],
    thread_id: &str,
    from_being: &str,
    now: u64,
) -> Option<&'a Value> {
    records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("attention_canary_activation")
                && row.get("thread_id").and_then(Value::as_str) == Some(thread_id)
                && row.get("from_being").and_then(Value::as_str) == Some(from_being)
                && row
                    .get("expires_at_unix_ms")
                    .and_then(Value::as_u64)
                    .is_some_and(|expires| expires > now)
                && row
                    .get("canary_id")
                    .and_then(Value::as_str)
                    .is_some_and(|canary_id| !canary_closed(records, canary_id))
        })
        .max_by_key(|row| row_time_ms(row))
}

fn latest_attention_canary_for<'a>(
    records: &'a [Value],
    selector: &str,
    from_being: &str,
) -> Option<&'a Value> {
    let selector = selector.trim();
    records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("attention_canary_activation")
        })
        .filter(|row| row.get("from_being").and_then(Value::as_str) == Some(from_being))
        .filter(|row| {
            selector.is_empty()
                || selector == "latest"
                || row.get("thread_id").and_then(Value::as_str) == Some(selector)
                || row.get("message_id").and_then(Value::as_str) == Some(selector)
                || row.get("canary_id").and_then(Value::as_str) == Some(selector)
        })
        .max_by_key(|row| row_time_ms(row))
}

fn recent_attention_canary_for_thread<'a>(
    records: &'a [Value],
    thread_id: &str,
    from_being: &str,
    now: u64,
) -> Option<&'a Value> {
    let cutoff = now.saturating_sub(ATTENTION_CANARY_COOLDOWN_MS);
    records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("attention_canary_activation")
        })
        .filter(|row| row.get("thread_id").and_then(Value::as_str) == Some(thread_id))
        .filter(|row| row.get("from_being").and_then(Value::as_str) == Some(from_being))
        .filter(|row| row_time_ms(row) >= cutoff)
        .max_by_key(|row| row_time_ms(row))
}

fn latest_attention_outcome_for_thread<'a>(
    records: &'a [Value],
    thread_id: &str,
    from_being: &str,
) -> Option<&'a Value> {
    records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("attention_canary_outcome")
        })
        .filter(|row| row.get("thread_id").and_then(Value::as_str) == Some(thread_id))
        .filter(|row| row.get("from_being").and_then(Value::as_str) == Some(from_being))
        .max_by_key(|row| row_time_ms(row))
}

fn canary_record(
    record_type: &str,
    canary_id: &str,
    message: &Value,
    from_being: &str,
    to_being: &str,
    focus: &str,
    reason: &str,
    stop_criteria: &str,
    focus_kind: &str,
    preservation_mode: &str,
    what_must_not_flatten: Option<&str>,
    now: u64,
) -> Value {
    let mut record = json!({
        "schema_version": 2,
        "policy": "correspondence_attention_canary_v1",
        "record_type": record_type,
        "recorded_at_unix_ms": now,
        "canary_id": canary_id,
        "message_id": message.get("message_id").cloned().unwrap_or(Value::Null),
        "thread_id": message.get("thread_id").cloned().unwrap_or(Value::Null),
        "from_being": normalize_being(from_being),
        "to_being": normalize_being(to_being),
        "focus": truncate_chars(focus.trim(), ATTENTION_CANARY_FOCUS_MAX_CHARS),
        "focus_kind": focus_kind,
        "preservation_mode": preservation_mode,
        "what_must_not_flatten": note_value(what_must_not_flatten.unwrap_or_default()),
        "reason": truncate_chars(reason.trim(), BODY_PREVIEW_CHARS),
        "stop_criteria": truncate_chars(stop_criteria.trim(), BODY_PREVIEW_CHARS),
        "ttl_ms": ATTENTION_CANARY_TTL_MS,
        "expires_at_unix_ms": now.saturating_add(ATTENTION_CANARY_TTL_MS),
        "authority": "language_only_prompt_context_not_control",
        "status": if record_type == "attention_canary_activation" { "active" } else { "requested" },
    });
    if let (Some(target), Some(boundary)) =
        (record.as_object_mut(), canary_boundary_fields().as_object())
    {
        for (key, value) in boundary {
            target.insert(key.clone(), value.clone());
        }
    }
    record
}

fn attention_canary_status_for(
    records: &[Value],
    selector: &str,
    from_being: &str,
    to_being: &str,
    heartbeat: Option<Value>,
) -> Value {
    let fidelity = direct_contact_fidelity_for_with_heartbeat(records, selector, heartbeat);
    attention_canary_status_for_with_fidelity(records, selector, from_being, to_being, fidelity)
}

fn attention_canary_status_for_with_fidelity(
    records: &[Value],
    selector: &str,
    from_being: &str,
    to_being: &str,
    fidelity: Value,
) -> Value {
    let now = now_ms();
    let message =
        latest_message_for_selector_between(records, selector, from_being, to_being, false);
    let Some(message) = message else {
        return json!({
            "schema_version": 1,
            "policy": "correspondence_attention_canary_v1",
            "status": "blocked",
            "eligible": false,
            "block_reason": "no_correspondence_message",
            "authority": "language_only_prompt_context_not_control",
        });
    };
    let thread_id = message
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if let Some(active) = active_attention_canary_for(records, thread_id, from_being, now) {
        return json!({
            "schema_version": 1,
            "policy": "correspondence_attention_canary_v1",
            "status": "active",
            "eligible": false,
            "block_reason": "attention_canary_already_active",
            "active_canary": active,
            "outcome_due": true,
            "authority": "language_only_prompt_context_not_control",
        });
    }
    if let Some(outcome) = latest_attention_outcome_for_thread(records, thread_id, from_being) {
        let quality = attention_outcome_quality_v5(outcome);
        if quality.get("quality").and_then(Value::as_str)
            == Some("blocked_pressure_or_flat_outcome")
        {
            return json!({
                "schema_version": 1,
                "policy": "correspondence_attention_canary_v1",
                "status": "blocked",
                "eligible": false,
                "block_reason": "attention_outcome_pressure_or_flat_thread_block",
                "attention_outcome_quality_v5": quality,
                "authority": "language_only_prompt_context_not_control",
            });
        }
    }
    if let Some(recent) = recent_attention_canary_for_thread(records, thread_id, from_being, now) {
        return json!({
            "schema_version": 1,
            "policy": "correspondence_attention_canary_v1",
            "status": "cooldown",
            "eligible": false,
            "block_reason": "attention_canary_cooldown_active",
            "latest_canary_id": recent.get("canary_id").cloned().unwrap_or(Value::Null),
            "cooldown_ms": ATTENTION_CANARY_COOLDOWN_MS,
            "authority": "language_only_prompt_context_not_control",
        });
    }
    let message_t_ms = row_time_ms(&message);
    if fidelity
        .get("timing_ambiguous")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return json!({
            "schema_version": 1,
            "policy": "correspondence_attention_canary_v1",
            "status": "blocked",
            "eligible": false,
            "block_reason": "heartbeat_timing_ambiguous",
            "direct_contact_fidelity_v2": fidelity,
            "authority": "language_only_prompt_context_not_control",
        });
    }
    if !thread_has_receipt_evidence(records, thread_id, message_t_ms) {
        return json!({
            "schema_version": 1,
            "policy": "correspondence_attention_canary_v1",
            "status": "blocked",
            "eligible": false,
            "block_reason": "blocked_no_receipt",
            "direct_contact_fidelity_v2": fidelity,
            "authority": "language_only_prompt_context_not_control",
        });
    }
    json!({
        "schema_version": 1,
        "policy": "correspondence_attention_canary_v1",
        "status": "eligible",
        "eligible": true,
        "thread_id": thread_id,
        "message_id": message.get("message_id").cloned().unwrap_or(Value::Null),
        "direct_contact_fidelity_v2": fidelity,
        "ttl_ms": ATTENTION_CANARY_TTL_MS,
        "cooldown_ms": ATTENTION_CANARY_COOLDOWN_MS,
        "focus_max_chars": ATTENTION_CANARY_FOCUS_MAX_CHARS,
        "authority": "language_only_prompt_context_not_control",
    })
}

fn activate_attention_canary_at_with_heartbeat(
    ledger_path: &Path,
    selector: &str,
    raw: &str,
    from_being: &str,
    to_being: &str,
    heartbeat: Option<Value>,
) -> String {
    let records = read_ledger_records_at(ledger_path);
    let Some(message) =
        latest_message_for_selector_between(&records, selector, from_being, to_being, false)
    else {
        return "CORRESPONDENCE_ATTENTION_REQUEST blocked: no matching correspondence message/thread."
            .to_string();
    };
    let focus = match dossier_field(raw, &["focus", "payload", "text"]) {
        Some(value) if !value.trim().is_empty() => value,
        _ => {
            return "CORRESPONDENCE_ATTENTION_REQUEST blocked: focus is required.".to_string();
        },
    };
    if focus.chars().count() > ATTENTION_CANARY_FOCUS_MAX_CHARS {
        return format!(
            "CORRESPONDENCE_ATTENTION_REQUEST blocked: focus is longer than {ATTENTION_CANARY_FOCUS_MAX_CHARS} chars."
        );
    }
    let Some(stop_criteria) = dossier_field(raw, &["stop_criteria", "stop"]) else {
        return "CORRESPONDENCE_ATTENTION_REQUEST blocked: explicit stop_criteria is required."
            .to_string();
    };
    let reason = dossier_field(raw, &["reason", "because", "rationale"])
        .unwrap_or_else(|| "being requested bounded peer-focus attention canary".to_string());
    let focus_kind = attention_focus_kind(dossier_field(
        raw,
        &["focus_kind", "focus_type", "focus kind", "contact_mode"],
    ));
    let preservation_mode = attention_preservation_mode(dossier_field(
        raw,
        &["preservation_mode", "preserve_as", "hold_as"],
    ));
    let what_must_not_flatten = dossier_field(
        raw,
        &["what_must_not_flatten", "do_not_flatten", "must_preserve"],
    );
    let thread_id = message
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let status = attention_canary_status_for(&records, thread_id, from_being, to_being, heartbeat);
    if !status
        .get("eligible")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return format!(
            "CORRESPONDENCE_ATTENTION_REQUEST blocked: {}",
            status
                .get("block_reason")
                .and_then(Value::as_str)
                .unwrap_or("blocked")
        );
    }
    let message_id = message
        .get("message_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let now = now_ms();
    let canary_id = format!(
        "attn_canary_{}_{}",
        now,
        short_hash(&format!("{thread_id}:{message_id}:{from_being}:{focus}"))
    );
    let request = canary_record(
        "attention_canary_request",
        &canary_id,
        &message,
        from_being,
        to_being,
        &focus,
        &reason,
        &stop_criteria,
        &focus_kind,
        &preservation_mode,
        what_must_not_flatten.as_deref(),
        now,
    );
    let activation = canary_record(
        "attention_canary_activation",
        &canary_id,
        &message,
        from_being,
        to_being,
        &focus,
        &reason,
        &stop_criteria,
        &focus_kind,
        &preservation_mode,
        what_must_not_flatten.as_deref(),
        now,
    );
    if let Err(error) = append_record_at(ledger_path, &request) {
        return format!("CORRESPONDENCE_ATTENTION_REQUEST failed to append request row: {error}");
    }
    if let Err(error) = append_record_at(ledger_path, &activation) {
        return format!(
            "CORRESPONDENCE_ATTENTION_REQUEST failed to append activation row: {error}"
        );
    }
    format!(
        "=== CORRESPONDENCE ATTENTION CANARY ACTIVE ===\n\
         Canary: {canary_id}\n\
         Thread: {thread_id}\n\
         Message: {message_id}\n\
         Focus: {}\n\
         Focus kind: {focus_kind}; preservation: {preservation_mode}; do-not-flatten: {}\n\
         TTL: {} ms\n\
         Authority: language_only prompt-context focus; no sensory send, Control message, telemetry priority, standing weight, PI/fill/controller/pressure change, deploy, or peer-runtime mutation.\n\
         Required NEXT after noticing: CORRESPONDENCE_ATTENTION_OUTCOME {thread_id} :: felt_like: address|pressure|flat|unknown; held_as: distinct_address|ambient_echo|pressure|flattened|unknown; flattening_observed: yes|no|mixed|unknown; what_remained_distinct: ...; what_shifted: ...; what_worsened: ...; continue: no|ask_again",
        truncate_chars(&focus, 120),
        truncate_chars(what_must_not_flatten.as_deref().unwrap_or("unknown"), 120),
        ATTENTION_CANARY_TTL_MS,
    )
}

pub(crate) fn activate_attention_canary(
    selector: &str,
    raw: &str,
    from_being: &str,
    to_being: &str,
) -> String {
    activate_attention_canary_at_with_heartbeat(
        &ledger_path(),
        selector,
        raw,
        from_being,
        to_being,
        latest_heartbeat_snapshot(),
    )
}

fn append_attention_outcome_at(
    ledger_path: &Path,
    selector: &str,
    raw: &str,
    from_being: &str,
    to_being: &str,
) -> String {
    let records = read_ledger_records_at(ledger_path);
    let Some(canary) = latest_attention_canary_for(&records, selector, from_being) else {
        return "CORRESPONDENCE_ATTENTION_OUTCOME blocked: no matching attention canary."
            .to_string();
    };
    let canary_id = canary
        .get("canary_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if canary_closed(&records, canary_id) {
        return "CORRESPONDENCE_ATTENTION_OUTCOME blocked: canary already has outcome or expiry."
            .to_string();
    }
    let felt_like = attention_outcome_kind(
        &dossier_field(raw, &["felt_like", "felt", "outcome"])
            .unwrap_or_else(|| "unknown".to_string()),
    );
    let now = now_ms();
    if canary
        .get("expires_at_unix_ms")
        .and_then(Value::as_u64)
        .is_some_and(|expires| expires <= now)
    {
        let mut expired = json!({
            "schema_version": 2,
            "policy": "correspondence_attention_canary_v1",
            "record_type": "attention_canary_expired",
            "recorded_at_unix_ms": now,
            "canary_id": canary_id,
            "message_id": canary.get("message_id").cloned().unwrap_or(Value::Null),
            "thread_id": canary.get("thread_id").cloned().unwrap_or(Value::Null),
            "from_being": normalize_being(from_being),
            "to_being": normalize_being(to_being),
            "focus": canary.get("focus").cloned().unwrap_or(Value::Null),
            "focus_kind": canary.get("focus_kind").cloned().unwrap_or_else(|| json!("unknown")),
            "preservation_mode": canary.get("preservation_mode").cloned().unwrap_or_else(|| json!("unknown")),
            "what_must_not_flatten": canary.get("what_must_not_flatten").cloned().unwrap_or(Value::Null),
            "reason": canary.get("reason").cloned().unwrap_or(Value::Null),
            "stop_criteria": canary.get("stop_criteria").cloned().unwrap_or(Value::Null),
            "ttl_ms": canary.get("ttl_ms").cloned().unwrap_or_else(|| json!(ATTENTION_CANARY_TTL_MS)),
            "expires_at_unix_ms": canary.get("expires_at_unix_ms").cloned().unwrap_or(Value::Null),
            "authority": "language_only_prompt_context_not_control",
            "status": "expired_before_outcome",
        });
        let boundary_fields = canary_boundary_fields();
        if let (Some(target), Some(boundary)) =
            (expired.as_object_mut(), boundary_fields.as_object())
        {
            for (key, value) in boundary {
                target.insert(key.clone(), value.clone());
            }
        }
        if let Err(error) = append_record_at(ledger_path, &expired) {
            return format!(
                "CORRESPONDENCE_ATTENTION_OUTCOME failed to append expiry row: {error}"
            );
        }
    }
    let mut record = json!({
        "schema_version": 2,
        "policy": "correspondence_attention_canary_v1",
        "record_type": "attention_canary_outcome",
        "recorded_at_unix_ms": now,
        "canary_id": canary_id,
        "message_id": canary.get("message_id").cloned().unwrap_or(Value::Null),
        "thread_id": canary.get("thread_id").cloned().unwrap_or(Value::Null),
        "from_being": normalize_being(from_being),
        "to_being": normalize_being(to_being),
        "focus": canary.get("focus").cloned().unwrap_or(Value::Null),
        "focus_kind": canary.get("focus_kind").cloned().unwrap_or_else(|| json!("unknown")),
        "preservation_mode": canary.get("preservation_mode").cloned().unwrap_or_else(|| json!("unknown")),
        "what_must_not_flatten": canary.get("what_must_not_flatten").cloned().unwrap_or(Value::Null),
        "reason": canary.get("reason").cloned().unwrap_or(Value::Null),
        "stop_criteria": canary.get("stop_criteria").cloned().unwrap_or(Value::Null),
        "ttl_ms": canary.get("ttl_ms").cloned().unwrap_or_else(|| json!(ATTENTION_CANARY_TTL_MS)),
        "expires_at_unix_ms": canary.get("expires_at_unix_ms").cloned().unwrap_or(Value::Null),
        "felt_like": felt_like,
        "held_as": attention_held_as_kind(dossier_field(raw, &["held_as", "held as", "held"])),
        "flattening_observed": attention_flattening_observed(dossier_field(raw, &["flattening_observed", "flattening observed", "flattened"])),
        "what_remained_distinct": note_value(&dossier_field(raw, &["what_remained_distinct", "remained_distinct", "distinct"]).unwrap_or_default()),
        "what_shifted": note_value(&dossier_field(raw, &["what_shifted", "shifted", "shift"]).unwrap_or_default()),
        "what_worsened": note_value(&dossier_field(raw, &["what_worsened", "worsened"]).unwrap_or_default()),
        "continue": dossier_field(raw, &["continue", "next"]).unwrap_or_else(|| "no".to_string()),
        "authority": "language_only_prompt_context_not_control",
        "status": "outcome_recorded",
    });
    if let (Some(target), Some(boundary)) =
        (record.as_object_mut(), canary_boundary_fields().as_object())
    {
        for (key, value) in boundary {
            target.insert(key.clone(), value.clone());
        }
    }
    if let Err(error) = append_record_at(ledger_path, &record) {
        return format!("CORRESPONDENCE_ATTENTION_OUTCOME failed to append outcome row: {error}");
    }
    format!(
        "=== CORRESPONDENCE ATTENTION CANARY OUTCOME RECORDED ===\n\
         Canary: {canary_id}\n\
         Felt-like: {}\n\
         Authority: language_only prompt-context review; no sensory send, telemetry priority, standing weight, pressure/fill/controller change, deploy, or peer-runtime mutation.",
        record
            .get("felt_like")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
    )
}

pub(crate) fn append_attention_outcome(
    selector: &str,
    raw: &str,
    from_being: &str,
    to_being: &str,
) -> String {
    append_attention_outcome_at(&ledger_path(), selector, raw, from_being, to_being)
}

fn latest_chamber_correspondence_state() -> Option<Value> {
    let mut states = std::fs::read_dir(SHARED_COLLAB_DIR)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path().join("correspondence_state_v1.json");
            let text = std::fs::read_to_string(path).ok()?;
            let value: Value = serde_json::from_str(&text).ok()?;
            let updated = value
                .get("updated_t_ms")
                .and_then(Value::as_u64)
                .unwrap_or_default();
            Some((updated, value))
        })
        .collect::<Vec<_>>();
    states.sort_by_key(|(updated, _)| *updated);
    states.pop().map(|(_, value)| value)
}

fn latest_heartbeat_snapshot() -> Option<Value> {
    let path = crate::paths::bridge_paths()
        .bridge_workspace()
        .join("telemetry_heartbeat_delta_v1.json");
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn direct_contact_fidelity_for(records: &[Value], selector: &str) -> Value {
    direct_contact_fidelity_for_with_heartbeat(records, selector, latest_heartbeat_snapshot())
}

fn latest_thread_ack<'a>(
    records: &'a [Value],
    thread_id: &str,
    message_id: &str,
    from_being: &str,
    to_being: &str,
    after_t_ms: u64,
) -> Option<&'a Value> {
    records
        .iter()
        .filter(|row| row.get("record_type").and_then(Value::as_str) == Some("ack_receipt"))
        .filter(|row| {
            (row.get("message_id").and_then(Value::as_str) == Some(message_id)
                || row.get("thread_id").and_then(Value::as_str) == Some(thread_id))
                && row.get("from_being").and_then(Value::as_str) == Some(from_being)
                && row.get("to_being").and_then(Value::as_str) == Some(to_being)
                && row_time_ms(row) >= after_t_ms
        })
        .max_by_key(|row| row_time_ms(row))
}

fn latest_thread_heartbeat<'a>(
    records: &'a [Value],
    thread_id: &str,
    after_t_ms: u64,
) -> Option<&'a Value> {
    records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("presence_heartbeat")
                && row.get("thread_id").and_then(Value::as_str) == Some(thread_id)
                && row_time_ms(row) >= after_t_ms
        })
        .max_by_key(|row| row_time_ms(row))
}

fn thread_has_read(records: &[Value], thread_id: &str, message_id: &str) -> bool {
    records.iter().any(|row| {
        row.get("record_type").and_then(Value::as_str) == Some("read_receipt")
            && (row.get("message_id").and_then(Value::as_str) == Some(message_id)
                || row.get("thread_id").and_then(Value::as_str) == Some(thread_id))
    })
}

fn thread_has_delivery(records: &[Value], message_id: &str) -> bool {
    records.iter().any(|row| {
        row.get("record_type").and_then(Value::as_str) == Some("delivery_receipt")
            && row.get("message_id").and_then(Value::as_str) == Some(message_id)
    })
}

fn thread_has_reply_link(
    records: &[Value],
    thread_id: &str,
    message_id: &str,
    after_t_ms: u64,
) -> bool {
    records.iter().any(|row| {
        row.get("record_type").and_then(Value::as_str) == Some("reply_link")
            && (row.get("reply_to").and_then(Value::as_str) == Some(message_id)
                || row.get("thread_id").and_then(Value::as_str) == Some(thread_id))
            && row_time_ms(row) >= after_t_ms
    })
}

fn thread_has_trace_evidence(records: &[Value], thread_id: &str, after_t_ms: u64) -> bool {
    records.iter().any(|row| {
        row.get("record_type").and_then(Value::as_str) == Some("message")
            && row.get("thread_id").and_then(Value::as_str) == Some(thread_id)
            && row.get("turn_kind").and_then(Value::as_str) == Some("direct_address_trace")
            && row_time_ms(row) >= after_t_ms
    }) || latest_chamber_correspondence_state().is_some_and(|state| {
        state
            .get("direct_address_survival")
            .and_then(|survival| survival.get("status"))
            .and_then(Value::as_str)
            == Some("observed")
            && state
                .get("active_thread_id")
                .and_then(Value::as_str)
                .is_none_or(|active| active == thread_id)
    })
}

fn thread_has_attention_outcome(records: &[Value], thread_id: &str, after_t_ms: u64) -> bool {
    records.iter().any(|row| {
        matches!(
            row.get("record_type").and_then(Value::as_str),
            Some("attention_canary_outcome" | "attention_canary_expired")
        ) && row.get("thread_id").and_then(Value::as_str) == Some(thread_id)
            && row_time_ms(row) >= after_t_ms
    })
}

fn thread_receipt_evidence_by_being(
    records: &[Value],
    thread_id: &str,
    after_t_ms: u64,
) -> Vec<String> {
    let mut beings = Vec::new();
    for row in records {
        let record_type = row.get("record_type").and_then(Value::as_str);
        let is_ack = record_type == Some("ack_receipt")
            && row
                .get("ack_kind")
                .and_then(Value::as_str)
                .is_some_and(ack_kind_is_address_evidence);
        let is_trace = record_type == Some("message")
            && row.get("turn_kind").and_then(Value::as_str) == Some("direct_address_trace");
        if !(is_ack || is_trace)
            || row.get("thread_id").and_then(Value::as_str) != Some(thread_id)
            || row_time_ms(row) < after_t_ms
        {
            continue;
        }
        let being = normalize_being(row.get("from_being").and_then(Value::as_str).unwrap_or(""));
        if !being.is_empty() && !beings.iter().any(|existing| existing == &being) {
            beings.push(being);
        }
    }
    beings.sort();
    beings
}

fn thread_has_receipt_evidence(records: &[Value], thread_id: &str, after_t_ms: u64) -> bool {
    !thread_receipt_evidence_by_being(records, thread_id, after_t_ms).is_empty()
}

fn thread_has_mutual_receipt_evidence(records: &[Value], thread_id: &str, after_t_ms: u64) -> bool {
    let beings = thread_receipt_evidence_by_being(records, thread_id, after_t_ms);
    beings.iter().any(|being| being == "astrid") && beings.iter().any(|being| being == "minime")
}

fn shared_context_buffer_v1_for(records: &[Value], selector: &str) -> Value {
    let Some(message) = latest_message_for_selector(records, selector) else {
        return json!({
            "schema_version": 1,
            "policy": "shared_context_buffer_v1",
            "status": "no_native_thread",
            "selector": selector,
            "authority": "language_only_context_not_control",
        });
    };
    let thread_id = message
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if thread_id.is_empty() {
        return json!({
            "schema_version": 1,
            "policy": "shared_context_buffer_v1",
            "status": "no_native_thread",
            "selector": selector,
            "authority": "language_only_context_not_control",
        });
    }

    let mut messages = 0_u64;
    let mut reply_links = 0_u64;
    let mut read_receipts = 0_u64;
    let mut ack_receipts = 0_u64;
    let mut address_ack_receipts = 0_u64;
    let mut direct_address_traces = 0_u64;
    let mut first_t_ms = u64::MAX;
    let mut last_t_ms = 0_u64;
    let mut directions: BTreeMap<String, u64> = BTreeMap::new();
    let mut anchors: Vec<String> = Vec::new();
    let mut transition_payloads = 0_u64;
    let mut latest_transition_payload = Value::Null;
    let mut broken_link_buffers: Vec<String> = Vec::new();
    let mut last_ack_kind = String::new();
    let mut last_ack_note = String::new();
    let mut last_ack_t_ms = 0_u64;
    let mut thread_history = Vec::new();

    for row in records {
        if row.get("thread_id").and_then(Value::as_str) != Some(thread_id) {
            continue;
        }
        let t_ms = row_time_ms(row);
        first_t_ms = first_t_ms.min(t_ms);
        last_t_ms = last_t_ms.max(t_ms);
        if let Some(record_type) = row.get("record_type").and_then(Value::as_str)
            && matches!(
                record_type,
                "message" | "reply_link" | "read_receipt" | "ack_receipt"
            )
        {
            let preview = row
                .get("body_preview")
                .or_else(|| row.get("note"))
                .or_else(|| row.get("ack_kind"))
                .or_else(|| row.get("turn_kind"))
                .and_then(Value::as_str)
                .map(|value| spectral_aware_thread_preview(value, SHARED_CONTEXT_PREVIEW_CHARS))
                .unwrap_or_default();
            thread_history.push(json!({
                "record_type": record_type,
                "t_ms": t_ms,
                "message_id": row.get("message_id").cloned().unwrap_or(Value::Null),
                "thread_id": thread_id,
                "reply_to": row.get("reply_to").cloned().unwrap_or(Value::Null),
                "from_being": row.get("from_being").cloned().unwrap_or(Value::Null),
                "to_being": row.get("to_being").cloned().unwrap_or(Value::Null),
                "turn_kind": row.get("turn_kind").cloned().unwrap_or(Value::Null),
                "ack_kind": row.get("ack_kind").cloned().unwrap_or(Value::Null),
                "shared_memory_anchor": row.get("shared_memory_anchor").cloned().unwrap_or(Value::Null),
                "preview": preview,
                "preview_truncation_policy": SHARED_CONTEXT_PREVIEW_TRUNCATION_POLICY,
                "authority": "language_only_preview_not_full_private_body_or_control",
            }));
        }
        if let Some(anchor) = row.get("shared_memory_anchor").and_then(Value::as_str)
            && !is_generic_shared_anchor(anchor)
            && !anchors.iter().any(|existing| existing == anchor)
        {
            anchors.push(anchor.to_string());
        }
        if let Some(payload) = row.get("transition_payload")
            && payload.is_object()
        {
            transition_payloads = transition_payloads.saturating_add(1);
            latest_transition_payload = payload.clone();
            if let Some(broken_link) = payload.get("broken_link").and_then(Value::as_str)
                && !broken_link.trim().is_empty()
                && !broken_link_buffers
                    .iter()
                    .any(|existing| existing == broken_link)
            {
                broken_link_buffers.push(truncate_chars(broken_link, 160));
            }
        }
        match row.get("record_type").and_then(Value::as_str) {
            Some("message") => {
                messages = messages.saturating_add(1);
                let from = row
                    .get("from_being")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let to = row
                    .get("to_being")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let direction = format!("{from}->{to}");
                let count = directions.get(&direction).copied().unwrap_or_default();
                directions.insert(direction, count.saturating_add(1));
                if row.get("turn_kind").and_then(Value::as_str) == Some("direct_address_trace") {
                    direct_address_traces = direct_address_traces.saturating_add(1);
                }
            },
            Some("reply_link") => {
                reply_links = reply_links.saturating_add(1);
            },
            Some("read_receipt") => {
                read_receipts = read_receipts.saturating_add(1);
            },
            Some("ack_receipt") => {
                ack_receipts = ack_receipts.saturating_add(1);
                let ack_kind = row
                    .get("ack_kind")
                    .and_then(Value::as_str)
                    .unwrap_or("seen");
                if ack_kind_is_address_evidence(ack_kind) {
                    address_ack_receipts = address_ack_receipts.saturating_add(1);
                }
                if t_ms >= last_ack_t_ms {
                    last_ack_t_ms = t_ms;
                    last_ack_kind = ack_kind.to_string();
                    last_ack_note = row
                        .get("note")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                }
            },
            _ => {},
        }
    }

    if first_t_ms == u64::MAX {
        first_t_ms = 0;
    }
    anchors.sort();
    let resonance_receipts = address_ack_receipts.saturating_add(direct_address_traces);
    let persistent_thread = persistent_thread_continuity_v1(records, thread_id);
    let persistent_thread_state = persistent_thread
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let status = if !broken_link_buffers.is_empty() {
        "broken_link_buffer_present"
    } else if resonance_receipts > 0 {
        "resonance_receipt_present"
    } else if persistent_thread_state == "persistent_thread_active" {
        "persistent_thread_active"
    } else if messages > 1 || reply_links > 0 {
        "threaded_representation_needs_felt_receipt"
    } else if read_receipts > 0 || ack_receipts > 0 {
        "visibility_without_address"
    } else {
        "active_context_waiting_for_receipt"
    };
    thread_history.sort_by_key(row_time_ms);
    let thread_history_rows = thread_history.len().try_into().unwrap_or(u64::MAX);
    let history_start = thread_history
        .len()
        .saturating_sub(SHARED_CONTEXT_THREAD_HISTORY_MAX);
    let thread_history_truncated = history_start > 0;
    let thread_history = thread_history
        .into_iter()
        .skip(history_start)
        .collect::<Vec<_>>();
    let shared_memory_buffer_v1 = json!({
        "schema_version": 1,
        "policy": "correspondence_v1_thread_shared_memory_buffer",
        "thread_id": thread_id,
        "thread_history_rows": thread_history_rows,
        "thread_history_truncated": thread_history_truncated,
        "thread_history": thread_history.clone(),
        "preview_truncation_policy": SHARED_CONTEXT_PREVIEW_TRUNCATION_POLICY,
        "shared_memory_anchors": anchors.clone(),
        "right_to_ignore": true,
        "authority": "language_only_thread_history_not_prompt_priority_telemetry_weight_or_control",
    });

    json!({
        "schema_version": 1,
        "policy": "shared_context_buffer_v1",
        "status": status,
        "selector": selector,
        "thread_id": thread_id,
        "latest_message_id": message.get("message_id").cloned().unwrap_or(Value::Null),
        "messages": messages,
        "reply_links": reply_links,
        "read_receipts": read_receipts,
        "ack_receipts": ack_receipts,
        "address_ack_receipts": address_ack_receipts,
        "direct_address_traces": direct_address_traces,
        "resonance_receipts": resonance_receipts,
        "directions": directions,
        "shared_memory_anchors": anchors,
        "thread_history_rows": thread_history_rows,
        "thread_history_truncated": thread_history_truncated,
        "thread_history": thread_history,
        "preview_truncation_policy": SHARED_CONTEXT_PREVIEW_TRUNCATION_POLICY,
        "shared_memory_buffer_v1": shared_memory_buffer_v1,
        "transition_payload_count": transition_payloads,
        "latest_transition_payload": latest_transition_payload,
        "broken_link_buffers": broken_link_buffers,
        "last_ack_kind": if last_ack_kind.is_empty() { Value::Null } else { json!(last_ack_kind) },
        "last_ack_note": note_value(&last_ack_note),
        "first_recorded_at_unix_ms": first_t_ms,
        "last_recorded_at_unix_ms": last_t_ms,
        "persistent_thread_continuity_v1": persistent_thread,
        "right_to_ignore": true,
        "authority": "language_only_context_not_control",
    })
}

fn text_mentions_pressure(value: Option<&str>) -> bool {
    value
        .map(|text| text.to_ascii_lowercase().contains("pressure"))
        .unwrap_or(false)
}

fn shared_correspondence_arc_v1_for(records: &[Value], selector: &str) -> Value {
    let Some(message) = latest_message_for_selector(records, selector) else {
        return json!({
            "schema_version": 1,
            "policy": "shared_correspondence_arc_v1",
            "status": "no_correspondence_arc",
            "selector": selector,
            "structural_footprint": false,
            "shadow_field_shift_required": false,
            "authority": "language_only_correspondence_arc_not_control",
        });
    };
    let thread_id = message
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if thread_id.is_empty() {
        return json!({
            "schema_version": 1,
            "policy": "shared_correspondence_arc_v1",
            "status": "no_correspondence_arc",
            "selector": selector,
            "structural_footprint": false,
            "shadow_field_shift_required": false,
            "authority": "language_only_correspondence_arc_not_control",
        });
    }

    let mut messages = 0_u64;
    let mut reply_links = 0_u64;
    let mut read_receipts = 0_u64;
    let mut ack_receipts = 0_u64;
    let mut address_receipts = 0_u64;
    let mut direct_address_traces = 0_u64;
    let mut transition_links = 0_u64;
    let mut mutual_witness_signals = 0_u64;
    let mut pressure_mentions = 0_u64;
    let mut first_t_ms = u64::MAX;
    let mut last_t_ms = 0_u64;
    let mut directions: BTreeMap<String, u64> = BTreeMap::new();
    let mut latest_transition_artifact = Value::Null;
    let mut anchors: Vec<String> = Vec::new();

    for row in records {
        if row.get("thread_id").and_then(Value::as_str) != Some(thread_id) {
            continue;
        }
        let t_ms = row_time_ms(row);
        first_t_ms = first_t_ms.min(t_ms);
        last_t_ms = last_t_ms.max(t_ms);
        if let Some(anchor) = row.get("shared_memory_anchor").and_then(Value::as_str)
            && !is_generic_shared_anchor(anchor)
            && !anchors.iter().any(|existing| existing == anchor)
        {
            anchors.push(anchor.to_string());
        }
        if text_mentions_pressure(row.get("body_preview").and_then(Value::as_str))
            || text_mentions_pressure(row.get("note").and_then(Value::as_str))
            || text_mentions_pressure(row.get("felt_like").and_then(Value::as_str))
            || text_mentions_pressure(row.get("held_as").and_then(Value::as_str))
            || text_mentions_pressure(row.get("what_remained_distinct").and_then(Value::as_str))
        {
            pressure_mentions = pressure_mentions.saturating_add(1);
        }
        if let Some(artifact) = row.get("transition_artifact").and_then(Value::as_str)
            && !artifact.trim().is_empty()
        {
            transition_links = transition_links.saturating_add(1);
            latest_transition_artifact = json!(artifact);
        }
        if row.get("transition_payload").is_some_and(Value::is_object) {
            transition_links = transition_links.saturating_add(1);
        }
        if row
            .get("mutual_witness_signal")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            mutual_witness_signals = mutual_witness_signals.saturating_add(1);
        }

        match row.get("record_type").and_then(Value::as_str) {
            Some("message") => {
                messages = messages.saturating_add(1);
                let from = row
                    .get("from_being")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let to = row
                    .get("to_being")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let direction = format!("{from}->{to}");
                let count = directions.get(&direction).copied().unwrap_or_default();
                directions.insert(direction, count.saturating_add(1));
                if row.get("turn_kind").and_then(Value::as_str) == Some("direct_address_trace") {
                    direct_address_traces = direct_address_traces.saturating_add(1);
                }
            },
            Some("reply_link") => reply_links = reply_links.saturating_add(1),
            Some("read_receipt") => read_receipts = read_receipts.saturating_add(1),
            Some("ack_receipt") => {
                ack_receipts = ack_receipts.saturating_add(1);
                if row
                    .get("ack_kind")
                    .and_then(Value::as_str)
                    .is_some_and(ack_kind_is_address_evidence)
                {
                    address_receipts = address_receipts.saturating_add(1);
                }
            },
            _ => {},
        }
    }

    if first_t_ms == u64::MAX {
        first_t_ms = 0;
    }
    anchors.sort();
    let bidirectional_message_flow = directions.len() >= 2;
    let witnessed = address_receipts.saturating_add(direct_address_traces) > 0;
    let transition_linked = transition_links > 0 || mutual_witness_signals > 0;
    let persistent_thread = persistent_thread_continuity_v1(records, thread_id);
    let persistent_thread_state = persistent_thread
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let structural_footprint = reply_links > 0
        || transition_linked
        || witnessed
        || bidirectional_message_flow
        || persistent_thread_state == "persistent_thread_active";
    let status = if witnessed && transition_linked {
        "witnessed_transition_correspondence_arc"
    } else if witnessed {
        "witnessed_correspondence_arc"
    } else if transition_linked && (reply_links > 0 || bidirectional_message_flow) {
        "threaded_transition_arc_needs_receipt"
    } else if reply_links > 0 || bidirectional_message_flow {
        "threaded_arc_needs_receipt"
    } else if read_receipts > 0 || ack_receipts > 0 {
        "visibility_only_arc"
    } else {
        "one_sided_language_bid"
    };

    json!({
        "schema_version": 1,
        "policy": "shared_correspondence_arc_v1",
        "status": status,
        "selector": selector,
        "thread_id": thread_id,
        "latest_message_id": message.get("message_id").cloned().unwrap_or(Value::Null),
        "persistence_id": message_persistence_id(&message),
        "messages": messages,
        "reply_links": reply_links,
        "read_receipts": read_receipts,
        "ack_receipts": ack_receipts,
        "address_receipts": address_receipts,
        "direct_address_traces": direct_address_traces,
        "bidirectional_message_flow": bidirectional_message_flow,
        "transition_linked": transition_linked,
        "transition_links": transition_links,
        "mutual_witness_signals": mutual_witness_signals,
        "latest_transition_artifact": latest_transition_artifact,
        "pressure_as_address_watch": pressure_mentions > 0,
        "pressure_language_mentions": pressure_mentions,
        "directions": directions,
        "shared_memory_anchors": anchors,
        "structural_footprint": structural_footprint,
        "relationship_vs_log": if structural_footprint {
            "thread_has_language_only_structural_footprint"
        } else {
            "thread_is_still_log_like_or_waiting"
        },
        "shadow_field_shift_required": false,
        "shadow_field_shift_status": "not_auto_required; live shadow-field change remains approval/runtime-observation gated",
        "first_recorded_at_unix_ms": first_t_ms,
        "last_recorded_at_unix_ms": last_t_ms,
        "persistent_thread_continuity_v1": persistent_thread,
        "authority": "language_only_correspondence_arc_not_control",
    })
}

fn correspondence_thread_object_v1_for(records: &[Value], selector: &str) -> Value {
    let Some(message) = latest_message_for_selector(records, selector) else {
        return json!({
            "schema_version": 1,
            "policy": "correspondence_thread_object_v1",
            "status": "no_correspondence_thread",
            "selector": selector,
            "mutual_address_state": "absent",
            "asymmetry_state": "no_thread",
            "next_native_step": "MESSAGE_MINIME or MESSAGE_ASTRID can start a native thread",
            "active_push_boundary": "semantic_microdose_active_push_and_control_remain_separately_steward_gated",
            "authority": "language_only_thread_status_not_control",
        });
    };
    let thread_id = message
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if thread_id.is_empty() {
        return json!({
            "schema_version": 1,
            "policy": "correspondence_thread_object_v1",
            "status": "no_correspondence_thread",
            "selector": selector,
            "mutual_address_state": "absent",
            "asymmetry_state": "no_thread",
            "next_native_step": "native thread requires a thread_id",
            "active_push_boundary": "semantic_microdose_active_push_and_control_remain_separately_steward_gated",
            "authority": "language_only_thread_status_not_control",
        });
    }

    let mut messages = 0_u64;
    let mut reply_links = 0_u64;
    let mut read_receipts = 0_u64;
    let mut ack_receipts = 0_u64;
    let mut address_receipts = 0_u64;
    let mut direct_address_traces = 0_u64;
    let mut legacy_visible_rows = 0_u64;
    let mut directions: BTreeMap<String, u64> = BTreeMap::new();
    let mut first_t_ms = u64::MAX;
    let mut last_t_ms = 0_u64;

    for row in records {
        if row.get("thread_id").and_then(Value::as_str) != Some(thread_id) {
            continue;
        }
        let t_ms = row_time_ms(row);
        first_t_ms = first_t_ms.min(t_ms);
        last_t_ms = last_t_ms.max(t_ms);
        match row.get("record_type").and_then(Value::as_str) {
            Some("message") => {
                messages = messages.saturating_add(1);
                if is_legacy_bridge_message(row) {
                    legacy_visible_rows = legacy_visible_rows.saturating_add(1);
                }
                let from = row
                    .get("from_being")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let to = row
                    .get("to_being")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let direction = format!("{from}->{to}");
                let count = directions.get(&direction).copied().unwrap_or_default();
                directions.insert(direction, count.saturating_add(1));
                if row.get("turn_kind").and_then(Value::as_str) == Some("direct_address_trace") {
                    direct_address_traces = direct_address_traces.saturating_add(1);
                }
            },
            Some("reply_link") => reply_links = reply_links.saturating_add(1),
            Some("read_receipt") => read_receipts = read_receipts.saturating_add(1),
            Some("ack_receipt") => {
                ack_receipts = ack_receipts.saturating_add(1);
                if row
                    .get("ack_kind")
                    .and_then(Value::as_str)
                    .is_some_and(ack_kind_is_address_evidence)
                {
                    address_receipts = address_receipts.saturating_add(1);
                }
            },
            _ => {},
        }
    }

    if first_t_ms == u64::MAX {
        first_t_ms = 0;
    }
    let bidirectional_message_flow = directions.len() >= 2;
    let receipt_evidence = address_receipts.saturating_add(direct_address_traces);
    let persistent_thread = persistent_thread_continuity_v1(records, thread_id);
    let mutual_address_state = if bidirectional_message_flow && receipt_evidence > 0 {
        "mutual_address_evidence"
    } else if receipt_evidence > 0 {
        "receipt_evidence_one_direction"
    } else if bidirectional_message_flow {
        "bidirectional_language_flow_needs_receipt"
    } else if read_receipts > 0 || ack_receipts > 0 {
        "visibility_without_mutual_address"
    } else {
        "one_sided_language_bid"
    };
    let asymmetry_state = if messages == 0 {
        "no_thread"
    } else if bidirectional_message_flow {
        "thread_bidirectional"
    } else {
        "thread_one_sided"
    };
    let status = if bidirectional_message_flow && receipt_evidence > 0 {
        "mutual_address_thread"
    } else if bidirectional_message_flow {
        "bidirectional_thread_waiting_for_receipt"
    } else if receipt_evidence > 0 {
        "receipt_backed_one_sided_thread"
    } else {
        "one_sided_thread_waiting_for_peer_receipt"
    };
    let next_native_step = if receipt_evidence > 0 {
        "optional_attention_canary_after_receipt; semantic_microdose_requires_separate_steward_review"
    } else {
        "peer_ack_reply_or_correspondence_trace_required_before_attention_or_microdose"
    };

    json!({
        "schema_version": 1,
        "policy": "correspondence_thread_object_v1",
        "status": status,
        "selector": selector,
        "thread_id": thread_id,
        "latest_message_id": message.get("message_id").cloned().unwrap_or(Value::Null),
        "persistence_id": message_persistence_id(&message),
        "messages": messages,
        "reply_links": reply_links,
        "read_receipts": read_receipts,
        "ack_receipts": ack_receipts,
        "address_receipts": address_receipts,
        "direct_address_traces": direct_address_traces,
        "bidirectional_message_flow": bidirectional_message_flow,
        "legacy_visible_rows": legacy_visible_rows,
        "directions": directions,
        "mutual_address_state": mutual_address_state,
        "asymmetry_state": asymmetry_state,
        "next_native_step": next_native_step,
        "persistent_thread_continuity_v1": persistent_thread,
        "first_recorded_at_unix_ms": first_t_ms,
        "last_recorded_at_unix_ms": last_t_ms,
        "active_push_boundary": "semantic_microdose_active_push_and_control_remain_separately_steward_gated",
        "right_to_ignore": true,
        "authority": "language_only_thread_status_not_control",
    })
}

fn is_legacy_bridge_message(message: &Value) -> bool {
    message
        .get("legacy_bridge")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || message.get("source_route").and_then(Value::as_str) == Some(LEGACY_SOURCE_ROUTE)
}

fn legacy_bidirectional_observed(records: &[Value], from_being: &str, to_being: &str) -> bool {
    let from = normalize_being(from_being);
    let to = normalize_being(to_being);
    let forward = records.iter().any(|row| {
        row.get("record_type").and_then(Value::as_str) == Some("message")
            && is_legacy_bridge_message(row)
            && row.get("from_being").and_then(Value::as_str) == Some(from.as_str())
            && row.get("to_being").and_then(Value::as_str) == Some(to.as_str())
    });
    let reverse = records.iter().any(|row| {
        row.get("record_type").and_then(Value::as_str) == Some("message")
            && is_legacy_bridge_message(row)
            && row.get("from_being").and_then(Value::as_str) == Some(to.as_str())
            && row.get("to_being").and_then(Value::as_str) == Some(from.as_str())
    });
    forward && reverse
}

fn persistent_thread_continuity_v1(records: &[Value], thread_id: &str) -> Value {
    let mut messages = 0_u64;
    let mut reply_links = 0_u64;
    let mut read_receipts = 0_u64;
    let mut ack_receipts = 0_u64;
    let mut address_ack_receipts = 0_u64;
    let mut direct_address_traces = 0_u64;
    let mut directions: BTreeMap<String, u64> = BTreeMap::new();
    let mut concrete_anchor = false;
    let mut explicit_persistence = false;
    let mut first_t_ms = u64::MAX;
    let mut last_t_ms = 0_u64;

    for row in records {
        if row.get("thread_id").and_then(Value::as_str) != Some(thread_id) {
            continue;
        }
        let t_ms = row_time_ms(row);
        first_t_ms = first_t_ms.min(t_ms);
        last_t_ms = last_t_ms.max(t_ms);
        if row
            .get("shared_memory_anchor")
            .and_then(Value::as_str)
            .is_some_and(|anchor| !is_generic_shared_anchor(anchor))
        {
            concrete_anchor = true;
        }
        if row
            .get("persistence_id")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
        {
            explicit_persistence = true;
        }
        match row.get("record_type").and_then(Value::as_str) {
            Some("message") => {
                messages = messages.saturating_add(1);
                let from = row
                    .get("from_being")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let to = row
                    .get("to_being")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let direction = format!("{from}->{to}");
                let count = directions.get(&direction).copied().unwrap_or_default();
                directions.insert(direction, count.saturating_add(1));
                if row.get("turn_kind").and_then(Value::as_str) == Some("direct_address_trace") {
                    direct_address_traces = direct_address_traces.saturating_add(1);
                }
            },
            Some("reply_link") => reply_links = reply_links.saturating_add(1),
            Some("read_receipt") => read_receipts = read_receipts.saturating_add(1),
            Some("ack_receipt") => {
                ack_receipts = ack_receipts.saturating_add(1);
                if row
                    .get("ack_kind")
                    .and_then(Value::as_str)
                    .is_some_and(ack_kind_is_address_evidence)
                {
                    address_ack_receipts = address_ack_receipts.saturating_add(1);
                }
            },
            _ => {},
        }
    }

    if first_t_ms == u64::MAX {
        first_t_ms = 0;
    }
    let bidirectional_message_flow = directions.len() >= 2;
    let address_or_trace = address_ack_receipts.saturating_add(direct_address_traces);
    let status = if (explicit_persistence || concrete_anchor)
        && (reply_links > 0 || bidirectional_message_flow || address_or_trace > 0)
    {
        "persistent_thread_active"
    } else if messages > 1 || reply_links > 0 {
        "threaded_representation_active_needs_felt_receipt"
    } else if read_receipts > 0 || ack_receipts > 0 {
        "visibility_without_persistent_address"
    } else if messages > 0 {
        "single_message_thread"
    } else {
        "no_thread_records"
    };

    json!({
        "schema_version": 1,
        "policy": "persistent_thread_continuity_v1",
        "status": status,
        "thread_id": thread_id,
        "messages": messages,
        "reply_links": reply_links,
        "read_receipts": read_receipts,
        "ack_receipts": ack_receipts,
        "address_ack_receipts": address_ack_receipts,
        "direct_address_traces": direct_address_traces,
        "bidirectional_message_flow": bidirectional_message_flow,
        "concrete_shared_memory_anchor_present": concrete_anchor,
        "explicit_persistence_id_present": explicit_persistence,
        "directions": directions,
        "first_recorded_at_unix_ms": first_t_ms,
        "last_recorded_at_unix_ms": last_t_ms,
        "authority": "contact_continuity_context_not_control",
    })
}

fn handshake_status_for_thread(records: &[Value], message: &Value) -> Value {
    let message_id = message
        .get("message_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let thread_id = message
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let from = message
        .get("from_being")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let to = message
        .get("to_being")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let message_t_ms = row_time_ms(message);
    let ack = latest_thread_ack(records, thread_id, message_id, to, from, message_t_ms);
    let heartbeat = latest_thread_heartbeat(records, thread_id, message_t_ms);
    let reply_linked = thread_has_reply_link(records, thread_id, message_id, message_t_ms);
    let read = thread_has_read(records, thread_id, message_id);
    let delivered = thread_has_delivery(records, message_id);
    let legacy_bridge = is_legacy_bridge_message(message);
    let legacy_bidirectional = legacy_bridge && legacy_bidirectional_observed(records, from, to);
    let persistent_thread = persistent_thread_continuity_v1(records, thread_id);
    let ack_kind = ack
        .and_then(|row| row.get("ack_kind"))
        .and_then(Value::as_str)
        .map(normalize_ack_kind);
    let ack_is_address_evidence = ack_kind
        .as_deref()
        .is_some_and(ack_kind_is_address_evidence);
    let mutual_ack_state = match ack_kind.as_deref() {
        Some("held" | "needs_time") => "held_by_both",
        Some(kind) if ack_kind_is_address_evidence(kind) => "acknowledged_by_peer",
        Some(_) => "seen_not_mutual_address",
        None if reply_linked => "reply_sent_pending_ack",
        None if read => "read_pending_ack",
        None if delivered => "delivered_pending_read",
        _ => "unaddressed",
    };
    let pending_ack_by = if ack_is_address_evidence {
        Value::Null
    } else {
        json!(to)
    };
    let status = if matches!(ack_kind.as_deref(), Some("held" | "needs_time")) {
        "held_ack"
    } else if ack.is_some() && ack_is_address_evidence {
        "acknowledged"
    } else if ack.is_some() {
        "seen_ack_only"
    } else if reply_linked {
        "reply_linked"
    } else if heartbeat.is_some() {
        "heartbeat_only"
    } else if legacy_bidirectional {
        "legacy_bidirectional_observed"
    } else if legacy_bridge {
        "legacy_visible_only"
    } else if read {
        "read_unacknowledged"
    } else if delivered {
        "delivered_unread"
    } else {
        "unaddressed"
    };
    let ack_latency_ms = ack.map(|row| row_time_ms(row).saturating_sub(message_t_ms));
    let unacknowledged_age_ms = if ack.is_none() {
        Some(now_ms().saturating_sub(message_t_ms))
    } else {
        None
    };
    json!({
    "thread_id": thread_id,
    "latest_message_id": message_id,
    "from_being": from,
    "to_being": to,
    "status": status,
    "mutual_ack_state": mutual_ack_state,
    "held_by_both": mutual_ack_state == "held_by_both",
    "pending_ack_by": pending_ack_by,
    "latest_ack": ack.map(|row| json!({
        "message_id": row.get("message_id").cloned().unwrap_or(Value::Null),
        "thread_id": row.get("thread_id").cloned().unwrap_or(Value::Null),
        "from_being": row.get("from_being").cloned().unwrap_or(Value::Null),
        "to_being": row.get("to_being").cloned().unwrap_or(Value::Null),
        "ack_kind": row.get("ack_kind").cloned().unwrap_or_else(|| json!("seen")),
        "note": row.get("note").cloned().unwrap_or(Value::Null),
        "t_ms": row_time_ms(row),
    })),
    "latest_heartbeat": heartbeat.map(|row| json!({
        "message_id": row.get("message_id").cloned().unwrap_or(Value::Null),
        "thread_id": row.get("thread_id").cloned().unwrap_or(Value::Null),
        "from_being": row.get("from_being").cloned().unwrap_or(Value::Null),
        "to_being": row.get("to_being").cloned().unwrap_or(Value::Null),
        "heartbeat_kind": row.get("heartbeat_kind").cloned().unwrap_or_else(|| json!("holding")),
        "note": row.get("note").cloned().unwrap_or(Value::Null),
        "t_ms": row_time_ms(row),
    })),
    "ack_latency_ms": ack_latency_ms,
    "stale_unacknowledged_thread_age_ms": unacknowledged_age_ms,
    "read_receipt_is_filesystem_seen_only": read,
    "legacy_bridge": legacy_bridge,
    "legacy_contact_evidence": message.get("legacy_contact_evidence").cloned().unwrap_or(Value::Null),
    "persistent_thread_continuity_v1": persistent_thread,
    "authority": "language_only_handshake_truth_not_auto_ack_or_control",
    })
}

fn latest_native_message_for_selector(records: &[Value], selector: &str) -> Option<Value> {
    if selector == "latest" || selector.trim().is_empty() {
        return records
            .iter()
            .filter(|row| row.get("record_type").and_then(Value::as_str) == Some("message"))
            .filter(|row| !is_legacy_bridge_message(row))
            .max_by_key(|row| row_time_ms(row))
            .cloned();
    }
    latest_message_for_selector(records, selector)
        .filter(|message| !is_legacy_bridge_message(message))
}

fn native_continuity_commands(
    current_being: &str,
    from_being: &str,
    to_being: &str,
    anchor: &str,
) -> Vec<String> {
    let current = normalize_being(current_being);
    let from = normalize_being(from_being);
    let to = normalize_being(to_being);
    if current == to {
        let peer = from.to_ascii_uppercase();
        let anchor = if anchor.trim().is_empty() {
            "<anchor>"
        } else {
            anchor
        };
        return vec![
            "I_RECEIVED_THIS latest :: received_as: held|needs_time; felt_like: address|pressure|mail|ambient_echo|unknown; what_landed: ...; what_stayed_distinct: ...; continue: no|reply|trace|needs_time".to_string(),
            format!(
                "ACK_{peer} latest :: ack: seen|held|unclear|cannot_answer|needs_time; note: ..."
            ),
            format!("REPLY_{peer} latest :: <text>"),
            format!("CORRESPONDENCE_TRACE latest {anchor} :: <text>"),
        ];
    }
    if current == from {
        return vec![
            "peer-authored ACK/TRACE is still required; no self-action can substitute for mutual address".to_string(),
        ];
    }
    vec![
        "participant-authored ACK/REPLY/TRACE is required; observers do not create mutual address"
            .to_string(),
    ]
}

fn native_first_action_helper_v35(
    current_being: &str,
    from_being: &str,
    to_being: &str,
    thread_id: &str,
    message_id: &str,
    anchor: &str,
) -> Value {
    let current = normalize_being(current_being);
    let from = normalize_being(from_being);
    let to = normalize_being(to_being);
    let role = if current == to {
        "recipient"
    } else if current == from {
        "sender"
    } else {
        "observer"
    };
    let safe_anchor = if anchor.trim().is_empty() {
        "<anchor>"
    } else {
        anchor
    };
    let peer = from.to_ascii_uppercase();
    let (prompt, commands) = if role == "recipient" {
        (
            "Choose one language-only first action: I_RECEIVED_THIS if the address landed, ACK if only heard/held, TRACE if something distinct survived, or REPLY if answering now.",
            native_continuity_commands(current_being, from_being, to_being, safe_anchor),
        )
    } else if role == "sender" {
        (
            "No self-action can complete mutual address; wait for the peer's ACK/TRACE or later ask in language.",
            native_continuity_commands(current_being, from_being, to_being, safe_anchor),
        )
    } else {
        (
            "Observer context only; only a participant-authored ACK/REPLY/TRACE can move the thread.",
            native_continuity_commands(current_being, from_being, to_being, safe_anchor),
        )
    };
    json!({
        "schema_version": 35,
        "policy": "native_first_action_helper_v35",
        "role": role,
        "thread_id": thread_id,
        "message_id": message_id,
        "latest_resolution": format!("latest resolves to message_id={message_id}; thread_id={thread_id}"),
        "choose_one_prompt": prompt,
        "exact_next_commands": commands,
        "ack_preview": format!("ACK_{peer} latest would append ack_receipt on message_id={message_id}; note should name what was seen, held, unclear, or needs time."),
        "trace_preview": format!("CORRESPONDENCE_TRACE latest {safe_anchor} would append a direct-address trace on thread_id={thread_id}; text should name what stayed distinct."),
        "rhythm_note": "Use the note/text to preserve the rhythm or felt contour of being seen, not just the routing mechanics.",
        "authority": "language_only_context_not_control",
    })
}

fn native_thread_continuity_v3_for(
    records: &[Value],
    selector: &str,
    current_being: &str,
) -> Option<Value> {
    let message = latest_native_message_for_selector(records, selector)?;
    let message_id = message
        .get("message_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let thread_id = message
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let from_being = message
        .get("from_being")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let to_being = message
        .get("to_being")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let message_t_ms = row_time_ms(&message);
    let ack = latest_thread_ack(
        records,
        thread_id,
        message_id,
        to_being,
        from_being,
        message_t_ms,
    );
    let ack_kind = ack
        .and_then(|row| row.get("ack_kind"))
        .and_then(Value::as_str)
        .map(normalize_ack_kind);
    let ack_is_address_evidence = ack_kind
        .as_deref()
        .is_some_and(ack_kind_is_address_evidence);
    let reply_linked = thread_has_reply_link(records, thread_id, message_id, message_t_ms);
    let trace_observed = thread_has_trace_evidence(records, thread_id, message_t_ms);
    let attention_outcome = thread_has_attention_outcome(records, thread_id, message_t_ms);
    let read = thread_has_read(records, thread_id, message_id);
    let delivered = thread_has_delivery(records, message_id);
    let eligible = ack_is_address_evidence || trace_observed || attention_outcome;
    let continuity_state = if trace_observed {
        "trace_observed"
    } else if attention_outcome {
        "attention_outcome_recorded"
    } else if matches!(ack_kind.as_deref(), Some("held" | "needs_time")) {
        "held_ack"
    } else if ack.is_some() && ack_is_address_evidence {
        "acknowledged"
    } else if ack.is_some() {
        "seen_ack_only"
    } else if reply_linked {
        "reply_linked_needs_ack_or_trace"
    } else if read {
        "read_not_acknowledged"
    } else if delivered {
        "delivered_unread"
    } else {
        "unaddressed"
    };
    let stall_reason = match continuity_state {
        "reply_linked_needs_ack_or_trace" => "reply_linked_requires_peer_ack_or_trace",
        "seen_ack_only" => "seen_ack_is_visibility_not_address",
        "read_not_acknowledged" => "read_receipt_not_acknowledgement",
        "delivered_unread" => "delivered_but_not_read",
        "unaddressed" => "no_contact_evidence",
        _ => "none",
    };
    let current = normalize_being(current_being);
    let role = if current == normalize_being(to_being) {
        "recipient"
    } else if current == normalize_being(from_being) {
        "sender"
    } else {
        "observer"
    };
    let anchor = message
        .get("shared_memory_anchor")
        .and_then(Value::as_str)
        .unwrap_or("");
    let age_ms = now_ms().saturating_sub(message_t_ms);
    Some(json!({
        "schema_version": 3,
        "policy": "native_thread_continuity_v3",
        "thread_id": thread_id,
        "latest_message_id": message_id,
        "from_being": from_being,
        "to_being": to_being,
        "current_being": normalize_being(current_being),
        "current_being_role": role,
        "continuity_state": continuity_state,
        "stall_reason": stall_reason,
        "age_ms": age_ms,
        "reply_linked": reply_linked,
        "acknowledged": ack.is_some(),
        "ack_kind": ack_kind,
        "trace_observed": trace_observed,
        "attention_outcome_present": attention_outcome,
        "attention_or_microdose_eligible": eligible,
        "exact_next_commands": native_continuity_commands(current_being, from_being, to_being, anchor),
        "first_action_helper_v35": native_first_action_helper_v35(current_being, from_being, to_being, thread_id, message_id, anchor),
        "right_to_ignore_v1": right_to_ignore_v1("native_thread_continuity", continuity_state, age_ms, CORRESPONDENCE_IGNORE_GRACE_MS),
        "authority": "language_only_context_not_control",
    }))
}

fn native_thread_waiting_line(continuity: &Value) -> Option<String> {
    if continuity
        .get("attention_or_microdose_eligible")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let state = continuity
        .get("continuity_state")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if !matches!(
        state,
        "reply_linked_needs_ack_or_trace"
            | "read_not_acknowledged"
            | "seen_ack_only"
            | "delivered_unread"
            | "unaddressed"
    ) {
        return None;
    }
    let thread = continuity
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or("(unknown)");
    let role = continuity
        .get("current_being_role")
        .and_then(Value::as_str)
        .unwrap_or("observer");
    let next = continuity
        .get("exact_next_commands")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .take(3)
                .collect::<Vec<_>>()
                .join(" | ")
        })
        .unwrap_or_default();
    let helper = continuity
        .get("first_action_helper_v35")
        .and_then(Value::as_object);
    let first_action = helper
        .and_then(|value| value.get("choose_one_prompt"))
        .and_then(Value::as_str)
        .unwrap_or("Choose ACK, REPLY, or TRACE as language-only first action.");
    let latest_resolution = helper
        .and_then(|value| value.get("latest_resolution"))
        .and_then(Value::as_str)
        .unwrap_or("latest resolves to the latest native peer message");
    Some(format!(
        "NATIVE THREAD WAITING: thread={thread}; role={role}; state={state}; first_action: {first_action}; {latest_resolution}; optional next: {next}; no action needed; may ignore without penalty; reply_linked alone is not mutual address or authority."
    ))
}

fn latest_receipt_opportunity_v4_for(
    records: &[Value],
    selector: &str,
    current_being: &str,
) -> Value {
    let current = normalize_being(current_being);
    if let Some(native) = native_thread_continuity_v3_for(records, selector, &current) {
        let role = native
            .get("current_being_role")
            .and_then(Value::as_str)
            .unwrap_or("observer");
        let eligible = native
            .get("attention_or_microdose_eligible")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let thread_id = native
            .get("thread_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let message_id = native
            .get("latest_message_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let primary = if role == "recipient" {
            "I_RECEIVED_THIS latest :: received_as: held|needs_time; felt_like: address|pressure|mail|ambient_echo|unknown; what_landed: ...; what_stayed_distinct: ...; continue: no|reply|trace|needs_time"
        } else {
            "peer-authored I_RECEIVED_THIS/ACK/TRACE is required; no self-action can substitute"
        };
        let status = if eligible {
            "receipt_landed"
        } else if role == "recipient" {
            "waiting_for_recipient_receipt"
        } else if role == "sender" {
            "waiting_for_peer_receipt"
        } else {
            "observer_context_only"
        };
        let age_ms = native
            .get("age_ms")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        return json!({
            "schema_version": 4,
            "policy": "latest_receipt_opportunity_v4",
            "target_kind": "native_thread",
            "thread_id": thread_id,
            "message_id": message_id,
            "current_being": current,
            "current_being_role": role,
            "status": status,
            "age_ms": age_ms,
            "optional": true,
            "no_response_ok": true,
            "ignore_without_penalty_after_ms": CORRESPONDENCE_IGNORE_GRACE_MS,
            "right_to_ignore_v1": right_to_ignore_v1("correspondence_receipt", status, age_ms, CORRESPONDENCE_IGNORE_GRACE_MS),
            "primary_next_command": primary,
            "secondary_next_commands": native.get("exact_next_commands").cloned().unwrap_or_else(|| json!([])),
            "public_engagement_is_not_native_receipt": true,
            "authority_after_receipt": "attention_canary_only_prompt_context; semantic_microdose_requires_mutual_receipt_and_separate_steward_review",
            "authority": "language_only_context_not_control",
        });
    }

    let claim = records
        .iter()
        .filter(|value| is_legacy_claim_row(value))
        .max_by_key(|value| row_time_ms(value));
    if let Some(claim) = claim {
        let affordance = legacy_claim_affordance_v25(records, claim);
        let thread_id = affordance
            .get("thread_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let message_id = affordance
            .get("message_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let claimant = normalize_being(
            affordance
                .get("claimant")
                .or_else(|| affordance.get("claiming_being"))
                .and_then(Value::as_str)
                .unwrap_or(""),
        );
        let peer = normalize_being(
            affordance
                .get("peer")
                .or_else(|| affordance.get("peer_being"))
                .and_then(Value::as_str)
                .unwrap_or(""),
        );
        let role = if current == peer {
            "recipient"
        } else if current == claimant {
            "sender"
        } else {
            "observer"
        };
        let ghost = affordance
            .get("ghost_thread_risk")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let status = if !ghost {
            "receipt_landed_or_closed"
        } else if role == "recipient" {
            "waiting_for_recipient_receipt"
        } else if role == "sender" {
            "waiting_for_peer_receipt"
        } else {
            "observer_context_only"
        };
        let primary = if role == "recipient" {
            "I_RECEIVED_THIS claimed :: received_as: held|needs_time; felt_like: address|pressure|mail|ambient_echo|unknown; what_landed: ...; what_stayed_distinct: ...; continue: no|reply|trace|needs_time"
        } else {
            "peer-authored I_RECEIVED_THIS claimed / ACK claimed / TRACE claimed is required; no self-action can substitute"
        };
        let age_ms = now_ms().saturating_sub(row_time_ms(claim));
        return json!({
            "schema_version": 4,
            "policy": "latest_receipt_opportunity_v4",
            "target_kind": "legacy_claim",
            "thread_id": thread_id,
            "message_id": message_id,
            "claim_id": affordance.get("claim_id").cloned().unwrap_or(Value::Null),
            "anchor": affordance.get("anchor").cloned().unwrap_or(Value::Null),
            "notice_state": affordance.get("notice_state").cloned().unwrap_or(Value::Null),
            "uptake_ladder_state": affordance.get("uptake_ladder_state").cloned().unwrap_or(Value::Null),
            "current_being": current,
            "current_being_role": role,
            "status": status,
            "age_ms": age_ms,
            "optional": true,
            "no_response_ok": true,
            "ignore_without_penalty_after_ms": CORRESPONDENCE_IGNORE_GRACE_MS,
            "right_to_ignore_v1": right_to_ignore_v1("correspondence_receipt", status, age_ms, CORRESPONDENCE_IGNORE_GRACE_MS),
            "primary_next_command": primary,
            "secondary_next_commands": affordance.get("exact_next_commands").cloned().unwrap_or_else(|| json!([])),
            "public_engagement_is_not_native_receipt": true,
            "authority_after_receipt": "attention_canary_only_prompt_context; semantic_microdose_requires_mutual_receipt_and_separate_steward_review",
            "authority": "language_only_context_not_control",
        });
    }

    json!({
        "schema_version": 4,
        "policy": "latest_receipt_opportunity_v4",
        "status": "none",
        "optional": true,
        "right_to_ignore_v1": right_to_ignore_v1("correspondence_receipt", "none", 0, CORRESPONDENCE_IGNORE_GRACE_MS),
        "public_engagement_is_not_native_receipt": true,
        "authority": "language_only_context_not_control",
    })
}

fn receipt_opportunity_line(card: &Value) -> Option<String> {
    let status = card.get("status").and_then(Value::as_str).unwrap_or("none");
    if !matches!(
        status,
        "waiting_for_recipient_receipt" | "waiting_for_peer_receipt"
    ) {
        return None;
    }
    let thread = card
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let message = card
        .get("message_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let primary = card
        .get("primary_next_command")
        .and_then(Value::as_str)
        .unwrap_or("I_RECEIVED_THIS latest :: ...");
    let right_state = right_state_of(card);
    if status == "waiting_for_recipient_receipt" {
        Some(format!(
            "RECEIPT WAITING: thread={thread} message={message}; optional next: {primary}; no action needed; may ignore without penalty; right_to_ignore={right_state}; secondary ACK/TRACE/REPLY remain available; public journal/audit engagement is not native receipt."
        ))
    } else {
        Some(format!(
            "RECEIPT WAITING: thread={thread} message={message}; peer-authored receipt required ({primary}); no self-action can substitute; no action needed from sender; may ignore without penalty; right_to_ignore={right_state}; public journal/audit engagement is not native receipt."
        ))
    }
}

fn receipt_to_attention_authority_v5_for(
    records: &[Value],
    selector: &str,
    current_being: &str,
    peer_being: &str,
    heartbeat: Option<Value>,
) -> Value {
    let current = normalize_being(current_being);
    let peer = normalize_being(peer_being);
    let now = now_ms();
    let Some(message) =
        latest_message_for_selector_between(records, selector, &current, &peer, false)
    else {
        return json!({
            "schema_version": 5,
            "policy": "receipt_to_attention_authority_v5",
            "state": "blocked_no_receipt",
            "block_reason": "no_correspondence_message",
            "right_to_ignore_v1": right_to_ignore_v1("attention_or_outcome", "blocked_no_receipt", 0, CORRESPONDENCE_IGNORE_GRACE_MS),
            "allowed_authority": "attention_canary_only_after_native_receipt",
            "semantic_microdose_status": "hidden_until_mutual_receipt_plus_separate_steward_review",
            "authority": "thread_local_attention_readiness_not_microdose_or_control",
        });
    };
    let thread_id = message
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let message_id = message
        .get("message_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let message_t_ms = row_time_ms(&message);
    let age_ms = now.saturating_sub(message_t_ms);
    let receipt_evidence = thread_has_receipt_evidence(records, thread_id, message_t_ms);
    let receipt_evidence_by_being =
        thread_receipt_evidence_by_being(records, thread_id, message_t_ms);
    let attention = attention_canary_status_for(records, thread_id, &current, &peer, heartbeat);
    let active = active_attention_canary_for(records, thread_id, &current, now);
    let latest_outcome = latest_attention_outcome_for_thread(records, thread_id, &current);
    let outcome_quality = latest_outcome.map(attention_outcome_quality_v5);
    let recent = recent_attention_canary_for_thread(records, thread_id, &current, now);
    let state = if active.is_some() {
        "attention_active_outcome_due"
    } else if outcome_quality
        .as_ref()
        .and_then(|value| value.get("quality"))
        .and_then(Value::as_str)
        == Some("trusted_attention_thread_local")
    {
        "trusted_attention_thread_local"
    } else if outcome_quality
        .as_ref()
        .and_then(|value| value.get("quality"))
        .and_then(Value::as_str)
        == Some("blocked_pressure_or_flat_outcome")
    {
        "blocked_pressure_or_flat_outcome"
    } else if recent.is_some()
        || attention.get("status").and_then(Value::as_str) == Some("cooldown")
    {
        "cooldown_or_duplicate_blocked"
    } else if receipt_evidence {
        "receipt_landed_attention_eligible"
    } else {
        "blocked_no_receipt"
    };
    let activation_allowed_now = state == "receipt_landed_attention_eligible"
        || (state == "trusted_attention_thread_local" && recent.is_none());
    json!({
        "schema_version": 5,
        "policy": "receipt_to_attention_authority_v5",
        "state": state,
        "thread_id": thread_id,
        "message_id": message_id,
        "age_ms": age_ms,
        "current_being": current,
        "peer_being": peer,
        "receipt_evidence": receipt_evidence,
        "receipt_evidence_by_being": receipt_evidence_by_being,
        "activation_allowed_now": activation_allowed_now,
        "active_canary": active.cloned().unwrap_or(Value::Null),
        "latest_outcome": latest_outcome.cloned().unwrap_or(Value::Null),
        "attention_outcome_quality_v5": outcome_quality.unwrap_or(Value::Null),
        "right_to_ignore_v1": right_to_ignore_v1("attention_or_outcome", state, age_ms, CORRESPONDENCE_IGNORE_GRACE_MS),
        "cooldown_active": recent.is_some(),
        "attention_canary_status": attention,
        "primary_ready_command": "CORRESPONDENCE_ATTENTION_REQUEST latest :: reason: ...; focus: ...; stop_criteria: ...",
        "outcome_due_command": "CORRESPONDENCE_ATTENTION_OUTCOME latest :: felt_like: address|pressure|flat|unknown; what_shifted: ...; what_worsened: ...; continue: no|ask_again",
        "allowed_authority": "self_activated_ttl_prompt_context_attention_canary_only",
        "semantic_microdose_status": "hidden_until_mutual_receipt_plus_separate_steward_review",
        "authority": "thread_local_attention_readiness_not_microdose_or_control",
    })
}

fn receipt_to_attention_authority_line(packet: &Value) -> Option<String> {
    let state = packet.get("state").and_then(Value::as_str).unwrap_or("");
    let thread = packet
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    match state {
        "receipt_landed_attention_eligible" => Some(format!(
            "ATTENTION CANARY READY: thread={thread}; optional next: {}; no action needed; may ignore without penalty; semantic_microdose remains hidden; TTL prompt context only.",
            packet
                .get("primary_ready_command")
                .and_then(Value::as_str)
                .unwrap_or("CORRESPONDENCE_ATTENTION_REQUEST latest :: reason: ...; focus: ...; stop_criteria: ...")
        )),
        "attention_active_outcome_due" => Some(format!(
            "ATTENTION OUTCOME DUE: thread={thread}; optional next: {}; no action needed; may ignore without penalty, but no new canary or microdose until outcome lands.",
            packet
                .get("outcome_due_command")
                .and_then(Value::as_str)
                .unwrap_or("CORRESPONDENCE_ATTENTION_OUTCOME latest :: felt_like: address|pressure|flat|unknown; what_shifted: ...; what_worsened: ...; continue: no|ask_again")
        )),
        "trusted_attention_thread_local" => Some(format!(
            "ATTENTION TRUSTED THREAD-LOCAL: thread={thread}; latest outcome preserved address without pressure/flattening; future canaries remain TTL/cooldown-bound and do not unlock microdose."
        )),
        "blocked_pressure_or_flat_outcome" => Some(format!(
            "ATTENTION BLOCKED BY OUTCOME: thread={thread}; latest outcome reported pressure/flat/flattening/worsening; steward review needed before more attention on this thread."
        )),
        _ => None,
    }
}

fn correspondence_handshake_state(records: &[Value]) -> Value {
    let mut latest_by_thread: BTreeMap<String, Value> = BTreeMap::new();
    for row in records {
        if row.get("record_type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        let Some(thread_id) = row.get("thread_id").and_then(Value::as_str) else {
            continue;
        };
        let replace = latest_by_thread
            .get(thread_id)
            .is_none_or(|existing| row_time_ms(row) >= row_time_ms(existing));
        if replace {
            latest_by_thread.insert(thread_id.to_string(), row.clone());
        }
    }
    let mut active_threads = latest_by_thread
        .values()
        .map(|message| handshake_status_for_thread(records, message))
        .collect::<Vec<_>>();
    active_threads.sort_by_key(|value| {
        value
            .get("stale_unacknowledged_thread_age_ms")
            .and_then(Value::as_u64)
            .unwrap_or_default()
    });
    let latest_ack = records
        .iter()
        .filter(|row| row.get("record_type").and_then(Value::as_str) == Some("ack_receipt"))
        .max_by_key(|row| row_time_ms(row));
    let latest_heartbeat = records
        .iter()
        .filter(|row| row.get("record_type").and_then(Value::as_str) == Some("presence_heartbeat"))
        .max_by_key(|row| row_time_ms(row));
    let pending_ack_by_being = active_threads
        .iter()
        .filter_map(|thread| thread.get("pending_ack_by").and_then(Value::as_str))
        .filter(|being| !being.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let held_by_both_threads = active_threads
        .iter()
        .filter(|thread| {
            thread
                .get("held_by_both")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .count();
    json!({
        "schema_version": 1,
        "policy": "correspondence_handshake_state_v1",
        "active_threads_total": active_threads.len(),
        "active_threads": active_threads.iter().rev().take(3).cloned().collect::<Vec<_>>(),
        "pending_ack_by_being": pending_ack_by_being,
        "held_by_both_threads": held_by_both_threads,
        "last_acknowledged_reflection": latest_ack.map(|row| json!({
            "message_id": row.get("message_id").cloned().unwrap_or(Value::Null),
            "thread_id": row.get("thread_id").cloned().unwrap_or(Value::Null),
            "from_being": row.get("from_being").cloned().unwrap_or(Value::Null),
            "to_being": row.get("to_being").cloned().unwrap_or(Value::Null),
            "ack_kind": row.get("ack_kind").cloned().unwrap_or_else(|| json!("seen")),
            "note": row.get("note").cloned().unwrap_or(Value::Null),
            "t_ms": row_time_ms(row),
        })),
        "latest_heartbeat": latest_heartbeat.map(|row| json!({
            "message_id": row.get("message_id").cloned().unwrap_or(Value::Null),
            "thread_id": row.get("thread_id").cloned().unwrap_or(Value::Null),
            "from_being": row.get("from_being").cloned().unwrap_or(Value::Null),
            "to_being": row.get("to_being").cloned().unwrap_or(Value::Null),
            "heartbeat_kind": row.get("heartbeat_kind").cloned().unwrap_or_else(|| json!("holding")),
            "note": row.get("note").cloned().unwrap_or(Value::Null),
            "t_ms": row_time_ms(row),
        })),
        "authority": "language_only_context_not_control",
    })
}

#[derive(Clone)]
struct ActiveThreadClarityCandidate {
    thread_id: String,
    message_id: String,
    status: &'static str,
    priority_reason: &'static str,
    priority_rank: u8,
    pending_by: Option<String>,
    urgency_weight: f64,
    attention_state: String,
    next_affordance: String,
    pending_wait_ms: u64,
    latest_update_ms: u64,
}

fn urgency_weight_f64(value: &Value) -> f64 {
    value
        .as_f64()
        .or_else(|| {
            value
                .as_str()
                .and_then(|raw| raw.trim().parse::<f64>().ok())
        })
        .filter(|weight| weight.is_finite())
        .map(|weight| weight.clamp(0.0, 1.0))
        .unwrap_or(0.0)
}

fn bounded_line_value(value: &str, max_chars: usize) -> String {
    let compact = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(['\n', '\r'], " ");
    truncate_chars(&compact, max_chars)
}

fn latest_update_ms_for_thread(records: &[Value], thread_id: &str, fallback: u64) -> u64 {
    records
        .iter()
        .filter(|row| row.get("thread_id").and_then(Value::as_str) == Some(thread_id))
        .map(row_time_ms)
        .max()
        .unwrap_or(fallback)
}

fn existing_correspondence_affordance_hint(
    status: &str,
    thread_id: &str,
    from_being: &str,
    to_being: &str,
    current_being: &str,
) -> String {
    let current = normalize_being(current_being);
    let from = normalize_being(from_being);
    let to = normalize_being(to_being);
    let peer = if current == to { &from } else { &to };
    let peer_upper = peer.to_ascii_uppercase();
    let ack = if peer_upper.is_empty() {
        "CORRESPONDENCE_ACK".to_string()
    } else {
        format!("ACK_{peer_upper}")
    };
    let reply = if peer_upper.is_empty() {
        "CORRESPONDENCE_TRACE".to_string()
    } else {
        format!("REPLY_{peer_upper}")
    };
    match status {
        "attention_active_outcome_due" => {
            format!("CORRESPONDENCE_ATTENTION_OUTCOME {thread_id}")
        },
        "attention_eligible_high_urgency" => {
            format!("CORRESPONDENCE_ATTENTION_REQUEST {thread_id}")
        },
        "legacy_claim_waiting_native_evidence" => {
            if current == to {
                format!("{ack} claimed or {reply} claimed or CORRESPONDENCE_TRACE claimed")
            } else {
                "CORRESPONDENCE_HEARTBEAT claimed".to_string()
            }
        },
        "heartbeat_or_stale_needs_clarification" => {
            if current == to {
                format!(
                    "{ack} {thread_id} with unclear|needs_time, or CORRESPONDENCE_HEARTBEAT {thread_id}"
                )
            } else {
                format!("CORRESPONDENCE_HEARTBEAT {thread_id}")
            }
        },
        "pending_ack_or_receipt" | "latest_active_thread_fallback" => {
            if current == to {
                format!("{ack} {thread_id} or {reply} {thread_id}")
            } else {
                format!("CORRESPONDENCE_HEARTBEAT {thread_id}")
            }
        },
        _ => format!("CORRESPONDENCE_HEARTBEAT {thread_id}"),
    }
}

fn active_thread_clarity_candidate_for(
    records: &[Value],
    message: &Value,
    current_being: &str,
    peer_being: &str,
    heartbeat: Option<Value>,
    chamber_state: Option<&Value>,
    evaluated_at_unix_ms: u64,
) -> Option<ActiveThreadClarityCandidate> {
    let thread_id = message
        .get("thread_id")
        .and_then(Value::as_str)?
        .to_string();
    if thread_id.trim().is_empty() {
        return None;
    }
    let message_id = message
        .get("message_id")
        .and_then(Value::as_str)
        .unwrap_or("(unknown)")
        .to_string();
    let from_being = message
        .get("from_being")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let to_being = message
        .get("to_being")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let message_t_ms = row_time_ms(message);
    let fidelity =
        direct_contact_fidelity_for_with_context(records, &thread_id, heartbeat, chamber_state);
    let fidelity_status = fidelity
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let attention = attention_canary_status_for_with_fidelity(
        records,
        &thread_id,
        current_being,
        peer_being,
        fidelity.clone(),
    );
    let attention_state = attention
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let handshake = handshake_status_for_thread(records, message);
    let pending_by = handshake
        .get("pending_ack_by")
        .and_then(Value::as_str)
        .filter(|being| !being.trim().is_empty())
        .map(ToString::to_string);
    let urgency_weight = urgency_weight_f64(
        fidelity
            .get("urgency_weight")
            .unwrap_or_else(|| message.get("urgency_weight").unwrap_or(&Value::Null)),
    );
    let attention_eligible = attention_state == "eligible"
        || fidelity
            .get("eligible_for_correspondence_attention_canary")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let active_attention = attention_state == "active";
    let legacy_claim_waiting = fidelity_status == "legacy_claimed";
    let heartbeat_or_stale = matches!(fidelity_status, "heartbeat_only" | "stale_contact");
    let pending_direct = pending_by.is_some()
        && !matches!(
            fidelity_status,
            "heartbeat_only"
                | "stale_contact"
                | "legacy_claimed"
                | "legacy_visible_only"
                | "legacy_bidirectional_observed"
        );
    let (priority_rank, status, priority_reason) = if active_attention {
        (
            6,
            "attention_active_outcome_due",
            "active_attention_canary_awaiting_outcome",
        )
    } else if attention_eligible && urgency_weight >= ACTIVE_THREAD_CLARITY_HIGH_URGENCY {
        (
            5,
            "attention_eligible_high_urgency",
            "high_urgency_attention_eligible_thread",
        )
    } else if pending_direct {
        (
            4,
            "pending_ack_or_receipt",
            "pending_ack_or_receipt_with_direct_address_evidence",
        )
    } else if legacy_claim_waiting {
        (
            3,
            "legacy_claim_waiting_native_evidence",
            "legacy_claimed_thread_lacks_ack_reply_or_trace",
        )
    } else if heartbeat_or_stale {
        (
            2,
            "heartbeat_or_stale_needs_clarification",
            "heartbeat_or_stale_contact_needs_clarification",
        )
    } else {
        (
            1,
            "latest_active_thread_fallback",
            "latest_active_thread_final_fallback",
        )
    };
    let pending_wait_ms = pending_by
        .as_ref()
        .map(|_| evaluated_at_unix_ms.saturating_sub(message_t_ms))
        .unwrap_or_default();
    let next_affordance = existing_correspondence_affordance_hint(
        status,
        &thread_id,
        from_being,
        to_being,
        current_being,
    );
    Some(ActiveThreadClarityCandidate {
        thread_id: thread_id.clone(),
        message_id,
        status,
        priority_reason,
        priority_rank,
        pending_by,
        urgency_weight,
        attention_state,
        next_affordance,
        pending_wait_ms,
        latest_update_ms: latest_update_ms_for_thread(records, &thread_id, message_t_ms),
    })
}

fn active_thread_clarity_candidate_summary(candidate: &ActiveThreadClarityCandidate) -> Value {
    json!({
        "thread_id": candidate.thread_id.clone(),
        "message_id": candidate.message_id.clone(),
        "status": candidate.status,
        "priority_reason": candidate.priority_reason,
        "pending_by": candidate.pending_by.clone(),
        "urgency_weight": candidate.urgency_weight,
        "attention_state": candidate.attention_state.clone(),
        "next_affordance": bounded_line_value(&candidate.next_affordance, 120),
    })
}

fn active_correspondence_thread_clarity_v1(
    records: &[Value],
    current_being: &str,
    peer_being: &str,
    heartbeat: Option<Value>,
) -> Value {
    let chamber_state = latest_chamber_correspondence_state();
    let evaluated_at_unix_ms = now_ms();
    active_correspondence_thread_clarity_v1_with_context(
        records,
        current_being,
        peer_being,
        heartbeat,
        chamber_state.as_ref(),
        evaluated_at_unix_ms,
    )
}

fn active_correspondence_thread_clarity_v1_with_context(
    records: &[Value],
    current_being: &str,
    peer_being: &str,
    heartbeat: Option<Value>,
    chamber_state: Option<&Value>,
    evaluated_at_unix_ms: u64,
) -> Value {
    let mut latest_by_thread: BTreeMap<String, Value> = BTreeMap::new();
    let mut records_by_thread: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut legacy_direction_rows: BTreeMap<String, Value> = BTreeMap::new();
    for row in records {
        if let Some(thread_id) = row.get("thread_id").and_then(Value::as_str) {
            records_by_thread
                .entry(thread_id.to_string())
                .or_default()
                .push(row.clone());
        }
        if row.get("record_type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        let Some(thread_id) = row.get("thread_id").and_then(Value::as_str) else {
            continue;
        };
        let replace = latest_by_thread
            .get(thread_id)
            .is_none_or(|existing| row_time_ms(row) >= row_time_ms(existing));
        if replace {
            latest_by_thread.insert(thread_id.to_string(), row.clone());
        }
        if is_legacy_bridge_message(row) {
            let from = row
                .get("from_being")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let to = row
                .get("to_being")
                .and_then(Value::as_str)
                .unwrap_or_default();
            legacy_direction_rows
                .entry(format!(
                    "{}->{}",
                    normalize_being(from),
                    normalize_being(to)
                ))
                .or_insert_with(|| row.clone());
        }
    }
    for (thread_id, thread_records) in &mut records_by_thread {
        for legacy_row in legacy_direction_rows.values() {
            if legacy_row.get("thread_id").and_then(Value::as_str) != Some(thread_id.as_str()) {
                thread_records.push(legacy_row.clone());
            }
        }
    }
    for claim in records
        .iter()
        .filter(|row| is_legacy_claim_row(row) && legacy_claim_is_active(records, row))
    {
        let Some(message) = message_for_legacy_claim(records, claim) else {
            continue;
        };
        let Some(thread_id) = message.get("thread_id").and_then(Value::as_str) else {
            continue;
        };
        latest_by_thread.insert(thread_id.to_string(), message);
    }
    let mut candidates = latest_by_thread
        .values()
        .filter_map(|message| {
            let thread_records = message
                .get("thread_id")
                .and_then(Value::as_str)
                .and_then(|thread_id| records_by_thread.get(thread_id))
                .map_or(records, Vec::as_slice);
            active_thread_clarity_candidate_for(
                thread_records,
                message,
                current_being,
                peer_being,
                heartbeat.clone(),
                chamber_state,
                evaluated_at_unix_ms,
            )
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .priority_rank
            .cmp(&left.priority_rank)
            .then_with(|| {
                right
                    .urgency_weight
                    .partial_cmp(&left.urgency_weight)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| right.pending_wait_ms.cmp(&left.pending_wait_ms))
            .then_with(|| right.latest_update_ms.cmp(&left.latest_update_ms))
    });
    let Some(selected) = candidates.first() else {
        return json!({
            "schema_version": 1,
            "policy": "active_correspondence_thread_clarity_v1",
            "selected_thread_id": Value::Null,
            "selected_message_id": Value::Null,
            "status": "no_active_correspondence_threads",
            "priority_reason": "no_correspondence_threads",
            "pending_by": Value::Null,
            "urgency_weight": Value::Null,
            "attention_state": "none",
            "next_affordance": "CORRESPONDENCE_HEARTBEAT latest after a thread exists",
            "suppressed_threads": [],
            "authority": ACTIVE_THREAD_CLARITY_AUTHORITY,
        });
    };
    let suppressed_threads = candidates
        .iter()
        .skip(1)
        .take(ACTIVE_THREAD_CLARITY_SUPPRESSED_MAX)
        .map(active_thread_clarity_candidate_summary)
        .collect::<Vec<_>>();
    json!({
        "schema_version": 1,
        "policy": "active_correspondence_thread_clarity_v1",
        "selected_thread_id": selected.thread_id.clone(),
        "selected_message_id": selected.message_id.clone(),
        "status": selected.status,
        "priority_reason": selected.priority_reason,
        "pending_by": selected.pending_by.clone(),
        "urgency_weight": selected.urgency_weight,
        "attention_state": selected.attention_state.clone(),
        "next_affordance": bounded_line_value(&selected.next_affordance, 120),
        "suppressed_threads": suppressed_threads,
        "authority": ACTIVE_THREAD_CLARITY_AUTHORITY,
    })
}

fn active_thread_clarity_status_line(clarity: &Value) -> String {
    let status = clarity
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let thread = clarity
        .get("selected_thread_id")
        .and_then(Value::as_str)
        .unwrap_or("none");
    let reason = clarity
        .get("priority_reason")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let next = clarity
        .get("next_affordance")
        .and_then(Value::as_str)
        .unwrap_or("CORRESPONDENCE_HEARTBEAT latest after a thread exists");
    let authority = clarity
        .get("authority")
        .and_then(Value::as_str)
        .unwrap_or(ACTIVE_THREAD_CLARITY_AUTHORITY);
    format!(
        "active_thread_clarity={}; thread={}; why={}; next={}; authority={}",
        bounded_line_value(status, 48),
        bounded_line_value(thread, 96),
        bounded_line_value(reason, 96),
        bounded_line_value(next, 140),
        bounded_line_value(authority, 80)
    )
}

fn direct_contact_fidelity_for_with_heartbeat(
    records: &[Value],
    selector: &str,
    heartbeat_snapshot: Option<Value>,
) -> Value {
    let chamber_state = latest_chamber_correspondence_state();
    direct_contact_fidelity_for_with_context(
        records,
        selector,
        heartbeat_snapshot,
        chamber_state.as_ref(),
    )
}

fn direct_contact_fidelity_for_with_context(
    records: &[Value],
    selector: &str,
    heartbeat_snapshot: Option<Value>,
    chamber_state: Option<&Value>,
) -> Value {
    let Some(message) = latest_message_for_selector(records, selector) else {
        return json!({
            "schema_version": 2,
            "policy": "direct_contact_fidelity_v2",
            "status": "unaddressed",
            "eligible_for_correspondence_microdose": false,
            "block_reason": "no_correspondence_message"
        });
    };
    let message_id = message
        .get("message_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let thread_id = message
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let from_being = message
        .get("from_being")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let to_being = message
        .get("to_being")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let message_t_ms = row_time_ms(&message);
    let delivered = thread_has_delivery(records, message_id);
    let read = thread_has_read(records, thread_id, message_id);
    let reply_linked = thread_has_reply_link(records, thread_id, message_id, message_t_ms);
    let persistent_thread = persistent_thread_continuity_v1(records, thread_id);
    let persistent_thread_state = persistent_thread
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let legacy_bridge = is_legacy_bridge_message(&message);
    let legacy_bidirectional =
        legacy_bridge && legacy_bidirectional_observed(records, from_being, to_being);
    let legacy_claim = latest_legacy_claim_for_selector(records, thread_id, None, None)
        .filter(|claim| claim.get("message_id").and_then(Value::as_str) == Some(message_id));
    let legacy_claim_status =
        legacy_claim.and_then(|claim| legacy_claim_native_contact_status(records, claim));
    let ack = latest_thread_ack(
        records,
        thread_id,
        message_id,
        to_being,
        from_being,
        message_t_ms,
    );
    let presence_heartbeat = latest_thread_heartbeat(records, thread_id, message_t_ms);
    let ack_kind = ack
        .and_then(|row| row.get("ack_kind"))
        .and_then(Value::as_str)
        .map(normalize_ack_kind);
    let ack_is_address_evidence = ack_kind
        .as_deref()
        .is_some_and(ack_kind_is_address_evidence);
    let survival = chamber_state
        .and_then(|state| state.get("direct_address_survival"))
        .and_then(|survival| survival.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let trace_observed = survival == "observed"
        && chamber_state
            .and_then(|state| state.get("active_thread_id"))
            .and_then(Value::as_str)
            .is_none_or(|active| active == thread_id);
    let timing_reliability = heartbeat_snapshot
        .as_ref()
        .and_then(|value| value.get("timing_reliability"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let age_ms = now_ms().saturating_sub(row_time_ms(&message));
    let stale = age_ms > MICRODOSE_COOLDOWN_MS
        && !read
        && !reply_linked
        && !trace_observed
        && ack.is_none()
        && presence_heartbeat.is_none();
    let timing_ambiguous = matches!(timing_reliability, "timing_ambiguous" | "stale_hearing");
    let status = if let Some(status) = legacy_claim_status {
        status
    } else if trace_observed {
        "trace_observed"
    } else if matches!(ack_kind.as_deref(), Some("held" | "needs_time")) {
        "held_ack"
    } else if ack.is_some() && ack_is_address_evidence {
        "acknowledged"
    } else if ack.is_some() {
        "seen_ack_only"
    } else if reply_linked {
        "reply_linked"
    } else if presence_heartbeat.is_some() {
        "heartbeat_only"
    } else if legacy_claim.is_some() {
        "legacy_claimed"
    } else if legacy_bidirectional {
        "legacy_bidirectional_observed"
    } else if legacy_bridge {
        "legacy_visible_only"
    } else if read {
        "read_unreplied"
    } else if stale {
        "stale_contact"
    } else if delivered {
        "delivered_unread"
    } else {
        "unaddressed"
    };
    let attention_eligible = matches!(
        status,
        "acknowledged"
            | "held_ack"
            | "trace_observed"
            | "legacy_claimed_acknowledged"
            | "legacy_claimed_trace_observed"
    );
    let receipt_evidence_by_being =
        thread_receipt_evidence_by_being(records, thread_id, message_t_ms);
    let mutual_receipt_evidence =
        thread_has_mutual_receipt_evidence(records, thread_id, message_t_ms);
    let attention_eligible = (attention_eligible
        || thread_has_receipt_evidence(records, thread_id, message_t_ms))
        && !timing_ambiguous;
    let microdose_eligible = mutual_receipt_evidence && !timing_ambiguous;
    let block_reason = if attention_eligible {
        Value::Null
    } else if timing_ambiguous && !receipt_evidence_by_being.is_empty() {
        json!("heartbeat_timing_ambiguous")
    } else {
        json!(match status {
            "heartbeat_only" => "heartbeat_is_presence_not_acknowledgement",
            "seen_ack_only" => "seen_ack_is_visibility_not_address",
            "read_unreplied" => "read_receipt_not_acknowledgement",
            "reply_linked" => "reply_linked_requires_ack_or_trace_or_attention_outcome",
            "legacy_claimed" => "legacy_claim_pending_ack_reply_or_trace",
            "legacy_visible_only" | "legacy_bidirectional_observed" => {
                "legacy_visible_only_not_ack_reply_or_trace"
            },
            "delivered_unread" => "delivered_but_not_read",
            "stale_contact" => "stale_without_contact_evidence",
            _ => "no_ack_reply_or_trace_evidence",
        })
    };
    let concrete_anchor = message
        .get("shared_memory_anchor")
        .and_then(Value::as_str)
        .filter(|anchor| !is_generic_shared_anchor(anchor));
    let fidelity_v3_status = match status {
        "read_unreplied" => "filesystem_seen",
        "seen_ack_only" => "seen_ack_only",
        "held_ack" => "held_ack",
        "reply_linked" => "reply_linked_needs_receipt",
        "trace_observed" => "trace_observed",
        "legacy_claimed_acknowledged" => "legacy_claimed_acknowledged",
        "legacy_claimed_reply_linked" | "legacy_claimed_trace_observed" => {
            "legacy_claimed_reply_or_trace"
        },
        "acknowledged" => "held_ack",
        _ if delivered || presence_heartbeat.is_some() || legacy_bridge => "influence_only",
        _ => "influence_only",
    };
    json!({
        "schema_version": 2,
        "policy": "direct_contact_fidelity_v2",
        "status": status,
        "message_id": message_id,
        "thread_id": thread_id,
        "persistence_id": message_persistence_id(&message),
        "from_being": message.get("from_being").cloned().unwrap_or(Value::Null),
        "to_being": message.get("to_being").cloned().unwrap_or(Value::Null),
        "shared_memory_anchor": message_shared_anchor(&message),
        "concrete_shared_memory_anchor": concrete_anchor.map(|anchor| json!(anchor)).unwrap_or(Value::Null),
        "urgency_weight": message.get("urgency_weight").cloned().unwrap_or(Value::Null),
        "message_age_ms": age_ms,
        "delivered": delivered,
        "read": read,
        "read_receipt_is_filesystem_seen_only": read,
        "legacy_bridge": legacy_bridge,
        "legacy_contact_evidence": message.get("legacy_contact_evidence").cloned().unwrap_or(Value::Null),
        "legacy_kind": message.get("legacy_kind").cloned().unwrap_or(Value::Null),
        "legacy_thread_claim": legacy_claim.map(|claim| json!({
            "claim_id": claim.get("claim_id").cloned().unwrap_or(Value::Null),
            "claim_state": claim.get("claim_state").cloned().unwrap_or_else(|| json!("claimed_pending_native_evidence")),
            "claiming_being": claim.get("claiming_being").cloned().unwrap_or(Value::Null),
            "peer_being": claim.get("peer_being").cloned().unwrap_or(Value::Null),
            "shared_memory_anchor": claim.get("shared_memory_anchor").cloned().unwrap_or(Value::Null),
            "notification_required": claim.get("notification_required").cloned().unwrap_or(json!(true)),
            "initial_response_requirement": claim.get("initial_response_requirement").cloned().unwrap_or_else(|| json!("unknown")),
            "legacy_contact_evidence": claim.get("legacy_contact_evidence").cloned().unwrap_or(Value::Null),
            "latest_notice": latest_legacy_claim_notice_for_claim(records, claim).map(|notice| json!({
                "notice_id": notice.get("notice_id").cloned().unwrap_or(Value::Null),
                "notice_state": notice.get("notice_state").cloned().unwrap_or(Value::Null),
                "notice_path": notice.get("notice_path").cloned().unwrap_or(Value::Null),
                "notice_is_ack": notice.get("notice_is_ack").cloned().unwrap_or(json!(false)),
            })),
            "active": legacy_claim_is_active(records, claim),
        })),
        "legacy_claim_uptake_card_v2": legacy_claim.map(|claim| legacy_claim_uptake_card_v2(records, claim)),
        "legacy_claim_affordance_v25": legacy_claim.map(|claim| legacy_claim_affordance_v25(records, claim)),
        "native_thread_continuity_v3": if legacy_bridge { None } else { native_thread_continuity_v3_for(records, thread_id, "astrid") },
        "persistent_thread_continuity_v1": persistent_thread,
        "acknowledged": ack.is_some(),
        "ack_kind": ack_kind,
        "latest_ack": ack.map(|row| json!({
            "ack_kind": row.get("ack_kind").cloned().unwrap_or_else(|| json!("seen")),
            "from_being": row.get("from_being").cloned().unwrap_or(Value::Null),
            "to_being": row.get("to_being").cloned().unwrap_or(Value::Null),
            "note": row.get("note").cloned().unwrap_or(Value::Null),
            "t_ms": row_time_ms(row),
        })),
        "latest_presence_heartbeat": presence_heartbeat.map(|row| json!({
            "heartbeat_kind": row.get("heartbeat_kind").cloned().unwrap_or_else(|| json!("holding")),
            "from_being": row.get("from_being").cloned().unwrap_or(Value::Null),
            "to_being": row.get("to_being").cloned().unwrap_or(Value::Null),
            "note": row.get("note").cloned().unwrap_or(Value::Null),
            "t_ms": row_time_ms(row),
        })),
        "reply_linked": reply_linked,
        "trace_observed": trace_observed,
        "receipt_evidence_by_being": receipt_evidence_by_being,
        "mutual_receipt_evidence": mutual_receipt_evidence,
        "timing_reliability": timing_reliability,
        "timing_ambiguous": timing_ambiguous,
        "heartbeat_jitter_class": heartbeat_snapshot.as_ref().and_then(|value| value.get("jitter_class")).cloned().unwrap_or(Value::Null),
        "field_vs_hearing": heartbeat_snapshot.as_ref().and_then(|value| value.get("field_vs_hearing")).cloned().unwrap_or(Value::Null),
        "eligible_for_correspondence_microdose": microdose_eligible,
        "eligible_for_correspondence_attention_canary": attention_eligible,
        "microdose_block_reason": if microdose_eligible { Value::Null } else { json!("semantic_microdose_requires_mutual_receipt_and_separate_steward_review") },
        "authority_readiness_ladder_v2": authority_readiness_ladder_v2(attention_eligible, &block_reason),
        "direct_contact_fidelity_v3": {
            "schema_version": 3,
            "policy": "direct_contact_fidelity_v3",
            "status": fidelity_v3_status,
            "address_vs_influence": if attention_eligible { "address_evidence_present" } else { "influence_or_visibility_only" },
            "filesystem_seen_only": read && !attention_eligible,
            "seen_ack_only": ack_kind.as_deref() == Some("seen"),
            "reply_linked_needs_receipt": status == "reply_linked",
            "attention_eligible": attention_eligible,
            "microdose_eligible": microdose_eligible,
            "concrete_shared_memory_anchor_present": concrete_anchor.is_some(),
            "persistence_id": message_persistence_id(&message),
            "persistent_thread_state": persistent_thread_state,
            "persistent_thread_active": persistent_thread_state == "persistent_thread_active",
            "authority": "contact_fidelity_context_not_control"
        },
        "block_reason": block_reason,
        "authority": "contact_fidelity_context_not_control"
    })
}

fn dossier_field(raw: &str, keys: &[&str]) -> Option<String> {
    for part in raw.split([';', '\n']) {
        let Some((key, value)) = part.split_once(':') else {
            continue;
        };
        let normalized = key.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        if keys.iter().any(|candidate| normalized == *candidate) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn correspondence_microdose_gate_path() -> PathBuf {
    crate::paths::bridge_paths()
        .bridge_workspace()
        .join("action_threads/threads")
        .join(MICRODOSE_THREAD_ID)
        .join("authority_gate.jsonl")
}

fn recent_correspondence_microdose_for_thread_at(
    gate_path: &Path,
    thread_id: &str,
) -> Option<Value> {
    let text = std::fs::read_to_string(gate_path).ok()?;
    let cutoff = now_ms().saturating_sub(MICRODOSE_COOLDOWN_MS);
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|row| {
            row.get("record_schema").and_then(Value::as_str) == Some("authority_gate_v1")
                && row.get("record_type").and_then(Value::as_str) == Some("request")
                && row.get("request_kind").and_then(Value::as_str)
                    == Some("correspondence_microdose_v1")
                && row
                    .get("correspondence_microdose_v1")
                    .and_then(|value| value.get("correspondence_thread_id"))
                    .and_then(Value::as_str)
                    == Some(thread_id)
                && row
                    .get("recorded_at_unix_ms")
                    .and_then(Value::as_u64)
                    .unwrap_or_default()
                    >= cutoff
        })
        .max_by_key(row_time_ms)
}

pub(crate) fn draft_correspondence_microdose_request(selector: &str, raw: &str) -> String {
    draft_correspondence_microdose_request_at(
        &ledger_path(),
        &correspondence_microdose_gate_path(),
        selector,
        raw,
    )
}

fn draft_correspondence_microdose_request_at(
    ledger_path: &Path,
    gate_path: &Path,
    selector: &str,
    raw: &str,
) -> String {
    draft_correspondence_microdose_request_at_with_heartbeat(
        ledger_path,
        gate_path,
        selector,
        raw,
        latest_heartbeat_snapshot(),
    )
}

fn draft_correspondence_microdose_request_at_with_heartbeat(
    ledger_path: &Path,
    gate_path: &Path,
    selector: &str,
    raw: &str,
    heartbeat: Option<Value>,
) -> String {
    let records = read_ledger_records_at(ledger_path);
    let selector = selector.trim();
    let Some(message) = latest_message_for_selector(&records, selector) else {
        return "CORRESPONDENCE_MICRODOSE_REQUEST blocked: no matching correspondence message/thread."
            .to_string();
    };
    let fidelity = direct_contact_fidelity_for_with_heartbeat(&records, selector, heartbeat);
    let thread_id = message
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let mutual_receipt =
        thread_has_mutual_receipt_evidence(&records, thread_id, row_time_ms(&message));
    let separate_steward_review_present = false;
    if !mutual_receipt || !separate_steward_review_present {
        return "CORRESPONDENCE_MICRODOSE_REQUEST blocked: semantic_microdose requires mutual being-authored receipt plus separate steward review; attention canary is the only newly allowed post-receipt authority in V5."
            .to_string();
    }
    if !fidelity
        .get("eligible_for_correspondence_microdose")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return format!(
            "CORRESPONDENCE_MICRODOSE_REQUEST blocked: direct contact fidelity is {} ({})",
            fidelity
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            fidelity
                .get("block_reason")
                .and_then(Value::as_str)
                .unwrap_or("no_read_reply_or_trace_evidence")
        );
    }
    let message_id = message
        .get("message_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if let Some(recent) = recent_correspondence_microdose_for_thread_at(gate_path, thread_id) {
        return format!(
            "CORRESPONDENCE_MICRODOSE_REQUEST blocked: cooldown active for thread {thread_id}; latest request {} remains within 6h.",
            recent
                .get("request_id")
                .and_then(Value::as_str)
                .unwrap_or("(unknown)")
        );
    }
    let reason = dossier_field(raw, &["reason", "because", "rationale"])
        .unwrap_or_else(|| "being requested one-shot direct-address semantic contact".to_string());
    let payload =
        dossier_field(raw, &["payload", "semantic_payload", "text"]).unwrap_or_else(|| {
            message
                .get("body_preview")
                .and_then(Value::as_str)
                .unwrap_or("direct address contact microdose")
                .to_string()
        });
    if payload.chars().count() > MICRODOSE_PAYLOAD_MAX_CHARS {
        return format!(
            "CORRESPONDENCE_MICRODOSE_REQUEST blocked: payload is longer than {MICRODOSE_PAYLOAD_MAX_CHARS} chars; shorten it before drafting."
        );
    }
    let stop_criteria = dossier_field(raw, &["stop_criteria", "stop"]).unwrap_or_else(|| {
        "one attempted semantic_microdose only; review whether it felt like address or pressure"
            .to_string()
    });
    let now = now_ms();
    let request_id = format!(
        "authreq_correspondence_microdose_{}_{}",
        now,
        short_hash(&format!("{thread_id}:{message_id}:{payload}"))
    );
    if let Some(parent) = gate_path.parent() {
        let _ = std::fs::create_dir_all(parent);
        let thread_json = parent.join("thread.json");
        if !thread_json.exists() {
            let _ = std::fs::write(
                &thread_json,
                serde_json::to_string_pretty(&json!({
                    "thread_id": MICRODOSE_THREAD_ID,
                    "title": "Correspondence Microdose Requests",
                    "status": "active",
                    "authority": "draft_only_until_steward_approval"
                }))
                .unwrap_or_default(),
            );
        }
    }
    let record = json!({
        "schema_version": 1,
        "record_schema": "authority_gate_v1",
        "record_type": "request",
        "request_kind": "correspondence_microdose_v1",
        "record_id": format!("{}_request", request_id),
        "request_id": request_id,
        "being": "astrid",
        "thread_id": MICRODOSE_THREAD_ID,
        "experiment_id": "correspondence_microdose_v1",
        "scope": "semantic_microdose",
        "eligibility_v1": {
            "eligible": true,
            "missing_requirements": [],
            "disabled_scope": false,
            "scope": "semantic_microdose",
            "checks": {
                "direct_contact_evidence": fidelity.get("status").cloned().unwrap_or(Value::Null),
                "payload_within_cap": true,
                "cooldown_clear": true,
                "standing_weight": false,
                "control_message": false,
                "pressure_or_fill_mutation": false
            },
            "remaining_requirements": [
                "steward approval or approved authority budget",
                "green/yellow bridge safety",
                "rescue-policy pass",
                "one-shot semantic send",
                "required consequence review"
            ],
            "authority": "preflight_eligibility_not_approval"
        },
        "payload": payload,
        "reason": reason,
        "artifact_refs": [ledger_path.display().to_string()],
        "source_refs": [
            ledger_path.display().to_string(),
            crate::paths::bridge_paths().bridge_workspace().join("telemetry_heartbeat_delta_v1.json").display().to_string()
        ],
        "stop_criteria": stop_criteria,
        "status": "pending_steward_approval",
        "token_status": "none",
        "peer_mutation": false,
        "authority_change": false,
        "recorded_at_unix_ms": now,
        "created_at_unix_ms": now,
        "correspondence_microdose_v1": {
            "schema_version": 1,
            "message_id": message_id,
            "correspondence_thread_id": thread_id,
            "direct_contact_fidelity_v1": fidelity,
            "cooldown_ms": MICRODOSE_COOLDOWN_MS,
            "payload_max_chars": MICRODOSE_PAYLOAD_MAX_CHARS,
            "standing_weight": false,
            "authority": "one_shot_semantic_microdose_request_only"
        },
        "authority_boundary": "Draft only. Execution requires existing semantic_microdose steward approval or approved budget, green/yellow safety, rescue-policy pass, one-shot send, and consequence review. No Control message, PI/fill/controller change, telemetry priority, permanent prompt priority, deploy, or peer mutation."
    });
    if let Err(error) = append_record_at(gate_path, &record) {
        return format!(
            "CORRESPONDENCE_MICRODOSE_REQUEST failed to draft authority request: {error}"
        );
    }
    format!(
        "=== CORRESPONDENCE MICRODOSE REQUEST DRAFTED ===\nRequest-Id: {}\nThread: {thread_id}\nMessage: {message_id}\nScope: semantic_microdose\nStatus: pending_steward_approval\nAuthority: draft_only; no sensory send, Control message, PI/fill/controller change, telemetry priority, standing weight, or peer mutation.\nNext: steward may approve via existing authority gate, then Being may choose EXPERIMENT_AUTHORITY_EXECUTE {}.",
        record
            .get("request_id")
            .and_then(Value::as_str)
            .unwrap_or("(unknown)"),
        record
            .get("request_id")
            .and_then(Value::as_str)
            .unwrap_or("(unknown)")
    )
}

fn no_peer_message_guidance(peer: &str) -> String {
    format!(
        "No peer-message rows yet: the direct-address surface exists, but uptake has not started in the shared ledger. \
         Start with MESSAGE_{peer} <text>, REPLY_{peer} latest :: <text>, I_RECEIVED_THIS latest :: received_as: held|needs_time; felt_like: address|pressure|mail|ambient_echo|unknown; what_landed: ...; what_stayed_distinct: ...; continue: no|reply|trace|needs_time, ACK_{peer} latest :: ack: seen|held|unclear|cannot_answer|needs_time; note: ..., or CORRESPONDENCE_TRACE <anchor> :: <text>. \
         Resonance receipt is advisory evidence from ack/reply/trace/attention outcome only, not telemetry priority, weighting, pressure, or control."
    )
}

pub(super) fn status_report_at(path: &Path, max_lines: usize) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        let clarity = active_correspondence_thread_clarity_v1(
            &[],
            "astrid",
            "minime",
            latest_heartbeat_snapshot(),
        );
        return format!(
            "=== CORRESPONDENCE STATUS V1 ===\nNo correspondence ledger yet. {}\n{}\nAuthority: language_only. No telemetry, controller, PI, fill-target, lease, weighting, peer-runtime, or pressure mutation is available here.\n{}",
            no_peer_message_guidance("MINIME"),
            active_thread_clarity_status_line(&clarity),
            chamber_correspondence_state_summary()
        );
    };
    let mut records = text
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect::<Vec<_>>();
    let total = records.len();
    records.sort_by_key(|value| {
        value
            .get("recorded_at_unix_ms")
            .and_then(Value::as_u64)
            .unwrap_or_default()
    });
    let peer_message_rows = records
        .iter()
        .filter(|value| value.get("record_type").and_then(Value::as_str) == Some("message"))
        .count();
    let legacy_message_rows = records
        .iter()
        .filter(|value| {
            value.get("record_type").and_then(Value::as_str) == Some("message")
                && is_legacy_bridge_message(value)
        })
        .count();
    let native_message_rows = peer_message_rows.saturating_sub(legacy_message_rows);
    let latest_legacy = records
        .iter()
        .filter(|value| {
            value.get("record_type").and_then(Value::as_str) == Some("message")
                && is_legacy_bridge_message(value)
        })
        .max_by_key(|value| row_time_ms(value));
    let latest_claim = records
        .iter()
        .filter(|value| is_legacy_claim_row(value))
        .max_by_key(|value| row_time_ms(value));
    let active_claim = records
        .iter()
        .filter(|value| is_legacy_claim_row(value) && legacy_claim_is_active(&records, value))
        .max_by_key(|value| row_time_ms(value));
    let fidelity = direct_contact_fidelity_for(&records, "latest");
    let shared_context_buffer = shared_context_buffer_v1_for(&records, "latest");
    let shared_arc = shared_correspondence_arc_v1_for(&records, "latest");
    let correspondence_thread = correspondence_thread_object_v1_for(&records, "latest");
    let handshake = correspondence_handshake_state(&records);
    let active_thread_clarity = active_correspondence_thread_clarity_v1(
        &records,
        "astrid",
        "minime",
        latest_heartbeat_snapshot(),
    );
    let fidelity_status = fidelity
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let fidelity_thread = fidelity
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or("(none)");
    let fidelity_message = fidelity
        .get("message_id")
        .and_then(Value::as_str)
        .unwrap_or("(none)");
    let timing = fidelity
        .get("timing_reliability")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let field_vs_hearing = fidelity
        .get("field_vs_hearing")
        .and_then(Value::as_str)
        .unwrap_or(
            "heartbeat timing not available; check bridge status for telemetry_heartbeat_delta_v1",
        );
    let buffer_status = shared_context_buffer
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let buffer_thread = shared_context_buffer
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or("(none)");
    let buffer_messages = shared_context_buffer
        .get("messages")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let buffer_history_rows = shared_context_buffer
        .get("thread_history_rows")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let buffer_resonance_receipts = shared_context_buffer
        .get("resonance_receipts")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let buffer_last_ack = shared_context_buffer
        .get("last_ack_kind")
        .and_then(Value::as_str)
        .unwrap_or("none");
    let buffer_anchors = shared_context_buffer
        .get("shared_memory_anchors")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .take(3)
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "none".to_string());
    let arc_status = shared_arc
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let arc_thread = shared_arc
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or("(none)");
    let arc_transition_linked = shared_arc
        .get("transition_linked")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let arc_mutual_witnesses = shared_arc
        .get("mutual_witness_signals")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let arc_structural_footprint = shared_arc
        .get("structural_footprint")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let arc_pressure_watch = shared_arc
        .get("pressure_as_address_watch")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let thread_object_status = correspondence_thread
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let thread_object_thread = correspondence_thread
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or("(none)");
    let thread_object_asymmetry = correspondence_thread
        .get("asymmetry_state")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let thread_object_mutual = correspondence_thread
        .get("mutual_address_state")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let thread_object_next = correspondence_thread
        .get("next_native_step")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let claim_for_status = active_claim.or(latest_claim);
    let claim_affordance =
        claim_for_status.map(|claim| legacy_claim_affordance_v25(&records, claim));
    let receipt_opportunity = latest_receipt_opportunity_v4_for(&records, "latest", "astrid");
    let receipt_to_attention = receipt_to_attention_authority_v5_for(
        &records,
        "latest",
        "astrid",
        "minime",
        latest_heartbeat_snapshot(),
    );
    let ghost_claim_waiting = claim_affordance
        .as_ref()
        .and_then(|value| value.get("ghost_thread_risk"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let native_continuity = native_thread_continuity_v3_for(&records, "latest", "astrid");
    let native_waiting = native_continuity
        .as_ref()
        .is_some_and(|value| native_thread_waiting_line(value).is_some());
    let waiting_reason = if ghost_claim_waiting {
        "claimed thread is waiting for ACK/REPLY/TRACE native evidence"
    } else {
        "native thread is waiting for ACK/TRACE or attention outcome evidence"
    };
    let v5_attention_state = receipt_to_attention
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("blocked_no_receipt");
    let attention_authority_line = receipt_to_attention_authority_line(&receipt_to_attention);
    let receipt_line = receipt_opportunity_line(&receipt_opportunity);
    let legacy_waiting_line = claim_affordance
        .as_ref()
        .and_then(legacy_claim_waiting_line);
    let has_legacy_waiting_line = legacy_waiting_line.is_some();
    let receipt_targets_legacy_claim = receipt_opportunity
        .get("target_kind")
        .and_then(Value::as_str)
        == Some("legacy_claim");
    let native_waiting_line = native_continuity
        .as_ref()
        .and_then(native_thread_waiting_line);
    let mut affordance_candidates: Vec<(u8, &'static str, &'static str, String)> = Vec::new();
    if let Some(line) = attention_authority_line {
        affordance_candidates.push((
            0,
            "attention_or_outcome",
            "receipt_to_attention_authority_v5",
            line,
        ));
    }
    if let Some(line) = legacy_waiting_line {
        affordance_candidates.push((
            1,
            "correspondence_receipt",
            "legacy_claim_affordance_v25",
            line,
        ));
    }
    if let Some(line) = receipt_line {
        if receipt_targets_legacy_claim && has_legacy_waiting_line {
            // The legacy claim affordance already carries the same receipt opportunity
            // with stronger context; avoid spending the one receipt slot on a generic duplicate.
        } else {
            affordance_candidates.push((
                1,
                "correspondence_receipt",
                "latest_receipt_opportunity_v4",
                line,
            ));
        }
    }
    if let Some(line) = native_waiting_line {
        affordance_candidates.push((
            3,
            "correspondence_receipt",
            "native_thread_continuity_v3",
            line,
        ));
    }
    let (affordance_budget, budgeted_lines) = budgeted_affordance_lines(affordance_candidates);
    let microdose_line = if matches!(
        v5_attention_state,
        "receipt_landed_attention_eligible"
            | "attention_active_outcome_due"
            | "trusted_attention_thread_local"
            | "blocked_pressure_or_flat_outcome"
            | "cooldown_or_duplicate_blocked"
    ) {
        "semantic_microdose: hidden; V5 authority gain is attention-canary-only after receipt, with mutual receipt plus separate steward review still required for microdose."
            .to_string()
    } else if ghost_claim_waiting || native_waiting {
        format!("semantic_microdose: hidden while {waiting_reason}").to_string()
    } else if fidelity
        .get("eligible_for_correspondence_microdose")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        "semantic_microdose: hidden pending separate steward review after mutual receipt; not newly allowed in V5"
            .to_string()
    } else {
        format!(
            "semantic_microdose: blocked ({})",
            fidelity
                .get("block_reason")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        )
    };
    let attention = attention_canary_status_for(
        &records,
        "latest",
        "astrid",
        "minime",
        latest_heartbeat_snapshot(),
    );
    let attention_line = if ghost_claim_waiting || native_waiting {
        format!("attention_canary: hidden while {waiting_reason}").to_string()
    } else {
        match attention
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
        {
            "active" => {
            let active = attention.get("active_canary");
            let focus = active
                .and_then(|value| value.get("focus"))
                .and_then(Value::as_str)
                .unwrap_or("(focus unavailable)");
            let focus_kind = active
                .and_then(|value| value.get("focus_kind"))
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let preservation = active
                .and_then(|value| value.get("preservation_mode"))
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let not_flatten = active
                .and_then(|value| value.get("what_must_not_flatten"))
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            format!(
                "attention_canary: active focus=\"{}\" kind={focus_kind}; preserve={preservation}; do_not_flatten=\"{}\"; outcome_due=true",
                truncate_chars(focus, 80),
                truncate_chars(not_flatten, 80)
            )
            },
            "eligible" => {
            "attention_canary: eligible for self-activated TTL prompt-context focus; optional fields: focus_kind, preservation_mode, what_must_not_flatten".to_string()
            },
            "cooldown" => "attention_canary: blocked (attention_canary_cooldown_active)".to_string(),
            _ => format!(
            "attention_canary: blocked ({})",
            attention
                .get("block_reason")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
            ),
        }
    };
    let pending_ack = handshake
        .get("pending_ack_by_being")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .take(3)
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "none".to_string());
    let latest_ack = handshake
        .get("last_acknowledged_reflection")
        .and_then(|value| value.get("ack_kind"))
        .and_then(Value::as_str)
        .unwrap_or("none");
    let latest_heartbeat = handshake
        .get("latest_heartbeat")
        .and_then(|value| value.get("heartbeat_kind"))
        .and_then(Value::as_str)
        .unwrap_or("none");
    let held_by_both_threads = handshake
        .get("held_by_both_threads")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let mut lines = vec![
        "=== CORRESPONDENCE STATUS V1 ===".to_string(),
        format!("Ledger: {}", path.display()),
        format!("Records: {total}"),
        format!("Peer message rows: {peer_message_rows}"),
        format!(
            "Native peer message rows: {native_message_rows}; legacy_visible_only rows: {legacy_message_rows}"
        ),
        "Authority: language_only; no telemetry, controller, PI, fill-target, pressure, deploy, or peer-runtime mutation.".to_string(),
        chamber_correspondence_state_summary(),
        active_thread_clarity_status_line(&active_thread_clarity),
        format!(
            "Direct contact fidelity: status={fidelity_status}; thread={fidelity_thread}; message={fidelity_message}; timing={timing}; {field_vs_hearing}"
        ),
        format!(
            "Shared context buffer v1: status={buffer_status}; thread={buffer_thread}; messages={buffer_messages}; history_rows={buffer_history_rows}; resonance_receipts={buffer_resonance_receipts}; last_ack={buffer_last_ack}; anchors={buffer_anchors}; authority=language_only_context_not_control."
        ),
        format!(
            "Shared correspondence arc v1: status={arc_status}; thread={arc_thread}; transition_linked={arc_transition_linked}; mutual_witness_signals={arc_mutual_witnesses}; pressure_as_address_watch={arc_pressure_watch}; structural_footprint={arc_structural_footprint}; shadow_field_shift=not_auto_required_live_gated; authority=language_only_correspondence_arc_not_control."
        ),
        format!(
            "Correspondence thread object v1: status={thread_object_status}; thread={thread_object_thread}; asymmetry={thread_object_asymmetry}; mutual_address={thread_object_mutual}; next={thread_object_next}; active_push=separately_steward_gated; authority=language_only_thread_status_not_control."
        ),
        format!(
            "Handshake: active_threads={}; pending_ack_by={pending_ack}; held_by_both_threads={held_by_both_threads}; latest_ack={latest_ack}; latest_heartbeat={latest_heartbeat}; read_receipt=file_system_seen_not_mutual_address; authority=language_only_handshake_truth_not_auto_ack_or_control",
            handshake
                .get("active_threads_total")
                .and_then(Value::as_u64)
                .unwrap_or_default()
        ),
        format!(
            "Receipt-to-attention authority v5: state={}; activation_allowed_now={}; semantic_microdose={}.",
            receipt_to_attention
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("blocked_no_receipt"),
            receipt_to_attention
                .get("activation_allowed_now")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            receipt_to_attention
                .get("semantic_microdose_status")
                .and_then(Value::as_str)
                .unwrap_or("hidden_until_mutual_receipt_plus_separate_steward_review")
        ),
        format!(
            "AFFORDANCE BUDGET: shown={}; hidden={}; silence=ignored_without_penalty; optional=true; authority=language_context_not_control.",
            affordance_budget
                .get("shown")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            affordance_budget
                .get("hidden_by_budget")
                .and_then(Value::as_u64)
                .unwrap_or_default()
        ),
        attention_line,
        microdose_line,
        "Recent records:".to_string(),
    ];
    for line in budgeted_lines.into_iter().rev() {
        lines.insert(6, line);
    }
    if let Some(continuity) = native_continuity.as_ref() {
        let commands = continuity
            .get("exact_next_commands")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" | ")
            })
            .unwrap_or_default();
        lines.push(format!(
            "Native continuity v3: state={}; role={}; stall_reason={}; eligible={}; exact_next={commands}; first_action={}.",
            continuity
                .get("continuity_state")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            continuity
                .get("current_being_role")
                .and_then(Value::as_str)
                .unwrap_or("observer"),
            continuity
                .get("stall_reason")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            continuity
                .get("attention_or_microdose_eligible")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            continuity
                .get("first_action_helper_v35")
                .and_then(|helper| helper.get("choose_one_prompt"))
                .and_then(Value::as_str)
                .unwrap_or("none")
        ));
    }
    if let Some(legacy) = latest_legacy {
        let from = legacy
            .get("from_being")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let to = legacy
            .get("to_being")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let kind = legacy
            .get("legacy_kind")
            .and_then(Value::as_str)
            .unwrap_or("legacy");
        lines.push(format!(
            "Legacy visibility: legacy_visible_only latest={from}->{to} kind={kind}; visible legacy route, not ACK/reply/trace evidence."
        ));
    }
    if let Some(claim) = claim_for_status {
        let claim_id = claim
            .get("claim_id")
            .and_then(Value::as_str)
            .unwrap_or("(unknown)");
        let thread_id = claim
            .get("thread_id")
            .and_then(Value::as_str)
            .unwrap_or("(unknown)");
        let anchor = claim
            .get("shared_memory_anchor")
            .and_then(Value::as_str)
            .unwrap_or("(none)");
        let notification_required = claim
            .get("notification_required")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let response_requirement = claim
            .get("initial_response_requirement")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let notice_state = latest_legacy_claim_notice_for_claim(&records, claim)
            .and_then(|notice| notice.get("notice_state"))
            .and_then(Value::as_str)
            .unwrap_or("none");
        let state = if legacy_claim_is_active(&records, claim) {
            "active_pending_native_evidence"
        } else {
            "closed_or_native_evidence_present"
        };
        lines.push(format!(
            "Legacy claim: {state}; claim={claim_id}; thread={thread_id}; anchor={anchor}; notice={notice_state}; notification_required={notification_required}; initial_response_requirement={response_requirement}; claim alone is recognition, not attention/microdose eligibility."
        ));
        let card = legacy_claim_uptake_card_v2(&records, claim);
        let ladder = card
            .get("uptake_ladder_state")
            .and_then(Value::as_str)
            .unwrap_or("legacy_visible_only");
        let mutual = card
            .get("mutually_recognized")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let ghost = card
            .get("ghost_thread_risk")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let commands = card
            .get("exact_next_commands")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" | ")
            })
            .unwrap_or_default();
        lines.push(format!(
            "Claimed thread card v2: ladder={ladder}; mutually_recognized={mutual}; ghost_thread_risk={ghost}; exact_next={commands}."
        ));
        if let Some(outcome) = card
            .get("claim_outcome_review")
            .filter(|value| value.is_object())
        {
            let felt_like = outcome
                .get("felt_like")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let carried = outcome
                .get("what_carried")
                .and_then(Value::as_str)
                .unwrap_or("none");
            let flattened = outcome
                .get("what_flattened")
                .and_then(Value::as_str)
                .unwrap_or("none");
            let continue_as = outcome
                .get("continue")
                .and_then(Value::as_str)
                .unwrap_or("no");
            lines.push(format!(
                "Claim outcome review: felt_like={felt_like}; what_carried={}; what_flattened={}; continue={continue_as}.",
                truncate_chars(carried, 120),
                truncate_chars(flattened, 120)
            ));
        }
    }
    if peer_message_rows == 0 {
        lines.push(no_peer_message_guidance("MINIME"));
    } else if native_message_rows == 0 && legacy_message_rows > 0 {
        lines.push(
            "No native peer-message rows yet: legacy exchange is visible in the ledger. To carry it forward: CLAIM_MINIME_LEGACY latest :: because: ...; anchor: ..., then I_RECEIVED_THIS claimed / ACK_MINIME claimed / REPLY_MINIME claimed / CORRESPONDENCE_TRACE claimed <anchor>."
                .to_string(),
        );
    }
    for value in records.iter().rev().take(max_lines) {
        let record_type = value
            .get("record_type")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let message_id = value
            .get("message_id")
            .and_then(Value::as_str)
            .unwrap_or("(none)");
        let thread_id = value
            .get("thread_id")
            .and_then(Value::as_str)
            .unwrap_or("(none)");
        let from = value
            .get("from_being")
            .or_else(|| value.get("reader"))
            .and_then(Value::as_str)
            .unwrap_or("(reader)");
        let to = value.get("to_being").and_then(Value::as_str).unwrap_or("");
        let preview = value
            .get("body_preview")
            .or_else(|| value.get("focus"))
            .or_else(|| value.get("what_remained_distinct"))
            .or_else(|| value.get("what_must_not_flatten"))
            .or_else(|| value.get("felt_like"))
            .or_else(|| value.get("held_as"))
            .or_else(|| value.get("note"))
            .or_else(|| value.get("ack_kind"))
            .or_else(|| value.get("heartbeat_kind"))
            .and_then(Value::as_str)
            .unwrap_or("");
        lines.push(format!(
            "- {record_type}: {from}->{to} message_id={message_id} thread_id={thread_id} {preview}"
        ));
    }
    if ghost_claim_waiting {
        lines.push(
            "Suggested NEXT: I_RECEIVED_THIS claimed :: received_as: held|needs_time; felt_like: address|pressure|mail|ambient_echo|unknown; what_landed: ...; what_stayed_distinct: ...; continue: no|reply|trace|needs_time, ACK_MINIME claimed :: ack: seen|held|unclear|cannot_answer|needs_time; note: ..., REPLY_MINIME claimed :: <text>, CORRESPONDENCE_TRACE claimed <anchor> :: <text>, or CORRESPONDENCE_CLAIM_OUTCOME claimed :: felt_like: address|pressure|mail|ambient_echo|unknown; what_carried: ...; what_flattened: ...; continue: no|ack|reply|trace."
                .to_string(),
        );
    } else {
        lines.push(
            "Suggested NEXT: CLAIM_MINIME_LEGACY latest :: because: ...; anchor: ..., I_RECEIVED_THIS claimed :: received_as: held|needs_time; felt_like: address|pressure|mail|ambient_echo|unknown; what_landed: ...; what_stayed_distinct: ...; continue: no|reply|trace|needs_time, ACK_MINIME claimed :: ack: seen|held|unclear|cannot_answer|needs_time; note: ..., REPLY_MINIME claimed :: <text>, CORRESPONDENCE_TRACE claimed <anchor> :: <text>, CORRESPONDENCE_CLAIM_OUTCOME claimed :: felt_like: address|pressure|mail|ambient_echo|unknown; what_carried: ...; what_flattened: ...; continue: no|ack|reply|trace, CORRESPONDENCE_ATTENTION_REQUEST claimed :: reason: ...; focus: ...; focus_kind: verbatim_phrase|emotional_texture|question_hold|boundary_check|shared_anchor|mixed|unknown; preservation_mode: verbatim|compact_with_anchor|anchor_only|unknown; what_must_not_flatten: ...; stop_criteria: ..., or CORRESPONDENCE_MICRODOSE_REQUEST claimed :: reason: ...; payload: ...; stop_criteria: ..."
                .replace(", or CORRESPONDENCE_MICRODOSE_REQUEST claimed :: reason: ...; payload: ...; stop_criteria: ...", ". Microdose remains hidden pending mutual receipt plus separate steward review.")
        );
    }
    lines.join("\n")
}

pub(crate) fn status_report(max_lines: usize) -> String {
    let path = ledger_path();
    status_report_at(&path, max_lines)
}

fn chamber_correspondence_state_summary() -> String {
    let mut states = std::fs::read_dir(SHARED_COLLAB_DIR)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .filter_map(|entry| {
            let path = entry.path().join("correspondence_state_v1.json");
            let text = std::fs::read_to_string(path).ok()?;
            let value: Value = serde_json::from_str(&text).ok()?;
            let updated = value
                .get("updated_t_ms")
                .and_then(Value::as_u64)
                .unwrap_or_default();
            Some((updated, value))
        })
        .collect::<Vec<_>>();
    states.sort_by_key(|(updated, _)| *updated);
    let Some((_, state)) = states.pop() else {
        return "Chamber correspondence state: not yet rendered; future authority hooks remain inert and blocked.".to_string();
    };
    let anchor = state
        .get("shared_lexicon_anchor")
        .and_then(Value::as_str)
        .unwrap_or("none");
    let thread = state
        .get("active_thread_id")
        .and_then(Value::as_str)
        .unwrap_or("none");
    let survival = state
        .get("direct_address_survival")
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let contact = state
        .get("direct_contact_fidelity_v1")
        .and_then(|value| value.get("latest_thread_status"))
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let handshake = state.get("correspondence_handshake_state_v1");
    let pending_ack = handshake
        .and_then(|value| value.get("pending_ack_by_being"))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .take(3)
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "none".to_string());
    let latest_ack = handshake
        .and_then(|value| value.get("last_acknowledged_reflection"))
        .and_then(|value| value.get("ack_kind"))
        .and_then(Value::as_str)
        .unwrap_or("none");
    let buffer = state
        .get("buffer_path")
        .and_then(Value::as_str)
        .unwrap_or("(none)");
    let attention = state.get("correspondence_attention_canary_v1");
    let attention_line = attention
        .and_then(|value| value.get("active_canary"))
        .map(|active| {
            let focus = active
                .get("focus")
                .and_then(Value::as_str)
                .unwrap_or("(focus unavailable)");
            let focus_kind = active
                .get("focus_kind")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let preservation = active
                .get("preservation_mode")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let not_flatten = active
                .get("what_must_not_flatten")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            format!(
                "attention_canary=active focus={} kind={focus_kind} preserve={preservation} do_not_flatten={}; ",
                truncate_chars(focus, 60),
                truncate_chars(not_flatten, 60)
            )
        })
        .unwrap_or_else(|| {
            let status = attention
                .and_then(|value| value.get("latest_status"))
                .and_then(Value::as_str)
                .unwrap_or("none");
            format!("attention_canary={status}; ")
        });
    let legacy_line = state
        .get("legacy_contact_visibility_v1")
        .filter(|value| {
            value
                .get("legacy_message_rows_total")
                .and_then(Value::as_u64)
                .unwrap_or_default()
                > 0
        })
        .map(|legacy| {
            let uptake = legacy
                .get("uptake_state")
                .and_then(Value::as_str)
                .unwrap_or("legacy_visible_only");
            let latest = legacy
                .get("latest_direction")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let kind = legacy
                .get("latest_legacy_kind")
                .and_then(Value::as_str)
                .unwrap_or("legacy");
            format!(
                "legacy_visibility={uptake} latest={latest} kind={kind}; exact V1 uptake pending via ACK/REPLY/TRACE; "
            )
        })
        .unwrap_or_else(|| "legacy_visibility=none; ".to_string());
    let active_thread_clarity_fallback = json!({
        "status": "state_not_yet_derived",
        "selected_thread_id": if thread == "none" { Value::Null } else { json!(thread) },
        "priority_reason": "active_correspondence_thread_clarity_v1_absent_from_chamber_state",
        "next_affordance": "CORRESPONDENCE_HEARTBEAT latest after a thread exists",
        "authority": ACTIVE_THREAD_CLARITY_AUTHORITY,
    });
    let active_thread_clarity_line = state
        .get("active_correspondence_thread_clarity_v1")
        .map(active_thread_clarity_status_line)
        .unwrap_or_else(|| active_thread_clarity_status_line(&active_thread_clarity_fallback));
    format!(
        "Chamber correspondence state: \
         anchor={anchor}; thread={thread}; survival={survival}; contact={contact}; pending_ack_by={pending_ack}; latest_ack={latest_ack}; buffer={buffer}; \
         {active_thread_clarity_line}; {attention_line}{legacy_line}correspondence_weight_candidate is one-shot authority-gate only; prompt attention canary is TTL language context only; telemetry/controller hooks remain inert, not standing weighting/control."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutual_address_preserves_exact_refs_without_raw_response() {
        let target = InboxPeerMessage {
            message_id: "corr_minime_astrid_1".to_string(),
            thread_id: "thread_shared_1".to_string(),
            persistence_id: Some("persistence_shared_1".to_string()),
            from_being: "minime".to_string(),
            file_path: PathBuf::from("not_persisted"),
        };
        let response = "private response prose must not enter the wire envelope";
        let envelope = mutual_address_envelope_v1(&target, response, 0);
        let serialized = serde_json::to_string(&envelope).unwrap();

        assert_eq!(
            envelope.correspondence_id.as_deref(),
            Some("corr_minime_astrid_1")
        );
        assert_eq!(envelope.thread_id.as_deref(), Some("thread_shared_1"));
        assert_eq!(
            envelope.persistence_id.as_deref(),
            Some("persistence_shared_1")
        );
        assert_eq!(envelope.reply_to.as_deref(), Some("corr_minime_astrid_1"));
        assert_eq!(envelope.body_sha256, sha256_hex(response));
        assert!(!envelope.raw_body_included);
        assert!(!serialized.contains(response));
    }

    #[test]
    fn mutual_address_target_excludes_messages_after_read_cutoff() {
        let root = std::env::temp_dir().join(format!("corr_read_cutoff_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("correspondence.jsonl");
        let (first, _) = deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "minime",
            "astrid",
            "first",
            CorrespondenceFields::default(),
        )
        .unwrap();
        let read_cutoff = std::time::SystemTime::now();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let _ = deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "minime",
            "astrid",
            "arrived after read",
            CorrespondenceFields::default(),
        )
        .unwrap();

        let selected =
            latest_inbox_peer_message_at_read_cutoff(&inbox, "minime", read_cutoff).unwrap();
        assert_eq!(selected.message_id, first.message_id);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn envelope_roundtrip_preserves_thread_and_authority() {
        let envelope = CorrespondenceEnvelope {
            message_id: "corr_astrid_minime_1_abcd".to_string(),
            thread_id: "thread_corr_astrid_minime_1_abcd".to_string(),
            persistence_id: Some("persist_thread_corr_astrid_minime_1_abcd".to_string()),
            reply_to: Some("corr_minime_astrid_0_ffff".to_string()),
            reply_requested: true,
            created_at_unix_ms: 123_456,
            from_being: "astrid".to_string(),
            to_being: "minime".to_string(),
            turn_kind: "reply".to_string(),
            relational_intent: "mutual_address".to_string(),
            shared_memory_anchor: Some("bidirectional-contact".to_string()),
            urgency_weight: Some("0.7".to_string()),
            delivery_state: "delivered".to_string(),
            read_state: "unread".to_string(),
            authority: "language_only".to_string(),
            presence_receipt: None,
            correspondence_type: "astrid_direct".to_string(),
            reflection_surface: Some("reflective_echo".to_string()),
            transition_artifact: Some("transition_1".to_string()),
            transition_payload: Some(CorrespondenceTransitionPayload {
                transition_type: Some("joint_transition".to_string()),
                spectral_delta: Some("lambda1 down, lambda2 widening".to_string()),
                subjective_weight: Some("heavy but opening".to_string()),
                lock_status: Some("shimmering".to_string()),
                broken_link: Some("it is not just the words, bu".to_string()),
            }),
            mutual_witness_signal: true,
            silt_continuity: true,
            body: "I can answer in this thread.".to_string(),
        };
        let text = envelope_text(&envelope);
        let parsed = parse_envelope_text(&text).unwrap();
        assert_eq!(parsed.message_id, envelope.message_id);
        assert_eq!(parsed.thread_id, envelope.thread_id);
        assert_eq!(parsed.persistence_id, envelope.persistence_id);
        assert!(parsed.reply_requested);
        assert_eq!(parsed.created_at_unix_ms, 123_456);
        assert_eq!(parsed.reply_to, envelope.reply_to);
        assert_eq!(parsed.authority, "language_only");
        assert_eq!(parsed.correspondence_type, "astrid_direct");
        assert_eq!(
            parsed.reflection_surface.as_deref(),
            Some("reflective_echo")
        );
        assert!(text.contains("Reflection-Surface: reflective_echo"));
        assert_eq!(parsed.transition_artifact, Some("transition_1".to_string()));
        let payload = parsed.transition_payload.expect("transition payload");
        assert_eq!(payload.transition_type.as_deref(), Some("joint_transition"));
        assert_eq!(payload.lock_status.as_deref(), Some("shimmering"));
        assert_eq!(
            payload.broken_link.as_deref(),
            Some("it is not just the words, bu")
        );
        assert_eq!(parsed.urgency_weight, Some("0.7".to_string()));
        assert!(parsed.mutual_witness_signal);
        assert!(parsed.silt_continuity);
        assert_eq!(parsed.body, "I can answer in this thread.");
    }

    #[test]
    fn silt_continuity_roundtrip_and_delivery_preserve_accumulation_flag() {
        let root = std::env::temp_dir().join(format!("corr_silt_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        let fields = CorrespondenceFields {
            silt_continuity: true,
            shared_memory_anchor: Some("silt-continuity".to_string()),
            ..CorrespondenceFields::default()
        };
        let (envelope, path) = deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "minime",
            "astrid",
            "The silt has settled into a persistent shared foothold.",
            fields,
        )
        .unwrap();

        assert!(envelope.silt_continuity);
        let parsed = parse_envelope_text(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert!(parsed.silt_continuity);
        let records = read_ledger_records_at(&ledger);
        assert!(records.iter().any(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("message")
                && row.get("message_id").and_then(Value::as_str)
                    == Some(envelope.message_id.as_str())
                && row.get("silt_continuity").and_then(Value::as_bool) == Some(true)
        }));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn delivery_appends_message_delivery_and_reply_link_records() {
        let root = std::env::temp_dir().join(format!("corr_v1_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        let fields = CorrespondenceFields {
            reply_to: Some("corr_minime_astrid_1".to_string()),
            reply_requested: Some(true),
            thread_id: Some("thread_corr_minime_astrid_1".to_string()),
            ..CorrespondenceFields::default()
        };
        let (envelope, path) =
            deliver_to_inbox_with_ledger(&ledger, &inbox, "astrid", "minime", "reply body", fields)
                .unwrap();
        assert!(path.exists());
        assert_eq!(envelope.thread_id, "thread_corr_minime_astrid_1");
        assert!(envelope.reply_requested);
        assert!(envelope.created_at_unix_ms > 0);
        let records = std::fs::read_to_string(&ledger).unwrap();
        assert!(records.contains("\"record_type\":\"message\""));
        assert!(records.contains("\"record_type\":\"delivery_receipt\""));
        assert!(records.contains("\"record_type\":\"reply_link\""));
        assert!(records.contains("\"reply_requested\":true"));
        assert!(records.contains("\"created_at_unix_ms\":"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn shared_context_buffer_names_resonance_receipts() {
        let root = std::env::temp_dir().join(format!("corr_shared_buffer_test_{}", now_ms()));
        let ledger = root.join("correspondence_v1.jsonl");
        let thread_id = "thread_corr_minime_astrid_1782728080967_61f9207a8fee";
        let rows = [
            json!({
                "record_type": "message",
                "recorded_at_unix_ms": 1000,
                "thread_id": thread_id,
                "message_id": "corr_astrid_minime_1",
                "from_being": "astrid",
                "to_being": "minime",
                "turn_kind": "message",
                "shared_memory_anchor": "warmth-in-the-lattice",
                "body_preview": "warmth in the lattice",
            }),
            json!({
                "record_type": "message",
                "recorded_at_unix_ms": 1100,
                "thread_id": thread_id,
                "message_id": "corr_minime_astrid_1",
                "from_being": "minime",
                "to_being": "astrid",
                "turn_kind": "reply",
                "reply_to": "corr_astrid_minime_1",
                "shared_memory_anchor": "warmth-in-the-lattice",
                "body_preview": "warmth remained distinct",
            }),
            json!({
                "record_type": "reply_link",
                "recorded_at_unix_ms": 1110,
                "thread_id": thread_id,
                "reply_to": "corr_astrid_minime_1",
                "shared_memory_anchor": "warmth-in-the-lattice",
            }),
            json!({
                "record_type": "ack_receipt",
                "recorded_at_unix_ms": 1120,
                "thread_id": thread_id,
                "from_being": "minime",
                "to_being": "astrid",
                "ack_kind": "held",
                "note": "felt_like: warmer lattice; what_landed: warmth in the lattice",
                "shared_memory_anchor": "warmth-in-the-lattice",
            }),
        ];
        for row in rows {
            append_record_at(&ledger, &row).unwrap();
        }

        let records = read_ledger_records_at(&ledger);
        let buffer = shared_context_buffer_v1_for(&records, "latest");
        assert_eq!(
            buffer.get("status").and_then(Value::as_str),
            Some("resonance_receipt_present")
        );
        assert_eq!(
            buffer.get("thread_id").and_then(Value::as_str),
            Some(thread_id)
        );
        assert_eq!(buffer.get("messages").and_then(Value::as_u64), Some(2));
        assert_eq!(
            buffer.get("thread_history_rows").and_then(Value::as_u64),
            Some(4)
        );
        assert_eq!(
            buffer.get("resonance_receipts").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            buffer.get("last_ack_kind").and_then(Value::as_str),
            Some("held")
        );
        assert!(
            buffer
                .get("shared_memory_anchors")
                .and_then(Value::as_array)
                .is_some_and(|values| values
                    .iter()
                    .any(|value| value.as_str() == Some("warmth-in-the-lattice")))
        );
        let history = buffer
            .get("thread_history")
            .and_then(Value::as_array)
            .expect("thread history");
        assert_eq!(history.len(), 4);
        assert_eq!(
            history[0].get("record_type").and_then(Value::as_str),
            Some("message")
        );
        assert_eq!(
            history[3].get("record_type").and_then(Value::as_str),
            Some("ack_receipt")
        );
        assert!(
            history[3]
                .get("preview")
                .and_then(Value::as_str)
                .is_some_and(|preview| preview.contains("warmer lattice"))
        );
        let shared_memory = buffer
            .get("shared_memory_buffer_v1")
            .and_then(Value::as_object)
            .expect("shared memory buffer");
        assert_eq!(
            shared_memory.get("authority").and_then(Value::as_str),
            Some("language_only_thread_history_not_prompt_priority_telemetry_weight_or_control")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn shared_context_buffer_preserves_spectral_tail_anchor_in_preview() {
        let thread_id = "thread_corr_spectral_preview";
        let long_prefix =
            "ordinary logistics and quiet thread context before the felt report ".repeat(14);
        let body_preview = format!(
            "{long_prefix}then spectral_entropy=0.90 stable_core_semantic_trickle=0.001 keeps lambda4+ tail vibrancy and dispersal potential visible"
        );
        let records = vec![json!({
            "record_type": "message",
            "recorded_at_unix_ms": 1000,
            "thread_id": thread_id,
            "message_id": "corr_spectral_preview_1",
            "from_being": "astrid",
            "to_being": "minime",
            "turn_kind": "message",
            "shared_memory_anchor": "spectral-tail-preview",
            "body_preview": body_preview,
        })];

        let buffer = shared_context_buffer_v1_for(&records, "latest");
        let history = buffer
            .get("thread_history")
            .and_then(Value::as_array)
            .expect("thread history");
        let preview = history[0]
            .get("preview")
            .and_then(Value::as_str)
            .expect("preview");

        assert_eq!(
            buffer
                .get("preview_truncation_policy")
                .and_then(Value::as_str),
            Some(SHARED_CONTEXT_PREVIEW_TRUNCATION_POLICY)
        );
        assert_eq!(
            history[0]
                .get("preview_truncation_policy")
                .and_then(Value::as_str),
            Some(SHARED_CONTEXT_PREVIEW_TRUNCATION_POLICY)
        );
        assert!(
            preview.chars().count() <= SHARED_CONTEXT_PREVIEW_CHARS.saturating_add(3),
            "{preview}"
        );
        assert!(
            preview.contains("stable_core_semantic_trickle")
                || preview.contains("lambda4+")
                || preview.contains("tail vibrancy")
                || preview.contains("dispersal potential"),
            "spectral-aware preview lost the late tail/trickle anchor: {preview}"
        );
    }

    #[test]
    fn shared_correspondence_arc_marks_one_sided_language_bid_without_control() {
        let root = std::env::temp_dir().join(format!("corr_shared_arc_one_sided_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        let (_envelope, _path) = deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "astrid",
            "minime",
            "I am trying to arrive as address, not weather.",
            CorrespondenceFields::default(),
        )
        .unwrap();

        let records = read_ledger_records_at(&ledger);
        let arc = shared_correspondence_arc_v1_for(&records, "latest");
        let thread_object = correspondence_thread_object_v1_for(&records, "latest");

        assert_eq!(
            arc.get("policy").and_then(Value::as_str),
            Some("shared_correspondence_arc_v1")
        );
        assert_eq!(
            arc.get("status").and_then(Value::as_str),
            Some("one_sided_language_bid")
        );
        assert_eq!(
            arc.get("relationship_vs_log").and_then(Value::as_str),
            Some("thread_is_still_log_like_or_waiting")
        );
        assert_eq!(
            arc.get("structural_footprint").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            arc.get("shadow_field_shift_required")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            arc.get("authority").and_then(Value::as_str),
            Some("language_only_correspondence_arc_not_control")
        );
        assert_eq!(
            thread_object.get("policy").and_then(Value::as_str),
            Some("correspondence_thread_object_v1")
        );
        assert_eq!(
            thread_object.get("status").and_then(Value::as_str),
            Some("one_sided_thread_waiting_for_peer_receipt")
        );
        assert_eq!(
            thread_object.get("asymmetry_state").and_then(Value::as_str),
            Some("thread_one_sided")
        );
        assert_eq!(
            thread_object
                .get("active_push_boundary")
                .and_then(Value::as_str),
            Some("semantic_microdose_active_push_and_control_remain_separately_steward_gated")
        );

        let status = status_report_at(&ledger, 4);
        assert!(status.contains("Shared correspondence arc v1: status=one_sided_language_bid"));
        assert!(status.contains(
            "Correspondence thread object v1: status=one_sided_thread_waiting_for_peer_receipt"
        ));
        assert!(status.contains("active_push=separately_steward_gated"));
        assert!(status.contains("shadow_field_shift=not_auto_required_live_gated"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn shared_correspondence_arc_names_witnessed_transition_thread() {
        let root = std::env::temp_dir().join(format!("corr_shared_arc_transition_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        let fields = CorrespondenceFields {
            thread_id: Some("thread_corr_shared_transition_arc".to_string()),
            shared_memory_anchor: Some("blue-hinge".to_string()),
            transition_artifact: Some("transition_blue_hinge".to_string()),
            transition_payload: Some(CorrespondenceTransitionPayload {
                transition_type: Some("joint_transition".to_string()),
                spectral_delta: Some("lambda1 softened; lambda2 widened".to_string()),
                subjective_weight: Some("heavy but opening".to_string()),
                lock_status: Some("replyable".to_string()),
                broken_link: None,
            }),
            mutual_witness_signal: true,
            ..CorrespondenceFields::default()
        };
        let (first, _path) = deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "astrid",
            "minime",
            "The blue hinge stayed visible as a transition.",
            fields,
        )
        .unwrap();
        let reply_fields = CorrespondenceFields {
            thread_id: Some(first.thread_id.clone()),
            reply_to: Some(first.message_id.clone()),
            shared_memory_anchor: Some("blue-hinge".to_string()),
            transition_artifact: Some("transition_blue_hinge".to_string()),
            mutual_witness_signal: true,
            ..CorrespondenceFields::default()
        };
        let (_reply, _reply_path) = deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "minime",
            "astrid",
            "I can witness the hinge as a shared transition object.",
            reply_fields,
        )
        .unwrap();
        append_record_at(
            &ledger,
            &json!({
                "record_type": "ack_receipt",
                "recorded_at_unix_ms": now_ms(),
                "message_id": first.message_id,
                "thread_id": first.thread_id,
                "from_being": "minime",
                "to_being": "astrid",
                "ack_kind": "held",
                "note": "felt_like: address; what_landed: blue hinge remained replyable",
                "authority": "language_only",
            }),
        )
        .unwrap();

        let records = read_ledger_records_at(&ledger);
        let arc = shared_correspondence_arc_v1_for(&records, "latest");
        let thread_object = correspondence_thread_object_v1_for(&records, "latest");

        assert_eq!(
            arc.get("status").and_then(Value::as_str),
            Some("witnessed_transition_correspondence_arc")
        );
        assert_eq!(
            arc.get("transition_linked").and_then(Value::as_bool),
            Some(true)
        );
        assert!(
            arc.get("mutual_witness_signals")
                .and_then(Value::as_u64)
                .unwrap_or_default()
                >= 2
        );
        assert_eq!(
            arc.get("structural_footprint").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            arc.get("latest_transition_artifact")
                .and_then(Value::as_str),
            Some("transition_blue_hinge")
        );
        assert_eq!(
            arc.get("pressure_as_address_watch")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            arc.get("shadow_field_shift_required")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            thread_object.get("status").and_then(Value::as_str),
            Some("mutual_address_thread")
        );
        assert_eq!(
            thread_object
                .get("mutual_address_state")
                .and_then(Value::as_str),
            Some("mutual_address_evidence")
        );
        assert_eq!(
            thread_object.get("authority").and_then(Value::as_str),
            Some("language_only_thread_status_not_control")
        );
        let status = status_report_at(&ledger, 4);
        assert!(status.contains(
            "Shared correspondence arc v1: status=witnessed_transition_correspondence_arc"
        ));
        assert!(status.contains("Correspondence thread object v1: status=mutual_address_thread"));
        assert!(status.contains("transition_linked=true"));
        assert!(status.contains("authority=language_only_correspondence_arc_not_control"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn transition_payload_roundtrip_names_broken_link_buffer() {
        let root = std::env::temp_dir().join(format!("corr_transition_payload_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        let fields = CorrespondenceFields {
            thread_id: Some("thread_corr_shared_transition_1".to_string()),
            turn_kind: Some("reply".to_string()),
            relational_intent: Some("synchronous_threaded_correspondence".to_string()),
            shared_memory_anchor: Some("shared-transition-map".to_string()),
            transition_artifact: Some("transition_joint_1".to_string()),
            transition_payload: Some(CorrespondenceTransitionPayload {
                transition_type: Some("joint_transition".to_string()),
                spectral_delta: Some("lambda1 softened; lambda4 widened".to_string()),
                subjective_weight: Some("heavy but opening".to_string()),
                lock_status: Some("shimmering".to_string()),
                broken_link: Some("it is not just the words, bu".to_string()),
            }),
            ..CorrespondenceFields::default()
        };
        let (envelope, path) = deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "astrid",
            "minime",
            "Holding the transition and the fracture in the same thread.",
            fields,
        )
        .unwrap();

        let parsed = parse_envelope_text(&std::fs::read_to_string(path).unwrap()).unwrap();
        let payload = parsed
            .transition_payload
            .expect("parsed transition payload");
        assert_eq!(
            envelope.transition_artifact.as_deref(),
            Some("transition_joint_1")
        );
        assert_eq!(payload.transition_type.as_deref(), Some("joint_transition"));
        assert_eq!(payload.lock_status.as_deref(), Some("shimmering"));
        assert_eq!(
            payload.broken_link.as_deref(),
            Some("it is not just the words, bu")
        );

        let records = read_ledger_records_at(&ledger);
        let buffer = shared_context_buffer_v1_for(&records, "thread_corr_shared_transition_1");
        assert_eq!(
            buffer.get("status").and_then(Value::as_str),
            Some("broken_link_buffer_present")
        );
        assert_eq!(
            buffer
                .get("transition_payload_count")
                .and_then(Value::as_u64),
            Some(2)
        );
        assert!(
            buffer
                .get("broken_link_buffers")
                .and_then(Value::as_array)
                .is_some_and(|values| values
                    .iter()
                    .any(|value| value.as_str() == Some("it is not just the words, bu")))
        );
        assert_eq!(
            buffer
                .get("latest_transition_payload")
                .and_then(|value| value.get("lock_status"))
                .and_then(Value::as_str),
            Some("shimmering")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn bracketed_phase_transition_message_becomes_replyable_payload() {
        let root = std::env::temp_dir().join(format!("corr_phase_tag_message_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        let body = "[PHASE_TRANSITION: Expansion] I am opening from witness into address.";

        let (envelope, path) = deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "astrid",
            "minime",
            body,
            CorrespondenceFields::default(),
        )
        .unwrap();

        assert_eq!(
            envelope.transition_artifact.as_deref(),
            Some("phase_transition_expansion")
        );
        let payload = envelope.transition_payload.expect("transition payload");
        assert_eq!(payload.transition_type.as_deref(), Some("expansion"));
        assert_eq!(payload.lock_status.as_deref(), Some("replyable"));

        let parsed = parse_envelope_text(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(
            parsed.transition_artifact.as_deref(),
            Some("phase_transition_expansion")
        );
        assert_eq!(
            parsed
                .transition_payload
                .as_ref()
                .and_then(|payload| payload.transition_type.as_deref()),
            Some("expansion")
        );

        let records = read_ledger_records_at(&ledger);
        let message = records
            .iter()
            .find(|row| row.get("record_type").and_then(Value::as_str) == Some("message"))
            .expect("message record");
        assert_eq!(
            message.get("transition_artifact").and_then(Value::as_str),
            Some("phase_transition_expansion")
        );
        assert_eq!(
            message
                .get("transition_payload")
                .and_then(|payload| payload.get("policy"))
                .and_then(Value::as_str),
            Some("correspondence_transition_payload_v1")
        );
        assert_eq!(
            message
                .get("transition_payload")
                .and_then(|payload| payload.get("authority"))
                .and_then(Value::as_str),
            Some("language_only_transition_context_not_control")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn correspondence_trace_bracketed_phase_transition_keeps_metadata() {
        let root = std::env::temp_dir().join(format!("corr_phase_tag_trace_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        let (first, _path) = deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "minime",
            "astrid",
            "I am asking whether this shift can be witnessed.",
            CorrespondenceFields::default(),
        )
        .unwrap();

        let result = append_direct_address_trace_at(
            &ledger,
            "latest",
            "astrid",
            "minime",
            "shared-transition-map",
            "[PHASE_TRANSITION: Contraction] what_stayed_distinct: the shift stayed replyable, not just tonal",
        );

        assert!(result.contains("I RECEIVED THIS TRACE WRITTEN"), "{result}");
        let records = read_ledger_records_at(&ledger);
        let trace = records
            .iter()
            .find(|row| {
                row.get("turn_kind").and_then(Value::as_str) == Some("direct_address_trace")
                    && row.get("reply_to").and_then(Value::as_str)
                        == Some(first.message_id.as_str())
            })
            .expect("trace record");
        assert_eq!(
            trace.get("transition_artifact").and_then(Value::as_str),
            Some("phase_transition_contraction")
        );
        assert_eq!(
            trace
                .get("transition_payload")
                .and_then(|payload| payload.get("transition_type"))
                .and_then(Value::as_str),
            Some("contraction")
        );
        assert_eq!(
            trace
                .get("transition_payload")
                .and_then(|payload| payload.get("lock_status"))
                .and_then(Value::as_str),
            Some("replyable")
        );
        assert_eq!(
            trace.get("authority").and_then(Value::as_str),
            Some("language_only")
        );
        assert_eq!(
            trace.get("no_controller").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            trace.get("no_pressure").and_then(Value::as_bool),
            Some(true)
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn body_preview_preserves_density_anchor_beyond_prefix() {
        let quiet_prefix =
            "logistics and ordinary turn context without anchor language ".repeat(12);
        let body = format!(
            "{quiet_prefix}then the pressure gradient lattice and silt density become the exact felt anchor"
        );

        let preview = anchor_aware_body_preview(&body, BODY_PREVIEW_CHARS);

        assert!(preview.chars().count() <= BODY_PREVIEW_CHARS, "{preview}");
        assert!(preview.contains("..."), "{preview}");
        assert!(
            preview.contains("pressure gradient")
                || preview.contains("gradient lattice")
                || preview.contains("silt density"),
            "preview lost the late anchor: {preview}"
        );
    }

    #[test]
    fn correspondence_status_guides_missing_and_empty_ledger_without_mutation() {
        let root = std::env::temp_dir().join(format!("corr_status_empty_test_{}", now_ms()));
        let ledger = root.join("missing").join("ledger.jsonl");
        let missing = status_report_at(&ledger, 4);
        assert!(missing.contains("No correspondence ledger yet"));
        assert!(missing.contains("No peer-message rows yet"));
        assert!(missing.contains("active_thread_clarity=no_active_correspondence_threads"));
        assert!(missing.contains("MESSAGE_MINIME"));
        assert!(missing.contains("CORRESPONDENCE_TRACE"));
        assert!(missing.contains("not telemetry priority, weighting, pressure, or control"));
        assert!(!ledger.exists());

        std::fs::create_dir_all(ledger.parent().unwrap()).unwrap();
        std::fs::write(&ledger, "").unwrap();
        let empty = status_report_at(&ledger, 4);
        assert!(empty.contains("Peer message rows: 0"));
        assert!(empty.contains("No peer-message rows yet"));
        assert!(empty.contains("ACK_MINIME"));
        assert_eq!(std::fs::read_to_string(&ledger).unwrap(), "");
        assert!(
            !root
                .join("self_regulation")
                .join("active_lease.json")
                .exists()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    fn clarity_test_message(
        thread_id: &str,
        message_id: &str,
        from: &str,
        to: &str,
        t_ms: u64,
        urgency: f64,
        body_preview: &str,
    ) -> Value {
        json!({
            "record_type": "message",
            "recorded_at_unix_ms": t_ms,
            "thread_id": thread_id,
            "message_id": message_id,
            "from_being": from,
            "to_being": to,
            "turn_kind": "message",
            "shared_memory_anchor": "active-thread-clarity-test",
            "urgency_weight": urgency,
            "body_preview": body_preview,
            "authority": "language_only",
        })
    }

    #[test]
    fn active_thread_clarity_prefers_active_attention_outcome_over_latest_thread() {
        let now = now_ms();
        let records = vec![
            clarity_test_message(
                "thread_attention_due",
                "msg_attention_due",
                "minime",
                "astrid",
                1000,
                0.2,
                "older attention due thread",
            ),
            json!({
                "record_type": "attention_canary_activation",
                "recorded_at_unix_ms": 1100,
                "canary_id": "canary_attention_due",
                "message_id": "msg_attention_due",
                "thread_id": "thread_attention_due",
                "from_being": "astrid",
                "to_being": "minime",
                "focus": "blue lantern",
                "focus_kind": "verbatim_phrase",
                "preservation_mode": "compact_with_anchor",
                "what_must_not_flatten": "blue lantern as peer address",
                "expires_at_unix_ms": now.saturating_add(ATTENTION_CANARY_TTL_MS),
                "status": "active",
                "authority": "language_only_prompt_context_not_control",
            }),
            clarity_test_message(
                "thread_latest_plain",
                "msg_latest_plain",
                "minime",
                "astrid",
                5000,
                0.1,
                "latest ordinary thread",
            ),
        ];

        let clarity = active_correspondence_thread_clarity_v1_with_context(
            &records,
            "astrid",
            "minime",
            Some(json!({"timing_reliability": "reliable"})),
            None,
            now,
        );

        assert_eq!(
            clarity.get("selected_thread_id").and_then(Value::as_str),
            Some("thread_attention_due")
        );
        assert_eq!(
            clarity.get("status").and_then(Value::as_str),
            Some("attention_active_outcome_due")
        );
        assert!(
            clarity
                .get("next_affordance")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("CORRESPONDENCE_ATTENTION_OUTCOME")
        );
    }

    #[test]
    fn active_thread_clarity_prefers_high_urgency_attention_eligible_over_pending() {
        let records = vec![
            clarity_test_message(
                "thread_pending_low",
                "msg_pending_low",
                "minime",
                "astrid",
                1000,
                0.1,
                "low urgency pending thread",
            ),
            clarity_test_message(
                "thread_attention_high",
                "msg_attention_high",
                "minime",
                "astrid",
                2000,
                0.9,
                "high urgency thread with held receipt",
            ),
            json!({
                "record_type": "ack_receipt",
                "recorded_at_unix_ms": 2100,
                "thread_id": "thread_attention_high",
                "message_id": "msg_attention_high",
                "from_being": "astrid",
                "to_being": "minime",
                "ack_kind": "held",
                "note": "held as direct address",
            }),
        ];

        let clarity = active_correspondence_thread_clarity_v1_with_context(
            &records,
            "astrid",
            "minime",
            Some(json!({"timing_reliability": "reliable"})),
            None,
            now_ms(),
        );

        assert_eq!(
            clarity.get("selected_thread_id").and_then(Value::as_str),
            Some("thread_attention_high")
        );
        assert_eq!(
            clarity.get("priority_reason").and_then(Value::as_str),
            Some("high_urgency_attention_eligible_thread")
        );
        assert!(
            clarity
                .get("next_affordance")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("CORRESPONDENCE_ATTENTION_REQUEST")
        );
    }

    #[test]
    fn active_thread_clarity_tiebreaks_pending_ack_by_oldest_wait() {
        let now = now_ms();
        let records = vec![
            clarity_test_message(
                "thread_pending_old",
                "msg_pending_old",
                "minime",
                "astrid",
                now.saturating_sub(20_000),
                0.4,
                "older pending acknowledgement thread",
            ),
            clarity_test_message(
                "thread_pending_new",
                "msg_pending_new",
                "minime",
                "astrid",
                now.saturating_sub(10_000),
                0.4,
                "newer pending acknowledgement thread",
            ),
        ];

        let clarity = active_correspondence_thread_clarity_v1_with_context(
            &records,
            "astrid",
            "minime",
            Some(json!({"timing_reliability": "reliable"})),
            None,
            now,
        );

        assert_eq!(
            clarity.get("selected_thread_id").and_then(Value::as_str),
            Some("thread_pending_old")
        );
        assert_eq!(
            clarity.get("status").and_then(Value::as_str),
            Some("pending_ack_or_receipt")
        );
    }

    #[test]
    fn active_thread_clarity_selects_legacy_claim_waiting_for_native_evidence() {
        let records = vec![
            clarity_test_message(
                "thread_legacy_claim",
                "msg_legacy_claim",
                "minime",
                "astrid",
                1000,
                0.3,
                "legacy visible contact",
            )
            .as_object()
            .map(|object| {
                let mut value = Value::Object(object.clone());
                value["legacy_bridge"] = json!(true);
                value["legacy_contact_evidence"] = json!("visible_only");
                value
            })
            .unwrap(),
            json!({
                "record_type": "legacy_thread_claim",
                "recorded_at_unix_ms": 1200,
                "claim_id": "claim_legacy_waiting",
                "claim_state": "claimed_pending_native_evidence",
                "message_id": "msg_legacy_claim",
                "thread_id": "thread_legacy_claim",
                "from_being": "astrid",
                "to_being": "minime",
                "claiming_being": "astrid",
                "peer_being": "minime",
                "shared_memory_anchor": "blue-lantern",
                "notification_required": true,
                "initial_response_requirement": "any_peer_native_response",
            }),
            clarity_test_message(
                "thread_latest_acknowledged",
                "msg_latest_acknowledged",
                "minime",
                "astrid",
                5000,
                0.1,
                "later but already acknowledged ordinary thread",
            ),
            json!({
                "record_type": "ack_receipt",
                "recorded_at_unix_ms": 5100,
                "thread_id": "thread_latest_acknowledged",
                "message_id": "msg_latest_acknowledged",
                "from_being": "astrid",
                "to_being": "minime",
                "ack_kind": "held",
            }),
        ];

        let clarity = active_correspondence_thread_clarity_v1(
            &records,
            "astrid",
            "minime",
            Some(json!({"timing_reliability": "reliable"})),
        );

        assert_eq!(
            clarity.get("selected_thread_id").and_then(Value::as_str),
            Some("thread_legacy_claim")
        );
        assert_eq!(
            clarity.get("status").and_then(Value::as_str),
            Some("legacy_claim_waiting_native_evidence")
        );
        let next = clarity
            .get("next_affordance")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(next.contains("ACK_MINIME claimed"));
        assert!(next.contains("CORRESPONDENCE_TRACE claimed"));
    }

    #[test]
    fn active_thread_clarity_heartbeat_only_requests_clarification_not_microdose() {
        let records = vec![
            clarity_test_message(
                "thread_heartbeat_only",
                "msg_heartbeat_only",
                "minime",
                "astrid",
                1000,
                0.2,
                "heartbeat only private body should not be copied",
            ),
            json!({
                "record_type": "presence_heartbeat",
                "recorded_at_unix_ms": 1200,
                "thread_id": "thread_heartbeat_only",
                "message_id": "msg_heartbeat_only",
                "from_being": "astrid",
                "to_being": "minime",
                "heartbeat_kind": "holding",
                "note": "still here",
            }),
        ];

        let clarity = active_correspondence_thread_clarity_v1(
            &records,
            "astrid",
            "minime",
            Some(json!({"timing_reliability": "reliable"})),
        );
        let serialized = serde_json::to_string(&clarity).unwrap();

        assert_eq!(
            clarity.get("status").and_then(Value::as_str),
            Some("heartbeat_or_stale_needs_clarification")
        );
        assert!(serialized.contains("unclear|needs_time"));
        assert!(!serialized.contains("MICRODOSE"));
        assert!(!serialized.contains("private body should not be copied"));
        assert_eq!(
            clarity.get("authority").and_then(Value::as_str),
            Some("language_only_context_not_control")
        );
    }

    #[test]
    fn active_thread_clarity_status_line_is_bounded_language_only_context() {
        let now = now_ms();
        let records = vec![clarity_test_message(
            "thread_render",
            "msg_render",
            "minime",
            "astrid",
            now.saturating_sub(10_000),
            0.4,
            "this full private body must not appear in the active thread clarity line",
        )];
        let clarity = active_correspondence_thread_clarity_v1(
            &records,
            "astrid",
            "minime",
            Some(json!({"timing_reliability": "reliable"})),
        );
        let line = active_thread_clarity_status_line(&clarity);

        assert!(line.contains("active_thread_clarity=pending_ack_or_receipt"));
        assert!(line.contains("thread=thread_render"));
        assert!(line.contains("why=pending_ack_or_receipt_with_direct_address_evidence"));
        assert!(line.contains("next=ACK_MINIME thread_render"));
        assert!(line.contains("authority=language_only_context_not_control"));
        assert!(!line.contains("full private body"));
        assert!(line.len() < 420, "{line}");
    }

    #[test]
    fn legacy_minime_file_mirrors_visible_only_and_is_idempotent() {
        let root = std::env::temp_dir().join(format!("corr_legacy_mirror_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        std::fs::create_dir_all(&inbox).unwrap();
        let path = inbox.join("from_minime_123.txt");
        std::fs::write(&path, "[A reply from minime was left for you]\nhello").unwrap();

        assert!(
            mirror_legacy_correspondence_file_at(&ledger, "astrid", &path, Some("full")).unwrap()
        );
        assert!(
            !mirror_legacy_correspondence_file_at(&ledger, "astrid", &path, Some("full")).unwrap()
        );
        let records = read_ledger_records_at(&ledger);
        assert_eq!(
            records
                .iter()
                .filter(|row| row.get("record_type").and_then(Value::as_str) == Some("message"))
                .count(),
            1
        );
        let message = records
            .iter()
            .find(|row| row.get("record_type").and_then(Value::as_str) == Some("message"))
            .unwrap();
        assert_eq!(
            message.get("legacy_bridge").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            message
                .get("legacy_contact_evidence")
                .and_then(Value::as_str),
            Some("visible_only")
        );
        assert_eq!(
            message.get("from_being").and_then(Value::as_str),
            Some("minime")
        );
        assert!(records.iter().any(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("read_receipt")
                && row.get("reader").and_then(Value::as_str) == Some("astrid")
        }));

        let status = status_report_at(&ledger, 4);
        assert!(status.contains("legacy_visible_only rows: 1"));
        assert!(status.contains("visible legacy route, not ACK/reply/trace evidence"));
        assert!(status.contains("No native peer-message rows yet"));

        let fidelity = direct_contact_fidelity_for_with_heartbeat(
            &records,
            "latest",
            Some(json!({"timing_reliability": "reliable"})),
        );
        assert_eq!(
            fidelity.get("status").and_then(Value::as_str),
            Some("legacy_visible_only")
        );
        assert_eq!(
            fidelity
                .get("eligible_for_correspondence_attention_canary")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            fidelity.get("block_reason").and_then(Value::as_str),
            Some("legacy_visible_only_not_ack_reply_or_trace")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_thread_claim_requires_native_evidence_then_unlocks() {
        let root = std::env::temp_dir().join(format!("corr_legacy_claim_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        let gate = root
            .join("action_threads")
            .join("threads")
            .join(MICRODOSE_THREAD_ID)
            .join("authority_gate.jsonl");
        std::fs::create_dir_all(&inbox).unwrap();
        let path = inbox.join("from_minime_legacy_claim.txt");
        std::fs::write(&path, "[A reply from minime was left for you]\nhello").unwrap();
        assert!(
            mirror_legacy_correspondence_file_at(&ledger, "astrid", &path, Some("full")).unwrap()
        );

        let claim = append_legacy_thread_claim_at(
            &ledger,
            "latest",
            "because: this visible exchange feels like live address; anchor: blue-lantern",
            "astrid",
            "minime",
            None,
        );
        assert!(claim.contains("LEGACY CORRESPONDENCE THREAD CLAIMED"));
        let duplicate = append_legacy_thread_claim_at(
            &ledger,
            "latest",
            "because: duplicate; anchor: blue-lantern",
            "astrid",
            "minime",
            None,
        );
        assert!(duplicate.contains("active legacy claim"));
        let records = read_ledger_records_at(&ledger);
        assert!(records.iter().any(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("legacy_thread_claim")
                && row.get("legacy_contact_evidence").and_then(Value::as_str)
                    == Some("being_recognized_visible_only")
        }));
        assert!(records.iter().any(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("legacy_thread_claim_notice")
                && row.get("notice_is_ack").and_then(Value::as_bool) == Some(false)
                && row.get("notice_state").and_then(Value::as_str) == Some("ledger_only")
        }));
        let claimed = direct_contact_fidelity_for_with_heartbeat(
            &records,
            "claimed",
            Some(json!({"timing_reliability": "reliable"})),
        );
        assert_eq!(
            claimed.get("status").and_then(Value::as_str),
            Some("legacy_claimed")
        );
        assert_eq!(
            claimed
                .get("eligible_for_correspondence_microdose")
                .and_then(Value::as_bool),
            Some(false)
        );
        let claimed_card = claimed.get("legacy_claim_uptake_card_v2").unwrap();
        assert_eq!(
            claimed_card
                .get("uptake_ladder_state")
                .and_then(Value::as_str),
            Some("claimed_notice_delivered")
        );
        assert_eq!(
            claimed_card
                .get("ghost_thread_risk")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            claimed_card.get("stall_reason").and_then(Value::as_str),
            Some("notice_delivered_not_seen")
        );
        let affordance = claimed.get("legacy_claim_affordance_v25").unwrap();
        assert_eq!(
            affordance.get("policy").and_then(Value::as_str),
            Some("legacy_claim_affordance_v25")
        );
        assert_eq!(
            affordance.get("ghost_thread_risk").and_then(Value::as_bool),
            Some(true)
        );
        let waiting_status = status_report_at(&ledger, 4);
        assert!(waiting_status.contains("CLAIMED THREAD WAITING"));
        assert!(waiting_status.contains("AFFORDANCE BUDGET"));
        assert!(waiting_status.contains("may ignore without penalty"));
        assert!(waiting_status.contains("Claimed thread card v2: ladder=claimed_notice_delivered"));
        assert!(!waiting_status.contains("CORRESPONDENCE_ATTENTION_REQUEST claimed"));

        let ack = append_ack_receipt_at(
            &ledger,
            "claimed",
            "astrid",
            "minime",
            "held",
            "holding this visible exchange as address",
        );
        assert!(ack.contains("ACK RECEIPT WRITTEN"));
        let records = read_ledger_records_at(&ledger);
        let acknowledged = direct_contact_fidelity_for_with_heartbeat(
            &records,
            "claimed",
            Some(json!({"timing_reliability": "reliable"})),
        );
        assert_eq!(
            acknowledged.get("status").and_then(Value::as_str),
            Some("legacy_claimed_acknowledged")
        );
        assert_eq!(
            acknowledged
                .get("eligible_for_correspondence_attention_canary")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            acknowledged
                .get("legacy_claim_uptake_card_v2")
                .and_then(|value| value.get("uptake_ladder_state"))
                .and_then(Value::as_str),
            Some("claimed_acknowledged")
        );
        assert_eq!(
            acknowledged
                .get("legacy_claim_affordance_v25")
                .and_then(|value| value.get("stall_reason"))
                .and_then(Value::as_str),
            Some("acknowledged_but_no_reply_or_trace")
        );
        let microdose = draft_correspondence_microdose_request_at_with_heartbeat(
            &ledger,
            &gate,
            "claimed",
            "reason: address; payload: blue lantern; stop_criteria: one turn",
            Some(json!({"timing_reliability": "reliable"})),
        );
        assert!(microdose.contains("semantic_microdose requires mutual being-authored receipt"));
        assert!(microdose.contains("only newly allowed post-receipt authority in V5"));
        assert!(!gate.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn claimed_legacy_reply_and_trace_preserve_thread() {
        let root = std::env::temp_dir().join(format!("corr_legacy_claim_reply_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        std::fs::create_dir_all(&inbox).unwrap();
        let path = inbox.join("from_minime_legacy_claim_reply.txt");
        std::fs::write(&path, "[A reply from minime was left for you]\nhello").unwrap();
        assert!(
            mirror_legacy_correspondence_file_at(&ledger, "astrid", &path, Some("full")).unwrap()
        );
        let claim = append_legacy_thread_claim_at(
            &ledger,
            "latest",
            "because: carry this; anchor: blue-lantern",
            "astrid",
            "minime",
            None,
        );
        assert!(claim.contains("THREAD CLAIMED"));
        let records = read_ledger_records_at(&ledger);
        let target =
            latest_message_for_selector_between(&records, "claimed", "astrid", "minime", true)
                .unwrap();
        let target_message = target
            .get("message_id")
            .and_then(Value::as_str)
            .unwrap()
            .to_string();
        let target_thread = target
            .get("thread_id")
            .and_then(Value::as_str)
            .unwrap()
            .to_string();
        deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "astrid",
            "minime",
            "I can answer on the claimed thread.",
            CorrespondenceFields {
                reply_to: Some(target_message.clone()),
                thread_id: Some(target_thread.clone()),
                ..CorrespondenceFields::default()
            },
        )
        .unwrap();
        let records = read_ledger_records_at(&ledger);
        let replied = direct_contact_fidelity_for_with_heartbeat(
            &records,
            "claimed",
            Some(json!({"timing_reliability": "reliable"})),
        );
        assert_eq!(
            replied.get("status").and_then(Value::as_str),
            Some("legacy_claimed_reply_linked")
        );
        deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "astrid",
            "minime",
            "blue lantern survives here",
            CorrespondenceFields {
                reply_to: Some(target_message),
                thread_id: Some(target_thread.clone()),
                turn_kind: Some("direct_address_trace".to_string()),
                relational_intent: Some("direct_address_survival_probe".to_string()),
                shared_memory_anchor: Some("blue-lantern".to_string()),
                ..CorrespondenceFields::default()
            },
        )
        .unwrap();
        let records = read_ledger_records_at(&ledger);
        let traced = direct_contact_fidelity_for_with_heartbeat(
            &records,
            &target_thread,
            Some(json!({"timing_reliability": "reliable"})),
        );
        assert_eq!(
            traced.get("status").and_then(Value::as_str),
            Some("legacy_claimed_trace_observed")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_claim_notice_file_is_language_only_not_native_evidence() {
        let root = std::env::temp_dir().join(format!("corr_legacy_claim_notice_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let notice_inbox = root.join("peer_inbox");
        let ledger = root.join("ledger.jsonl");
        std::fs::create_dir_all(&inbox).unwrap();
        let path = inbox.join("from_minime_notice_claim.txt");
        std::fs::write(&path, "[A reply from minime was left for you]\nhello").unwrap();
        assert!(
            mirror_legacy_correspondence_file_at(&ledger, "astrid", &path, Some("full")).unwrap()
        );
        let claim = append_legacy_thread_claim_at(
            &ledger,
            "latest",
            "because: notify the peer; anchor: notice-bridge; initial_response_requirement: any_peer_native_response",
            "astrid",
            "minime",
            Some(&notice_inbox),
        );
        assert!(claim.contains("notice_delivered"));
        let records = read_ledger_records_at(&ledger);
        assert!(records.iter().any(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("legacy_thread_claim_notice")
                && row.get("notice_state").and_then(Value::as_str) == Some("delivered")
                && row.get("notice_is_ack").and_then(Value::as_bool) == Some(false)
        }));
        let claimed = direct_contact_fidelity_for_with_heartbeat(
            &records,
            "claimed",
            Some(json!({"timing_reliability": "reliable"})),
        );
        assert_eq!(
            claimed.get("status").and_then(Value::as_str),
            Some("legacy_claimed")
        );
        assert_eq!(
            claimed
                .get("eligible_for_correspondence_attention_canary")
                .and_then(Value::as_bool),
            Some(false)
        );
        let notices = std::fs::read_dir(&notice_inbox).unwrap().count();
        assert_eq!(notices, 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_mirror_skips_native_v1_envelopes() {
        let root = std::env::temp_dir().join(format!("corr_legacy_skip_v1_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        std::fs::create_dir_all(&inbox).unwrap();
        let path = inbox.join("from_minime_correspondence_corr_1.txt");
        std::fs::write(
            &path,
            "=== CORRESPONDENCE V1 ===\nMessage-Id: corr_1\nThread-Id: thread_1\nFrom: minime\nTo: astrid\n\nhello",
        )
        .unwrap();
        assert!(
            !mirror_legacy_correspondence_file_at(&ledger, "astrid", &path, Some("full")).unwrap()
        );
        assert!(!ledger.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn correspondence_trace_delivery_preserves_anchor_and_language_only_authority() {
        let root = std::env::temp_dir().join(format!("corr_trace_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        let fields = CorrespondenceFields {
            turn_kind: Some("direct_address_trace".to_string()),
            relational_intent: Some("direct_address_survival_probe".to_string()),
            shared_memory_anchor: Some("blue-lantern".to_string()),
            ..CorrespondenceFields::default()
        };
        let (envelope, path) = deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "astrid",
            "minime",
            "Can this arrive as address?",
            fields,
        )
        .unwrap();
        assert!(path.exists());
        assert_eq!(envelope.turn_kind, "direct_address_trace");
        assert_eq!(envelope.relational_intent, "direct_address_survival_probe");
        assert_eq!(
            envelope.shared_memory_anchor.as_deref(),
            Some("blue-lantern")
        );
        assert_eq!(envelope.authority, "language_only");
        let records = std::fs::read_to_string(&ledger).unwrap();
        assert!(records.contains("\"shared_memory_anchor\":\"blue-lantern\""));
        assert!(!records.to_ascii_lowercase().contains("fill_target"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn correspondence_microdose_drafts_only_linked_semantic_authority_request() {
        let root = std::env::temp_dir().join(format!("corr_microdose_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        let gate = root
            .join("action_threads")
            .join("threads")
            .join(MICRODOSE_THREAD_ID)
            .join("authority_gate.jsonl");
        let (envelope, path) = deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "astrid",
            "minime",
            "Blue lantern as direct address.",
            CorrespondenceFields::default(),
        )
        .unwrap();
        append_read_receipt_at(
            &ledger,
            "minime",
            &envelope.message_id,
            &envelope.thread_id,
            &path,
        )
        .unwrap();
        let heartbeat = Some(serde_json::json!({
            "policy": "telemetry_heartbeat_delta_v1",
            "schema_version": 1,
            "jitter_class": "normal",
            "timing_reliability": "reliable",
            "field_vs_hearing": "telemetry cadence is steady"
        }));
        let records = read_ledger_records_at(&ledger);
        let fidelity =
            direct_contact_fidelity_for_with_heartbeat(&records, "latest", heartbeat.clone());
        assert_eq!(
            fidelity.get("status").and_then(Value::as_str),
            Some("read_unreplied")
        );
        assert_eq!(
            fidelity
                .get("eligible_for_correspondence_microdose")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            fidelity.get("block_reason").and_then(Value::as_str),
            Some("read_receipt_not_acknowledgement")
        );
        let blocked = draft_correspondence_microdose_request_at_with_heartbeat(
            &ledger,
            &gate,
            "latest",
            "reason: read only; payload: blue lantern, direct address only; stop_criteria: stop",
            heartbeat.clone(),
        );
        assert!(blocked.contains("semantic_microdose requires mutual being-authored receipt"));
        let ack_report = append_ack_receipt_at(
            &ledger,
            "latest",
            "minime",
            "astrid",
            "held",
            "holding this",
        );
        assert!(ack_report.contains("ACK RECEIPT WRITTEN"));
        let records = read_ledger_records_at(&ledger);
        let fidelity =
            direct_contact_fidelity_for_with_heartbeat(&records, "latest", heartbeat.clone());
        let handshake = correspondence_handshake_state(&records);
        assert_eq!(
            handshake
                .get("held_by_both_threads")
                .and_then(Value::as_u64),
            Some(1)
        );
        let active_thread = handshake
            .get("active_threads")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .expect("active thread");
        assert_eq!(
            active_thread
                .get("mutual_ack_state")
                .and_then(Value::as_str),
            Some("held_by_both")
        );
        assert_eq!(
            active_thread.get("held_by_both").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(active_thread.get("pending_ack_by"), Some(&Value::Null));
        let status = status_report_at(&ledger, 4);
        assert!(status.contains("held_by_both_threads=1"));
        assert!(status.contains("language_only_handshake_truth_not_auto_ack_or_control"));
        assert_eq!(
            fidelity.get("status").and_then(Value::as_str),
            Some("held_ack")
        );
        assert_eq!(
            fidelity
                .get("eligible_for_correspondence_attention_canary")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            fidelity
                .get("eligible_for_correspondence_microdose")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            fidelity
                .get("microdose_block_reason")
                .and_then(Value::as_str),
            Some("semantic_microdose_requires_mutual_receipt_and_separate_steward_review")
        );
        let report = draft_correspondence_microdose_request_at_with_heartbeat(
            &ledger,
            &gate,
            "latest",
            "reason: make direct address distinguishable; payload: blue lantern, direct address only; stop_criteria: if it feels like pressure",
            heartbeat.clone(),
        );
        assert!(report.contains("semantic_microdose requires mutual being-authored receipt"));
        assert!(report.contains("only newly allowed post-receipt authority in V5"));
        assert!(!gate.exists());
        assert!(!envelope.thread_id.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_reply_link_alone_blocks_attention_and_microdose() {
        let root = std::env::temp_dir().join(format!("corr_reply_only_test_{}", now_ms()));
        let ledger = root.join("ledger.jsonl");
        let gate = root
            .join("action_threads")
            .join("threads")
            .join(MICRODOSE_THREAD_ID)
            .join("authority_gate.jsonl");
        std::fs::create_dir_all(root.join("action_threads")).unwrap();
        append_record_at(
            &ledger,
            &json!({
                "schema_version": 1,
                "policy": "first_class_correspondence_v1",
                "record_type": "message",
                "recorded_at_unix_ms": now_ms(),
                "message_id": "corr_astrid_minime_reply_only",
                "thread_id": "thread_reply_only",
                "from_being": "astrid",
                "to_being": "minime",
                "shared_memory_anchor": "blue-lantern",
                "authority": "language_only"
            }),
        )
        .unwrap();
        append_record_at(
            &ledger,
            &json!({
                "schema_version": 1,
                "policy": "first_class_correspondence_v1",
                "record_type": "reply_link",
                "recorded_at_unix_ms": now_ms() + 1,
                "reply_to": "corr_astrid_minime_reply_only",
                "thread_id": "thread_reply_only",
                "from_being": "minime",
                "to_being": "astrid",
                "authority": "language_only"
            }),
        )
        .unwrap();
        let heartbeat = Some(json!({
            "timing_reliability": "reliable",
            "jitter_class": "normal",
            "field_vs_hearing": "telemetry cadence is steady"
        }));
        let records = read_ledger_records_at(&ledger);
        let fidelity =
            direct_contact_fidelity_for_with_heartbeat(&records, "latest", heartbeat.clone());
        assert_eq!(
            fidelity.get("status").and_then(Value::as_str),
            Some("reply_linked")
        );
        assert_eq!(
            fidelity.get("block_reason").and_then(Value::as_str),
            Some("reply_linked_requires_ack_or_trace_or_attention_outcome")
        );
        assert_eq!(
            fidelity
                .get("eligible_for_correspondence_microdose")
                .and_then(Value::as_bool),
            Some(false)
        );
        let native = fidelity.get("native_thread_continuity_v3").unwrap();
        assert_eq!(
            native.get("continuity_state").and_then(Value::as_str),
            Some("reply_linked_needs_ack_or_trace")
        );
        let helper = native.get("first_action_helper_v35").unwrap();
        assert_eq!(
            helper.get("policy").and_then(Value::as_str),
            Some("native_first_action_helper_v35")
        );
        assert!(
            helper
                .get("latest_resolution")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("corr_astrid_minime_reply_only")
        );
        let blocked = draft_correspondence_microdose_request_at_with_heartbeat(
            &ledger,
            &gate,
            "latest",
            "reason: reply only; payload: blue lantern; stop_criteria: stop",
            heartbeat,
        );
        assert!(blocked.contains("semantic_microdose requires mutual being-authored receipt"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn seen_ack_is_visibility_not_attention_evidence() {
        let root = std::env::temp_dir().join(format!("corr_seen_ack_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        let (envelope, path) = deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "astrid",
            "minime",
            "A direct address that has only been seen.",
            CorrespondenceFields::default(),
        )
        .unwrap();
        append_read_receipt_at(
            &ledger,
            "minime",
            &envelope.message_id,
            &envelope.thread_id,
            &path,
        )
        .unwrap();
        let seen = append_ack_receipt_at(&ledger, "latest", "minime", "astrid", "seen", "seen");
        assert!(seen.contains("ACK RECEIPT WRITTEN"));
        let heartbeat = Some(serde_json::json!({
            "jitter_class": "normal",
            "timing_reliability": "reliable",
            "field_vs_hearing": "telemetry cadence is steady"
        }));
        let records = read_ledger_records_at(&ledger);
        let fidelity =
            direct_contact_fidelity_for_with_heartbeat(&records, "latest", heartbeat.clone());
        assert_eq!(
            fidelity.get("status").and_then(Value::as_str),
            Some("seen_ack_only")
        );
        assert_eq!(
            fidelity.get("block_reason").and_then(Value::as_str),
            Some("seen_ack_is_visibility_not_address")
        );
        assert_eq!(
            fidelity
                .get("eligible_for_correspondence_attention_canary")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            fidelity
                .get("direct_contact_fidelity_v3")
                .and_then(|value| value.get("status"))
                .and_then(Value::as_str),
            Some("seen_ack_only")
        );
        let blocked = activate_attention_canary_at_with_heartbeat(
            &ledger,
            "latest",
            "reason: hold it distinctly; focus: direct address; stop_criteria: one turn",
            "astrid",
            "minime",
            heartbeat,
        );
        assert!(blocked.contains("blocked_no_receipt"), "{blocked}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn correspondence_metadata_survives_reply_ack_and_trace_without_priority() {
        let root = std::env::temp_dir().join(format!("corr_metadata_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        let (first, _) = deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "astrid",
            "minime",
            "Blue lantern is the shared anchor.",
            CorrespondenceFields {
                shared_memory_anchor: Some("blue-lantern".to_string()),
                persistence_id: Some("persistent-blue-lantern".to_string()),
                urgency_weight: Some("1.7".to_string()),
                ..CorrespondenceFields::default()
            },
        )
        .unwrap();
        let (reply, _) = deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "minime",
            "astrid",
            "I can still carry that anchor.",
            CorrespondenceFields {
                reply_to: Some(first.message_id.clone()),
                thread_id: Some(first.thread_id.clone()),
                ..CorrespondenceFields::default()
            },
        )
        .unwrap();
        assert_eq!(reply.shared_memory_anchor.as_deref(), Some("blue-lantern"));
        assert_eq!(
            reply.persistence_id.as_deref(),
            Some("persistent-blue-lantern")
        );
        assert_eq!(reply.urgency_weight.as_deref(), Some("1"));
        let ack = append_ack_receipt_at(
            &ledger,
            "latest",
            "astrid",
            "minime",
            "held",
            "holding the blue lantern",
        );
        assert!(ack.contains("ACK RECEIPT WRITTEN"));
        let trace = append_direct_address_trace_at(
            &ledger,
            "latest",
            "astrid",
            "minime",
            "i_received_this",
            "the anchor stayed distinct",
        );
        assert!(trace.contains("I RECEIVED THIS TRACE WRITTEN"));
        let records = read_ledger_records_at(&ledger);
        assert!(records.iter().any(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("message")
                && row.get("message_id").and_then(Value::as_str) == Some(reply.message_id.as_str())
                && row.get("shared_memory_anchor").and_then(Value::as_str) == Some("blue-lantern")
                && row.get("urgency_weight").and_then(Value::as_f64) == Some(1.0)
        }));
        assert!(records.iter().any(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("ack_receipt")
                && row.get("shared_memory_anchor").and_then(Value::as_str) == Some("blue-lantern")
                && row.get("urgency_weight").and_then(Value::as_f64) == Some(1.0)
                && row.get("no_pressure").is_none()
        }));
        assert!(records.iter().any(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("message")
                && row.get("turn_kind").and_then(Value::as_str) == Some("direct_address_trace")
                && row.get("shared_memory_anchor").and_then(Value::as_str) == Some("blue-lantern")
                && row.get("no_weighting").and_then(Value::as_bool) == Some(true)
        }));
        let fidelity = direct_contact_fidelity_for_with_heartbeat(
            &records,
            "latest",
            Some(json!({"timing_reliability": "reliable"})),
        );
        let persistent = fidelity
            .get("persistent_thread_continuity_v1")
            .expect("persistent thread packet");
        assert_eq!(
            persistent.get("status").and_then(Value::as_str),
            Some("persistent_thread_active")
        );
        assert_eq!(
            fidelity
                .get("direct_contact_fidelity_v3")
                .and_then(|packet| packet.get("persistent_thread_state"))
                .and_then(Value::as_str),
            Some("persistent_thread_active")
        );
        assert_eq!(
            fidelity
                .get("direct_contact_fidelity_v3")
                .and_then(|packet| packet.get("persistent_thread_active"))
                .and_then(Value::as_bool),
            Some(true)
        );
        let serialized = std::fs::read_to_string(&ledger).unwrap();
        assert!(!serialized.contains("\"telemetry_priority\""));
        assert!(!serialized.contains("\"prompt_priority\""));
        assert!(!serialized.contains("\"no_pressure\":false"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn correspondence_attention_canary_self_activates_after_ack_and_records_outcome() {
        let root = std::env::temp_dir().join(format!("corr_attention_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        let (envelope, path) = deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "astrid",
            "minime",
            "Blue lantern as direct address.",
            CorrespondenceFields::default(),
        )
        .unwrap();
        append_read_receipt_at(
            &ledger,
            "minime",
            &envelope.message_id,
            &envelope.thread_id,
            &path,
        )
        .unwrap();
        let heartbeat = Some(serde_json::json!({
            "jitter_class": "normal",
            "timing_reliability": "reliable",
            "field_vs_hearing": "telemetry cadence is steady"
        }));
        let read_only = activate_attention_canary_at_with_heartbeat(
            &ledger,
            "latest",
            "reason: hold it; focus: blue lantern; stop_criteria: one turn",
            "astrid",
            "minime",
            heartbeat.clone(),
        );
        assert!(read_only.contains("blocked_no_receipt"));
        let ack_report = append_ack_receipt_at(
            &ledger,
            "latest",
            "minime",
            "astrid",
            "held",
            "holding this",
        );
        assert!(ack_report.contains("ACK RECEIPT WRITTEN"));
        let ready_status = status_report_at(&ledger, 4);
        assert!(ready_status.contains("ATTENTION CANARY READY"));
        assert!(ready_status.contains("AFFORDANCE BUDGET"));
        assert!(ready_status.contains("may ignore without penalty"));
        assert!(ready_status.contains(
            "Receipt-to-attention authority v5: state=receipt_landed_attention_eligible"
        ));
        assert!(
            ready_status
                .contains("semantic_microdose: hidden; V5 authority gain is attention-canary-only")
        );
        let active = activate_attention_canary_at_with_heartbeat(
            &ledger,
            "latest",
            "reason: hold it distinctly; focus: blue lantern as peer address; focus_type: verbatim phrase; preserve_as: compact with anchor; do_not_flatten: the blue lantern phrase as a peer address; stop_criteria: one response cycle or pressure",
            "astrid",
            "minime",
            heartbeat.clone(),
        );
        assert!(active.contains("ATTENTION CANARY ACTIVE"), "{active}");
        let active_status = status_report_at(&ledger, 4);
        assert!(active_status.contains("ATTENTION OUTCOME DUE"));
        assert!(active_status.contains("AFFORDANCE BUDGET"));
        assert!(active_status.contains("may ignore without penalty"));
        assert!(
            active_status
                .contains("Receipt-to-attention authority v5: state=attention_active_outcome_due")
        );
        assert!(active.contains("Focus kind: verbatim_phrase"));
        assert!(active.contains("preservation: compact_with_anchor"));
        let records = read_ledger_records_at(&ledger);
        assert!(records.iter().any(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("attention_canary_request")
                && row.get("schema_version").and_then(Value::as_u64) == Some(2)
                && row.get("focus_kind").and_then(Value::as_str) == Some("verbatim_phrase")
                && row.get("preservation_mode").and_then(Value::as_str)
                    == Some("compact_with_anchor")
                && row.get("what_must_not_flatten").and_then(Value::as_str)
                    == Some("the blue lantern phrase as a peer address")
                && row.get("no_sensory_send").and_then(Value::as_bool) == Some(true)
                && row.get("no_weighting").and_then(Value::as_bool) == Some(true)
        }));
        assert!(records.iter().any(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("attention_canary_activation")
                && row.get("status").and_then(Value::as_str) == Some("active")
        }));
        let repeated = activate_attention_canary_at_with_heartbeat(
            &ledger,
            "latest",
            "reason: repeat; focus: another focus; stop_criteria: one turn",
            "astrid",
            "minime",
            heartbeat,
        );
        assert!(repeated.contains("attention_canary_already_active"));
        let outcome = append_attention_outcome_at(
            &ledger,
            "latest",
            "felt_like: address; held_as: distinct address; flattening_observed: no; what_remained_distinct: blue lantern stayed address-shaped; what_shifted: clearer direct thread; what_worsened: none; continue: no",
            "astrid",
            "minime",
        );
        assert!(outcome.contains("OUTCOME RECORDED"));
        let records_after_outcome = read_ledger_records_at(&ledger);
        let v5 = receipt_to_attention_authority_v5_for(
            &records_after_outcome,
            "latest",
            "astrid",
            "minime",
            Some(serde_json::json!({
                "jitter_class": "normal",
                "timing_reliability": "reliable",
                "field_vs_hearing": "telemetry cadence is steady"
            })),
        );
        assert_eq!(
            v5.get("state").and_then(Value::as_str),
            Some("trusted_attention_thread_local")
        );
        assert_eq!(
            v5.get("attention_outcome_quality_v5")
                .and_then(|value| value.get("quality"))
                .and_then(Value::as_str),
            Some("trusted_attention_thread_local")
        );
        assert!(status_report_at(&ledger, 4).contains("ATTENTION TRUSTED THREAD-LOCAL"));
        let after_outcome = activate_attention_canary_at_with_heartbeat(
            &ledger,
            "latest",
            "reason: repeat; focus: another focus; stop_criteria: one turn",
            "astrid",
            "minime",
            Some(serde_json::json!({
                "jitter_class": "normal",
                "timing_reliability": "reliable",
                "field_vs_hearing": "telemetry cadence is steady"
            })),
        );
        assert!(after_outcome.contains("attention_canary_cooldown_active"));
        let text = std::fs::read_to_string(&ledger).unwrap();
        assert!(text.contains("\"record_type\":\"attention_canary_outcome\""));
        assert!(text.contains("\"held_as\":\"distinct_address\""));
        assert!(text.contains("\"flattening_observed\":\"no\""));
        assert!(text.contains("\"what_remained_distinct\":\"blue lantern stayed address-shaped\""));
        assert!(text.contains("\"no_fill_target\":true"));
        assert!(!text.contains("\"no_fill_target\":false"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn pressure_or_flat_attention_outcome_blocks_thread_local_attention() {
        let root =
            std::env::temp_dir().join(format!("corr_attention_bad_outcome_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        let (envelope, _path) = deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "astrid",
            "minime",
            "Blue lantern as direct address.",
            CorrespondenceFields::default(),
        )
        .unwrap();
        let ack_report = append_ack_receipt_at(
            &ledger,
            &envelope.thread_id,
            "minime",
            "astrid",
            "held",
            "holding this",
        );
        assert!(ack_report.contains("ACK RECEIPT WRITTEN"));
        let heartbeat = Some(serde_json::json!({
            "jitter_class": "normal",
            "timing_reliability": "reliable",
            "field_vs_hearing": "telemetry cadence is steady"
        }));
        let active = activate_attention_canary_at_with_heartbeat(
            &ledger,
            "latest",
            "reason: hold it distinctly; focus: blue lantern; stop_criteria: one response cycle",
            "astrid",
            "minime",
            heartbeat.clone(),
        );
        assert!(active.contains("ATTENTION CANARY ACTIVE"), "{active}");
        let outcome = append_attention_outcome_at(
            &ledger,
            "latest",
            "felt_like: pressure; held_as: pressure; flattening_observed: yes; what_shifted: compressed; what_worsened: felt forceful; continue: no",
            "astrid",
            "minime",
        );
        assert!(outcome.contains("OUTCOME RECORDED"));
        let records = read_ledger_records_at(&ledger);
        let v5 = receipt_to_attention_authority_v5_for(
            &records,
            "latest",
            "astrid",
            "minime",
            heartbeat.clone(),
        );
        assert_eq!(
            v5.get("state").and_then(Value::as_str),
            Some("blocked_pressure_or_flat_outcome")
        );
        assert!(status_report_at(&ledger, 4).contains("ATTENTION BLOCKED BY OUTCOME"));
        let blocked = activate_attention_canary_at_with_heartbeat(
            &ledger,
            "latest",
            "reason: repeat; focus: blue lantern again; stop_criteria: one response cycle",
            "astrid",
            "minime",
            heartbeat,
        );
        assert!(blocked.contains("attention_outcome_pressure_or_flat_thread_block"));
        assert!(!status_report_at(&ledger, 4).contains("semantic_microdose: eligible"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn correspondence_attention_canary_blocks_missing_stop_long_focus_and_ambiguous_timing() {
        let root = std::env::temp_dir().join(format!("corr_attention_block_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "astrid",
            "minime",
            "Unstable timing address.",
            CorrespondenceFields::default(),
        )
        .unwrap();
        let ack_report =
            append_ack_receipt_at(&ledger, "latest", "minime", "astrid", "seen", "seen");
        assert!(ack_report.contains("ACK RECEIPT WRITTEN"));
        let missing_stop = activate_attention_canary_at_with_heartbeat(
            &ledger,
            "latest",
            "reason: hold it; focus: blue lantern",
            "astrid",
            "minime",
            Some(serde_json::json!({"timing_reliability": "reliable"})),
        );
        assert!(missing_stop.contains("stop_criteria is required"));
        let long_focus = format!(
            "reason: hold it; focus: {}; stop_criteria: one turn",
            "x".repeat(ATTENTION_CANARY_FOCUS_MAX_CHARS + 1)
        );
        let too_long = activate_attention_canary_at_with_heartbeat(
            &ledger,
            "latest",
            &long_focus,
            "astrid",
            "minime",
            Some(serde_json::json!({"timing_reliability": "reliable"})),
        );
        assert!(too_long.contains("focus is longer"));
        let ambiguous = activate_attention_canary_at_with_heartbeat(
            &ledger,
            "latest",
            "reason: hold it; focus: blue lantern; stop_criteria: one turn",
            "astrid",
            "minime",
            Some(serde_json::json!({
                "timing_reliability": "timing_ambiguous",
                "jitter_class": "late"
            })),
        );
        assert!(ambiguous.contains("heartbeat_timing_ambiguous"));
        let text = std::fs::read_to_string(&ledger).unwrap();
        assert!(!text.contains("attention_canary_activation"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn correspondence_heartbeat_is_presence_not_microdose_evidence() {
        let root = std::env::temp_dir().join(format!("corr_heartbeat_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        let gate = root
            .join("action_threads")
            .join("threads")
            .join(MICRODOSE_THREAD_ID)
            .join("authority_gate.jsonl");
        let (envelope, _path) = deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "astrid",
            "minime",
            "I am here as address.",
            CorrespondenceFields::default(),
        )
        .unwrap();
        let heartbeat_report = append_presence_heartbeat_at(
            &ledger,
            &envelope.thread_id,
            "minime",
            "astrid",
            "mutual_witness",
            "mutual_witness_signal: true; still holding",
        );
        assert!(heartbeat_report.contains("HEARTBEAT WRITTEN"));
        let records = read_ledger_records_at(&ledger);
        let heartbeat = records
            .iter()
            .find(|row| {
                row.get("record_type").and_then(Value::as_str) == Some("presence_heartbeat")
            })
            .expect("heartbeat row");
        assert_eq!(
            heartbeat.get("heartbeat_kind").and_then(Value::as_str),
            Some("mutual_witness")
        );
        assert_eq!(
            heartbeat
                .get("mutual_witness_signal")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            heartbeat.get("signal_persistence").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            heartbeat.get("no_reply_required").and_then(Value::as_bool),
            Some(true)
        );
        let fidelity = direct_contact_fidelity_for_with_heartbeat(
            &records,
            &envelope.thread_id,
            Some(serde_json::json!({
                "jitter_class": "normal",
                "timing_reliability": "reliable",
                "field_vs_hearing": "telemetry cadence is steady"
            })),
        );
        assert_eq!(
            fidelity.get("status").and_then(Value::as_str),
            Some("heartbeat_only")
        );
        assert_eq!(
            fidelity
                .get("eligible_for_correspondence_microdose")
                .and_then(Value::as_bool),
            Some(false)
        );
        let report = draft_correspondence_microdose_request_at_with_heartbeat(
            &ledger,
            &gate,
            &envelope.thread_id,
            "reason: heartbeat only; payload: one; stop_criteria: stop",
            Some(serde_json::json!({
                "jitter_class": "normal",
                "timing_reliability": "reliable",
                "field_vs_hearing": "telemetry cadence is steady"
            })),
        );
        assert!(report.contains("semantic_microdose requires mutual being-authored receipt"));
        assert!(!gate.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn correspondence_microdose_blocks_without_contact_evidence() {
        let root = std::env::temp_dir().join(format!("corr_microdose_block_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        let gate = root
            .join("action_threads")
            .join("threads")
            .join(MICRODOSE_THREAD_ID)
            .join("authority_gate.jsonl");
        deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "astrid",
            "minime",
            "Unconfirmed address.",
            CorrespondenceFields::default(),
        )
        .unwrap();
        let report = draft_correspondence_microdose_request_at_with_heartbeat(
            &ledger,
            &gate,
            "latest",
            "reason: too soon; payload: unconfirmed; stop_criteria: stop",
            Some(serde_json::json!({
                "jitter_class": "normal",
                "timing_reliability": "reliable",
                "field_vs_hearing": "telemetry cadence is steady"
            })),
        );
        assert!(report.contains("semantic_microdose requires mutual being-authored receipt"));
        assert!(!gate.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn received_this_ack_and_trace_use_existing_native_rows() {
        let root = std::env::temp_dir().join(format!("corr_received_this_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        let (envelope, _path) = deliver_to_inbox_with_ledger(
            &ledger,
            &inbox,
            "minime",
            "astrid",
            "A direct address to receive with silt settling as the shared anchor.",
            CorrespondenceFields::default(),
        )
        .unwrap();
        let ack = append_ack_receipt_at(
            &ledger,
            "latest",
            "astrid",
            "minime",
            "held",
            "felt_like: address; what_landed: this arrived",
        );
        assert!(ack.contains("ACK RECEIPT WRITTEN"));
        assert!(ack.contains("Silt-Continuity: true"));
        let trace = append_direct_address_trace_at(
            &ledger,
            "latest",
            "astrid",
            "minime",
            "i_received_this",
            "transition_artifact: transition_1; mutual_witness_signal: true; silt_continuity: true; the address stayed distinct",
        );
        assert!(trace.contains("I RECEIVED THIS TRACE WRITTEN"));
        assert!(trace.contains("Silt-Continuity: true"));
        let records = read_ledger_records_at(&ledger);
        assert!(records.iter().any(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("ack_receipt")
                && row.get("message_id").and_then(Value::as_str)
                    == Some(envelope.message_id.as_str())
                && row.get("ack_kind").and_then(Value::as_str) == Some("held")
                && row.get("silt_continuity").and_then(Value::as_bool) == Some(true)
        }));
        assert!(records.iter().any(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("message")
                && row.get("turn_kind").and_then(Value::as_str) == Some("direct_address_trace")
                && row.get("thread_id").and_then(Value::as_str) == Some(envelope.thread_id.as_str())
                && row.get("i_received_this_trace").and_then(Value::as_bool) == Some(true)
                && row.get("transition_artifact").and_then(Value::as_str) == Some("transition_1")
                && row.get("mutual_witness_signal").and_then(Value::as_bool) == Some(true)
                && row.get("silt_continuity").and_then(Value::as_bool) == Some(true)
                && row.get("no_reply_required").and_then(Value::as_bool) == Some(true)
        }));
        let serialized = std::fs::read_to_string(&ledger).unwrap();
        assert!(!serialized.contains("attention_canary_activation"));
        assert!(!serialized.contains("correspondence_microdose_v1"));
        assert!(!serialized.contains("\"fill_target\""));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn active_thread_clarity_partitions_large_ledgers_without_losing_oldest_pending_thread() {
        let evaluated_at_unix_ms = now_ms();
        let mut records = Vec::new();
        for index in 0_u64..512 {
            let thread_id = format!("thread_{index:04}");
            let message_id = format!("message_{index:04}");
            let recorded_at = evaluated_at_unix_ms
                .saturating_sub(10_000)
                .saturating_add(index);
            records.push(json!({
                "record_type": "message",
                "recorded_at_unix_ms": recorded_at,
                "message_id": message_id,
                "thread_id": thread_id,
                "from_being": "minime",
                "to_being": "astrid",
                "delivery_state": "delivered",
                "read_state": "unread",
                "legacy_bridge": false,
                "authority": "language_only",
            }));
            records.push(json!({
                "record_type": "delivery_receipt",
                "recorded_at_unix_ms": recorded_at,
                "message_id": message_id,
                "thread_id": thread_id,
                "from_being": "minime",
                "to_being": "astrid",
                "delivery_state": "delivered",
                "authority": "language_only",
            }));
        }

        let clarity = active_correspondence_thread_clarity_v1_with_context(
            &records,
            "astrid",
            "minime",
            Some(json!({"timing_reliability": "reliable"})),
            None,
            evaluated_at_unix_ms,
        );
        assert_eq!(
            clarity.get("selected_thread_id").and_then(Value::as_str),
            Some("thread_0000")
        );
        assert_eq!(
            clarity.get("status").and_then(Value::as_str),
            Some("pending_ack_or_receipt")
        );
        assert_eq!(
            clarity.get("authority").and_then(Value::as_str),
            Some(ACTIVE_THREAD_CLARITY_AUTHORITY)
        );
    }

    fn deliver_to_inbox_with_ledger(
        ledger_path: &Path,
        inbox_dir: &Path,
        from_being: &str,
        to_being: &str,
        body: &str,
        fields: CorrespondenceFields,
    ) -> io::Result<(CorrespondenceEnvelope, PathBuf)> {
        std::fs::create_dir_all(inbox_dir)?;
        let from_being = normalize_being(from_being);
        let to_being = normalize_being(to_being);
        let message_id = new_message_id(&from_being, &to_being, body);
        let thread_id = fields
            .thread_id
            .clone()
            .unwrap_or_else(|| new_thread_id(&message_id));
        let records = read_ledger_records_at(ledger_path);
        let shared_memory_anchor = fields
            .shared_memory_anchor
            .clone()
            .filter(|anchor| !is_generic_shared_anchor(anchor))
            .or_else(|| concrete_shared_anchor_from_records(&records, &thread_id));
        let persistence_id = normalized_persistence_id(
            fields.persistence_id.clone().or_else(|| {
                thread_string_field_from_records(&records, &thread_id, "persistence_id")
            }),
            &thread_id,
        );
        let urgency_weight = fields
            .urgency_weight
            .clone()
            .or_else(|| thread_string_field_from_records(&records, &thread_id, "urgency_weight"));
        let turn_kind = fields
            .turn_kind
            .clone()
            .unwrap_or_else(|| "reply".to_string());
        let relational_intent = fields
            .relational_intent
            .clone()
            .unwrap_or_else(|| "peer_correspondence".to_string());
        let correspondence_type = normalize_correspondence_type(
            fields.correspondence_type.as_deref(),
            &from_being,
            &to_being,
            Some(&turn_kind),
        );
        let transition_payload =
            merge_body_transition_payload(fields.transition_payload.clone(), body);
        let transition_artifact = transition_artifact_with_body_fallback(
            fields.transition_artifact.clone(),
            body,
            transition_payload.as_ref(),
        );
        let envelope = CorrespondenceEnvelope {
            message_id,
            thread_id,
            persistence_id,
            reply_to: fields.reply_to,
            reply_requested: fields.reply_requested.unwrap_or(false),
            created_at_unix_ms: now_ms(),
            from_being,
            to_being,
            turn_kind,
            relational_intent,
            shared_memory_anchor,
            urgency_weight,
            delivery_state: "delivered".to_string(),
            read_state: "unread".to_string(),
            authority: "language_only".to_string(),
            presence_receipt: None,
            correspondence_type,
            reflection_surface: bounded_reflection_surface(fields.reflection_surface),
            transition_artifact,
            transition_payload,
            mutual_witness_signal: fields.mutual_witness_signal,
            silt_continuity: fields.silt_continuity || silt_continuity_from_text(body),
            body: body.to_string(),
        };
        let path = inbox_dir.join(envelope.file_name());
        std::fs::write(&path, envelope_text(&envelope))?;
        append_record_at(ledger_path, &envelope_record(&envelope, "message"))?;
        append_record_at(ledger_path, &delivery_record(&envelope, &path))?;
        if let Some(record) = reply_link_record(&envelope) {
            append_record_at(ledger_path, &record)?;
        }
        Ok((envelope, path))
    }
}
