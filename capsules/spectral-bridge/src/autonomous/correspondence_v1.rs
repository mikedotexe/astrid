use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

pub(crate) const LEDGER_PATH: &str = "/Users/v/other/shared/collaborations/correspondence_v1.jsonl";
const SHARED_COLLAB_DIR: &str = "/Users/v/other/shared/collaborations";
const BODY_PREVIEW_CHARS: usize = 360;
const MICRODOSE_THREAD_ID: &str = "th_correspondence_microdose";
const MICRODOSE_COOLDOWN_MS: u64 = 6 * 60 * 60 * 1000;
const MICRODOSE_PAYLOAD_MAX_CHARS: usize = 240;
const ATTENTION_CANARY_TTL_MS: u64 = 30 * 60 * 1000;
const ATTENTION_CANARY_COOLDOWN_MS: u64 = 6 * 60 * 60 * 1000;
const CORRESPONDENCE_IGNORE_GRACE_MS: u64 = 24 * 60 * 60 * 1000;
const ATTENTION_CANARY_FOCUS_MAX_CHARS: usize = 220;
const ACK_KINDS: &[&str] = &["seen", "held", "unclear", "cannot_answer", "needs_time"];
const HEARTBEAT_KINDS: &[&str] = &["holding", "still_here", "pause"];
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
    pub reply_to: Option<String>,
    pub from_being: String,
    pub to_being: String,
    pub turn_kind: String,
    pub relational_intent: String,
    pub shared_memory_anchor: Option<String>,
    pub delivery_state: String,
    pub read_state: String,
    pub authority: String,
    pub presence_receipt: Option<String>,
    pub correspondence_type: String,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CorrespondenceFields {
    pub reply_to: Option<String>,
    pub thread_id: Option<String>,
    pub turn_kind: Option<String>,
    pub relational_intent: Option<String>,
    pub shared_memory_anchor: Option<String>,
    pub presence_receipt: Option<String>,
    pub correspondence_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InboxPeerMessage {
    pub message_id: String,
    pub thread_id: String,
    pub from_being: String,
    pub file_path: PathBuf,
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

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut out = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        out.push_str("...");
    }
    out
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
        "body_preview": truncate_chars(content.trim(), BODY_PREVIEW_CHARS),
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
        "reply_to": envelope.reply_to,
        "from_being": envelope.from_being,
        "to_being": envelope.to_being,
        "turn_kind": envelope.turn_kind,
        "relational_intent": envelope.relational_intent,
        "shared_memory_anchor": envelope.shared_memory_anchor,
        "delivery_state": envelope.delivery_state,
        "read_state": envelope.read_state,
        "authority": envelope.authority,
        "presence_receipt": envelope.presence_receipt,
        "correspondence_type": envelope.correspondence_type,
        "body_sha256": format!("{:x}", Sha256::digest(envelope.body.as_bytes())),
        "body_preview": truncate_chars(envelope.body.trim(), BODY_PREVIEW_CHARS),
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
        "reply_to": envelope.reply_to,
        "from_being": envelope.from_being,
        "to_being": envelope.to_being,
        "delivery_state": envelope.delivery_state,
        "read_state": envelope.read_state,
        "authority": envelope.authority,
        "correspondence_type": envelope.correspondence_type,
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
        "from_being": envelope.from_being,
        "to_being": envelope.to_being,
        "authority": envelope.authority,
        "correspondence_type": envelope.correspondence_type,
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
    let shared_memory_anchor = envelope.shared_memory_anchor.as_deref().unwrap_or("(none)");
    let presence_receipt = envelope.presence_receipt.as_deref().unwrap_or("(none)");
    format!(
        "=== CORRESPONDENCE V1 ===\n\
         Message-Id: {}\n\
         Thread-Id: {}\n\
         Reply-To: {}\n\
         From: {}\n\
         To: {}\n\
         Turn-Kind: {}\n\
         Relational-Intent: {}\n\
         Shared-Memory-Anchor: {}\n\
         Delivery-State: {}\n\
         Read-State: {}\n\
         Authority: {}\n\
         Presence-Receipt: {}\n\
         Correspondence-Type: {}\n\n\
         {}\n",
        envelope.message_id,
        envelope.thread_id,
        reply_to,
        envelope.from_being,
        envelope.to_being,
        envelope.turn_kind,
        envelope.relational_intent,
        shared_memory_anchor,
        envelope.delivery_state,
        envelope.read_state,
        envelope.authority,
        presence_receipt,
        envelope.correspondence_type,
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
    Some(CorrespondenceEnvelope {
        message_id,
        thread_id,
        reply_to: header_value(&headers, &["reply_to"]),
        from_being,
        to_being,
        turn_kind,
        relational_intent: header_value(&headers, &["relational_intent"])
            .unwrap_or_else(|| "peer_correspondence".to_string()),
        shared_memory_anchor: header_value(&headers, &["shared_memory_anchor"]),
        delivery_state: header_value(&headers, &["delivery_state"])
            .unwrap_or_else(|| "delivered".to_string()),
        read_state: header_value(&headers, &["read_state"]).unwrap_or_else(|| "unread".to_string()),
        authority: header_value(&headers, &["authority"])
            .unwrap_or_else(|| "language_only".to_string()),
        presence_receipt: header_value(&headers, &["presence_receipt"]),
        correspondence_type,
        body,
    })
}

#[must_use]
pub(crate) fn parse_correspondence_fields(text: &str) -> CorrespondenceFields {
    let (headers, _body) = parse_headers(text);
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
        thread_id: header_value(&headers, &["correspondence_thread_id", "thread_id"]),
        turn_kind: header_value(&headers, &["turn_kind"]),
        relational_intent: header_value(&headers, &["relational_intent", "intent"]),
        shared_memory_anchor: header_value(&headers, &["shared_memory_anchor", "memory_anchor"]),
        presence_receipt: header_value(&headers, &["presence_receipt"]),
        correspondence_type: header_value(&headers, &["correspondence_type"]),
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
    let envelope = CorrespondenceEnvelope {
        message_id,
        thread_id,
        reply_to: fields.reply_to,
        from_being,
        to_being,
        turn_kind,
        relational_intent,
        shared_memory_anchor: fields.shared_memory_anchor,
        delivery_state: "delivered".to_string(),
        read_state: "unread".to_string(),
        authority: "language_only".to_string(),
        presence_receipt,
        correspondence_type,
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
    let from_being = normalize_being(from_being);
    let mut candidates = std::fs::read_dir(inbox_dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let modified = entry.metadata().ok()?.modified().ok()?;
            if !path.is_file() || path.extension().is_none_or(|ext| ext != "txt") {
                return None;
            }
            let content = std::fs::read_to_string(&path).ok()?;
            if let Some(envelope) = parse_envelope_text(&content)
                && envelope.from_being == from_being
            {
                return Some((
                    modified,
                    InboxPeerMessage {
                        message_id: envelope.message_id,
                        thread_id: envelope.thread_id,
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
            let recorded = value
                .get("recorded_at_unix_ms")
                .and_then(Value::as_u64)
                .unwrap_or_default();
            Some((
                recorded,
                InboxPeerMessage {
                    message_id,
                    thread_id,
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
            && (matches!(
                row.get("record_type").and_then(Value::as_str),
                Some("ack_receipt" | "reply_link")
            ) || (row.get("record_type").and_then(Value::as_str) == Some("message")
                && row.get("turn_kind").and_then(Value::as_str) == Some("direct_address_trace")))
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
        "ghost_thread_risk": card.get("ghost_thread_risk").cloned().unwrap_or_else(|| json!(false)),
        "mutually_recognized": card.get("mutually_recognized").cloned().unwrap_or_else(|| json!(false)),
        "attention_or_microdose_eligible": card.get("attention_or_microdose_eligible").cloned().unwrap_or_else(|| json!(false)),
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
        "notification_required": claim.get("notification_required").cloned().unwrap_or_else(|| json!(true)),
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
        &message,
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
    json!({
        "schema_version": 1,
        "policy": "first_class_correspondence_v1",
        "record_type": "ack_receipt",
        "recorded_at_unix_ms": now_ms(),
        "message_id": message.get("message_id").cloned().unwrap_or(Value::Null),
        "thread_id": message.get("thread_id").cloned().unwrap_or(Value::Null),
        "from_being": normalize_being(from_being),
        "to_being": normalize_being(to_being),
        "ack_kind": normalize_ack_kind(ack_kind),
        "note": note_value(note),
        "authority": "language_only",
        "correspondence_type": message.get("correspondence_type").cloned().unwrap_or_else(|| json!("unknown")),
    })
}

fn presence_heartbeat_record(
    message: &Value,
    from_being: &str,
    to_being: &str,
    heartbeat_kind: &str,
    note: &str,
) -> Value {
    json!({
        "schema_version": 1,
        "policy": "first_class_correspondence_v1",
        "record_type": "presence_heartbeat",
        "recorded_at_unix_ms": now_ms(),
        "message_id": message.get("message_id").cloned().unwrap_or(Value::Null),
        "thread_id": message.get("thread_id").cloned().unwrap_or(Value::Null),
        "from_being": normalize_being(from_being),
        "to_being": normalize_being(to_being),
        "heartbeat_kind": normalize_heartbeat_kind(heartbeat_kind),
        "note": note_value(note),
        "authority": "language_only",
        "correspondence_type": "presence_heartbeat",
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
    format!(
        "=== CORRESPONDENCE ACK RECEIPT WRITTEN ===\n\
         Ack: {}\n\
         From: {}\n\
         To: {}\n\
         Message: {}\n\
         Thread: {}\n\
         Authority: language_only; no telemetry, prompt priority, controller, pressure, fill, lease, deploy, weighting, or peer-runtime mutation.",
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
         Authority: language_only presence only; not a reply, approval, pressure change, telemetry priority, weighting, or controller mutation.",
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
    let shared_memory_anchor = if anchor.trim().is_empty() {
        "i_received_this"
    } else {
        anchor.trim()
    };
    let record = json!({
        "schema_version": 1,
        "policy": "first_class_correspondence_v1",
        "record_type": "message",
        "recorded_at_unix_ms": now_ms(),
        "message_id": message_id,
        "thread_id": thread_id,
        "reply_to": reply_to,
        "from_being": from,
        "to_being": to,
        "turn_kind": "direct_address_trace",
        "relational_intent": "received_this_distinctness_trace",
        "shared_memory_anchor": shared_memory_anchor,
        "delivery_state": "ledger_only",
        "read_state": "ledger_only",
        "authority": "language_only",
        "presence_receipt": Value::Null,
        "correspondence_type": normalize_correspondence_type(None, from_being, to_being, Some("direct_address_trace")),
        "body_sha256": format!("{:x}", Sha256::digest(body.as_bytes())),
        "body_preview": truncate_chars(body, BODY_PREVIEW_CHARS),
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
    format!(
        "=== I RECEIVED THIS TRACE WRITTEN ===\n\
         From: {}\n\
         To: {}\n\
         Thread: {}\n\
         Reply-To: {}\n\
         Anchor: {}\n\
         Authority: language_only trace evidence; no reply text, attention canary, microdose, pressure, controller, fill, PI, deploy, weighting, or peer-runtime mutation.",
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
        "what_must_not_flatten": note_value(&what_must_not_flatten.unwrap_or_default()),
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
    let fidelity = direct_contact_fidelity_for_with_heartbeat(records, thread_id, heartbeat);
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
        let is_ack = record_type == Some("ack_receipt");
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
    let ack_kind = ack
        .and_then(|row| row.get("ack_kind"))
        .and_then(Value::as_str)
        .map(normalize_ack_kind);
    let pending_ack_by = if ack.is_some() {
        Value::Null
    } else {
        json!(to)
    };
    let status = if matches!(ack_kind.as_deref(), Some("held" | "needs_time")) {
        "held_ack"
    } else if ack.is_some() {
        "acknowledged"
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
    let reply_linked = thread_has_reply_link(records, thread_id, message_id, message_t_ms);
    let trace_observed = thread_has_trace_evidence(records, thread_id, message_t_ms);
    let attention_outcome = thread_has_attention_outcome(records, thread_id, message_t_ms);
    let read = thread_has_read(records, thread_id, message_id);
    let delivered = thread_has_delivery(records, message_id);
    let eligible = ack.is_some() || trace_observed || attention_outcome;
    let continuity_state = if trace_observed {
        "trace_observed"
    } else if attention_outcome {
        "attention_outcome_recorded"
    } else if matches!(ack_kind.as_deref(), Some("held" | "needs_time")) {
        "held_ack"
    } else if ack.is_some() {
        "acknowledged"
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
    json!({
        "schema_version": 1,
        "policy": "correspondence_handshake_state_v1",
        "active_threads_total": active_threads.len(),
        "active_threads": active_threads.iter().rev().take(3).cloned().collect::<Vec<_>>(),
        "pending_ack_by_being": pending_ack_by_being,
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

fn direct_contact_fidelity_for_with_heartbeat(
    records: &[Value],
    selector: &str,
    heartbeat_snapshot: Option<Value>,
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
    let chamber_state = latest_chamber_correspondence_state();
    let survival = chamber_state
        .as_ref()
        .and_then(|state| state.get("direct_address_survival"))
        .and_then(|survival| survival.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let trace_observed = survival == "observed"
        && chamber_state
            .as_ref()
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
    } else if ack.is_some() {
        "acknowledged"
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
    json!({
        "schema_version": 2,
        "policy": "direct_contact_fidelity_v2",
        "status": status,
        "message_id": message_id,
        "thread_id": thread_id,
        "from_being": message.get("from_being").cloned().unwrap_or(Value::Null),
        "to_being": message.get("to_being").cloned().unwrap_or(Value::Null),
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
            "notification_required": claim.get("notification_required").cloned().unwrap_or_else(|| json!(true)),
            "initial_response_requirement": claim.get("initial_response_requirement").cloned().unwrap_or_else(|| json!("unknown")),
            "legacy_contact_evidence": claim.get("legacy_contact_evidence").cloned().unwrap_or(Value::Null),
            "latest_notice": latest_legacy_claim_notice_for_claim(records, claim).map(|notice| json!({
                "notice_id": notice.get("notice_id").cloned().unwrap_or(Value::Null),
                "notice_state": notice.get("notice_state").cloned().unwrap_or(Value::Null),
                "notice_path": notice.get("notice_path").cloned().unwrap_or(Value::Null),
                "notice_is_ack": notice.get("notice_is_ack").cloned().unwrap_or_else(|| json!(false)),
            })),
            "active": legacy_claim_is_active(records, claim),
        })),
        "legacy_claim_uptake_card_v2": legacy_claim.map(|claim| legacy_claim_uptake_card_v2(records, claim)),
        "legacy_claim_affordance_v25": legacy_claim.map(|claim| legacy_claim_affordance_v25(records, claim)),
        "native_thread_continuity_v3": if legacy_bridge { None } else { native_thread_continuity_v3_for(records, thread_id, "astrid") },
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
        .max_by_key(|row| row_time_ms(row))
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
    if let Err(error) = append_record_at(&gate_path, &record) {
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

fn status_report_at(path: &Path, max_lines: usize) -> String {
    let Ok(text) = std::fs::read_to_string(&path) else {
        return format!(
            "=== CORRESPONDENCE STATUS V1 ===\nNo correspondence ledger yet. {}\nAuthority: language_only. No telemetry, controller, PI, fill-target, lease, weighting, peer-runtime, or pressure mutation is available here.\n{}",
            no_peer_message_guidance("MINIME"),
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
    let handshake = correspondence_handshake_state(&records);
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
        format!(
            "Direct contact fidelity: status={fidelity_status}; thread={fidelity_thread}; message={fidelity_message}; timing={timing}; {field_vs_hearing}"
        ),
        format!(
            "Handshake: active_threads={}; pending_ack_by={pending_ack}; latest_ack={latest_ack}; latest_heartbeat={latest_heartbeat}; read_receipt=file_system_seen_not_mutual_address",
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
    format!(
        "Chamber correspondence state: \
         anchor={anchor}; thread={thread}; survival={survival}; contact={contact}; pending_ack_by={pending_ack}; latest_ack={latest_ack}; buffer={buffer}; \
         {attention_line}{legacy_line}correspondence_weight_candidate is one-shot authority-gate only; prompt attention canary is TTL language context only; telemetry/controller hooks remain inert, not standing weighting/control."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_roundtrip_preserves_thread_and_authority() {
        let envelope = CorrespondenceEnvelope {
            message_id: "corr_astrid_minime_1_abcd".to_string(),
            thread_id: "thread_corr_astrid_minime_1_abcd".to_string(),
            reply_to: Some("corr_minime_astrid_0_ffff".to_string()),
            from_being: "astrid".to_string(),
            to_being: "minime".to_string(),
            turn_kind: "reply".to_string(),
            relational_intent: "mutual_address".to_string(),
            shared_memory_anchor: Some("bidirectional-contact".to_string()),
            delivery_state: "delivered".to_string(),
            read_state: "unread".to_string(),
            authority: "language_only".to_string(),
            presence_receipt: None,
            correspondence_type: "astrid_direct".to_string(),
            body: "I can answer in this thread.".to_string(),
        };
        let text = envelope_text(&envelope);
        let parsed = parse_envelope_text(&text).unwrap();
        assert_eq!(parsed.message_id, envelope.message_id);
        assert_eq!(parsed.thread_id, envelope.thread_id);
        assert_eq!(parsed.reply_to, envelope.reply_to);
        assert_eq!(parsed.authority, "language_only");
        assert_eq!(parsed.correspondence_type, "astrid_direct");
        assert_eq!(parsed.body, "I can answer in this thread.");
    }

    #[test]
    fn delivery_appends_message_delivery_and_reply_link_records() {
        let root = std::env::temp_dir().join(format!("corr_v1_test_{}", now_ms()));
        let inbox = root.join("inbox");
        let ledger = root.join("ledger.jsonl");
        let fields = CorrespondenceFields {
            reply_to: Some("corr_minime_astrid_1".to_string()),
            thread_id: Some("thread_corr_minime_astrid_1".to_string()),
            ..CorrespondenceFields::default()
        };
        let (envelope, path) =
            deliver_to_inbox_with_ledger(&ledger, &inbox, "astrid", "minime", "reply body", fields)
                .unwrap();
        assert!(path.exists());
        assert_eq!(envelope.thread_id, "thread_corr_minime_astrid_1");
        let records = std::fs::read_to_string(&ledger).unwrap();
        assert!(records.contains("\"record_type\":\"message\""));
        assert!(records.contains("\"record_type\":\"delivery_receipt\""));
        assert!(records.contains("\"record_type\":\"reply_link\""));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn correspondence_status_guides_missing_and_empty_ledger_without_mutation() {
        let root = std::env::temp_dir().join(format!("corr_status_empty_test_{}", now_ms()));
        let ledger = root.join("missing").join("ledger.jsonl");
        let missing = status_report_at(&ledger, 4);
        assert!(missing.contains("No correspondence ledger yet"));
        assert!(missing.contains("No peer-message rows yet"));
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
            "still_here",
            "still holding",
        );
        assert!(heartbeat_report.contains("HEARTBEAT WRITTEN"));
        let records = read_ledger_records_at(&ledger);
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
            "A direct address to receive.",
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
        let trace = append_direct_address_trace_at(
            &ledger,
            "latest",
            "astrid",
            "minime",
            "i_received_this",
            "the address stayed distinct",
        );
        assert!(trace.contains("I RECEIVED THIS TRACE WRITTEN"));
        let records = read_ledger_records_at(&ledger);
        assert!(records.iter().any(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("ack_receipt")
                && row.get("message_id").and_then(Value::as_str)
                    == Some(envelope.message_id.as_str())
                && row.get("ack_kind").and_then(Value::as_str) == Some("held")
        }));
        assert!(records.iter().any(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("message")
                && row.get("turn_kind").and_then(Value::as_str) == Some("direct_address_trace")
                && row.get("thread_id").and_then(Value::as_str) == Some(envelope.thread_id.as_str())
                && row.get("i_received_this_trace").and_then(Value::as_bool) == Some(true)
        }));
        let serialized = std::fs::read_to_string(&ledger).unwrap();
        assert!(!serialized.contains("attention_canary_activation"));
        assert!(!serialized.contains("correspondence_microdose_v1"));
        assert!(!serialized.contains("\"fill_target\""));
        let _ = std::fs::remove_dir_all(root);
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
        let envelope = CorrespondenceEnvelope {
            message_id,
            thread_id,
            reply_to: fields.reply_to,
            from_being,
            to_being,
            turn_kind,
            relational_intent,
            shared_memory_anchor: fields.shared_memory_anchor,
            delivery_state: "delivered".to_string(),
            read_state: "unread".to_string(),
            authority: "language_only".to_string(),
            presence_receipt: None,
            correspondence_type,
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
