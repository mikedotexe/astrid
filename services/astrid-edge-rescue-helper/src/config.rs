//! Exact root-owned rescue-helper configuration and immutable policy validation.

use std::collections::BTreeSet;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::fs_guard::{
    MAX_JSON_BYTES, read_json, require_absolute, require_active_generation_link,
    require_private_root_file, sha256_file,
};
use crate::{Error, Result};

pub const CONFIG_SCHEMA: &str = "astrid.edge_rescue_helper.config.v1";
pub const RUST_RELEASE: &str = "1.94.1";
pub const RUST_COMMIT: &str = "e408947bfd200af42db322daf0fadfe7e26d3bd1";
const ALLOWED_SERVICES: &[&str] = &[
    "astrid.service",
    "astrid-edge-runtime.service",
    "astrid-model-warmup.service",
];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    pub appliance_id: String,
    pub target: String,
    pub model: String,
    pub ollama_origin: String,
    pub source: SourceConfig,
    pub roots: RootConfig,
    pub identities: IdentityConfig,
    pub executables: Executables,
    pub services: ServiceConfig,
    pub storage: StorageConfig,
    pub policy: Policy,
    pub drain: DrainConfig,
    pub health: HealthConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceConfig {
    pub root: PathBuf,
    pub manifest: PathBuf,
    pub signature: PathBuf,
    pub signing_key: PathBuf,
    pub intent_attestation_key: PathBuf,
    pub ledger_attestation_key: PathBuf,
    pub vendor: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootConfig {
    pub supervisor_state: PathBuf,
    pub candidate_store: PathBuf,
    pub model_handoff_root: PathBuf,
    pub model_handoff_ledger: PathBuf,
    pub candidate_work: PathBuf,
    pub build_store: PathBuf,
    pub releases: PathBuf,
    pub active_link: PathBuf,
    pub generation_binding: PathBuf,
    pub maintenance_lease: PathBuf,
    pub maintenance_mutex: PathBuf,
    pub state_snapshots: PathBuf,
    pub workspace: PathBuf,
    pub system_unit_root: PathBuf,
    pub unit_policy: PathBuf,
    pub unit_transactions: PathBuf,
    pub candidate_sandbox_root: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityConfig {
    pub steward_uid: u32,
    pub steward_gid: u32,
    pub builder_uid: u32,
    pub builder_gid: u32,
    pub updater_uid: u32,
    pub updater_gid: u32,
    pub runtime_uid: u32,
    pub runtime_gid: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedExecutable {
    pub path: PathBuf,
    pub sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Executables {
    pub cargo: TrustedExecutable,
    pub rustc: TrustedExecutable,
    pub rustfmt: TrustedExecutable,
    pub python: TrustedExecutable,
    pub systemctl: TrustedExecutable,
    pub systemd_run: TrustedExecutable,
    pub systemd_analyze: TrustedExecutable,
    pub checkpoint: TrustedExecutable,
    pub capsule_builder: TrustedExecutable,
    pub invariant_runner: TrustedExecutable,
    pub package_verifier: TrustedExecutable,
    pub state_store: TrustedExecutable,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceConfig {
    pub core: String,
    pub warmup: String,
    pub edge: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    pub config: PathBuf,
    pub config_sha256: String,
    pub install_attestation: PathBuf,
    pub health_attestation: PathBuf,
    pub runtime_state_mount: PathBuf,
    pub rollback_mount: PathBuf,
    pub backing_uuid: String,
    pub runtime_filesystem_uuid: String,
    pub rollback_filesystem_uuid: String,
    pub image_bytes: u64,
    pub host_reserve_bytes: u64,
    pub store_minimum_free_bytes: u64,
    pub emergency_inode_reserve_files: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub maximum_files: usize,
    pub maximum_changed_lines: usize,
    pub build_workers: usize,
    pub command_timeout_seconds: u64,
    pub pipeline_timeout_seconds: u64,
    pub maximum_candidate_bytes: u64,
    pub minimum_free_disk_bytes: u64,
    pub candidate_memory_max_bytes: u64,
    pub candidate_memory_swap_max_bytes: u64,
    pub candidate_tasks_max: u64,
    pub candidate_cpu_quota_percent: u64,
    pub network_policy: String,
    pub dependency_policy: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrainConfig {
    pub autonomy_state: PathBuf,
    pub model_lock: PathBuf,
    pub model_lock_gid: u32,
    pub maintenance_edge_acknowledgement: PathBuf,
    pub maintenance_core_acknowledgement: PathBuf,
    pub activity_ledgers: Vec<PathBuf>,
    pub maximum_wait_seconds: u64,
    pub poll_milliseconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthConfig {
    pub sensor_state: PathBuf,
    pub hindsight_state: PathBuf,
    pub fill_history: PathBuf,
    pub model_warmup_receipt: PathBuf,
    pub model_warmup_uid: u32,
    pub meminfo: PathBuf,
    pub swaps: PathBuf,
    pub thermal_celsius: PathBuf,
    pub telemetry_addr: SocketAddr,
    pub audio_policy: AudioPolicy,
    pub expected_audio_source: String,
    pub maximum_age_seconds: u64,
    pub maximum_thermal_celsius: f64,
    pub minimum_available_ram_bytes: u64,
    pub maximum_swap_bytes: u64,
    pub minimum_fill_samples: usize,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudioPolicy {
    RequiredFreshNumeric,
    RequiredUnavailable,
}

impl Config {
    pub fn from_root_owned_file(path: &Path) -> Result<Self> {
        require_private_root_file(path, "rescue-helper config")?;
        let config: Self = read_json(path, MAX_JSON_BYTES)?;
        config.validate()?;
        for executable in config.executables.all() {
            executable.verify()?;
        }
        require_private_root_file(&config.source.signing_key, "source signing key")?;
        require_private_root_file(
            &config.source.intent_attestation_key,
            "intent attestation key",
        )?;
        let ledger_key =
            crate::ledger_auth::LedgerKey::load(&config.source.ledger_attestation_key, true)?;
        let trust_domains = BTreeSet::from([
            sha256_file(&config.source.signing_key, 4_096)?,
            sha256_file(&config.source.intent_attestation_key, 4_096)?,
            ledger_key.fingerprint().to_owned(),
        ]);
        if trust_domains.len() != 3 {
            return Err(Error::new(
                "source, intent, and lifecycle-ledger keys must be independent",
            ));
        }
        Ok(config)
    }

    /// Load only the immutable subset needed by the scheduled-reflection
    /// admission guard. The guard runs inside the steward's restricted mount
    /// namespace, where build, release, and signing-key paths are deliberately
    /// unavailable; accepting this subset grants no build or transition
    /// authority.
    pub fn from_root_owned_file_for_reflection(path: &Path) -> Result<Self> {
        require_private_root_file(path, "rescue-helper config")?;
        let config: Self = read_json(path, MAX_JSON_BYTES)?;
        config.validate_reflection_guard_subset()?;
        Ok(config)
    }

    fn validate_reflection_guard_subset(&self) -> Result<()> {
        let workspace_state = self
            .roots
            .workspace
            .ancestors()
            .nth(3)
            .filter(|_| self.roots.workspace.ends_with("home/default/edge"))
            .ok_or_else(|| Error::new("reflection workspace layout is invalid"))?;
        let steward_state = self
            .roots
            .candidate_store
            .parent()
            .ok_or_else(|| Error::new("reflection steward-state root is absent"))?;
        let release_root = self
            .roots
            .releases
            .parent()
            .ok_or_else(|| Error::new("reflection release root is absent"))?;
        let paths = [
            &self.roots.supervisor_state,
            &self.roots.generation_binding,
            &self.roots.maintenance_lease,
            &self.roots.maintenance_mutex,
            &self.roots.workspace,
            &self.roots.releases,
            &self.roots.active_link,
            steward_state,
            &self.drain.model_lock,
            &self.drain.maintenance_edge_acknowledgement,
            &self.drain.maintenance_core_acknowledgement,
            &self.drain.autonomy_state,
        ];
        if self.schema != CONFIG_SCHEMA
            || !valid_identifier(&self.appliance_id)
            || paths.iter().any(|path| !path.is_absolute())
            || self.identities.steward_uid == 0
            || self.identities.steward_gid == 0
            || self.identities.runtime_uid == 0
            || self.identities.runtime_gid == 0
            || self.identities.steward_uid == self.identities.runtime_uid
            || self.identities.steward_gid == self.identities.runtime_gid
            || !(30..=3_600).contains(&self.drain.maximum_wait_seconds)
            || !(100..=5_000).contains(&self.drain.poll_milliseconds)
            || self.roots.generation_binding
                != self.roots.supervisor_state.join("current-generation")
            || self.roots.maintenance_lease != self.roots.supervisor_state.join("maintenance.json")
            || self.roots.maintenance_mutex != self.roots.supervisor_state.join("maintenance.lock")
            || self.roots.active_link != release_root.join("current")
            || self.drain.model_lock != self.roots.supervisor_state.join("model.lock")
            || self.drain.maintenance_edge_acknowledgement
                != self
                    .roots
                    .workspace
                    .join("runtime/maintenance-edge-ack.json")
            || self.drain.maintenance_core_acknowledgement
                != workspace_state.join("run/maintenance-core-ack.json")
            || self.drain.autonomy_state != self.roots.workspace.join("autonomous/state.json")
            || steward_state.join("candidate-outbox") != self.roots.candidate_store
        {
            return Err(Error::new(
                "scheduled-reflection guard configuration escaped its exact layout",
            ));
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<()> {
        if self.schema != CONFIG_SCHEMA
            || !valid_identifier(&self.appliance_id)
            || !valid_target(&self.target)
            || self.model.is_empty()
            || self.model.len() > 128
            || self.model.chars().any(char::is_whitespace)
            || !valid_loopback_origin(&self.ollama_origin)
        {
            return Err(Error::new("invalid rescue-helper identity or schema"));
        }
        if self.executables.invariant_runner.path != self.executables.package_verifier.path
            || self.executables.invariant_runner.sha256 != self.executables.package_verifier.sha256
        {
            return Err(Error::new(
                "invariant and package replay must use the same pinned rescue helper",
            ));
        }
        let updater_staging = self
            .roots
            .state_snapshots
            .parent()
            .ok_or_else(|| Error::new("updater state root is absent"))?
            .join("generation-staging");
        let paths = [
            &self.source.root,
            &self.source.manifest,
            &self.source.signature,
            &self.source.signing_key,
            &self.source.intent_attestation_key,
            &self.source.ledger_attestation_key,
            &self.source.vendor,
            &self.roots.supervisor_state,
            &self.roots.candidate_store,
            &self.roots.model_handoff_root,
            &self.roots.model_handoff_root,
            &self.roots.model_handoff_ledger,
            &self.roots.candidate_work,
            &self.roots.build_store,
            &self.roots.releases,
            &self.roots.generation_binding,
            &self.roots.maintenance_lease,
            &self.roots.maintenance_mutex,
            &self.roots.state_snapshots,
            &updater_staging,
            &self.roots.workspace,
            &self.roots.system_unit_root,
            &self.roots.unit_policy,
            &self.roots.unit_transactions,
            &self.roots.candidate_sandbox_root,
            &self.drain.autonomy_state,
            &self.drain.model_lock,
            &self.drain.maintenance_edge_acknowledgement,
            &self.drain.maintenance_core_acknowledgement,
            &self.health.sensor_state,
            &self.health.hindsight_state,
            &self.health.fill_history,
            &self.health.model_warmup_receipt,
            &self.health.meminfo,
            &self.health.swaps,
            &self.health.thermal_celsius,
            &self.storage.config,
            &self.storage.runtime_state_mount,
            &self.storage.rollback_mount,
            &self.storage.install_attestation,
            &self.storage.health_attestation,
        ];
        for path in paths {
            require_absolute(path, "configured path")?;
        }
        let expected_system_unit_alias = self
            .roots
            .state_snapshots
            .parent()
            .ok_or_else(|| Error::new("updater state root is absent"))?
            .join("system-units");
        if self.roots.system_unit_root != expected_system_unit_alias
            || self.roots.unit_policy != self.roots.supervisor_state.join("unit-policy.json")
            || self.roots.unit_transactions != self.roots.state_snapshots.join("unit-transactions")
            || self
                .roots
                .state_snapshots
                .file_name()
                .and_then(|name| name.to_str())
                != Some("snapshots")
        {
            return Err(Error::new(
                "transactional unit roots differ from the immutable appliance layout",
            ));
        }
        if self.roots.candidate_sandbox_root
            != Path::new("/usr/libexec/astrid-edge/immutable/candidate-rootfs")
        {
            return Err(Error::new(
                "candidate sandbox root differs from the immutable appliance layout",
            ));
        }
        let sandbox_metadata = fs::symlink_metadata(&self.roots.candidate_sandbox_root)?;
        if !sandbox_metadata.is_dir()
            || sandbox_metadata.file_type().is_symlink()
            || sandbox_metadata.uid() != 0
            || sandbox_metadata.gid() != 0
            || sandbox_metadata.mode() & 0o777 != 0o555
        {
            return Err(Error::new(
                "candidate sandbox root is not an exact root-owned read-only directory",
            ));
        }
        validate_candidate_sandbox_skeleton(
            &self.roots.candidate_sandbox_root,
            &self.roots.workspace,
        )?;
        if self.executables.systemd_run.path != Path::new("/usr/bin/systemd-run") {
            return Err(Error::new(
                "candidate transient-unit launcher differs from the immutable path",
            ));
        }
        let workspace_state_root = self
            .roots
            .workspace
            .ancestors()
            .nth(3)
            .ok_or_else(|| Error::new("workspace state root is unavailable"))?;
        let resolved_workspace_state = fs::canonicalize(workspace_state_root)?;
        let resolved_runtime_mount = fs::canonicalize(&self.storage.runtime_state_mount)?;
        if self.storage.config != Path::new("/etc/astrid/edge-state-store.json")
            || !valid_hex64(&self.storage.config_sha256)
            || self.storage.install_attestation
                != Path::new("/run/astrid-edge-state-store/install-attestation.json")
            || self.storage.health_attestation
                != Path::new("/run/astrid-edge-state-store/health-attestation.json")
            || resolved_workspace_state != resolved_runtime_mount
            || self.storage.rollback_mount != self.roots.state_snapshots
            || self.storage.runtime_filesystem_uuid == self.storage.rollback_filesystem_uuid
            || !valid_uuid(&self.storage.backing_uuid)
            || !valid_uuid(&self.storage.runtime_filesystem_uuid)
            || !valid_uuid(&self.storage.rollback_filesystem_uuid)
            || self.storage.image_bytes != 32 * 1024 * 1024 * 1024
            || self.storage.host_reserve_bytes != 64 * 1024 * 1024 * 1024
            || self.storage.store_minimum_free_bytes != 4 * 1024 * 1024 * 1024
            || self.storage.emergency_inode_reserve_files != 65_536
            || sha256_file(&self.storage.config, 32 * 1024)? != self.storage.config_sha256
        {
            return Err(Error::new(
                "bounded runtime/rollback storage contract is invalid",
            ));
        }
        require_active_generation_link(
            &self.roots.active_link,
            &self.roots.releases,
            "active generation link",
        )?;
        if self.drain.activity_ledgers.is_empty()
            || self.drain.activity_ledgers.len() > 16
            || self.drain.model_lock_gid == 0
            || [
                self.identities.steward_gid,
                self.identities.builder_gid,
                self.identities.updater_gid,
                self.identities.runtime_gid,
            ]
            .contains(&self.drain.model_lock_gid)
            || !(30..=3_600).contains(&self.drain.maximum_wait_seconds)
            || !(100..=5_000).contains(&self.drain.poll_milliseconds)
            || !valid_drain_paths(self)
        {
            return Err(Error::new("runtime drain paths or bounds are invalid"));
        }
        let isolated = [
            &self.source.root,
            &self.roots.supervisor_state,
            &self.roots.candidate_store,
            &self.roots.candidate_work,
            &self.roots.build_store,
            &self.roots.releases,
            &self.roots.state_snapshots,
            &updater_staging,
            &self.roots.workspace,
            &self.roots.candidate_sandbox_root,
        ];
        for (index, left) in isolated.iter().enumerate() {
            for right in isolated.iter().skip(index.saturating_add(1)) {
                if left.starts_with(right) || right.starts_with(left) {
                    return Err(Error::new(
                        "source, candidate, build, release, and state roots overlap",
                    ));
                }
            }
        }
        if self.roots.generation_binding != self.roots.supervisor_state.join("current-generation") {
            return Err(Error::new(
                "generation binding must be the supervisor-state current-generation file",
            ));
        }
        if self.roots.active_link.parent() != self.roots.releases.parent() {
            return Err(Error::new(
                "active generation link must live beside the releases root",
            ));
        }
        let steward_state = self
            .roots
            .candidate_store
            .parent()
            .ok_or_else(|| Error::new("candidate store has no steward-state parent"))?;
        if self
            .roots
            .candidate_store
            .file_name()
            .and_then(|name| name.to_str())
            != Some("candidate-outbox")
            || self.roots.model_handoff_root != steward_state.join("model-handoff")
            || self.roots.model_handoff_ledger != steward_state.join("model-unload-receipts.jsonl")
            || self.source.intent_attestation_key == self.source.signing_key
            || self.source.ledger_attestation_key == self.source.signing_key
            || self.source.ledger_attestation_key == self.source.intent_attestation_key
        {
            return Err(Error::new(
                "steward handoff roots or key separation are invalid",
            ));
        }
        if !self.source.vendor.starts_with(&self.source.root)
            || !self.source.manifest.starts_with(&self.source.root)
            || !self.source.signature.starts_with(&self.source.root)
            || self.source.signing_key.starts_with(&self.source.root)
        {
            return Err(Error::new(
                "signed source paths or key separation are invalid",
            ));
        }
        let identities = [
            (self.identities.steward_uid, self.identities.steward_gid),
            (self.identities.builder_uid, self.identities.builder_gid),
            (self.identities.updater_uid, self.identities.updater_gid),
            (self.identities.runtime_uid, self.identities.runtime_gid),
        ];
        if identities.iter().any(|(uid, gid)| *uid == 0 || *gid == 0)
            || identities
                .iter()
                .map(|(uid, _)| uid)
                .collect::<BTreeSet<_>>()
                .len()
                != identities.len()
            || identities
                .iter()
                .map(|(_, gid)| gid)
                .collect::<BTreeSet<_>>()
                .len()
                != identities.len()
        {
            return Err(Error::new(
                "steward, builder, updater, and runtime identities must be separate non-root identities",
            ));
        }
        let services = [
            &self.services.core,
            &self.services.warmup,
            &self.services.edge,
        ];
        if services.iter().collect::<BTreeSet<_>>().len() != 3
            || services
                .iter()
                .any(|service| !ALLOWED_SERVICES.contains(&service.as_str()))
        {
            return Err(Error::new("service plan is not the fixed Astrid allowlist"));
        }
        if self.policy.maximum_files != 25
            || self.policy.maximum_changed_lines != 4_000
            || !(1..=4).contains(&self.policy.build_workers)
            || !(60..=3_600).contains(&self.policy.command_timeout_seconds)
            || !(600..=86_400).contains(&self.policy.pipeline_timeout_seconds)
            || self.policy.maximum_candidate_bytes > 16 * 1024 * 1024
            || self.policy.maximum_candidate_bytes < 1024
            || self.policy.minimum_free_disk_bytes < 1024 * 1024 * 1024
            || !(2 * 1024 * 1024 * 1024..=64 * 1024 * 1024 * 1024)
                .contains(&self.policy.candidate_memory_max_bytes)
            || self.policy.candidate_memory_swap_max_bytes > 128 * 1024 * 1024
            || !(32..=512).contains(&self.policy.candidate_tasks_max)
            || self.policy.candidate_cpu_quota_percent
                != u64::try_from(self.policy.build_workers)
                    .unwrap_or(u64::MAX)
                    .saturating_mul(100)
            || self.policy.network_policy != "private-network-none:v1"
            || self.policy.dependency_policy != "signed-vendor-offline-locked:v1"
        {
            return Err(Error::new(
                "immutable build policy is outside the compiled envelope",
            ));
        }
        if !(60..=1_800).contains(&self.health.maximum_age_seconds)
            || !(60.0..=90.0).contains(&self.health.maximum_thermal_celsius)
            || self.health.minimum_available_ram_bytes < 2 * 1024 * 1024 * 1024
            || self.health.maximum_swap_bytes > 128 * 1024 * 1024
            || !(10..=360).contains(&self.health.minimum_fill_samples)
            || self.health.model_warmup_uid == 0
            || [
                self.identities.steward_uid,
                self.identities.builder_uid,
                self.identities.updater_uid,
                self.identities.runtime_uid,
            ]
            .contains(&self.health.model_warmup_uid)
            || self.health.telemetry_addr != SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 7_878)
            || self.health.expected_audio_source.is_empty()
            || self.health.expected_audio_source.len() > 256
            || self
                .health
                .expected_audio_source
                .chars()
                .any(char::is_whitespace)
            || match self.health.audio_policy {
                AudioPolicy::RequiredFreshNumeric => !self
                    .health
                    .expected_audio_source
                    .starts_with("physical_alsa:"),
                AudioPolicy::RequiredUnavailable => {
                    self.health.expected_audio_source != "unavailable_no_audio_input"
                },
            }
        {
            return Err(Error::new("health policy is outside the compiled envelope"));
        }
        let state_root = self
            .roots
            .workspace
            .ancestors()
            .nth(3)
            .ok_or_else(|| Error::new("workspace state root is unavailable"))?;
        if self.health.sensor_state != self.roots.workspace.join("runtime/spectral_state.json")
            || self.health.fill_history != self.roots.workspace.join("runtime/fill_history.jsonl")
            || self.health.hindsight_state != state_root.join("operator/hindsight/latest.json")
            || self.health.model_warmup_receipt
                != Path::new("/var/lib/astrid-edge-model-warmup/receipt.json")
            || self.health.meminfo != Path::new("/proc/meminfo")
            || self.health.swaps != Path::new("/proc/swaps")
            || !self.health.thermal_celsius.starts_with("/sys/")
            || self
                .health
                .thermal_celsius
                .file_name()
                .and_then(|name| name.to_str())
                != Some("temp")
        {
            return Err(Error::new(
                "health inputs differ from the immutable appliance layout",
            ));
        }
        Ok(())
    }
}

impl Executables {
    #[must_use]
    pub fn all(&self) -> [&TrustedExecutable; 12] {
        [
            &self.cargo,
            &self.rustc,
            &self.rustfmt,
            &self.python,
            &self.systemctl,
            &self.systemd_run,
            &self.systemd_analyze,
            &self.checkpoint,
            &self.capsule_builder,
            &self.invariant_runner,
            &self.package_verifier,
            &self.state_store,
        ]
    }
}

impl TrustedExecutable {
    pub fn verify(&self) -> Result<()> {
        require_absolute(&self.path, "trusted executable")?;
        let metadata = fs::symlink_metadata(&self.path)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.nlink() != 1
            || metadata.uid() != 0
            || metadata.mode() & 0o022 != 0
            || !valid_hex64(&self.sha256)
            || sha256_file(&self.path, 256 * 1024 * 1024)? != self.sha256
        {
            return Err(Error::new(format!(
                "trusted executable identity failed: {}",
                self.path.display()
            )));
        }
        Ok(())
    }
}

fn valid_drain_paths(config: &Config) -> bool {
    let workspace = &config.roots.workspace;
    let state_root = workspace
        .ancestors()
        .nth(3)
        .filter(|_| workspace.ends_with("home/default/edge"));
    let Some(state_root) = state_root else {
        return false;
    };
    let expected_ledgers = BTreeSet::from([
        workspace.join("actions/receipts.jsonl"),
        workspace.join("web/receipts.jsonl"),
        workspace.join("introspection/receipts.jsonl"),
    ]);
    config.drain.autonomy_state == workspace.join("autonomous/state.json")
        && config.drain.model_lock == config.roots.supervisor_state.join("model.lock")
        && config.drain.maintenance_edge_acknowledgement
            == workspace.join("runtime/maintenance-edge-ack.json")
        && config.drain.maintenance_core_acknowledgement
            == state_root.join("run/maintenance-core-ack.json")
        && config
            .drain
            .activity_ledgers
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            == expected_ledgers
        && config
            .drain
            .activity_ledgers
            .iter()
            .all(|path| require_absolute(path, "activity ledger").is_ok())
}

fn validate_candidate_sandbox_skeleton(root: &Path, workspace: &Path) -> Result<()> {
    let workspace_suffix = workspace
        .strip_prefix("/")
        .map_err(|_| Error::new("candidate workspace denial path is not absolute"))?
        .to_path_buf();
    let mut allowed = BTreeSet::from_iter(
        [
            "bin",
            "dev",
            "etc",
            "home",
            "lib",
            "lib64",
            "media",
            "mnt",
            "opt",
            "proc",
            "root",
            "run",
            "sbin",
            "sys",
            "tmp",
            "usr",
            "usr/bin",
            "usr/lib",
            "usr/lib64",
            "usr/libexec",
            "usr/local",
            "usr/sbin",
            "usr/share",
            "var",
            "var/tmp",
        ]
        .map(PathBuf::from),
    );
    let mut prefix = PathBuf::new();
    for component in workspace_suffix.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err(Error::new(
                "candidate workspace denial path contains a non-normal component",
            ));
        }
        prefix.push(component.as_os_str());
        allowed.insert(prefix.clone());
    }
    for relative in &allowed {
        let path = root.join(relative);
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != 0
            || metadata.gid() != 0
            || metadata.mode() & 0o777 != 0o555
        {
            return Err(Error::new(
                "candidate sandbox skeleton contains a mutable or foreign entry",
            ));
        }
    }
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|_| Error::new("candidate sandbox entry escaped its root"))?;
            let metadata = fs::symlink_metadata(&path)?;
            if !allowed.contains(relative)
                || !metadata.is_dir()
                || metadata.file_type().is_symlink()
            {
                return Err(Error::new(
                    "candidate sandbox skeleton contains an unexpected entry",
                ));
            }
            pending.push(path);
        }
    }
    if fs::read_dir(root.join(&workspace_suffix))?.next().is_some() {
        return Err(Error::new("candidate workspace denial decoy is not empty"));
    }
    Ok(())
}

#[must_use]
pub fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

#[must_use]
pub fn valid_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_target(value: &str) -> bool {
    matches!(
        value,
        "x86_64-unknown-linux-gnu" | "aarch64-unknown-linux-gnu"
    )
}

fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()
            }
        })
}

fn valid_loopback_origin(value: &str) -> bool {
    let Some(authority) = value.strip_prefix("http://") else {
        return false;
    };
    let Some((host, port)) = authority.rsplit_once(':') else {
        return false;
    };
    matches!(host, "127.0.0.1" | "[::1]") && port.parse::<u16>().is_ok_and(|value| value != 0)
}

#[cfg(test)]
mod tests {
    use super::{valid_hex64, valid_identifier};

    #[test]
    fn identifiers_and_hashes_are_exact() {
        assert!(valid_identifier("avado.generation-1"));
        assert!(!valid_identifier("../generation"));
        assert!(valid_hex64(&"a".repeat(64)));
        assert!(!valid_hex64(&"A".repeat(64)));
    }
}
