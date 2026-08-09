use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::util::{
    MAX_JSON_BYTES, read_stable_regular, require_absolute_no_symlink, sha256, validate_hex64,
    validate_identifier,
};
use crate::{CONFIG_SCHEMA, Error, Result};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    pub appliance_id: String,
    pub target: String,
    pub model: String,
    pub ollama_origin: String,
    pub connect_timeout_ms: u64,
    pub header_timeout_ms: u64,
    pub total_timeout_ms: u64,
    pub provider_broker: Option<ProviderBrokerConfig>,
    pub web_broker: Option<WebBrokerConfig>,
    pub context_tokens: u32,
    pub output_tokens: u32,
    pub source_authoring_output_tokens: u32,
    pub model_lock: PathBuf,
    pub workspace_root: PathBuf,
    pub workspace_uid: u32,
    pub workspace_gid: u32,
    pub source_root: PathBuf,
    pub source_manifest: PathBuf,
    pub source_manifest_sha256: String,
    pub source_signature: PathBuf,
    pub expected_source_id: String,
    pub source_signing_key: PathBuf,
    pub source_signing_key_sha256: String,
    pub attestor_key: PathBuf,
    pub attestor_key_sha256: String,
    pub state_root: PathBuf,
    pub supervisor_inbox: PathBuf,
    pub supervisor_status: PathBuf,
    pub current_generation: PathBuf,
    pub active_generation_link: PathBuf,
    pub maintenance_lease: PathBuf,
    pub patch_export_root: PathBuf,
    pub owned_inputs: Vec<OwnedInput>,
    pub gates: GateConfig,
}

