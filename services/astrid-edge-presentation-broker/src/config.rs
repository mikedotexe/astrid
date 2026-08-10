use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::contract::valid_identifier;
use crate::fs_guard::{
    read_stable_regular, sha256, sha256_file, valid_hex64, verify_immutable_ancestors,
};
use crate::{Error, Result};

pub const CONFIG_SCHEMA: &str = "astrid.edge_candidate_presentation.broker_config.v1";
pub const SANDBOX_CONTRACT: &str =
    "unprivileged_no_network_no_home_read_only_generation_projection_only_v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedExecutable {
    pub path: PathBuf,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrokerPolicy {
    pub timeout_ms: u64,
    pub maximum_request_bytes: usize,
    pub maximum_projection_bytes: usize,
    pub maximum_stdout_bytes: usize,
    pub maximum_stderr_bytes: usize,
    pub memory_max_bytes: u64,
    pub require_cgroup_v2: bool,
    pub sandbox_contract: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrokerConfig {
    pub schema: String,
    pub appliance_id: String,
    pub target: String,
    pub releases_root: PathBuf,
    pub active_link: PathBuf,
    pub generation_binding: PathBuf,
    pub python: TrustedExecutable,
    pub policy: BrokerPolicy,
}

impl BrokerConfig {
    /// Read and validate the immutable root-owned broker configuration.
    ///
    /// # Errors
    ///
    /// Returns an error when the file identity, schema, path policy, executable
    /// identity, or any configured resource bound is invalid.
    pub fn from_root_owned_file(path: &Path) -> Result<Self> {
        verify_immutable_ancestors(path, true)?;
        let (bytes, _) = read_stable_regular(path, 64 * 1024, true)?;
        let value: Self = serde_json::from_slice(&bytes)?;
        value.validate(true)?;
        Ok(value)
    }

    /// Validate the complete fixed broker policy.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed identities, mutable/overlapping paths,
    /// an unbounded resource setting, or a Python executable digest mismatch.
    pub fn validate(&self, require_root_owner: bool) -> Result<()> {
        if self.schema != CONFIG_SCHEMA
            || !valid_identifier(&self.appliance_id, 128)
            || !valid_identifier(&self.target, 128)
            || self.policy.sandbox_contract != SANDBOX_CONTRACT
            || !(100..=30_000).contains(&self.policy.timeout_ms)
            // A request may carry one already-sanitized 64 KiB operator
            // projection plus its bounded activity metadata. The outer
            // 256 KiB envelope remains independent from the substantially
            // smaller stdout/stderr and projection ceilings below.
            || !(256..=262_144).contains(&self.policy.maximum_request_bytes)
            || !(1_024..=262_144).contains(&self.policy.maximum_projection_bytes)
            || !(1_024..=65_536).contains(&self.policy.maximum_stdout_bytes)
            || !(512..=16_384).contains(&self.policy.maximum_stderr_bytes)
            || !(64 * 1024 * 1024..=512 * 1024 * 1024).contains(&self.policy.memory_max_bytes)
            || !self.policy.require_cgroup_v2
        {
            return Err(Error::new(
                "presentation broker configuration escaped policy",
            ));
        }
        let paths = [
            &self.releases_root,
            &self.active_link,
            &self.generation_binding,
            &self.python.path,
        ];
        if paths.iter().any(|path| !path.is_absolute()) {
            return Err(Error::new("presentation broker paths must be absolute"));
        }
        let unique: BTreeSet<&PathBuf> = paths.into_iter().collect();
        if unique.len() != paths.len() || self.active_link.parent() != self.releases_root.parent() {
            return Err(Error::new(
                "presentation broker path identities overlap or drift",
            ));
        }
        if !valid_hex64(&self.python.sha256) {
            return Err(Error::new("presentation Python digest is invalid"));
        }
        if require_root_owner {
            verify_immutable_ancestors(&self.python.path, true)?;
        }
        let python_sha256 = if require_root_owner {
            let (_, metadata) = read_stable_regular(&self.python.path, 64 * 1024 * 1024, true)?;
            if std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o111 == 0 {
                return Err(Error::new("presentation Python is not executable"));
            }
            sha256_file(&self.python.path, 64 * 1024 * 1024, true)?
        } else {
            sha256(&fs::read(&self.python.path)?)
        };
        if python_sha256 != self.python.sha256 {
            return Err(Error::new("presentation Python executable digest mismatch"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{BrokerConfig, CONFIG_SCHEMA, SANDBOX_CONTRACT};
    use crate::fs_guard::sha256;

    #[test]
    fn shipped_template_renders_to_the_maximum_bounded_request_contract() {
        let executable = std::env::current_exe().expect("current test executable");
        let executable_sha256 = sha256(&std::fs::read(&executable).expect("read executable"));
        let rendered = include_str!("../../../packaging/headless/edge-presentation-broker.json.in")
            .replace("@@APPLIANCE_ID@@", "test-edge")
            .replace("@@TARGET@@", "x86_64-unknown-linux-gnu")
            .replace("@@PYTHON_PATH@@", &executable.display().to_string())
            .replace("@@PYTHON_SHA256@@", &executable_sha256);
        let config: BrokerConfig = serde_json::from_str(&rendered).expect("rendered config");

        config.validate(false).expect("shipped policy is valid");
        assert_eq!(config.schema, CONFIG_SCHEMA);
        assert_eq!(config.policy.sandbox_contract, SANDBOX_CONTRACT);
        assert_eq!(config.policy.maximum_request_bytes, 262_144);
        assert_eq!(config.policy.maximum_projection_bytes, 65_536);
    }
}
