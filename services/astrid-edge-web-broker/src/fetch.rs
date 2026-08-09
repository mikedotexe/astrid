//! Immutable bounded public-source retrieval for CPU-edge `READ_SOURCE`.

use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::{
    CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, HeaderMap, TRANSFER_ENCODING,
};
use reqwest::{Response, StatusCode, Version};
use serde::{Deserialize, Serialize};

use crate::html::extract_readable_text;
use crate::search::BraveSearch;
use crate::{Error, Result, is_public_upstream_ip};

pub const FETCH_PATH: &str = "/v1/fetch";
pub const FETCH_REQUEST_SCHEMA: &str = "astrid.edge.web_fetch.request.v2";
pub const FETCH_RESPONSE_SCHEMA: &str = "astrid.edge.web_fetch.response.v1";
const MAXIMUM_FETCH_CHARS: usize = 64 * 1024;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FetchRequest {
    pub schema: String,
    pub trace_id: String,
    pub url: String,
    pub max_chars: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FetchResponse {
    pub schema: String,
    pub url: String,
    pub status: u16,
    pub original_body_bytes: u64,
    pub truncated: bool,
    pub body: String,
}

impl FetchRequest {
    /// Validate a bounded HTTPS public-source request before DNS or I/O.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, private, credentialed, fragmented,
    /// unsupported, or over-limit requests.
    pub fn validate(&self) -> Result<reqwest::Url> {
        let url =
            reqwest::Url::parse(&self.url).map_err(|_| Error::new("fetch URL is malformed"))?;
        let host = url
            .host_str()
            .ok_or_else(|| Error::new("fetch URL omitted host"))?;
        let host_is_public = host.parse().map_or_else(
            |_| {
                let normalized = host.to_ascii_lowercase();
                normalized.contains('.')
                    && normalized != "localhost"
                    && !normalized.ends_with(".localhost")
                    && !normalized
                        .rsplit_once('.')
                        .is_some_and(|(_, suffix)| suffix.eq_ignore_ascii_case("local"))
                    && normalized
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
            },
            is_public_upstream_ip,
        );
        let path_lower = url.path().to_ascii_lowercase();
        if self.schema != FETCH_REQUEST_SCHEMA
            || !crate::search::trace_id_is_canonical(&self.trace_id)
            || self.url.trim() != self.url
            || self.url.chars().count() > 2_048
            || self.url.chars().any(char::is_control)
            || url.scheme() != "https"
            || url.port_or_known_default() != Some(443)
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || !host_is_public
            || std::path::Path::new(&path_lower)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
            || !(256..=u32::try_from(MAXIMUM_FETCH_CHARS).unwrap_or(u32::MAX))
                .contains(&self.max_chars)
        {
            return Err(Error::new("fetch request is outside immutable bounds"));
        }
        Ok(url)
    }
}

impl FetchResponse {
    /// Revalidate the complete response before returning it to a mutable client.
    ///
    /// # Errors
    ///
    /// Returns an error when the response does not bind the request or escapes
    /// the immutable status, text, character, or transport bounds.
    pub fn validate(&self, request: &FetchRequest) -> Result<()> {
        let canonical = request.validate()?.to_string();
        if self.schema != FETCH_RESPONSE_SCHEMA
            || self.url != canonical
            || self.status != 200
            || self.body.chars().count()
                > usize::try_from(request.max_chars).unwrap_or(MAXIMUM_FETCH_CHARS)
            || self
                .body
                .chars()
                .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
            || self.original_body_bytes == 0
            || self.original_body_bytes > u64::from(u32::MAX)
        {
            return Err(Error::new("fetch response escaped immutable bounds"));
        }
        Ok(())
    }
}

pub trait FetchBackend: Send + Sync + 'static {
    /// Fetch one validated, readable public source without redirects.
    ///
    /// # Errors
    ///
    /// Returns a bounded error when validation, DNS, transport, extraction, or
    /// final response validation fails.
    fn fetch(&self, request: &FetchRequest) -> Result<FetchResponse>;
}

impl FetchBackend for BraveSearch {
    fn fetch(&self, request: &FetchRequest) -> Result<FetchResponse> {
        let url = request.validate()?;
        self.authorize_fetch(&request.trace_id, url.as_str())?;
        let client = self.client().clone();
        let config = self.config();
        let header_timeout = Duration::from_millis(config.header_timeout_ms);
        let total_timeout = Duration::from_millis(config.total_timeout_ms);
        let maximum = usize::try_from(config.maximum_upstream_body_bytes)
            .map_err(|_| Error::new("upstream body bound is not representable"))?;
        let (body, content_type) = self.runtime().block_on(async move {
            let started = tokio::time::Instant::now();
            let response = tokio::time::timeout(header_timeout, client.get(url.clone()).send())
                .await
                .map_err(|_| Error::new("upstream response-header deadline exceeded"))??;
            validate_fetch_origin(&response, &url)?;
            let content_type = validate_fetch_headers(
                response.headers(),
                response.version(),
                config.maximum_upstream_body_bytes,
            )?;
            let deadline = started
                .checked_add(total_timeout)
                .ok_or_else(|| Error::new("upstream total deadline overflow"))?;
            let body = tokio::time::timeout_at(deadline, read_bounded(response, maximum))
                .await
                .map_err(|_| Error::new("upstream total deadline exceeded"))??;
            Ok::<_, Error>((body, content_type))
        })?;
        let original_body_bytes = u64::try_from(body.len()).unwrap_or(u64::MAX);
        let maximum_chars = usize::try_from(request.max_chars).unwrap_or(MAXIMUM_FETCH_CHARS);
        let (text, truncated) =
            if content_type == "text/html" || content_type == "application/xhtml+xml" {
                extract_readable_text(&String::from_utf8_lossy(&body), maximum_chars)
            } else {
                bounded_utf8_text(&body, maximum_chars)
            };
        let response = FetchResponse {
            schema: FETCH_RESPONSE_SCHEMA.to_owned(),
            url: request.validate()?.to_string(),
            status: 200,
            original_body_bytes,
            truncated,
            body: text,
        };
        response.validate(request)?;
        Ok(response)
    }
}

