//! Immutable Unix-broker adapter for CPU-edge native ReAct web tools.
//!
//! The root-managed core has no network namespace access. Only an exact,
//! traced invocation of the read-only HTTP capsule may cross this adapter;
//! every other capsule HTTP request retains the ordinary host policy.

use std::fs::{self, OpenOptions};
use std::io::Read as _;
use std::os::unix::fs::{
    FileTypeExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _,
};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use astrid_events::ipc::{IpcMessage, IpcPayload};
use ed25519_dalek::{Signature, VerifyingKey};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::engine::wasm::bindings::astrid::capsule::types::{
    HttpRequestData, HttpResponseData, KeyValuePair,
};
use crate::engine::wasm::host::util;

const HTTP_CAPSULE_ID: &str = "astrid-capsule-http";
const REACT_CAPSULE_ID: &str = "astrid-capsule-react";
const CLIENT_ID: &str = "edge-core";
const BROKER_AUTHORITY: &str = "astrid-edge-web-broker";
const BROKER_SOCKET: &str = "/run/astrid-edge-self-change/web-core.sock";
const REQUEST_CREDENTIAL: &str = "/run/credentials/astrid.service/web-request.key";
const RESPONSE_CREDENTIAL: &str = "/run/credentials/astrid.service/web-response.pub";
const SEARCH_PATH: &str = "/v1/search";
const FETCH_PATH: &str = "/v1/fetch";
const SEARCH_REQUEST_SCHEMA: &str = "astrid.edge.web_search.request.v2";
const SEARCH_RESPONSE_SCHEMA: &str = "astrid.edge.web_search.response.v1";
const FETCH_REQUEST_SCHEMA: &str = "astrid.edge.web_fetch.request.v2";
const FETCH_RESPONSE_SCHEMA: &str = "astrid.edge.web_fetch.response.v1";
const BROKER_VERIFIED_HEADER: &str = "x-astrid-immutable-web-broker";
const PROTOCOL_VERSION: &[u8] = b"astrid.edge.web_broker.auth.v2";
const REQUEST_AUTH_DOMAIN: &[u8] = b"astrid.edge.web_broker.request_auth.v2";
const REQUEST_HASH_DOMAIN: &[u8] = b"astrid.edge.web_broker.request_hash.v2";
const RESPONSE_SIGNATURE_DOMAIN: &[u8] = b"astrid.edge.web_broker.response_signature.v2";
const CLIENT_HEADER: &str = "x-astrid-web-client";
const NONCE_HEADER: &str = "x-astrid-web-nonce";
const AUTH_HEADER: &str = "x-astrid-web-auth";
const REQUEST_HASH_HEADER: &str = "x-astrid-web-request-hash";
const SIGNATURE_HEADER: &str = "x-astrid-web-signature";
const MAXIMUM_SEARCH_RESPONSE_BYTES: usize = 64 * 1_024;
const MAXIMUM_FETCH_RESPONSE_BYTES: usize = 512 * 1_024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug)]
struct Config {
    socket_path: PathBuf,
    request_credential: PathBuf,
    response_credential: PathBuf,
    response_key_sha256: String,
}

#[derive(Debug)]
enum Configuration {
    Absent,
    Configured(Config),
    Invalid(String),
}

