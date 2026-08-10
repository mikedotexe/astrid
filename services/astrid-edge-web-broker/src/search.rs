use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use reqwest::header::{
    ACCEPT, ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, HeaderMap,
    HeaderValue, TRANSFER_ENCODING, USER_AGENT,
};
use reqwest::{Client, Response, StatusCode, Version};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tokio::runtime::Runtime;

use crate::config::{Config, REQUEST_SCHEMA, RESPONSE_SCHEMA};
use crate::html::parse_brave_results;
use crate::is_public_upstream_ip;
use crate::security::PublicDnsResolver;
use crate::{Error, Result};

const USER_AGENT_VALUE: &str = "Astrid-Edge-Immutable-Web-Broker/1.0 (read-only search)";
const FETCH_GRANT_LIFETIME: Duration = Duration::from_secs(30 * 60);
const MAXIMUM_FETCH_GRANTS: usize = 128;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchRequest {
    pub schema: String,
    pub trace_id: String,
    pub query: String,
    pub limit: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SearchResponse {
    pub schema: String,
    pub results: Vec<SearchResult>,
}

impl SearchRequest {
    /// Validate the exact public broker request envelope.
    ///
    /// # Errors
    ///
    /// Returns an error if the schema, query, or result bound is invalid.
    pub fn validate(&self, config: &Config) -> Result<()> {
        let query = self.query.trim();
        if self.schema != REQUEST_SCHEMA
            || !trace_id_is_canonical(&self.trace_id)
            || query.is_empty()
            || query != self.query
            || query.chars().count() > 160
            || query.chars().any(char::is_control)
            || !query_is_public_research(query)
            || !(1..=config.maximum_results).contains(&self.limit)
        {
            return Err(Error::new("search request is outside immutable bounds"));
        }
        Ok(())
    }
}

impl SearchResponse {
    /// Validate the final immutable response boundary before bytes are emitted
    /// onto the loopback socket.
    ///
    /// # Errors
    ///
    /// Returns an error if a backend produces excess results, control text,
    /// oversized fields, an unsafe URL, an unknown schema, or an oversized
    /// serialized response.
    pub fn validate(&self, config: &Config, requested_limit: u8) -> Result<()> {
        if self.schema != RESPONSE_SCHEMA
            || self.results.len() > usize::from(requested_limit)
            || self.results.len() > usize::from(config.maximum_results)
        {
            return Err(Error::new(
                "upstream result count or schema escaped immutable bound",
            ));
        }
        for result in &self.results {
            if result.title.is_empty()
                || result.title.chars().count() > 200
                || result.url.chars().count() > 2_048
                || result.snippet.chars().count() > 500
                || result.title.chars().any(char::is_control)
                || result.snippet.chars().any(char::is_control)
                || !result_url_is_safe(&result.url)
            {
                return Err(Error::new(
                    "upstream result metadata escaped immutable bound",
                ));
            }
        }
        if serde_json::to_vec(self)?.len() > 64 * 1024 {
            return Err(Error::new(
                "upstream result envelope exceeds immutable bound",
            ));
        }
        Ok(())
    }
}

pub(crate) fn result_url_is_safe(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some_and(safe_result_host)
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && value.chars().count() <= 2_048
        && !value.chars().any(char::is_control)
}

fn safe_result_host(host: &str) -> bool {
    let normalized = host.to_ascii_lowercase();
    if let Ok(ip) = normalized.parse() {
        return is_public_upstream_ip(ip);
    }
    normalized.contains('.')
        && normalized != "localhost"
        && !has_dns_suffix(&normalized, "localhost")
        && !has_dns_suffix(&normalized, "local")
        && normalized
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
}

fn has_dns_suffix(host: &str, suffix: &str) -> bool {
    host.rsplit_once('.')
        .is_some_and(|(_, final_label)| final_label == suffix)
}

pub trait SearchBackend: Send + Sync + 'static {
    /// Search public metadata using a validated request.
    ///
    /// # Errors
    ///
    /// Returns a bounded, non-sensitive failure if the upstream is unavailable
    /// or violates transport policy.
    fn search(&self, request: &SearchRequest) -> Result<SearchResponse>;
}

