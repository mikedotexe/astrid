use std::fs::{self, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use ed25519_dalek::SigningKey;
use reqwest::Url;
use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use crate::{Error, Result};

pub const CONFIG_SCHEMA: &str = "astrid.edge.web_broker.config.v3";
pub const REQUEST_SCHEMA: &str = "astrid.edge.web_search.request.v2";
pub const RESPONSE_SCHEMA: &str = "astrid.edge.web_search.response.v1";
pub const LISTEN_PATH: &str = "/v1/search";
pub const BROKER_HTTP_AUTHORITY: &str = "astrid-edge-web-broker";
pub const RUNTIME_SOCKET_PATH: &str = "/run/astrid-edge-self-change/web-runtime.sock";
pub const STEWARD_SOCKET_PATH: &str = "/run/astrid-edge-self-change/web-steward.sock";
pub const CORE_SOCKET_PATH: &str = "/run/astrid-edge-self-change/web-core.sock";
const CONFIG_MAX_BYTES: u64 = 32 * 1024;
const ALLOWED_UPSTREAM_HOST: &str = "search.brave.com";
const ALLOWED_UPSTREAM_PATH: &str = "/search";
pub(crate) const RUNTIME_QUOTA_STATE_PATH: &str =
    "/var/lib/astrid-edge-web-runtime/search-quota.jsonl";
pub(crate) const STEWARD_QUOTA_STATE_PATH: &str =
    "/var/lib/astrid-edge-web-steward/search-quota.jsonl";
pub(crate) const CORE_QUOTA_STATE_PATH: &str = "/var/lib/astrid-edge-web-core/search-quota.jsonl";
pub(crate) const RUNTIME_SEARCHES_PER_HOUR: u16 = 8;
pub(crate) const RUNTIME_SEARCHES_PER_UTC_DAY: u16 = 24;
pub(crate) const STEWARD_SEARCHES_PER_HOUR: u16 = 2;
pub(crate) const STEWARD_SEARCHES_PER_UTC_DAY: u16 = 12;
pub(crate) const CORE_SEARCHES_PER_HOUR: u16 = 8;
pub(crate) const CORE_SEARCHES_PER_UTC_DAY: u16 = 24;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    pub client_id: String,
    pub socket_path: PathBuf,
    pub expected_peer_uid: u32,
    pub socket_gid: u32,
    pub upstream_origin: String,
    pub connect_timeout_ms: u64,
    pub header_timeout_ms: u64,
    pub total_timeout_ms: u64,
    pub client_read_timeout_ms: u64,
    pub client_write_timeout_ms: u64,
    pub maximum_request_body_bytes: u32,
    pub maximum_upstream_body_bytes: u32,
    pub maximum_results: u8,
    pub maximum_concurrent_requests: u8,
    pub maximum_searches_per_hour: u16,
    pub maximum_searches_per_utc_day: u16,
    pub quota_state_path: PathBuf,
    pub request_key_path: PathBuf,
    pub request_key_sha256: String,
    pub response_signing_key_path: PathBuf,
    pub response_signing_key_sha256: String,
    pub response_verify_key_sha256: String,
}

impl Config {
    /// Load an exact root-owned mode-0440 configuration without following links.
    ///
    /// # Errors
    ///
    /// Returns an error if ownership, mode, path identity, size, schema, origin, or
    /// any immutable resource envelope is invalid.
    pub fn from_root_owned_file(path: &Path) -> Result<Self> {
        if !path.is_absolute() {
            return Err(Error::new("configuration path must be absolute"));
        }
        reject_symlink_components(path)?;
        let before = fs::symlink_metadata(path)?;
        validate_config_metadata(&before)?;
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
            .open(path)?;
        let opened = file.metadata()?;
        validate_config_metadata(&opened)?;
        if before.dev() != opened.dev() || before.ino() != opened.ino() {
            return Err(Error::new("configuration identity changed while opening"));
        }
        let mut bytes = Vec::new();
        file.by_ref()
            .take(CONFIG_MAX_BYTES.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > CONFIG_MAX_BYTES {
            return Err(Error::new("configuration exceeds immutable size bound"));
        }
        let after = fs::symlink_metadata(path)?;
        validate_config_metadata(&after)?;
        if before.dev() != after.dev()
            || before.ino() != after.ino()
            || before.len() != after.len()
            || opened.len() != after.len()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
        {
            return Err(Error::new("configuration changed while reading"));
        }
        let config = Self::from_json(&bytes)?;
        Ok(config)
    }

    /// Parse and validate a configuration that has already crossed a trusted
    /// ownership boundary. Production entry points use `from_root_owned_file`.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown fields, an invalid schema, an unexpected
    /// per-client Unix socket, a non-allowlisted upstream, or out-of-envelope
    /// limits.
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        let config: Self = serde_json::from_slice(bytes)?;
        config.validate()?;
        Ok(config)
    }

