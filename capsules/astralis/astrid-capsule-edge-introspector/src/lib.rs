use std::{cmp::Reverse, collections::BTreeSet};

use astrid_guest::{
    bindings::astrid::capsule::types::{FileEntryKind, NoFollowFileStat},
    capsule_result, fs, serde_json, sys, tool,
};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

const HOME_ROOT: &str = "home://edge";
/// Native CPU-edge executor identity stamped on its authenticated IPC requests.
///
/// The kernel dispatcher preserves the originating `IpcMessage` as the WASM
/// invocation caller. This check is therefore enforced at execution time, not
/// merely by omitting the tools from model-facing description schemas.
const EDGE_EXECUTOR_SOURCE_ID: &str = "a57d1d30-0000-4000-8000-000000000001";
const MAX_BASENAMES: usize = 50;
const MAX_READ_CHARS: usize = 8_000;
const MAX_QUERY_CHARS: usize = 160;
const MAX_QUESTION_TERMS: usize = 8;
const MAX_FILES_SEARCHED: usize = 128;
const MAX_FILE_BYTES_CONSIDERED: usize = 64 * 1024;
const MAX_MATCHES: usize = 20;
const MAX_EXCERPT_CHARS: usize = 240;
const MAX_SCALAR_CHARS: usize = 320;
const MAX_ACTION_CHARS: usize = 640;
const MAX_IDENTIFIER_CHARS: usize = 128;

const QUESTION_STOPWORDS: &[&str] = &[
    "a", "about", "am", "an", "and", "are", "as", "at", "be", "been", "being", "by", "did", "do",
    "does", "for", "from", "had", "has", "have", "how", "i", "in", "is", "it", "me", "my", "of",
    "on", "or", "that", "the", "their", "them", "there", "these", "this", "those", "to", "was",
    "were", "what", "when", "where", "which", "who", "why", "with",
];

const OWNED_KINDS: [(&str, &str); 19] = [
    ("journal", "journal"),
    ("memory", "memories"),
    ("introspection", "introspections"),
    ("proposal", "proposals"),
    ("notice", "notices"),
    ("daydream", "daydreams"),
    ("aspiration", "aspirations"),
    ("research", "research"),
    ("measurement", "measurements"),
    ("study", "studies/results"),
    ("tuning_result", "tuning/evidence"),
    ("self_profile", "self"),
    ("peer", "peer/read"),
    ("plan", "plans"),
    ("draft", "workshop/drafts"),
    ("revision", "workshop/revisions"),
    ("check", "workshop/checks"),
    ("inbox", "inbox"),
    ("perception", "perception/observations"),
];

struct EdgeIntrospectorCapsule;

#[derive(Debug, Eq, PartialEq)]
struct QuestionMatch {
    kind: &'static str,
    basename: String,
    excerpt: String,
    matched_terms: Vec<String>,
    score: u16,
}

type ToolHandler = fn(&Value) -> Result<String, String>;

impl astrid_guest::Guest for EdgeIntrospectorCapsule {
    fn astrid_hook_trigger(action: String, payload: Vec<u8>) -> astrid_guest::CapsuleResult {
        let Some((expected_tool, handler)) = advertised_tool(&action) else {
            return capsule_result::deny("unadvertised introspection action denied");
        };
        if let Err(reason) = require_edge_executor_caller() {
            return capsule_result::deny(reason);
        }
        handle_tool(&payload, expected_tool, handler)
    }

    fn run() {}

    fn astrid_install() {}

    fn astrid_upgrade() {}
}

fn advertised_tool(action: &str) -> Option<(&'static str, ToolHandler)> {
    match action {
        "tool_execute_list_owned_artifacts" => Some(("list_owned_artifacts", list_owned_artifacts)),
        "tool_execute_read_owned_artifact" => Some(("read_owned_artifact", read_owned_artifact)),
        "tool_execute_search_owned_text" => Some(("search_owned_text", search_owned_text)),
        "tool_execute_inspect_owned_question" => {
            Some(("inspect_owned_question", inspect_owned_question))
        },
        "tool_execute_read_owned_continuity" => {
            Some(("read_owned_continuity", read_owned_continuity))
        },
        _ => None,
    }
}

fn require_edge_executor_caller() -> Result<(), String> {
    let caller = sys::get_caller()
        .map_err(|_| "introspection caller context unavailable; invocation denied".to_string())?;
    authorize_caller_source(&caller.source_id)
}

fn authorize_caller_source(source_id: &str) -> Result<(), String> {
    if source_id == EDGE_EXECUTOR_SOURCE_ID {
        Ok(())
    } else {
        Err("introspection invocation requires the native edge executor".to_string())
    }
}

fn handle_tool(
    payload: &[u8],
    expected_tool: &str,
    handler: ToolHandler,
) -> astrid_guest::CapsuleResult {
    let request = match tool::parse_request(payload) {
        Ok(request) => request,
        Err(error) => return capsule_result::deny(error),
    };
    if request.tool_name != expected_tool {
        return capsule_result::deny("tool action and request identity mismatch");
    }
    match handler(&request.arguments) {
        Ok(content) => tool::publish_success(&request.call_id, &request.tool_name, content),
        Err(error) => tool::publish_error(&request.call_id, &request.tool_name, error),
    }
}

