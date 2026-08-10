use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use serde::Deserialize;

use crate::auth::{
    AUTH_HEADER, CLIENT_HEADER, NONCE_HEADER, ReplayGuard, request_hash, request_signature, verify,
};
use crate::{BROKER_AUTHORITY, Config, Error, INFERENCE_PATH, Result, UNLOAD_PATH, WARMUP_PATH};

const MAXIMUM_HEADER_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Inference,
    Warmup,
    Unload,
}

impl Operation {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Inference => "inference",
            Self::Warmup => "warmup",
            Self::Unload => "unload",
        }
    }
}

#[derive(Debug)]
pub struct AuthenticatedRequest {
    pub operation: Operation,
    pub client_id: String,
    pub request_hash: String,
    pub body: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixedModelOperation {
    schema: String,
    model: String,
}

pub fn read_request(
    reader: &mut impl Read,
    config: &Config,
    listener_client: &str,
    credential_directory: &Path,
    replay: &ReplayGuard,
) -> Result<AuthenticatedRequest> {
    let mut wire = Vec::new();
    let header_end = loop {
        let mut block = [0_u8; 1024];
        let count = reader.read(&mut block)?;
        if count == 0 {
            return Err(Error::new("request ended before complete headers"));
        }
        wire.extend_from_slice(&block[..count]);
        if let Some(position) = wire.windows(4).position(|value| value == b"\r\n\r\n") {
            break position.saturating_add(4);
        }
        if wire.len() > MAXIMUM_HEADER_BYTES {
            return Err(Error::new("request headers exceed immutable bound"));
        }
    };
    if header_end > MAXIMUM_HEADER_BYTES {
        return Err(Error::new("request headers exceed immutable bound"));
    }
    let header = std::str::from_utf8(&wire[..header_end])
        .map_err(|_| Error::new("request headers are not ASCII"))?;
    if !header.is_ascii() {
        return Err(Error::new("request headers are not ASCII"));
    }
    let parsed = parse_headers(header)?;
    if parsed.length == 0
        || parsed.length > usize::try_from(config.maximum_request_body_bytes).unwrap_or(usize::MAX)
    {
        return Err(Error::new(
            "request body length is outside immutable bounds",
        ));
    }
    let mut body = wire[header_end..].to_vec();
    if body.len() > parsed.length {
        return Err(Error::new("request pipelining is rejected"));
    }
    while body.len() < parsed.length {
        let remaining = parsed.length.saturating_sub(body.len()).min(4096);
        let mut block = [0_u8; 4096];
        let count = reader.read(&mut block[..remaining])?;
        if count == 0 {
            return Err(Error::new("request body ended before Content-Length"));
        }
        body.extend_from_slice(&block[..count]);
    }
    if parsed.client_id != listener_client {
        return Err(Error::new(
            "provider client attempted to cross an isolated Unix endpoint",
        ));
    }
    let client = config.client(&parsed.client_id)?;
    let key = config.request_key(&parsed.client_id, credential_directory)?;
    let expected = request_signature(
        &key,
        &parsed.client_id,
        parsed.operation_path,
        &parsed.nonce,
        &body,
    )?;
    if !verify(&expected, &parsed.auth) {
        return Err(Error::new("provider request authentication failed"));
    }
    replay.accept(&parsed.client_id, &parsed.nonce)?;
    let (operation, canonical_body) =
        validate_body(parsed.operation_path, &parsed.client_id, &body, config)?;
    if client.client_id != parsed.client_id {
        return Err(Error::new("provider client configuration mismatch"));
    }
    Ok(AuthenticatedRequest {
        operation,
        request_hash: request_hash(
            &parsed.client_id,
            parsed.operation_path,
            &parsed.nonce,
            &body,
        )?,
        client_id: parsed.client_id,
        body: canonical_body,
    })
}

struct ParsedHeaders {
    operation_path: &'static str,
    client_id: String,
    nonce: String,
    auth: String,
    length: usize,
}

fn parse_headers(header: &str) -> Result<ParsedHeaders> {
    let mut lines = header.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| Error::new("request line is absent"))?;
    let path = match request_line {
        "POST /v1/chat/completions HTTP/1.1" => INFERENCE_PATH,
        "POST /internal/warmup HTTP/1.1" => WARMUP_PATH,
        "POST /internal/unload HTTP/1.1" => UNLOAD_PATH,
        _ => {
            return Err(Error::new(
                "request method, path, or protocol is not allowlisted",
            ));
        },
    };
    let mut values = BTreeMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        if line.starts_with([' ', '\t']) {
            return Err(Error::new("folded request headers are rejected"));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| Error::new("request header is malformed"))?;
        let name = name.to_ascii_lowercase();
        if !matches!(
            name.as_str(),
            "host"
                | "content-type"
                | "accept"
                | "connection"
                | "content-length"
                | CLIENT_HEADER
                | NONCE_HEADER
                | AUTH_HEADER
                | "user-agent"
                | "accept-encoding"
        ) || values.insert(name, value.trim()).is_some()
        {
            return Err(Error::new(
                "request header is duplicated or not allowlisted",
            ));
        }
    }
    if values.get("host").copied() != Some(BROKER_AUTHORITY)
        || values.get("content-type").copied() != Some("application/json")
        || values.get("accept").copied() != Some("application/json")
        || values.get("connection").copied() != Some("close")
    {
        return Err(Error::new(
            "request headers do not match the exact provider contract",
        ));
    }
    let client_id = required(&values, CLIENT_HEADER)?.to_owned();
    let nonce = required(&values, NONCE_HEADER)?.to_owned();
    let auth = required(&values, AUTH_HEADER)?.to_owned();
    let raw_length = required(&values, "content-length")?;
    let length = raw_length
        .parse::<usize>()
        .map_err(|_| Error::new("request Content-Length is invalid"))?;
    if length.to_string() != raw_length {
        return Err(Error::new("request Content-Length is not canonical"));
    }
    Ok(ParsedHeaders {
        operation_path: path,
        client_id,
        nonce,
        auth,
        length,
    })
}

