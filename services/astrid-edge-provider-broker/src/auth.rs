use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest as _, Sha256};

use crate::{Error, Result};

pub const CLIENT_HEADER: &str = "x-astrid-provider-client";
pub const NONCE_HEADER: &str = "x-astrid-provider-nonce";
pub const AUTH_HEADER: &str = "x-astrid-provider-auth";
pub const RUNTIME_CLIENT: &str = "edge-runtime";
pub const STEWARD_CLIENT: &str = "edge-steward";
pub const WARMUP_CLIENT: &str = "model-warmup";

const DOMAIN: &[u8] = b"astrid.edge.provider_broker.request.v1";
const MAXIMUM_NONCE_AGE_MS: u64 = 120_000;
const MAXIMUM_FUTURE_SKEW_MS: u64 = 30_000;
const MAXIMUM_NONCES: usize = 4_096;
static NONCE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub fn request_signature(
    key: &[u8; 32],
    client: &str,
    path: &str,
    nonce: &str,
    body: &[u8],
) -> Result<String> {
    validate_client(client)?;
    validate_nonce(nonce)?;
    let digest = Sha256::digest(body);
    Ok(hmac_fields(
        key,
        DOMAIN,
        &[
            client.as_bytes(),
            path.as_bytes(),
            nonce.as_bytes(),
            &digest,
        ],
    ))
}

pub fn request_hash(client: &str, path: &str, nonce: &str, body: &[u8]) -> Result<String> {
    validate_client(client)?;
    validate_nonce(nonce)?;
    let body_hash = Sha256::digest(body);
    let encoded = encoded_fields(
        b"astrid.edge.provider_broker.request_hash.v1",
        &[
            client.as_bytes(),
            path.as_bytes(),
            nonce.as_bytes(),
            &body_hash,
        ],
    );
    Ok(format!("{:x}", Sha256::digest(encoded)))
}

pub fn verify(expected: &str, supplied: &str) -> bool {
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

pub fn fresh_nonce() -> Result<String> {
    let now = unix_millis()?;
    let pid = u64::from(std::process::id());
    let sequence = NONCE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let material = format!("{now}:{pid}:{sequence}:{:?}", std::thread::current().id());
    let suffix = format!("{:x}", Sha256::digest(material.as_bytes()));
    Ok(format!("{now:016x}{}", &suffix[..48]))
}

pub struct ReplayGuard(Mutex<BTreeMap<String, u64>>);

impl Default for ReplayGuard {
    fn default() -> Self {
        Self(Mutex::new(BTreeMap::new()))
    }
}

impl ReplayGuard {
    pub fn accept(&self, client: &str, nonce: &str) -> Result<()> {
        validate_client(client)?;
        validate_nonce(nonce)?;
        let now = unix_millis()?;
        let recorded = u64::from_str_radix(&nonce[..16], 16)
            .map_err(|_| Error::new("authentication nonce timestamp is malformed"))?;
        if recorded > now.saturating_add(MAXIMUM_FUTURE_SKEW_MS)
            || now.saturating_sub(recorded) > MAXIMUM_NONCE_AGE_MS
        {
            return Err(Error::new(
                "authentication nonce is outside freshness window",
            ));
        }
        let mut values = self
            .0
            .lock()
            .map_err(|_| Error::new("authentication replay state is unavailable"))?;
        values.retain(|_, timestamp| now.saturating_sub(*timestamp) <= MAXIMUM_NONCE_AGE_MS);
        if values.contains_key(nonce) {
            return Err(Error::new("authentication nonce was replayed"));
        }
        if values.len() >= MAXIMUM_NONCES {
            return Err(Error::new("authentication replay window is full"));
        }
        values.insert(nonce.to_owned(), recorded);
        Ok(())
    }
}

pub struct Quotas(Mutex<BTreeMap<String, VecDeque<u64>>>);

impl Default for Quotas {
    fn default() -> Self {
        Self(Mutex::new(BTreeMap::new()))
    }
}

impl Quotas {
    pub fn accept(&self, client: &str, maximum_per_hour: u16) -> Result<()> {
        let now = unix_millis()?;
        let cutoff = now.saturating_sub(3_600_000);
        let mut all = self
            .0
            .lock()
            .map_err(|_| Error::new("provider quota state is unavailable"))?;
        let values = all.entry(client.to_owned()).or_default();
        while values.front().is_some_and(|value| *value < cutoff) {
            let _ = values.pop_front();
        }
        if values.len() >= usize::from(maximum_per_hour) {
            return Err(Error::new("provider client quota exhausted"));
        }
        values.push_back(now);
        Ok(())
    }
}

pub fn key_from_bytes(bytes: &[u8]) -> Result<[u8; 32]> {
    bytes
        .try_into()
        .map_err(|_| Error::new("authentication key must contain exactly 32 bytes"))
}

fn validate_client(value: &str) -> Result<()> {
    if !matches!(value, RUNTIME_CLIENT | STEWARD_CLIENT | WARMUP_CLIENT) {
        return Err(Error::new("provider client is not allowlisted"));
    }
    Ok(())
}

fn validate_nonce(value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(Error::new("authentication nonce is not canonical"));
    }
    Ok(())
}

fn unix_millis() -> Result<u64> {
    let value = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::new("system clock is before the Unix epoch"))?
        .as_millis();
    u64::try_from(value).map_err(|_| Error::new("Unix time does not fit u64"))
}

pub fn hmac_fields(key: &[u8; 32], domain: &[u8], fields: &[&[u8]]) -> String {
    const BLOCK: usize = 64;
    let mut normalized = [0_u8; BLOCK];
    normalized[..key.len()].copy_from_slice(key);
    let mut inner_pad = [0x36_u8; BLOCK];
    let mut outer_pad = [0x5c_u8; BLOCK];
    for index in 0..BLOCK {
        inner_pad[index] ^= normalized[index];
        outer_pad[index] ^= normalized[index];
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(encoded_fields(domain, fields));
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner.finalize());
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

fn append_field(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    target.extend_from_slice(value);
}

#[cfg(test)]
mod tests {
    use super::{Quotas, ReplayGuard, request_signature, verify};

    #[test]
    fn signatures_bind_client_route_nonce_and_body() {
        let nonce = format!("{:016x}{}", now_millis(), "a".repeat(48));
        let expected = request_signature(
            &[7; 32],
            "edge-runtime",
            "/v1/chat/completions",
            &nonce,
            b"{}",
        )
        .unwrap();
        assert!(verify(&expected, &expected));
        assert!(!verify(&expected, &"0".repeat(64)));
        let replay = ReplayGuard::default();
        replay.accept("edge-runtime", &nonce).unwrap();
        assert!(replay.accept("edge-runtime", &nonce).is_err());
        assert!(replay.accept("edge-steward", &nonce).is_err());
    }

    #[test]
    fn quotas_are_per_client_and_fail_closed_at_the_limit() {
        let quotas = Quotas::default();
        quotas.accept("edge-runtime", 1).unwrap();
        assert!(quotas.accept("edge-runtime", 1).is_err());
        quotas.accept("edge-steward", 1).unwrap();
        assert!(quotas.accept("edge-steward", 1).is_err());
    }

    fn now_millis() -> u64 {
        u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap()
    }
}
