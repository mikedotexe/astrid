//! Client for the immutable, client-isolated Unix-socket CPU-edge web broker.

use std::fs::{self, OpenOptions};
use std::io::Read as _;
use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use ed25519_dalek::{Signature, VerifyingKey};
use rand::{RngCore as _, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::UnixStream;

use crate::config::Config;

const SEARCH_PATH: &str = "/v1/search";
const FETCH_PATH: &str = "/v1/fetch";
const SEARCH_REQUEST_SCHEMA: &str = "astrid.edge.web_search.request.v2";
const SEARCH_RESPONSE_SCHEMA: &str = "astrid.edge.web_search.response.v1";
const FETCH_REQUEST_SCHEMA: &str = "astrid.edge.web_fetch.request.v2";
const FETCH_RESPONSE_SCHEMA: &str = "astrid.edge.web_fetch.response.v1";
const MAXIMUM_HEADER_BYTES: usize = 16 * 1_024;
const MAXIMUM_SEARCH_RESPONSE_BYTES: usize = 64 * 1_024;
const MAXIMUM_FETCH_RESPONSE_BYTES: usize = 512 * 1_024;
const PROTOCOL_VERSION: &[u8] = b"astrid.edge.web_broker.auth.v2";
const CLIENT_ID: &str = "edge-runtime";
const REQUEST_AUTH_DOMAIN: &[u8] = b"astrid.edge.web_broker.request_auth.v2";
const REQUEST_HASH_DOMAIN: &[u8] = b"astrid.edge.web_broker.request_hash.v2";
const RESPONSE_SIGNATURE_DOMAIN: &[u8] = b"astrid.edge.web_broker.response_signature.v2";
const CLIENT_HEADER: &str = "x-astrid-web-client";
const NONCE_HEADER: &str = "x-astrid-web-nonce";
const REQUEST_HASH_HEADER: &str = "x-astrid-web-request-hash";
const SIGNATURE_HEADER: &str = "x-astrid-web-signature";
const BROKER_HTTP_AUTHORITY: &str = "astrid-edge-web-broker";
const RUNTIME_SOCKET_PATH: &str = "/run/astrid-edge-self-change/web-runtime.sock";

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct SearchRequest<'a> {
    schema: &'static str,
    trace_id: &'a str,
    query: &'a str,
    limit: u8,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SearchResponse {
    schema: String,
    results: Vec<SearchResult>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct FetchRequest<'a> {
    schema: &'static str,
    trace_id: &'a str,
    url: &'a str,
    max_chars: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FetchResponse {
    pub schema: String,
    pub url: String,
    pub status: u16,
    pub original_body_bytes: u64,
    pub truncated: bool,
    pub body: String,
}

/// Validate the immutable broker socket configured for the mutable runtime.
///
/// # Errors
///
/// Returns an error unless this is the exact root-created runtime endpoint.
pub fn validate_socket_path(value: &Path) -> Result<()> {
    if value != Path::new(RUNTIME_SOCKET_PATH) {
        bail!("web broker socket escaped the exact runtime endpoint");
    }
    Ok(())
}

/// Perform a bounded search through the immutable broker.
///
/// # Errors
///
/// Returns an error for unavailable configuration, invalid requests, transport
/// failures, ambiguous framing, or a response outside the immutable schema.
pub async fn search(
    config: &Config,
    trace_id: &str,
    query: &str,
    limit: u8,
) -> Result<Vec<SearchResult>> {
    let query = query.trim();
    if query.is_empty()
        || query.chars().count() > 160
        || query.chars().any(char::is_control)
        || !(1..=5).contains(&limit)
    {
        bail!("immutable broker search request exceeds bounds");
    }
    let response = post_json(
        config,
        SEARCH_PATH,
        &SearchRequest {
            schema: SEARCH_REQUEST_SCHEMA,
            trace_id,
            query,
            limit,
        },
        MAXIMUM_SEARCH_RESPONSE_BYTES,
    )
    .await?;
    let response: SearchResponse =
        serde_json::from_slice(&response).context("decode immutable broker search response")?;
    if response.schema != SEARCH_RESPONSE_SCHEMA || response.results.len() > usize::from(limit) {
        bail!("immutable broker search response escaped schema or count");
    }
    Ok(response.results)
}

/// Retrieve bounded readable source text through the immutable broker.
///
/// # Errors
///
/// Returns an error for unavailable configuration, invalid requests, transport
/// failures, ambiguous framing, or a response outside the immutable schema.
pub async fn fetch(
    config: &Config,
    trace_id: &str,
    url: &str,
    max_chars: u32,
) -> Result<FetchResponse> {
    if !(256..=64 * 1_024).contains(&max_chars) {
        bail!("immutable broker fetch character bound is invalid");
    }
    let response = post_json(
        config,
        FETCH_PATH,
        &FetchRequest {
            schema: FETCH_REQUEST_SCHEMA,
            trace_id,
            url,
            max_chars,
        },
        MAXIMUM_FETCH_RESPONSE_BYTES,
    )
    .await?;
    let response: FetchResponse =
        serde_json::from_slice(&response).context("decode immutable broker fetch response")?;
    if response.schema != FETCH_RESPONSE_SCHEMA
        || response.status != 200
        || response.url.chars().count() > 2_048
        || response.body.chars().count() > usize::try_from(max_chars).unwrap_or(64 * 1_024)
        || response
            .body
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        bail!("immutable broker fetch response escaped schema or text bounds");
    }
    Ok(response)
}

async fn post_json(
    config: &Config,
    path: &str,
    request: &impl Serialize,
    maximum_response_bytes: usize,
) -> Result<Vec<u8>> {
    let socket_path = config
        .web_broker_socket_path
        .as_deref()
        .context("immutable web broker is not configured")?;
    validate_socket_path(socket_path)?;
    let before = validate_socket_filesystem(socket_path)?;
    validate_timeouts(config)?;
    let request_key = load_request_key(config)?;
    let response_verify_key = load_response_verify_key(config)?;
    let body = serde_json::to_vec(request)?;
    if body.is_empty() || body.len() > 4_096 {
        bail!("immutable broker request body escaped bounds");
    }
    let nonce = request_nonce()?;
    let authentication =
        request_signature(&request_key, path, BROKER_HTTP_AUTHORITY, &nonce, &body)?;
    let expected_request_hash = request_hash(path, BROKER_HTTP_AUTHORITY, &nonce, &body)?;
    let started = tokio::time::Instant::now();
    let header_deadline = checked_deadline(started, config.web_broker_header_timeout_ms)?;
    let total_deadline = checked_deadline(started, config.web_broker_total_timeout_ms)?;
    let mut socket = tokio::time::timeout(
        Duration::from_millis(config.web_broker_connect_timeout_ms),
        UnixStream::connect(socket_path),
    )
    .await
    .context("immutable broker connection deadline exceeded")??;
    let after = validate_socket_filesystem(socket_path)?;
    if before.dev() != after.dev() || before.ino() != after.ino() {
        bail!("immutable broker socket changed while connecting");
    }
    let request_header = format!(
        "POST {path} HTTP/1.1\r\nHost: {BROKER_HTTP_AUTHORITY}\r\nContent-Type: application/json\r\nAccept: application/json\r\nConnection: close\r\nContent-Length: {}\r\nX-Astrid-Web-Client: {CLIENT_ID}\r\nX-Astrid-Web-Nonce: {nonce}\r\nX-Astrid-Web-Auth: {authentication}\r\n\r\n",
        body.len()
    );
    tokio::time::timeout_at(header_deadline, async {
        socket.write_all(request_header.as_bytes()).await?;
        socket.write_all(&body).await?;
        socket.flush().await
    })
    .await
    .context("immutable broker request-write deadline exceeded")??;
    let mut wire = Vec::new();
    let header_end = loop {
        let mut block = [0_u8; 4 * 1_024];
        let count = tokio::time::timeout_at(header_deadline, socket.read(&mut block))
            .await
            .context("immutable broker response-header deadline exceeded")??;
        if count == 0 {
            bail!("immutable broker closed before complete headers");
        }
        wire.extend_from_slice(&block[..count]);
        if let Some(position) = wire.windows(4).position(|value| value == b"\r\n\r\n") {
            break position.saturating_add(4);
        }
        if wire.len() > MAXIMUM_HEADER_BYTES {
            bail!("immutable broker response headers exceed bounds");
        }
    };
    if header_end > MAXIMUM_HEADER_BYTES {
        bail!("immutable broker response headers exceed bounds");
    }
    let headers = std::str::from_utf8(&wire[..header_end])
        .context("immutable broker response headers are not UTF-8")?;
    let response_headers = response_headers(headers)?;
    let length = response_headers.content_length;
    if length == 0 || length > maximum_response_bytes {
        bail!("immutable broker response body exceeds bounds");
    }
    let mut response_body = wire[header_end..].to_vec();
    if response_body.len() > length {
        bail!("immutable broker returned excess framed bytes");
    }
    while response_body.len() < length {
        let remaining = length.saturating_sub(response_body.len()).min(4 * 1_024);
        let mut block = [0_u8; 4 * 1_024];
        let count = tokio::time::timeout_at(total_deadline, socket.read(&mut block[..remaining]))
            .await
            .context("immutable broker total deadline exceeded")??;
        if count == 0 {
            bail!("immutable broker response ended before Content-Length");
        }
        response_body.extend_from_slice(&block[..count]);
    }
    if response_headers.client_id != CLIENT_ID
        || response_headers.nonce != nonce
        || response_headers.request_hash != expected_request_hash
    {
        bail!("immutable broker response identity does not match the exact request");
    }
    verify_response_signature(
        &response_verify_key,
        &nonce,
        200,
        &expected_request_hash,
        &response_body,
        &response_headers.signature,
    )?;
    Ok(response_body)
}

fn checked_deadline(
    started: tokio::time::Instant,
    timeout_ms: u64,
) -> Result<tokio::time::Instant> {
    started
        .checked_add(Duration::from_millis(timeout_ms))
        .context("immutable broker deadline overflow")
}

/// Verify the exact appliance-local client credential without retaining it.
///
/// # Errors
///
/// Returns an error when the configured path, file identity, mode, length, or
/// digest differs from the immutable broker-client contract.
pub(crate) fn validate_client_credential(config: &Config) -> Result<()> {
    let request_path = config
        .web_broker_request_key_path
        .as_deref()
        .context("immutable web broker request key path is not configured")?;
    let verify_path = config
        .web_broker_response_verify_key_path
        .as_deref()
        .context("immutable web broker verify key path is not configured")?;
    let request_hash = config
        .web_broker_request_key_sha256
        .as_deref()
        .context("immutable web broker request key hash is not configured")?;
    let verify_hash = config
        .web_broker_response_verify_key_sha256
        .as_deref()
        .context("immutable web broker verify key hash is not configured")?;
    if request_path == verify_path || request_hash == verify_hash {
        bail!("immutable web broker request and response identities must be separate");
    }
    let _ = load_request_key(config)?;
    let _ = load_response_verify_key(config)?;
    Ok(())
}

fn validate_socket_filesystem(path: &Path) -> Result<fs::Metadata> {
    validate_socket_path(path)?;
    let parent = path
        .parent()
        .context("immutable broker socket has no parent")?;
    let parent_metadata = fs::symlink_metadata(parent)?;
    if !parent_metadata.is_dir()
        || parent_metadata.file_type().is_symlink()
        || parent_metadata.uid() != 0
        || parent_metadata.permissions().mode() & 0o022 != 0
    {
        bail!("immutable broker socket parent is not root-owned and non-writable");
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_socket()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o7777 != 0o660
    {
        bail!("immutable broker socket type, owner, link count, or mode is invalid");
    }
    Ok(metadata)
}

fn validate_timeouts(config: &Config) -> Result<()> {
    if !(100..=5_000).contains(&config.web_broker_connect_timeout_ms)
        || !(500..=15_000).contains(&config.web_broker_header_timeout_ms)
        || config.web_broker_total_timeout_ms <= config.web_broker_header_timeout_ms
        || config.web_broker_total_timeout_ms > 60_000
    {
        bail!("immutable broker client deadlines escaped bounds");
    }
    Ok(())
}

struct ResponseHeaders {
    content_length: usize,
    client_id: String,
    nonce: String,
    request_hash: String,
    signature: String,
}

fn response_headers(headers: &str) -> Result<ResponseHeaders> {
    if !headers.is_ascii() {
        bail!("immutable broker response headers are not ASCII");
    }
    let mut lines = headers.split("\r\n");
    if lines.next() != Some("HTTP/1.1 200 OK") {
        bail!("immutable broker did not return exact HTTP 200");
    }
    let mut content_length = None;
    let mut content_type = false;
    let mut connection_close = false;
    let mut cache_control = false;
    let mut nosniff = false;
    let mut client_id = None;
    let mut nonce = None;
    let mut request_hash = None;
    let mut signature = None;
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .context("immutable broker response header is malformed")?;
        let name = name.to_ascii_lowercase();
        let value = value.trim();
        match name.as_str() {
            "content-length" if content_length.is_none() => {
                let parsed = value
                    .parse::<usize>()
                    .context("immutable broker Content-Length is invalid")?;
                if parsed.to_string() != value {
                    bail!("immutable broker Content-Length is not canonical");
                }
                content_length = Some(parsed);
            },
            "content-type" if !content_type && value == "application/json" => {
                content_type = true;
            },
            "connection" if !connection_close && value.eq_ignore_ascii_case("close") => {
                connection_close = true;
            },
            "cache-control" if !cache_control && value == "no-store" => {
                cache_control = true;
            },
            "x-content-type-options" if !nosniff && value == "nosniff" => {
                nosniff = true;
            },
            CLIENT_HEADER if client_id.is_none() && value == CLIENT_ID => {
                client_id = Some(value.to_string());
            },
            NONCE_HEADER if nonce.is_none() && is_lower_hex64(value) => {
                nonce = Some(value.to_string());
            },
            REQUEST_HASH_HEADER if request_hash.is_none() && is_lower_hex64(value) => {
                request_hash = Some(value.to_string());
            },
            SIGNATURE_HEADER if signature.is_none() && is_lower_hex128(value) => {
                signature = Some(value.to_string());
            },
            _ => bail!("immutable broker response header is duplicated or not allowlisted"),
        }
    }
    if !content_type || !connection_close || !cache_control || !nosniff {
        bail!("immutable broker response omitted an exact security header");
    }
    Ok(ResponseHeaders {
        content_length: content_length
            .context("immutable broker response omitted Content-Length")?,
        client_id: client_id.context("immutable broker response omitted client identity")?,
        nonce: nonce.context("immutable broker response omitted authentication nonce")?,
        request_hash: request_hash.context("immutable broker response omitted request hash")?,
        signature: signature.context("immutable broker response omitted response signature")?,
    })
}

fn load_request_key(config: &Config) -> Result<[u8; 32]> {
    let path = config
        .web_broker_request_key_path
        .as_deref()
        .context("immutable web broker request key path is not configured")?;
    let expected_hash = config
        .web_broker_request_key_sha256
        .as_deref()
        .context("immutable web broker request key hash is not configured")?;
    load_exact_key(path, expected_hash, "request key")
}

fn load_response_verify_key(config: &Config) -> Result<VerifyingKey> {
    let path = config
        .web_broker_response_verify_key_path
        .as_deref()
        .context("immutable web broker response verify key path is not configured")?;
    let expected_hash = config
        .web_broker_response_verify_key_sha256
        .as_deref()
        .context("immutable web broker response verify key hash is not configured")?;
    let bytes = load_exact_key(path, expected_hash, "response verify key")?;
    VerifyingKey::from_bytes(&bytes).context("immutable web broker verify key is malformed")
}

fn load_exact_key(path: &Path, expected_hash: &str, label: &str) -> Result<[u8; 32]> {
    if !is_lower_hex64(expected_hash) {
        bail!("immutable web broker {label} hash is not canonical");
    }
    reject_symlink_components(path)?;
    let before = fs::symlink_metadata(path)
        .with_context(|| format!("inspect web broker {label} {}", path.display()))?;
    validate_credential_metadata(&before)?;
    let mut file = OpenOptions::new()
        .read(true)
        .open(path)
        .with_context(|| format!("open web broker {label} {}", path.display()))?;
    let opened = file.metadata()?;
    validate_credential_metadata(&opened)?;
    let mut bytes = Vec::new();
    file.by_ref().take(33).read_to_end(&mut bytes)?;
    let after = fs::symlink_metadata(path)?;
    validate_credential_metadata(&after)?;
    if before.dev() != opened.dev()
        || before.ino() != opened.ino()
        || opened.dev() != after.dev()
        || opened.ino() != after.ino()
        || before.len() != opened.len()
        || opened.len() != after.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        bail!("immutable web broker {label} changed while reading");
    }
    let key: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("immutable web broker {label} is not exactly 32 bytes"))?;
    if format!("{:x}", Sha256::digest(key)) != expected_hash {
        bail!("immutable web broker {label} digest mismatch");
    }
    Ok(key)
}

