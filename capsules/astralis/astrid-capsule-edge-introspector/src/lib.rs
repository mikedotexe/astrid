use astrid_guest::{capsule_result, fs, serde_json, tool};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

const HOME_ROOT: &str = "home://edge";
const MAX_BASENAMES: usize = 50;
const MAX_READ_CHARS: usize = 8_000;
const MAX_QUERY_CHARS: usize = 160;
const MAX_FILES_SEARCHED: usize = 128;
const MAX_FILE_BYTES_CONSIDERED: usize = 64 * 1024;
const MAX_MATCHES: usize = 20;
const MAX_EXCERPT_CHARS: usize = 240;

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

type ToolHandler = fn(&Value) -> Result<String, String>;

impl astrid_guest::Guest for EdgeIntrospectorCapsule {
    fn astrid_hook_trigger(action: String, payload: Vec<u8>) -> astrid_guest::CapsuleResult {
        match action.as_str() {
            "tool_execute_list_owned_artifacts" => handle_tool(&payload, list_owned_artifacts),
            "tool_execute_read_owned_artifact" => handle_tool(&payload, read_owned_artifact),
            "tool_execute_search_owned_text" => handle_tool(&payload, search_owned_text),
            "tool_execute_read_owned_continuity" => handle_tool(&payload, read_owned_continuity),
            "tool_describe" => describe(),
            _ => capsule_result::continue_empty(),
        }
    }

    fn run() {}

    fn astrid_install() {}

    fn astrid_upgrade() {}
}

fn handle_tool(payload: &[u8], handler: ToolHandler) -> astrid_guest::CapsuleResult {
    let request = match tool::parse_request(payload) {
        Ok(request) => request,
        Err(error) => return capsule_result::deny(error),
    };
    match handler(&request.arguments) {
        Ok(content) => tool::publish_success(&request.call_id, &request.tool_name, content),
        Err(error) => tool::publish_error(&request.call_id, &request.tool_name, error),
    }
}

