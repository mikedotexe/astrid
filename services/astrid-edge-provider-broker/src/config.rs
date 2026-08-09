use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use crate::auth::{RUNTIME_CLIENT, STEWARD_CLIENT, WARMUP_CLIENT, key_from_bytes};
use crate::{CONFIG_SCHEMA, Error, Result};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    pub appliance_id: String,
    pub ollama_origin: String,
    pub model: String,
    pub keep_alive: String,
    pub context_tokens: u32,
    pub maximum_output_tokens: u32,
    pub maximum_request_body_bytes: u32,
    pub maximum_response_body_bytes: u64,
    pub connect_timeout_ms: u64,
    pub header_timeout_ms: u64,
    pub inter_chunk_timeout_ms: u64,
    pub total_timeout_ms: u64,
    pub client_read_timeout_ms: u64,
    pub client_write_timeout_ms: u64,
    pub maximum_concurrent_requests: u8,
    pub model_lock: PathBuf,
    pub maintenance_lease: PathBuf,
    pub reflection_lease: PathBuf,
    pub ledger_path: PathBuf,
    pub runtime: ClientConfig,
    pub steward: ClientConfig,
    pub warmup: ClientConfig,
    pub ledger_key_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientConfig {
    pub client_id: String,
    pub expected_peer_uid: u32,
    pub socket_path: PathBuf,
    pub socket_gid: u32,
    pub request_key_sha256: String,
    pub maximum_requests_per_hour: u16,
    pub maximum_output_tokens: u32,
}

impl Config {
    /// Load and validate an immutable broker configuration.
    ///
    /// # Errors
    ///
    /// Returns an error when the file identity, schema, endpoint, model, client, or resource
    /// policy is not exact.
    pub fn from_file(path: &Path) -> Result<Self> {
        validate_absolute(path, "configuration")?;
        let metadata = fs::symlink_metadata(path)?;
        let production = cfg!(not(debug_assertions));
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.nlink() != 1
            || (production && metadata.uid() != 0)
            || metadata.permissions().mode() & 0o022 != 0
            || metadata.len() == 0
            || metadata.len() > 64 * 1024
        {
            return Err(Error::new("provider broker configuration is not immutable"));
        }
        let body = fs::read(path)?;
        if body.len() != usize::try_from(metadata.len()).unwrap_or(usize::MAX) {
            return Err(Error::new(
                "provider broker configuration changed while read",
            ));
        }
        Self::from_json(&body)
    }

    /// Decode and validate a broker configuration without reading its credentials.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown fields or an invalid immutable policy.
    pub fn from_json(body: &[u8]) -> Result<Self> {
        let value: Self = serde_json::from_slice(body)?;
        value.validate()?;
        Ok(value)
    }

    pub(crate) fn client(&self, client_id: &str) -> Result<&ClientConfig> {
        match client_id {
            RUNTIME_CLIENT => Ok(&self.runtime),
            STEWARD_CLIENT => Ok(&self.steward),
            WARMUP_CLIENT => Ok(&self.warmup),
            _ => Err(Error::new("provider client is not configured")),
        }
    }

    pub(crate) fn request_key(
        &self,
        client_id: &str,
        credential_directory: &Path,
    ) -> Result<[u8; 32]> {
        let client = self.client(client_id)?;
        validate_credential_directory(client_id, credential_directory)?;
        let bytes = read_key(
            &credential_directory.join("request.key"),
            &client.request_key_sha256,
        )?;
        key_from_bytes(&bytes)
    }

    pub(crate) fn ledger_key(
        &self,
        client_id: &str,
        credential_directory: &Path,
    ) -> Result<[u8; 32]> {
        validate_credential_directory(client_id, credential_directory)?;
        let bytes = read_key(
            &credential_directory.join("ledger.key"),
            &self.ledger_key_sha256,
        )?;
        key_from_bytes(&bytes)
    }