fn validate_fetch_origin(response: &Response, expected: &reqwest::Url) -> Result<()> {
    if response.status() != StatusCode::OK
        || response.url().scheme() != expected.scheme()
        || response.url().host_str() != expected.host_str()
        || response.url().port_or_known_default() != expected.port_or_known_default()
        || response.url().path() != expected.path()
        || response.url().query() != expected.query()
        || response.url().fragment().is_some()
    {
        return Err(Error::new(
            "fetch redirect, status, or origin failed immutable policy",
        ));
    }
    Ok(())
}

fn validate_fetch_headers(
    headers: &HeaderMap,
    version: Version,
    maximum_body: u32,
) -> Result<String> {
    let content_types = headers.get_all(CONTENT_TYPE).iter().collect::<Vec<_>>();
    if content_types.len() != 1 {
        return Err(Error::new("fetch content type is missing or ambiguous"));
    }
    let content_type = content_types[0]
        .to_str()
        .map_err(|_| Error::new("fetch content type is invalid"))?
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if !matches!(
        content_type.as_str(),
        "text/html" | "application/xhtml+xml" | "text/plain" | "application/json"
    ) {
        return Err(Error::new("fetch content type is not readable text"));
    }
    if headers.get(CONTENT_ENCODING).is_some() {
        return Err(Error::new("fetch encoded bodies are rejected"));
    }
    let lengths = headers.get_all(CONTENT_LENGTH).iter().collect::<Vec<_>>();
    let transfers = headers
        .get_all(TRANSFER_ENCODING)
        .iter()
        .collect::<Vec<_>>();
    if lengths.len() > 1 || transfers.len() > 1 || (!lengths.is_empty() && !transfers.is_empty()) {
        return Err(Error::new("fetch response framing is ambiguous"));
    }
    if let Some(length) = lengths.first() {
        let length = length
            .to_str()
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or_else(|| Error::new("fetch Content-Length is invalid"))?;
        if length == 0 || length > u64::from(maximum_body) {
            return Err(Error::new("fetch response exceeds body bound"));
        }
    } else if version == Version::HTTP_11
        && !transfers
            .first()
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.eq_ignore_ascii_case("chunked"))
    {
        return Err(Error::new("fetch HTTP/1.1 response omitted framing"));
    }
    Ok(content_type)
}

async fn read_bounded(response: Response, maximum: usize) -> Result<Vec<u8>> {
    let mut body = Vec::with_capacity(maximum.min(64 * 1024));
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let prospective = body
            .len()
            .checked_add(chunk.len())
            .ok_or_else(|| Error::new("fetch response size overflowed"))?;
        if prospective > maximum {
            return Err(Error::new("fetch response exceeded body bound"));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn bounded_utf8_text(bytes: &[u8], maximum_chars: usize) -> (String, bool) {
    let text = String::from_utf8_lossy(bytes);
    let original = text.chars().count();
    let bounded = text
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .take(maximum_chars)
        .collect::<String>();
    (bounded, original > maximum_chars)
}

#[cfg(test)]
mod tests {
    use reqwest::Version;
    use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE, HeaderMap, HeaderValue};

    use super::{FetchRequest, bounded_utf8_text, validate_fetch_headers};

    #[test]
    fn fetch_rejects_ssrf_plaintext_credentials_fragments_and_pdfs() {
        for url in [
            "http://example.com/x",
            "https://127.0.0.1/x",
            "https://user:pass@example.com/x",
            "https://example.com/x#fragment",
            "https://example.com/paper.PDF",
        ] {
            assert!(
                FetchRequest {
                    schema: super::FETCH_REQUEST_SCHEMA.to_owned(),
                    trace_id: "11111111-1111-4111-8111-111111111111".to_owned(),
                    url: url.to_owned(),
                    max_chars: 8_000,
                }
                .validate()
                .is_err()
            );
        }
    }

    #[test]
    fn fetch_header_policy_rejects_binary_and_oversize() {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/pdf"));
        headers.insert(CONTENT_LENGTH, HeaderValue::from_static("10"));
        assert!(validate_fetch_headers(&headers, Version::HTTP_11, 20).is_err());
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/html"));
        headers.insert(CONTENT_LENGTH, HeaderValue::from_static("21"));
        assert!(validate_fetch_headers(&headers, Version::HTTP_11, 20).is_err());
    }

    #[test]
    fn text_output_is_bounded_and_control_filtered() {
        assert_eq!(bounded_utf8_text(b"ab\0cd", 3), ("abc".to_owned(), true));
    }
}