fn list_owned_artifacts(args: &Value) -> Result<String, String> {
    let kind = tool::required_string_arg(args, "kind")?;
    let directory = owned_directory(&kind)?;
    let limit = bounded_limit(args, MAX_BASENAMES);
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
    let kind = tool::required_string_arg(args, "kind")?;
    let basename = tool::required_string_arg(args, "basename")?;
    validate_basename(&basename)?;
    if !kind_allows_basename(&kind, &basename) {
        return Err("artifact basename does not belong to the selected kind".to_string());
    }
    let maximum = bounded_limit(args, MAX_READ_CHARS);
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
    let query = tool::required_string_arg(args, "query")?;
    let query = query.trim();
    if query.is_empty()
        || query.chars().count() > MAX_QUERY_CHARS
        || query.chars().any(char::is_control)
    {
        return Err(format!(
            "query must contain 1-{MAX_QUERY_CHARS} non-control characters"
        ));
    }
    let selected = selected_kinds(args)?;
    let maximum = bounded_limit(args, MAX_MATCHES);
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

fn read_owned_continuity(_args: &Value) -> Result<String, String> {
    let current = read_json_if_allowed("home://edge/autonomous/thread_state.json", |value| {
        !contains_recovery(value)
    })
    .map(|value| sanitize_thread(&value));
    let revisions = read_jsonl_tail("home://edge/autonomous/thread_state.jsonl", 8, |value| {
        !contains_recovery(value)
    })
    .into_iter()
    .map(|value| sanitize_thread(&value))
    .collect::<Vec<_>>();
    let mut evidence = read_jsonl_tail(
        "home://edge/actions/receipts.jsonl",
        6,
        valid_action_evidence,
    );
    evidence.extend(read_jsonl_tail(
        "home://edge/web/receipts.jsonl",
        6,
        valid_web_evidence,
    ));
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
    entries.retain(|entry| validate_basename(entry).is_ok() && kind_allows_basename(kind, entry));
    Ok(entries)
}

fn kind_allows_basename(kind: &str, basename: &str) -> bool {
    kind != "tuning_result" || basename.ends_with("_result.json")
}

fn read_bounded_file(path: &str, maximum_bytes: usize) -> Result<String, String> {
    if fs::is_dir(path).unwrap_or(false) {
        return Err("directories cannot be read as artifacts".to_string());
    }
    let content = fs::read_text(path)?;
    if content.len() > maximum_bytes {
        return Err(format!(
            "artifact exceeds the {maximum_bytes}-byte read limit"
        ));
    }
    Ok(content)
}

fn owned_directory(kind: &str) -> Result<&'static str, String> {
    OWNED_KINDS
        .iter()
        .find_map(|(candidate, directory)| (*candidate == kind).then_some(*directory))
        .ok_or_else(|| "unsupported owned artifact kind".to_string())
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
    if value.is_empty()
        || value.chars().count() > 128
        || value.starts_with('.')
        || value.contains('/')
        || value.contains('\\')
        || value.contains("..")
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

fn bounded_limit(args: &Value, maximum: usize) -> usize {
    args.get("limit")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(maximum)
        .clamp(1, maximum)
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

fn read_json_if_allowed(path: &str, predicate: fn(&Value) -> bool) -> Option<Value> {
    let raw = read_bounded_file(path, MAX_FILE_BYTES_CONSIDERED).ok()?;
    let value = serde_json::from_str::<Value>(&raw).ok()?;
    predicate(&value).then_some(value)
}

fn read_jsonl_tail(path: &str, limit: usize, predicate: fn(&Value) -> bool) -> Vec<Value> {
    let Ok(raw) = fs::read_text(path) else {
        return Vec::new();
    };
    let mut start = raw.len().saturating_sub(MAX_FILE_BYTES_CONSIDERED);
    while start < raw.len() && !raw.is_char_boundary(start) {
        start = start.saturating_add(1);
    }
    let mut rows = raw[start..]
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(predicate)
        .take(limit)
        .collect::<Vec<_>>();
    rows.reverse();
    rows
}

fn contains_recovery(value: &Value) -> bool {
    value.to_string().to_ascii_lowercase().contains("recovery")
        || value
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|status| {
                matches!(
                    status,
                    "transport_recovery" | "interrupted" | "interrupted_by_restart"
                )
            })
}

fn valid_action_evidence(value: &Value) -> bool {
    value.get("status").and_then(Value::as_str) == Some("executed")
        && matches!(
            value.get("decision_source").and_then(Value::as_str),
            Some("astrid_declared" | "local_format_repair_preserved_astrid_declaration")
        )
        && !contains_recovery(value)
}

fn valid_web_evidence(value: &Value) -> bool {
    value.get("phase").and_then(Value::as_str) == Some("completed")
        && value.get("status").and_then(Value::as_str) == Some("success")
        && !contains_recovery(value)
}

fn sanitize_evidence(value: &Value) -> Value {
    json!({
        "recorded_at_unix_ms": value.get("recorded_at_unix_ms"),
        "kind": value.get("tool_name").or_else(|| value.get("outcome")),
        "status": value.get("status"),
        "artifact_basename": value
            .get("artifact_path")
            .and_then(Value::as_str)
            .and_then(|path| path.rsplit('/').next()),
        "result_summary": value.get("result_summary"),
        "response_sha256": value.get("response_sha256"),
        "result_sha256": value.get("result_sha256"),
        "trace": value.get("trace"),
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
        "schema": value.get("schema"),
        "revision": value.get("revision"),
        "thread_id": value.get("thread_id"),
        "status": value.get("status"),
        "focus": value.get("focus"),
        "question": value.get("question"),
        "hypothesis": value.get("hypothesis"),
        "latest_note": value.get("latest_note"),
        "last_action": value.get("last_action"),
        "findings": bounded_array("findings", 8),
        "open_questions": bounded_array("open_questions", 8),
        "conclusion": value.get("conclusion"),
        "uncertainty": value.get("uncertainty"),
        "updated_at_unix_ms": value.get("updated_at_unix_ms"),
        "trace": value.get("trace"),
    })
}

fn recorded_at(value: &Value) -> u64 {
    value
        .get("recorded_at_unix_ms")
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

fn describe() -> astrid_guest::CapsuleResult {
    let payload = json!({
        "capsule": "astrid-capsule-edge-introspector",
        "tools": [
            {
                "name": "list_owned_artifacts",
                "description": "List bounded basenames in one private owned artifact class.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "kind": {"type": "string"},
                        "limit": {"type": "integer", "minimum": 1, "maximum": MAX_BASENAMES}
                    },
                    "required": ["kind"],
                    "additionalProperties": false
                }
            },
            {
                "name": "read_owned_artifact",
                "description": "Read one bounded private artifact by kind and basename.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "kind": {"type": "string"},
                        "basename": {"type": "string"},
                        "limit": {"type": "integer", "minimum": 1, "maximum": MAX_READ_CHARS}
                    },
                    "required": ["kind", "basename"],
                    "additionalProperties": false
                }
            },
            {
                "name": "search_owned_text",
                "description": "Literal bounded search of this appliance's private owned text.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "maxLength": MAX_QUERY_CHARS},
                        "kinds": {"type": "array", "items": {"type": "string"}},
                        "limit": {"type": "integer", "minimum": 1, "maximum": MAX_MATCHES}
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }
            },
            {
                "name": "read_owned_continuity",
                "description": "Read the current bounded working thread and verified evidence summaries.",
                "input_schema": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            }
        ]
    });
    capsule_result::continue_json(&payload)
}

astrid_guest::export!(EdgeIntrospectorCapsule with_types_in astrid_guest::bindings);

#[cfg(test)]
mod tests {
    use super::{kind_allows_basename, literal_excerpt, selected_kinds, validate_basename};
    use astrid_guest::serde_json::json;

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
    }

    #[test]
    fn literal_search_is_case_insensitive_and_bounded() {
        let line = format!("before {} after", "A".repeat(300));
        let excerpt = literal_excerpt(&line, "before").unwrap();
        assert!(excerpt.chars().count() <= 240);
        assert!(literal_excerpt("one\nAstrid notices heat\nthree", "ASTRID").is_some());
        assert!(literal_excerpt("one\ntwo", "missing").is_none());
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
}
