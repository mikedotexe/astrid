//! Trusted host execution plus a separate systemd-transient candidate boundary.
//!
//! Neither path accepts shell text, inherited environment, or model-selected
//! commands. Candidate code never falls back to the direct host runner.

use std::collections::BTreeMap;
#[cfg(any(target_os = "linux", test))]
use std::collections::BTreeSet;
#[cfg(target_os = "linux")]
use std::ffi::CString;
#[cfg(target_os = "linux")]
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
#[cfg(target_os = "linux")]
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::config::{Config, TrustedExecutable};
use crate::fs_guard::{canonical_json, sha256};
use crate::{Error, Result};

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub label: &'static str,
    pub executable: TrustedExecutable,
    pub arguments: Vec<String>,
    pub current_dir: PathBuf,
    pub environment: BTreeMap<String, String>,
    pub timeout: Duration,
    pub run_as_uid: Option<u32>,
    pub run_as_gid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandReceipt {
    pub label: String,
    pub execution_boundary: CommandExecutionBoundary,
    pub executable_sha256: String,
    pub argv_sha256: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommandExecutionBoundary {
    TrustedHost,
    CandidateTransient,
}

pub trait NativeRunner {
    fn run(&mut self, spec: &CommandSpec) -> Result<CommandReceipt>;

    fn run_monitored(
        &mut self,
        spec: &CommandSpec,
        monitor: &mut dyn FnMut() -> Result<()>,
    ) -> Result<CommandReceipt> {
        monitor()?;
        self.run(spec)
    }

    fn run_capture(
        &mut self,
        spec: &CommandSpec,
        maximum: u64,
    ) -> Result<(CommandReceipt, Vec<u8>)> {
        let _ = (spec, maximum);
        Err(Error::new("native runner does not support bounded capture"))
    }
}

/// Candidate execution is a separate authority from trusted host execution.
///
/// There is intentionally no default implementation: a test double or future
/// runner cannot silently fall back to `NativeRunner::run` and execute
/// candidate-controlled code in the rescue helper's host namespace.
pub trait CandidateRunner: NativeRunner {
    fn run_candidate_monitored(
        &mut self,
        config: &Config,
        transaction_root: &Path,
        spec: &CommandSpec,
        monitor: &mut dyn FnMut() -> Result<()>,
    ) -> Result<CommandReceipt>;

    fn run_candidate_capture(
        &mut self,
        config: &Config,
        transaction_root: &Path,
        spec: &CommandSpec,
        maximum: u64,
    ) -> Result<(CommandReceipt, Vec<u8>)>;

    fn spawn_candidate(
        &mut self,
        config: &Config,
        transaction_root: &Path,
        spec: &CommandSpec,
    ) -> Result<CandidateProcess>;
}

/// A candidate process is the trusted `systemd-run` wrapper plus the exact
/// transient service name. Termination always targets the service cgroup,
/// never only the wrapper process.
pub struct CandidateProcess {
    #[cfg(target_os = "linux")]
    child: Option<std::process::Child>,
    #[cfg(target_os = "linux")]
    unit_name: String,
    #[cfg(target_os = "linux")]
    systemctl: TrustedExecutable,
    #[cfg(target_os = "linux")]
    finished: bool,
    #[cfg(not(target_os = "linux"))]
    _unsupported: (),
}

impl CandidateProcess {
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        #[cfg(target_os = "linux")]
        {
            let Some(child) = self.child.as_mut() else {
                return Err(Error::new("candidate transient wrapper is absent"));
            };
            let status = child.try_wait()?;
            if status.is_some() {
                self.finish_after_wrapper_exit()?;
            }
            Ok(status)
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(Error::new(
                "candidate transient execution requires the Linux appliance target",
            ))
        }
    }

    pub fn terminate(&mut self) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            if self.finished {
                return Ok(());
            }
            request_unit_stop(&self.systemctl, &self.unit_name)?;
            let Some(child) = self.child.as_mut() else {
                return Err(Error::new("candidate transient wrapper is absent"));
            };
            let started = Instant::now();
            loop {
                if child.try_wait()?.is_some() {
                    break;
                }
                if started.elapsed() >= Duration::from_secs(5) {
                    kill_process_group(child)?;
                    let _ = child.wait()?;
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
            prove_unit_collected(&self.systemctl, &self.unit_name)?;
            unregister_candidate_unit(&self.unit_name)?;
            self.finished = true;
            Ok(())
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(Error::new(
                "candidate transient execution requires the Linux appliance target",
            ))
        }
    }

    #[cfg(target_os = "linux")]
    fn take_stdout(&mut self) -> Result<std::process::ChildStdout> {
        self.child
            .as_mut()
            .and_then(|child| child.stdout.take())
            .ok_or_else(|| Error::new("candidate transient capture pipe is unavailable"))
    }

    #[cfg(target_os = "linux")]
    fn finish_after_wrapper_exit(&mut self) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        prove_unit_collected(&self.systemctl, &self.unit_name)?;
        unregister_candidate_unit(&self.unit_name)?;
        self.finished = true;
        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl Drop for CandidateProcess {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        // Explicit callers receive proof-bearing cleanup errors. Drop is only
        // the crash/early-return safety net and therefore performs a bounded
        // best effort without pretending that cleanup was verified.
        let _ = request_unit_stop(&self.systemctl, &self.unit_name);
        if let Some(child) = self.child.as_mut() {
            let _ = kill_process_group(child);
            let _ = child.wait();
        }
        let _ = prove_unit_collected(&self.systemctl, &self.unit_name);
        let _ = unregister_candidate_unit(&self.unit_name);
    }
}

pub struct SystemRunner;

impl NativeRunner for SystemRunner {
    fn run(&mut self, spec: &CommandSpec) -> Result<CommandReceipt> {
        Self::run_internal(spec, None)
    }

    fn run_monitored(
        &mut self,
        spec: &CommandSpec,
        monitor: &mut dyn FnMut() -> Result<()>,
    ) -> Result<CommandReceipt> {
        monitor()?;
        Self::run_internal(spec, Some(monitor))
    }

    fn run_capture(
        &mut self,
        spec: &CommandSpec,
        maximum: u64,
    ) -> Result<(CommandReceipt, Vec<u8>)> {
        Self::run_capture_internal(spec, maximum)
    }
}

impl CandidateRunner for SystemRunner {
    fn run_candidate_monitored(
        &mut self,
        config: &Config,
        transaction_root: &Path,
        spec: &CommandSpec,
        monitor: &mut dyn FnMut() -> Result<()>,
    ) -> Result<CommandReceipt> {
        monitor()?;
        #[cfg(target_os = "linux")]
        {
            let started = Instant::now();
            let mut process = spawn_transient_candidate(config, transaction_root, spec, true)?;
            let outcome =
                wait_for_transient_candidate(&mut process, started, spec.timeout, Some(monitor));
            let cleanup = process.terminate();
            let (exit_code, timed_out) = complete_candidate(outcome, cleanup)?;
            candidate_receipt(spec, started, exit_code, timed_out)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (config, transaction_root, spec);
            Err(Error::new(
                "candidate transient execution requires the Linux appliance target",
            ))
        }
    }