    pub(crate) fn warmup_client_key(&self, path: &Path) -> Result<[u8; 32]> {
        validate_absolute(path, "warmup provider credential")?;
        if cfg!(not(debug_assertions))
            && path
                != Path::new("/run/credentials/astrid-model-warmup.service/provider-request.key")
        {
            return Err(Error::new(
                "warmup provider credential escaped the immutable service",
            ));
        }
        key_from_bytes(&read_key(path, &self.warmup.request_key_sha256)?)
    }

    fn validate(&self) -> Result<()> {
        if self.schema != CONFIG_SCHEMA {
            return Err(Error::new(
                "provider broker configuration schema is unsupported",
            ));
        }
        if !safe_identifier(&self.appliance_id)
            || !safe_model(&self.model)
            || !matches!(self.keep_alive.as_str(), "2h" | "120m")
        {
            return Err(Error::new("provider identity or keep-alive is invalid"));
        }
        let (host, port) = parse_loopback_origin(&self.ollama_origin)?;
        if host != "127.0.0.1" || port == 0 {
            return Err(Error::new("provider upstream must be exact IPv4 loopback"));
        }
        if !(512..=32_768).contains(&self.context_tokens)
            || !(32..=1_024).contains(&self.maximum_output_tokens)
            || !(4_096..=262_144).contains(&self.maximum_request_body_bytes)
            || !(1_048_576..=67_108_864).contains(&self.maximum_response_body_bytes)
            || !(100..=30_000).contains(&self.connect_timeout_ms)
            || !(60_000..=420_000).contains(&self.header_timeout_ms)
            || !(1_000..=120_000).contains(&self.inter_chunk_timeout_ms)
            || self.total_timeout_ms < self.header_timeout_ms
            || self.total_timeout_ms > 660_000
            || !(100..=30_000).contains(&self.client_read_timeout_ms)
            || !(1_000..=120_000).contains(&self.client_write_timeout_ms)
            || self.maximum_concurrent_requests != 1
        {
            return Err(Error::new("provider resource bounds are invalid"));
        }
        for path in [
            &self.model_lock,
            &self.maintenance_lease,
            &self.reflection_lease,
            &self.ledger_path,
            &self.runtime.socket_path,
            &self.steward.socket_path,
            &self.warmup.socket_path,
        ] {
            validate_absolute(path, "provider path")?;
        }
        let expected = [
            (
                &self.runtime,
                RUNTIME_CLIENT,
                "/run/astrid-edge-self-change/provider-runtime.sock",
            ),
            (
                &self.steward,
                STEWARD_CLIENT,
                "/run/astrid-edge-self-change/provider-steward.sock",
            ),
            (
                &self.warmup,
                WARMUP_CLIENT,
                "/run/astrid-edge-self-change/provider-warmup.sock",
            ),
        ];
        let mut uids = BTreeSet::new();
        let mut hashes = BTreeSet::new();
        for (client, identifier, socket_path) in expected {
            if client.client_id != identifier
                || client.expected_peer_uid == 0
                || client.socket_gid == 0
                || client.socket_path != Path::new(socket_path)
                || !(1..=256).contains(&client.maximum_requests_per_hour)
                || client.maximum_output_tokens == 0
                || client.maximum_output_tokens > self.maximum_output_tokens
                || !canonical_hex64(&client.request_key_sha256)
                || !uids.insert(client.expected_peer_uid)
                || !hashes.insert(client.request_key_sha256.as_str())
            {
                return Err(Error::new(
                    "provider client identity, key, or quota is invalid",
                ));
            }
        }
        if !canonical_hex64(&self.ledger_key_sha256)
            || hashes.contains(self.ledger_key_sha256.as_str())
        {
            return Err(Error::new("provider ledger key identity is invalid"));
        }
        Ok(())
    }
}