fn required<'a>(values: &'a BTreeMap<String, &str>, name: &str) -> Result<&'a str> {
    values
        .get(name)
        .copied()
        .ok_or_else(|| Error::new(format!("request omitted {name}")))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InferenceRequest {
    model: String,
    messages: Vec<serde_json::Value>,
    stream: bool,
    stream_options: Option<StreamOptions>,
    max_tokens: u32,
    temperature: Option<f64>,
    seed: Option<i64>,
    reasoning_effort: Option<String>,
    tools: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct StreamOptions {
    include_usage: bool,
}

fn validate_body(
    path: &str,
    client: &str,
    body: &[u8],
    config: &Config,
) -> Result<(Operation, Vec<u8>)> {
    match path {
        INFERENCE_PATH => validate_inference_body(client, body, config),
        WARMUP_PATH | UNLOAD_PATH => validate_fixed_model_body(path, client, body, config),
        _ => Err(Error::new("provider operation is not allowlisted")),
    }
}

fn validate_inference_body(
    client: &str,
    body: &[u8],
    config: &Config,
) -> Result<(Operation, Vec<u8>)> {
    if !matches!(client, "edge-runtime" | "edge-steward") {
        return Err(Error::new("client is not authorized for inference"));
    }
    let request: InferenceRequest = serde_json::from_slice(body)?;
    let client_policy = config.client(client)?;
    let expected_stream = client == "edge-runtime";
    if request.model != config.model
        || request.stream != expected_stream
        || request.messages.is_empty()
        || request.messages.len() > 64
        || request.max_tokens == 0
        || request.max_tokens > client_policy.maximum_output_tokens
        || request
            .temperature
            .is_some_and(|value| !value.is_finite() || !(0.0..=2.0).contains(&value))
    {
        return Err(Error::new(
            "inference model, stream, messages, or resource bounds are not exact",
        ));
    }
    match (client, request.stream_options.as_ref(), request.seed) {
        ("edge-runtime", Some(options), None) if options.include_usage => {},
        ("edge-steward", None, Some(0) | None) => {},
        _ => {
            return Err(Error::new(
                "inference streaming or deterministic-seed policy is not exact",
            ));
        },
    }
    match (client, request.reasoning_effort.as_deref()) {
        ("edge-runtime", Some("none") | None) | ("edge-steward", None) => {},
        _ => {
            return Err(Error::new(
                "inference reasoning effort escaped the CPU-edge policy",
            ));
        },
    }
    for message in &request.messages {
        validate_message(message)?;
    }
    if let Some(tools) = request.tools.as_ref() {
        if client != "edge-runtime" || tools.is_empty() || tools.len() > 32 {
            return Err(Error::new("inference tool set is outside policy"));
        }
        for tool in tools {
            validate_tool(tool)?;
        }
    }
    canonical_inference(request, expected_stream, config)
}

fn canonical_inference(
    request: InferenceRequest,
    expected_stream: bool,
    config: &Config,
) -> Result<(Operation, Vec<u8>)> {
    let mut canonical = serde_json::Map::new();
    canonical.insert(
        "model".to_owned(),
        serde_json::Value::String(config.model.clone()),
    );
    canonical.insert(
        "messages".to_owned(),
        serde_json::Value::Array(request.messages),
    );
    canonical.insert(
        "stream".to_owned(),
        serde_json::Value::Bool(expected_stream),
    );
    if let Some(options) = request.stream_options {
        canonical.insert("stream_options".to_owned(), serde_json::to_value(options)?);
    }
    canonical.insert(
        "max_tokens".to_owned(),
        serde_json::Value::from(request.max_tokens),
    );
    if let Some(temperature) = request.temperature {
        canonical.insert(
            "temperature".to_owned(),
            serde_json::Value::from(temperature),
        );
    }
    if let Some(seed) = request.seed {
        canonical.insert("seed".to_owned(), serde_json::Value::from(seed));
    }
    if let Some(reasoning_effort) = request.reasoning_effort {
        canonical.insert(
            "reasoning_effort".to_owned(),
            serde_json::Value::String(reasoning_effort),
        );
    }
    if let Some(tools) = request.tools {
        canonical.insert("tools".to_owned(), serde_json::Value::Array(tools));
    }
    canonical.insert(
        "keep_alive".to_owned(),
        serde_json::Value::String(config.keep_alive.clone()),
    );
    canonical.insert(
        "options".to_owned(),
        serde_json::json!({"num_ctx": config.context_tokens}),
    );
    let canonical = serde_json::to_vec(&serde_json::Value::Object(canonical))?;
    if canonical.len() > usize::try_from(config.maximum_request_body_bytes).unwrap_or(usize::MAX) {
        return Err(Error::new(
            "canonical inference request exceeds immutable body bound",
        ));
    }
    Ok((Operation::Inference, canonical))
}

fn validate_fixed_model_body(
    path: &str,
    client: &str,
    body: &[u8],
    config: &Config,
) -> Result<(Operation, Vec<u8>)> {
    let operation: FixedModelOperation = serde_json::from_slice(body)?;
    let expected_schema = if path == WARMUP_PATH {
        "astrid.edge.provider_broker.warmup.v1"
    } else {
        "astrid.edge.provider_broker.unload.v1"
    };
    if operation.schema != expected_schema || operation.model != config.model {
        return Err(Error::new(
            "fixed model operation does not match immutable policy",
        ));
    }
    match (path, client) {
        (WARMUP_PATH, "model-warmup") => Ok((Operation::Warmup, body.to_vec())),
        (UNLOAD_PATH, "edge-steward") => Ok((Operation::Unload, body.to_vec())),
        _ => Err(Error::new(
            "client is not authorized for fixed model operation",
        )),
    }
}

fn validate_message(value: &serde_json::Value) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::new("inference message is not an object"))?;
    let role = object
        .get("role")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Error::new("inference message role is absent"))?;
    match role {
        "system" | "user" | "assistant" if !object.contains_key("tool_calls") => {
            exact_keys(object, &["content", "role"])?;
            bounded_string(object.get("content"), 32_768, "message content")?;
        },
        "assistant" if object.contains_key("tool_calls") => {
            exact_keys(object, &["content", "role", "tool_calls"])?;
            if !object
                .get("content")
                .is_some_and(serde_json::Value::is_null)
            {
                return Err(Error::new("assistant tool-call content must be null"));
            }
            let calls = object
                .get("tool_calls")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| Error::new("assistant tool calls are malformed"))?;
            if calls.is_empty() || calls.len() > 32 {
                return Err(Error::new("assistant tool call count is outside policy"));
            }
            for call in calls {
                validate_tool_call(call)?;
            }
        },
        "tool" => {
            exact_keys(object, &["content", "role", "tool_call_id"])?;
            bounded_string(object.get("content"), 32_768, "tool result")?;
            bounded_identifier(object.get("tool_call_id"), 256, "tool call id")?;
        },
        _ => {
            return Err(Error::new(
                "inference message role or shape is not allowlisted",
            ));
        },
    }
    Ok(())
}

