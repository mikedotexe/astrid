use astrid_minime_protocol::public_web::PUBLIC_WEB_MAX_BODY_BYTES;
use reqwest::header::HeaderValue;
use serde::{Deserialize, Serialize};
use url::Url;
use zeroize::Zeroizing;

use crate::{BraveSearchConfig, PublicWebErrorCode as E, Result, guard, network::public_url};

pub(crate) const SEARCH_MEDIA: &str = "application/vnd.astrid.search+json";
const ENDPOINT: &str = "https://api.search.brave.com/res/v1/web/search";

pub(crate) fn request_url(query: &str) -> Result<Url> {
    if query.trim().is_empty()
        || query.chars().any(char::is_control)
        || query.chars().count() > 600
        || query.split_whitespace().count() > 75
    {
        return Err(E::InvalidRequest);
    }
    let mut url = public_url(ENDPOINT)?;
    url.query_pairs_mut()
        .append_pair("q", query)
        .append_pair("count", "5")
        .append_pair("country", "US")
        .append_pair("search_lang", "en")
        .append_pair("ui_lang", "en-US")
        .append_pair("safesearch", "moderate")
        .append_pair("spellcheck", "false")
        .append_pair("text_decorations", "false")
        .append_pair("result_filter", "web");
    Ok(url)
}

pub(crate) fn is_endpoint(url: &Url) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some("api.search.brave.com")
        && url.port_or_known_default() == Some(443)
        && url.path() == "/res/v1/web/search"
        && url.username().is_empty()
        && url.password().is_none()
}

pub(crate) fn credential(config: &BraveSearchConfig) -> Result<HeaderValue> {
    let bytes = Zeroizing::new(
        guard::read_private(&config.credential_file).map_err(|_| E::CredentialUnavailable)?,
    );
    let value = bytes.strip_suffix(b"\n").unwrap_or(&bytes);
    if value.is_empty() || value.len() > 4096 || !value.iter().all(u8::is_ascii_graphic) {
        return Err(E::CredentialUnavailable);
    }
    let mut header = HeaderValue::from_bytes(value).map_err(|_| E::CredentialUnavailable)?;
    header.set_sensitive(true);
    Ok(header)
}

#[derive(Deserialize)]
struct ApiResponse {
    #[serde(rename = "type")]
    kind: String,
    query: ApiQuery,
    web: Option<ApiWeb>,
}

#[derive(Deserialize)]
struct ApiQuery {
    original: String,
    more_results_available: Option<bool>,
}

#[derive(Deserialize)]
struct ApiWeb {
    results: Vec<ApiResult>,
}

#[derive(Deserialize)]
struct ApiResult {
    title: String,
    url: String,
    #[serde(default)]
    description: String,
}

#[derive(Serialize)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

#[derive(Serialize)]
struct SearchResponse<'a> {
    schema: &'static str,
    provider: &'static str,
    query: &'a str,
    results: Vec<SearchResult>,
}

pub(crate) fn normalize(body: &[u8], query: &str) -> Result<Vec<u8>> {
    let response: ApiResponse =
        serde_json::from_slice(body).map_err(|_| E::SearchResponseInvalid)?;
    if response.kind != "search"
        || response.query.original != query
        || (response.web.is_none() && response.query.more_results_available == Some(true))
    {
        return Err(E::SearchResponseInvalid);
    }
    let raw = response.web.map_or_else(Vec::new, |web| web.results);
    // No automatic pagination. Results are discovery candidates, not page delivery.
    let mut results = Vec::new();
    for result in raw.into_iter().take(5) {
        if result.title.trim().is_empty()
            || result.title.len() > 16_384
            || result.description.len() > 65_536
        {
            return Err(E::SearchResponseInvalid);
        }
        public_url(&result.url).map_err(|_| E::SearchResponseInvalid)?;
        results.push(SearchResult {
            title: result.title,
            url: result.url,
            snippet: result.description,
        });
    }
    let normalized = serde_json::to_vec(&SearchResponse {
        schema: "astrid.search.results.v1",
        provider: "brave_web_v1",
        query,
        results,
    })
    .map_err(|_| E::SearchResponseInvalid)?;
    if u64::try_from(normalized.len()).unwrap_or(u64::MAX) > PUBLIC_WEB_MAX_BODY_BYTES {
        return Err(E::BodyLimit);
    }
    Ok(normalized)
}