pub fn parse_loopback_origin(value: &str) -> Result<(String, u16)> {
    let rest = value
        .strip_prefix("http://")
        .ok_or_else(|| Error::new("provider origin must use HTTP"))?;
    if rest.contains(['/', '?', '#', '@']) {
        return Err(Error::new(
            "provider origin contains unsupported components",
        ));
    }
    let (host, port) = rest
        .rsplit_once(':')
        .ok_or_else(|| Error::new("provider origin omits an explicit port"))?;
    let port = port
        .parse::<u16>()
        .map_err(|_| Error::new("provider origin port is invalid"))?;
    Ok((host.to_owned(), port))
}

fn read_key(path: &Path, expected_hash: &str) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.len() != 32
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(Error::new("provider credential identity is invalid"));
    }
    let body = fs::read(path)?;
    if format!("{:x}", Sha256::digest(&body)) != expected_hash {
        return Err(Error::new("provider credential digest mismatch"));
    }
    Ok(body)
}

fn validate_credential_directory(client_id: &str, path: &Path) -> Result<()> {
    validate_absolute(path, "provider credential directory")?;
    if cfg!(not(debug_assertions)) {
        let expected = match client_id {
            RUNTIME_CLIENT => "/run/credentials/astrid-edge-provider-broker@edge-runtime.service",
            STEWARD_CLIENT => "/run/credentials/astrid-edge-provider-broker@edge-steward.service",
            WARMUP_CLIENT => "/run/credentials/astrid-edge-provider-broker@model-warmup.service",
            _ => return Err(Error::new("provider credential client is not allowlisted")),
        };
        if path != Path::new(expected) {
            return Err(Error::new(
                "provider credential directory escaped the immutable service",
            ));
        }
    }
    Ok(())
}

fn validate_absolute(path: &Path, label: &str) -> Result<()> {
    if !path.is_absolute()
        || path == Path::new("/")
        || path
            .components()
            .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
    {
        return Err(Error::new(format!("{label} is not an exact absolute path")));
    }
    Ok(())
}

fn safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn safe_model(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'/')
        })
}