fn validate_tool_call(value: &serde_json::Value) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::new("assistant tool call is not an object"))?;
    exact_keys(object, &["function", "id", "type"])?;
    bounded_identifier(object.get("id"), 256, "tool call id")?;
    if object.get("type").and_then(serde_json::Value::as_str) != Some("function") {
        return Err(Error::new("assistant tool-call type is not function"));
    }
    let function = object
        .get("function")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| Error::new("assistant tool-call function is malformed"))?;
    exact_keys(function, &["arguments", "name"])?;
    bounded_identifier(function.get("name"), 128, "tool function name")?;
    bounded_string(function.get("arguments"), 32_768, "tool arguments")?;
    Ok(())
}

fn validate_tool(value: &serde_json::Value) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::new("inference tool is not an object"))?;
    exact_keys(object, &["function", "type"])?;
    if object.get("type").and_then(serde_json::Value::as_str) != Some("function") {
        return Err(Error::new("inference tool type is not function"));
    }
    let function = object
        .get("function")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| Error::new("inference tool function is malformed"))?;
    exact_keys(function, &["description", "name", "parameters"])?;
    bounded_identifier(function.get("name"), 128, "tool function name")?;
    bounded_string(function.get("description"), 4_096, "tool description")?;
    if !function
        .get("parameters")
        .is_some_and(serde_json::Value::is_object)
    {
        return Err(Error::new("tool parameters are not a JSON object"));
    }
    let mut nodes = 0_usize;
    bounded_json(
        function
            .get("parameters")
            .ok_or_else(|| Error::new("tool parameters are absent"))?,
        0,
        &mut nodes,
    )
}

