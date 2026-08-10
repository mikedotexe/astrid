//! Exact request/response authentication for the loopback broker protocol.

use std::collections::BTreeMap;
use std::sync::Mutex;

use ed25519_dalek::{Signer as _, SigningKey};
use sha2::{Digest as _, Sha256};

use crate::{Error, Result};

const PROTOCOL_VERSION: &[u8] = b"astrid.edge.web_broker.auth.v2";
const REQUEST_DOMAIN: &[u8] = b"astrid.edge.web_broker.request_auth.v2";
const REQUEST_HASH_DOMAIN: &[u8] = b"astrid.edge.web_broker.request_hash.v2";
const RESPONSE_DOMAIN: &[u8] = b"astrid.edge.web_broker.response_signature.v2";
const MAXIMUM_NONCE_AGE_MS: u64 = 120_000;
const MAXIMUM_FUTURE_SKEW_MS: u64 = 30_000;
const MAXIMUM_TRACKED_NONCES_PER_CLIENT: usize = 2_048;

pub(crate) const RUNTIME_CLIENT_ID: &str = "edge-runtime";
pub(crate) const STEWARD_CLIENT_ID: &str = "edge-steward";
pub(crate) const CORE_CLIENT_ID: &str = "edge-core";
pub(crate) const CLIENT_HEADER: &str = "x-astrid-web-client";
pub(crate) const NONCE_HEADER: &str = "x-astrid-web-nonce";
pub(crate) const AUTH_HEADER: &str = "x-astrid-web-auth";

#[derive(Debug, Default)]
pub(crate) struct ReplayGuard {
    runtime: Mutex<BTreeMap<String, u64>>,
    steward: Mutex<BTreeMap<String, u64>>,
    core: Mutex<BTreeMap<String, u64>>,
}

impl ReplayGuard {
    pub(crate) fn accept(&self, client_id: &str, nonce: &str) -> Result<()> {
        self.accept_at(client_id, nonce, unix_millis())
    }

    fn accept_at(&self, client_id: &str, nonce: &str, now: u64) -> Result<()> {
        validate_client_id(client_id)?;
        let recorded = nonce_timestamp(nonce)?;
        if recorded > now.saturating_add(MAXIMUM_FUTURE_SKEW_MS)
            || now.saturating_sub(recorded) > MAXIMUM_NONCE_AGE_MS
        {
            return Err(Error::new(
                "broker request nonce is outside the freshness window",
            ));
        }
        let window = match client_id {
            RUNTIME_CLIENT_ID => &self.runtime,
            STEWARD_CLIENT_ID => &self.steward,
            CORE_CLIENT_ID => &self.core,
            _ => return Err(Error::new("broker client identity is not allowlisted")),
        };
        let mut accepted = window
            .lock()
            .map_err(|_| Error::new("broker replay state is unavailable"))?;
        accepted.retain(|_, timestamp| {
            now.saturating_sub(*timestamp) <= MAXIMUM_NONCE_AGE_MS
                && *timestamp <= now.saturating_add(MAXIMUM_FUTURE_SKEW_MS)
        });
        if accepted.contains_key(nonce) {
            return Err(Error::new("broker request nonce was replayed"));
        }
        if accepted.len() >= MAXIMUM_TRACKED_NONCES_PER_CLIENT {
            return Err(Error::new("broker client replay window is full"));
        }
        accepted.insert(nonce.to_owned(), recorded);
        Ok(())
    }
}

pub(crate) fn request_signature(
    key: &[u8; 32],
    client_id: &str,
    path: &str,
    host: &str,
    nonce: &str,
    body: &[u8],
) -> Result<String> {
    validate_client_id(client_id)?;
    validate_nonce(nonce)?;
    let body_hash = Sha256::digest(body);
    Ok(hmac_fields(
        key,
        REQUEST_DOMAIN,
        &[
            PROTOCOL_VERSION,
            client_id.as_bytes(),
            path.as_bytes(),
            host.as_bytes(),
            nonce.as_bytes(),
            &body_hash,
        ],
    ))
}

pub(crate) fn request_hash(
    client_id: &str,
    path: &str,
    host: &str,
    nonce: &str,
    body: &[u8],
) -> Result<String> {
    validate_client_id(client_id)?;
    validate_nonce(nonce)?;
    let body_hash = Sha256::digest(body);
    let encoded = encoded_fields(
        REQUEST_HASH_DOMAIN,
        &[
            PROTOCOL_VERSION,
            client_id.as_bytes(),
            path.as_bytes(),
            host.as_bytes(),
            nonce.as_bytes(),
            &body_hash,
        ],
    );
    Ok(format!("{:x}", Sha256::digest(encoded)))
}

pub(crate) fn response_signature(
    key: &SigningKey,
    client_id: &str,
    nonce: &str,
    status: u16,
    request_hash: &str,
    body: &[u8],
) -> Result<String> {
    let message = response_message(client_id, nonce, status, request_hash, body)?;
    Ok(lower_hex(&key.sign(&message).to_bytes()))
}

fn response_message(
    client_id: &str,
    nonce: &str,
    status: u16,
    request_hash: &str,
    body: &[u8],
) -> Result<Vec<u8>> {
    validate_client_id(client_id)?;
    validate_nonce(nonce)?;
    validate_hex64(request_hash, "request hash")?;
    let status = status.to_string();
    let body_hash = Sha256::digest(body);
    Ok(encoded_fields(
        RESPONSE_DOMAIN,
        &[
            PROTOCOL_VERSION,
            client_id.as_bytes(),
            nonce.as_bytes(),
            status.as_bytes(),
            request_hash.as_bytes(),
            &body_hash,
        ],
    ))
}

