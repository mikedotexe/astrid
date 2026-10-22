use std::io::{Cursor, Read as _};
use std::time::Duration;

use astrid_minime_protocol::public_web::{
    PublicWebLimitsV2, PublicWebOperationV2, PublicWebOwnerV2, PublicWebPolicyV2,
};
use tokio::time::Instant;
use url::Url;

use crate::authority::Authority;
use crate::guard::{OwnerLease, headroom};
use crate::network::{Backend, HopResponse, HttpBackend, public_url, validate_addresses};
use crate::{
    PublicWebConfig, PublicWebErrorCode as E, PublicWebFailure, PublicWebReceipt, PublicWebRequest,
    PublicWebResponse, RECEIPT_SCHEMA, REQUEST_SCHEMA, Result, digest,
};

#[derive(Default)]
pub(crate) struct Progress {
    wire: u64,
    decoded: u64,
    redirects: u8,
    http_status: Option<u16>,
}

impl Progress {
    fn failure(&self, request: &PublicWebRequest, code: E) -> PublicWebFailure {
        PublicWebFailure {
            schema: RECEIPT_SCHEMA.into(),
            complete: false,
            code,
            request_sha256: request.sha256(),
            decoded_bytes_observed: self.decoded,
            wire_bytes_observed: self.wire,
            redirects_followed: self.redirects,
            http_status: self.http_status,
        }
    }
}

/// Fetch through the same transport from native callers or the JSON helper.
///
/// # Errors
/// Every failure is incomplete, has no partial body, and contains no URL,
/// query, server body, filesystem path, or underlying exception text.
pub async fn fetch(
    config: &PublicWebConfig,
    runtime_owner: &PublicWebOwnerV2,
    request: &PublicWebRequest,
) -> std::result::Result<PublicWebResponse, PublicWebFailure> {
    if runtime_owner != &config.expected_owner || !runtime_owner.is_well_formed() {
        return Err(Progress::default().failure(request, E::OwnerMismatch));
    }
    fetch_with(config, request, config, &HttpBackend).await
}

pub(crate) async fn fetch_with(
    config: &PublicWebConfig,
    request: &PublicWebRequest,
    authority: &impl Authority,
    backend: &impl Backend,
) -> std::result::Result<PublicWebResponse, PublicWebFailure> {
    let mut progress = Progress::default();
    let result = async {
        let policy = authority.current(request.operation)?;
        let url = request_url(request, policy.search_api.is_some())?;
        let _lease =
            OwnerLease::acquire(&config.owner_lock_dir, &policy.owner.lock_identity_sha256())?;
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(policy.limits.total_timeout_ms))
            .ok_or(E::Deadline)?;
        tokio::time::timeout_at(
            deadline,
            run(
                config,
                request,
                authority,
                backend,
                url,
                &policy,
                deadline,
                &mut progress,
            ),
        )
        .await
        .map_err(|_| E::Deadline)?
    }
    .await;
    result.map_err(|code| progress.failure(request, code))
}

fn request_url(request: &PublicWebRequest, brave: bool) -> Result<Url> {
    if request.schema != REQUEST_SCHEMA
        || request.target.trim().is_empty()
        || request.target.len() > 8192
        || request.target.chars().any(char::is_control)
    {
        return Err(E::InvalidRequest);
    }
    match request.operation {
        PublicWebOperationV2::Page => public_url(&request.target),
        PublicWebOperationV2::Search => {
            if brave {
                return crate::brave::request_url(&request.target);
            }
            if request.target.len() > 2048 {
                return Err(E::InvalidRequest);
            }
            let mut url = public_url("https://html.duckduckgo.com/html/")?;
            url.query_pairs_mut().append_pair("q", &request.target);
            Ok(url)
        },
    }
}