fn reject_symlink_components(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        bail!("immutable web broker credential path must be absolute");
    }
    let mut cursor = PathBuf::new();
    let components = path.components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        cursor.push(component);
        if cursor == Path::new("/") {
            continue;
        }
        let metadata = fs::symlink_metadata(&cursor)?;
        if metadata.file_type().is_symlink() {
            bail!("immutable web broker credential path contains a symlink");
        }
        if index.saturating_add(1) < components.len()
            && (!metadata.is_dir() || metadata.permissions().mode() & 0o022 != 0)
        {
            bail!("immutable web broker credential ancestor is writable or not a directory");
        }
    }
    Ok(())
}

fn validate_credential_metadata(metadata: &fs::Metadata) -> Result<()> {
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.permissions().mode() & 0o7777 != 0o400
        || metadata.len() != 32
    {
        bail!(
            "immutable web broker credential must be regular, nlink-one, mode 0400, and exactly 32 bytes"
        );
    }
    Ok(())
}

fn request_nonce() -> Result<String> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("system clock precedes the Unix epoch")?;
    let millis = u64::try_from(timestamp.as_millis()).unwrap_or(u64::MAX);
    let mut random = [0_u8; 24];
    OsRng.fill_bytes(&mut random);
    Ok(format!("{millis:016x}{}", lower_hex(&random)))
}