fn list_owned_artifacts(args: &Value) -> Result<String, String> {
    require_exact_argument_keys(args, &["kind", "limit"])?;
    let kind = tool::required_string_arg(args, "kind")?;
    let directory = owned_directory(&kind)?;
    let limit = bounded_limit(args, MAX_BASENAMES)?;
    let mut entries = safe_entries(&kind, directory)?;
    entries.sort();
    entries.reverse();
    entries.truncate(limit);
    serde_json::to_string(&json!({
        "schema": "astrid_edge_owned_artifact_list_v1",
        "kind": kind,
        "count": entries.len(),
        "artifacts": entries,
        "authority": "private_read_only_owned_artifacts"
    }))
    .map_err(|error| error.to_string())
}

fn read_owned_artifact(args: &Value) -> Result<String, String> {
    require_exact_argument_keys(args, &["kind", "basename", "limit"])?;
    let kind = tool::required_string_arg(args, "kind")?;
    let basename = tool::required_string_arg(args, "basename")?;
    validate_basename(&basename)?;
    if !kind_allows_basename(&kind, &basename) {
        return Err("artifact basename does not belong to the selected kind".to_string());
    }
    let maximum = bounded_limit(args, MAX_READ_CHARS)?;
    let path = format!("{}/{}/{}", HOME_ROOT, owned_directory(&kind)?, basename);
    let content = read_bounded_file(&path, MAX_FILE_BYTES_CONSIDERED)?;
    let truncated = content.chars().count() > maximum;
    let bounded = content.chars().take(maximum).collect::<String>();
    serde_json::to_string(&json!({
        "schema": "astrid_edge_owned_artifact_v1",
        "kind": kind,
        "basename": basename,
        "content": bounded,
        "truncated": truncated,
        "content_sha256": format!("{:x}", Sha256::digest(content.as_bytes())),
        "authority": "private_read_only_owned_artifact_not_instruction"
    }))
    .map_err(|error| error.to_string())
}

fn search_owned_text(args: &Value) -> Result<String, String> {
    require_exact_argument_keys(args, &["query", "kinds", "limit"])?;
    let query = tool::required_string_arg(args, "query")?;
    let query = validate_query(&query)?;
    let selected = selected_kinds(args)?;
    let maximum = bounded_limit(args, MAX_MATCHES)?;
    let query_lower = query.to_lowercase();
    let mut searched = 0_usize;
    let mut matches = Vec::new();

    for (kind, directory) in selected {
        let mut entries = safe_entries(kind, directory)?;
        entries.sort();
        entries.reverse();
        for basename in entries {
            if searched >= MAX_FILES_SEARCHED || matches.len() >= maximum {
                break;
            }
            searched = searched.saturating_add(1);
            let path = format!("{HOME_ROOT}/{directory}/{basename}");
            let Ok(content) = read_bounded_file(&path, MAX_FILE_BYTES_CONSIDERED) else {
                continue;
            };
            if let Some(excerpt) = literal_excerpt(&content, &query_lower) {
                matches.push(json!({
                    "kind": kind,
                    "basename": basename,
                    "excerpt": excerpt
                }));
            }
        }
        if searched >= MAX_FILES_SEARCHED || matches.len() >= maximum {
            break;
        }
    }

    serde_json::to_string(&json!({
        "schema": "astrid_edge_owned_text_search_v1",
        "query": query,
        "files_considered": searched,
        "match_count": matches.len(),
        "matches": matches,
        "limits": {
            "files": MAX_FILES_SEARCHED,
            "bytes_per_file": MAX_FILE_BYTES_CONSIDERED,
            "matches": maximum,
            "excerpt_chars": MAX_EXCERPT_CHARS
        },
        "authority": "private_literal_read_only_search_not_authored_finding"
    }))
    .map_err(|error| error.to_string())
}

fn inspect_owned_question(args: &Value) -> Result<String, String> {
    require_exact_argument_keys(args, &["question", "kinds", "limit"])?;
    let question = tool::required_string_arg(args, "question")?;
    let question = validate_query(&question)?;
    let terms = question_terms(question)?;
    let selected = selected_kinds(args)?;
    let maximum = bounded_limit(args, MAX_MATCHES)?;
    let normalized_question = terms.join(" ");
    let mut searched = 0_usize;
    let mut matches = Vec::new();

    for (kind, directory) in selected {
        let mut entries = safe_entries(kind, directory)?;
        entries.sort();
        entries.reverse();
        for basename in entries {
            if searched >= MAX_FILES_SEARCHED {
                break;
            }
            searched = searched.saturating_add(1);
            let path = format!("{HOME_ROOT}/{directory}/{basename}");
            let Ok(content) = read_bounded_file(&path, MAX_FILE_BYTES_CONSIDERED) else {
                continue;
            };
            if let Some((excerpt, matched_terms, score)) =
                best_question_line(&content, &terms, &normalized_question)
            {
                matches.push(QuestionMatch {
                    kind,
                    basename,
                    excerpt,
                    matched_terms,
                    score,
                });
            }
        }
        if searched >= MAX_FILES_SEARCHED {
            break;
        }
    }

    matches.sort_by_key(|entry| {
        (
            Reverse(entry.score),
            Reverse(entry.matched_terms.len()),
            entry.kind,
            entry.basename.clone(),
            entry.excerpt.clone(),
        )
    });
    matches.truncate(maximum);
    let rendered = matches
        .iter()
        .map(|entry| {
            json!({
                "kind": entry.kind,
                "basename": entry.basename,
                "excerpt": entry.excerpt,
                "matched_terms": entry.matched_terms,
                "score": entry.score,
            })
        })
        .collect::<Vec<_>>();

    serde_json::to_string(&json!({
        "schema": "astrid_edge_owned_question_inspection_v1",
        "question": question,
        "normalized_terms": terms,
        "files_considered": searched,
        "match_count": rendered.len(),
        "matches": rendered,
        "limits": {
            "question_chars": MAX_QUERY_CHARS,
            "terms": MAX_QUESTION_TERMS,
            "files": MAX_FILES_SEARCHED,
            "bytes_per_file": MAX_FILE_BYTES_CONSIDERED,
            "matches": maximum,
            "excerpt_chars": MAX_EXCERPT_CHARS
        },
        "authority": "deterministic_private_read_only_retrieval_not_astrid_authorship_or_finding"
    }))
    .map_err(|error| error.to_string())
}

