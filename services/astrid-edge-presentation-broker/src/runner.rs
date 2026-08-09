use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use nix::sys::signal::{Signal, killpg};
use nix::unistd::{Pid, Uid};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::BrokerConfig;
use crate::contract::{
    BrokerRequest, CandidatePresentation, ENVELOPE_SCHEMA, PRESENTATION_AUTHORITY,
    PRESENTATION_PROVENANCE, PresentationEnvelope, PresentationStatus, ProjectionInput,
    valid_identifier,
};
use crate::fs_guard::{
    canonical_sha256, read_stable_regular, read_utf8_line, sha256, valid_hex64,
    verify_immutable_ancestors,
};
use crate::{Error, Result};

const GENERATION_SCHEMA: &str = "astrid.edge_self_change.generation.v1";
const INITIAL_GENERATION_SCHEMA: &str = "astrid.edge_self_change.initial_generation.v1";
const RUNTIME_PROJECTION_SCHEMA: &str = "astrid.edge_self_change.runtime_projections.v1";
const RUNTIME_PROJECTION_AUTHORITY: &str = "immutable_root_validated_profile_projection:v1";
const RUNTIME_PROJECTION_PATH: &str = "metadata/runtime-projections.json";
const REPORT_ENTRYPOINTS: &[&str] = &[
    "scripts/astrid_at_a_glance.py",
    "scripts/report_edge_activity.py",
    "scripts/report_edge_appliance.py",
];
const MAX_GENERATION_MANIFEST_BYTES: usize = 16 * 1024 * 1024;
const MAX_RUNTIME_PROJECTION_BYTES: usize = 128 * 1024;
const MAX_ENTRYPOINT_BYTES: usize = 4 * 1024 * 1024;

pub trait Clock: Send + Sync {
    fn now_unix_ms(&self) -> u64;
}

#[derive(Debug, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| {
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostSecurityState {
    pub effective_uid: u32,
    pub no_new_privileges: bool,
    pub seccomp_filter: bool,
    pub cgroup_v2_memory_max_bytes: Option<u64>,
}

impl HostSecurityState {
    /// Read the kernel-enforced service identity, seccomp, and cgroup state.
    ///
    /// # Errors
    ///
    /// Returns an error when Linux process or cgroup-v2 state cannot be read
    /// or is malformed.
    pub fn detect() -> Result<Self> {
        let status = fs::read_to_string("/proc/self/status")?;
        let no_new_privileges = status.lines().any(|line| line == "NoNewPrivs:\t1");
        let seccomp_filter = status.lines().any(|line| line == "Seccomp:\t2");
        Ok(Self {
            effective_uid: Uid::effective().as_raw(),
            no_new_privileges,
            seccomp_filter,
            cgroup_v2_memory_max_bytes: cgroup_v2_memory_max()?,
        })
    }

    fn validate(self, configured_maximum: u64) -> Result<()> {
        if self.effective_uid == 0 || !self.no_new_privileges || !self.seccomp_filter {
            return Err(Error::new(
                "presentation broker requires an unprivileged no-new-privileges seccomp service",
            ));
        }
        let actual = self
            .cgroup_v2_memory_max_bytes
            .ok_or_else(|| Error::new("presentation broker cgroup-v2 memory cap is unavailable"))?;
        if actual == 0 || actual > configured_maximum {
            return Err(Error::new(
                "presentation broker cgroup-v2 memory cap exceeds immutable policy",
            ));
        }
        Ok(())
    }
}