pub struct BraveSearch {
    config: Config,
    client: Client,
    runtime: Runtime,
    fetch_grants: Mutex<FetchGrantStore>,
}

#[derive(Debug)]
struct FetchGrant {
    client_id: String,
    trace_id: String,
    query_sha256: String,
    result_index: usize,
    canonical_url: String,
    expires_at: Instant,
}

#[derive(Debug, Default)]
struct FetchGrantStore {
    grants: VecDeque<FetchGrant>,
}

impl BraveSearch {
    /// Build the fixed-origin HTTPS client with no environment proxy, no
    /// redirects, a safe resolver, and no connection reuse across searches.
    ///
    /// # Errors
    ///
    /// Returns an error if the immutable client policy cannot be constructed.
    pub fn new(config: Config) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("text/html,application/xhtml+xml"),
        );
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        let client = Client::builder()
            .https_only(true)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_millis(config.connect_timeout_ms))
            .pool_max_idle_per_host(0)
            .http1_only()
            .default_headers(headers)
            .dns_resolver(Arc::new(PublicDnsResolver))
            .build()?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .thread_name("astrid-edge-web-io")
            .build()
            .map_err(|error| {
                Error::new(format!("could not build immutable I/O runtime: {error}"))
            })?;
        Ok(Self {
            config,
            client,
            runtime,
            fetch_grants: Mutex::new(FetchGrantStore::default()),
        })
    }

    pub(crate) const fn config(&self) -> &Config {
        &self.config
    }

    pub(crate) const fn client(&self) -> &Client {
        &self.client
    }

    pub(crate) const fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    pub(crate) fn authorize_fetch(&self, trace_id: &str, canonical_url: &str) -> Result<()> {
        self.fetch_grants
            .lock()
            .map_err(|_| Error::new("ephemeral fetch grant store is unavailable"))?
            .consume(
                &self.config.client_id,
                trace_id,
                canonical_url,
                Instant::now(),
            )
    }

    fn remember_fetch_results(
        &self,
        request: &SearchRequest,
        results: &[SearchResult],
    ) -> Result<()> {
        let mut grants = self
            .fetch_grants
            .lock()
            .map_err(|_| Error::new("ephemeral fetch grant store is unavailable"))?;
        let now = Instant::now();
        let query_sha256 = format!("{:x}", Sha256::digest(request.query.as_bytes()));
        for (result_index, result) in results.iter().enumerate() {
            let Ok(url) = reqwest::Url::parse(&result.url) else {
                continue;
            };
            if url.scheme() == "https" {
                grants.insert(
                    self.config.client_id.clone(),
                    request.trace_id.clone(),
                    query_sha256.clone(),
                    result_index,
                    url.to_string(),
                    now,
                );
            }
        }
        Ok(())
    }
}

impl FetchGrantStore {
    fn prune(&mut self, now: Instant) {
        self.grants.retain(|grant| grant.expires_at > now);
    }

    fn insert(
        &mut self,
        client_id: String,
        trace_id: String,
        query_sha256: String,
        result_index: usize,
        canonical_url: String,
        now: Instant,
    ) {
        self.prune(now);
        self.grants.retain(|grant| {
            grant.client_id != client_id
                || grant.trace_id != trace_id
                || grant.canonical_url != canonical_url
        });
        self.grants.push_back(FetchGrant {
            client_id,
            trace_id,
            query_sha256,
            result_index,
            canonical_url,
            expires_at: now.checked_add(FETCH_GRANT_LIFETIME).unwrap_or(now),
        });
        while self.grants.len() > MAXIMUM_FETCH_GRANTS {
            let _ = self.grants.pop_front();
        }
    }