pub const REQUIRED_OWNED_INPUTS: [(&str, &str); 5] = [
    ("continuity", "autonomous/thread_state.json"),
    ("self_profile", "self/profile.json"),
    ("verified_evidence", "autonomous/thread_state.jsonl"),
    ("machine_observation", "perception/latest.json"),
    ("spectral_host_state", "runtime/spectral_state.json"),
];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedInput {
    pub kind: String,
    pub path: PathBuf,
    pub maximum_files: u16,
    pub maximum_bytes_per_file: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateConfig {
    pub autonomy_state: PathBuf,
    pub action_receipts: PathBuf,
    pub thermal_celsius: PathBuf,
    pub maximum_thermal_celsius: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebBrokerConfig {
    pub socket_path: PathBuf,
    pub request_key_path: PathBuf,
    pub request_key_sha256: String,
    pub response_verify_key_path: PathBuf,
    pub response_verify_key_sha256: String,
    pub connect_timeout_ms: u64,
    pub header_timeout_ms: u64,
    pub total_timeout_ms: u64,
    pub result_limit: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderBrokerConfig {
    pub socket_path: PathBuf,
    pub request_key_path: PathBuf,
    pub request_key_sha256: String,
}

impl Config {
    /// Load an exact root-owned, non-writable immutable helper configuration.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe ownership, paths, credentials, provider policy, or schema.
    pub fn from_root_owned_file(path: &Path) -> Result<Self> {
        Self::from_root_owned_file_with_credentials(path, None)
    }

    /// Load configuration and replace trust-key paths with exact systemd credentials.
    ///
    /// The credential directory must contain only the requested regular files named
    /// `source.key`, `intent.key`, the bounded non-secret `supervisor-status`,
    /// and, when configured, `web-broker-request.key` plus
    /// `web-broker-response.pub`; key hashes remain pinned by the root config.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe config or credential ownership, names, links, size,
    /// content identity, provider policy, paths, or schema.
    #[allow(clippy::too_many_lines)] // Credential projection and identity checks are one trust gate.
    pub fn from_root_owned_file_with_credentials(
        path: &Path,
        credential_directory: Option<&Path>,
    ) -> Result<Self> {
        require_absolute_no_symlink(path, "configuration")?;
        require_root_controlled(path, false, "configuration")?;
        let bytes = read_stable_regular(path, MAX_JSON_BYTES)?;
        let mut config: Self = serde_json::from_slice(&bytes)?;
        config.validate()?;
        require_root_controlled(&config.source_root, true, "signed source root")?;
        require_root_controlled(&config.source_manifest, false, "source manifest")?;
        require_root_controlled(&config.source_signature, false, "source signature")?;
        require_root_controlled(
            &config.supervisor_status,
            false,
            "supervisor status projection",
        )?;
        require_root_controlled(
            &config.current_generation,
            false,
            "current generation binding",
        )?;
        if sha256(&read_stable_regular(
            &config.source_manifest,
            MAX_JSON_BYTES,
        )?) != config.source_manifest_sha256
        {
            return Err(Error::new(
                "source manifest does not match configured identity",
            ));
        }
        if let Some(directory) = credential_directory {
            require_absolute_no_symlink(directory, "credential directory")?;
            let metadata = fs::symlink_metadata(directory)?;
            let effective_uid = current_effective_uid()?;
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || ![0, effective_uid].contains(&std::os::unix::fs::MetadataExt::uid(&metadata))
                || std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o222 != 0
            {
                return Err(Error::new(
                    "credential directory must be root/unit-owned and non-writable",
                ));
            }
            let names = fs::read_dir(directory)?
                .map(|entry| entry.map(|value| value.file_name()).map_err(Error::from))
                .collect::<Result<BTreeSet<_>>>()?;
            let mut expected_names = BTreeSet::from([
                std::ffi::OsString::from("intent.key"),
                std::ffi::OsString::from("source.key"),
                std::ffi::OsString::from("supervisor-status"),
            ]);
            if config.web_broker.is_some() {
                expected_names.insert(std::ffi::OsString::from("web-broker-request.key"));
                expected_names.insert(std::ffi::OsString::from("web-broker-response.pub"));
            }
            if config.provider_broker.is_some() {
                expected_names.insert(std::ffi::OsString::from("provider-request.key"));
            }
            if names != expected_names {
                return Err(Error::new("credential directory has unexpected names"));
            }
            config.source_signing_key = directory.join("source.key");
            config.attestor_key = directory.join("intent.key");
            config.supervisor_status = directory.join("supervisor-status");
            if let Some(web) = &mut config.web_broker {
                web.request_key_path = directory.join("web-broker-request.key");
                web.response_verify_key_path = directory.join("web-broker-response.pub");
            }
            if let Some(provider) = &mut config.provider_broker {
                provider.request_key_path = directory.join("provider-request.key");
            }
            for path in [&config.source_signing_key, &config.attestor_key] {
                require_credential_file(path, &[0, effective_uid])?;
            }
            if let Some(web) = &config.web_broker {
                require_broker_credential(&web.request_key_path, &[0, effective_uid])?;
                require_broker_credential(&web.response_verify_key_path, &[0, effective_uid])?;
            }
            if let Some(provider) = &config.provider_broker {
                require_broker_credential(&provider.request_key_path, &[0, effective_uid])?;
            }
            require_status_snapshot(&config.supervisor_status, &[0, effective_uid])?;
        } else {
            for path in [&config.source_signing_key, &config.attestor_key] {
                require_credential_file(path, &[0])?;
            }
            if let Some(web) = &config.web_broker {
                require_broker_credential(&web.request_key_path, &[0])?;
                require_broker_credential(&web.response_verify_key_path, &[0])?;
            }
            if let Some(provider) = &config.provider_broker {
                require_broker_credential(&provider.request_key_path, &[0])?;
            }
        }
        if config.source_signing_key == config.attestor_key
            || read_stable_regular(&config.source_signing_key, 32)?
                == read_stable_regular(&config.attestor_key, 32)?
        {
            return Err(Error::new(
                "source signing and intent attestation credentials must be separate",
            ));
        }
        let source_hash = sha256(&read_stable_regular(&config.source_signing_key, 32)?);
        let intent_hash = sha256(&read_stable_regular(&config.attestor_key, 32)?);
        if source_hash != config.source_signing_key_sha256
            || intent_hash != config.attestor_key_sha256
        {
            return Err(Error::new(
                "credential content does not match root-configured identity",
            ));
        }
        if let Some(web) = &config.web_broker {
            let request_hash = sha256(&read_stable_regular(&web.request_key_path, 32)?);
            let verify_hash = sha256(&read_stable_regular(&web.response_verify_key_path, 32)?);
            if request_hash != web.request_key_sha256
                || verify_hash != web.response_verify_key_sha256
            {
                return Err(Error::new(
                    "web broker credentials do not match root-configured identities",
                ));
            }
        }
        if let Some(provider) = &config.provider_broker {
            let request_hash = sha256(&read_stable_regular(&provider.request_key_path, 32)?);
            if request_hash != provider.request_key_sha256 {
                return Err(Error::new(
                    "provider broker credential does not match root-configured identity",
                ));
            }
        }
        Ok(config)
    }

    /// Validate the immutable authority and resource envelope without reading credentials.
    ///
    /// # Errors
    ///
    /// Returns an error when any configured authority, bound, or path is unsafe.
    #[allow(clippy::too_many_lines)] // One auditable immutable configuration envelope.
    pub fn validate(&self) -> Result<()> {
        if self.schema != CONFIG_SCHEMA {
            return Err(Error::new(
                "unsupported steward helper configuration schema",
            ));
        }
        validate_identifier(&self.appliance_id, "appliance_id")?;
        if !matches!(
            self.target.as_str(),
            "x86_64-unknown-linux-gnu" | "aarch64-unknown-linux-gnu"
        ) {
            return Err(Error::new("unsupported CPU-edge target"));
        }
        validate_hex64(&self.source_signing_key_sha256, "source_signing_key_sha256")?;
        validate_hex64(&self.attestor_key_sha256, "attestor_key_sha256")?;
        validate_hex64(&self.source_manifest_sha256, "source_manifest_sha256")?;
        let expected_source_hash = self
            .expected_source_id
            .strip_prefix("cpu-edge:")
            .ok_or_else(|| Error::new("expected_source_id has unsupported form"))?;
        validate_hex64(expected_source_hash, "expected_source_id")?;
        if self.source_signing_key_sha256 == self.attestor_key_sha256 {
            return Err(Error::new("trust-key identities must be different"));
        }
        if self.model.is_empty()
            || self.model.len() > 128
            || self.model.chars().any(char::is_whitespace)
        {
            return Err(Error::new("invalid model identifier"));
        }
        validate_loopback_origin(&self.ollama_origin)?;
        if let Some(provider) = &self.provider_broker {
            validate_hex64(
                &provider.request_key_sha256,
                "provider broker request key hash",
            )?;
            if provider.socket_path
                != Path::new("/run/astrid-edge-self-change/provider-steward.sock")
                || !provider.request_key_path.is_absolute()
            {
                return Err(Error::new(
                    "provider broker exceeds its immutable Unix-socket envelope",
                ));
            }
        }
        if let Some(web) = &self.web_broker {
            validate_hex64(&web.request_key_sha256, "web broker request key hash")?;
            validate_hex64(
                &web.response_verify_key_sha256,
                "web broker response verify key hash",
            )?;
            if web.socket_path != Path::new("/run/astrid-edge-self-change/web-steward.sock")
                || !web.request_key_path.is_absolute()
                || !web.response_verify_key_path.is_absolute()
                || web.request_key_path == web.response_verify_key_path
                || web.request_key_sha256 == web.response_verify_key_sha256
                || !(100..=5_000).contains(&web.connect_timeout_ms)
                || !(500..=30_000).contains(&web.header_timeout_ms)
                || web.total_timeout_ms <= web.header_timeout_ms
                || web.total_timeout_ms > 60_000
                || !(1..=5).contains(&web.result_limit)
            {
                return Err(Error::new("web broker exceeds its Unix-socket envelope"));
            }
        }
        if !(100..=30_000).contains(&self.connect_timeout_ms)
            || !(1_000..=600_000).contains(&self.header_timeout_ms)
            || self.total_timeout_ms <= self.header_timeout_ms
            // Eight direct completions are shared across both model lanes. The
            // 660-second per-call ceiling keeps their combined provider wall
            // time below the immutable 2h10m service deadline with margin for
            // tools and durable finalization.
            || self.total_timeout_ms > 660_000
            || !(1_024..=8_192).contains(&self.context_tokens)
            || !(64..=512).contains(&self.output_tokens)
            || !(64..=512).contains(&self.source_authoring_output_tokens)
        {
            return Err(Error::new(
                "provider limits are outside the immutable envelope",
            ));
        }
        let absolute_paths = [
            &self.model_lock,
            &self.workspace_root,
            &self.source_root,
            &self.source_manifest,
            &self.source_signature,
            &self.source_signing_key,
            &self.attestor_key,
            &self.state_root,
            &self.supervisor_inbox,
            &self.supervisor_status,
            &self.current_generation,
            &self.patch_export_root,
            &self.gates.autonomy_state,
            &self.gates.action_receipts,
            &self.gates.thermal_celsius,
        ];
        for path in absolute_paths {
            require_absolute_no_symlink(path, "configured path")?;
        }
        if !self.active_generation_link.is_absolute()
            || self
                .active_generation_link
                .file_name()
                .and_then(|name| name.to_str())
                != Some("current")
        {
            return Err(Error::new(
                "active generation link must be the absolute current pointer",
            ));
        }
        let release_parent = self
            .active_generation_link
            .parent()
            .ok_or_else(|| Error::new("active generation link has no parent"))?;
        require_absolute_no_symlink(release_parent, "active generation parent")?;
        require_absolute_no_symlink(
            &release_parent.join("releases"),
            "active generation releases root",
        )?;
        if self.state_root == self.supervisor_inbox
            || self.state_root.starts_with(&self.source_root)
            || self.source_root.starts_with(&self.state_root)
            || self.attestor_key.starts_with(&self.state_root)
            || self.source_signing_key.starts_with(&self.state_root)
            || self.supervisor_status.starts_with(&self.state_root)
            || self.active_generation_link.starts_with(&self.state_root)
            || self.active_generation_link.starts_with(&self.source_root)
        {
            return Err(Error::new(
                "authority, source, and mutable roots must be separate",
            ));
        }
        if self.maintenance_lease
            != self
                .current_generation
                .parent()
                .ok_or_else(|| Error::new("current generation binding has no parent"))?
                .join("maintenance.json")
        {
            return Err(Error::new(
                "maintenance lease must be the exact supervisor-state sibling",
            ));
        }
        if self.workspace_uid == 0 || self.workspace_gid == 0 {
            return Err(Error::new(
                "workspace owner must be an unprivileged identity",
            ));
        }
        if self.owned_inputs.len() != REQUIRED_OWNED_INPUTS.len() {
            return Err(Error::new(
                "owned_inputs must contain the exact five scheduled-introspection inputs",
            ));
        }
        let mut kinds = BTreeSet::new();
        for input in &self.owned_inputs {
            validate_identifier(&input.kind, "owned input kind")?;
            if !kinds.insert(&input.kind)
                || !input.path.starts_with(&self.workspace_root)
                || !(1..=128).contains(&input.maximum_files)
                || !(1_024..=65_536).contains(&input.maximum_bytes_per_file)
                || input.path.starts_with(&self.patch_export_root)
                || self.patch_export_root.starts_with(&input.path)
            {
                return Err(Error::new("invalid or duplicate owned input"));
            }
            require_absolute_no_symlink(&input.path, "owned input")?;
        }
        let expected = REQUIRED_OWNED_INPUTS
            .iter()
            .map(|(kind, relative)| ((*kind).to_owned(), self.workspace_root.join(relative)))
            .collect::<BTreeSet<_>>();
        let actual = self
            .owned_inputs
            .iter()
            .map(|input| (input.kind.clone(), input.path.clone()))
            .collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(Error::new(
                "owned_inputs do not match the exact canonical introspection contract",
            ));
        }
        if self.patch_export_root != self.workspace_root.join("self-change/patch-outbox") {
            return Err(Error::new(
                "patch export root must be the exact dedicated workspace directory",
            ));
        }
        if self.gates.maximum_thermal_celsius < 50
            || self.gates.maximum_thermal_celsius > 95
            || ![&self.gates.autonomy_state, &self.gates.action_receipts]
                .iter()
                .all(|path| path.starts_with(&self.workspace_root))
        {
            return Err(Error::new("invalid gate configuration"));
        }
        Ok(())
    }
}

fn require_root_controlled(path: &Path, directory: bool, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let type_matches = if directory {
        metadata.is_dir()
    } else {
        metadata.is_file() && std::os::unix::fs::MetadataExt::nlink(&metadata) == 1
    };
    if !type_matches
        || metadata.file_type().is_symlink()
        || std::os::unix::fs::MetadataExt::uid(&metadata) != 0
        || std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o022 != 0
    {
        return Err(Error::new(format!(
            "{label} must be root-owned and not group/world writable"
        )));
    }
    Ok(())
}

fn require_credential_file(path: &Path, allowed_owners: &[u32]) -> Result<()> {
    require_absolute_no_symlink(path, "credential")?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || std::os::unix::fs::MetadataExt::nlink(&metadata) != 1
        || !allowed_owners.contains(&std::os::unix::fs::MetadataExt::uid(&metadata))
        || std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o222 != 0
        || metadata.len() != 32
    {
        return Err(Error::new(
            "credential must be a non-linked, non-writable, exact 32-byte root/unit-owned file",
        ));
    }
    Ok(())
}

fn require_broker_credential(path: &Path, allowed_owners: &[u32]) -> Result<()> {
    require_credential_file(path, allowed_owners)?;
    let metadata = fs::symlink_metadata(path)?;
    if std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o7777 != 0o400 {
        return Err(Error::new(
            "web broker credential must have exact mode 0400",
        ));
    }
    Ok(())
}

fn require_status_snapshot(path: &Path, allowed_owners: &[u32]) -> Result<()> {
    require_absolute_no_symlink(path, "supervisor status credential")?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || std::os::unix::fs::MetadataExt::nlink(&metadata) != 1
        || !allowed_owners.contains(&std::os::unix::fs::MetadataExt::uid(&metadata))
        || std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o222 != 0
        || metadata.len() == 0
        || metadata.len() > 64 * 1024
    {
        return Err(Error::new(
            "supervisor status credential must be a bounded non-linked non-writable root/unit-owned file",
        ));
    }
    Ok(())
}

fn current_effective_uid() -> Result<u32> {
    let status = fs::read_to_string("/proc/self/status")?;
    let line = status
        .lines()
        .find(|line| line.starts_with("Uid:"))
        .ok_or_else(|| Error::new("cannot determine effective service UID"))?;
    line.split_whitespace()
        .nth(2)
        .ok_or_else(|| Error::new("effective UID field is absent"))?
        .parse::<u32>()
        .map_err(|_| Error::new("effective UID field is malformed"))
}

pub fn validate_loopback_origin(origin: &str) -> Result<(String, u16)> {
    let rest = origin
        .strip_prefix("http://")
        .ok_or_else(|| Error::new("Ollama origin must use plaintext loopback HTTP"))?;
    if rest.contains(['/', '?', '#', '@']) {
        return Err(Error::new(
            "Ollama origin must contain only loopback host and port",
        ));
    }
    let (host, port) = if let Some(port) = rest.strip_prefix("127.0.0.1:") {
        ("127.0.0.1", port)
    } else if let Some(port) = rest.strip_prefix("[::1]:") {
        ("::1", port)
    } else {
        return Err(Error::new("Ollama origin is not exact loopback"));
    };
    let port = port
        .parse::<u16>()
        .map_err(|_| Error::new("invalid Ollama port"))?;
    if port == 0 {
        return Err(Error::new("invalid Ollama port"));
    }
    Ok((host.to_owned(), port))
}

#[cfg(test)]
mod tests {
    use super::validate_loopback_origin;

    #[test]
    fn only_exact_loopback_origins_are_accepted() {
        assert!(validate_loopback_origin("http://127.0.0.1:11434").is_ok());
        assert!(validate_loopback_origin("http://[::1]:11434").is_ok());
        assert!(validate_loopback_origin("http://localhost:11434").is_err());
        assert!(validate_loopback_origin("https://127.0.0.1:11434").is_err());
        assert!(validate_loopback_origin("http://127.0.0.1:11434/api").is_err());
    }
}