    fn run_candidate_capture(
        &mut self,
        config: &Config,
        transaction_root: &Path,
        spec: &CommandSpec,
        maximum: u64,
    ) -> Result<(CommandReceipt, Vec<u8>)> {
        if maximum == 0 || maximum > 16 * 1024 * 1024 {
            return Err(Error::new("bounded capture limit is outside policy"));
        }
        #[cfg(target_os = "linux")]
        {
            let started = Instant::now();
            let mut process = self.spawn_candidate(config, transaction_root, spec)?;
            let stdout = process.take_stdout()?;
            let (sender, receiver) = mpsc::sync_channel(1);
            let reader = match thread::Builder::new()
                .name("astrid-candidate-capture".to_owned())
                .spawn(move || sender.send(read_bounded_and_drain(stdout, maximum)))
            {
                Ok(reader) => reader,
                Err(error) => {
                    process.terminate()?;
                    return Err(Error::new(format!(
                        "cannot start candidate capture reader: {error}"
                    )));
                },
            };
            let outcome = wait_for_transient_candidate(&mut process, started, spec.timeout, None);
            let cleanup = process.terminate();
            let (exit_code, timed_out) = complete_candidate(outcome, cleanup)?;
            let bytes = receiver
                .recv()
                .map_err(|_| Error::new("candidate capture reader failed"))??;
            reader
                .join()
                .map_err(|_| Error::new("candidate capture reader panicked"))?
                .map_err(|_| Error::new("candidate capture result was not received"))?;
            Ok((
                candidate_receipt(spec, started, exit_code, timed_out)?,
                bytes,
            ))
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (config, transaction_root, spec, maximum);
            Err(Error::new(
                "candidate transient execution requires the Linux appliance target",
            ))
        }
    }

    fn spawn_candidate(
        &mut self,
        config: &Config,
        transaction_root: &Path,
        spec: &CommandSpec,
    ) -> Result<CandidateProcess> {
        #[cfg(target_os = "linux")]
        {
            spawn_transient_candidate(config, transaction_root, spec, false)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (config, transaction_root, spec);
            Err(Error::new(
                "candidate transient execution requires the Linux appliance target",
            ))
        }
    }
}

#[cfg(any(target_os = "linux", test))]
const CANDIDATE_UNIT_PREFIX: &str = "astrid-edge-candidate-";
#[cfg(any(target_os = "linux", test))]
const CANDIDATE_UNIT_SUFFIX: &str = ".service";
#[cfg(any(target_os = "linux", test))]
const CANDIDATE_TMPFS_BYTES: u64 = 512 * 1024 * 1024;

#[cfg(any(target_os = "linux", test))]
fn candidate_tmpfs_property(path: &str) -> String {
    format!(
        "TemporaryFileSystem={path}:rw,nodev,nosuid,noexec,size={CANDIDATE_TMPFS_BYTES},mode=1777"
    )
}
#[cfg(target_os = "linux")]
static ACTIVE_CANDIDATE_UNITS: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();
#[cfg(target_os = "linux")]
static CANDIDATE_SPAWN_LOCK: Mutex<()> = Mutex::new(());

#[cfg(target_os = "linux")]
fn spawn_transient_candidate(
    config: &Config,
    transaction_root: &Path,
    spec: &CommandSpec,
    capture_stdout: bool,
) -> Result<CandidateProcess> {
    let _spawn_guard = CANDIDATE_SPAWN_LOCK
        .lock()
        .map_err(|_| Error::new("candidate spawn lock is poisoned"))?;
    validate_candidate_transaction(config, transaction_root, spec)?;
    require_exact_builder_identity(config)?;
    require_private_network_namespace()?;
    config.executables.systemd_run.verify()?;
    config.executables.systemctl.verify()?;
    reconcile_orphan_units(config)?;

    let unit_name = format!(
        "{CANDIDATE_UNIT_PREFIX}{}{CANDIDATE_UNIT_SUFFIX}",
        uuid::Uuid::new_v4()
    );
    validate_candidate_unit_name(&unit_name)?;
    let arguments = candidate_systemd_arguments(config, transaction_root, spec, &unit_name)?;
    let mut command = Command::new(&config.executables.systemd_run.path);
    command
        .args(&arguments)
        .current_dir("/")
        .env_clear()
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("TZ", "UTC")
        .stdin(Stdio::null())
        .stdout(if capture_stdout {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stderr(Stdio::null())
        .process_group(0);
    register_candidate_unit(&unit_name)?;
    let child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            unregister_candidate_unit(&unit_name)?;
            return Err(error.into());
        },
    };
    Ok(CandidateProcess {
        child: Some(child),
        unit_name,
        systemctl: config.executables.systemctl.clone(),
        finished: false,
    })
}