fn check(
    config: &PublicWebConfig,
    request: &PublicWebRequest,
    authority: &impl Authority,
    policy: &PublicWebPolicyV2,
    deadline: Instant,
) -> Result<()> {
    if Instant::now() >= deadline {
        return Err(E::Deadline);
    }
    if authority.current(request.operation)?.sha256() != policy.sha256() {
        return Err(E::AuthorityChanged);
    }
    headroom(
        &config.storage_path,
        policy.limits.storage_headroom_bytes,
        policy.limits.decoded_body_bytes,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "one logical fetch carries a single shared limit/accounting context"
)]
async fn run(
    config: &PublicWebConfig,
    request: &PublicWebRequest,
    authority: &impl Authority,
    backend: &impl Backend,
    mut url: Url,
    policy: &PublicWebPolicyV2,
    deadline: Instant,
    progress: &mut Progress,
) -> Result<PublicWebResponse> {
    let limits = &policy.limits;
    let brave = if request.operation == PublicWebOperationV2::Search {
        policy
            .search_api
            .as_ref()
            .map(|policy| {
                let config = config.brave.as_ref().ok_or(E::InvalidConfig)?;
                Ok::<_, E>((policy, config, crate::brave::credential(config)?))
            })
            .transpose()?
    } else {
        None
    };
    let hop_timeout = match request.operation {
        PublicWebOperationV2::Search => limits.search_hop_timeout_ms,
        PublicWebOperationV2::Page => limits.page_hop_timeout_ms,
    };
    loop {
        progress.http_status = None;
        check(config, request, authority, policy, deadline)?;
        let hop_deadline = Instant::now()
            .checked_add(Duration::from_millis(hop_timeout))
            .ok_or(E::Deadline)?
            .min(deadline);
        let mut response = tokio::time::timeout_at(hop_deadline, async {
            let addresses = backend.resolve(&url).await?;
            validate_addresses(&addresses, url.port_or_known_default().ok_or(E::UnsafeUrl)?)?;
            check(config, request, authority, policy, hop_deadline)?;
            if let Some((policy, config, _)) = &brave {
                crate::spending::reserve(&config.account_directory, policy)?;
            }
            backend
                .get(
                    &url,
                    &addresses,
                    Duration::from_millis(limits.connect_timeout_ms),
                    hop_deadline.saturating_duration_since(Instant::now()),
                    brave.as_ref().map(|(_, _, credential)| credential),
                )
                .await
        })
        .await
        .map_err(|_| E::Deadline)??;
        progress.http_status = Some(response.status);
        if let Some(error) = response.header_error {
            return Err(error);
        }
        let checkpoint = || check(config, request, authority, policy, hop_deadline);
        let body = tokio::time::timeout_at(
            hop_deadline,
            read_body(&mut response, limits, progress, checkpoint),
        )
        .await
        .map_err(|_| E::Deadline)??;
        check(config, request, authority, policy, deadline)?;
        if matches!(response.status, 301 | 302 | 303 | 307 | 308) {
            if brave.is_some() {
                return Err(E::InvalidRedirect);
            }
            if progress.redirects >= limits.redirects {
                return Err(E::RedirectLimit);
            }
            url = redirected_url(&url, response.location.as_deref())?;
            progress.redirects = progress.redirects.checked_add(1).ok_or(E::RedirectLimit)?;
            continue;
        }
        // Known challenge markup can arrive with 200 or 202. It is neither a
        // search result nor evidence of queued work. Never submit its form.
        if search_challenge(request, &url, response.content_type.as_deref(), &body) {
            return Err(E::SearchChallenge);
        }
        let (content_type, body) = complete_body(
            &response,
            body,
            brave.is_some(),
            &request.target,
            limits.decoded_body_bytes,
        )?;
        return Ok(PublicWebResponse {
            final_url: url.to_string(),
            content_type,
            receipt: PublicWebReceipt {
                schema: RECEIPT_SCHEMA.into(),
                complete: true,
                request_sha256: request.sha256(),
                policy_sha256: policy.sha256(),
                owner_sha256: policy.owner.lock_identity_sha256(),
                final_url_sha256: digest(url.as_str().as_bytes()),
                body_sha256: digest(&body),
                decoded_bytes_observed: progress.decoded,
                wire_bytes_observed: progress.wire,
                redirects_followed: progress.redirects,
                http_status: response.status,
            },
            body,
        });
    }
}