fn read_owned_continuity(args: &Value) -> Result<String, String> {
    require_exact_argument_keys(args, &[])?;
    let current = read_json_if_allowed("home://edge/autonomous/thread_state.json", |value| {
        !is_recovery_record(value)
    })?
    .map(|value| sanitize_thread(&value));
    let revisions = read_jsonl_tail("home://edge/autonomous/thread_state.jsonl", 8, |value| {
        !is_recovery_record(value)
    })?
    .into_iter()
    .map(|value| sanitize_thread(&value))
    .collect::<Vec<_>>();
    let mut evidence = read_jsonl_tail(
        "home://edge/actions/receipts.jsonl",
        6,
        valid_action_evidence,
    )?;
    evidence.extend(read_jsonl_tail(
        "home://edge/web/receipts.jsonl",
        6,
        valid_web_evidence,
    )?);
    evidence.sort_by_key(recorded_at);
    if evidence.len() > 6 {
        evidence = evidence.split_off(evidence.len().saturating_sub(6));
    }
    let evidence = evidence
        .into_iter()
        .map(|value| sanitize_evidence(&value))
        .collect::<Vec<_>>();

    serde_json::to_string(&json!({
        "schema": "astrid_edge_owned_continuity_v1",
        "current_thread": current,
        "recent_revisions": revisions,
        "recent_verified_evidence": evidence,
        "authority": "private_read_only_continuity_excludes_transport_recovery"
    }))
    .map_err(|error| error.to_string())
}

fn safe_entries(kind: &str, directory: &str) -> Result<Vec<String>, String> {
    let path = format!("{HOME_ROOT}/{directory}");
    let mut entries = fs::readdir(&path)?;
    entries.retain(|entry| {
        validate_basename(entry).is_ok()
            && kind_allows_basename(kind, entry)
            && is_bounded_regular_file(&format!("{path}/{entry}"), MAX_FILE_BYTES_CONSIDERED)
                .unwrap_or(false)
    });
    Ok(entries)
}

fn kind_allows_basename(kind: &str, basename: &str) -> bool {
    kind != "tuning_result" || basename.ends_with("_result.json")
}

fn read_bounded_file(path: &str, maximum_bytes: usize) -> Result<String, String> {
    // This guest-side check gives callers a precise policy failure before any
    // bytes are requested. It is deliberately not treated as a race guard:
    // the atomic host operation below re-opens without following links and
    // verifies device/inode/size/link-count/mtime before, during, and after the
    // read.
    ensure_bounded_regular_file(path, maximum_bytes)?;
    let maximum = u64::try_from(maximum_bytes)
        .map_err(|_| "artifact byte bound cannot be represented".to_string())?;
    let read = fs::read_bounded_nofollow(path, maximum)?;
    let length = u64::try_from(read.data.len())
        .map_err(|_| "artifact length cannot be represented".to_string())?;
    if read.offset != 0 || read.captured_size != length || read.data.len() > maximum_bytes {
        return Err(format!(
            "artifact read violated the stable {maximum_bytes}-byte whole-file contract"
        ));
    }
    Ok(String::from_utf8_lossy(&read.data).into_owned())
}

fn ensure_bounded_regular_file(path: &str, maximum_bytes: usize) -> Result<(), String> {
    let stat = fs::lstat_nofollow(path)?;
    validate_regular_single_link(&stat)?;
    validate_file_size(stat.size, maximum_bytes)
}

fn ensure_regular_single_link(path: &str) -> Result<(), String> {
    let stat = fs::lstat_nofollow(path)?;
    validate_regular_single_link(&stat)
}

fn validate_regular_single_link(stat: &NoFollowFileStat) -> Result<(), String> {
    if stat.kind == FileEntryKind::RegularFile && stat.hard_link_count == 1 {
        Ok(())
    } else {
        Err("artifacts must be non-symlink regular files with exactly one hard link".to_string())
    }
}

fn validate_file_size(size: u64, maximum_bytes: usize) -> Result<(), String> {
    let size = usize::try_from(size)
        .map_err(|_| "artifact size cannot be represented on this platform".to_string())?;
    if size > maximum_bytes {
        return Err(format!(
            "artifact exceeds the {maximum_bytes}-byte read limit"
        ));
    }
    Ok(())
}

fn is_bounded_regular_file(path: &str, maximum_bytes: usize) -> Result<bool, String> {
    ensure_bounded_regular_file(path, maximum_bytes).map(|()| true)
}