static CONFIGURATION: std::sync::LazyLock<Configuration> = std::sync::LazyLock::new(|| {
    let socket = std::env::var_os("ASTRID_EDGE_CORE_WEB_BROKER_SOCKET");
    let request = std::env::var_os("ASTRID_EDGE_CORE_WEB_BROKER_REQUEST_CREDENTIAL");
    let response = std::env::var_os("ASTRID_EDGE_CORE_WEB_BROKER_RESPONSE_CREDENTIAL");
    let digest = std::env::var("ASTRID_EDGE_CORE_WEB_BROKER_RESPONSE_KEY_SHA256").ok();
    match configuration_from_values(socket, request, response, digest) {
        Ok(Some(config)) => Configuration::Configured(config),
        Ok(None) => Configuration::Absent,
        Err(error) => Configuration::Invalid(error),
    }
});

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct SearchRequest {
    schema: &'static str,
    trace_id: String,
    query: String,
    limit: u8,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SearchResponse {
    schema: String,
    results: Vec<SearchResult>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct FetchRequest {
    schema: &'static str,
    trace_id: String,
    url: String,
    max_chars: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FetchResponse {
    schema: String,
    url: String,
    status: u16,
    original_body_bytes: u64,
    truncated: bool,
    body: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorResponse {
    schema: String,
    code: String,
}

enum BrokerRequest {
    Search(SearchRequest),
    Fetch(FetchRequest),
}

impl BrokerRequest {
    const fn path(&self) -> &'static str {
        match self {
            Self::Search(_) => SEARCH_PATH,
            Self::Fetch(_) => FETCH_PATH,
        }
    }

    const fn maximum_response_bytes(&self) -> usize {
        match self {
            Self::Search(_) => MAXIMUM_SEARCH_RESPONSE_BYTES,
            Self::Fetch(_) => MAXIMUM_FETCH_RESPONSE_BYTES,
        }
    }

    fn body(&self) -> Result<Vec<u8>, String> {
        match self {
            Self::Search(request) => serde_json::to_vec(request),
            Self::Fetch(request) => serde_json::to_vec(request),
        }
        .map_err(|error| format!("encode immutable web-broker request: {error}"))
    }
}

/// Route an exact CPU-edge native web-tool call through the immutable broker.
///
/// `Ok(None)` means the CPU-edge broker is not configured or this is not the
/// dedicated HTTP capsule. A configured HTTP capsule fails closed on any
/// caller, argument, or broker-authentication mismatch.
pub(super) fn route(
    capsule_id: &str,
    caller: Option<&IpcMessage>,
    request: &HttpRequestData,
    runtime_handle: &tokio::runtime::Handle,
    host_semaphore: &Arc<tokio::sync::Semaphore>,
) -> Result<Option<HttpResponseData>, String> {
    let config = match &*CONFIGURATION {
        Configuration::Absent => return Ok(None),
        Configuration::Invalid(error) => {
            if capsule_id == HTTP_CAPSULE_ID {
                return Err(format!(
                    "immutable core web-broker configuration is invalid: {error}"
                ));
            }
            return Ok(None);
        },
        Configuration::Configured(config) if capsule_id == HTTP_CAPSULE_ID => config,
        Configuration::Configured(_) => return Ok(None),
    };
    let (broker_request, output_kind) = validated_request(caller, request)?;
    let response = util::bounded_block_on(runtime_handle, host_semaphore, async {
        post(config, &broker_request).await
    })?;
    match output_kind {
        OutputKind::Search => {
            let response: SearchResponse = serde_json::from_slice(&response)
                .map_err(|error| format!("decode immutable search response: {error}"))?;
            if response.schema != SEARCH_RESPONSE_SCHEMA || response.results.len() > 5 {
                return Err("immutable search response escaped schema or count".to_string());
            }
            let body = serde_json::to_vec(&response)
                .map_err(|error| format!("encode verified search response: {error}"))?;
            Ok(Some(verified_http_response(body)))
        },
        OutputKind::Fetch { url, max_chars } => {
            let response: FetchResponse = serde_json::from_slice(&response)
                .map_err(|error| format!("decode immutable fetch response: {error}"))?;
            if response.schema != FETCH_RESPONSE_SCHEMA
                || response.url != url
                || response.status != 200
                || response.original_body_bytes
                    < u64::try_from(response.body.len()).unwrap_or(u64::MAX)
                || response.body.chars().count() > max_chars
                || (response.truncated && response.body.is_empty())
                || response
                    .body
                    .chars()
                    .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
            {
                return Err("immutable fetch response escaped schema or text bounds".to_string());
            }
            Ok(Some(verified_http_response(response.body.into_bytes())))
        },
    }
}

#[derive(Debug)]
enum OutputKind {
    Search,
    Fetch { url: String, max_chars: usize },
}

fn validated_request(
    caller: Option<&IpcMessage>,
    request: &HttpRequestData,
) -> Result<(BrokerRequest, OutputKind), String> {
    let caller = caller.ok_or_else(|| "native web tool has no direct caller".to_string())?;
    let producer = caller
        .producer
        .as_ref()
        .filter(|producer| producer.is_supported())
        .ok_or_else(|| "native web tool caller lacks supported producer identity".to_string())?;
    if producer.kind != "wasm_capsule" || producer.id != REACT_CAPSULE_ID {
        return Err("native web tool caller is not the exact ReAct capsule".to_string());
    }
    let trace = caller
        .trace
        .as_ref()
        .filter(|trace| {
            trace.is_supported()
                && trace.turn_id.is_some()
                && trace
                    .session_id
                    .as_deref()
                    .is_some_and(|value| !value.is_empty())
        })
        .ok_or_else(|| "native web tool caller lacks a complete trace".to_string())?;
    let IpcPayload::ToolExecuteRequest {
        call_id,
        tool_name,
        arguments,
    } = &caller.payload
    else {
        return Err("native web tool caller is not a tool request".to_string());
    };
    if call_id.is_empty()
        || call_id.len() > 256
        || !call_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err("native web tool call identifier is invalid".to_string());
    }
    let object = arguments
        .as_object()
        .ok_or_else(|| "native web tool arguments are not an object".to_string())?;
    match tool_name.as_str() {
        "search_web" => {
            if caller.topic != "tool.v1.execute.search_web"
                || !object
                    .keys()
                    .all(|key| matches!(key.as_str(), "query" | "count"))
                || request.method != "GET"
                || request.body.is_some()
            {
                return Err("native search request escaped the read-only contract".to_string());
            }
            let query = object
                .get("query")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| {
                    !value.is_empty()
                        && value.chars().count() <= 300
                        && !value.chars().any(char::is_control)
                })
                .ok_or_else(|| "native search query exceeds bounds".to_string())?;
            let count = object
                .get("count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(5)
                .clamp(1, 5);
            validate_search_url(&request.url, query)?;
            Ok((
                BrokerRequest::Search(SearchRequest {
                    schema: SEARCH_REQUEST_SCHEMA,
                    trace_id: trace.trace_id.to_string(),
                    query: query.to_string(),
                    limit: u8::try_from(count).unwrap_or(5),
                }),
                OutputKind::Search,
            ))
        },
        "fetch_url" => {
            if caller.topic != "tool.v1.execute.fetch_url"
                || !object
                    .keys()
                    .all(|key| matches!(key.as_str(), "url" | "method" | "headers" | "max_chars"))
                || request.method != "GET"
                || request.body.is_some()
                || !request.headers.is_empty()
            {
                return Err(
                    "native fetch request escaped the broker's GET-only contract".to_string(),
                );
            }
            if object
                .get("headers")
                .is_some_and(|value| value.as_object().is_none_or(|headers| !headers.is_empty()))
            {
                return Err("native broker fetch does not accept caller headers".to_string());
            }
            let method = object
                .get("method")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("GET");
            let url = object
                .get("url")
                .and_then(serde_json::Value::as_str)
                .filter(|value| *value == request.url && value.chars().count() <= 2_048)
                .ok_or_else(|| "native fetch URL does not match tool arguments".to_string())?;
            if !method.eq_ignore_ascii_case("GET") {
                return Err("native broker fetch supports GET only".to_string());
            }
            validate_public_url_shape(url)?;
            let max_chars = object
                .get("max_chars")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(16_000)
                .min(32_000);
            let max_chars_u32 = u32::try_from(max_chars).unwrap_or(32_000);
            Ok((
                BrokerRequest::Fetch(FetchRequest {
                    schema: FETCH_REQUEST_SCHEMA,
                    trace_id: trace.trace_id.to_string(),
                    url: url.to_string(),
                    max_chars: max_chars_u32,
                }),
                OutputKind::Fetch {
                    url: url.to_string(),
                    max_chars: usize::try_from(max_chars_u32).unwrap_or(32_000),
                },
            ))
        },
        _ => Err("native HTTP capsule invoked an unadvertised web tool".to_string()),
    }
}

fn validate_search_url(value: &str, expected_query: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(value).map_err(|_| "native search URL is invalid".to_string())?;
    if url.scheme() != "https"
        || url.host_str() != Some("search.brave.com")
        || url.port_or_known_default() != Some(443)
        || url.path() != "/search"
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("native search URL escaped the compiled provider".to_string());
    }
    let pairs = url.query_pairs().collect::<Vec<_>>();
    if pairs.len() != 2
        || pairs.iter().filter(|(key, _)| key == "q").count() != 1
        || pairs.iter().filter(|(key, _)| key == "source").count() != 1
        || !pairs
            .iter()
            .any(|(key, value)| key == "q" && value == expected_query)
        || !pairs
            .iter()
            .any(|(key, value)| key == "source" && value == "web")
    {
        return Err("native search URL query does not match tool arguments".to_string());
    }
    Ok(())
}

fn validate_public_url_shape(value: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(value).map_err(|_| "native fetch URL is invalid".to_string())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("native fetch URL shape is not public HTTPS eligible".to_string());
    }
    Ok(())
}

