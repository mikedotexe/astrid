use reqwest::header::{CONNECTION, HeaderMap, HeaderName, HeaderValue};
use sha2::{Digest as _, Sha256};
use std::fs::{self, OpenOptions};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::engine::wasm::bindings::astrid::capsule::http;
use crate::engine::wasm::bindings::astrid::capsule::types::{
    HttpRequestData, HttpResponseData, HttpStreamStartResponse, KeyValuePair,
};
use crate::engine::wasm::host::util;
use crate::engine::wasm::host_state::HostState;
use astrid_events::ipc::{
    IpcMessage, IpcPayload, IpcTraceContextV1, LocalProviderRequestOutcomeV1,
};

const LOCAL_PROVIDER_LLM_REQUEST_TOPIC: &str = "llm.v1.request.generate.openai-compat";
const LOCAL_PROVIDER_CAPSULE_ID: &str = "astrid-capsule-openai-compat";
const LOCAL_PROVIDER_COMPLETIONS_PATH: &str = "/v1/chat/completions";
const REACT_CAPSULE_ID: &str = "astrid-capsule-react";
const PROVIDER_BROKER_SOCKET: &str = "/run/astrid-edge-self-change/provider-runtime.sock";
const PROVIDER_BROKER_AUTHORITY: &str = "astrid-edge-provider";
const PROVIDER_BROKER_CLIENT: &str = "edge-runtime";
const PROVIDER_BROKER_CLIENT_HEADER: &str = "x-astrid-provider-client";
const PROVIDER_BROKER_NONCE_HEADER: &str = "x-astrid-provider-nonce";
const PROVIDER_BROKER_AUTH_HEADER: &str = "x-astrid-provider-auth";
const PROVIDER_BROKER_DOMAIN: &[u8] = b"astrid.edge.provider_broker.request.v1";

#[derive(Clone, Debug)]
struct ProviderBrokerConfig {
    socket_path: PathBuf,
    credential_path: PathBuf,
}

#[derive(Debug)]
enum ProviderBrokerState {
    Absent,
    Configured(ProviderBrokerConfig),
    Invalid(String),
}

static PROVIDER_BROKER: std::sync::LazyLock<ProviderBrokerState> =
    std::sync::LazyLock::new(|| match provider_broker_from_environment() {
        Ok(Some(config)) => ProviderBrokerState::Configured(config),
        Ok(None) => ProviderBrokerState::Absent,
        Err(error) => {
            tracing::error!(%error, "invalid immutable local-provider broker configuration");
            ProviderBrokerState::Invalid(error)
        },
    });

fn configured_provider_broker() -> Result<Option<&'static ProviderBrokerConfig>, String> {
    match &*PROVIDER_BROKER {
        ProviderBrokerState::Absent => Ok(None),
        ProviderBrokerState::Configured(config) => Ok(Some(config)),
        ProviderBrokerState::Invalid(error) => Err(format!(
            "immutable local-provider broker configuration is invalid: {error}"
        )),
    }
}

fn provider_broker_from_environment() -> Result<Option<ProviderBrokerConfig>, String> {
    let socket = std::env::var_os("ASTRID_LOCAL_PROVIDER_UNIX_SOCKET");
    let credential = std::env::var_os("ASTRID_LOCAL_PROVIDER_CREDENTIAL");
    match (socket, credential) {
        (None, None) => Ok(None),
        (Some(socket), Some(credential)) => {
            let socket_path = PathBuf::from(socket);
            let credential_path = PathBuf::from(credential);
            if socket_path != Path::new(PROVIDER_BROKER_SOCKET)
                || !exact_absolute_path(&credential_path)
                || credential_path
                    .file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    != Some("provider-request.key")
                || !credential_path.starts_with("/run/credentials/astrid.service")
            {
                return Err(
                    "provider broker socket or credential escaped the root-owned contract"
                        .to_string(),
                );
            }
            Ok(Some(ProviderBrokerConfig {
                socket_path,
                credential_path,
            }))
        },
        _ => Err("provider broker socket and credential must be configured together".to_string()),
    }
}

fn exact_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path != Path::new("/")
        && path
            .components()
            .all(|component| matches!(component, Component::RootDir | Component::Normal(_)))
}

// ── SSRF prevention ──────────────────────────────────────────────────

/// A DNS resolver that prevents SSRF by blocking resolution to local,
/// private, or multicast IP addresses.
#[derive(Clone)]
struct SafeDnsResolver {
    /// Permit private resolution only for an explicitly allowlisted request
    /// origin. This is scoped per client/request rather than process-wide.
    allow_local_origin: bool,
}

impl reqwest::dns::Resolve for SafeDnsResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let name_str = name.as_str().to_string();
        let allow_local_origin = self.allow_local_origin;
        Box::pin(async move {
            let addrs = tokio::net::lookup_host((name_str.as_str(), 0))
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

            let mut safe_addrs = Vec::new();
            for addr in addrs {
                if allow_local_origin || is_safe_ip(addr.ip()) {
                    safe_addrs.push(addr);
                }
            }

            if safe_addrs.is_empty() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "DNS resolved to an unauthorized private or local IP address",
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }

            let iter: reqwest::dns::Addrs = Box::new(safe_addrs.into_iter());
            Ok(iter)
        })
    }
}

/// Checks if an IP address is safe to connect to (not local, private, or multicast).
/// Cached SSRF escape-hatch check. Evaluated once per process; logs a
/// warning on first access if either env var is set.
static SSRF_BYPASS: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
    if std::env::var("ASTRID_TEST_ALLOW_LOCAL_IP").is_ok() {
        tracing::warn!(
            "ASTRID_TEST_ALLOW_LOCAL_IP is set - SSRF protection disabled for ALL capsules"
        );
        return true;
    }
    if std::env::var("ASTRID_ALLOW_LOCAL_IPS").is_ok() {
        tracing::warn!(
            "ASTRID_ALLOW_LOCAL_IPS is set - SSRF protection disabled for ALL capsules. \
             Private/loopback IP ranges are reachable by every loaded capsule."
        );
        return true;
    }
    false
});

