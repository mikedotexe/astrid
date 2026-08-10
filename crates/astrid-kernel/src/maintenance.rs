//! CPU-edge immutable-updater maintenance gate and core drain acknowledgement.

use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tracing::{error, warn};

use crate::Kernel;

const TRANSITION_LEASE_SCHEMA: &str = "astrid.edge_self_change.maintenance_lease.v2";
const REFLECTION_LEASE_SCHEMA: &str = "astrid.edge_scheduled_reflection.lease.v1";
const ACK_SCHEMA: &str = "astrid.edge.maintenance_ack.v2";
const ACK_AUTHORITY: &str = "mutable_runtime_acknowledgement_subject_to_immutable_verification";
const MAXIMUM_LEASE_BYTES: u64 = 8 * 1_024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_field_names,
    reason = "lease_id is the canonical signed maintenance protocol field"
)]
struct TransitionLease {
    schema: String,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    reason: String,
    owner: String,
    lease_id: String,
    nonce: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReflectionLease {
    schema: String,
    lease_kind: String,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    reason: String,
    owner: String,
    lease_id: String,
    nonce: String,
    host_boot_id: String,
    service_invocation_id: String,
    generation_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeaseKind {
    GenerationTransition,
    ScheduledReflection,
}

impl LeaseKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::GenerationTransition => "generation_transition",
            Self::ScheduledReflection => "scheduled_reflection",
        }
    }

    const fn schema(self) -> &'static str {
        match self {
            Self::GenerationTransition => TRANSITION_LEASE_SCHEMA,
            Self::ScheduledReflection => REFLECTION_LEASE_SCHEMA,
        }
    }
}

#[derive(Debug)]
struct BoundLease {
    kind: LeaseKind,
    lease_id: String,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    generation_id: Option<String>,
    payload_sha256: String,
    nonce_sha256: String,
}

#[derive(Debug, Serialize)]
struct CoreAck<'a> {
    schema: &'static str,
    role: &'static str,
    lease_schema: &'static str,
    lease_kind: &'static str,
    lease_id: &'a str,
    lease_nonce_sha256: &'a str,
    lease_payload_sha256: &'a str,
    generation_id: &'a str,
    blocked_since_unix_ms: u64,
    acknowledged_at_unix_ms: u64,
    pid: u64,
    process_start_ticks: u64,
    authority: &'static str,
    ipc_user_input_blocked: bool,
    active_conversations: u64,
    active_sessions: u64,
    active_tools: u64,
    active_llm_requests: u64,
    drain_barrier_sequence: u64,
}

enum LeaseState {
    Absent,
    Expired,
    Active(BoundLease),
}

#[derive(Debug)]
struct Paths {
    transition_lease: PathBuf,
    reflection_lease: PathBuf,
    acknowledgement: PathBuf,
    generation_binding: PathBuf,
    runtime_gid: u32,
}

/// Initialize the maintenance gate before any native socket listener starts.
/// Partial configuration and any present lease-like filesystem object fail
/// closed; the polling task alone may reopen the gate.
pub(crate) fn initialize_gate(event_bus: &astrid_events::EventBus) {
    let blocked = match Paths::from_environment() {
        Ok(None) => false,
        Ok(Some(paths)) => [paths.transition_lease, paths.reflection_lease]
            .iter()
            .any(|path| match fs::symlink_metadata(path) {
                Ok(_) => true,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                Err(error) => {
                    error!(error = %error, path = %path.display(), "maintenance lease presence cannot be established; failing closed");
                    true
                },
            }),
        Err(error) => {
            error!(error = %error, "invalid CPU-edge maintenance configuration; failing closed");
            true
        },
    };
    event_bus.set_user_input_blocked(blocked);
}

/// Spawn the immutable-updater drain acknowledgement task.
pub(crate) fn spawn(kernel: Arc<Kernel>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(run(kernel))
}

