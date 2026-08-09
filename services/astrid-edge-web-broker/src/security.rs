use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

#[derive(Clone, Debug, Default)]
pub struct PublicDnsResolver;

impl Resolve for PublicDnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_ascii_lowercase();
        Box::pin(async move {
            if !safe_public_dns_name(&host) {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "upstream DNS host escaped public-name policy",
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }
            let resolved = tokio::net::lookup_host((host.as_str(), 443_u16))
                .await
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>)?
                .collect::<Vec<_>>();
            if resolved.is_empty()
                || resolved
                    .iter()
                    .any(|value| !is_public_upstream_ip(value.ip()))
            {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "upstream DNS returned a private, local, special, or empty address set",
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }
            let addresses: Addrs = Box::new(
                resolved
                    .into_iter()
                    .map(|value| SocketAddr::new(value.ip(), 0)),
            );
            Ok(addresses)
        })
    }
}

fn safe_public_dns_name(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.contains('.')
        && host != "localhost"
        && !host
            .rsplit_once('.')
            .is_some_and(|(_, suffix)| matches!(suffix, "localhost" | "local"))
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

/// Return true only for public-unicast addresses approved for an upstream
/// search connection. Documentation, benchmarking, local, private, link-local,
/// multicast, unspecified, mapped-private, and reserved space are rejected.
#[must_use]
pub fn is_public_upstream_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(value) => is_public_ipv4(value),
        IpAddr::V6(value) => is_public_ipv6(value),
    }
}

fn is_public_ipv4(value: Ipv4Addr) -> bool {
    let octets = value.octets();
    !(octets[0] == 0
        || octets[0] == 10
        || octets[0] == 127
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 169 && octets[1] == 254)
        || (octets[0] == 172 && (16..=31).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
        || (octets[0] == 192 && octets[1] == 88 && octets[2] == 99)
        || (octets[0] == 192 && octets[1] == 168)
        || (octets[0] == 198 && (octets[1] == 18 || octets[1] == 19))
        || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
        || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
        || octets[0] >= 224)
}

fn is_public_ipv6(value: Ipv6Addr) -> bool {
    if let Some(mapped) = value.to_ipv4_mapped() {
        return is_public_ipv4(mapped);
    }
    let segments = value.segments();
    let in_global_unicast = (segments[0] & 0xe000) == 0x2000;
    let documentation = segments[0] == 0x2001 && segments[1] == 0x0db8;
    let benchmarking = segments[0] == 0x2001 && segments[1] == 0x0002 && segments[2] == 0;
    in_global_unicast
        && !documentation
        && !benchmarking
        && !value.is_unspecified()
        && !value.is_loopback()
        && !value.is_multicast()
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use super::{is_public_upstream_ip, safe_public_dns_name};

    #[test]
    fn ssrf_ranges_are_rejected_including_mapped_ipv4() {
        for text in [
            "0.0.0.0",
            "10.1.2.3",
            "100.64.1.2",
            "127.0.0.1",
            "169.254.169.254",
            "172.31.2.3",
            "192.168.1.1",
            "198.18.0.1",
            "224.0.0.1",
            "::1",
            "fe80::1",
            "fd00::1",
            "2001:db8::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(
                !is_public_upstream_ip(text.parse::<IpAddr>().unwrap()),
                "{text}"
            );
        }
        for text in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
            assert!(
                is_public_upstream_ip(text.parse::<IpAddr>().unwrap()),
                "{text}"
            );
        }
    }

    #[test]
    fn only_canonical_public_dns_names_reach_resolution() {
        for host in ["search.brave.com", "docs.rs", "sub.example.org"] {
            assert!(safe_public_dns_name(host));
        }
        for host in [
            "localhost",
            "metadata.local",
            "singlelabel",
            ".example.org",
            "example..org",
            "-bad.example",
        ] {
            assert!(!safe_public_dns_name(host), "accepted {host}");
        }
    }
}