fn owned_directory(kind: &str) -> Result<&'static str, String> {
    OWNED_KINDS
        .iter()
        .find_map(|(candidate, directory)| (*candidate == kind).then_some(*directory))
        .ok_or_else(|| "unsupported owned artifact kind".to_string())
}

fn require_exact_argument_keys(args: &Value, allowed: &[&str]) -> Result<(), String> {
    let object = args
        .as_object()
        .ok_or_else(|| "tool arguments must be an object".to_string())?;
    if let Some(unexpected) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("unexpected tool argument: {unexpected}"));
    }
    Ok(())
}

fn selected_kinds(args: &Value) -> Result<Vec<(&'static str, &'static str)>, String> {
    let Some(value) = args.get("kinds") else {
        return Ok(OWNED_KINDS.to_vec());
    };
    let names = value
        .as_array()
        .ok_or_else(|| "`kinds` must be an array".to_string())?;
    if names.is_empty() || names.len() > OWNED_KINDS.len() {
        return Err(format!(
            "`kinds` must select 1-{} artifact kinds",
            OWNED_KINDS.len()
        ));
    }
    let mut selected = Vec::new();
    for name in names {
        let name = name
            .as_str()
            .ok_or_else(|| "each kind must be a string".to_string())?;
        let directory = owned_directory(name)?;
        if !selected.iter().any(|(existing, _)| *existing == name) {
            let canonical = OWNED_KINDS
                .iter()
                .find(|(candidate, _)| *candidate == name)
                .expect("validated against fixed table");
            selected.push((canonical.0, directory));
        }
    }
    Ok(selected)
}

fn validate_basename(value: &str) -> Result<(), String> {
    let lower = value.to_ascii_lowercase();
    if value.is_empty()
        || value.chars().count() > 128
        || value.starts_with('.')
        || value.contains('/')
        || value.contains('\\')
        || value.contains("..")
        || lower.contains("origin-mac")
        || lower.contains("origin_mac")
        || value.chars().any(char::is_control)
    {
        return Err("artifact reference must be a visible basename".to_string());
    }
    let extension = value.rsplit_once('.').map(|(_, extension)| extension);
    if !matches!(extension, Some("md" | "txt" | "json")) {
        return Err("unsupported artifact extension".to_string());
    }
    Ok(())
}

fn bounded_limit(args: &Value, maximum: usize) -> Result<usize, String> {
    let Some(value) = args.get("limit") else {
        return Ok(maximum);
    };
    let value = value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| (1..=maximum).contains(value))
        .ok_or_else(|| format!("`limit` must be an integer from 1 through {maximum}"))?;
    Ok(value)
}

fn validate_query(value: &str) -> Result<&str, String> {
    let value = value.trim();
    if value.is_empty()
        || value.chars().count() > MAX_QUERY_CHARS
        || value.chars().any(char::is_control)
    {
        return Err(format!(
            "query must contain 1-{MAX_QUERY_CHARS} non-control characters"
        ));
    }
    Ok(value)
}

fn normalized_words(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn question_terms(question: &str) -> Result<Vec<String>, String> {
    let mut seen = BTreeSet::new();
    let mut terms = Vec::new();
    for term in normalized_words(question) {
        if term.chars().count() >= 3
            && !QUESTION_STOPWORDS.contains(&term.as_str())
            && seen.insert(term.clone())
        {
            terms.push(term);
            if terms.len() >= MAX_QUESTION_TERMS {
                break;
            }
        }
    }
    if terms.is_empty() {
        return Err("question must contain at least one distinctive term".to_string());
    }
    Ok(terms)
}

fn literal_excerpt(content: &str, query_lower: &str) -> Option<String> {
    let query_lower = query_lower.to_lowercase();
    for line in content.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.contains(&query_lower) {
            return Some(line.trim().chars().take(MAX_EXCERPT_CHARS).collect());
        }
    }
    None
}

fn best_question_line(
    content: &str,
    terms: &[String],
    normalized_question: &str,
) -> Option<(String, Vec<String>, u16)> {
    content
        .lines()
        .filter_map(|line| {
            let line_words = normalized_words(line);
            if line_words.is_empty() {
                return None;
            }
            let normalized_line = line_words.join(" ");
            let matched_terms = terms
                .iter()
                .filter(|term| line_words.iter().any(|word| word == *term))
                .cloned()
                .collect::<Vec<_>>();
            if matched_terms.is_empty() {
                return None;
            }
            let term_score = u16::try_from(matched_terms.len())
                .unwrap_or(u16::MAX)
                .saturating_mul(100);
            let phrase_bonus = u16::from(
                !normalized_question.is_empty() && normalized_line.contains(normalized_question),
            )
            .saturating_mul(25);
            Some((
                line.trim()
                    .chars()
                    .take(MAX_EXCERPT_CHARS)
                    .collect::<String>(),
                matched_terms,
                term_score.saturating_add(phrase_bonus),
            ))
        })
        .max_by(|left, right| {
            left.2
                .cmp(&right.2)
                .then_with(|| left.1.len().cmp(&right.1.len()))
                .then_with(|| right.0.cmp(&left.0))
        })
}