async fn run(kernel: Arc<Kernel>) {
    let paths = match Paths::from_environment() {
        Ok(paths) => paths,
        Err(error) => {
            kernel.event_bus.set_user_input_blocked(true);
            error!(error = %error, "CPU-edge maintenance acknowledgement configuration is invalid");
            return;
        },
    };
    let mut interval = tokio::time::interval(Duration::from_millis(250));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut active_lease_hash = None;
    let mut blocked_since_unix_ms = None;
    let mut last_acknowledged_at = 0_u64;
    let mut last_barrier_at = 0_u64;
    let mut drain_barrier_sequence = None;

    loop {
        interval.tick().await;
        let Some(paths) = paths.as_ref() else {
            continue;
        };
        let now = unix_millis();
        match active_bound_lease(paths, now) {
            Ok(Some(bound)) => {
                kernel.event_bus.set_user_input_blocked(true);
                if active_lease_hash.as_deref() != Some(bound.payload_sha256.as_str()) {
                    active_lease_hash = Some(bound.payload_sha256.clone());
                    blocked_since_unix_ms = Some(now);
                    last_acknowledged_at = 0;
                    last_barrier_at = 0;
                    drain_barrier_sequence = None;
                    remove_ack(&paths.acknowledgement);
                }
                let activity = kernel.event_bus.maintenance_activity();
                if activity.exact
                    && activity.active_conversations == 0
                    && activity.active_tools == 0
                    && activity.active_llm_requests == 0
                {
                    if drain_barrier_sequence.is_none()
                        || now.saturating_sub(last_barrier_at) >= 30_000
                    {
                        drain_barrier_sequence = kernel.event_bus.publish_maintenance_barrier(
                            kernel.session_id.0,
                            bound.kind.schema(),
                            bound.kind.as_str(),
                            &bound.lease_id,
                            &bound.payload_sha256,
                        );
                        last_barrier_at = now;
                        last_acknowledged_at = 0;
                        remove_ack(&paths.acknowledgement);
                    }
                    if let Some(sequence) = drain_barrier_sequence
                        && (last_acknowledged_at == 0
                            || now.saturating_sub(last_acknowledged_at) >= 10_000)
                    {
                        match write_ack(
                            paths,
                            &bound,
                            blocked_since_unix_ms.unwrap_or(now),
                            now,
                            activity,
                            sequence,
                        ) {
                            Ok(()) => last_acknowledged_at = now,
                            Err(error) => {
                                remove_ack(&paths.acknowledgement);
                                last_acknowledged_at = 0;
                                warn!(error = %error, "core maintenance acknowledgement failed closed");
                            },
                        }
                    }
                } else {
                    remove_ack(&paths.acknowledgement);
                    last_acknowledged_at = 0;
                    last_barrier_at = 0;
                    drain_barrier_sequence = None;
                }
            },
            Ok(None) => {
                kernel.event_bus.set_user_input_blocked(false);
                active_lease_hash = None;
                blocked_since_unix_ms = None;
                last_acknowledged_at = 0;
                last_barrier_at = 0;
                drain_barrier_sequence = None;
                remove_ack(&paths.acknowledgement);
            },
            Err(error) => {
                // A present but malformed root lease is a fail-closed
                // maintenance request and can never receive an ACK.
                kernel.event_bus.set_user_input_blocked(true);
                remove_ack(&paths.acknowledgement);
                warn!(error = %error, "immutable maintenance lease failed validation");
            },
        }
    }
}

impl Paths {
    fn from_environment() -> anyhow::Result<Option<Self>> {
        let transition_lease = std::env::var_os("ASTRID_EDGE_MAINTENANCE_LEASE_PATH");
        let reflection_lease = std::env::var_os("ASTRID_EDGE_REFLECTION_LEASE_PATH");
        let acknowledgement = std::env::var_os("ASTRID_EDGE_MAINTENANCE_CORE_ACK_PATH");
        let generation_binding = std::env::var_os("ASTRID_EDGE_GENERATION_BINDING_PATH");
        if transition_lease.is_none()
            && reflection_lease.is_none()
            && acknowledgement.is_none()
            && generation_binding.is_none()
        {
            return Ok(None);
        }
        let paths = Self {
            transition_lease: PathBuf::from(
                transition_lease.context("generation-transition lease path is missing")?,
            ),
            reflection_lease: PathBuf::from(
                reflection_lease.context("scheduled-reflection lease path is missing")?,
            ),
            acknowledgement: PathBuf::from(
                acknowledgement.context("core maintenance ACK path is missing")?,
            ),
            generation_binding: PathBuf::from(
                generation_binding.context("generation binding path is missing")?,
            ),
            runtime_gid: nix::unistd::getegid().as_raw(),
        };
        let all = [
            &paths.transition_lease,
            &paths.reflection_lease,
            &paths.acknowledgement,
            &paths.generation_binding,
        ];
        if all.iter().any(|path| !path.is_absolute())
            || all.iter().enumerate().any(|(index, left)| {
                all.iter()
                    .skip(index.saturating_add(1))
                    .any(|right| left == right)
            })
        {
            anyhow::bail!("maintenance paths must be distinct and absolute");
        }
        Ok(Some(paths))
    }
}

