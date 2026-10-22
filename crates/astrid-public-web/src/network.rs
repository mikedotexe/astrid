use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use async_trait::async_trait;
use reqwest::header::{CONTENT_ENCODING, CONTENT_TYPE, LOCATION};
use url::{Host, Url};

use crate::{PublicWebErrorCode as E, Result};

pub(crate) fn public_url(raw: &str) -> Result<Url> {
    if raw.is_empty()
        || raw.len() > 8192
        || raw.trim() != raw
        || raw.chars().any(|c| c.is_control() || c == '\\')
    {
        return Err(E::UnsafeUrl);
    }
    let mut url = Url::parse(raw).map_err(|_| E::UnsafeUrl)?;
    if !matches!(
        (url.scheme(), url.port_or_known_default()),
        ("http", Some(80)) | ("https", Some(443))
    ) || !url.username().is_empty()
        || url.password().is_some()
        || raw.split_once("://").is_none_or(|(_, tail)| {
            tail.split(['/', '?', '#'])
                .next()
                .unwrap_or_default()
                .contains('@')
        })
    {
        return Err(E::UnsafeUrl);
    }
    match url.host().ok_or(E::UnsafeUrl)? {
        Host::Ipv4(ip) if public_ip(IpAddr::V4(ip)) => {},
        Host::Ipv6(ip) if public_ip(IpAddr::V6(ip)) => {},
        Host::Domain(host) if public_name(host) => {},
        _ => return Err(E::UnsafeUrl),
    }
    // Fragments are local navigation, never sent as network authority.
    url.set_fragment(None);
    Ok(url)
}

fn public_name(host: &str) -> bool {
    host.len() <= 253
        && host.contains('.')
        && !host.ends_with('.')
        && ![
            "localhost",
            "local",
            "internal",
            "lan",
            "home",
            "arpa",
            "onion",
            "invalid",
            "test",
        ]
        .iter()
        .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")))
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

// Conservative public-unicast policy adapted from edge-web-broker/security.rs.
// Reject transition/special ranges too; never recover a private address from a
// mixed DNS answer set or through a proxy's independent DNS lookup.
pub(crate) fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => public_v4(ip),
        IpAddr::V6(ip) => public_v6(ip),
    }
}

fn public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(matches!(a, 0 | 10 | 127)
        || a >= 224
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && (b == 168 || (b == 0 && matches!(c, 0 | 2)) || (b == 88 && c == 99)))
        || (a == 198 && ((18..=19).contains(&b) || (b == 51 && c == 100)))
        || (a == 203 && b == 0 && c == 113))
}

fn public_v6(ip: Ipv6Addr) -> bool {
    let s = ip.segments();
    (s[0] & 0xe000) == 0x2000
        && !(s[0] == 0x2001 && (s[1] <= 0x01ff || s[1] == 0x0db8))
        && s[0] != 0x2002
        && !(s[0] == 0x3fff && s[1] <= 0x0fff)
        && ip.to_ipv4_mapped().is_none()
}

pub(crate) fn validate_addresses(addresses: &[SocketAddr], port: u16) -> Result<()> {
    if addresses.is_empty()
        || addresses.len() > 32
        || addresses
            .iter()
            .any(|address| address.port() != port || !public_ip(address.ip()))
    {
        return Err(E::UnsafeAddress);
    }
    Ok(())
}

#[async_trait]
pub(crate) trait Body: Send {
    async fn next(&mut self) -> Result<Option<Vec<u8>>>;
}

pub(crate) struct HopResponse {
    pub status: u16,
    pub location: Option<String>,
    pub content_type: Option<String>,
    pub encoding: String,
    pub body: Box<dyn Body>,
}

#[async_trait]
pub(crate) trait Backend: Send + Sync {
    async fn resolve(&self, url: &Url) -> Result<Vec<SocketAddr>>;
    async fn get(
        &self,
        url: &Url,
        pinned: &[SocketAddr],
        connect: Duration,
        timeout: Duration,
    ) -> Result<HopResponse>;
}

pub(crate) struct HttpBackend;

struct HttpBody(reqwest::Response);

#[async_trait]
impl Body for HttpBody {
    async fn next(&mut self) -> Result<Option<Vec<u8>>> {
        self.0
            .chunk()
            .await
            .map(|chunk| chunk.map(|bytes| bytes.to_vec()))
            .map_err(|_| E::IncompleteBody)
    }
}

fn one_header(
    headers: &reqwest::header::HeaderMap,
    name: reqwest::header::HeaderName,
) -> Result<Option<String>> {
    let mut values = headers.get_all(name).iter();
    let value = values
        .next()
        .map(|value| value.to_str().map(str::to_owned))
        .transpose()
        .map_err(|_| E::InvalidHeaders)?;
    if values.next().is_some() || value.as_ref().is_some_and(|value| value.len() > 8192) {
        return Err(E::InvalidHeaders);
    }
    Ok(value)
}

#[async_trait]
impl Backend for HttpBackend {
    async fn resolve(&self, url: &Url) -> Result<Vec<SocketAddr>> {
        let port = url.port_or_known_default().ok_or(E::UnsafeUrl)?;
        match url.host().ok_or(E::UnsafeUrl)? {
            Host::Ipv4(ip) => Ok(vec![SocketAddr::new(IpAddr::V4(ip), port)]),
            Host::Ipv6(ip) => Ok(vec![SocketAddr::new(IpAddr::V6(ip), port)]),
            Host::Domain(host) => tokio::net::lookup_host((host, port))
                .await
                .map(|addresses| addresses.take(33).collect())
                .map_err(|_| E::DnsFailed),
        }
    }

    async fn get(
        &self,
        url: &Url,
        pinned: &[SocketAddr],
        connect: Duration,
        timeout: Duration,
    ) -> Result<HopResponse> {
        validate_addresses(pinned, url.port_or_known_default().ok_or(E::UnsafeUrl)?)?;
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .referer(false)
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .connect_timeout(connect.min(timeout))
            .timeout(timeout)
            .resolve_to_addrs(url.host_str().ok_or(E::UnsafeUrl)?, pinned)
            .build()
            .map_err(|_| E::TransportFailed)?;
        let response = client
            .get(url.clone())
            .header("Accept-Encoding", "identity")
            .header(
                "Accept",
                "text/html, application/xhtml+xml, text/plain, application/json",
            )
            .header("User-Agent", "Astrid-Public-Web/2")
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    E::Deadline
                } else {
                    E::TransportFailed
                }
            })?;
        if response.url() != url
            || response
                .remote_addr()
                .is_none_or(|address| !pinned.contains(&address))
        {
            return Err(E::UnsafeAddress);
        }
        let location = one_header(response.headers(), LOCATION)?;
        let content_type = one_header(response.headers(), CONTENT_TYPE)?;
        let encoding =
            one_header(response.headers(), CONTENT_ENCODING)?.unwrap_or_else(|| "identity".into());
        Ok(HopResponse {
            status: response.status().as_u16(),
            location,
            content_type,
            encoding,
            body: Box::new(HttpBody(response)),
        })
    }
}