fn bounded_json(value: &serde_json::Value, depth: usize, nodes: &mut usize) -> Result<()> {
    if depth > 12 || *nodes >= 4_096 {
        return Err(Error::new("tool schema complexity exceeds policy"));
    }
    *nodes = nodes.saturating_add(1);
    match value {
        serde_json::Value::String(text) if text.len() > 16_384 => {
            Err(Error::new("tool schema string exceeds policy"))
        },
        serde_json::Value::Array(values) if values.len() > 128 => {
            Err(Error::new("tool schema array exceeds policy"))
        },
        serde_json::Value::Array(values) => {
            for item in values {
                bounded_json(item, depth.saturating_add(1), nodes)?;
            }
            Ok(())
        },
        serde_json::Value::Object(values) if values.len() > 128 => {
            Err(Error::new("tool schema object exceeds policy"))
        },
        serde_json::Value::Object(values) => {
            for (key, item) in values {
                if key.is_empty() || key.len() > 128 || !key.is_ascii() {
                    return Err(Error::new("tool schema key exceeds policy"));
                }
                bounded_json(item, depth.saturating_add(1), nodes)?;
            }
            Ok(())
        },
        _ => Ok(()),
    }
}

fn exact_keys(
    object: &serde_json::Map<String, serde_json::Value>,
    expected: &[&str],
) -> Result<()> {
    if object.len() != expected.len() || !expected.iter().all(|key| object.contains_key(*key)) {
        return Err(Error::new("inference object has unadvertised fields"));
    }
    Ok(())
}