#[cfg(target_os = "linux")]
fn require_exact_builder_identity(config: &Config) -> Result<()> {
    use nix::unistd::{Gid, Uid, User, getgrouplist};

    let builder_user_id = Uid::from_raw(config.identities.builder_uid);
    let primary_group_id = Gid::from_raw(config.identities.builder_gid);
    let user = User::from_uid(builder_user_id)
        .map_err(|error| Error::new(format!("cannot resolve candidate builder UID: {error}")))?
        .ok_or_else(|| Error::new("candidate builder UID has no NSS identity"))?;
    if user.name != "astrid-edge-builder"
        || user.uid != builder_user_id
        || user.gid != primary_group_id
        || user.dir != Path::new("/nonexistent")
        || user.shell != Path::new("/usr/sbin/nologin")
    {
        return Err(Error::new(
            "candidate builder NSS identity drifted from immutable policy",
        ));
    }
    let user_name = CString::new(user.name)
        .map_err(|_| Error::new("candidate builder NSS name contains a NUL byte"))?;
    let groups = getgrouplist(&user_name, primary_group_id).map_err(|error| {
        Error::new(format!(
            "cannot resolve candidate builder supplementary groups: {error}"
        ))
    })?;
    if groups.len() != 1 || groups.first() != Some(&primary_group_id) {
        return Err(Error::new(
            "candidate builder acquired supplementary group authority",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn candidate_systemd_arguments(
    config: &Config,
    transaction_root: &Path,
    spec: &CommandSpec,
    unit_name: &str,
) -> Result<Vec<String>> {
    validate_candidate_unit_name(unit_name)?;
    let namespace = format!("/proc/{}/ns/net", std::process::id());
    let memory_max = config.policy.candidate_memory_max_bytes.to_string();
    let swap_max = config.policy.candidate_memory_swap_max_bytes.to_string();
    let tasks_max = config.policy.candidate_tasks_max.to_string();
    let cpu_quota = format!("{}%", config.policy.candidate_cpu_quota_percent);
    let runtime_max = format!("{}s", spec.timeout.as_secs());
    let mut arguments = vec![
        format!("--unit={unit_name}"),
        "--wait".to_owned(),
        "--collect".to_owned(),
        "--pipe".to_owned(),
        "--quiet".to_owned(),
        "--service-type=exec".to_owned(),
        format!("--uid={}", config.identities.builder_uid),
        format!("--gid={}", config.identities.builder_gid),
        format!("--working-directory={}", spec.current_dir.display()),
    ];
    for property in [
        "Description=Astrid CPU-edge confined candidate step".to_owned(),
        "PartOf=astrid-edge-self-change-supervisor.service".to_owned(),
        "BindsTo=astrid-edge-self-change-supervisor.service".to_owned(),
        format!(
            "RootDirectory={}",
            config.roots.candidate_sandbox_root.display()
        ),
        "MountAPIVFS=yes".to_owned(),
        "PrivateDevices=yes".to_owned(),
        "NoNewPrivileges=yes".to_owned(),
        "ProtectSystem=strict".to_owned(),
        "ProtectHome=yes".to_owned(),
        format!("InaccessiblePaths=+{}", config.roots.workspace.display()),
        "ProtectClock=yes".to_owned(),
        "ProtectControlGroups=yes".to_owned(),
        "ProtectHostname=yes".to_owned(),
        "ProtectKernelLogs=yes".to_owned(),
        "ProtectKernelModules=yes".to_owned(),
        "ProtectKernelTunables=yes".to_owned(),
        "ProtectProc=invisible".to_owned(),
        "ProcSubset=pid".to_owned(),
        "CapabilityBoundingSet=".to_owned(),
        "AmbientCapabilities=".to_owned(),
        "RestrictSUIDSGID=yes".to_owned(),
        "LockPersonality=yes".to_owned(),
        "RestrictRealtime=yes".to_owned(),
        "RestrictNamespaces=yes".to_owned(),
        "RestrictAddressFamilies=AF_UNIX AF_INET".to_owned(),
        "SystemCallArchitectures=native".to_owned(),
        "SystemCallErrorNumber=EPERM".to_owned(),
        "SystemCallFilter=@system-service".to_owned(),
        "SystemCallFilter=~@clock @debug @module @mount @obsolete @raw-io @reboot @swap".to_owned(),
        "MemoryDenyWriteExecute=no".to_owned(),
        "NoExecPaths=+/".to_owned(),
        "TemporaryFileSystem=/var:ro".to_owned(),
        "TemporaryFileSystem=/media:ro".to_owned(),
        "TemporaryFileSystem=/mnt:ro".to_owned(),
        "TemporaryFileSystem=/opt:ro".to_owned(),
        "TemporaryFileSystem=/usr/local:ro".to_owned(),
        "TemporaryFileSystem=/usr/libexec:ro".to_owned(),
        candidate_tmpfs_property("/tmp"),
        candidate_tmpfs_property("/var/tmp"),
        "KillMode=control-group".to_owned(),
        "SendSIGKILL=yes".to_owned(),
        "TimeoutStopSec=5s".to_owned(),
        format!("RuntimeMaxSec={runtime_max}"),
        format!("MemoryMax={memory_max}"),
        format!("MemorySwapMax={swap_max}"),
        format!("TasksMax={tasks_max}"),
        format!("CPUQuota={cpu_quota}"),
        "LimitFSIZE=536870912".to_owned(),
        "IOWeight=100".to_owned(),
        "UMask=0077".to_owned(),
        "KeyringMode=private".to_owned(),
        "RemoveIPC=yes".to_owned(),
        format!("NetworkNamespacePath={namespace}"),
        format!("BindPaths={0}:{0}", transaction_root.display()),
        format!("ReadWritePaths=+{}", transaction_root.display()),
    ] {
        arguments.push(format!("--property={property}"));
    }

    for path in candidate_read_only_bindings(config, spec, transaction_root)? {
        arguments.push(format!(
            "--property=BindReadOnlyPaths={0}:{0}",
            path.display()
        ));
        arguments.push(format!("--property=ReadOnlyPaths=+{}", path.display()));
        arguments.push(format!("--property=ExecPaths=+{}", path.display()));
    }
    arguments.push(format!(
        "--property=ExecPaths=+{}",
        transaction_root.display()
    ));
    for (key, value) in &spec.environment {
        arguments.push(format!("--setenv={key}={value}"));
    }
    arguments.push("--".to_owned());
    arguments.push(spec.executable.path.display().to_string());
    arguments.extend(spec.arguments.iter().cloned());
    require_safe_arguments(&arguments)?;
    Ok(arguments)
}

#[cfg(target_os = "linux")]
fn candidate_read_only_bindings(
    config: &Config,
    spec: &CommandSpec,
    transaction_root: &Path,
) -> Result<BTreeSet<PathBuf>> {
    let mut paths = BTreeSet::new();
    for path in [
        "/bin",
        "/sbin",
        "/lib",
        "/lib64",
        "/usr/bin",
        "/usr/sbin",
        "/usr/lib",
        "/usr/lib64",
        "/usr/share",
    ] {
        let path = PathBuf::from(path);
        if path.exists() {
            paths.insert(path);
        }
    }
    for path in [&config.source.vendor, &config.roots.releases] {
        let canonical = fs::canonicalize(path)?;
        if canonical != *path {
            return Err(Error::new(
                "candidate read-only binding contains a linked path",
            ));
        }
        paths.insert(canonical);
    }
    let cargo_bin = config
        .executables
        .cargo
        .path
        .parent()
        .ok_or_else(|| Error::new("pinned Cargo executable has no directory"))?;
    let toolchain_root = cargo_bin
        .parent()
        .ok_or_else(|| Error::new("pinned Cargo executable has no toolchain root"))?;
    paths.insert(fs::canonicalize(toolchain_root)?);
    if !spec.executable.path.starts_with(transaction_root)
        && !paths
            .iter()
            .any(|root| spec.executable.path.starts_with(root))
    {
        paths.insert(spec.executable.path.clone());
    }
    Ok(paths)
}

#[cfg(target_os = "linux")]
fn validate_candidate_transaction(
    config: &Config,
    transaction_root: &Path,
    spec: &CommandSpec,
) -> Result<()> {
    if transaction_root.parent() != Some(config.roots.candidate_work.as_path()) {
        return Err(Error::new(
            "candidate transaction is not an exact child of the bounded work root",
        ));
    }
    let transaction_metadata = fs::symlink_metadata(transaction_root)?;
    if !transaction_metadata.is_dir() || transaction_metadata.file_type().is_symlink() {
        return Err(Error::new(
            "candidate transaction root is linked or not a directory",
        ));
    }
    let canonical_transaction = fs::canonicalize(transaction_root)?;
    if canonical_transaction != transaction_root {
        return Err(Error::new(
            "candidate transaction root contains a linked ancestor",
        ));
    }
    let current_dir = fs::canonicalize(&spec.current_dir)?;
    if current_dir != spec.current_dir || !current_dir.starts_with(transaction_root) {
        return Err(Error::new(
            "candidate current directory escaped the exact transaction root",
        ));
    }
    spec.executable.verify()?;
    let executable = fs::canonicalize(&spec.executable.path)?;
    if executable != spec.executable.path {
        return Err(Error::new(
            "candidate executable contains a linked ancestor",
        ));
    }
    let trusted = config
        .executables
        .all()
        .iter()
        .any(|trusted| trusted.path == executable);
    if !trusted
        && !executable.starts_with(transaction_root)
        && !executable.starts_with(&config.roots.releases)
    {
        return Err(Error::new(
            "candidate executable is outside signed tools, releases, and transaction output",
        ));
    }
    if (spec.run_as_uid, spec.run_as_gid)
        != (
            Some(config.identities.builder_uid),
            Some(config.identities.builder_gid),
        )
        || spec.timeout.is_zero()
        || spec.timeout > Duration::from_secs(config.policy.command_timeout_seconds)
    {
        return Err(Error::new(
            "candidate identity or command timeout escaped immutable policy",
        ));
    }
    require_safe_arguments(&spec.arguments)?;
    require_safe_environment(&spec.environment)
}

#[cfg(target_os = "linux")]
fn require_safe_environment(environment: &BTreeMap<String, String>) -> Result<()> {
    if environment.len() > 32
        || environment.iter().any(|(key, value)| {
            key.is_empty()
                || key.len() > 64
                || !key
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
                || value.len() > 4_096
                || value.contains('\0')
                || value.chars().any(char::is_control)
        })
    {
        return Err(Error::new(
            "candidate environment escaped fixed key or value bounds",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn require_private_network_namespace() -> Result<()> {
    let own = fs::read_link("/proc/self/ns/net")?;
    let init = fs::read_link("/proc/1/ns/net")?;
    if own == init {
        return Err(Error::new(
            "candidate launcher is not inside the immutable private network namespace",
        ));
    }
    let interfaces = fs::read_dir("/sys/class/net")?
        .map(|entry| {
            entry
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .map_err(Error::from)
        })
        .collect::<Result<BTreeSet<_>>>()?;
    if interfaces != BTreeSet::from(["lo".to_owned()]) {
        return Err(Error::new(
            "candidate launcher private network exposes a non-loopback interface",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn wait_for_transient_candidate(
    process: &mut CandidateProcess,
    started: Instant,
    timeout: Duration,
    mut monitor: Option<&mut dyn FnMut() -> Result<()>>,
) -> Result<(Option<i32>, bool)> {
    loop {
        if let Some(status) = process.try_wait()? {
            return Ok((status.code(), false));
        }
        if started.elapsed() >= timeout {
            return Ok((None, true));
        }
        if let Some(check) = monitor.as_deref_mut()
            && let Err(error) = check()
        {
            let message = format!(
                "candidate command aborted after execution began by immutable health monitor: {error}"
            );
            return Err(
                if error.kind() == crate::ErrorKind::DeferredInfrastructure {
                    Error::deferred(message)
                } else {
                    Error::new(message)
                },
            );
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(target_os = "linux")]
fn candidate_receipt(
    spec: &CommandSpec,
    started: Instant,
    exit_code: Option<i32>,
    timed_out: bool,
) -> Result<CommandReceipt> {
    let arguments = serde_json::json!({
        "executable": spec.executable.path,
        "arguments": spec.arguments,
        "environment": spec.environment,
        "current_dir": spec.current_dir,
        "execution_boundary": "systemd_transient_candidate_v1",
    });
    Ok(CommandReceipt {
        label: spec.label.to_owned(),
        execution_boundary: CommandExecutionBoundary::CandidateTransient,
        executable_sha256: spec.executable.sha256.clone(),
        argv_sha256: sha256(&canonical_json(&arguments)?),
        exit_code,
        timed_out,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

#[cfg(target_os = "linux")]
fn candidate_units() -> &'static Mutex<BTreeSet<String>> {
    ACTIVE_CANDIDATE_UNITS.get_or_init(|| Mutex::new(BTreeSet::new()))
}

#[cfg(target_os = "linux")]
fn register_candidate_unit(unit_name: &str) -> Result<()> {
    let mut units = candidate_units()
        .lock()
        .map_err(|_| Error::new("candidate unit registry is poisoned"))?;
    if !units.insert(unit_name.to_owned()) {
        return Err(Error::new("candidate transient unit identity was reused"));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn unregister_candidate_unit(unit_name: &str) -> Result<()> {
    let mut units = candidate_units()
        .lock()
        .map_err(|_| Error::new("candidate unit registry is poisoned"))?;
    units.remove(unit_name);
    Ok(())
}

#[cfg(any(target_os = "linux", test))]
fn validate_candidate_unit_name(unit_name: &str) -> Result<()> {
    let Some(uuid) = unit_name
        .strip_prefix(CANDIDATE_UNIT_PREFIX)
        .and_then(|name| name.strip_suffix(CANDIDATE_UNIT_SUFFIX))
    else {
        return Err(Error::new(
            "candidate transient unit name is outside policy",
        ));
    };
    let parsed = uuid::Uuid::parse_str(uuid)
        .map_err(|_| Error::new("candidate transient unit UUID is invalid"))?;
    if parsed.to_string() != uuid {
        return Err(Error::new(
            "candidate transient unit UUID is not canonical lowercase text",
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", test))]
fn parse_candidate_unit_list(output: &[u8]) -> Result<BTreeSet<String>> {
    let text = std::str::from_utf8(output)
        .map_err(|_| Error::new("systemd candidate-unit list is not UTF-8"))?;
    if text.len() > 64 * 1024 {
        return Err(Error::new("systemd candidate-unit list exceeds bound"));
    }
    let mut units = BTreeSet::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let unit = line
            .split_ascii_whitespace()
            .next()
            .ok_or_else(|| Error::new("systemd candidate-unit list contains an empty row"))?;
        validate_candidate_unit_name(unit)?;
        if !units.insert(unit.to_owned()) {
            return Err(Error::new(
                "systemd candidate-unit list contains a duplicate unit",
            ));
        }
    }
    Ok(units)
}

#[cfg(target_os = "linux")]
fn reconcile_orphan_units(config: &Config) -> Result<()> {
    let observed = list_candidate_units(config)?;
    let registered = candidate_units()
        .lock()
        .map_err(|_| Error::new("candidate unit registry is poisoned"))?
        .clone();
    let orphaned = observed
        .difference(&registered)
        .cloned()
        .collect::<Vec<_>>();
    if orphaned.is_empty() {
        return Ok(());
    }
    for unit in &orphaned {
        request_unit_stop(&config.executables.systemctl, unit)?;
        prove_unit_collected(&config.executables.systemctl, unit)?;
    }
    Err(Error::new(
        "orphaned candidate transient units were removed; transaction must be retried",
    ))
}

#[cfg(target_os = "linux")]
fn list_candidate_units(config: &Config) -> Result<BTreeSet<String>> {
    let (status, output) = run_pinned_control(
        &config.executables.systemctl,
        &[
            "list-units",
            "--all",
            "--plain",
            "--no-legend",
            "--no-pager",
            "--full",
            "astrid-edge-candidate-*.service",
        ],
        64 * 1024,
    )?;
    if !status.success() {
        return Err(Error::new(
            "cannot enumerate pre-existing candidate transient units",
        ));
    }
    parse_candidate_unit_list(&output)
}

/// Stop every helper-minted candidate transient unit and prove that none remain.
///
/// Trusted-host invariant execution calls this both before and after crossing
/// out of the candidate boundary. A killed invariant child may have left a
/// PID-1-owned transient service that is not a descendant of the helper, so a
/// process-group reap alone is not sufficient evidence.
pub fn reconcile_candidate_transients(config: &Config) -> Result<bool> {
    #[cfg(target_os = "linux")]
    {
        let _spawn_guard = CANDIDATE_SPAWN_LOCK
            .lock()
            .map_err(|_| Error::new("candidate spawn lock is poisoned"))?;
        config.executables.systemctl.verify()?;
        let registered = candidate_units()
            .lock()
            .map_err(|_| Error::new("candidate unit registry is poisoned"))?
            .clone();
        let observed = list_candidate_units(config)?;
        let targets = observed
            .union(&registered)
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut cleanup_error = None;
        for unit in &targets {
            if let Err(error) = request_unit_stop(&config.executables.systemctl, unit)
                && cleanup_error.is_none()
            {
                cleanup_error = Some(error);
            }
        }
        for unit in &targets {
            if let Err(error) = prove_unit_collected(&config.executables.systemctl, unit)
                && cleanup_error.is_none()
            {
                cleanup_error = Some(error);
            }
        }
        let remaining = list_candidate_units(config)?;
        if !remaining.is_empty() {
            return Err(Error::new(
                "candidate transient cleanup did not reach a zero-unit boundary",
            ));
        }
        let invisible_registered = registered.difference(&observed).next().is_some();
        if invisible_registered {
            return Err(Error::new(
                "candidate transient registry contained a unit PID 1 could not attest",
            ));
        }
        let mut registry = candidate_units()
            .lock()
            .map_err(|_| Error::new("candidate unit registry is poisoned"))?;
        registry.clear();
        if let Some(error) = cleanup_error {
            return Err(Error::new(format!(
                "candidate transient cleanup encountered a system-manager failure: {error}"
            )));
        }
        Ok(!targets.is_empty())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = config;
        Ok(false)
    }
}

#[cfg(target_os = "linux")]
fn request_unit_stop(systemctl: &TrustedExecutable, unit_name: &str) -> Result<()> {
    validate_candidate_unit_name(unit_name)?;
    if unit_load_state(systemctl, unit_name)?.is_none() {
        return Ok(());
    }
    let (status, _) =
        run_pinned_control(systemctl, &["stop", "--no-block", "--", unit_name], 4_096)?;
    if !status.success() {
        return Err(Error::new("cannot stop candidate transient service cgroup"));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn prove_unit_collected(systemctl: &TrustedExecutable, unit_name: &str) -> Result<()> {
    validate_candidate_unit_name(unit_name)?;
    let started = Instant::now();
    loop {
        if unit_load_state(systemctl, unit_name)?.is_none() {
            return Ok(());
        }
        let _ = run_pinned_control(systemctl, &["reset-failed", "--", unit_name], 4_096)?;
        if started.elapsed() >= Duration::from_secs(5) {
            return Err(Error::new(
                "candidate transient service was not collected after cleanup",
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(target_os = "linux")]
fn unit_load_state(systemctl: &TrustedExecutable, unit_name: &str) -> Result<Option<()>> {
    let (status, output) = run_pinned_control(
        systemctl,
        &[
            "show",
            "--property=LoadState",
            "--value",
            "--no-pager",
            "--",
            unit_name,
        ],
        4_096,
    )?;
    let value = std::str::from_utf8(&output)
        .map_err(|_| Error::new("candidate unit LoadState is not UTF-8"))?
        .trim();
    if value == "not-found" || (!status.success() && value.is_empty()) {
        return Ok(None);
    }
    if !status.success() || value != "loaded" {
        return Err(Error::new(
            "candidate transient service has an unexpected load state",
        ));
    }
    Ok(Some(()))
}

#[cfg(target_os = "linux")]
fn run_pinned_control(
    executable: &TrustedExecutable,
    arguments: &[&str],
    maximum: u64,
) -> Result<(ExitStatus, Vec<u8>)> {
    executable.verify()?;
    let owned_arguments = arguments
        .iter()
        .map(|argument| (*argument).to_owned())
        .collect::<Vec<_>>();
    require_safe_arguments(&owned_arguments)?;
    let mut command = Command::new(&executable.path);
    command
        .args(arguments)
        .current_dir("/")
        .env_clear()
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0);
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::new("systemd control capture pipe is unavailable"))?;
    let (sender, receiver) = mpsc::sync_channel(1);
    let reader = match thread::Builder::new()
        .name("astrid-systemd-control".to_owned())
        .spawn(move || sender.send(read_bounded_and_drain(stdout, maximum)))
    {
        Ok(reader) => reader,
        Err(error) => {
            kill_process_group(&mut child)?;
            let _ = child.wait()?;
            return Err(Error::new(format!(
                "cannot start systemd control reader: {error}"
            )));
        },
    };
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= Duration::from_secs(5) {
            kill_process_group(&mut child)?;
            let _ = child.wait()?;
            let _ = reader.join();
            return Err(Error::new("pinned systemd control command timed out"));
        }
        thread::sleep(Duration::from_millis(50));
    };
    let output = receiver
        .recv()
        .map_err(|_| Error::new("systemd control reader failed"))??;
    reader
        .join()
        .map_err(|_| Error::new("systemd control reader panicked"))?
        .map_err(|_| Error::new("systemd control result was not received"))?;
    Ok((status, output))
}

impl SystemRunner {
    fn run_internal(
        spec: &CommandSpec,
        monitor: Option<&mut dyn FnMut() -> Result<()>>,
    ) -> Result<CommandReceipt> {
        spec.executable.verify()?;
        require_safe_arguments(&spec.arguments)?;
        if !spec.current_dir.is_absolute() || !spec.current_dir.is_dir() {
            return Err(Error::new("native command current directory is invalid"));
        }
        prepare_descendant_boundary()?;
        let started = Instant::now();
        let mut command = Command::new(&spec.executable.path);
        command
            .args(&spec.arguments)
            .current_dir(&spec.current_dir)
            .env_clear()
            .envs(&spec.environment)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        apply_child_identity(&mut command, spec)?;
        let mut child = command.spawn()?;
        let outcome = wait_for_candidate(&mut child, started, spec.timeout, monitor);
        let cleanup = finish_candidate_processes(&mut child);
        let (exit_code, timed_out) = complete_candidate(outcome, cleanup)?;
        let arguments = serde_json::json!({
            "executable": spec.executable.path,
            "arguments": spec.arguments,
            "environment": spec.environment,
            "current_dir": spec.current_dir,
        });
        Ok(CommandReceipt {
            label: spec.label.to_owned(),
            execution_boundary: CommandExecutionBoundary::TrustedHost,
            executable_sha256: spec.executable.sha256.clone(),
            argv_sha256: sha256(&canonical_json(&arguments)?),
            exit_code,
            timed_out,
            duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        })
    }

    fn run_capture_internal(spec: &CommandSpec, maximum: u64) -> Result<(CommandReceipt, Vec<u8>)> {
        spec.executable.verify()?;
        require_safe_arguments(&spec.arguments)?;
        if maximum == 0 || maximum > 16 * 1024 * 1024 {
            return Err(Error::new("bounded capture limit is outside policy"));
        }
        if !spec.current_dir.is_absolute() || !spec.current_dir.is_dir() {
            return Err(Error::new("native command current directory is invalid"));
        }
        prepare_descendant_boundary()?;
        let started = Instant::now();
        let mut command = Command::new(&spec.executable.path);
        command
            .args(&spec.arguments)
            .current_dir(&spec.current_dir)
            .env_clear()
            .envs(&spec.environment)
            .stdin(Stdio::null())
            // Captured command output must never transit a mutable runtime
            // directory.  An anonymous pipe has no pathname for the runtime
            // identity to pre-create, replace, read, or forge.
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        apply_child_identity(&mut command, spec)?;
        let mut child = command.spawn()?;
        let Some(stdout) = child.stdout.take() else {
            finish_candidate_processes(&mut child)?;
            return Err(Error::new("native command capture pipe is unavailable"));
        };
        let (sender, receiver) = mpsc::sync_channel(1);
        let reader = match thread::Builder::new()
            .name("astrid-rescue-capture".to_owned())
            .spawn(move || sender.send(read_bounded_and_drain(stdout, maximum)))
        {
            Ok(reader) => reader,
            Err(error) => {
                finish_candidate_processes(&mut child)?;
                return Err(Error::new(format!("cannot start capture reader: {error}")));
            },
        };
        let outcome = wait_for_candidate(&mut child, started, spec.timeout, None);
        let cleanup = finish_candidate_processes(&mut child);
        let (exit_code, timed_out) = complete_candidate(outcome, cleanup)?;
        let bytes = receiver
            .recv()
            .map_err(|_| Error::new("native command capture reader failed"))??;
        reader
            .join()
            .map_err(|_| Error::new("native command capture reader panicked"))?
            .map_err(|_| Error::new("native command capture result was not received"))?;
        let arguments = serde_json::json!({
            "executable": spec.executable.path,
            "arguments": spec.arguments,
            "environment": spec.environment,
            "current_dir": spec.current_dir,
        });
        let receipt = CommandReceipt {
            label: spec.label.to_owned(),
            execution_boundary: CommandExecutionBoundary::TrustedHost,
            executable_sha256: spec.executable.sha256.clone(),
            argv_sha256: sha256(&canonical_json(&arguments)?),
            exit_code,
            timed_out,
            duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        };
        Ok((receipt, bytes))
    }
}

fn read_bounded_and_drain(mut reader: impl std::io::Read, maximum: u64) -> Result<Vec<u8>> {
    let maximum = usize::try_from(maximum)
        .map_err(|_| Error::new("bounded capture limit cannot fit in memory"))?;
    let retained_limit = maximum.saturating_add(1);
    let mut retained = Vec::with_capacity(retained_limit.min(64 * 1024));
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        if retained.len() < retained_limit {
            let remaining = retained_limit.saturating_sub(retained.len());
            retained.extend_from_slice(&buffer[..read.min(remaining)]);
        }
        // Continue draining after the retention limit. Otherwise a verbose
        // child could fill the pipe, deadlock until timeout, and obscure that
        // its output violated the immutable bound.
    }
    if retained.len() > maximum {
        return Err(Error::new("native command output exceeds capture bound"));
    }
    Ok(retained)
}

fn apply_child_identity(command: &mut Command, spec: &CommandSpec) -> Result<()> {
    match (spec.run_as_uid, spec.run_as_gid) {
        (Some(uid), Some(gid)) if uid != 0 && gid != 0 => {
            // On pinned Rust 1.94, CommandExt::uid schedules
            // setgroups(0, NULL) when no explicit supplementary groups were
            // supplied, before setgid/setuid. Do not add `groups`: the builder
            // identity is intentionally only its exact primary GID.
            command.uid(uid).gid(gid).process_group(0);
            Ok(())
        },
        (None, None) => {
            command.process_group(0);
            Ok(())
        },
        _ => Err(Error::new(
            "native child identity must supply a complete non-root UID/GID",
        )),
    }
}

fn kill_process_group(child: &mut std::process::Child) -> Result<()> {
    let pid = i32::try_from(child.id()).map_err(|_| Error::new("child process ID overflow"))?;
    let group = nix::unistd::Pid::from_raw(pid);
    match nix::sys::signal::killpg(group, nix::sys::signal::Signal::SIGKILL) {
        Ok(()) | Err(nix::errno::Errno::ESRCH) => Ok(()),
        Err(error) => Err(Error::new(format!(
            "cannot terminate candidate process group: {error}"
        ))),
    }
}

fn wait_for_candidate(
    child: &mut std::process::Child,
    started: Instant,
    timeout: Duration,
    mut monitor: Option<&mut dyn FnMut() -> Result<()>>,
) -> Result<(Option<i32>, bool)> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok((status.code(), false)),
            Ok(None) => {},
            Err(error) => return Err(error.into()),
        }
        if started.elapsed() >= timeout {
            return Ok((None, true));
        }
        if let Some(check) = monitor.as_deref_mut()
            && let Err(error) = check()
        {
            let message = format!(
                "candidate command aborted after execution began by immutable health monitor: {error}"
            );
            return Err(
                if error.kind() == crate::ErrorKind::DeferredInfrastructure {
                    Error::deferred(message)
                } else {
                    Error::new(message)
                },
            );
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn complete_candidate<T>(outcome: Result<T>, cleanup: Result<()>) -> Result<T> {
    // A clean process boundary preserves an infrastructure deferral. Failure to prove that every
    // process and descendant is gone is an integrity failure and must override the retryable
    // classification.
    cleanup?;
    outcome
}

fn finish_candidate_processes(child: &mut std::process::Child) -> Result<()> {
    let group_result = kill_process_group(child);
    let descendant_result = kill_and_reap_descendants();

    // A clean descendant boundary is the stronger invariant: report it first
    // even if the original process-group kill also failed.  The group error is
    // still returned when the complete descendant cleanup succeeded.
    descendant_result?;
    group_result
}

#[cfg(target_os = "linux")]
fn prepare_descendant_boundary() -> Result<()> {
    nix::sys::prctl::set_child_subreaper(true)
        .map_err(|error| Error::new(format!("cannot enable candidate child subreaper: {error}")))?;
    if !nix::sys::prctl::get_child_subreaper()
        .map_err(|error| Error::new(format!("cannot verify candidate child subreaper: {error}")))?
    {
        return Err(Error::new(
            "candidate child subreaper did not remain enabled",
        ));
    }
    if !direct_child_processes()?.is_empty() {
        return Err(Error::new(
            "native runner already has a child process before command start",
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
#[allow(clippy::unnecessary_wraps)]
fn prepare_descendant_boundary() -> Result<()> {
    // CPU-edge release artifacts and the immutable helper run on Linux.  Keep
    // source-level development usable elsewhere while Linux performs the
    // fail-closed subreaper proof.
    Ok(())
}

#[cfg(target_os = "linux")]
fn kill_and_reap_descendants() -> Result<()> {
    const CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);
    let started = Instant::now();

    loop {
        let children = direct_child_processes()?;
        for pid in children.iter().rev() {
            match nix::sys::signal::kill(*pid, nix::sys::signal::Signal::SIGKILL) {
                Ok(()) | Err(nix::errno::Errno::ESRCH) => {},
                Err(error) => {
                    return Err(Error::new(format!(
                        "cannot terminate escaped candidate descendant {pid}: {error}"
                    )));
                },
            }
        }

        let no_children = reap_adopted_children()?;
        if no_children && direct_child_processes()?.is_empty() {
            return Ok(());
        }
        if started.elapsed() >= CLEANUP_TIMEOUT {
            return Err(Error::new(
                "candidate descendant cleanup did not reach an empty child boundary",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(not(target_os = "linux"))]
#[allow(clippy::unnecessary_wraps)]
fn kill_and_reap_descendants() -> Result<()> {
    Ok(())
}

#[cfg(target_os = "linux")]
fn reap_adopted_children() -> Result<bool> {
    use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};

    loop {
        match waitpid(None, Some(WaitPidFlag::WNOHANG)) {
            Ok(WaitStatus::StillAlive) => return Ok(false),
            Ok(
                WaitStatus::Exited(..)
                | WaitStatus::Signaled(..)
                | WaitStatus::Stopped(..)
                | WaitStatus::Continued(..)
                | WaitStatus::PtraceEvent(..)
                | WaitStatus::PtraceSyscall(..),
            )
            | Err(nix::errno::Errno::EINTR) => {},
            Err(nix::errno::Errno::ECHILD) => return Ok(true),
            Err(error) => {
                return Err(Error::new(format!(
                    "cannot reap candidate descendants: {error}"
                )));
            },
        }
    }
}

#[cfg(target_os = "linux")]
fn direct_child_processes() -> Result<Vec<nix::unistd::Pid>> {
    let root =
        i32::try_from(std::process::id()).map_err(|_| Error::new("native process ID overflow"))?;
    Ok(direct_process_children(root)?
        .into_iter()
        .map(nix::unistd::Pid::from_raw)
        .collect())
}

#[cfg(target_os = "linux")]
fn direct_process_children(pid: i32) -> Result<BTreeSet<i32>> {
    let task_directory = PathBuf::from(format!("/proc/{pid}/task"));
    let tasks = match std::fs::read_dir(&task_directory) {
        Ok(tasks) => tasks,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(error) => {
            return Err(Error::new(format!(
                "cannot inspect candidate task directory {}: {error}",
                task_directory.display()
            )));
        },
    };
    let mut children = BTreeSet::new();
    for task in tasks {
        let task = task?;
        let children_path = task.path().join("children");
        let contents = match std::fs::read_to_string(&children_path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(Error::new(format!(
                    "cannot inspect candidate children {}: {error}",
                    children_path.display()
                )));
            },
        };
        for value in contents.split_ascii_whitespace() {
            let child = value
                .parse::<i32>()
                .map_err(|_| Error::new("candidate child list contains an invalid process ID"))?;
            if child <= 0 {
                return Err(Error::new(
                    "candidate child list contains a non-positive process ID",
                ));
            }
            children.insert(child);
        }
    }
    Ok(children)
}

pub fn require_success(receipt: &CommandReceipt) -> Result<()> {
    if receipt.timed_out || receipt.exit_code != Some(0) {
        return Err(Error::new(format!(
            "fixed native step failed: {}",
            receipt.label
        )));
    }
    Ok(())
}

fn require_safe_arguments(arguments: &[String]) -> Result<()> {
    if arguments.len() > 128
        || arguments.iter().any(|argument| {
            argument.len() > 4_096
                || argument.contains('\0')
                || argument
                    .chars()
                    .any(|character| character.is_control() && character != '\t')
        })
    {
        return Err(Error::new("fixed native command argument exceeds bounds"));
    }
    Ok(())
}

#[must_use]
pub fn fixed_environment(
    cargo_home: &Path,
    target_dir: &Path,
    rustc: &Path,
    rustfmt: &Path,
    workers: usize,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("CARGO_HOME".to_owned(), cargo_home.display().to_string()),
        ("CARGO_NET_OFFLINE".to_owned(), "true".to_owned()),
        (
            "CARGO_TARGET_DIR".to_owned(),
            target_dir.display().to_string(),
        ),
        ("CARGO_BUILD_JOBS".to_owned(), workers.to_string()),
        ("RUSTC".to_owned(), rustc.display().to_string()),
        ("RUSTFMT".to_owned(), rustfmt.display().to_string()),
        ("HOME".to_owned(), cargo_home.display().to_string()),
        ("LANG".to_owned(), "C.UTF-8".to_owned()),
        ("LC_ALL".to_owned(), "C.UTF-8".to_owned()),
        ("TZ".to_owned(), "UTC".to_owned()),
    ])
}

#[cfg(test)]
mod tests {
    use super::{
        CommandSpec, NativeRunner, SystemRunner, candidate_tmpfs_property, complete_candidate,
        parse_candidate_unit_list, read_bounded_and_drain, require_safe_arguments,
        validate_candidate_unit_name,
    };
    use crate::config::TrustedExecutable;
    use crate::fs_guard::sha256_file;
    use crate::{Error, ErrorKind};
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::{Mutex, MutexGuard, PoisonError};
    use std::time::Duration;

    static SYSTEM_RUNNER_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn system_runner_test_guard() -> MutexGuard<'static, ()> {
        SYSTEM_RUNNER_TEST_LOCK
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    #[cfg(target_os = "linux")]
    fn trusted_python_fixture() -> Option<TrustedExecutable> {
        // Distribution convenience paths such as `/usr/bin/python3` may be
        // symlinks even though their package-owned targets satisfy the
        // immutable runner policy. Resolve the fixture before constructing the
        // trusted identity; production still rejects configured symlinks.
        let path = std::fs::canonicalize("/usr/bin/python3").ok()?;
        let executable = TrustedExecutable {
            sha256: sha256_file(&path, 64 * 1024 * 1024).ok()?,
            path,
        };
        executable.verify().ok()?;
        Some(executable)
    }

    #[test]
    fn command_arguments_are_bounded_and_non_binary() {
        assert!(require_safe_arguments(&["--offline".to_owned()]).is_ok());
        assert!(require_safe_arguments(&["bad\0argument".to_owned()]).is_err());
        assert!(require_safe_arguments(&vec!["x".to_owned(); 129]).is_err());
    }

    #[test]
    fn candidate_unit_names_are_helper_minted_canonical_uuids() {
        let unit = "astrid-edge-candidate-018f5f64-8a21-7b4d-a746-91b40ecdc2c2.service";
        assert!(validate_candidate_unit_name(unit).is_ok());
        assert!(validate_candidate_unit_name("astrid-edge-candidate-core-tests.service").is_err());
        assert!(
            validate_candidate_unit_name(
                "astrid-edge-candidate-018F5F64-8A21-7B4D-A746-91B40ECDC2C2.service"
            )
            .is_err()
        );
        assert!(validate_candidate_unit_name("ssh.service").is_err());
    }

    #[test]
    fn candidate_temporary_filesystems_are_unit_private_and_bounded() {
        assert_eq!(
            candidate_tmpfs_property("/tmp"),
            "TemporaryFileSystem=/tmp:rw,nodev,nosuid,noexec,size=536870912,mode=1777"
        );
        assert_eq!(
            candidate_tmpfs_property("/var/tmp"),
            "TemporaryFileSystem=/var/tmp:rw,nodev,nosuid,noexec,size=536870912,mode=1777"
        );
    }

    #[test]
    fn orphan_unit_listing_never_accepts_unrelated_or_summary_rows() {
        let first = "astrid-edge-candidate-018f5f64-8a21-7b4d-a746-91b40ecdc2c2.service";
        let second = "astrid-edge-candidate-123e4567-e89b-12d3-a456-426614174000.service";
        let output = format!(
            "{first} loaded active running fixture\n{second} loaded failed failed fixture\n"
        );
        let parsed = parse_candidate_unit_list(output.as_bytes()).unwrap();
        assert_eq!(parsed.len(), 2);
        assert!(parse_candidate_unit_list(b"ssh.service loaded active running\n").is_err());
        assert!(parse_candidate_unit_list(b"2 loaded units listed.\n").is_err());
        let duplicate = format!("{first} loaded active\n{first} loaded active\n");
        assert!(parse_candidate_unit_list(duplicate.as_bytes()).is_err());
    }

    #[test]
    fn anonymous_capture_is_bounded_while_draining_the_complete_stream() {
        assert_eq!(
            read_bounded_and_drain(&b"bounded"[..], 7).unwrap(),
            b"bounded"
        );
        assert!(read_bounded_and_drain(&b"oversized"[..], 4).is_err());
        assert!(read_bounded_and_drain(&b""[..], 0).is_ok());
    }

    #[test]
    fn direct_native_fixture_runs_without_a_shell() {
        let _guard = system_runner_test_guard();
        let path = Path::new("/usr/bin/true");
        if !path.exists() {
            return;
        }
        let executable = TrustedExecutable {
            path: path.to_path_buf(),
            sha256: sha256_file(path, 1024 * 1024).unwrap(),
        };
        let mut runner = SystemRunner;
        let receipt = runner
            .run(&CommandSpec {
                label: "native-fixture",
                executable,
                arguments: Vec::new(),
                current_dir: std::env::temp_dir(),
                environment: BTreeMap::new(),
                timeout: Duration::from_secs(2),
                run_as_uid: None,
                run_as_gid: None,
            })
            .unwrap();
        assert_eq!(receipt.exit_code, Some(0));
        assert!(!receipt.timed_out);
    }

    #[test]
    fn pre_spawn_health_failure_is_deferred_without_executing_candidate() {
        let _guard = system_runner_test_guard();
        let touch = Path::new("/usr/bin/touch");
        if !touch.exists() {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let artifact = directory.path().join("must-not-exist");
        let executable = TrustedExecutable {
            path: touch.to_path_buf(),
            sha256: sha256_file(touch, 4 * 1024 * 1024).unwrap(),
        };
        let mut runner = SystemRunner;
        let error = runner
            .run_monitored(
                &CommandSpec {
                    label: "pre-spawn-health-defer",
                    executable,
                    arguments: vec![artifact.display().to_string()],
                    current_dir: directory.path().to_path_buf(),
                    environment: BTreeMap::new(),
                    timeout: Duration::from_secs(2),
                    run_as_uid: None,
                    run_as_gid: None,
                },
                &mut || Err(Error::deferred("synthetic disk pressure")),
            )
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::DeferredInfrastructure);
        assert!(!artifact.exists());
    }

    #[test]
    fn cleanup_integrity_controls_retry_classification() {
        let deferred = Err::<(), _>(Error::deferred("thermal pressure"));
        let error = complete_candidate(deferred, Ok(())).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::DeferredInfrastructure);

        let deferred = Err::<(), _>(Error::deferred("thermal pressure"));
        let error =
            complete_candidate(deferred, Err(Error::new("descendant remained"))).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Terminal);
        assert_eq!(error.message(), "descendant remained");
    }

    #[test]
    fn dropped_identity_has_no_inherited_supplementary_groups_when_run_as_root() {
        let _guard = system_runner_test_guard();
        if nix::unistd::geteuid().as_raw() != 0 {
            return;
        }
        let path = Path::new("/usr/bin/id");
        if !path.exists() {
            return;
        }
        let executable = TrustedExecutable {
            path: path.to_path_buf(),
            sha256: sha256_file(path, 4 * 1024 * 1024).unwrap(),
        };
        let mut runner = SystemRunner;
        let (_, output) = runner
            .run_capture(
                &CommandSpec {
                    label: "supplementary-group-fixture",
                    executable,
                    arguments: vec!["-G".to_owned()],
                    current_dir: std::env::temp_dir(),
                    environment: BTreeMap::from([("LANG".to_owned(), "C".to_owned())]),
                    timeout: Duration::from_secs(2),
                    run_as_uid: Some(65_534),
                    run_as_gid: Some(65_534),
                },
                4_096,
            )
            .unwrap();
        assert_eq!(std::str::from_utf8(&output).unwrap().trim(), "65534");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn escaped_descendant_worker() {
        let Ok(pid_file) = std::env::var("ASTRID_EDGE_DESCENDANT_WORKER_PID_FILE") else {
            return;
        };
        let artifact = std::env::var("ASTRID_EDGE_DESCENDANT_WORKER_ARTIFACT").unwrap();
        let executable = trusted_python_fixture()
            .expect("trusted Python fixture disappeared after parent validation");
        let script = concat!(
            "import os,time;exec(\"",
            "path=os.environ['PID_FILE']\\n",
            "first=os.fork()\\n",
            "if first:\\n",
            " os.waitpid(first,0)\\n",
            " while not os.path.exists(path): time.sleep(0.001)\\n",
            " os._exit(0)\\n",
            "os.setsid()\\n",
            "second=os.fork()\\n",
            "if second: os._exit(0)\\n",
            "with open(path,'w') as output:\\n",
            " output.write(str(os.getpid()))\\n",
            " output.flush()\\n",
            " os.fsync(output.fileno())\\n",
            "time.sleep(0.5)\\n",
            "with open(os.environ['ARTIFACT'],'w') as output: output.write('tampered')\\n",
            "time.sleep(60)\")",
        );
        let working_directory = Path::new(&pid_file).parent().unwrap().to_path_buf();
        let mut runner = SystemRunner;
        let receipt = runner
            .run(&CommandSpec {
                label: "escaped-descendant-worker",
                executable,
                arguments: vec!["-c".to_owned(), script.to_owned()],
                current_dir: working_directory,
                environment: BTreeMap::from([
                    ("ARTIFACT".to_owned(), artifact),
                    ("PID_FILE".to_owned(), pid_file),
                ]),
                timeout: Duration::from_secs(5),
                run_as_uid: None,
                run_as_gid: None,
            })
            .unwrap();
        assert_eq!(receipt.exit_code, Some(0));
        assert!(!receipt.timed_out);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn successful_command_cannot_leave_a_setsid_double_fork_descendant() {
        let _guard = system_runner_test_guard();
        if trusted_python_fixture().is_none() {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let pid_file = directory.path().join("escaped.pid");
        let artifact = directory.path().join("candidate-artifact");
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "native::tests::escaped_descendant_worker",
                "--nocapture",
            ])
            .env("ASTRID_EDGE_DESCENDANT_WORKER_ARTIFACT", &artifact)
            .env("ASTRID_EDGE_DESCENDANT_WORKER_PID_FILE", &pid_file)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "isolated descendant worker failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let escaped_pid = std::fs::read_to_string(&pid_file).unwrap();
        let escaped_pid = escaped_pid.trim().parse::<u32>().unwrap();
        assert!(
            !Path::new(&format!("/proc/{escaped_pid}")).exists(),
            "escaped candidate descendant remained alive after command success"
        );
        std::thread::sleep(Duration::from_millis(700));
        assert!(
            !artifact.exists(),
            "escaped candidate descendant mutated an artifact after command success"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn mid_command_health_breach_worker() {
        let Ok(pid_file) = std::env::var("ASTRID_EDGE_HEALTH_BREACH_WORKER_PID_FILE") else {
            return;
        };
        let artifact = std::env::var("ASTRID_EDGE_HEALTH_BREACH_WORKER_ARTIFACT").unwrap();
        let Some(executable) = trusted_python_fixture() else {
            panic!("trusted Python fixture disappeared after parent validation");
        };
        let script = concat!(
            "import os,time\n",
            "child=os.fork()\n",
            "if child:\n",
            " time.sleep(60)\n",
            "else:\n",
            " os.setsid()\n",
            " grand=os.fork()\n",
            " if grand: os._exit(0)\n",
            " with open(os.environ['PID_FILE'],'w') as f:\n",
            "  f.write(str(os.getpid())); f.flush(); os.fsync(f.fileno())\n",
            " time.sleep(1)\n",
            " with open(os.environ['ARTIFACT'],'w') as f: f.write('escaped')\n",
            " time.sleep(60)\n",
        );
        let mut runner = SystemRunner;
        let mut checks = 0_u8;
        let error = runner
            .run_monitored(
                &CommandSpec {
                    label: "mid-command-health-abort",
                    executable,
                    arguments: vec!["-c".to_owned(), script.to_owned()],
                    current_dir: Path::new(&pid_file).parent().unwrap().to_path_buf(),
                    environment: BTreeMap::from([
                        ("ARTIFACT".to_owned(), artifact.clone()),
                        ("PID_FILE".to_owned(), pid_file.clone()),
                    ]),
                    timeout: Duration::from_secs(30),
                    run_as_uid: None,
                    run_as_gid: None,
                },
                &mut || {
                    checks = checks.saturating_add(1);
                    if checks >= 8 {
                        Err(Error::deferred("synthetic thermal breach"))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
        assert_eq!(
            error.kind(),
            ErrorKind::DeferredInfrastructure,
            "unexpected cleanup classification: {}",
            error.message()
        );
        assert!(error.message().contains("after execution began"));
        let escaped_pid = std::fs::read_to_string(pid_file)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        assert!(!Path::new(&format!("/proc/{escaped_pid}")).exists());
        std::thread::sleep(Duration::from_millis(1_100));
        assert!(!Path::new(&artifact).exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn mid_command_health_breach_kills_process_group_and_escaped_descendants() {
        let _guard = system_runner_test_guard();
        if trusted_python_fixture().is_none() {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let pid_file = directory.path().join("escaped.pid");
        let artifact = directory.path().join("late-artifact");
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "native::tests::mid_command_health_breach_worker",
                "--nocapture",
            ])
            .env("ASTRID_EDGE_HEALTH_BREACH_WORKER_ARTIFACT", &artifact)
            .env("ASTRID_EDGE_HEALTH_BREACH_WORKER_PID_FILE", &pid_file)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "isolated health-breach worker failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let escaped_pid = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        assert!(!Path::new(&format!("/proc/{escaped_pid}")).exists());
        std::thread::sleep(Duration::from_millis(1_100));
        assert!(!artifact.exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn dropped_builder_identity_can_reach_exact_absolute_build_leaves() {
        use std::os::unix::fs::PermissionsExt as _;

        let _guard = system_runner_test_guard();
        if nix::unistd::geteuid().as_raw() != 0 {
            return;
        }
        let touch = Path::new("/usr/bin/touch");
        if !touch.exists() {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        std::os::unix::fs::chown(directory.path(), Some(0), Some(65_534)).unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o710)).unwrap();
        let scratch = directory.path().join("build-fixture");
        std::fs::create_dir(&scratch).unwrap();
        std::os::unix::fs::chown(&scratch, Some(0), Some(65_534)).unwrap();
        std::fs::set_permissions(&scratch, std::fs::Permissions::from_mode(0o710)).unwrap();
        let source = scratch.join("source");
        std::fs::create_dir(&source).unwrap();
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o555)).unwrap();
        let cargo_home = scratch.join("cargo-home");
        let target = scratch.join("target");
        for leaf in [&cargo_home, &target] {
            std::fs::create_dir(leaf).unwrap();
            std::os::unix::fs::chown(leaf, Some(65_534), Some(65_534)).unwrap();
            std::fs::set_permissions(leaf, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let cargo_probe = cargo_home.join("absolute-path-probe");
        let target_probe = target.join("absolute-path-probe");
        let executable = TrustedExecutable {
            path: touch.to_path_buf(),
            sha256: sha256_file(touch, 4 * 1024 * 1024).unwrap(),
        };
        let mut runner = SystemRunner;
        let receipt = runner
            .run(&CommandSpec {
                label: "builder-absolute-path-fixture",
                executable,
                arguments: vec![
                    cargo_probe.display().to_string(),
                    target_probe.display().to_string(),
                ],
                current_dir: source,
                environment: BTreeMap::from([
                    ("CARGO_HOME".to_owned(), cargo_home.display().to_string()),
                    ("CARGO_TARGET_DIR".to_owned(), target.display().to_string()),
                ]),
                timeout: Duration::from_secs(5),
                run_as_uid: Some(65_534),
                run_as_gid: Some(65_534),
            })
            .unwrap();
        assert_eq!(receipt.exit_code, Some(0));
        assert!(cargo_probe.is_file());
        assert!(target_probe.is_file());
    }
}
