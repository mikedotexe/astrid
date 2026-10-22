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
        let url = request_url(request)?;
        let policy = authority.current(request.operation)?;
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

fn request_url(request: &PublicWebRequest) -> Result<Url> {
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
    let hop_timeout = match request.operation {
        PublicWebOperationV2::Search => limits.search_hop_timeout_ms,
        PublicWebOperationV2::Page => limits.page_hop_timeout_ms,
    };
    loop {
        check(config, request, authority, policy, deadline)?;
        let hop_deadline = Instant::now()
            .checked_add(Duration::from_millis(hop_timeout))
            .ok_or(E::Deadline)?
            .min(deadline);
        let mut response = tokio::time::timeout_at(hop_deadline, async {
            let addresses = backend.resolve(&url).await?;
            validate_addresses(&addresses, url.port_or_known_default().ok_or(E::UnsafeUrl)?)?;
            check(config, request, authority, policy, hop_deadline)?;
            backend
                .get(
                    &url,
                    &addresses,
                    Duration::from_millis(limits.connect_timeout_ms),
                    hop_deadline.saturating_duration_since(Instant::now()),
                )
                .await
        })
        .await
        .map_err(|_| E::Deadline)??;
        let checkpoint = || check(config, request, authority, policy, hop_deadline);
        let body = tokio::time::timeout_at(
            hop_deadline,
            read_body(&mut response, limits, progress, checkpoint),
        )
        .await
        .map_err(|_| E::Deadline)??;
        check(config, request, authority, policy, deadline)?;
        if matches!(response.status, 301 | 302 | 303 | 307 | 308) {
            if progress.redirects >= limits.redirects {
                return Err(E::RedirectLimit);
            }
            let location = response.location.as_deref().ok_or(E::InvalidRedirect)?;
            if location.len() > 8192
                || location.chars().any(char::is_control)
                || location.contains('\\')
            {
                return Err(E::InvalidRedirect);
            }
            let destination = url.join(location).map_err(|_| E::InvalidRedirect)?;
            url = public_url(destination.as_str())?;
            progress.redirects = progress.redirects.checked_add(1).ok_or(E::RedirectLimit)?;
            continue;
        }
        // A 206 response is incomplete even when its received body reaches EOF.
        if response.status != 200 {
            return Err(E::HttpStatus);
        }
        let content_type = response
            .content_type
            .as_deref()
            .unwrap_or_default()
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
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