fn bounded_string(value: Option<&serde_json::Value>, maximum: usize, label: &str) -> Result<()> {
    let text = value
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Error::new(format!("{label} is not a string")))?;
    if text.len() > maximum {
        return Err(Error::new(format!("{label} exceeds immutable bound")));
    }
    Ok(())
}

fn bounded_identifier(
    value: Option<&serde_json::Value>,
    maximum: usize,
    label: &str,
) -> Result<()> {
    bounded_string(value, maximum, label)?;
    let text = value
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if text.is_empty()
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
    {
        return Err(Error::new(format!("{label} is not canonical")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{Operation, read_request};
    use crate::Config;
    use crate::auth::{ReplayGuard, request_signature};

    fn config() -> (Config, tempfile::TempDir) {
        super::super::config::tests_support::config_for_protocol_tests()
    }

    fn request(path: &str, client: &str, body: &[u8], key: &[u8; 32]) -> Vec<u8> {
        let nonce = format!("{:016x}{}", now_millis(), "a".repeat(48));
        let auth = request_signature(key, client, path, &nonce, body).unwrap();
        format!(
            "POST {path} HTTP/1.1\r\nHost: astrid-edge-provider\r\nContent-Type: application/json\r\nAccept: application/json\r\nConnection: close\r\nContent-Length: {}\r\nX-Astrid-Provider-Client: {client}\r\nX-Astrid-Provider-Nonce: {nonce}\r\nX-Astrid-Provider-Auth: {auth}\r\n\r\n",
            body.len()
        )
        .into_bytes()
        .into_iter()
        .chain(body.iter().copied())
        .collect()
    }

    #[test]
    fn exact_inference_succeeds_and_admin_routes_never_parse() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "qwen3.5:4b",
            "messages": [
                {"role": "system", "content": "You are the AVADO edge instance."},
                {"role": "user", "content": "Find one bounded source."},
                {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "web_search",
                            "arguments": "{\"query\":\"echo state network spectral radius\"}"
                        }
                    }]
                },
                {
                    "role": "tool",
                    "tool_call_id": "call_1",
                    "content": "one bounded, untrusted result"
                }
            ],
            "stream": true,
            "stream_options": {"include_usage": true},
            "max_tokens": 64,
            "temperature": 0.3,
            "reasoning_effort": "none",
            "tools": [{
                "type": "function",
                "function": {
                    "name": "web_search",
                    "description": "Search the read-only web broker.",
                    "parameters": {
                        "type": "object",
                        "properties": {"query": {"type": "string"}},
                        "required": ["query"],
                        "additionalProperties": false
                    }
                }
            }]
        }))
        .unwrap();
        let (config, temporary) = config();
        let parsed = read_request(
            &mut Cursor::new(request(
                "/v1/chat/completions",
                "edge-runtime",
                &body,
                &[1; 32],
            )),
            &config,
            "edge-runtime",
            temporary.path(),
            &ReplayGuard::default(),
        )
        .unwrap();
        assert_eq!(parsed.operation, Operation::Inference);
        let canonical: serde_json::Value = serde_json::from_slice(&parsed.body).unwrap();
        assert_eq!(canonical["keep_alive"], "2h");
        assert_eq!(canonical["options"]["num_ctx"], 4096);
        assert_eq!(canonical["reasoning_effort"], "none");
        assert_eq!(canonical["messages"][2]["tool_calls"][0]["id"], "call_1");
        assert_eq!(canonical["tools"][0]["function"]["name"], "web_search");
        assert!(canonical.get("tool_choice").is_none());
        let inventory_hash = crate::receipt::body_hash(b"qwen3.5:4b\n");
        let delete = request("/api/delete", "edge-runtime", b"{}", &[1; 32]);
        assert!(
            read_request(
                &mut Cursor::new(delete),
                &config,
                "edge-runtime",
                temporary.path(),
                &ReplayGuard::default(),
            )
            .is_err()
        );
        assert_eq!(inventory_hash, crate::receipt::body_hash(b"qwen3.5:4b\n"));
    }

    #[test]
    fn smuggled_provider_extensions_and_cross_endpoint_clients_are_rejected() {
        let (config, temporary) = config();
        for body in [
            br#"{"model":"qwen3.5:4b","messages":[{"role":"user","content":"hi"}],"stream":true,"stream_options":{"include_usage":true},"max_tokens":64,"keep_alive":"99h"}"#.as_slice(),
            br#"{"model":"qwen3.5:4b","messages":[{"role":"user","content":"hi"}],"stream":true,"stream_options":{"include_usage":true},"max_tokens":64,"options":{"num_ctx":999999}}"#.as_slice(),
            br#"{"model":"qwen3.5:4b","messages":[{"role":"user","content":"hi"}],"stream":true,"stream_options":{"include_usage":true}}"#.as_slice(),
            br#"{"model":"qwen3.5:4b","messages":[{"role":"user","content":"hi"}],"stream":true,"stream_options":{"include_usage":true},"max_tokens":64,"reasoning_effort":"high"}"#.as_slice(),
            br#"{"model":"qwen3.5:4b","messages":[{"role":"user","content":"hi"}],"stream":true,"stream_options":{"include_usage":true},"max_tokens":64,"tool_choice":"auto"}"#.as_slice(),
        ] {
            let wire = request("/v1/chat/completions", "edge-runtime", body, &[1; 32]);
            assert!(
                read_request(
                    &mut Cursor::new(wire),
                    &config,
                    "edge-runtime",
                    temporary.path(),
                    &ReplayGuard::default(),
                )
                .is_err()
            );
        }
        let steward_body = br#"{"model":"qwen3.5:4b","messages":[{"role":"user","content":"hi"}],"stream":false,"max_tokens":64,"seed":0}"#;
        let wire = request(
            "/v1/chat/completions",
            "edge-steward",
            steward_body,
            &[2; 32],
        );
        assert!(
            read_request(
                &mut Cursor::new(wire),
                &config,
                "edge-runtime",
                temporary.path(),
                &ReplayGuard::default(),
            )
            .is_err()
        );
    }

    #[test]
    fn model_stream_and_output_policy_are_enforced() {
        for body in [
            br#"{"model":"other","messages":[{}],"stream":true}"#.as_slice(),
            br#"{"model":"qwen3.5:4b","messages":[{}],"stream":true,"max_tokens":193}"#.as_slice(),
        ] {
            let (config, temporary) = config();
            let wire = request("/v1/chat/completions", "edge-runtime", body, &[1; 32]);
            assert!(
                read_request(
                    &mut Cursor::new(wire),
                    &config,
                    "edge-runtime",
                    temporary.path(),
                    &ReplayGuard::default(),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn output_ceiling_is_isolated_per_client() {
        let (config, temporary) = config();
        let key_path = temporary.path().join("request.key");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        std::fs::write(&key_path, [2_u8; 32]).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o400)).unwrap();
        }
        let accepted = br#"{"model":"qwen3.5:4b","messages":[{"role":"user","content":"hi"}],"stream":false,"max_tokens":512,"seed":0}"#;
        let parsed = read_request(
            &mut Cursor::new(request(
                "/v1/chat/completions",
                "edge-steward",
                accepted,
                &[2; 32],
            )),
            &config,
            "edge-steward",
            temporary.path(),
            &ReplayGuard::default(),
        )
        .unwrap();
        assert_eq!(parsed.operation, Operation::Inference);

        let rejected = br#"{"model":"qwen3.5:4b","messages":[{"role":"user","content":"hi"}],"stream":false,"max_tokens":513,"seed":0}"#;
        assert!(
            read_request(
                &mut Cursor::new(request(
                    "/v1/chat/completions",
                    "edge-steward",
                    rejected,
                    &[2; 32],
                )),
                &config,
                "edge-steward",
                temporary.path(),
                &ReplayGuard::default(),
            )
            .is_err()
        );
    }

    fn now_millis() -> u64 {
        u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap()
    }
}