/// Exact `capsule@host:port` bindings that may resolve to private addresses.
///
/// Unlike `ASTRID_ALLOW_LOCAL_IPS`, this does not weaken unrelated capsules
/// or public-web requests. A typical local-provider deployment sets only
/// `astrid-capsule-openai-compat@127.0.0.1:11434`.
static LOCAL_HTTP_ALLOWLIST: std::sync::LazyLock<Vec<String>> = std::sync::LazyLock::new(|| {
    let entries = std::env::var("ASTRID_LOCAL_HTTP_ALLOWLIST")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    if !entries.is_empty() {
        tracing::info!(
            origins = ?entries,
            "private HTTP access enabled for exact local origins"
        );
    }
    entries
});

fn local_origin_allowed(capsule_id: &str, url: &reqwest::Url) -> bool {
    origin_allowed_by(capsule_id, url, &LOCAL_HTTP_ALLOWLIST)
}

fn origin_allowed_by(capsule_id: &str, url: &reqwest::Url, allowlist: &[String]) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let Some(port) = url.port_or_known_default() else {
        return false;
    };
    let binding = format!(
        "{}@{}:{port}",
        capsule_id.to_ascii_lowercase(),
        host.to_ascii_lowercase()
    );
    allowlist.iter().any(|entry| entry == &binding)
}

fn is_loopback_origin(url: &reqwest::Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    host.trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<std::net::IpAddr>()
        .is_ok_and(|ip| ip.is_loopback())
}

fn local_provider_attempt_allowed(
    capsule_id: &str,
    url: &reqwest::Url,
    method: &str,
    allowlist: &[String],
) -> bool {
    capsule_id == LOCAL_PROVIDER_CAPSULE_ID
        && method.eq_ignore_ascii_case("POST")
        && url.path() == LOCAL_PROVIDER_COMPLETIONS_PATH
        && url.query().is_none()
        && url.fragment().is_none()
        && (origin_allowed_by(capsule_id, url, allowlist)
            || provider_broker_origin_allowed(
                url,
                matches!(&*PROVIDER_BROKER, ProviderBrokerState::Configured(_)),
            ))
        && is_loopback_origin(url)
}

fn provider_broker_origin_allowed(url: &reqwest::Url, configured: bool) -> bool {
    configured
        && url.scheme() == "http"
        && url.host_str() == Some("127.0.0.1")
        && url.port() == Some(11434)
        && url.path() == LOCAL_PROVIDER_COMPLETIONS_PATH
        && url.query().is_none()
        && url.fragment().is_none()
}

fn provider_broker_request_headers(
    broker: &ProviderBrokerConfig,
    body: &[u8],
) -> Result<HeaderMap, String> {
    let key = read_provider_broker_key(&broker.credential_path)?;
    let nonce = provider_broker_nonce()?;
    let signature = provider_broker_signature(
        &key,
        PROVIDER_BROKER_CLIENT,
        LOCAL_PROVIDER_COMPLETIONS_PATH,
        &nonce,
        body,
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::HOST,
        HeaderValue::from_static(PROVIDER_BROKER_AUTHORITY),
    );
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        reqwest::header::ACCEPT,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(CONNECTION, HeaderValue::from_static("close"));
    headers.insert(
        HeaderName::from_static(PROVIDER_BROKER_CLIENT_HEADER),
        HeaderValue::from_static(PROVIDER_BROKER_CLIENT),
    );
    headers.insert(
        HeaderName::from_static(PROVIDER_BROKER_NONCE_HEADER),
        HeaderValue::from_str(&nonce).map_err(|error| error.to_string())?,
    );
    headers.insert(
        HeaderName::from_static(PROVIDER_BROKER_AUTH_HEADER),
        HeaderValue::from_str(&signature).map_err(|error| error.to_string())?,
    );
    Ok(headers)
}

fn read_provider_broker_key(path: &Path) -> Result<[u8; 32], String> {
    let before = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    let effective_uid = nix::unistd::geteuid().as_raw();
    validate_provider_credential(&before, effective_uid)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(path)
        .map_err(|error| error.to_string())?;
    let opened = file.metadata().map_err(|error| error.to_string())?;
    let after = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    validate_provider_credential(&opened, effective_uid)?;
    validate_provider_credential(&after, effective_uid)?;
    let identity = |metadata: &fs::Metadata| (metadata.dev(), metadata.ino());
    if identity(&before) != identity(&opened) || identity(&opened) != identity(&after) {
        return Err("provider broker credential changed during verified open".to_string());
    }
    let bytes = std::io::Read::bytes(file)
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|error| error.to_string())?;
    bytes
        .try_into()
        .map_err(|_| "provider broker credential must contain exactly 32 bytes".to_string())
}

fn validate_provider_credential(metadata: &fs::Metadata, effective_uid: u32) -> Result<(), String> {
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || ![0, effective_uid].contains(&metadata.uid())
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.len() != 32
    {
        return Err("provider broker credential identity is invalid".to_string());
    }
    Ok(())
}