fn active_bound_lease(paths: &Paths, now: u64) -> anyhow::Result<Option<BoundLease>> {
    let transition = read_bound_lease(
        &paths.transition_lease,
        LeaseKind::GenerationTransition,
        now,
        paths.runtime_gid,
    )?;
    let reflection = read_bound_lease(
        &paths.reflection_lease,
        LeaseKind::ScheduledReflection,
        now,
        paths.runtime_gid,
    )?;
    select_active_lease(transition, reflection)
}

fn select_active_lease(
    transition: LeaseState,
    reflection: LeaseState,
) -> anyhow::Result<Option<BoundLease>> {
    match (transition, reflection) {
        (LeaseState::Active(_), LeaseState::Active(_)) => {
            anyhow::bail!("generation transition and scheduled reflection leases overlap")
        },
        (LeaseState::Active(bound), LeaseState::Absent | LeaseState::Expired)
        | (LeaseState::Absent | LeaseState::Expired, LeaseState::Active(bound)) => Ok(Some(bound)),
        (LeaseState::Absent | LeaseState::Expired, LeaseState::Absent | LeaseState::Expired) => {
            Ok(None)
        },
    }
}

fn read_bound_lease(
    path: &Path,
    kind: LeaseKind,
    now: u64,
    runtime_gid: u32,
) -> anyhow::Result<LeaseState> {
    read_bound_lease_for_owner(path, kind, now, 0, runtime_gid)
}

fn read_bound_lease_for_owner(
    path: &Path,
    kind: LeaseKind,
    now: u64,
    root_uid: u32,
    runtime_gid: u32,
) -> anyhow::Result<LeaseState> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LeaseState::Absent);
        },
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != root_uid
        || match kind {
            LeaseKind::GenerationTransition => metadata.permissions().mode() & 0o777 != 0o444,
            LeaseKind::ScheduledReflection => {
                metadata.permissions().mode() & 0o777 != 0o440 || metadata.gid() != runtime_gid
            },
        }
        || metadata.len() > MAXIMUM_LEASE_BYTES
    {
        anyhow::bail!("maintenance lease identity or mode failed");
    }
    let bytes = stable_read(path, MAXIMUM_LEASE_BYTES)?;
    let payload_sha256 = sha256(&bytes);
    match kind {
        LeaseKind::GenerationTransition => {
            let lease: TransitionLease = serde_json::from_slice(&bytes)?;
            let nonce_sha256 = validate_transition_lease(&lease, now)?;
            if lease.expires_at_unix_ms <= now {
                return Ok(LeaseState::Expired);
            }
            Ok(LeaseState::Active(BoundLease {
                kind,
                lease_id: lease.lease_id,
                created_at_unix_ms: lease.created_at_unix_ms,
                expires_at_unix_ms: lease.expires_at_unix_ms,
                generation_id: None,
                payload_sha256,
                nonce_sha256,
            }))
        },
        LeaseKind::ScheduledReflection => {
            let lease: ReflectionLease = serde_json::from_slice(&bytes)?;
            let nonce_sha256 = validate_reflection_lease(&lease, now)?;
            if lease.expires_at_unix_ms <= now {
                return Ok(LeaseState::Expired);
            }
            Ok(LeaseState::Active(BoundLease {
                kind,
                lease_id: lease.lease_id,
                created_at_unix_ms: lease.created_at_unix_ms,
                expires_at_unix_ms: lease.expires_at_unix_ms,
                generation_id: Some(lease.generation_id),
                payload_sha256,
                nonce_sha256,
            }))
        },
    }
}

fn validate_transition_lease(lease: &TransitionLease, now: u64) -> anyhow::Result<String> {
    if lease.schema != TRANSITION_LEASE_SCHEMA
        || lease.owner != "immutable_astrid_edge_rescue_helper"
        || lease.reason.is_empty()
        || lease.reason.chars().count() > 128
        || lease.reason.chars().any(char::is_control)
        || lease.created_at_unix_ms > now.saturating_add(30_000)
        || lease.expires_at_unix_ms <= lease.created_at_unix_ms
        || lease
            .expires_at_unix_ms
            .saturating_sub(lease.created_at_unix_ms)
            > 48 * 60 * 60 * 1_000
        || !is_lower_hex(&lease.nonce, 64)
    {
        anyhow::bail!("maintenance lease content escaped bounds");
    }
    let nonce_sha256 = sha256(lease.nonce.as_bytes());
    if lease.lease_id != format!("lease-{}", &nonce_sha256[..24]) {
        anyhow::bail!("maintenance lease ID does not bind its nonce");
    }
    Ok(nonce_sha256)
}