    fn consume(
        &mut self,
        client_id: &str,
        trace_id: &str,
        canonical_url: &str,
        now: Instant,
    ) -> Result<()> {
        self.prune(now);
        let index = self
            .grants
            .iter()
            .position(|grant| {
                grant.client_id == client_id
                    && grant.trace_id == trace_id
                    && grant.canonical_url == canonical_url
                    && grant.query_sha256.len() == 64
                    && grant.result_index < 5
            })
            .ok_or_else(|| {
                Error::new(
                    "fetch URL has no fresh client/trace-bound immutable-search result grant",
                )
            })?;
        let _ = self.grants.remove(index);
        Ok(())
    }
}

impl SearchBackend for BraveSearch {
    fn search(&self, request: &SearchRequest) -> Result<SearchResponse> {
        request.validate(&self.config)?;
        let mut url = self.config.upstream_url()?;
        url.query_pairs_mut()
            .append_pair("q", &request.query)
            .append_pair("source", "web");
        let client = self.client.clone();
        let header_timeout = Duration::from_millis(self.config.header_timeout_ms);
        let total_timeout = Duration::from_millis(self.config.total_timeout_ms);
        let maximum = usize::try_from(self.config.maximum_upstream_body_bytes)
            .map_err(|_| Error::new("upstream body bound is not representable"))?;
        let body = self.runtime.block_on(async move {
            let started = tokio::time::Instant::now();
            let response = tokio::time::timeout(header_timeout, client.get(url.clone()).send())
                .await
                .map_err(|_| Error::new("upstream response-header deadline exceeded"))??;
            validate_response_origin(&response, &url)?;
            validate_upstream_headers(&response, u32::try_from(maximum).unwrap_or(u32::MAX))?;
            let deadline = started
                .checked_add(total_timeout)
                .ok_or_else(|| Error::new("upstream total deadline overflow"))?;
            tokio::time::timeout_at(deadline, read_bounded(response, maximum))
                .await
                .map_err(|_| Error::new("upstream total deadline exceeded"))?
        })?;
        let body = String::from_utf8_lossy(&body);
        let results = parse_brave_results(&body, usize::from(request.limit));
        let response = SearchResponse {
            schema: RESPONSE_SCHEMA.to_string(),
            results,
        };
        response.validate(&self.config, request.limit)?;
        self.remember_fetch_results(request, &response.results)?;
        Ok(response)
    }
}

pub(crate) fn trace_id_is_canonical(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                matches!(byte, b'0'..=b'9' | b'a'..=b'f')
            }
        })
}

fn query_is_public_research(query: &str) -> bool {
    if !query.is_ascii()
        || query.contains("  ")
        || query.contains("://")
        || query.bytes().any(|byte| {
            !byte.is_ascii_alphanumeric()
                && !matches!(
                    byte,
                    b' ' | b'-' | b'+' | b'?' | b'.' | b',' | b'(' | b')' | b'\''
                )
        })
    {
        return false;
    }
    let tokens = query.split_ascii_whitespace().collect::<Vec<_>>();
    if !(2..=24).contains(&tokens.len()) {
        return false;
    }
    tokens.iter().all(|token| token_is_public_research(token))
}

fn token_is_public_research(token: &str) -> bool {
    let token = token.trim_matches(['-', '+', '?', '.', ',', '(', ')', '\'']);
    if token.is_empty() || token.len() > 24 || looks_like_dns_name(token) {
        return false;
    }
    let lowered = token.to_ascii_lowercase();
    if matches!(
        lowered.as_str(),
        "authorization" | "bearer" | "password" | "passwd" | "private-key" | "ssh-rsa"
    ) || lowered.starts_with("akia")
        || lowered.starts_with("ghp-")
        || lowered.starts_with("sk-")
    {
        return false;
    }
    let compact = token
        .bytes()
        .filter(u8::is_ascii_alphanumeric)
        .collect::<Vec<_>>();
    if compact.len() >= 12 && compact.iter().all(u8::is_ascii_hexdigit) {
        return false;
    }
    let digit_count = compact.iter().filter(|byte| byte.is_ascii_digit()).count();
    let upper_count = compact
        .iter()
        .filter(|byte| byte.is_ascii_uppercase())
        .count();
    let lower_count = compact
        .iter()
        .filter(|byte| byte.is_ascii_lowercase())
        .count();
    if compact.len() >= 16 && digit_count > 0 && (upper_count > 0 || lower_count > 0) {
        return false;
    }
    !compact
        .windows(7)
        .any(|window| window.iter().all(u8::is_ascii_digit))
}