fn provider_broker_nonce() -> Result<String, String> {
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

fn provider_broker_signature(
    key: &[u8; 32],
    client: &str,
    path: &str,
    nonce: &str,
    body: &[u8],
) -> String {
    let body_hash = Sha256::digest(body);
    provider_broker_hmac(
        key,
        PROVIDER_BROKER_DOMAIN,
        &[
            client.as_bytes(),
            path.as_bytes(),
            nonce.as_bytes(),
            &body_hash,
        ],
    )
}

fn provider_broker_hmac(key: &[u8; 32], domain: &[u8], fields: &[&[u8]]) -> String {
    const BLOCK: usize = 64;
    let mut normalized = [0_u8; BLOCK];
    normalized[..key.len()].copy_from_slice(key);
    let mut inner_pad = [0x36_u8; BLOCK];
    let mut outer_pad = [0x5c_u8; BLOCK];
    for index in 0..BLOCK {
        inner_pad[index] ^= normalized[index];
        outer_pad[index] ^= normalized[index];
    }
    let mut encoded = Vec::new();
    append_provider_field(&mut encoded, domain);
    for field in fields {
        append_provider_field(&mut encoded, field);
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(encoded);
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner.finalize());
    format!("{:x}", outer.finalize())
}

fn append_provider_field(target: &mut Vec<u8>, field: &[u8]) {
    target.extend_from_slice(&u64::try_from(field.len()).unwrap_or(u64::MAX).to_be_bytes());
    target.extend_from_slice(field);
}

fn validate_direct_ip(url: &reqwest::Url, allow_local_origin: bool) -> Result<(), String> {
    if allow_local_origin {
        return Ok(());
    }
    let ip = url
        .host_str()
        .and_then(|host| host.parse::<std::net::IpAddr>().ok());
    if ip.is_some_and(|ip| !is_safe_ip(ip)) {
        return Err("HTTP request targets an unauthorized private or local IP address".to_string());
    }
    Ok(())
}

fn redirect_policy(allow_local_origin: bool) -> reqwest::redirect::Policy {
    if allow_local_origin {
        return reqwest::redirect::Policy::none();
    }

    reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= 10 {
            return attempt.error("HTTP redirect limit exceeded");
        }
        if let Err(reason) = validate_direct_ip(attempt.url(), false) {
            return attempt.error(reason);
        }
        attempt.follow()
    })
}

fn is_safe_ip(mut ip: std::net::IpAddr) -> bool {
    if *SSRF_BYPASS {
        return true;
    }

    if let std::net::IpAddr::V6(ipv6) = ip {
        if let Some(ipv4) = ipv6.to_ipv4_mapped() {
            ip = std::net::IpAddr::V4(ipv4);
        } else if ipv6.segments()[..6].iter().all(|&s| s == 0) {
            // IPv4-compatible addresses (::x.x.x.x) are deprecated by RFC 4291
            // but must still be blocked (e.g. ::127.0.0.1 is loopback).
            let [.., hi, lo] = ipv6.segments();
            let [a, b] = hi.to_be_bytes();
            let [c, d] = lo.to_be_bytes();
            ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(a, b, c, d));
        }
    }

    if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() {
        return false;
    }

    match ip {
        std::net::IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            let is_private = octets[0] == 10
                || octets[0] == 0       // 0.0.0.0/8
                || octets[0] == 255     // Broadcast
                || (octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31)
                || (octets[0] == 192 && octets[1] == 168)
                || (octets[0] == 169 && octets[1] == 254)
                || (octets[0] == 100 && octets[1] >= 64 && octets[1] <= 127)
                || octets[0] == 127;
            !is_private
        },
        std::net::IpAddr::V6(ipv6) => {
            let segments = ipv6.segments();
            let is_private = (segments[0] & 0xfe00) == 0xfc00 || (segments[0] & 0xffc0) == 0xfe80;
            !is_private
        },
    }
}

// ── Shared helpers ───────────────────────────────────────────────────

/// Parse and validate an HTTP method string.
fn parse_method(method: &str) -> Result<reqwest::Method, String> {
    match method.to_uppercase().as_str() {
        "GET" => Ok(reqwest::Method::GET),
        "POST" => Ok(reqwest::Method::POST),
        "PUT" => Ok(reqwest::Method::PUT),
        "DELETE" => Ok(reqwest::Method::DELETE),
        "PATCH" => Ok(reqwest::Method::PATCH),
        "HEAD" => Ok(reqwest::Method::HEAD),
        "OPTIONS" => Ok(reqwest::Method::OPTIONS),
        other => Err(format!("unsupported http method: {other}")),
    }
}

/// Build a `HeaderMap` from a list of key-value pairs.
fn build_headers(raw: &[KeyValuePair]) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    for kv in raw {
        let h_name = HeaderName::from_bytes(kv.key.as_bytes())
            .map_err(|e| format!("invalid header name {}: {e}", kv.key))?;
        let h_value = HeaderValue::from_str(&kv.value)
            .map_err(|e| format!("invalid header value {}: {e}", kv.value))?;
        headers.insert(h_name, h_value);
    }
    Ok(headers)
}

/// Run the security gate check for an HTTP request.
fn check_http_security(
    security: &Option<Arc<dyn crate::security::CapsuleSecurityGate>>,
    capsule_id: String,
    url: &str,
    method: &str,
    runtime_handle: &tokio::runtime::Handle,
    host_semaphore: &Arc<tokio::sync::Semaphore>,
) -> Result<(), String> {
    if let Some(gate) = security {
        let url_obj = reqwest::Url::parse(url).map_err(|e| format!("invalid url {url}: {e}"))?;
        let _ = url_obj
            .host_str()
            .ok_or_else(|| "URL missing host".to_string())?;

        let full_url = url.to_string();
        let m = method.to_string();
        let gate = gate.clone();
        let check = util::bounded_block_on(runtime_handle, host_semaphore, async move {
            gate.check_http_request(&capsule_id, &m, &full_url).await
        });
        if let Err(reason) = check {
            return Err(format!("security denied network access: {reason}"));
        }
    }
    Ok(())
}

/// Maximum concurrent HTTP streaming responses per capsule.
const MAX_ACTIVE_HTTP_STREAMS: usize = 4;
/// Connect timeout for streaming HTTP requests.
const HTTP_STREAM_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// Maximum time to wait for streaming response headers.
const HTTP_STREAM_START_TIMEOUT: Duration = Duration::from_secs(300);
/// Maximum response-header deadline admitted for an exact allowlisted local
/// origin. The provider capsule's guest deadline is ordered above this plus
/// one complete inter-chunk wait.
pub(in crate::engine::wasm) const LOCAL_HTTP_STREAM_START_TIMEOUT_MAX_SECS: u64 = 420;
/// Per-chunk read timeout for streaming HTTP responses.
pub(in crate::engine::wasm) const HTTP_STREAM_READ_TIMEOUT_SECS: u64 = 120;
const HTTP_STREAM_READ_TIMEOUT: Duration = Duration::from_secs(HTTP_STREAM_READ_TIMEOUT_SECS);