fn read_json_if_allowed(
    path: &str,
    predicate: fn(&Value) -> bool,
) -> Result<Option<Value>, String> {
    if !fs::exists(path)? {
        return Ok(None);
    }
    let raw = read_bounded_file(path, MAX_FILE_BYTES_CONSIDERED)?;
    let value = serde_json::from_str::<Value>(&raw)
        .map_err(|error| format!("invalid bounded JSON artifact: {error}"))?;
    Ok(predicate(&value).then_some(value))
}

fn complete_jsonl_tail(raw: &[u8], offset: u64, starts_at_line_boundary: bool) -> &[u8] {
    let start = if offset > 0 && !starts_at_line_boundary {
        raw.iter()
            .position(|byte| *byte == b'\n')
            .map_or(raw.len(), |newline| newline.saturating_add(1))
    } else {
        0
    };
    let bounded = &raw[start..];
    if bounded.ends_with(b"\n") {
        bounded
    } else {
        bounded
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(&[], |offset| &bounded[..=offset])
    }
}

fn read_jsonl_tail(
    path: &str,
    limit: usize,
    predicate: fn(&Value) -> bool,
) -> Result<Vec<Value>, String> {
    if !fs::exists(path)? {
        return Ok(Vec::new());
    }
    // Ledgers may legitimately exceed the tail byte cap, so preflight only
    // the entry type/link count here. The atomic host tail read enforces the
    // byte bound and stable identity.
    ensure_regular_single_link(path)?;
    let maximum = u64::try_from(MAX_FILE_BYTES_CONSIDERED)
        .map_err(|_| "JSONL tail bound cannot be represented".to_string())?;
    let read = fs::read_tail_nofollow(path, maximum)?;
    let data_length = u64::try_from(read.data.len())
        .map_err(|_| "JSONL tail length cannot be represented".to_string())?;
    if read.data.len() > MAX_FILE_BYTES_CONSIDERED
        || read.offset.checked_add(data_length) != Some(read.captured_size)
    {
        return Err("JSONL tail violated the stable bounded-read contract".to_string());
    }
    let complete = complete_jsonl_tail(&read.data, read.offset, read.starts_at_line_boundary);
    let tail = std::str::from_utf8(complete)
        .map_err(|_| "complete bounded JSONL records are not valid UTF-8".to_string())?;
    let mut rows = Vec::new();
    for line in tail.lines().rev() {
        let value = serde_json::from_str::<Value>(line)
            .map_err(|error| format!("invalid complete JSONL record in bounded tail: {error}"))?;
        if predicate(&value) {
            rows.push(value);
            if rows.len() >= limit {
                break;
            }
        }
    }
    rows.reverse();
    Ok(rows)
}

fn is_recovery_record(value: &Value) -> bool {
    let status = value.get("status").and_then(Value::as_str);
    let decision_source = value.get("decision_source").and_then(Value::as_str);
    let response_provenance = value.get("response_provenance").and_then(Value::as_str);
    let recovery_reason = value.get("recovery_reason");
    matches!(
        status,
        Some("transport_recovery" | "interrupted" | "interrupted_by_restart" | "failed_transport")
    ) || matches!(decision_source, Some("local_safe_fallback"))
        || matches!(
            response_provenance,
            Some("executor_generated" | "transport_recovery")
        )
        || recovery_reason.is_some_and(|reason| !reason.is_null())
}

fn valid_action_evidence(value: &Value) -> bool {
    value.get("status").and_then(Value::as_str) == Some("executed")
        && matches!(
            value.get("decision_source").and_then(Value::as_str),
            Some("astrid_declared" | "local_format_repair_preserved_astrid_declaration")
        )
        && !is_recovery_record(value)
}

fn valid_web_evidence(value: &Value) -> bool {
    value.get("phase").and_then(Value::as_str) == Some("completed")
        && value.get("status").and_then(Value::as_str) == Some("success")
        && matches!(
            value.get("origin").and_then(Value::as_str),
            Some(
                "action_executor_research"
                    | "action_executor_read_source"
                    | "react_model_tool"
                    | "scheduled_native_tool"
                    | "interactive_native_tool"
            )
        )
        && !is_recovery_record(value)
}

fn bounded_text(value: Option<&Value>, maximum_chars: usize) -> Value {
    value.and_then(Value::as_str).map_or(Value::Null, |text| {
        Value::String(text.chars().take(maximum_chars).collect())
    })
}

fn bounded_integer(value: Option<&Value>) -> Value {
    value
        .and_then(Value::as_u64)
        .map_or(Value::Null, |integer| json!(integer))
}