async fn post(config: &Config, request: &BrokerRequest) -> Result<Vec<u8>, String> {
    let socket_before = validate_socket(&config.socket_path)?;
    let request_key = read_exact_credential(&config.request_credential, None)?;
    let response_key = read_exact_credential(
        &config.response_credential,
        Some(&config.response_key_sha256),
    )?;
    let body = request.body()?;
    if body.is_empty() || body.len() > 4_096 {
        return Err("immutable web-broker request body exceeds bounds".to_string());
    }
    let nonce = nonce()?;
    let request_hash = request_hash(request.path(), &nonce, &body);
    let authentication = request_authentication(&request_key, request.path(), &nonce, &body);
    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::HOST,
        HeaderValue::from_static(BROKER_AUTHORITY),
    );
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        reqwest::header::ACCEPT,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        reqwest::header::CONNECTION,
        HeaderValue::from_static("close"),
    );
    headers.insert(
        HeaderName::from_static(CLIENT_HEADER),
        HeaderValue::from_static(CLIENT_ID),
    );
    headers.insert(
        HeaderName::from_static(NONCE_HEADER),
        HeaderValue::from_str(&nonce).map_err(|error| error.to_string())?,
    );
    headers.insert(
        HeaderName::from_static(AUTH_HEADER),
        HeaderValue::from_str(&authentication).map_err(|error| error.to_string())?,
    );
    let client = reqwest::Client::builder()
        .unix_socket(config.socket_path.clone())
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|error| format!("build immutable web-broker client: {error}"))?;
    let response = client
        .post(format!("http://{BROKER_AUTHORITY}{}", request.path()))
        .headers(headers)
        .body(body)
        .send()
        .await
        .map_err(|error| format!("immutable web-broker request failed: {error}"))?;
    let socket_after = validate_socket(&config.socket_path)?;
    if socket_before != socket_after {
        return Err("immutable web-broker socket changed during request".to_string());
    }
    let status = response.status().as_u16();
    let response_headers = response.headers().clone();
    let response_body = response
        .bytes()
        .await
        .map_err(|error| format!("read immutable web-broker response: {error}"))?;
    if response_body.is_empty() || response_body.len() > request.maximum_response_bytes() {
        return Err("immutable web-broker response body exceeds bounds".to_string());
    }
    verify_response(
        &response_key,
        &response_headers,
        &nonce,
        status,
        &request_hash,
        &response_body,
    )?;
    if status != 200 {
        let error: ErrorResponse = serde_json::from_slice(&response_body)
            .map_err(|_| "immutable web-broker error is malformed".to_string())?;
        if error.schema != "astrid.edge.web_broker.error.v1"
            || error.code.is_empty()
            || error.code.len() > 64
            || !error
                .code
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
        {
            return Err("immutable web-broker error escaped schema".to_string());
        }
        return Err(format!(
            "immutable web broker rejected request: {}",
            error.code
        ));
    }
    Ok(response_body.to_vec())
}