fn validate_reflection_lease(lease: &ReflectionLease, now: u64) -> anyhow::Result<String> {
    let boot_id = current_boot_id().context("kernel boot identity is unavailable")?;
    validate_reflection_lease_for_boot(lease, now, &boot_id)
}

fn validate_reflection_lease_for_boot(
    lease: &ReflectionLease,
    now: u64,
    boot_id: &str,
) -> anyhow::Result<String> {
    if lease.schema != REFLECTION_LEASE_SCHEMA
        || lease.lease_kind != LeaseKind::ScheduledReflection.as_str()
        || lease.owner != "immutable_astrid_edge_reflection_guard"
        || lease.reason != "scheduled_reflection"
        || lease.created_at_unix_ms > now.saturating_add(30_000)
        || lease.expires_at_unix_ms <= lease.created_at_unix_ms
        || lease
            .expires_at_unix_ms
            .saturating_sub(lease.created_at_unix_ms)
            != 3 * 60 * 60 * 1_000
        || !is_lower_hex(&lease.nonce, 64)
        || !is_lower_hex(&lease.service_invocation_id, 32)
        || !valid_boot_id(&lease.host_boot_id)
        || !valid_identifier(&lease.generation_id)
        || boot_id != lease.host_boot_id
    {
        anyhow::bail!("scheduled-reflection lease content escaped bounds");
    }
    let nonce_sha256 = sha256(lease.nonce.as_bytes());
    if lease.lease_id != format!("reflection-{}", &nonce_sha256[..24]) {
        anyhow::bail!("scheduled-reflection lease ID does not bind its nonce");
    }
    Ok(nonce_sha256)
}

fn write_ack(
    paths: &Paths,
    bound: &BoundLease,
    blocked_since_unix_ms: u64,
    now: u64,
    activity: astrid_events::MaintenanceActivitySnapshot,
    drain_barrier_sequence: u64,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        drain_barrier_sequence > 0,
        "maintenance drain barrier is absent"
    );
    let generation_id = read_generation(&paths.generation_binding)?;
    anyhow::ensure!(
        bound
            .generation_id
            .as_deref()
            .is_none_or(|expected| expected == generation_id),
        "scheduled-reflection lease generation differs from active generation"
    );
    anyhow::ensure!(
        now >= bound.created_at_unix_ms && now < bound.expires_at_unix_ms,
        "maintenance lease expired before core acknowledgement"
    );
    let ack = CoreAck {
        schema: ACK_SCHEMA,
        role: "core",
        lease_schema: bound.kind.schema(),
        lease_kind: bound.kind.as_str(),
        lease_id: &bound.lease_id,
        lease_nonce_sha256: &bound.nonce_sha256,
        lease_payload_sha256: &bound.payload_sha256,
        generation_id: &generation_id,
        blocked_since_unix_ms,
        acknowledged_at_unix_ms: now,
        pid: u64::from(std::process::id()),
        process_start_ticks: process_start_ticks()?,
        authority: ACK_AUTHORITY,
        ipc_user_input_blocked: true,
        active_conversations: u64::try_from(activity.active_conversations)?,
        active_sessions: u64::try_from(activity.active_sessions)?,
        active_tools: u64::try_from(activity.active_tools)?,
        active_llm_requests: u64::try_from(activity.active_llm_requests)?,
        drain_barrier_sequence,
    };
    atomic_owner_write(&paths.acknowledgement, &serde_json::to_vec(&ack)?)
}

fn read_generation(path: &Path) -> anyhow::Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o777 != 0o444
        || metadata.len() > 256
    {
        anyhow::bail!("generation binding identity failed");
    }
    let bytes = stable_read(path, 256)?;
    let generation = std::str::from_utf8(&bytes)?.trim_end_matches('\n');
    if generation.is_empty()
        || generation.len() > 128
        || generation == "."
        || generation == ".."
        || !generation
            .bytes()
            .enumerate()
            .all(|(index, byte)| match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => true,
                b'.' | b'_' | b'-' => index > 0,
                _ => false,
            })
        || bytes != format!("{generation}\n").as_bytes()
    {
        anyhow::bail!("generation binding is not canonical");
    }
    Ok(generation.to_owned())
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value.bytes().enumerate().all(|(index, byte)| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => true,
            b'.' | b'_' | b'-' => index > 0,
            _ => false,
        })
}