fn canonical_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::Config;

    fn body(origin: &str) -> Vec<u8> {
        format!(
            r#"{{"schema":"astrid.edge.provider_broker.config.v1","appliance_id":"avado","ollama_origin":"{origin}","model":"qwen3.5:4b","keep_alive":"2h","context_tokens":4096,"maximum_output_tokens":512,"maximum_request_body_bytes":131072,"maximum_response_body_bytes":8388608,"connect_timeout_ms":30000,"header_timeout_ms":300000,"inter_chunk_timeout_ms":120000,"total_timeout_ms":660000,"client_read_timeout_ms":5000,"client_write_timeout_ms":120000,"maximum_concurrent_requests":1,"model_lock":"/run/astrid-edge-self-change/model.lock","maintenance_lease":"/run/astrid-edge-self-change/maintenance.json","reflection_lease":"/run/astrid-edge-self-change/reflection.json","ledger_path":"/var/lib/astrid-edge-provider/receipts.jsonl","runtime":{{"client_id":"edge-runtime","expected_peer_uid":1000,"socket_path":"/run/astrid-edge-self-change/provider-runtime.sock","socket_gid":1004,"request_key_sha256":"{}","maximum_requests_per_hour":48,"maximum_output_tokens":192}},"steward":{{"client_id":"edge-steward","expected_peer_uid":1001,"socket_path":"/run/astrid-edge-self-change/provider-steward.sock","socket_gid":1005,"request_key_sha256":"{}","maximum_requests_per_hour":4,"maximum_output_tokens":512}},"warmup":{{"client_id":"model-warmup","expected_peer_uid":1002,"socket_path":"/run/astrid-edge-self-change/provider-warmup.sock","socket_gid":1006,"request_key_sha256":"{}","maximum_requests_per_hour":12,"maximum_output_tokens":2}},"ledger_key_sha256":"{}"}}"#,
            "a".repeat(64),
            "b".repeat(64),
            "c".repeat(64),
            "d".repeat(64),
        )
        .into_bytes()
    }

    #[test]
    fn accepts_exact_loopback_and_rejects_dns_or_other_loopback_shapes() {
        assert!(Config::from_json(&body("http://127.0.0.1:11434")).is_ok());
        for origin in [
            "http://localhost:11434",
            "http://127.0.0.53:53",
            "http://127.0.0.1:11434/api/delete",
            "https://127.0.0.1:11434",
            "http://10.0.0.1:11434",
        ] {
            assert!(
                Config::from_json(&body(origin)).is_err(),
                "accepted {origin}"
            );
        }
    }

    #[test]
    fn rejects_a_client_output_ceiling_above_the_global_envelope() {
        let mut config = Config::from_json(&body("http://127.0.0.1:11434")).unwrap();
        config.runtime.maximum_output_tokens = config.maximum_output_tokens.saturating_add(1);
        assert!(config.validate().is_err());
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use std::fs;

    use tempfile::TempDir;

    use super::Config;

    pub fn config_for_protocol_tests() -> (Config, TempDir) {
        let temporary = tempfile::tempdir().unwrap();
        for (path, byte) in [
            (temporary.path().join("request.key"), 1_u8),
            (temporary.path().join("ledger.key"), 4_u8),
        ] {
            fs::write(&path, vec![byte; 32]).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                fs::set_permissions(path, fs::Permissions::from_mode(0o400)).unwrap();
            }
        }
        let hashes = (1_u8..=4)
            .map(|byte| {
                use sha2::{Digest as _, Sha256};
                format!("{:x}", Sha256::digest([byte; 32]))
            })
            .collect::<Vec<_>>();
        let body = format!(
            r#"{{"schema":"astrid.edge.provider_broker.config.v1","appliance_id":"avado","ollama_origin":"http://127.0.0.1:11434","model":"qwen3.5:4b","keep_alive":"2h","context_tokens":4096,"maximum_output_tokens":512,"maximum_request_body_bytes":131072,"maximum_response_body_bytes":8388608,"connect_timeout_ms":30000,"header_timeout_ms":300000,"inter_chunk_timeout_ms":120000,"total_timeout_ms":660000,"client_read_timeout_ms":5000,"client_write_timeout_ms":120000,"maximum_concurrent_requests":1,"model_lock":"/run/astrid-edge-self-change/model.lock","maintenance_lease":"/run/astrid-edge-self-change/maintenance.json","reflection_lease":"/run/astrid-edge-self-change/reflection.json","ledger_path":"/var/lib/astrid-edge-provider/receipts.jsonl","runtime":{{"client_id":"edge-runtime","expected_peer_uid":1000,"socket_path":"/run/astrid-edge-self-change/provider-runtime.sock","socket_gid":1004,"request_key_sha256":"{}","maximum_requests_per_hour":48,"maximum_output_tokens":192}},"steward":{{"client_id":"edge-steward","expected_peer_uid":1001,"socket_path":"/run/astrid-edge-self-change/provider-steward.sock","socket_gid":1005,"request_key_sha256":"{}","maximum_requests_per_hour":4,"maximum_output_tokens":512}},"warmup":{{"client_id":"model-warmup","expected_peer_uid":1002,"socket_path":"/run/astrid-edge-self-change/provider-warmup.sock","socket_gid":1006,"request_key_sha256":"{}","maximum_requests_per_hour":12,"maximum_output_tokens":2}},"ledger_key_sha256":"{}"}}"#,
            hashes[0], hashes[1], hashes[2], hashes[3],
        );
        let mut config = Config::from_json(body.as_bytes()).unwrap();
        config.model_lock = temporary.path().join("model.lock");
        config.maintenance_lease = temporary.path().join("maintenance.json");
        config.reflection_lease = temporary.path().join("reflection.json");
        fs::write(&config.model_lock, b"immutable-provider-lock-v1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&config.model_lock, fs::Permissions::from_mode(0o640)).unwrap();
        }
        (config, temporary)
    }
}