    /// Return the exact compile-time-allowlisted upstream URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the configured origin differs from the immutable
    /// public search endpoint.
    pub fn upstream_url(&self) -> Result<Url> {
        validate_upstream_origin(&self.upstream_origin)
    }

    /// Load the exact per-listener request key bound by immutable configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the credential path, identity, mode, length, or
    /// configured digest differs from the immutable contract.
    pub(crate) fn request_key(&self) -> Result<[u8; 32]> {
        load_exact_key(
            &self.request_key_path,
            &self.request_key_sha256,
            "listener request key",
        )
    }

    /// Load the broker-only response-signing key and verify its public identity.
    ///
    /// # Errors
    ///
    /// Returns an error if the private seed or derived public key differs from
    /// the immutable configuration.
    pub(crate) fn response_signing_key(&self) -> Result<SigningKey> {
        let seed = load_exact_key(
            &self.response_signing_key_path,
            &self.response_signing_key_sha256,
            "response signing key",
        )?;
        let signing = SigningKey::from_bytes(&seed);
        if format!("{:x}", Sha256::digest(signing.verifying_key().as_bytes()))
            != self.response_verify_key_sha256
        {
            return Err(Error::new(
                "broker response signing key does not match configured public identity",
            ));
        }
        Ok(signing)
    }

    fn validate(&self) -> Result<()> {
        if self.schema != CONFIG_SCHEMA {
            return Err(Error::new("configuration schema is not exact"));
        }
        let (expected_socket, expected_quota_path, hourly_limit, daily_limit) =
            match self.client_id.as_str() {
                crate::auth::RUNTIME_CLIENT_ID => (
                    Path::new(RUNTIME_SOCKET_PATH),
                    Path::new(RUNTIME_QUOTA_STATE_PATH),
                    RUNTIME_SEARCHES_PER_HOUR,
                    RUNTIME_SEARCHES_PER_UTC_DAY,
                ),
                crate::auth::STEWARD_CLIENT_ID => (
                    Path::new(STEWARD_SOCKET_PATH),
                    Path::new(STEWARD_QUOTA_STATE_PATH),
                    STEWARD_SEARCHES_PER_HOUR,
                    STEWARD_SEARCHES_PER_UTC_DAY,
                ),
                crate::auth::CORE_CLIENT_ID => (
                    Path::new(CORE_SOCKET_PATH),
                    Path::new(CORE_QUOTA_STATE_PATH),
                    CORE_SEARCHES_PER_HOUR,
                    CORE_SEARCHES_PER_UTC_DAY,
                ),
                _ => {
                    return Err(Error::new(
                        "broker listener client identity is not allowlisted",
                    ));
                },
            };
        if self.socket_path != expected_socket
            || self.quota_state_path != expected_quota_path
            || self.maximum_searches_per_hour != hourly_limit
            || self.maximum_searches_per_utc_day != daily_limit
            || self.expected_peer_uid == 0
            || self.socket_gid == 0
        {
            return Err(Error::new(
                "broker Unix listener identity escaped the immutable envelope",
            ));
        }
        validate_upstream_origin(&self.upstream_origin)?;
        let key_paths = [&self.request_key_path, &self.response_signing_key_path];
        let key_hashes = [
            &self.request_key_sha256,
            &self.response_signing_key_sha256,
            &self.response_verify_key_sha256,
        ];
        if key_paths.iter().any(|path| !path.is_absolute())
            || key_paths[0] == key_paths[1]
            || key_hashes.iter().any(|hash| !is_lower_hex64(hash))
            || self.request_key_sha256 == self.response_signing_key_sha256
        {
            return Err(Error::new(
                "broker request/signing credential configuration is invalid",
            ));
        }
        if !(100..=5_000).contains(&self.connect_timeout_ms)
            || !(500..=15_000).contains(&self.header_timeout_ms)
            || self.total_timeout_ms <= self.header_timeout_ms
            || self.total_timeout_ms > 30_000
            || !(100..=5_000).contains(&self.client_read_timeout_ms)
            || !(100..=5_000).contains(&self.client_write_timeout_ms)
            || !(256..=4_096).contains(&self.maximum_request_body_bytes)
            || !(64 * 1024..=1024 * 1024).contains(&self.maximum_upstream_body_bytes)
            || !(1..=5).contains(&self.maximum_results)
            || !(1..=8).contains(&self.maximum_concurrent_requests)
        {
            return Err(Error::new(
                "configuration exceeds the immutable resource envelope",
            ));
        }
        Ok(())
    }
}