fn cgroup_v2_memory_max() -> Result<Option<u64>> {
    let membership = fs::read_to_string("/proc/self/cgroup")?;
    let relative = membership
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .ok_or_else(|| Error::new("unified cgroup-v2 membership is unavailable"))?;
    if relative.contains('\0') || relative.split('/').any(|part| part == "..") {
        return Err(Error::new("cgroup-v2 membership is malformed"));
    }
    let memory_path = Path::new("/sys/fs/cgroup")
        .join(relative.trim_start_matches('/'))
        .join("memory.max");
    let value = fs::read_to_string(memory_path)?;
    let value = value.trim();
    if value == "max" {
        return Ok(None);
    }
    let bytes = value
        .parse::<u64>()
        .map_err(|_| Error::new("cgroup-v2 memory.max is malformed"))?;
    Ok(Some(bytes))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GenerationBinding {
    generation_id: String,
    manifest_sha256: String,
    payload_sha256: String,
    report_projection_sha256: Option<String>,
    entrypoint: String,
    entrypoint_sha256: String,
    entrypoint_path: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CandidateGenerationManifest {
    schema: String,
    appliance_id: String,
    generation_id: String,
    build_id: String,
    candidate_id: String,
    candidate_sha256: String,
    base_generation: String,
    bundle_sha256: String,
    tests_sha256: String,
    target: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InitialGenerationManifest {
    schema: String,
    appliance_id: String,
    version: String,
    target: String,
    inventory: Vec<InitialFile>,
    authority: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InitialFile {
    path: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ReportProjection {
    path: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeProjectionManifest {
    schema: String,
    appliance_id: String,
    profile_source: String,
    profile_source_sha256: String,
    profile_projection_sha256: String,
    profile_mutated_by_candidate: bool,
    report_scripts: Vec<ReportProjection>,
    report_projection_sha256: String,
    reports_mutated_by_candidate: bool,
    authority: String,
}

pub struct Broker<C: Clock = SystemClock> {
    config: BrokerConfig,
    security: HostSecurityState,
    clock: C,
    require_root_owner: bool,
}

impl Broker<SystemClock> {
    /// Construct a production broker after all immutable and kernel checks.
    ///
    /// # Errors
    ///
    /// Returns an error if configuration, executable identity, service
    /// privilege, seccomp, or cgroup memory enforcement is invalid.
    pub fn new(config: BrokerConfig) -> Result<Self> {
        config.validate(true)?;
        let security = HostSecurityState::detect()?;
        security.validate(config.policy.memory_max_bytes)?;
        Ok(Self {
            config,
            security,
            clock: SystemClock,
            require_root_owner: true,
        })
    }
}

impl<C: Clock> Broker<C> {
    #[cfg(test)]
    fn for_test(config: BrokerConfig, security: HostSecurityState, clock: C) -> Result<Self> {
        config.validate(false)?;
        security.validate(config.policy.memory_max_bytes)?;
        Ok(Self {
            config,
            security,
            clock,
            require_root_owner: false,
        })
    }

    #[allow(clippy::too_many_lines)] // One ordered fail-closed state machine keeps fallback precedence reviewable.
    pub fn run(&self, request: &BrokerRequest) -> PresentationEnvelope {
        let started = Instant::now();
        let entrypoint = request.view.entrypoint().to_owned();
        if request.validate().is_err() {
            return self.failure(
                request,
                started,
                PresentationStatus::RequestRejected,
                "request_contract_rejected",
                &entrypoint,
                None,
                None,
                ProcessMetadata::default(),
            );
        }
        if self
            .security
            .validate(self.config.policy.memory_max_bytes)
            .is_err()
        {
            return self.failure(
                request,
                started,
                PresentationStatus::SandboxRejected,
                "kernel_sandbox_preflight_rejected",
                &entrypoint,
                None,
                None,
                ProcessMetadata::default(),
            );
        }
        let projection = &request.projection;
        if projection.validate(&self.config.appliance_id).is_err() {
            return self.failure(
                request,
                started,
                PresentationStatus::ProjectionRejected,
                "sanitized_projection_rejected",
                &entrypoint,
                None,
                None,
                ProcessMetadata::default(),
            );
        }
        let Ok(generation) = self.read_generation(&entrypoint) else {
            return self.failure(
                request,
                started,
                PresentationStatus::GenerationRejected,
                "active_generation_binding_rejected",
                &entrypoint,
                None,
                Some(projection),
                ProcessMetadata::default(),
            );
        };
        let Ok(process) = self.execute(request, &generation, projection) else {
            return self.failure(
                request,
                started,
                PresentationStatus::SpawnFailed,
                "fixed_entrypoint_spawn_failed",
                &entrypoint,
                Some(&generation),
                Some(projection),
                ProcessMetadata::default(),
            );
        };
        if self.read_generation(&entrypoint).ok().as_ref() != Some(&generation) {
            return self.failure(
                request,
                started,
                PresentationStatus::GenerationChanged,
                "generation_changed_during_execution",
                &entrypoint,
                Some(&generation),
                Some(projection),
                process.metadata(),
            );
        }
        if process.timed_out {
            return self.failure(
                request,
                started,
                PresentationStatus::TimedOut,
                "candidate_entrypoint_timeout",
                &entrypoint,
                Some(&generation),
                Some(projection),
                process.metadata(),
            );
        }
        if process.stdout_exceeded || process.stderr_exceeded {
            return self.failure(
                request,
                started,
                PresentationStatus::OutputExceeded,
                "candidate_entrypoint_output_bound_exceeded",
                &entrypoint,
                Some(&generation),
                Some(projection),
                process.metadata(),
            );
        }
        if !process.status.is_some_and(|status| status.success()) {
            return self.failure(
                request,
                started,
                PresentationStatus::ProcessFailed,
                "candidate_entrypoint_nonzero_or_signaled",
                &entrypoint,
                Some(&generation),
                Some(projection),
                process.metadata(),
            );
        }
        let presentation = serde_json::from_slice::<CandidatePresentation>(&process.stdout)
            .ok()
            .filter(|value| value.validate(request.view).is_ok());
        let Some(presentation) = presentation else {
            return self.failure(
                request,
                started,
                PresentationStatus::OutputRejected,
                "candidate_entrypoint_json_contract_rejected",
                &entrypoint,
                Some(&generation),
                Some(projection),
                process.metadata(),
            );
        };
        self.completed(
            request,
            started,
            &generation,
            projection,
            process.metadata(),
            presentation,
        )
    }

    fn read_generation(&self, entrypoint: &str) -> Result<GenerationBinding> {
        if self.require_root_owner {
            verify_immutable_ancestors(&self.config.generation_binding, true)?;
            verify_immutable_ancestors(&self.config.active_link, true)?;
        }
        let generation_id = read_utf8_line(
            &self.config.generation_binding,
            256,
            self.require_root_owner,
        )?;
        if !valid_identifier(&generation_id, 128) {
            return Err(Error::new("active generation identifier is invalid"));
        }
        let link = fs::symlink_metadata(&self.config.active_link)?;
        if !link.file_type().is_symlink() || (self.require_root_owner && link.uid() != 0) {
            return Err(Error::new("active generation pointer is not immutable"));
        }
        let generation_root = self.config.releases_root.join(&generation_id);
        let resolved_link = fs::canonicalize(&self.config.active_link)?;
        let resolved_generation = fs::canonicalize(&generation_root)?;
        if resolved_link != resolved_generation {
            return Err(Error::new("active generation pointer and binding differ"));
        }
        let root_metadata = fs::symlink_metadata(&generation_root)?;
        if !root_metadata.is_dir()
            || root_metadata.file_type().is_symlink()
            || root_metadata.permissions().mode() & 0o022 != 0
            || (self.require_root_owner && root_metadata.uid() != 0)
        {
            return Err(Error::new("active generation root is mutable or linked"));
        }
        let manifest_path = generation_root.join(".astrid-edge-generation.json");
        let (manifest_bytes, _) = read_stable_regular(
            &manifest_path,
            MAX_GENERATION_MANIFEST_BYTES,
            self.require_root_owner,
        )?;
        let manifest_sha256 = sha256(&manifest_bytes);
        let (payload_sha256, initial_inventory, candidate_generation) = parse_generation_manifest(
            &manifest_bytes,
            &generation_id,
            &self.config.appliance_id,
            &self.config.target,
            entrypoint,
        )?;
        let report_projection_sha256 = candidate_generation
            .then(|| self.read_runtime_projection(&generation_root))
            .transpose()?;
        let entrypoint_path = generation_root.join(entrypoint);
        if self.require_root_owner {
            verify_immutable_ancestors(&entrypoint_path, true)?;
        }
        let (script, metadata) = read_stable_regular(
            &entrypoint_path,
            MAX_ENTRYPOINT_BYTES,
            self.require_root_owner,
        )?;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(Error::new(
                "candidate presentation entrypoint is not executable",
            ));
        }
        let entrypoint_sha256 = sha256(&script);
        if let Some(inventory) = initial_inventory {
            verify_initial_entrypoint(&inventory, entrypoint, &script, &entrypoint_sha256)?;
        }
        Ok(GenerationBinding {
            generation_id,
            manifest_sha256,
            payload_sha256,
            report_projection_sha256,
            entrypoint: entrypoint.to_owned(),
            entrypoint_sha256,
            entrypoint_path,
        })
    }

    fn read_runtime_projection(&self, generation_root: &Path) -> Result<String> {
        let manifest_path = generation_root.join(RUNTIME_PROJECTION_PATH);
        if self.require_root_owner {
            verify_immutable_ancestors(&manifest_path, true)?;
        }
        let (manifest_bytes, _) = read_stable_regular(
            &manifest_path,
            MAX_RUNTIME_PROJECTION_BYTES,
            self.require_root_owner,
        )?;
        let manifest: RuntimeProjectionManifest = serde_json::from_slice(&manifest_bytes)?;
        let expected_profile = if self.config.appliance_id.starts_with("icp") {
            "packaging/appliances/icp-j3455-8g.env"
        } else {
            "packaging/appliances/avado-i3-16g.env"
        };
        if manifest.schema != RUNTIME_PROJECTION_SCHEMA
            || manifest.appliance_id != self.config.appliance_id
            || manifest.profile_source != expected_profile
            || !valid_hex64(&manifest.profile_source_sha256)
            || !valid_hex64(&manifest.profile_projection_sha256)
            || !valid_hex64(&manifest.report_projection_sha256)
            || manifest.authority != RUNTIME_PROJECTION_AUTHORITY
        {
            return Err(Error::new(
                "candidate runtime projection identity or authority failed",
            ));
        }
        let reports = REPORT_ENTRYPOINTS
            .iter()
            .map(|relative| {
                let path = generation_root.join(relative);
                if self.require_root_owner {
                    verify_immutable_ancestors(&path, true)?;
                }
                let (bytes, metadata) =
                    read_stable_regular(&path, MAX_ENTRYPOINT_BYTES, self.require_root_owner)?;
                if metadata.permissions().mode() & 0o111 == 0 {
                    return Err(Error::new(
                        "candidate report projection contains a non-executable entrypoint",
                    ));
                }
                Ok(ReportProjection {
                    path: (*relative).to_owned(),
                    size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                    sha256: sha256(&bytes),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let digest = canonical_sha256(&reports)?;
        if manifest.report_scripts != reports || manifest.report_projection_sha256 != digest {
            return Err(Error::new(
                "candidate runtime report projection differs from release bytes",
            ));
        }
        Ok(digest)
    }

    fn execute(
        &self,
        request: &BrokerRequest,
        generation: &GenerationBinding,
        projection: &ProjectionInput,
    ) -> Result<ProcessOutput> {
        let mut command = Command::new(&self.config.python.path);
        command
            .arg("-I")
            .arg("-E")
            .arg("-s")
            .arg(&generation.entrypoint_path)
            .arg("--candidate-presentation")
            .arg("--input-stdin")
            .arg("--window-minutes")
            .arg(request.window_minutes.to_string())
            .arg("--limit")
            .arg(request.limit.to_string())
            .arg("--format")
            .arg("json")
            .current_dir("/")
            .env_clear()
            .env("HOME", "/nonexistent")
            .env("LANG", "C.UTF-8")
            .env("LC_ALL", "C.UTF-8")
            .env("PATH", "/nonexistent")
            .env("PYTHONHASHSEED", "0")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let mut child = command.spawn()?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| Error::new("candidate stdin pipe is unavailable"))?;
        let projection_bytes = crate::fs_guard::canonical_json(projection)?;
        if projection_bytes.len() > self.config.policy.maximum_projection_bytes {
            return Err(Error::new("candidate projection exceeded its byte bound"));
        }
        let stdin_writer = thread::spawn(move || -> std::io::Result<()> {
            stdin.write_all(&projection_bytes)?;
            stdin.flush()
        });
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::new("candidate stdout pipe is unavailable"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| Error::new("candidate stderr pipe is unavailable"))?;
        let stdout_limit = self.config.policy.maximum_stdout_bytes;
        let stderr_limit = self.config.policy.maximum_stderr_bytes;
        let output_limit_hit = Arc::new(AtomicBool::new(false));
        let stdout_limit_hit = Arc::clone(&output_limit_hit);
        let stderr_limit_hit = Arc::clone(&output_limit_hit);
        let stdout_reader =
            thread::spawn(move || bounded_drain(stdout, stdout_limit, &stdout_limit_hit));
        let stderr_reader =
            thread::spawn(move || bounded_drain(stderr, stderr_limit, &stderr_limit_hit));
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(self.config.policy.timeout_ms))
            .ok_or_else(|| Error::new("candidate deadline overflow"))?;
        let mut timed_out = false;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break Some(status);
            }
            if output_limit_hit.load(Ordering::Acquire) {
                let process_id = i32::try_from(child.id())
                    .map_err(|_| Error::new("candidate process identifier overflow"))?;
                let _ = killpg(Pid::from_raw(process_id), Signal::SIGKILL);
                let _ = child.kill();
                break child.wait().ok();
            }
            if Instant::now() >= deadline {
                timed_out = true;
                let process_id = i32::try_from(child.id())
                    .map_err(|_| Error::new("candidate process identifier overflow"))?;
                let _ = killpg(Pid::from_raw(process_id), Signal::SIGKILL);
                let _ = child.kill();
                break child.wait().ok();
            }
            thread::sleep(Duration::from_millis(10));
        };
        let stdout = stdout_reader
            .join()
            .map_err(|_| Error::new("candidate stdout reader failed"))??;
        let stderr = stderr_reader
            .join()
            .map_err(|_| Error::new("candidate stderr reader failed"))??;
        stdin_writer
            .join()
            .map_err(|_| Error::new("candidate stdin writer failed"))??;
        Ok(ProcessOutput {
            status,
            timed_out,
            stdout_exceeded: stdout.exceeded,
            stderr_exceeded: stderr.exceeded,
            stdout: stdout.retained,
            stderr: stderr.retained,
            stdout_bytes: stdout.total,
            stderr_bytes: stderr.total,
        })
    }

    fn completed(
        &self,
        request: &BrokerRequest,
        started: Instant,
        generation: &GenerationBinding,
        projection: &ProjectionInput,
        process: ProcessMetadata,
        presentation: CandidatePresentation,
    ) -> PresentationEnvelope {
        let presentation_sha256 = canonical_sha256(&presentation).ok();
        let mut envelope = self.base_envelope(
            request,
            started,
            PresentationStatus::Completed,
            None,
            &generation.entrypoint,
            Some(generation),
            Some(projection),
            process,
        );
        envelope.presentation_sha256 = presentation_sha256;
        envelope.presentation = Some(presentation);
        let _ = envelope.seal();
        envelope
    }

    #[allow(clippy::too_many_arguments)]
    fn failure(
        &self,
        request: &BrokerRequest,
        started: Instant,
        status: PresentationStatus,
        failure_class: &str,
        entrypoint: &str,
        generation: Option<&GenerationBinding>,
        projection: Option<&ProjectionInput>,
        process: ProcessMetadata,
    ) -> PresentationEnvelope {
        self.base_envelope(
            request,
            started,
            status,
            Some(failure_class),
            entrypoint,
            generation,
            projection,
            process,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn base_envelope(
        &self,
        request: &BrokerRequest,
        started: Instant,
        status: PresentationStatus,
        failure_class: Option<&str>,
        entrypoint: &str,
        generation: Option<&GenerationBinding>,
        projection: Option<&ProjectionInput>,
        process: ProcessMetadata,
    ) -> PresentationEnvelope {
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let mut value = PresentationEnvelope {
            schema: ENVELOPE_SCHEMA.to_owned(),
            provenance: PRESENTATION_PROVENANCE.to_owned(),
            authority: PRESENTATION_AUTHORITY.to_owned(),
            appliance_id: self.config.appliance_id.clone(),
            target: self.config.target.clone(),
            view: request.view,
            generated_at_unix_ms: self.clock.now_unix_ms(),
            status,
            failure_class: failure_class.map(str::to_owned),
            generation_id: generation.map(|value| value.generation_id.clone()),
            generation_manifest_sha256: generation.map(|value| value.manifest_sha256.clone()),
            generation_payload_sha256: generation.map(|value| value.payload_sha256.clone()),
            report_projection_sha256: generation
                .and_then(|value| value.report_projection_sha256.clone()),
            entrypoint: entrypoint.to_owned(),
            entrypoint_sha256: generation.map(|value| value.entrypoint_sha256.clone()),
            projection_sha256: projection.map(|value| value.projection_sha256.clone()),
            duration_ms,
            exit_code: process.exit_code,
            stdout_bytes: process.stdout_bytes,
            stderr_bytes: process.stderr_bytes,
            stderr_sha256: process.stderr_sha256,
            timeout_ms: self.config.policy.timeout_ms,
            memory_max_bytes: self
                .security
                .cgroup_v2_memory_max_bytes
                .unwrap_or(self.config.policy.memory_max_bytes),
            output_max_bytes: self.config.policy.maximum_stdout_bytes as u64,
            presentation_sha256: None,
            presentation: None,
            binding_sha256: String::new(),
        };
        let _ = value.seal();
        value
    }
}

fn parse_generation_manifest(
    bytes: &[u8],
    generation_id: &str,
    appliance_id: &str,
    target: &str,
    entrypoint: &str,
) -> Result<(String, Option<Vec<InitialFile>>, bool)> {
    let raw: Value = serde_json::from_slice(bytes)?;
    match raw.get("schema").and_then(Value::as_str) {
        Some(GENERATION_SCHEMA) => {
            let value: CandidateGenerationManifest = serde_json::from_value(raw)?;
            validate_candidate_manifest(&value, generation_id, appliance_id, target)?;
            Ok((value.bundle_sha256, None, true))
        },
        Some(INITIAL_GENERATION_SCHEMA) => {
            let value: InitialGenerationManifest = serde_json::from_value(raw)?;
            validate_initial_manifest(&value, appliance_id, target, entrypoint)?;
            let payload_sha256 = canonical_sha256(&value.inventory)?;
            Ok((payload_sha256, Some(value.inventory), false))
        },
        _ => Err(Error::new(
            "active generation manifest schema is unsupported",
        )),
    }
}

fn verify_initial_entrypoint(
    inventory: &[InitialFile],
    entrypoint: &str,
    script: &[u8],
    script_sha256: &str,
) -> Result<()> {
    let exact = inventory
        .iter()
        .find(|item| item.path == entrypoint)
        .ok_or_else(|| Error::new("initial generation omitted report entrypoint"))?;
    if exact.size != script.len() as u64 || exact.sha256 != script_sha256 {
        return Err(Error::new(
            "initial generation report inventory binding failed",
        ));
    }
    Ok(())
}

fn validate_candidate_manifest(
    value: &CandidateGenerationManifest,
    generation_id: &str,
    appliance_id: &str,
    target: &str,
) -> Result<()> {
    if value.schema != GENERATION_SCHEMA
        || value.appliance_id != appliance_id
        || value.generation_id != generation_id
        || value.target != target
        || !valid_identifier(&value.build_id, 128)
        || !valid_identifier(&value.candidate_id, 128)
        || !valid_identifier(&value.base_generation, 128)
        || !valid_hex64(&value.candidate_sha256)
        || !valid_hex64(&value.bundle_sha256)
        || !valid_hex64(&value.tests_sha256)
    {
        return Err(Error::new("candidate generation manifest failed"));
    }
    Ok(())
}

fn validate_initial_manifest(
    value: &InitialGenerationManifest,
    appliance_id: &str,
    target: &str,
    entrypoint: &str,
) -> Result<()> {
    if value.schema != INITIAL_GENERATION_SCHEMA
        || value.appliance_id != appliance_id
        || value.target != target
        || value.authority != "operator_packaged_initial_generation_not_model_candidate"
        || value.version.is_empty()
        || value.version.len() > 128
        || value.inventory.is_empty()
        || value.inventory.len() > 50_000
    {
        return Err(Error::new("initial generation manifest failed"));
    }
    let mut found = false;
    let mut previous = "";
    for item in &value.inventory {
        if item.path.as_str() <= previous
            || item.path.starts_with('/')
            || item
                .path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || item.size > 512 * 1024 * 1024
            || !valid_hex64(&item.sha256)
        {
            return Err(Error::new("initial generation inventory failed"));
        }
        found |= item.path == entrypoint;
        previous = &item.path;
    }
    if !found {
        return Err(Error::new(
            "initial generation omitted presentation entrypoint",
        ));
    }
    Ok(())
}

#[derive(Debug, Default)]
struct ProcessMetadata {
    exit_code: Option<i32>,
    stdout_bytes: u64,
    stderr_bytes: u64,
    stderr_sha256: Option<String>,
}

struct ProcessOutput {
    status: Option<ExitStatus>,
    timed_out: bool,
    stdout_exceeded: bool,
    stderr_exceeded: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_bytes: u64,
    stderr_bytes: u64,
}

impl ProcessOutput {
    fn metadata(&self) -> ProcessMetadata {
        ProcessMetadata {
            exit_code: self.status.and_then(|status| status.code()),
            stdout_bytes: self.stdout_bytes,
            stderr_bytes: self.stderr_bytes,
            stderr_sha256: (!self.stderr.is_empty()).then(|| sha256(&self.stderr)),
        }
    }
}

struct BoundedOutput {
    retained: Vec<u8>,
    total: u64,
    exceeded: bool,
}

fn bounded_drain<R: Read>(
    mut reader: R,
    maximum: usize,
    limit_hit: &AtomicBool,
) -> Result<BoundedOutput> {
    let mut retained = Vec::with_capacity(maximum.min(8_192));
    let mut total = 0_u64;
    let mut buffer = [0_u8; 8_192];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total = total.saturating_add(count as u64);
        if total > maximum as u64 {
            limit_hit.store(true, Ordering::Release);
        }
        if retained.len() < maximum {
            let available = maximum.saturating_sub(retained.len());
            let keep = count.min(available);
            retained.extend_from_slice(&buffer[..keep]);
        }
    }
    Ok(BoundedOutput {
        retained,
        total,
        exceeded: total > maximum as u64,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use tempfile::TempDir;

    use super::*;
    use crate::config::{BrokerPolicy, CONFIG_SCHEMA, SANDBOX_CONTRACT, TrustedExecutable};
    use crate::contract::{BrokerRequest, PROJECTION_SCHEMA, ProjectionFact, REQUEST_SCHEMA};
    use crate::fs_guard::canonical_sha256_with_blank_field;

    #[derive(Clone, Copy)]
    struct FixedClock;

    impl Clock for FixedClock {
        fn now_unix_ms(&self) -> u64 {
            1_234
        }
    }

    struct Fixture {
        temporary: TempDir,
        config: BrokerConfig,
    }

    impl Fixture {
        fn new(script: &str) -> Self {
            let temporary = tempfile::tempdir().unwrap();
            let root = temporary.path();
            let releases = root.join("releases");
            let generation = releases.join("gen-a");
            fs::create_dir_all(generation.join("scripts")).unwrap();
            let entrypoints = [
                "report_edge_appliance.py",
                "report_edge_activity.py",
                "astrid_at_a_glance.py",
            ];
            let mut inventory = Vec::new();
            for name in entrypoints {
                let path = generation.join("scripts").join(name);
                fs::write(&path, script).unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o555)).unwrap();
                inventory.push(serde_json::json!({
                    "path": format!("scripts/{name}"),
                    "size": script.len(),
                    "sha256": sha256(script.as_bytes()),
                }));
            }
            inventory.sort_by(|left, right| left["path"].as_str().cmp(&right["path"].as_str()));
            let manifest = serde_json::json!({
                "schema": INITIAL_GENERATION_SCHEMA,
                "appliance_id": "avado",
                "version": "test",
                "target": "x86_64-unknown-linux-gnu",
                "inventory": inventory,
                "authority": "operator_packaged_initial_generation_not_model_candidate",
            });
            let manifest_path = generation.join(".astrid-edge-generation.json");
            fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
            fs::set_permissions(&manifest_path, fs::Permissions::from_mode(0o444)).unwrap();
            fs::set_permissions(
                generation.join("scripts"),
                fs::Permissions::from_mode(0o555),
            )
            .unwrap();
            fs::set_permissions(&generation, fs::Permissions::from_mode(0o555)).unwrap();
            fs::set_permissions(&releases, fs::Permissions::from_mode(0o555)).unwrap();
            symlink("releases/gen-a", root.join("current")).unwrap();
            let binding = root.join("current-generation");
            fs::write(&binding, "gen-a\n").unwrap();
            fs::set_permissions(&binding, fs::Permissions::from_mode(0o444)).unwrap();
            let python = fs::canonicalize(PathBuf::from("/usr/bin/python3")).unwrap();
            let config = BrokerConfig {
                schema: CONFIG_SCHEMA.to_owned(),
                appliance_id: "avado".to_owned(),
                target: "x86_64-unknown-linux-gnu".to_owned(),
                releases_root: releases,
                active_link: root.join("current"),
                generation_binding: binding,
                python: TrustedExecutable {
                    sha256: sha256(&fs::read(&python).unwrap()),
                    path: python,
                },
                policy: BrokerPolicy {
                    timeout_ms: 1_000,
                    maximum_request_bytes: 1_024,
                    maximum_projection_bytes: 64 * 1024,
                    maximum_stdout_bytes: 8 * 1024,
                    maximum_stderr_bytes: 2 * 1024,
                    memory_max_bytes: 256 * 1024 * 1024,
                    require_cgroup_v2: true,
                    sandbox_contract: SANDBOX_CONTRACT.to_owned(),
                },
            };
            Self { temporary, config }
        }

        fn broker(&self) -> Broker<FixedClock> {
            Broker::for_test(
                self.config.clone(),
                HostSecurityState {
                    effective_uid: 9_999,
                    no_new_privileges: true,
                    seccomp_filter: true,
                    cgroup_v2_memory_max_bytes: Some(128 * 1024 * 1024),
                },
                FixedClock,
            )
            .unwrap()
        }

        fn convert_to_candidate_generation(&self) -> String {
            let generation = self.config.releases_root.join("gen-a");
            fs::set_permissions(&generation, fs::Permissions::from_mode(0o755)).unwrap();
            let report_scripts = REPORT_ENTRYPOINTS
                .iter()
                .map(|relative| {
                    let bytes = fs::read(generation.join(relative)).unwrap();
                    ReportProjection {
                        path: (*relative).to_owned(),
                        size: u64::try_from(bytes.len()).unwrap(),
                        sha256: sha256(&bytes),
                    }
                })
                .collect::<Vec<_>>();
            let report_projection_sha256 = canonical_sha256(&report_scripts).unwrap();
            let projection = RuntimeProjectionManifest {
                schema: RUNTIME_PROJECTION_SCHEMA.to_owned(),
                appliance_id: "avado".to_owned(),
                profile_source: "packaging/appliances/avado-i3-16g.env".to_owned(),
                profile_source_sha256: "1".repeat(64),
                profile_projection_sha256: "2".repeat(64),
                profile_mutated_by_candidate: false,
                report_scripts,
                report_projection_sha256: report_projection_sha256.clone(),
                reports_mutated_by_candidate: true,
                authority: RUNTIME_PROJECTION_AUTHORITY.to_owned(),
            };
            let metadata = generation.join("metadata");
            fs::create_dir(&metadata).unwrap();
            let projection_path = metadata.join("runtime-projections.json");
            fs::write(&projection_path, serde_json::to_vec(&projection).unwrap()).unwrap();
            fs::set_permissions(&projection_path, fs::Permissions::from_mode(0o444)).unwrap();
            fs::set_permissions(&metadata, fs::Permissions::from_mode(0o555)).unwrap();

            let manifest_path = generation.join(".astrid-edge-generation.json");
            fs::set_permissions(&manifest_path, fs::Permissions::from_mode(0o644)).unwrap();
            let manifest = serde_json::json!({
                "schema": GENERATION_SCHEMA,
                "appliance_id": "avado",
                "generation_id": "gen-a",
                "build_id": "build-a",
                "candidate_id": "candidate-a",
                "candidate_sha256": "3".repeat(64),
                "base_generation": "gen-base",
                "bundle_sha256": "4".repeat(64),
                "tests_sha256": "5".repeat(64),
                "target": "x86_64-unknown-linux-gnu",
            });
            fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
            fs::set_permissions(&manifest_path, fs::Permissions::from_mode(0o444)).unwrap();
            fs::set_permissions(&generation, fs::Permissions::from_mode(0o555)).unwrap();
            report_projection_sha256
        }
    }

    fn request(view: crate::PresentationView) -> BrokerRequest {
        BrokerRequest {
            schema: REQUEST_SCHEMA.to_owned(),
            view,
            window_minutes: 60,
            limit: 10,
            projection: test_projection(),
        }
    }

    fn test_projection() -> ProjectionInput {
        let mut projection = ProjectionInput {
            schema: PROJECTION_SCHEMA.to_owned(),
            appliance_id: "avado".to_owned(),
            generated_at_unix_ms: 1,
            source: "immutable_operator_reports_sanitized_projection".to_owned(),
            source_sha256: "a".repeat(64),
            facts: vec![ProjectionFact {
                key: "fill".to_owned(),
                value: "68%".to_owned(),
                provenance: "trusted_report".to_owned(),
            }],
            recent_activity: vec![],
            projection_sha256: String::new(),
        };
        projection.projection_sha256 =
            canonical_sha256_with_blank_field(&projection, "projection_sha256").unwrap();
        projection
    }

    fn script(body: &str) -> String {
        format!(
            "#!/usr/bin/python3\nimport json\nprint(json.dumps({body},sort_keys=True,separators=(',',':')))\n"
        )
    }

    fn successful_script(view: &str) -> String {
        script(&format!(
            "{{'schema':'astrid.edge_candidate_presentation.content.v1','view':'{view}','title':'Candidate view','summary':'untrusted layout','sections':[{{'heading':'State','lines':['fill 68%']}}]}}"
        ))
    }

    #[test]
    fn completed_output_is_bound_to_generation_script_and_projection() {
        let fixture = Fixture::new(&successful_script("appliance"));
        let output = fixture
            .broker()
            .run(&request(crate::PresentationView::Appliance));
        assert_eq!(output.status, PresentationStatus::Completed);
        assert_eq!(output.provenance, PRESENTATION_PROVENANCE);
        assert_eq!(output.generation_id.as_deref(), Some("gen-a"));
        assert!(output.entrypoint_sha256.is_some());
        assert!(output.projection_sha256.is_some());
        output.validate_binding().unwrap();
    }

    #[test]
    fn candidate_output_binds_all_three_reports_and_runtime_projection() {
        let fixture = Fixture::new(&successful_script("appliance"));
        let report_projection_sha256 = fixture.convert_to_candidate_generation();
        let output = fixture
            .broker()
            .run(&request(crate::PresentationView::Appliance));
        assert_eq!(output.status, PresentationStatus::Completed);
        assert_eq!(
            output.report_projection_sha256.as_deref(),
            Some(report_projection_sha256.as_str())
        );
        output.validate_binding().unwrap();

        let unselected = fixture
            .config
            .releases_root
            .join("gen-a/scripts/report_edge_activity.py");
        fs::set_permissions(&unselected, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(&unselected, successful_script("activity")).unwrap();
        fs::set_permissions(&unselected, fs::Permissions::from_mode(0o555)).unwrap();
        let rejected = fixture
            .broker()
            .run(&request(crate::PresentationView::Appliance));
        assert_eq!(rejected.status, PresentationStatus::GenerationRejected);
        assert!(rejected.report_projection_sha256.is_none());
    }

    #[test]
    fn candidate_projection_manifest_tamper_and_cross_appliance_reuse_fail_closed() {
        for field in ["report_projection_sha256", "appliance_id"] {
            let fixture = Fixture::new(&successful_script("appliance"));
            fixture.convert_to_candidate_generation();
            let projection = fixture
                .config
                .releases_root
                .join("gen-a/metadata/runtime-projections.json");
            fs::set_permissions(&projection, fs::Permissions::from_mode(0o644)).unwrap();
            let mut value: Value = serde_json::from_slice(&fs::read(&projection).unwrap()).unwrap();
            value[field] = Value::String(if field == "appliance_id" {
                "icp".to_owned()
            } else {
                "f".repeat(64)
            });
            fs::write(&projection, serde_json::to_vec(&value).unwrap()).unwrap();
            fs::set_permissions(&projection, fs::Permissions::from_mode(0o444)).unwrap();
            let output = fixture
                .broker()
                .run(&request(crate::PresentationView::Appliance));
            assert_eq!(output.status, PresentationStatus::GenerationRejected);
            assert!(output.presentation.is_none());
        }

        let fixture = Fixture::new(&successful_script("appliance"));
        fixture.convert_to_candidate_generation();
        let unselected = fixture
            .config
            .releases_root
            .join("gen-a/scripts/report_edge_activity.py");
        fs::set_permissions(&unselected, fs::Permissions::from_mode(0o444)).unwrap();
        let output = fixture
            .broker()
            .run(&request(crate::PresentationView::Appliance));
        assert_eq!(output.status, PresentationStatus::GenerationRejected);
    }

    #[test]
    fn wrong_view_terminal_output_and_nonzero_exit_are_nonpresenting_fallbacks() {
        let wrong = Fixture::new(&successful_script("activity"));
        let output = wrong
            .broker()
            .run(&request(crate::PresentationView::Appliance));
        assert_eq!(output.status, PresentationStatus::OutputRejected);
        assert!(output.presentation.is_none());

        let terminal = Fixture::new(&script(
            "{'schema':'astrid.edge_candidate_presentation.content.v1','view':'appliance','title':'bad\\u001b','summary':'x','sections':[]}",
        ));
        assert_eq!(
            terminal
                .broker()
                .run(&request(crate::PresentationView::Appliance))
                .status,
            PresentationStatus::OutputRejected
        );

        let failed = Fixture::new("#!/usr/bin/python3\nraise SystemExit(7)\n");
        assert_eq!(
            failed
                .broker()
                .run(&request(crate::PresentationView::Appliance))
                .status,
            PresentationStatus::ProcessFailed
        );
    }

    #[test]
    fn timeout_and_oversized_output_are_bounded_and_never_retained() {
        let mut sleeping = Fixture::new("#!/usr/bin/python3\nimport time\ntime.sleep(5)\n");
        sleeping.config.policy.timeout_ms = 100;
        let started = Instant::now();
        let output = sleeping
            .broker()
            .run(&request(crate::PresentationView::Appliance));
        assert_eq!(output.status, PresentationStatus::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(output.presentation.is_none());

        let mut loud = Fixture::new("#!/usr/bin/python3\nprint('x' * 20000)\n");
        loud.config.policy.maximum_stdout_bytes = 1_024;
        let output = loud
            .broker()
            .run(&request(crate::PresentationView::Appliance));
        assert_eq!(output.status, PresentationStatus::OutputExceeded);
        assert_eq!(output.output_max_bytes, 1_024);
        assert!(output.stdout_bytes > 1_024);
        assert!(output.presentation.is_none());
    }

    #[test]
    fn forged_projection_is_rejected_without_running_candidate_code() {
        let fixture = Fixture::new(&successful_script("appliance"));
        let mut request = request(crate::PresentationView::Appliance);
        request.projection.facts[0].value = "forged".to_owned();
        let output = fixture.broker().run(&request);
        assert_eq!(output.status, PresentationStatus::ProjectionRejected);
        assert!(output.presentation.is_none());
        let _ = fixture.temporary.path();
    }

    #[test]
    fn sandbox_preflight_rejects_root_no_seccomp_and_excess_memory() {
        let fixture = Fixture::new(&successful_script("appliance"));
        for security in [
            HostSecurityState {
                effective_uid: 0,
                no_new_privileges: true,
                seccomp_filter: true,
                cgroup_v2_memory_max_bytes: Some(128 * 1024 * 1024),
            },
            HostSecurityState {
                effective_uid: 9_999,
                no_new_privileges: true,
                seccomp_filter: false,
                cgroup_v2_memory_max_bytes: Some(128 * 1024 * 1024),
            },
            HostSecurityState {
                effective_uid: 9_999,
                no_new_privileges: true,
                seccomp_filter: true,
                cgroup_v2_memory_max_bytes: Some(300 * 1024 * 1024),
            },
        ] {
            assert!(Broker::for_test(fixture.config.clone(), security, FixedClock).is_err());
        }
    }

    #[test]
    fn no_paths_commands_or_environment_can_be_selected_by_request() {
        let encoded = br#"{"schema":"astrid.edge_candidate_presentation.request.v1","view":"appliance","window_minutes":60,"limit":10,"path":"/etc/shadow"}"#;
        assert!(serde_json::from_slice::<BrokerRequest>(encoded).is_err());
        let encoded = br#"{"schema":"astrid.edge_candidate_presentation.request.v1","view":"shell","window_minutes":60,"limit":10}"#;
        assert!(serde_json::from_slice::<BrokerRequest>(encoded).is_err());
    }

    #[test]
    fn generation_hash_or_pointer_tampering_is_rejected() {
        let fixture = Fixture::new(&successful_script("appliance"));
        let report = fixture
            .config
            .releases_root
            .join("gen-a/scripts/report_edge_appliance.py");
        fs::set_permissions(&report, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(
            &report,
            successful_script("appliance").replace("68%", "69%"),
        )
        .unwrap();
        fs::set_permissions(&report, fs::Permissions::from_mode(0o555)).unwrap();
        assert_eq!(
            fixture
                .broker()
                .run(&request(crate::PresentationView::Appliance))
                .status,
            PresentationStatus::GenerationRejected
        );

        let pointer = Fixture::new(&successful_script("appliance"));
        fs::remove_file(&pointer.config.active_link).unwrap();
        symlink("releases/missing", &pointer.config.active_link).unwrap();
        assert_eq!(
            pointer
                .broker()
                .run(&request(crate::PresentationView::Appliance))
                .status,
            PresentationStatus::GenerationRejected
        );
    }

    #[test]
    fn otherwise_identical_cross_appliance_generation_is_rejected() {
        let fixture = Fixture::new(&successful_script("appliance"));
        let manifest = fixture
            .config
            .releases_root
            .join("gen-a/.astrid-edge-generation.json");
        fs::set_permissions(&manifest, fs::Permissions::from_mode(0o644)).unwrap();
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        value["appliance_id"] = serde_json::Value::String("icp".to_owned());
        fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
        fs::set_permissions(&manifest, fs::Permissions::from_mode(0o444)).unwrap();
        let output = fixture
            .broker()
            .run(&request(crate::PresentationView::Appliance));
        assert_eq!(output.status, PresentationStatus::GenerationRejected);
        assert!(output.presentation.is_none());
    }

    #[test]
    fn generation_switch_during_execution_discards_candidate_output() {
        let fixture = Fixture::new(
            "#!/usr/bin/python3\nimport json,time\ntime.sleep(.2)\nprint(json.dumps({'schema':'astrid.edge_candidate_presentation.content.v1','view':'appliance','title':'late','summary':'must be discarded','sections':[]}))\n",
        );
        let broker = fixture.broker();
        let request = request(crate::PresentationView::Appliance);
        let handle = thread::spawn(move || broker.run(&request));
        thread::sleep(Duration::from_millis(50));
        fs::set_permissions(
            &fixture.config.generation_binding,
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        fs::write(&fixture.config.generation_binding, "gen-b\n").unwrap();
        fs::set_permissions(
            &fixture.config.generation_binding,
            fs::Permissions::from_mode(0o444),
        )
        .unwrap();
        let output = handle.join().unwrap();
        assert_eq!(output.status, PresentationStatus::GenerationChanged);
        assert!(output.presentation.is_none());
        output.validate_binding().unwrap();
    }

    #[test]
    fn input_and_output_hashes_detect_post_creation_tampering() {
        let fixture = Fixture::new(&successful_script("appliance"));
        let mut output = fixture
            .broker()
            .run(&request(crate::PresentationView::Appliance));
        output.presentation.as_mut().unwrap().summary = "forged".to_owned();
        assert!(output.validate_binding().is_err());
    }

    #[test]
    fn bounded_drain_counts_without_retaining_excess() {
        let limit_hit = AtomicBool::new(false);
        let value = bounded_drain(&b"abcdef"[..], 3, &limit_hit).unwrap();
        assert_eq!(value.retained, b"abc");
        assert_eq!(value.total, 6);
        assert!(value.exceeded);
        assert!(limit_hit.load(Ordering::Acquire));
    }

    #[test]
    fn executable_arguments_are_fixed_by_view() {
        assert_eq!(
            crate::PresentationView::AtAGlance.entrypoint(),
            "scripts/astrid_at_a_glance.py"
        );
        assert_eq!(
            crate::PresentationView::Activity.entrypoint(),
            "scripts/report_edge_activity.py"
        );
        let mut routed = [
            crate::PresentationView::Appliance.entrypoint(),
            crate::PresentationView::Activity.entrypoint(),
            crate::PresentationView::AtAGlance.entrypoint(),
        ];
        routed.sort_unstable();
        assert_eq!(routed, REPORT_ENTRYPOINTS);
    }

    fn _assert_path(_path: &Path) {}
}