fn looks_like_dns_name(token: &str) -> bool {
    let Some((prefix, suffix)) = token.rsplit_once('.') else {
        return false;
    };
    !prefix.is_empty()
        && prefix.bytes().any(|byte| byte.is_ascii_alphabetic())
        && (2..=24).contains(&suffix.len())
        && suffix.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn validate_response_origin(response: &Response, expected: &reqwest::Url) -> Result<()> {
    validate_response_metadata(response.status(), response.url(), expected)
}

fn validate_response_metadata(
    status: StatusCode,
    actual: &reqwest::Url,
    expected: &reqwest::Url,
) -> Result<()> {
    if status != StatusCode::OK
        || actual.scheme() != expected.scheme()
        || actual.host_str() != expected.host_str()
        || actual.port_or_known_default() != expected.port_or_known_default()
        || actual.path() != expected.path()
        || actual.query() != expected.query()
        || actual.fragment().is_some()
    {
        return Err(Error::new(
            "upstream redirect, status, or origin failed immutable policy",
        ));
    }
    Ok(())
}

fn validate_upstream_headers(response: &Response, maximum_body: u32) -> Result<()> {
    validate_upstream_header_map(response.headers(), response.version(), maximum_body)
}

fn validate_upstream_header_map(
    headers: &HeaderMap,
    version: Version,
    maximum_body: u32,
) -> Result<()> {
    let content_types = headers.get_all(CONTENT_TYPE).iter().collect::<Vec<_>>();
    if content_types.len() != 1 {
        return Err(Error::new("upstream content type is missing or ambiguous"));
    }
    let content_type = content_types[0]
        .to_str()
        .map_err(|_| Error::new("upstream content type is invalid"))?
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if !matches!(content_type.as_str(), "text/html" | "application/xhtml+xml") {
        return Err(Error::new("upstream content type is not allowlisted HTML"));
    }
    if headers.get(CONTENT_ENCODING).is_some() {
        return Err(Error::new("upstream encoded bodies are rejected"));
    }
    let lengths = headers.get_all(CONTENT_LENGTH).iter().collect::<Vec<_>>();
    let transfers = headers
        .get_all(TRANSFER_ENCODING)
        .iter()
        .collect::<Vec<_>>();
    if lengths.len() > 1 || transfers.len() > 1 || (!lengths.is_empty() && !transfers.is_empty()) {
        return Err(Error::new("upstream response framing is ambiguous"));
    }
    if let Some(length) = lengths.first() {
        let length = length
            .to_str()
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or_else(|| Error::new("upstream Content-Length is invalid"))?;
        if length > u64::from(maximum_body) {
            return Err(Error::new("upstream response exceeds body bound"));
        }
    } else if version == Version::HTTP_11 {
        let transfer = transfers
            .first()
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        if !transfer.eq_ignore_ascii_case("chunked") {
            return Err(Error::new("upstream HTTP/1.1 response omitted framing"));
        }
    }
    Ok(())
}

async fn read_bounded(response: Response, maximum: usize) -> Result<Vec<u8>> {
    let mut body = Vec::with_capacity(maximum.min(64 * 1024));
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let prospective = body
            .len()
            .checked_add(chunk.len())
            .ok_or_else(|| Error::new("upstream response size overflowed"))?;
        if prospective > maximum {
            return Err(Error::new(
                "upstream response exceeded body bound while reading",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use reqwest::header::{
        CONTENT_LENGTH, CONTENT_TYPE, HeaderMap, HeaderValue, TRANSFER_ENCODING,
    };
    use reqwest::{StatusCode, Url, Version};

    use super::{
        BraveSearch, FetchGrantStore, SearchBackend, SearchRequest, SearchResponse, SearchResult,
        validate_response_metadata, validate_upstream_header_map,
    };
    use crate::{Config, REQUEST_SCHEMA, RESPONSE_SCHEMA};

    fn config() -> Config {
        Config::from_json(
            br#"{
                "schema":"astrid.edge.web_broker.config.v3",
                "client_id":"edge-runtime",
                "socket_path":"/run/astrid-edge-self-change/web-runtime.sock",
                "expected_peer_uid":1001,
                "socket_gid":1003,
                "upstream_origin":"https://search.brave.com/search",
                "connect_timeout_ms":2000,
                "header_timeout_ms":8000,
                "total_timeout_ms":20000,
                "client_read_timeout_ms":2000,
                "client_write_timeout_ms":2000,
                "maximum_request_body_bytes":4096,
                "maximum_upstream_body_bytes":1048576,
                "maximum_results":5,
                "maximum_concurrent_requests":4,
                "maximum_searches_per_hour":8,
                "maximum_searches_per_utc_day":24,
                "quota_state_path":"/var/lib/astrid-edge-web-runtime/search-quota.jsonl",
                "request_key_path":"/run/credentials/astrid-edge-web-broker-runtime.service/request.key",
                "request_key_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "response_signing_key_path":"/run/credentials/astrid-edge-web-broker-runtime.service/response-signing.key",
                "response_signing_key_sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "response_verify_key_sha256":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn exact_request_rejects_whitespace_controls_and_expansion() {
        let config = config();
        let valid = SearchRequest {
            schema: REQUEST_SCHEMA.to_string(),
            trace_id: "11111111-1111-4111-8111-111111111111".to_string(),
            query: "echo state network".to_string(),
            limit: 5,
        };
        assert!(valid.validate(&config).is_ok());
        for query in ["", " padded", "line\nbreak"] {
            let request = SearchRequest {
                schema: REQUEST_SCHEMA.to_string(),
                trace_id: "11111111-1111-4111-8111-111111111111".to_string(),
                query: query.to_string(),
                limit: 5,
            };
            assert!(request.validate(&config).is_err());
        }
    }

    #[test]
    fn public_search_rejects_exfiltration_shapes_but_keeps_technical_queries() {
        let config = config();
        for query in [
            "echo state network spectral entropy",
            "Rust 1.94 offline Cargo metadata",
            "Qwen3.5 tool use on CPU",
            "How does mode turnover behave?",
        ] {
            assert!(
                SearchRequest {
                    schema: REQUEST_SCHEMA.to_owned(),
                    trace_id: "11111111-1111-4111-8111-111111111111".to_owned(),
                    query: query.to_owned(),
                    limit: 3,
                }
                .validate(&config)
                .is_ok(),
                "rejected technical query: {query}"
            );
        }
        for query in [
            "find https://example.org/private",
            "lookup leak.example.org",
            "read /home/astrid/state",
            "digest 0123456789abcdef0123456789abcdef",
            "trace 550e8400-e29b-41d4-a716-446655440000",
            "credential password=hunter2",
            "token AKIAIOSFODNN7EXAMPLE",
            "payload JBSWY3DPEHPK3PXP",
            "payload eyJhbGciOiJIUzI1NiJ9",
        ] {
            assert!(
                SearchRequest {
                    schema: REQUEST_SCHEMA.to_owned(),
                    trace_id: "11111111-1111-4111-8111-111111111111".to_owned(),
                    query: query.to_owned(),
                    limit: 3,
                }
                .validate(&config)
                .is_err(),
                "accepted exfiltration-shaped query: {query}"
            );
        }
    }

    #[test]
    fn response_contract_has_only_schema_and_bounded_result_list() {
        let response = SearchResponse {
            schema: RESPONSE_SCHEMA.to_string(),
            results: Vec::new(),
        };
        let value = serde_json::to_value(response).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 2);
        assert!(value.get("headers").is_none());
        assert!(value.get("body").is_none());
    }

    #[test]
    fn redirects_and_origin_changes_are_rejected_without_following() {
        let expected = Url::parse("https://search.brave.com/search?q=x&source=web").unwrap();
        let redirected = Url::parse("https://example.org/search?q=x&source=web").unwrap();
        assert!(validate_response_metadata(StatusCode::FOUND, &expected, &expected).is_err());
        assert!(validate_response_metadata(StatusCode::OK, &redirected, &expected).is_err());
        assert!(validate_response_metadata(StatusCode::OK, &expected, &expected).is_ok());
    }

    #[test]
    fn upstream_framing_and_declared_oversize_are_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        headers.insert(CONTENT_LENGTH, HeaderValue::from_static("1048577"));
        assert!(validate_upstream_header_map(&headers, Version::HTTP_11, 1_048_576).is_err());

        headers.insert(CONTENT_LENGTH, HeaderValue::from_static("12"));
        headers.insert(TRANSFER_ENCODING, HeaderValue::from_static("chunked"));
        assert!(validate_upstream_header_map(&headers, Version::HTTP_11, 1_048_576).is_err());

        headers.remove(CONTENT_LENGTH);
        assert!(validate_upstream_header_map(&headers, Version::HTTP_11, 1_048_576).is_ok());

        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        assert!(validate_upstream_header_map(&headers, Version::HTTP_11, 1_048_576).is_err());
    }

    #[test]
    fn final_response_boundary_rejects_backend_escape() {
        let config = config();
        let response = SearchResponse {
            schema: RESPONSE_SCHEMA.to_string(),
            results: vec![SearchResult {
                title: "private target".to_string(),
                url: "http://169.254.169.254/latest/meta-data".to_string(),
                snippet: "not fetched".to_string(),
            }],
        };
        assert!(response.validate(&config, 1).is_err());

        let response = SearchResponse {
            schema: RESPONSE_SCHEMA.to_string(),
            results: vec![SearchResult {
                title: "safe metadata".to_string(),
                url: "https://example.org/paper".to_string(),
                snippet: "bounded".to_string(),
            }],
        };
        assert!(response.validate(&config, 1).is_ok());
    }

    #[test]
    fn source_fetch_grants_are_client_trace_bound_expiring_and_single_use() {
        let now = std::time::Instant::now();
        let mut grants = FetchGrantStore::default();
        let trace = "11111111-1111-4111-8111-111111111111";
        grants.insert(
            "edge-runtime".to_owned(),
            trace.to_owned(),
            "a".repeat(64),
            0,
            "https://example.org/a".to_owned(),
            now,
        );
        assert!(
            grants
                .consume("edge-steward", trace, "https://example.org/a", now)
                .is_err()
        );
        assert!(
            grants
                .consume(
                    "edge-runtime",
                    "22222222-2222-4222-8222-222222222222",
                    "https://example.org/a",
                    now,
                )
                .is_err()
        );
        assert!(
            grants
                .consume("edge-runtime", trace, "https://example.org/b", now)
                .is_err()
        );
        assert!(
            grants
                .consume("edge-runtime", trace, "https://example.org/a", now)
                .is_ok()
        );
        assert!(
            grants
                .consume("edge-runtime", trace, "https://example.org/a", now)
                .is_err()
        );
        grants.insert(
            "edge-runtime".to_owned(),
            trace.to_owned(),
            "a".repeat(64),
            0,
            "https://example.org/a".to_owned(),
            now,
        );
        assert!(
            grants
                .consume(
                    "edge-runtime",
                    trace,
                    "https://example.org/a",
                    now + std::time::Duration::from_secs(30 * 60 + 1),
                )
                .is_err()
        );
    }

    #[test]
    #[ignore = "read-only live public-web harness; run explicitly during appliance acceptance"]
    fn fixed_origin_live_harness() {
        let config = config();
        let backend = BraveSearch::new(config).unwrap();
        let response = backend
            .search(&SearchRequest {
                schema: REQUEST_SCHEMA.to_string(),
                trace_id: "11111111-1111-4111-8111-111111111111".to_string(),
                query: "echo state network spectral entropy".to_string(),
                limit: 2,
            })
            .unwrap();
        assert_eq!(response.schema, RESPONSE_SCHEMA);
        assert!(!response.results.is_empty());
        assert!(response.results.len() <= 2);
    }
}