fn verified_http_response(body: Vec<u8>) -> HttpResponseData {
    HttpResponseData {
        status: 200,
        headers: vec![KeyValuePair {
            key: BROKER_VERIFIED_HEADER.to_string(),
            value: "v1".to_string(),
        }],
        body,
    }
}

fn verify_response(
    public_key: &[u8; 32],
    headers: &HeaderMap,
    nonce: &str,
    status: u16,
    expected_request_hash: &str,
    body: &[u8],
) -> Result<(), String> {
    let exact = |name: &'static str| -> Result<&str, String> {
        let values = headers.get_all(name);
        let mut values = values.iter();
        let value = values
            .next()
            .ok_or_else(|| format!("immutable web-broker response omitted {name}"))?;
        if values.next().is_some() {
            return Err(format!("immutable web-broker response duplicated {name}"));
        }
        value
            .to_str()
            .map_err(|_| format!("immutable web-broker response {name} is invalid"))
    };
    if exact(CLIENT_HEADER)? != CLIENT_ID
        || exact(NONCE_HEADER)? != nonce
        || exact(REQUEST_HASH_HEADER)? != expected_request_hash
    {
        return Err("immutable web-broker response binding mismatch".to_string());
    }
    let signature = decode_hex_64(exact(SIGNATURE_HEADER)?)?;
    let key = VerifyingKey::from_bytes(public_key)
        .map_err(|_| "immutable web-broker response key is malformed".to_string())?;
    key.verify_strict(
        &response_signature_message(nonce, status, expected_request_hash, body),
        &Signature::from_bytes(&signature),
    )
    .map_err(|_| "immutable web-broker response signature is invalid".to_string())
}