/// Loopback provider header deadline. Public requests always retain the fixed
/// deadline above; this setting applies only after an exact
/// capsule@host:port allowlist match.
static LOCAL_HTTP_STREAM_START_TIMEOUT: std::sync::LazyLock<Duration> =
    std::sync::LazyLock::new(|| {
        let raw = std::env::var("ASTRID_LOCAL_HTTP_RESPONSE_HEADER_TIMEOUT_SECONDS").ok();
        let seconds = local_header_timeout_seconds(raw.as_deref());
        if raw.is_some() {
            tracing::info!(
                seconds,
                "configured response-header deadline for exact local HTTP origins"
            );
        }
        Duration::from_secs(seconds)
    });

fn local_header_timeout_seconds(raw: Option<&str>) -> u64 {
    raw.and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (60..=LOCAL_HTTP_STREAM_START_TIMEOUT_MAX_SECS).contains(seconds))
        .unwrap_or(HTTP_STREAM_START_TIMEOUT.as_secs())
}

fn response_header_timeout(allow_local_origin: bool) -> Duration {
    if allow_local_origin {
        *LOCAL_HTTP_STREAM_START_TIMEOUT
    } else {
        HTTP_STREAM_START_TIMEOUT
    }
}

async fn send_stream_request(
    request: reqwest::RequestBuilder,
    start_timeout: Duration,
) -> Result<reqwest::Response, SendStreamError> {
    match tokio::time::timeout(start_timeout, request.send()).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(error)) => Err(SendStreamError::Transport(error.to_string())),
        Err(_) => Err(SendStreamError::Timeout(start_timeout)),
    }
}

#[derive(Debug)]
enum SendStreamError {
    Transport(String),
    Timeout(Duration),
}

impl SendStreamError {
    const fn outcome(&self) -> LocalProviderRequestOutcomeV1 {
        match self {
            Self::Transport(_) => LocalProviderRequestOutcomeV1::TransportError,
            Self::Timeout(_) => LocalProviderRequestOutcomeV1::Timeout,
        }
    }
}

impl std::fmt::Display for SendStreamError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(error) => write!(formatter, "HTTP stream request failed: {error}"),
            Self::Timeout(timeout) => write!(
                formatter,
                "HTTP stream response headers timed out after {}s",
                timeout.as_secs()
            ),
        }
    }
}

#[derive(Clone)]
struct LocalProviderRequestContext {
    trace: IpcTraceContextV1,
    request_id: uuid::Uuid,
}

fn current_llm_request_context(caller: Option<&IpcMessage>) -> Option<LocalProviderRequestContext> {
    let current = caller?;
    if current.topic != LOCAL_PROVIDER_LLM_REQUEST_TOPIC {
        return None;
    }
    let producer = current
        .producer
        .as_ref()
        .filter(|producer| producer.is_supported())?;
    if producer.kind != "wasm_capsule" || producer.id != REACT_CAPSULE_ID {
        return None;
    }
    let IpcPayload::LlmRequest { request_id, .. } = &current.payload else {
        return None;
    };
    if request_id.is_nil() {
        return None;
    }
    let trace = current
        .trace
        .as_ref()
        .filter(|trace| {
            trace.is_supported() && trace.turn_id.is_some() && trace.session_id.is_some()
        })?
        .clone();
    Some(LocalProviderRequestContext {
        trace,
        request_id: *request_id,
    })
}

fn begin_local_provider_request(
    state: &HostState,
    context: Option<&LocalProviderRequestContext>,
) -> Option<uuid::Uuid> {
    let context = context?;
    state
        .event_bus
        .begin_local_provider_request(&context.trace, context.request_id)
}

fn finish_local_provider_request(
    state: &HostState,
    context: Option<&LocalProviderRequestContext>,
    attempt_id: Option<uuid::Uuid>,
    outcome: LocalProviderRequestOutcomeV1,
    elapsed: Option<Duration>,
) {
    let (Some(context), Some(attempt_id)) = (context, attempt_id) else {
        return;
    };
    let latency_ms = if outcome == LocalProviderRequestOutcomeV1::SuccessfulHeaders {
        elapsed.and_then(|duration| u64::try_from(duration.as_millis()).ok())
    } else {
        None
    };
    let _ = state.event_bus.finish_local_provider_request(
        &context.trace,
        attempt_id,
        outcome,
        latency_ms,
    );
}

fn local_provider_response_outcome(
    status: reqwest::StatusCode,
    remote_addr: Option<std::net::SocketAddr>,
    authenticated_unix_broker: bool,
) -> LocalProviderRequestOutcomeV1 {
    if authenticated_unix_broker {
        return if status.is_success() {
            LocalProviderRequestOutcomeV1::SuccessfulHeaders
        } else {
            LocalProviderRequestOutcomeV1::NonSuccessStatus
        };
    }
    match remote_addr {
        Some(address) if !address.ip().is_loopback() => {
            LocalProviderRequestOutcomeV1::NonLoopbackPeer
        },
        None => LocalProviderRequestOutcomeV1::UnknownPeer,
        Some(_) if status.is_success() => LocalProviderRequestOutcomeV1::SuccessfulHeaders,
        Some(_) => LocalProviderRequestOutcomeV1::NonSuccessStatus,
    }
}