fn valid_boot_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                matches!(byte, b'0'..=b'9' | b'a'..=b'f')
            }
        })
}

fn current_boot_id() -> Option<String> {
    fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| valid_boot_id(value))
}

fn process_start_ticks() -> anyhow::Result<u64> {
    let stat = fs::read_to_string("/proc/self/stat")?;
    let after_name = stat.rfind(") ").context("process stat is malformed")?;
    let fields_start = after_name
        .checked_add(2)
        .context("process stat field offset overflowed")?;
    stat[fields_start..]
        .split_whitespace()
        .nth(19)
        .context("process start time is absent")?
        .parse()
        .context("process start time is invalid")
}

fn stable_read(path: &Path, maximum: u64) -> anyhow::Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)?;
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.nlink() != 1
        || before.len() > maximum
    {
        anyhow::bail!("stable input identity failed");
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc_o_nofollow_cloexec())
        .open(path)?;
    let opened = file.metadata()?;
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
    (&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let path_after = fs::symlink_metadata(path)?;
    let identity = |metadata: &fs::Metadata| {
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
        )
    };
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != opened.len()
        || identity(&before) != identity(&opened)
        || identity(&opened) != identity(&after)
        || identity(&after) != identity(&path_after)
    {
        anyhow::bail!("stable input changed during read");
    }
    Ok(bytes)
}