fn request_authentication(key: &[u8; 32], path: &str, nonce: &str, body: &[u8]) -> String {
    let body_hash = Sha256::digest(body);
    hmac_fields(
        key,
        REQUEST_AUTH_DOMAIN,
        &[
            PROTOCOL_VERSION,
            CLIENT_ID.as_bytes(),
            path.as_bytes(),
            BROKER_AUTHORITY.as_bytes(),
            nonce.as_bytes(),
            &body_hash,
        ],
    )
}

fn request_hash(path: &str, nonce: &str, body: &[u8]) -> String {
    let body_hash = Sha256::digest(body);
    format!(
        "{:x}",
        Sha256::digest(encoded_fields(
            REQUEST_HASH_DOMAIN,
            &[
                PROTOCOL_VERSION,
                CLIENT_ID.as_bytes(),
                path.as_bytes(),
                BROKER_AUTHORITY.as_bytes(),
                nonce.as_bytes(),
                &body_hash,
            ],
        ))
    )
}

fn response_signature_message(
    nonce: &str,
    status: u16,
    request_hash: &str,
    body: &[u8],
) -> Vec<u8> {
    let status = status.to_string();
    let body_hash = Sha256::digest(body);
    encoded_fields(
        RESPONSE_SIGNATURE_DOMAIN,
        &[
            PROTOCOL_VERSION,
            CLIENT_ID.as_bytes(),
            nonce.as_bytes(),
            status.as_bytes(),
            request_hash.as_bytes(),
            &body_hash,
        ],
    )
}

fn hmac_fields(key: &[u8; 32], domain: &[u8], fields: &[&[u8]]) -> String {
    const BLOCK: usize = 64;
    let mut normalized = [0_u8; BLOCK];
    normalized[..key.len()].copy_from_slice(key);
    let mut inner_pad = [0x36_u8; BLOCK];
    let mut outer_pad = [0x5c_u8; BLOCK];
    for index in 0..BLOCK {
        inner_pad[index] ^= normalized[index];
        outer_pad[index] ^= normalized[index];
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(encoded_fields(domain, fields));
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner.finalize());
    format!("{:x}", outer.finalize())
}

fn encoded_fields(domain: &[u8], fields: &[&[u8]]) -> Vec<u8> {
    let mut encoded = Vec::new();
    append_field(&mut encoded, domain);
    for field in fields {
        append_field(&mut encoded, field);
    }
    encoded
}

fn append_field(target: &mut Vec<u8>, field: &[u8]) {
    target.extend_from_slice(&u64::try_from(field.len()).unwrap_or(u64::MAX).to_be_bytes());
    target.extend_from_slice(field);
}

fn nonce() -> Result<String, String> {
    let now = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "system clock is before Unix epoch".to_string())?
            .as_millis(),
    )
    .map_err(|_| "Unix milliseconds do not fit u64".to_string())?;
    let first = uuid::Uuid::new_v4().simple().to_string();
    let second = uuid::Uuid::new_v4().simple().to_string();
    Ok(format!("{now:016x}{}", &format!("{first}{second}")[..48]))
}

fn read_exact_credential(path: &Path, expected_hash: Option<&str>) -> Result<[u8; 32], String> {
    reject_path(path)?;
    let before = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    validate_credential(&before)?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(path)
        .map_err(|error| error.to_string())?;
    let opened = file.metadata().map_err(|error| error.to_string())?;
    validate_credential(&opened)?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(33)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let after = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    validate_credential(&after)?;
    let identity = |metadata: &fs::Metadata| (metadata.dev(), metadata.ino(), metadata.len());
    if identity(&before) != identity(&opened) || identity(&opened) != identity(&after) {
        return Err("immutable web-broker credential changed while reading".to_string());
    }
    let key: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "immutable web-broker credential is not exactly 32 bytes".to_string())?;
    if expected_hash.is_some_and(|digest| format!("{:x}", Sha256::digest(key)) != digest) {
        return Err("immutable web-broker credential identity mismatch".to_string());
    }
    Ok(key)
}