pub(crate) fn verify_hmac(expected: &str, supplied: &str) -> bool {
    expected.len() == 64
        && supplied.len() == 64
        && expected
            .bytes()
            .zip(supplied.bytes())
            .fold(0_u8, |difference, (left, right)| {
                difference | (left ^ right)
            })
            == 0
}

fn validate_client_id(value: &str) -> Result<()> {
    if !matches!(
        value,
        RUNTIME_CLIENT_ID | STEWARD_CLIENT_ID | CORE_CLIENT_ID
    ) {
        return Err(Error::new("broker client identity is not allowlisted"));
    }
    Ok(())
}

fn validate_nonce(value: &str) -> Result<()> {
    validate_hex64(value, "request nonce")
}

fn validate_hex64(value: &str, label: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(Error::new(format!("broker {label} is not canonical")));
    }
    Ok(())
}

fn nonce_timestamp(value: &str) -> Result<u64> {
    validate_nonce(value)?;
    u64::from_str_radix(&value[..16], 16)
        .map_err(|_| Error::new("broker request nonce timestamp is malformed"))
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
    let capacity = domain
        .len()
        .saturating_add(fields.iter().map(|field| field.len()).sum::<usize>())
        .saturating_add(fields.len().saturating_add(1).saturating_mul(8));
    let mut encoded = Vec::with_capacity(capacity);
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

fn lower_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;

    use crate::config::BROKER_HTTP_AUTHORITY;

    use super::{
        CORE_CLIENT_ID, RUNTIME_CLIENT_ID, ReplayGuard, STEWARD_CLIENT_ID, request_hash,
        request_signature, response_signature, verify_hmac,
    };

    #[test]
    fn authentication_binds_client_direction_route_nonce_status_request_and_body() {
        let runtime_key = [0x42; 32];
        let steward_key = [0x43; 32];
        let signing = SigningKey::from_bytes(&[0x44; 32]);
        let nonce = "a".repeat(64);
        let request = request_signature(
            &runtime_key,
            RUNTIME_CLIENT_ID,
            "/v1/search",
            BROKER_HTTP_AUTHORITY,
            &nonce,
            b"body",
        )
        .unwrap();
        assert!(verify_hmac(&request, &request));
        assert_ne!(
            request,
            request_signature(
                &steward_key,
                STEWARD_CLIENT_ID,
                "/v1/search",
                BROKER_HTTP_AUTHORITY,
                &nonce,
                b"body",
            )
            .unwrap()
        );
        let hash = request_hash(
            RUNTIME_CLIENT_ID,
            "/v1/search",
            BROKER_HTTP_AUTHORITY,
            &nonce,
            b"body",
        )
        .unwrap();
        assert_eq!(
            response_signature(&signing, RUNTIME_CLIENT_ID, &nonce, 200, &hash, b"body")
                .unwrap()
                .len(),
            128
        );
    }

    #[test]
    fn replay_is_isolated_by_client_and_time_windows_are_exact() {
        let replay = ReplayGuard::default();
        let now = 1_700_000_000_000_u64;
        let nonce = format!("{now:016x}{}", "a".repeat(48));
        replay.accept_at(RUNTIME_CLIENT_ID, &nonce, now).unwrap();
        assert!(replay.accept_at(RUNTIME_CLIENT_ID, &nonce, now).is_err());
        replay.accept_at(STEWARD_CLIENT_ID, &nonce, now).unwrap();
        replay.accept_at(CORE_CLIENT_ID, &nonce, now).unwrap();
        let stale = format!("{:016x}{}", now - 120_001, "b".repeat(48));
        assert!(replay.accept_at(RUNTIME_CLIENT_ID, &stale, now).is_err());
        let future = format!("{:016x}{}", now + 30_001, "c".repeat(48));
        assert!(replay.accept_at(RUNTIME_CLIENT_ID, &future, now).is_err());
    }

    #[test]
    fn one_client_cannot_consume_the_other_replay_capacity() {
        let replay = ReplayGuard::default();
        let now = 1_700_000_000_000_u64;
        for sequence in 0..super::MAXIMUM_TRACKED_NONCES_PER_CLIENT {
            let nonce = format!("{now:016x}{sequence:048x}");
            replay.accept_at(RUNTIME_CLIENT_ID, &nonce, now).unwrap();
        }
        let overflow = format!(
            "{now:016x}{:048x}",
            super::MAXIMUM_TRACKED_NONCES_PER_CLIENT
        );
        assert!(replay.accept_at(RUNTIME_CLIENT_ID, &overflow, now).is_err());
        replay.accept_at(STEWARD_CLIENT_ID, &overflow, now).unwrap();
        replay.accept_at(CORE_CLIENT_ID, &overflow, now).unwrap();
    }

    #[test]
    fn constant_time_compare_rejects_wrong_shape_and_value() {
        let valid = "a".repeat(64);
        assert!(verify_hmac(&valid, &valid));
        assert!(!verify_hmac(&valid, &"b".repeat(64)));
        assert!(!verify_hmac(&valid, "short"));
    }
}