fn redirected_url(url: &Url, location: Option<&str>) -> Result<Url> {
    let location = location.ok_or(E::InvalidRedirect)?;
    if location.len() > 8192 || location.chars().any(char::is_control) || location.contains('\\') {
        return Err(E::InvalidRedirect);
    }
    let destination = url.join(location).map_err(|_| E::InvalidRedirect)?;
    public_url(destination.as_str())
}

fn complete_body(
    response: &HopResponse,
    body: Vec<u8>,
    brave: bool,
    query: &str,
    limit: u64,
) -> Result<(String, Vec<u8>)> {
    // A 206 response is incomplete even when its received body reaches EOF.
    if response.status != 200 {
        return Err(match (brave, response.status) {
            (true, 401 | 403) => E::SearchAuthentication,
            (true, 429) => E::SearchRateLimited,
            _ => E::HttpStatus,
        });
    }
    let mut content_type = media_type(response.content_type.as_deref());
    if !matches!(
        content_type.as_str(),
        "text/html"
            | "text/plain"
            | "application/xhtml+xml"
            | "application/json"
            | "application/pdf"
    ) {
        return Err(E::UnsupportedMedia);
    }
    let body = if brave {
        if content_type != "application/json" {
            return Err(E::SearchResponseInvalid);
        }
        content_type = crate::brave::SEARCH_MEDIA.into();
        crate::brave::normalize(&body, query)?
    } else {
        body
    };
    if u64::try_from(body.len()).unwrap_or(u64::MAX) > limit {
        return Err(E::BodyLimit);
    }
    Ok((content_type, body))
}

/// A fixed, transient operator check. Does not install authority or return any
/// search content, and reserves one request against the shared account ceiling.
///
/// # Errors
/// Returns content-free transport, credential or cost-accounting failures.
pub async fn check_brave_connection(
    config: &crate::BraveSearchConfig,
    account_id: &str,
) -> std::result::Result<BraveConnectionCheck, PublicWebFailure> {
    check_brave_with(config, account_id, &HttpBackend).await
}

#[derive(Debug, serde::Serialize)]
pub struct BraveConnectionCheck {
    pub schema: &'static str,
    pub authenticated: bool,
    pub result_count: usize,
    pub results_retained: bool,
    pub http_status: u16,
}

pub(crate) async fn check_brave_with(
    config: &crate::BraveSearchConfig,
    account_id: &str,
    backend: &impl Backend,
) -> std::result::Result<BraveConnectionCheck, PublicWebFailure> {
    let mut progress = Progress::default();
    let request = PublicWebRequest {
        schema: REQUEST_SCHEMA.into(),
        operation: PublicWebOperationV2::Search,
        target: "Brave Search API documentation".into(),
    };
    let result = run_brave_check(config, account_id, backend, &request.target, &mut progress).await;
    result.map_err(|code| progress.failure(&request, code))
}