fn validate_credential(metadata: &fs::Metadata) -> Result<(), String> {
    let effective_uid = nix::unistd::geteuid().as_raw();
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || ![0, effective_uid].contains(&metadata.uid())
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.len() != 32
    {
        return Err("immutable web-broker credential metadata is invalid".to_string());
    }
    Ok(())
}

fn validate_socket(path: &Path) -> Result<(u64, u64), String> {
    if path != Path::new(BROKER_SOCKET) {
        return Err("immutable web-broker socket escaped its exact endpoint".to_string());
    }
    let parent = path
        .parent()
        .ok_or_else(|| "immutable web-broker socket has no parent".to_string())?;
    let parent_metadata = fs::symlink_metadata(parent).map_err(|error| error.to_string())?;
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !parent_metadata.is_dir()
        || parent_metadata.file_type().is_symlink()
        || parent_metadata.uid() != 0
        || parent_metadata.permissions().mode() & 0o022 != 0
        || !metadata.file_type().is_socket()
        || metadata.file_type().is_symlink()
        || metadata.uid() != 0
        || metadata.nlink() != 1
        || metadata.permissions().mode() & 0o7777 != 0o660
    {
        return Err("immutable web-broker socket metadata is invalid".to_string());
    }
    Ok((metadata.dev(), metadata.ino()))
}

fn configuration_from_values(
    socket: Option<std::ffi::OsString>,
    request: Option<std::ffi::OsString>,
    response: Option<std::ffi::OsString>,
    digest: Option<String>,
) -> Result<Option<Config>, String> {
    match (socket, request, response, digest) {
        (None, None, None, None) => Ok(None),
        (Some(socket), Some(request), Some(response), Some(digest)) => {
            let config = Config {
                socket_path: socket.into(),
                request_credential: request.into(),
                response_credential: response.into(),
                response_key_sha256: digest,
            };
            if config.socket_path != Path::new(BROKER_SOCKET)
                || config.request_credential != Path::new(REQUEST_CREDENTIAL)
                || config.response_credential != Path::new(RESPONSE_CREDENTIAL)
                || !is_lower_hex64(&config.response_key_sha256)
            {
                return Err("core web-broker paths or key identity escaped contract".to_string());
            }
            Ok(Some(config))
        },
        _ => Err("core web-broker configuration is partial".to_string()),
    }
}

fn reject_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute()
        || path == Path::new("/")
        || !path
            .components()
            .all(|component| matches!(component, Component::RootDir | Component::Normal(_)))
    {
        return Err("immutable web-broker credential path is invalid".to_string());
    }
    Ok(())
}