fn request_signature(
    key: &[u8; 32],
    path: &str,
    host: &str,
    nonce: &str,
    body: &[u8],
) -> Result<String> {
    if !is_lower_hex64(nonce) {
        bail!("immutable web broker request nonce is not canonical");
    }
    let body_hash = Sha256::digest(body);
    Ok(hmac_fields(
        key,
        REQUEST_AUTH_DOMAIN,
        &[
            PROTOCOL_VERSION,
            CLIENT_ID.as_bytes(),
            path.as_bytes(),
            host.as_bytes(),
            nonce.as_bytes(),
            &body_hash,
        ],
    ))
}

fn request_hash(path: &str, host: &str, nonce: &str, body: &[u8]) -> Result<String> {
    if !is_lower_hex64(nonce) {
        bail!("immutable web broker request nonce is not canonical");
    }
    let body_hash = Sha256::digest(body);
    Ok(format!(
        "{:x}",
        Sha256::digest(encoded_fields(
            REQUEST_HASH_DOMAIN,
            &[
                PROTOCOL_VERSION,
                CLIENT_ID.as_bytes(),
                path.as_bytes(),
                host.as_bytes(),
                nonce.as_bytes(),
                &body_hash,
            ],
        ))
    ))
}

fn verify_response_signature(
    key: &VerifyingKey,
    nonce: &str,
    status: u16,
    request_hash: &str,
    body: &[u8],
    supplied_signature: &str,
) -> Result<()> {
    let message = response_message(nonce, status, request_hash, body)?;
    let signature_bytes = decode_hex_64(supplied_signature)?;
    let signature = Signature::from_bytes(&signature_bytes);
    key.verify_strict(&message, &signature)
        .context("immutable web broker response signature verification failed")
}