fn atomic_owner_write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path.parent().context("ACK path has no parent")?;
    let metadata = fs::symlink_metadata(parent)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o077 != 0
    {
        anyhow::bail!("ACK parent must be a private regular directory");
    }
    let temporary = parent.join(format!(
        ".maintenance-core-ack.{}.tmp",
        uuid::Uuid::new_v4()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn remove_ack(path: &Path) {
    if fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.is_file() && !metadata.file_type().is_symlink() && metadata.nlink() == 1
    }) {
        let _ = fs::remove_file(path);
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(target_os = "linux")]
const fn libc_o_nofollow_cloexec() -> i32 {
    0o00_400_000 | 0o02_000_000
}

#[cfg(not(target_os = "linux"))]
const fn libc_o_nofollow_cloexec() -> i32 {
    0
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

    use super::{
        BoundLease, CoreAck, LeaseKind, LeaseState, ReflectionLease, TransitionLease, is_lower_hex,
        read_bound_lease_for_owner, select_active_lease, sha256,
        validate_reflection_lease_for_boot, validate_transition_lease,
    };

    #[test]
    fn lower_hex_validation_is_exact() {
        assert!(is_lower_hex(&"a".repeat(64), 64));
        assert!(!is_lower_hex(&"A".repeat(64), 64));
    }

    #[test]
    fn structurally_valid_expired_lease_is_not_malformed() {
        let nonce = "a".repeat(64);
        let nonce_sha256 = sha256(nonce.as_bytes());
        let lease = TransitionLease {
            schema: super::TRANSITION_LEASE_SCHEMA.to_string(),
            created_at_unix_ms: 1_000,
            expires_at_unix_ms: 2_000,
            reason: "bounded maintenance".to_string(),
            owner: "immutable_astrid_edge_rescue_helper".to_string(),
            lease_id: format!("lease-{}", &nonce_sha256[..24]),
            nonce,
        };
        assert_eq!(
            validate_transition_lease(&lease, 3_000).unwrap(),
            nonce_sha256
        );
    }

    #[test]
    fn scheduled_reflection_lease_binds_boot_generation_kind_and_nonce() {
        let now = super::unix_millis();
        let nonce = "b".repeat(64);
        let nonce_sha256 = sha256(nonce.as_bytes());
        let boot_id = "00000000-0000-0000-0000-000000000001";
        let lease = ReflectionLease {
            schema: super::REFLECTION_LEASE_SCHEMA.to_owned(),
            lease_kind: LeaseKind::ScheduledReflection.as_str().to_owned(),
            created_at_unix_ms: now,
            expires_at_unix_ms: now.saturating_add(3 * 60 * 60 * 1_000),
            reason: "scheduled_reflection".to_owned(),
            owner: "immutable_astrid_edge_reflection_guard".to_owned(),
            lease_id: format!("reflection-{}", &nonce_sha256[..24]),
            nonce,
            host_boot_id: boot_id.to_owned(),
            service_invocation_id: "c".repeat(32),
            generation_id: "generation-1".to_owned(),
        };
        assert_eq!(
            validate_reflection_lease_for_boot(&lease, now, boot_id).unwrap(),
            nonce_sha256
        );
    }

    #[test]
    fn scheduled_reflection_lease_rejects_a_foreign_runtime_group() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("reflection.json");
        let now = super::unix_millis();
        let nonce = "d".repeat(64);
        let nonce_sha256 = sha256(nonce.as_bytes());
        let lease = ReflectionLease {
            schema: super::REFLECTION_LEASE_SCHEMA.to_owned(),
            lease_kind: LeaseKind::ScheduledReflection.as_str().to_owned(),
            created_at_unix_ms: now,
            expires_at_unix_ms: now.saturating_add(3 * 60 * 60 * 1_000),
            reason: "scheduled_reflection".to_owned(),
            owner: "immutable_astrid_edge_reflection_guard".to_owned(),
            lease_id: format!("reflection-{}", &nonce_sha256[..24]),
            nonce,
            host_boot_id: "00000000-0000-0000-0000-000000000001".to_owned(),
            service_invocation_id: "e".repeat(32),
            generation_id: "generation-1".to_owned(),
        };
        fs::write(&path, serde_json::to_vec(&lease).unwrap()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o440)).unwrap();
        let metadata = fs::metadata(&path).unwrap();
        let foreign_gid = metadata.gid().wrapping_add(1);
        assert!(
            read_bound_lease_for_owner(
                &path,
                LeaseKind::ScheduledReflection,
                now,
                metadata.uid(),
                foreign_gid,
            )
            .is_err()
        );
    }

    #[test]
    fn transition_and_reflection_can_never_be_selected_together() {
        fn bound(kind: LeaseKind) -> BoundLease {
            BoundLease {
                kind,
                lease_id: "lease".to_owned(),
                created_at_unix_ms: 1,
                expires_at_unix_ms: 2,
                generation_id: None,
                payload_sha256: "a".repeat(64),
                nonce_sha256: "b".repeat(64),
            }
        }
        assert!(
            select_active_lease(
                LeaseState::Active(bound(LeaseKind::GenerationTransition)),
                LeaseState::Active(bound(LeaseKind::ScheduledReflection)),
            )
            .is_err()
        );
        let selected = select_active_lease(
            LeaseState::Expired,
            LeaseState::Active(bound(LeaseKind::ScheduledReflection)),
        )
        .unwrap()
        .expect("reflection selected");
        assert_eq!(selected.kind, LeaseKind::ScheduledReflection);
    }

    #[test]
    fn core_ack_v2_exposes_the_exact_common_lease_binding() {
        let ack = CoreAck {
            schema: super::ACK_SCHEMA,
            role: "core",
            lease_schema: super::REFLECTION_LEASE_SCHEMA,
            lease_kind: LeaseKind::ScheduledReflection.as_str(),
            lease_id: "reflection-aaaaaaaaaaaaaaaaaaaaaaaa",
            lease_nonce_sha256: &"b".repeat(64),
            lease_payload_sha256: &"c".repeat(64),
            generation_id: "generation-1",
            blocked_since_unix_ms: 1,
            acknowledged_at_unix_ms: 2,
            pid: 3,
            process_start_ticks: 4,
            authority: super::ACK_AUTHORITY,
            ipc_user_input_blocked: true,
            active_conversations: 0,
            active_sessions: 0,
            active_tools: 0,
            active_llm_requests: 0,
            drain_barrier_sequence: 5,
        };
        let value = serde_json::to_value(ack).unwrap();
        assert_eq!(
            value.get("schema").and_then(serde_json::Value::as_str),
            Some("astrid.edge.maintenance_ack.v2")
        );
        assert_eq!(
            value
                .get("lease_schema")
                .and_then(serde_json::Value::as_str),
            Some("astrid.edge_scheduled_reflection.lease.v1")
        );
        assert_eq!(
            value.get("lease_kind").and_then(serde_json::Value::as_str),
            Some("scheduled_reflection")
        );
        assert_eq!(
            value
                .get("active_llm_requests")
                .and_then(serde_json::Value::as_u64),
            Some(0)
        );
    }
}