fn is_lower_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn decode_hex_64(value: &str) -> Result<[u8; 64], String> {
    if value.len() != 128
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err("immutable web-broker signature is not canonical".to_string());
    }
    let mut decoded = [0_u8; 64];
    for (index, output) in decoded.iter_mut().enumerate() {
        let offset = index.saturating_mul(2);
        *output = u8::from_str_radix(&value[offset..offset.saturating_add(2)], 16)
            .map_err(|_| "immutable web-broker signature is malformed".to_string())?;
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use astrid_events::ipc::{IpcMessage, IpcPayload, IpcProducerV1, IpcTraceContextV1};
    use ed25519_dalek::{Signer as _, SigningKey};
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        BROKER_AUTHORITY, BROKER_SOCKET, CLIENT_ID, FETCH_PATH, HTTP_CAPSULE_ID,
        REQUEST_CREDENTIAL, RESPONSE_CREDENTIAL, SEARCH_PATH, configuration_from_values,
        request_authentication, request_hash, response_signature_message, validate_search_url,
        validated_request,
    };
    use crate::engine::wasm::bindings::astrid::capsule::types::HttpRequestData;

    fn traced_call(tool_name: &str, arguments: serde_json::Value) -> IpcMessage {
        IpcMessage::new(
            format!("tool.v1.execute.{tool_name}"),
            IpcPayload::ToolExecuteRequest {
                call_id: "call-web-1".to_string(),
                tool_name: tool_name.to_string(),
                arguments,
            },
            Uuid::new_v4(),
        )
        .with_trace(IpcTraceContextV1::root(
            Uuid::new_v4(),
            "edge-session",
            Some("chain-1".to_string()),
        ))
        .with_producer(IpcProducerV1::new("wasm_capsule", "astrid-capsule-react"))
    }

    #[test]
    fn configuration_is_absent_or_exact_and_never_partial() {
        assert!(
            configuration_from_values(None, None, None, None)
                .unwrap()
                .is_none()
        );
        let config = configuration_from_values(
            Some(BROKER_SOCKET.into()),
            Some(REQUEST_CREDENTIAL.into()),
            Some(RESPONSE_CREDENTIAL.into()),
            Some("a".repeat(64)),
        )
        .unwrap()
        .unwrap();
        assert_eq!(config.socket_path, std::path::Path::new(BROKER_SOCKET));
        assert!(
            configuration_from_values(
                Some("/tmp/web.sock".into()),
                Some(REQUEST_CREDENTIAL.into()),
                Some(RESPONSE_CREDENTIAL.into()),
                Some("a".repeat(64)),
            )
            .is_err()
        );
        assert!(
            configuration_from_values(
                Some(BROKER_SOCKET.into()),
                None,
                Some(RESPONSE_CREDENTIAL.into()),
                Some("a".repeat(64)),
            )
            .is_err()
        );
    }

    #[test]
    fn search_requires_exact_caller_trace_arguments_and_compiled_url() {
        let query = "current reservoir computing research";
        let caller = traced_call("search_web", json!({"query": query, "count": 5}));
        let request = HttpRequestData {
            method: "GET".to_string(),
            url: "https://search.brave.com/search?q=current%20reservoir%20computing%20research&source=web"
                .to_string(),
            headers: vec![],
            body: None,
        };
        let (validated, _) = validated_request(Some(&caller), &request).unwrap();
        assert_eq!(validated.path(), SEARCH_PATH);
        assert!(
            validate_search_url(
                "https://search.brave.com/search?q=different&source=web",
                query
            )
            .is_err()
        );

        let mut forged = caller.clone();
        forged.producer = Some(IpcProducerV1::new(
            "native_socket_client",
            "astrid-capsule-react",
        ));
        assert!(validated_request(Some(&forged), &request).is_err());
        let mut untraced = caller;
        untraced.trace = None;
        assert!(validated_request(Some(&untraced), &request).is_err());
    }

    #[test]
    fn fetch_rejects_headers_non_get_and_argument_url_mismatch() {
        let caller = traced_call(
            "fetch_url",
            json!({"url":"https://example.com/paper","method":"GET","max_chars":8000}),
        );
        let request = HttpRequestData {
            method: "GET".to_string(),
            url: "https://example.com/paper".to_string(),
            headers: vec![],
            body: None,
        };
        let (validated, _) = validated_request(Some(&caller), &request).unwrap();
        assert_eq!(validated.path(), FETCH_PATH);

        let mut headed = request.clone();
        headed.headers.push(
            crate::engine::wasm::bindings::astrid::capsule::types::KeyValuePair {
                key: "Authorization".to_string(),
                value: "secret".to_string(),
            },
        );
        assert!(validated_request(Some(&caller), &headed).is_err());
        let mut wrong = request;
        wrong.url = "https://other.example/".to_string();
        assert!(validated_request(Some(&caller), &wrong).is_err());
    }

    #[test]
    fn authentication_binds_core_client_route_nonce_and_response_body() {
        let key = [0x42; 32];
        let nonce = format!("{:016x}{}", 1_700_000_000_000_u64, "a".repeat(48));
        let body = br#"{"schema":"astrid.edge.web_search.request.v2"}"#;
        let authentication = request_authentication(&key, SEARCH_PATH, &nonce, body);
        assert_eq!(authentication.len(), 64);
        assert_ne!(
            authentication,
            request_authentication(&key, FETCH_PATH, &nonce, body)
        );
        let request_hash = request_hash(SEARCH_PATH, &nonce, body);
        let signing = SigningKey::from_bytes(&[0x43; 32]);
        let message = response_signature_message(&nonce, 200, &request_hash, b"response");
        let signature = signing.sign(&message);
        signing
            .verifying_key()
            .verify_strict(&message, &signature)
            .unwrap();
        assert_eq!(CLIENT_ID, "edge-core");
        assert_eq!(BROKER_AUTHORITY, "astrid-edge-web-broker");
        assert_eq!(HTTP_CAPSULE_ID, "astrid-capsule-http");
    }
}