fn load_exact_key(path: &Path, expected_hash: &str, label: &str) -> Result<[u8; 32]> {
    reject_symlink_components(path)?;
    let before = fs::symlink_metadata(path)?;
    validate_credential_metadata(&before)?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)?;
    let opened = file.metadata()?;
    validate_credential_metadata(&opened)?;
    let mut bytes = Vec::new();
    file.by_ref().take(33).read_to_end(&mut bytes)?;
    let after = fs::symlink_metadata(path)?;
    validate_credential_metadata(&after)?;
    if before.dev() != opened.dev()
        || before.ino() != opened.ino()
        || before.len() != opened.len()
        || opened.dev() != after.dev()
        || opened.ino() != after.ino()
        || opened.len() != after.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        return Err(Error::new(format!("broker {label} changed while reading")));
    }
    let key: [u8; 32] = bytes
        .try_into()
        .map_err(|_| Error::new(format!("broker {label} must contain 32 bytes")))?;
    if format!("{:x}", Sha256::digest(key)) != expected_hash {
        return Err(Error::new(format!("broker {label} digest mismatch")));
    }
    Ok(key)
}

fn is_lower_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn validate_upstream_origin(value: &str) -> Result<Url> {
    let url = Url::parse(value).map_err(|_| Error::new("upstream origin is invalid"))?;
    if url.scheme() != "https"
        || url.host_str() != Some(ALLOWED_UPSTREAM_HOST)
        || url.port_or_known_default() != Some(443)
        || url.path() != ALLOWED_UPSTREAM_PATH
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(Error::new(
            "upstream origin is not the compiled HTTPS search allowlist",
        ));
    }
    Ok(url)
}

fn reject_symlink_components(path: &Path) -> Result<()> {
    let mut cursor = PathBuf::new();
    let components = path.components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        cursor.push(component);
        if cursor == Path::new("/") {
            continue;
        }
        let metadata = fs::symlink_metadata(&cursor)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::new("configuration path contains a symlink"));
        }
        if index.saturating_add(1) < components.len()
            && (!metadata.is_dir()
                || metadata.uid() != 0
                || metadata.permissions().mode() & 0o022 != 0)
        {
            return Err(Error::new(
                "configuration ancestors must be root-owned non-writable directories",
            ));
        }
    }
    Ok(())
}

fn validate_config_metadata(metadata: &fs::Metadata) -> Result<()> {
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o7777 != 0o440
        || metadata.len() > CONFIG_MAX_BYTES
    {
        return Err(Error::new(
            "configuration must be root-owned regular nlink-one mode 0440",
        ));
    }
    Ok(())
}