fn response_message(nonce: &str, status: u16, request_hash: &str, body: &[u8]) -> Result<Vec<u8>> {
    if !is_lower_hex64(nonce) || !is_lower_hex64(request_hash) {
        bail!("immutable web broker response binding is not canonical");
    }
    let status = status.to_string();
    let body_hash = Sha256::digest(body);
    Ok(encoded_fields(
        RESPONSE_SIGNATURE_DOMAIN,
        &[
            PROTOCOL_VERSION,
            CLIENT_ID.as_bytes(),
            nonce.as_bytes(),
            status.as_bytes(),
            request_hash.as_bytes(),
            &body_hash,
        ],
    ))
}

fn hmac_fields(key: &[u8; 32], domain: &[u8], fields: &[&[u8]]) -> String {
    const BLOCK_BYTES: usize = 64;
    let mut normalized = [0_u8; BLOCK_BYTES];
    normalized[..key.len()].copy_from_slice(key);
    let mut inner_pad = [0x36_u8; BLOCK_BYTES];
    let mut outer_pad = [0x5c_u8; BLOCK_BYTES];
    for index in 0..BLOCK_BYTES {
        inner_pad[index] ^= normalized[index];
        outer_pad[index] ^= normalized[index];
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(encoded_fields(domain, fields));
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_digest);
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

fn append_field(output: &mut Vec<u8>, field: &[u8]) {
    output.extend_from_slice(&u64::try_from(field.len()).unwrap_or(u64::MAX).to_be_bytes());
    output.extend_from_slice(field);
}

fn is_lower_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn is_lower_hex128(value: &str) -> bool {
    value.len() == 128
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn decode_hex_64(value: &str) -> Result<[u8; 64]> {
    if !is_lower_hex128(value) {
        bail!("immutable web broker response signature is not canonical");
    }
    let mut output = [0_u8; 64];
    for (index, byte) in output.iter_mut().enumerate() {
        let offset = index.saturating_mul(2);
        *byte = u8::from_str_radix(&value[offset..offset.saturating_add(2)], 16)
            .context("immutable web broker response signature is malformed")?;
    }
    Ok(output)
}

fn lower_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer as _, SigningKey};

    use super::{
        CLIENT_ID, lower_hex, request_hash, request_signature, response_headers, response_message,
        validate_socket_path, verify_response_signature,
    };

    #[test]
    fn socket_path_is_the_exact_runtime_endpoint_only() {
        assert!(
            validate_socket_path(std::path::Path::new(
                "/run/astrid-edge-self-change/web-runtime.sock"
            ))
            .is_ok()
        );
        for value in [
            "/run/astrid-edge-self-change/web-steward.sock",
            "/tmp/web-runtime.sock",
            "/run/astrid-edge-self-change/../web-runtime.sock",
        ] {
            assert!(
                validate_socket_path(std::path::Path::new(value)).is_err(),
                "accepted {value}"
            );
        }
    }

    #[test]
    fn response_headers_are_exact_and_unambiguous() {
        let valid = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 3\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nX-Astrid-Web-Client: {CLIENT_ID}\r\nX-Astrid-Web-Nonce: {}\r\nX-Astrid-Web-Request-Hash: {}\r\nX-Astrid-Web-Signature: {}\r\n\r\n",
            "a".repeat(64),
            "b".repeat(64),
            "c".repeat(128)
        );
        assert_eq!(response_headers(&valid).unwrap().content_length, 3);
        assert!(
            response_headers(&valid.replace("Content-Length: 3", "Content-Length: 03")).is_err()
        );
        assert!(
            response_headers(&valid.replace("\r\n\r\n", "\r\nTransfer-Encoding: chunked\r\n\r\n"))
                .is_err()
        );
    }

    #[test]
    fn request_authentication_and_response_signature_bind_every_identity() {
        let key = [0x42; 32];
        let nonce = "a".repeat(64);
        let request_auth = request_signature(
            &key,
            "/v1/search",
            "astrid-edge-web-broker",
            &nonce,
            b"body",
        )
        .unwrap();
        assert_ne!(
            request_auth,
            request_signature(&key, "/v1/fetch", "astrid-edge-web-broker", &nonce, b"body")
                .unwrap()
        );
        let request_hash =
            request_hash("/v1/search", "astrid-edge-web-broker", &nonce, b"body").unwrap();
        let signing = SigningKey::from_bytes(&[0x43; 32]);
        let message = response_message(&nonce, 200, &request_hash, b"body").unwrap();
        let signature = lower_hex(&signing.sign(&message).to_bytes());
        verify_response_signature(
            &signing.verifying_key(),
            &nonce,
            200,
            &request_hash,
            b"body",
            &signature,
        )
        .unwrap();
        assert!(
            verify_response_signature(
                &SigningKey::from_bytes(&[0x44; 32]).verifying_key(),
                &nonce,
                200,
                &request_hash,
                b"body",
                &signature,
            )
            .is_err()
        );
    }
}