impl http::Host for HostState {
    fn http_request(&mut self, request: HttpRequestData) -> Result<HttpResponseData, String> {
        let capsule_id = self.capsule_id.as_str().to_owned();
        let security = self.security.clone();
        let runtime_handle = self.runtime_handle.clone();
        let host_semaphore = self.host_semaphore.clone();

        check_http_security(
            &security,
            capsule_id.clone(),
            &request.url,
            &request.method,
            &runtime_handle,
            &host_semaphore,
        )?;

        #[cfg(unix)]
        if let Some(response) = super::core_web_broker::route(
            &capsule_id,
            self.caller_context.as_ref(),
            &request,
            &runtime_handle,
            &host_semaphore,
        )? {
            return Ok(response);
        }

        let parsed_url =
            reqwest::Url::parse(&request.url).map_err(|e| format!("invalid url: {e}"))?;
        let allow_local_origin = local_origin_allowed(&capsule_id, &parsed_url);
        validate_direct_ip(&parsed_url, allow_local_origin)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(redirect_policy(allow_local_origin))
            .dns_resolver(Arc::new(SafeDnsResolver { allow_local_origin }))
            .build()
            .map_err(|e| format!("failed to build http client: {e}"))?;

        let method = parse_method(&request.method)?;
        let headers = build_headers(&request.headers)?;

        let mut request_builder = client.request(method, &request.url).headers(headers);

        if let Some(body) = request.body {
            request_builder = request_builder.body(body);
        }

        let response = util::bounded_block_on(&runtime_handle, &host_semaphore, async move {
            request_builder.send().await
        })
        .map_err(|e| format!("http request failed: {e}"))?;

        let status = response.status().as_u16();

        let mut resp_headers = Vec::new();
        for (k, v) in response.headers() {
            if let Ok(v_str) = v.to_str() {
                resp_headers.push(KeyValuePair {
                    key: k.as_str().to_string(),
                    value: v_str.to_string(),
                });
            }
        }

        let body_result = util::bounded_block_on(&runtime_handle, &host_semaphore, async move {
            let mut response = response;
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
                if bytes.len() + chunk.len() > util::MAX_GUEST_PAYLOAD_LEN as usize {
                    return Err(format!(
                        "HTTP response exceeded maximum payload limit ({} bytes)",
                        util::MAX_GUEST_PAYLOAD_LEN
                    ));
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(bytes)
        });

        let body = body_result.map_err(|e| format!("failed to read http response body: {e}"))?;

        Ok(HttpResponseData {
            status,
            headers: resp_headers,
            body,
        })
    }

    fn http_stream_start(
        &mut self,
        request: HttpRequestData,
    ) -> Result<HttpStreamStartResponse, String> {
        // Check stream cap before doing any network I/O.
        if self.active_http_streams.len() >= MAX_ACTIVE_HTTP_STREAMS {
            return Err(format!(
                "HTTP stream cap reached ({}/{})",
                self.active_http_streams.len(),
                MAX_ACTIVE_HTTP_STREAMS
            ));
        }

        let capsule_id = self.capsule_id.as_str().to_owned();
        let security = self.security.clone();
        let runtime_handle = self.runtime_handle.clone();
        let host_semaphore = self.host_semaphore.clone();

        check_http_security(
            &security,
            capsule_id.clone(),
            &request.url,
            &request.method,
            &runtime_handle,
            &host_semaphore,
        )?;

        let parsed_url =
            reqwest::Url::parse(&request.url).map_err(|e| format!("invalid url: {e}"))?;
        let observe_local_provider = local_provider_attempt_allowed(
            &capsule_id,
            &parsed_url,
            &request.method,
            &LOCAL_HTTP_ALLOWLIST,
        );
        let provider_broker = if observe_local_provider {
            configured_provider_broker()?
        } else {
            None
        };
        let allow_local_origin = local_origin_allowed(&capsule_id, &parsed_url)
            || provider_broker_origin_allowed(&parsed_url, provider_broker.is_some());
        // Snapshot the validated, direct interceptor caller before network waiting. The selected
        // CPU-edge provider is a pooled executable interceptor, not a run-loop capsule; accepting
        // a remembered run-loop message here would make stale context look kernel-attested.
        let provider_context = observe_local_provider
            .then(|| current_llm_request_context(self.caller_context.as_ref()))
            .flatten();
        validate_direct_ip(&parsed_url, allow_local_origin)?;
        let mut client_builder = reqwest::Client::builder()
            .connect_timeout(HTTP_STREAM_CONNECT_TIMEOUT)
            .redirect(redirect_policy(allow_local_origin))
            .dns_resolver(Arc::new(SafeDnsResolver { allow_local_origin }));
        if allow_local_origin {
            // Local inference providers are long-lived while their individual
            // generations are not. Do not let a half-open loopback response
            // become the transport inherited by a later model turn.
            client_builder = client_builder.pool_max_idle_per_host(0);
        }
        if observe_local_provider {
            // Environment proxy settings must not turn a syntactically local provider request
            // into public-web traffic carrying trusted local timing provenance.
            client_builder = client_builder.no_proxy();
        }
        if let Some(broker) = provider_broker {
            client_builder = client_builder.unix_socket(broker.socket_path.clone());
        }
        let client = client_builder
            .build()
            .map_err(|e| format!("failed to build http client: {e}"))?;

        let method = parse_method(&request.method)?;
        let body = request.body.unwrap_or_default();
        let (request_url, headers) = if provider_broker.is_some() {
            if body.is_empty() {
                return Err("provider broker request body is absent".to_string());
            }
            (
                format!("http://{PROVIDER_BROKER_AUTHORITY}{LOCAL_PROVIDER_COMPLETIONS_PATH}"),
                provider_broker_request_headers(
                    provider_broker.ok_or_else(|| {
                        "provider broker disappeared during exact request".to_string()
                    })?,
                    body.as_bytes(),
                )?,
            )
        } else {
            let mut headers = build_headers(&request.headers)?;
            if allow_local_origin {
                headers.insert(CONNECTION, HeaderValue::from_static("close"));
            }
            (request.url, headers)
        };

        let mut request_builder = client.request(method, request_url).headers(headers);
        if !body.is_empty() {
            request_builder = request_builder.body(body);
        }

        // This clock intentionally starts before the host semaphore acquisition below. The exact
        // value therefore includes host queueing, connection setup, request upload, and provider
        // wait to successful response headers. It is not generation latency or first-token time.
        // The wait must be both bounded and cancellable: otherwise one provider request can pin
        // the capsule run loop after its caller has already timed out.
        let provider_attempt = begin_local_provider_request(self, provider_context.as_ref());
        let cancel_token = self.cancel_token.clone();
        let started_at = Instant::now();
        let result = util::bounded_block_on_cancellable(
            &runtime_handle,
            &host_semaphore,
            &cancel_token,
            send_stream_request(request_builder, response_header_timeout(allow_local_origin)),
        );
        let response = match result {
            Some(Ok(response)) => response,
            Some(Err(error)) => {
                finish_local_provider_request(
                    self,
                    provider_context.as_ref(),
                    provider_attempt,
                    error.outcome(),
                    None,
                );
                return Err(error.to_string());
            },
            None => {
                finish_local_provider_request(
                    self,
                    provider_context.as_ref(),
                    provider_attempt,
                    LocalProviderRequestOutcomeV1::Cancelled,
                    None,
                );
                return Err("HTTP stream request cancelled during capsule shutdown".to_string());
            },
        };
        let header_elapsed = started_at.elapsed();
        tracing::info!(
            capsule_id = %capsule_id,
            origin = %parsed_url.origin().ascii_serialization(),
            elapsed_ms = header_elapsed.as_millis(),
            "HTTP stream response headers received"
        );
        let provider_outcome = local_provider_response_outcome(
            response.status(),
            response.remote_addr(),
            provider_broker.is_some(),
        );
        finish_local_provider_request(
            self,
            provider_context.as_ref(),
            provider_attempt,
            provider_outcome,
            Some(header_elapsed),
        );

        let status = response.status().as_u16();

        let mut resp_headers = Vec::new();
        for (k, v) in response.headers() {
            if let Ok(v_str) = v.to_str() {
                resp_headers.push(KeyValuePair {
                    key: k.as_str().to_string(),
                    value: v_str.to_string(),
                });
            }
        }

        // Store the response body stream and allocate a handle.
        let handle_id = self.next_http_stream_id;
        self.next_http_stream_id = self
            .next_http_stream_id
            .checked_add(1)
            .ok_or_else(|| "HTTP stream handle ID space exhausted".to_string())?;

        debug_assert!(
            !self.active_http_streams.contains_key(&handle_id),
            "HTTP stream handle ID collision"
        );
        self.active_http_streams
            .insert(handle_id, Arc::new(tokio::sync::Mutex::new(response)));

        Ok(HttpStreamStartResponse {
            handle: handle_id,
            status,
            headers: resp_headers,
        })
    }

    fn http_stream_read(&mut self, stream_handle: u64) -> Result<Vec<u8>, String> {
        let response_arc = self
            .active_http_streams
            .get(&stream_handle)
            .ok_or_else(|| "HTTP stream handle not found".to_string())?
            .clone();

        let rt_handle = self.runtime_handle.clone();
        let cancel_token = self.cancel_token.clone();
        let host_semaphore = self.host_semaphore.clone();

        let result =
            util::bounded_block_on_cancellable(&rt_handle, &host_semaphore, &cancel_token, async {
                let mut resp = response_arc.lock().await;
                tokio::time::timeout(HTTP_STREAM_READ_TIMEOUT, resp.chunk()).await
            });

        let chunk_data = match result {
            // Cancelled (capsule unloading).
            None => Vec::new(),
            // Timeout waiting for next chunk.
            Some(Err(_elapsed)) => {
                return Err(format!(
                    "HTTP stream read timed out after {}s",
                    HTTP_STREAM_READ_TIMEOUT.as_secs()
                ));
            },
            // Network/body error.
            Some(Ok(Err(e))) => {
                return Err(format!("HTTP stream read error: {e}"));
            },
            // Got a chunk.
            Some(Ok(Ok(Some(bytes)))) => bytes.to_vec(),
            // EOF — stream exhausted.
            Some(Ok(Ok(None))) => Vec::new(),
        };

        Ok(chunk_data)
    }

    fn http_stream_close(&mut self, stream_handle: u64) -> Result<(), String> {
        // Idempotent: silently ignore if the handle was already removed.
        let _ = self.active_http_streams.remove(&stream_handle);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use astrid_events::ipc::IpcProducerV1;
    use std::net::IpAddr;
    use std::str::FromStr;
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    #[test]
    fn local_origin_allowlist_requires_exact_capsule_host_and_port() {
        let allowlist = vec!["astrid-capsule-openai-compat@127.0.0.1:11434".to_string()];
        assert!(origin_allowed_by(
            "astrid-capsule-openai-compat",
            &reqwest::Url::parse("http://127.0.0.1:11434/v1/chat/completions").unwrap(),
            &allowlist
        ));
        assert!(!origin_allowed_by(
            "astrid-capsule-http",
            &reqwest::Url::parse("http://127.0.0.1:11434/api/tags").unwrap(),
            &allowlist
        ));
        assert!(!origin_allowed_by(
            "astrid-capsule-openai-compat",
            &reqwest::Url::parse("http://127.0.0.1:8080/admin").unwrap(),
            &allowlist
        ));
        assert!(!origin_allowed_by(
            "astrid-capsule-openai-compat",
            &reqwest::Url::parse("http://192.168.2.1:11434/").unwrap(),
            &allowlist
        ));
    }

    #[test]
    fn provider_attempt_requires_exact_allowlist_and_loopback_origin() {
        let capsule = "astrid-capsule-openai-compat";
        let allowlist = vec![
            format!("{capsule}@127.0.0.1:11434"),
            format!("{capsule}@localhost:11435"),
            format!("{capsule}@example.com:443"),
            format!("{capsule}@192.168.1.8:11434"),
            "other-capsule@127.0.0.1:11434".to_string(),
        ];
        assert!(local_provider_attempt_allowed(
            capsule,
            &reqwest::Url::parse("http://127.0.0.1:11434/v1/chat/completions").unwrap(),
            "POST",
            &allowlist,
        ));
        assert!(!local_provider_attempt_allowed(
            capsule,
            &reqwest::Url::parse("http://localhost:11435/v1/chat/completions").unwrap(),
            "post",
            &allowlist,
        ));
        assert!(!local_provider_attempt_allowed(
            capsule,
            &reqwest::Url::parse("https://example.com/v1/chat/completions").unwrap(),
            "POST",
            &allowlist,
        ));
        assert!(!local_provider_attempt_allowed(
            capsule,
            &reqwest::Url::parse("http://192.168.1.8:11434/v1/chat/completions").unwrap(),
            "POST",
            &allowlist,
        ));
        assert!(!local_provider_attempt_allowed(
            "other-capsule",
            &reqwest::Url::parse("http://127.0.0.1:11434/v1/chat/completions").unwrap(),
            "POST",
            &allowlist,
        ));
        assert!(!local_provider_attempt_allowed(
            capsule,
            &reqwest::Url::parse("http://127.0.0.1:11434/v1/chat/completions").unwrap(),
            "GET",
            &allowlist,
        ));
        assert!(!local_provider_attempt_allowed(
            capsule,
            &reqwest::Url::parse("http://127.0.0.1:11434/api/tags").unwrap(),
            "POST",
            &allowlist,
        ));
        assert!(!local_provider_attempt_allowed(
            capsule,
            &reqwest::Url::parse("http://127.0.0.1:11434/v1/chat/completions?secret=1").unwrap(),
            "POST",
            &allowlist,
        ));
    }

    #[test]
    fn provider_attempt_terminal_status_covers_every_response_header_case() {
        let loopback = "127.0.0.1:11434".parse().unwrap();
        let public = "198.51.100.8:443".parse().unwrap();
        assert_eq!(
            local_provider_response_outcome(reqwest::StatusCode::OK, Some(loopback), false),
            LocalProviderRequestOutcomeV1::SuccessfulHeaders
        );
        assert_eq!(
            local_provider_response_outcome(
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                Some(loopback),
                false
            ),
            LocalProviderRequestOutcomeV1::NonSuccessStatus
        );
        assert_eq!(
            local_provider_response_outcome(reqwest::StatusCode::OK, Some(public), false),
            LocalProviderRequestOutcomeV1::NonLoopbackPeer
        );
        assert_eq!(
            local_provider_response_outcome(reqwest::StatusCode::OK, None, false),
            LocalProviderRequestOutcomeV1::UnknownPeer
        );
        assert_eq!(
            local_provider_response_outcome(reqwest::StatusCode::OK, None, true),
            LocalProviderRequestOutcomeV1::SuccessfulHeaders
        );
    }

    #[test]
    fn provider_broker_origin_is_exact_and_never_grants_dns_or_admin_routes() {
        assert!(provider_broker_origin_allowed(
            &reqwest::Url::parse("http://127.0.0.1:11434/v1/chat/completions").unwrap(),
            true,
        ));
        for value in [
            "http://localhost:11434/v1/chat/completions",
            "http://127.0.0.53:53/v1/chat/completions",
            "http://127.0.0.1:11434/api/delete",
            "http://127.0.0.1:11434/api/pull",
            "http://127.0.0.1:11434/v1/chat/completions?admin=1",
        ] {
            assert!(!provider_broker_origin_allowed(
                &reqwest::Url::parse(value).unwrap(),
                true,
            ));
        }
    }

    fn traced_llm_request(trace: IpcTraceContextV1, request_id: uuid::Uuid) -> IpcMessage {
        IpcMessage::new(
            LOCAL_PROVIDER_LLM_REQUEST_TOPIC,
            IpcPayload::LlmRequest {
                request_id,
                model: "local".to_string(),
                messages: Vec::new(),
                tools: Vec::new(),
                system: String::new(),
            },
            uuid::Uuid::new_v4(),
        )
        .with_trace(trace)
        .with_principal("caller-principal")
        .with_producer(IpcProducerV1::new("wasm_capsule", REACT_CAPSULE_ID))
    }

    #[test]
    fn provider_context_binds_exact_direct_llm_request_trace() {
        let trace = IpcTraceContextV1::root(
            uuid::Uuid::new_v4(),
            "edge-session",
            Some("chain-a".to_string()),
        );
        let request_id = uuid::Uuid::new_v4();
        let caller = traced_llm_request(trace.clone(), request_id);
        let context = current_llm_request_context(Some(&caller)).unwrap();
        assert_eq!(context.trace, trace);
        assert_eq!(context.request_id, request_id);
    }

    #[test]
    fn provider_attempt_rejects_untraced_or_unrelated_callers() {
        let request_id = uuid::Uuid::new_v4();
        let mut unsupported = IpcTraceContextV1::root(uuid::Uuid::new_v4(), "session", None);
        unsupported.schema_version = 99;
        let unsupported = traced_llm_request(unsupported, request_id);
        assert!(
            current_llm_request_context(Some(&unsupported)).is_none(),
            "unsupported traces must not become provider contexts"
        );

        let mut missing_turn = IpcTraceContextV1::root(uuid::Uuid::new_v4(), "session", None);
        missing_turn.turn_id = None;
        let missing_turn = traced_llm_request(missing_turn, request_id);
        assert!(current_llm_request_context(Some(&missing_turn)).is_none());

        let mut missing_session = IpcTraceContextV1::root(uuid::Uuid::new_v4(), "session", None);
        missing_session.session_id = None;
        let missing_session = traced_llm_request(missing_session, request_id);
        assert!(current_llm_request_context(Some(&missing_session)).is_none());

        let valid = traced_llm_request(
            IpcTraceContextV1::root(uuid::Uuid::new_v4(), "session", None),
            request_id,
        );
        assert!(current_llm_request_context(None).is_none());

        let mut wrong_topic = valid.clone();
        wrong_topic.topic = "llm.v1.request.generate.other".to_string();
        assert!(current_llm_request_context(Some(&wrong_topic)).is_none());

        let mut wrong_producer = valid.clone();
        wrong_producer.producer = Some(IpcProducerV1::new("wasm_capsule", "other-capsule"));
        assert!(current_llm_request_context(Some(&wrong_producer)).is_none());

        let mut native_spoof = valid.clone();
        native_spoof.producer = Some(IpcProducerV1::new("native_socket_client", REACT_CAPSULE_ID));
        assert!(current_llm_request_context(Some(&native_spoof)).is_none());

        let unrelated =
            IpcMessage::new("tool.v1.result", IpcPayload::Connect, uuid::Uuid::new_v4());
        assert!(
            current_llm_request_context(Some(&unrelated)).is_none(),
            "an unrelated direct caller must not fall back to remembered run-loop context"
        );

        // A valid message present only in run-loop state is deliberately ineligible for the
        // current pooled provider contract.
        assert!(current_llm_request_context(None).is_none());
        assert!(current_llm_request_context(Some(&valid)).is_some());
    }

    #[test]
    fn direct_private_ip_requires_allowlisted_origin() {
        let url = reqwest::Url::parse("http://127.0.0.1:11434/").unwrap();
        assert!(validate_direct_ip(&url, false).is_err());
        assert!(validate_direct_ip(&url, true).is_ok());
    }

    #[test]
    fn local_header_deadline_is_bounded_and_public_deadline_is_fixed() {
        assert_eq!(local_header_timeout_seconds(None), 300);
        assert_eq!(local_header_timeout_seconds(Some("420")), 420);
        assert_eq!(local_header_timeout_seconds(Some("59")), 300);
        assert_eq!(local_header_timeout_seconds(Some("421")), 300);
        assert_eq!(local_header_timeout_seconds(Some("600")), 300);
        assert_eq!(local_header_timeout_seconds(Some("601")), 300);
        assert_eq!(local_header_timeout_seconds(Some("not-a-number")), 300);
        assert_eq!(response_header_timeout(false), Duration::from_secs(300));
    }

    #[tokio::test]
    async fn stalled_response_headers_time_out_without_poisoning_next_request() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stalled, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 1_024];
            let _ = stalled.read(&mut request).await.unwrap();

            let (mut healthy, _) = listener.accept().await.unwrap();
            let _ = healthy.read(&mut request).await.unwrap();
            healthy
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .await
                .unwrap();
        });
        let url = format!("http://{address}/stream");

        let stalled_client = reqwest::Client::builder()
            .pool_max_idle_per_host(0)
            .build()
            .unwrap();
        let error = send_stream_request(
            stalled_client.get(&url).header(CONNECTION, "close"),
            Duration::from_millis(50),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("response headers timed out"));

        let healthy_client = reqwest::Client::builder()
            .pool_max_idle_per_host(0)
            .build()
            .unwrap();
        let response = send_stream_request(
            healthy_client.get(&url).header(CONNECTION, "close"),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(response.text().await.unwrap(), "ok");
        server.await.unwrap();
    }

    #[test]
    fn safe_public_ips() {
        assert!(is_safe_ip(IpAddr::from_str("8.8.8.8").unwrap()));
        assert!(is_safe_ip(IpAddr::from_str("1.1.1.1").unwrap()));
        assert!(is_safe_ip(IpAddr::from_str("198.51.100.1").unwrap()));
        assert!(is_safe_ip(
            IpAddr::from_str("2001:4860:4860::8888").unwrap()
        ));
    }

    #[test]
    fn blocks_loopback_and_unspecified() {
        assert!(!is_safe_ip(IpAddr::from_str("127.0.0.1").unwrap()));
        assert!(!is_safe_ip(IpAddr::from_str("::1").unwrap()));
        assert!(!is_safe_ip(IpAddr::from_str("0.0.0.0").unwrap()));
        assert!(!is_safe_ip(IpAddr::from_str("::").unwrap()));
    }

    #[test]
    fn blocks_zero_block() {
        assert!(!is_safe_ip(IpAddr::from_str("0.0.0.1").unwrap()));
        assert!(!is_safe_ip(IpAddr::from_str("0.255.255.255").unwrap()));
    }

    #[test]
    fn blocks_rfc1918_private() {
        assert!(!is_safe_ip(IpAddr::from_str("10.0.0.1").unwrap()));
        assert!(!is_safe_ip(IpAddr::from_str("10.255.255.255").unwrap()));
        assert!(!is_safe_ip(IpAddr::from_str("172.16.0.1").unwrap()));
        assert!(!is_safe_ip(IpAddr::from_str("172.31.255.255").unwrap()));
        assert!(!is_safe_ip(IpAddr::from_str("192.168.0.1").unwrap()));
        assert!(!is_safe_ip(IpAddr::from_str("192.168.255.255").unwrap()));
    }

    #[test]
    fn blocks_link_local_and_cgnat() {
        assert!(!is_safe_ip(IpAddr::from_str("169.254.169.254").unwrap()));
        assert!(!is_safe_ip(IpAddr::from_str("100.64.0.1").unwrap()));
        assert!(!is_safe_ip(IpAddr::from_str("100.127.255.255").unwrap()));
    }

    #[test]
    fn blocks_private_ipv6() {
        assert!(!is_safe_ip(IpAddr::from_str("fc00::1").unwrap()));
        assert!(!is_safe_ip(IpAddr::from_str("fd00::1").unwrap()));
        assert!(!is_safe_ip(IpAddr::from_str("fe80::1").unwrap()));
    }

    #[test]
    fn blocks_ipv4_mapped_ipv6_bypass() {
        assert!(!is_safe_ip(IpAddr::from_str("::ffff:127.0.0.1").unwrap()));
        assert!(!is_safe_ip(IpAddr::from_str("::ffff:10.0.0.1").unwrap()));
        assert!(!is_safe_ip(
            IpAddr::from_str("::ffff:169.254.169.254").unwrap()
        ));
    }

    #[test]
    fn blocks_ipv4_compatible_ipv6_bypass() {
        // IPv4-compatible (deprecated RFC 4291, no ::ffff prefix).
        // These exercise the explicit segment extraction that replaced
        // the deprecated Ipv6Addr::to_ipv4().
        assert!(!is_safe_ip(IpAddr::from_str("::127.0.0.1").unwrap()));
        assert!(!is_safe_ip(IpAddr::from_str("::10.0.0.1").unwrap()));
        assert!(!is_safe_ip(IpAddr::from_str("::169.254.169.254").unwrap()));
        // ::1 is IPv6 loopback; after compatible-branch extraction it
        // becomes 0.0.0.1, blocked by the 0.0.0.0/8 check (not loopback).
        assert!(!is_safe_ip(IpAddr::from_str("::0.0.0.1").unwrap()));
    }
}