fn validate_credential_metadata(metadata: &fs::Metadata) -> Result<()> {
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.permissions().mode() & 0o7777 != 0o400
        || metadata.len() != 32
    {
        return Err(Error::new(
            "broker client credential must be regular, nlink-one, mode 0400, and exactly 32 bytes",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CONFIG_SCHEMA, Config};

    fn valid() -> serde_json::Value {
        serde_json::json!({
            "schema": CONFIG_SCHEMA,
            "client_id": "edge-runtime",
            "socket_path": "/run/astrid-edge-self-change/web-runtime.sock",
            "expected_peer_uid": 1001,
            "socket_gid": 1003,
            "upstream_origin": "https://search.brave.com/search",
            "connect_timeout_ms": 2_000,
            "header_timeout_ms": 8_000,
            "total_timeout_ms": 20_000,
            "client_read_timeout_ms": 2_000,
            "client_write_timeout_ms": 2_000,
            "maximum_request_body_bytes": 4_096,
            "maximum_upstream_body_bytes": 1_048_576,
            "maximum_results": 5,
            "maximum_concurrent_requests": 4,
            "maximum_searches_per_hour": 8,
            "maximum_searches_per_utc_day": 24,
            "quota_state_path": "/var/lib/astrid-edge-web-runtime/search-quota.jsonl",
            "request_key_path": "/run/credentials/astrid-edge-web-broker-runtime.service/request.key",
            "request_key_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "response_signing_key_path": "/run/credentials/astrid-edge-web-broker-runtime.service/response-signing.key",
            "response_signing_key_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "response_verify_key_sha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
        })
    }

    #[test]
    fn exact_configuration_accepts_only_fixed_socket_and_upstream() {
        let value = valid();
        let config = Config::from_json(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(
            config.socket_path,
            std::path::Path::new("/run/astrid-edge-self-change/web-runtime.sock")
        );
        assert_eq!(
            config.upstream_url().unwrap().host_str(),
            Some("search.brave.com")
        );
    }

    #[test]
    fn steward_configuration_is_bound_to_its_own_socket_and_peer() {
        let mut value = valid();
        value["client_id"] = serde_json::json!("edge-steward");
        value["socket_path"] = serde_json::json!("/run/astrid-edge-self-change/web-steward.sock");
        value["expected_peer_uid"] = serde_json::json!(1002);
        value["socket_gid"] = serde_json::json!(1004);
        value["request_key_path"] = serde_json::json!(
            "/run/credentials/astrid-edge-web-broker-steward.service/request.key"
        );
        value["maximum_searches_per_hour"] = serde_json::json!(2);
        value["maximum_searches_per_utc_day"] = serde_json::json!(12);
        value["quota_state_path"] =
            serde_json::json!("/var/lib/astrid-edge-web-steward/search-quota.jsonl");
        let config = Config::from_json(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(config.client_id, "edge-steward");
        assert_eq!(config.expected_peer_uid, 1002);

        value["socket_path"] = serde_json::json!("/run/astrid-edge-self-change/web-runtime.sock");
        assert!(Config::from_json(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn core_configuration_has_a_distinct_socket_key_and_quota_ledger() {
        let mut value = valid();
        value["client_id"] = serde_json::json!("edge-core");
        value["socket_path"] = serde_json::json!("/run/astrid-edge-self-change/web-core.sock");
        value["expected_peer_uid"] = serde_json::json!(1001);
        value["socket_gid"] = serde_json::json!(1005);
        value["request_key_path"] =
            serde_json::json!("/run/credentials/astrid-edge-web-broker-core.service/request.key");
        value["quota_state_path"] =
            serde_json::json!("/var/lib/astrid-edge-web-core/search-quota.jsonl");
        let config = Config::from_json(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(config.client_id, "edge-core");

        value["socket_path"] = serde_json::json!("/run/astrid-edge-self-change/web-runtime.sock");
        assert!(Config::from_json(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn unknown_fields_and_origin_substitution_are_rejected() {
        let mut value = valid();
        value["model_supplied_url"] = serde_json::json!("https://example.com/");
        assert!(Config::from_json(&serde_json::to_vec(&value).unwrap()).is_err());

        let mut value = valid();
        value["upstream_origin"] = serde_json::json!("https://search.brave.com.evil.test/search");
        assert!(Config::from_json(&serde_json::to_vec(&value).unwrap()).is_err());

        let mut value = valid();
        value["socket_path"] = serde_json::json!("/tmp/web-runtime.sock");
        assert!(Config::from_json(&serde_json::to_vec(&value).unwrap()).is_err());

        let mut value = valid();
        value["client_id"] = serde_json::json!("edge-steward");
        assert!(Config::from_json(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn limits_cannot_expand_past_immutable_envelope() {
        for (field, value) in [
            ("maximum_results", serde_json::json!(6)),
            ("maximum_request_body_bytes", serde_json::json!(4_097)),
            ("maximum_upstream_body_bytes", serde_json::json!(1_048_577)),
            ("maximum_concurrent_requests", serde_json::json!(9)),
            ("maximum_searches_per_hour", serde_json::json!(9)),
            ("maximum_searches_per_utc_day", serde_json::json!(25)),
        ] {
            let mut config = valid();
            config[field] = value;
            assert!(Config::from_json(&serde_json::to_vec(&config).unwrap()).is_err());
        }
    }
}