async fn run_brave_check(
    config: &crate::BraveSearchConfig,
    account_id: &str,
    backend: &impl Backend,
    query: &str,
    progress: &mut Progress,
) -> Result<BraveConnectionCheck> {
    let limits = PublicWebLimitsV2::default();
    let credential = crate::brave::credential(config)?;
    let url = crate::brave::request_url(query)?;
    let _lease = OwnerLease::acquire(&config.account_directory, &digest(b"operator-brave-check"))?;
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(limits.search_hop_timeout_ms))
        .ok_or(E::Deadline)?;
    let checkpoint = || {
        if Instant::now() >= deadline {
            return Err(E::Deadline);
        }
        headroom(
            &config.account_directory,
            limits.storage_headroom_bytes,
            limits.decoded_body_bytes,
        )
    };
    tokio::time::timeout_at(deadline, async {
        checkpoint()?;
        let addresses = backend.resolve(&url).await?;
        validate_addresses(&addresses, 443)?;
        checkpoint()?;
        crate::spending::reserve_operator_check(&config.account_directory, account_id)?;
        let mut response = backend
            .get(
                &url,
                &addresses,
                Duration::from_millis(limits.connect_timeout_ms),
                deadline.saturating_duration_since(Instant::now()),
                Some(&credential),
            )
            .await?;
        progress.http_status = Some(response.status);
        if let Some(error) = response.header_error {
            return Err(error);
        }
        if matches!(response.status, 301 | 302 | 303 | 307 | 308) {
            return Err(E::InvalidRedirect);
        }
        let body = read_body(&mut response, &limits, progress, checkpoint).await?;
        let (_, body) = complete_body(&response, body, true, query, limits.decoded_body_bytes)?;
        let envelope: serde_json::Value =
            serde_json::from_slice(&body).map_err(|_| E::SearchResponseInvalid)?;
        let result_count = envelope
            .get("results")
            .and_then(serde_json::Value::as_array)
            .ok_or(E::SearchResponseInvalid)?
            .len();
        Ok(BraveConnectionCheck {
            schema: "astrid.brave_connection_check.v1",
            authenticated: true,
            result_count,
            results_retained: false,
            http_status: response.status,
        })
    })
    .await
    .map_err(|_| E::Deadline)?
}

fn media_type(value: Option<&str>) -> String {
    value
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

fn search_challenge(
    request: &PublicWebRequest,
    url: &Url,
    content_type: Option<&str>,
    body: &[u8],
) -> bool {
    if request.operation != PublicWebOperationV2::Search
        || url.host_str() != Some("html.duckduckgo.com")
        || !matches!(url.path(), "/html/" | "/html")
        || media_type(content_type) != "text/html"
    {
        return false;
    }
    // A deliberately narrow fingerprint of the observed service template,
    // not a general classifier of arbitrary pages or discussion of CAPTCHAs.
    std::str::from_utf8(body).is_ok_and(|html| {
        html.contains("<form id=\"challenge-form\"")
            && html.contains("class=\"anomaly-modal__description\"")
            && html.contains("Please complete the following challenge to confirm this search was made by a human.")
    })
}

async fn read_body(
    response: &mut HopResponse,
    limits: &PublicWebLimitsV2,
    progress: &mut Progress,
    checkpoint: impl Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let encoding = response.encoding.trim().to_ascii_lowercase();
    if !matches!(encoding.as_str(), "identity" | "gzip") {
        return Err(E::UnsupportedEncoding);
    }
    let mut wire = Vec::new();
    loop {
        checkpoint()?;
        let Some(chunk) = response.body.next().await? else {
            checkpoint()?;
            break;
        };
        progress.wire = progress
            .wire
            .checked_add(u64::try_from(chunk.len()).map_err(|_| E::BodyLimit)?)
            .ok_or(E::BodyLimit)?;
        if encoding == "identity" {
            count_decoded(progress, chunk.len(), limits)?;
        }
        if progress.wire > limits.wire_body_bytes {
            return Err(E::BodyLimit);
        }
        checkpoint()?;
        wire.extend_from_slice(&chunk);
    }
    if encoding == "identity" {
        return Ok(wire);
    }
    // Decode only bounded encoded input, and cap output before extending it.
    // MultiGzDecoder validates checksums, truncation, and concatenated members.
    let mut decoder = flate2::read::MultiGzDecoder::new(Cursor::new(wire));
    let mut output = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        checkpoint()?;
        let count = decoder.read(&mut chunk).map_err(|_| E::IncompleteBody)?;
        count_decoded(progress, count, limits)?;
        checkpoint()?;
        if count == 0 {
            break;
        }
        output.extend_from_slice(&chunk[..count]);
        tokio::task::yield_now().await;
    }
    Ok(output)
}

fn count_decoded(progress: &mut Progress, count: usize, limits: &PublicWebLimitsV2) -> Result<()> {
    progress.decoded = progress
        .decoded
        .checked_add(u64::try_from(count).map_err(|_| E::BodyLimit)?)
        .ok_or(E::BodyLimit)?;
    if progress.decoded > limits.decoded_body_bytes {
        return Err(E::BodyLimit);
    }
    Ok(())
}