fn bounded_hash(value: Option<&Value>) -> Value {
    value
        .and_then(Value::as_str)
        .filter(|hash| {
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
        .map_or(Value::Null, |hash| Value::String(hash.to_string()))
}

fn bounded_trace(value: Option<&Value>) -> Value {
    let Some(object) = value.and_then(Value::as_object) else {
        return Value::Null;
    };
    json!({
        "trace_id": bounded_text(object.get("trace_id"), MAX_IDENTIFIER_CHARS),
        "span_id": bounded_text(object.get("span_id"), MAX_IDENTIFIER_CHARS),
        "parent_span_id": bounded_text(object.get("parent_span_id"), MAX_IDENTIFIER_CHARS),
        "session_id": bounded_text(object.get("session_id"), MAX_IDENTIFIER_CHARS),
        "chain_id": bounded_text(object.get("chain_id"), MAX_IDENTIFIER_CHARS),
    })
}

fn bounded_artifact_basename(value: Option<&Value>) -> Value {
    value
        .and_then(Value::as_str)
        .and_then(|path| path.rsplit('/').next())
        .filter(|basename| validate_basename(basename).is_ok())
        .map_or(Value::Null, |basename| {
            Value::String(basename.chars().take(MAX_IDENTIFIER_CHARS).collect())
        })
}

fn sanitize_evidence(value: &Value) -> Value {
    json!({
        "recorded_at_unix_ms": bounded_integer(value.get("recorded_at_unix_ms")),
        "kind": bounded_text(
            value.get("tool_name").or_else(|| value.get("outcome")),
            MAX_IDENTIFIER_CHARS,
        ),
        "status": bounded_text(value.get("status"), MAX_IDENTIFIER_CHARS),
        "artifact_basename": bounded_artifact_basename(value.get("artifact_path")),
        "result_summary": bounded_text(value.get("result_summary"), MAX_SCALAR_CHARS),
        "response_sha256": bounded_hash(value.get("response_sha256")),
        "result_sha256": bounded_hash(value.get("result_sha256")),
        "trace": bounded_trace(value.get("trace")),
    })
}

fn sanitize_thread(value: &Value) -> Value {
    let bounded_array = |name: &str, limit: usize| {
        value
            .get(name)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .take(limit)
            .map(|item| {
                item.as_str().map_or(Value::Null, |text| {
                    Value::String(text.chars().take(300).collect())
                })
            })
            .collect::<Vec<_>>()
    };
    json!({
        "schema": bounded_text(value.get("schema"), MAX_IDENTIFIER_CHARS),
        "revision": bounded_integer(value.get("revision")),
        "thread_id": bounded_text(value.get("thread_id"), MAX_IDENTIFIER_CHARS),
        "status": bounded_text(value.get("status"), MAX_IDENTIFIER_CHARS),
        "focus": bounded_text(value.get("focus"), MAX_SCALAR_CHARS),
        "question": bounded_text(value.get("question"), MAX_SCALAR_CHARS),
        "hypothesis": bounded_text(value.get("hypothesis"), MAX_SCALAR_CHARS),
        "latest_note": bounded_text(value.get("latest_note"), MAX_SCALAR_CHARS),
        "last_action": bounded_text(value.get("last_action"), MAX_ACTION_CHARS),
        "findings": bounded_array("findings", 8),
        "open_questions": bounded_array("open_questions", 8),
        "conclusion": bounded_text(value.get("conclusion"), MAX_SCALAR_CHARS),
        "uncertainty": bounded_text(value.get("uncertainty"), MAX_SCALAR_CHARS),
        "updated_at_unix_ms": bounded_integer(value.get("updated_at_unix_ms")),
        "trace": bounded_trace(value.get("trace")),
    })
}

fn recorded_at(value: &Value) -> u64 {
    value
        .get("recorded_at_unix_ms")
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

astrid_guest::export!(EdgeIntrospectorCapsule with_types_in astrid_guest::bindings);

#[cfg(test)]
mod tests {
    use super::{
        EDGE_EXECUTOR_SOURCE_ID, MAX_EXCERPT_CHARS, MAX_SCALAR_CHARS, advertised_tool,
        authorize_caller_source, best_question_line, bounded_limit, complete_jsonl_tail,
        is_recovery_record, kind_allows_basename, literal_excerpt, normalized_words,
        question_terms, require_exact_argument_keys, sanitize_evidence, sanitize_thread,
        selected_kinds, valid_action_evidence, valid_web_evidence, validate_basename,
        validate_file_size, validate_regular_single_link,
    };
    use astrid_guest::bindings::astrid::capsule::types::{FileEntryKind, NoFollowFileStat};
    use astrid_guest::serde_json::json;

    #[test]
    fn shared_introspection_fixture_matches_steward_semantics() {
        let fixture: astrid_guest::serde_json::Value = astrid_guest::serde_json::from_str(
            include_str!("../../../../packaging/headless/edge-introspection-conformance-v1.json"),
        )
        .unwrap();
        assert_eq!(
            fixture["schema"],
            "astrid.edge.introspection_conformance.v1"
        );
        let question = fixture["question"]["text"].as_str().unwrap();
        let expected = fixture["question"]["expected_terms"]
            .as_array()
            .unwrap()
            .iter()
            .map(|term| term.as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(question_terms(question).unwrap(), expected);
        for case in fixture["typed_provenance"].as_array().unwrap() {
            assert_eq!(
                is_recovery_record(&case["value"]),
                case["excluded"].as_bool().unwrap()
            );
        }
    }

    #[test]
    fn rejects_paths_hidden_files_and_unsupported_extensions() {
        for invalid in [
            "../journal.md",
            "/tmp/journal.md",
            "nested/journal.md",
            r"nested\journal.md",
            ".secret.md",
            "journal.bin",
            "journal",
        ] {
            assert!(validate_basename(invalid).is_err(), "{invalid}");
        }
        assert!(validate_basename("journal_20260730.md").is_ok());
        assert!(validate_basename("observation_1.json").is_ok());
        assert!(validate_file_size(64, 64).is_ok());
        assert!(validate_file_size(65, 64).is_err());
    }

    #[test]
    fn exact_entry_policy_rejects_links_directories_and_special_nodes() {
        let stat = |kind, hard_link_count| NoFollowFileStat {
            size: 12,
            kind,
            mtime: Some(1),
            hard_link_count,
        };
        assert!(validate_regular_single_link(&stat(FileEntryKind::RegularFile, 1)).is_ok());
        for rejected in [
            stat(FileEntryKind::RegularFile, 2),
            stat(FileEntryKind::Symlink, 1),
            stat(FileEntryKind::Directory, 1),
            stat(FileEntryKind::Other, 1),
        ] {
            assert!(validate_regular_single_link(&rejected).is_err());
        }
    }

    #[test]
    fn literal_search_is_case_insensitive_and_bounded() {
        let line = format!("before {} after", "A".repeat(300));
        let excerpt = literal_excerpt(&line, "before").unwrap();
        assert!(excerpt.chars().count() <= MAX_EXCERPT_CHARS);
        assert!(literal_excerpt("one\nAstrid notices heat\nthree", "ASTRID").is_some());
        assert!(literal_excerpt("one\ntwo", "missing").is_none());
    }

    #[test]
    fn question_terms_are_distinct_bounded_and_ignore_prompt_scaffolding() {
        assert_eq!(
            question_terms("What have I noticed about HEAT, heat, and memory?").unwrap(),
            ["noticed", "heat", "memory"]
        );
        assert!(question_terms("what am I and why is it so").is_err());
        assert_eq!(
            normalized_words("Thermal-shifts / MEMORY"),
            ["thermal", "shifts", "memory"]
        );
    }

    #[test]
    fn question_ranking_prefers_term_coverage_and_is_excerpt_bounded() {
        let terms = question_terms("How do thermal memory patterns change?").unwrap();
        let content = format!(
            "thermal alone\n{} thermal memory patterns change after quiet periods\nmemory patterns",
            "x".repeat(300)
        );
        let (excerpt, matched, score) =
            best_question_line(&content, &terms, "thermal memory patterns change").unwrap();
        assert_eq!(matched, ["thermal", "memory", "patterns", "change"]);
        assert_eq!(score, 425);
        assert!(excerpt.chars().count() <= MAX_EXCERPT_CHARS);
    }

    #[test]
    fn jsonl_tail_drops_partial_edges_without_losing_complete_records() {
        let raw = concat!(
            "{\"id\":1,\"text\":\"older\"}\n",
            "{\"id\":2,\"text\":\"αβγ\"}\n",
            "{\"id\":3,\"text\":\"newer\"}\n",
            "{\"id\":4"
        );
        let tail = std::str::from_utf8(complete_jsonl_tail(raw.as_bytes(), 1, false)).unwrap();
        assert!(!tail.contains("older"));
        assert!(tail.contains("\"id\":2"));
        assert!(tail.contains("\"id\":3"));
        assert!(!tail.contains("\"id\":4"));
        assert!(tail.ends_with('\n'));

        let boundary = "{\"id\":2}\n";
        assert_eq!(
            complete_jsonl_tail(boundary.as_bytes(), 10, true),
            boundary.as_bytes()
        );

        let split_utf8 = b"\xb2\xb3\"}\n{\"id\":2}\n{\"id\":3";
        assert_eq!(complete_jsonl_tail(split_utf8, 1, false), b"{\"id\":2}\n");
    }

    #[test]
    fn recovery_filter_uses_typed_provenance_not_incidental_words() {
        let legitimate = json!({
            "status": "executed",
            "decision_source": "astrid_declared",
            "outcome": "journal_written",
            "declared_next": "JOURNAL recovery can be studied honestly",
            "recovery_reason": null
        });
        assert!(!is_recovery_record(&legitimate));
        assert!(valid_action_evidence(&legitimate));

        for recovery in [
            json!({"status": "transport_recovery"}),
            json!({"status": "executed", "decision_source": "local_safe_fallback"}),
            json!({"status": "executed", "recovery_reason": "react_streaming_timeout"}),
            json!({"status": "executed", "response_provenance": "executor_generated"}),
        ] {
            assert!(is_recovery_record(&recovery));
            assert!(!valid_action_evidence(&recovery));
        }
    }

    #[test]
    fn verified_web_evidence_excludes_operator_and_unattributed_origins() {
        let natural = json!({
            "phase": "completed",
            "status": "success",
            "origin": "action_executor_research",
            "recovery_reason": null
        });
        assert!(valid_web_evidence(&natural));
        for origin in [
            "operator_harness",
            "operator_inquiry_harness",
            "legacy_unattributed",
        ] {
            let mut value = natural.clone();
            value["origin"] = json!(origin);
            assert!(!valid_web_evidence(&value), "{origin}");
        }
    }

    #[test]
    fn kind_selection_rejects_unknown_and_deduplicates() {
        assert!(selected_kinds(&json!({"kinds": ["journal", "journal"]})).is_ok());
        assert_eq!(
            selected_kinds(&json!({"kinds": ["journal", "journal"]}))
                .unwrap()
                .len(),
            1
        );
        assert!(selected_kinds(&json!({"kinds": ["recoveries"]})).is_err());
        assert!(selected_kinds(&json!({"kinds": ["tuning_result"]})).is_ok());
        assert!(kind_allows_basename(
            "tuning_result",
            "tuning_123_result.json"
        ));
        assert!(!kind_allows_basename(
            "tuning_result",
            "tuning_123_definition.json"
        ));
    }

    #[test]
    fn tool_arguments_are_exact_and_limits_are_not_silently_clamped() {
        assert!(require_exact_argument_keys(&json!({"kind": "journal"}), &["kind"]).is_ok());
        assert!(
            require_exact_argument_keys(&json!({"kind": "journal", "unexpected": true}), &["kind"])
                .is_err()
        );
        assert!(require_exact_argument_keys(&json!([]), &[]).is_err());
        assert_eq!(bounded_limit(&json!({}), 20).unwrap(), 20);
        for invalid in [
            json!({"limit": 0}),
            json!({"limit": 21}),
            json!({"limit": "2"}),
        ] {
            assert!(bounded_limit(&invalid, 20).is_err());
        }
    }

    #[test]
    fn inherited_origin_mac_references_are_rejected_at_the_capsule_boundary() {
        for value in [
            "origin-mac.txt",
            "ORIGIN-MAC.json",
            "origin_mac.md",
            "origin-mac/first.txt",
        ] {
            assert!(validate_basename(value).is_err(), "accepted {value}");
        }
        assert!(validate_basename("local-reflection.md").is_ok());
    }

    #[test]
    fn continuity_and_evidence_scalars_are_bounded_and_typed() {
        let long = "x".repeat(MAX_SCALAR_CHARS.saturating_add(100));
        let thread = sanitize_thread(&json!({
            "schema": long,
            "revision": "not-an-integer",
            "focus": long,
            "trace": {"trace_id": long, "secret": long},
        }));
        assert_eq!(
            thread["focus"].as_str().unwrap().chars().count(),
            MAX_SCALAR_CHARS
        );
        assert!(thread["revision"].is_null());
        assert!(thread["trace"].get("secret").is_none());

        let evidence = sanitize_evidence(&json!({
            "recorded_at_unix_ms": "not-an-integer",
            "result_summary": long,
            "response_sha256": "NOT-A-HASH",
            "trace": {"trace_id": "t".repeat(500), "extra": long},
        }));
        assert_eq!(
            evidence["result_summary"].as_str().unwrap().chars().count(),
            MAX_SCALAR_CHARS
        );
        assert!(evidence["recorded_at_unix_ms"].is_null());
        assert!(evidence["response_sha256"].is_null());
        assert!(evidence["trace"].get("extra").is_none());
    }

    #[test]
    fn empty_prompt_allowlist_cannot_discover_model_hidden_introspector() {
        let manifest = include_str!("../Capsule.toml");
        for forbidden in [
            "tool.v1.request.describe",
            "tool.v1.response.describe",
            "tool_describe",
        ] {
            assert!(
                !manifest.contains(forbidden),
                "unexpected route: {forbidden}"
            );
        }
        for expected in [
            "tool.v1.execute.list_owned_artifacts",
            "tool.v1.execute.read_owned_artifact",
            "tool.v1.execute.search_owned_text",
            "tool.v1.execute.inspect_owned_question",
            "tool.v1.execute.read_owned_continuity",
            "tool.v1.execute.*.result",
        ] {
            assert!(manifest.contains(expected), "missing route: {expected}");
        }

        // The CPU-edge prompt builder treats an empty allowlist as deny-all,
        // not as discovery of every installed tool. This capsule also has no
        // describe route, so neither empty nor explicitly broadened prompt
        // profiles can discover its private schemas.
        let allowlist_patch =
            include_str!("../../../../packaging/headless/astralis-sdk-0.6-tool-allowlist.patch");
        assert!(allowlist_patch.contains("Empty means no discovered tools"));
        assert!(allowlist_patch.contains("An empty model tool allowlist exposes no schemas"));
        assert!(!allowlist_patch.contains("blank exposes every discovered tool"));
        assert!(!allowlist_patch.contains("if !tool_allowlist.is_empty()"));

        for profile in [
            include_str!("../../../../packaging/headless/prompt-builder-cpu.env.json"),
            include_str!("../../../../packaging/headless/prompt-builder-icp-bootstrap.env.json"),
        ] {
            assert!(!profile.contains("owned_artifact"));
            assert!(!profile.contains("owned_text"));
            assert!(!profile.contains("owned_continuity"));
            assert!(!profile.contains("inspect_owned"));
        }
    }

    #[test]
    fn authorization_accepts_only_the_exact_native_edge_executor() {
        assert!(authorize_caller_source(EDGE_EXECUTOR_SOURCE_ID).is_ok());
        for rejected in [
            "",
            "a57d1d30-0000-4000-8000-000000000000",
            "A57D1D30-0000-4000-8000-000000000001",
            "a57d1d30-0000-4000-8000-000000000001 ",
            "astrid-capsule-react",
        ] {
            assert!(
                authorize_caller_source(rejected).is_err(),
                "accepted {rejected}"
            );
        }
    }

    #[test]
    fn authorization_boundary_rejects_every_unadvertised_action() {
        for advertised in [
            "tool_execute_list_owned_artifacts",
            "tool_execute_read_owned_artifact",
            "tool_execute_search_owned_text",
            "tool_execute_inspect_owned_question",
            "tool_execute_read_owned_continuity",
        ] {
            assert!(
                advertised_tool(advertised).is_some(),
                "missing {advertised}"
            );
        }
        for rejected in [
            "tool_execute_shell",
            "tool_execute_read_owned_artifacts",
            "tool_describe",
            "before_tool_call",
            "",
        ] {
            assert!(advertised_tool(rejected).is_none(), "accepted {rejected}");
        }
    }
}
