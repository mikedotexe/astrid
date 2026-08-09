//! Candidate-coupled, isolated core/edge replay used by the immutable package gate.

use std::collections::BTreeMap;
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::io::{Read, Write};
#[cfg(any(target_os = "linux", test))]
use std::net::SocketAddr;
#[cfg(target_os = "linux")]
use std::net::{TcpListener, TcpStream};
#[cfg(target_os = "linux")]
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
#[cfg(target_os = "linux")]
use std::thread;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

use serde::Serialize;
#[cfg(any(target_os = "linux", test))]
use serde_json::Value;
#[cfg(target_os = "linux")]
use serde_json::json;

use crate::config::Config;
#[cfg(target_os = "linux")]
use crate::fs_guard::{canonical_json, ensure_within, read_json, sha256, sha256_file};
#[cfg(target_os = "linux")]
use crate::native::{CandidateProcess, CandidateRunner, CommandSpec, require_success};
#[cfg(target_os = "linux")]
use crate::transition::read_generation_binding;
use crate::{Error, Result};

#[cfg(target_os = "linux")]
const SHADOW_SEED: u64 = 0x05A5_71D1;
#[cfg(target_os = "linux")]
const INPUT_SAMPLES: usize = 128;
#[cfg(any(target_os = "linux", test))]
const TELEMETRY_SAMPLES: usize = 4;
#[cfg(target_os = "linux")]
const MAX_BINARY_BYTES: u64 = 512 * 1024 * 1024;
#[cfg(target_os = "linux")]
const JIT_CAPSULE_ID: &str = "astrid-capsule-edge-context";
#[cfg(target_os = "linux")]
const MAX_JIT_CAPSULE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[allow(clippy::struct_excessive_bools)] // Explicit negative authority claims are audit evidence.
pub struct CandidateShadowEvidence {
    pub schema: &'static str,
    pub appliance_id: String,
    pub provenance: &'static str,
    pub candidate_binary_sha256: BTreeMap<String, String>,
    pub previous_daemon_sha256: String,
    pub kernel_ingress_replay: crate::kernel_replay::KernelIngressEvidence,
    pub deterministic_input_sha256: String,
    pub input_samples: usize,
    pub reservoir_dimensions: usize,
    pub telemetry_samples: usize,
    pub fill_minimum: f64,
    pub fill_mean: f64,
    pub fill_maximum: f64,
    pub candidate_state_opened: bool,
    pub previous_generation_reopened_candidate_state: bool,
    pub wasm_jit_capsule_id: &'static str,
    pub candidate_wasm_jit_loaded: bool,
    pub previous_generation_wasm_jit_loaded: bool,
    pub wasm_jit_authority: &'static str,
    pub migration_authority: &'static str,
    pub production_workspace_write_authority: bool,
    pub production_workspace_path_exposed: bool,
    pub production_workspace_write_probe_denied: bool,
    pub continuity_or_reservoir_admission: bool,
    pub candidate_process_groups_reaped: bool,
    pub evidence_sha256: String,
}

/// Run sealed candidate binaries only inside the builder-owned disposable root.
///
/// The candidate daemon first opens and initializes a copied empty fixture. The
/// exact previous daemon then reopens that candidate-touched fixture. This is a
/// concrete dual-reader gate: a migration that prevents the prior generation
/// from opening the state is never eligible for autonomous activation.
#[cfg(target_os = "linux")]
#[allow(clippy::too_many_lines)] // Candidate/core/ESN binding is one fail-closed replay transaction.
pub fn run(config: &Config, bundle: &Path, replay_root: &Path) -> Result<CandidateShadowEvidence> {
    ensure_within(&config.roots.candidate_work, bundle, true)?;
    ensure_within(&config.roots.candidate_work, replay_root, false)?;
    let transaction_root = bundle
        .parent()
        .filter(|parent| parent.parent() == Some(config.roots.candidate_work.as_path()))
        .ok_or_else(|| Error::new("candidate shadow bundle has no exact transaction root"))?;
    if replay_root.exists() || replay_root.is_symlink() {
        return Err(Error::new("candidate shadow replay root already exists"));
    }
    assert_production_workspace_denied(config)?;
    probe_production_workspace_denial(config, transaction_root)?;
    fs::create_dir(replay_root)?;
    prepare_owned_directory(
        replay_root,
        config.identities.builder_uid,
        config.identities.builder_gid,
    )?;

    let candidate_daemon = bundle.join("astrid-daemon");
    let candidate_edge = bundle.join("astrid-edge-runtime");
    let candidate_cli = bundle.join("astrid");
    let base_generation = read_generation_binding(config, true)?;
    let previous_daemon = config
        .roots
        .releases
        .join(&base_generation)
        .join("astrid-daemon");
    let previous_cli = config.roots.releases.join(&base_generation).join("astrid");
    let mut binary_hashes = BTreeMap::new();
    for (name, path) in [
        ("astrid", &candidate_cli),
        ("astrid-daemon", &candidate_daemon),
        ("astrid-edge-runtime", &candidate_edge),
    ] {
        binary_hashes.insert(name.to_owned(), sha256_file(path, MAX_BINARY_BYTES)?);
    }
    let previous_daemon_sha256 = sha256_file(&previous_daemon, MAX_BINARY_BYTES)?;

    let state_root = replay_root.join("dual-reader-state");
    let home = state_root.join("home");
    let core_workspace = state_root.join("workspace");
    prepare_owned_directory(
        &state_root,
        config.identities.builder_uid,
        config.identities.builder_gid,
    )?;
    prepare_owned_directory(
        &home,
        config.identities.builder_uid,
        config.identities.builder_gid,
    )?;
    prepare_owned_directory(
        &core_workspace,
        config.identities.builder_uid,
        config.identities.builder_gid,
    )?;
    fs::write(
        core_workspace.join("SHADOW_FIXTURE.json"),
        canonical_json(&json!({
            "schema": "astrid.edge_rescue_helper.shadow_fixture.v1",
            "authority": "immutable_sanitized_fixture_no_appliance_memory",
            "base_generation": base_generation,
        }))?,
    )?;
    set_owner(
        &core_workspace.join("SHADOW_FIXTURE.json"),
        config.identities.builder_uid,
        config.identities.builder_gid,
        0o600,
    )?;
    install_jit_capsule_fixture(config, bundle, &home)?;
    let candidate_daemon_sha256 = binary_hashes
        .get("astrid-daemon")
        .ok_or_else(|| Error::new("candidate daemon hash is absent"))?
        .clone();
    let kernel_ingress_replay = run_daemon_once(
        config,
        transaction_root,
        &candidate_daemon,
        &candidate_cli,
        &home,
        &core_workspace,
        Some(&candidate_daemon_sha256),
    )?
    .ok_or_else(|| Error::new("candidate kernel replay evidence is absent"))?;
    let candidate_state_opened = home.join("run/system.ready").is_file();
    cleanup_runtime_endpoints(&home)?;
    run_daemon_once(
        config,
        transaction_root,
        &previous_daemon,
        &previous_cli,
        &home,
        &core_workspace,
        None,
    )?;
    let previous_reopened = home.join("run/system.ready").is_file();
    cleanup_runtime_endpoints(&home)?;

    let edge_root = replay_root.join("edge");
    let edge_home = edge_root.join("home");
    let edge_workspace = edge_root.join("workspace");
    prepare_owned_directory(
        &edge_root,
        config.identities.builder_uid,
        config.identities.builder_gid,
    )?;
    prepare_owned_directory(
        &edge_home,
        config.identities.builder_uid,
        config.identities.builder_gid,
    )?;
    prepare_owned_directory(
        &edge_workspace,
        config.identities.builder_uid,
        config.identities.builder_gid,
    )?;
    let (telemetry, sensory) = reserve_loopback_pair()?;
    let mut edge = spawn_edge(
        config,
        transaction_root,
        &candidate_edge,
        &candidate_cli,
        &edge_home,
        &edge_workspace,
        telemetry,
        sensory,
    )?;
    let replay = run_reservoir_replay(&mut edge, sensory, &edge_workspace);
    terminate(&mut edge)?;
    let (input_sha256, fills) = replay?;
    if binary_hashes.get("astrid-daemon")
        != Some(&sha256_file(&candidate_daemon, MAX_BINARY_BYTES)?)
        || binary_hashes.get("astrid-edge-runtime")
            != Some(&sha256_file(&candidate_edge, MAX_BINARY_BYTES)?)
        || previous_daemon_sha256 != sha256_file(&previous_daemon, MAX_BINARY_BYTES)?
    {
        return Err(Error::new(
            "candidate or previous binary changed during shadow replay",
        ));
    }
    let fill_minimum = fills.iter().copied().fold(f64::INFINITY, f64::min);
    let fill_maximum = fills.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let denominator = f64::from(u32::try_from(fills.len()).unwrap_or(u32::MAX));
    let fill_mean = fills.iter().sum::<f64>() / denominator;
    let mut evidence = CandidateShadowEvidence {
        schema: "astrid.edge_rescue_helper.candidate_shadow.v1",
        appliance_id: config.appliance_id.clone(),
        provenance: "deterministic_machine_evidence_not_astrid_authorship",
        candidate_binary_sha256: binary_hashes,
        previous_daemon_sha256,
        kernel_ingress_replay,
        deterministic_input_sha256: input_sha256,
        input_samples: INPUT_SAMPLES,
        reservoir_dimensions: 128,
        telemetry_samples: fills.len(),
        fill_minimum,
        fill_mean,
        fill_maximum,
        candidate_state_opened,
        previous_generation_reopened_candidate_state: previous_reopened,
        wasm_jit_capsule_id: JIT_CAPSULE_ID,
        candidate_wasm_jit_loaded: true,
        previous_generation_wasm_jit_loaded: true,
        wasm_jit_authority: "immutable_status_query_after_real_component_model_instantiation_under_effective_unit",
        migration_authority: "candidate_write_then_exact_previous_generation_reopen_required",
        production_workspace_write_authority: false,
        production_workspace_path_exposed: false,
        production_workspace_write_probe_denied: true,
        continuity_or_reservoir_admission: false,
        candidate_process_groups_reaped: true,
        evidence_sha256: String::new(),
    };
    evidence.evidence_sha256 = evidence_digest(&evidence)?;
    validate_evidence(&evidence)?;
    Ok(evidence)
}

#[cfg(any(target_os = "linux", test))]
fn validate_evidence(evidence: &CandidateShadowEvidence) -> Result<()> {
    if evidence.schema != "astrid.edge_rescue_helper.candidate_shadow.v1"
        || !crate::config::valid_identifier(&evidence.appliance_id)
        || evidence.provenance != "deterministic_machine_evidence_not_astrid_authorship"
        || evidence.candidate_binary_sha256.len() < 3
        || evidence.candidate_binary_sha256.values().any(|digest| {
            digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        || evidence.previous_daemon_sha256.len() != 64
        || crate::kernel_replay::validate_evidence(&evidence.kernel_ingress_replay).is_err()
        || evidence.candidate_binary_sha256.get("astrid-daemon")
            != Some(&evidence.kernel_ingress_replay.daemon_sha256)
        || evidence.deterministic_input_sha256.len() != 64
        || evidence.input_samples != 128
        || evidence.reservoir_dimensions != 128
        || evidence.telemetry_samples < TELEMETRY_SAMPLES
        || !evidence.fill_minimum.is_finite()
        || !evidence.fill_mean.is_finite()
        || !evidence.fill_maximum.is_finite()
        || !evidence.candidate_state_opened
        || !evidence.previous_generation_reopened_candidate_state
        || evidence.wasm_jit_capsule_id != "astrid-capsule-edge-context"
        || !evidence.candidate_wasm_jit_loaded
        || !evidence.previous_generation_wasm_jit_loaded
        || evidence.wasm_jit_authority
            != "immutable_status_query_after_real_component_model_instantiation_under_effective_unit"
        || evidence.production_workspace_write_authority
        || evidence.production_workspace_path_exposed
        || !evidence.production_workspace_write_probe_denied
        || evidence.continuity_or_reservoir_admission
        || !evidence.candidate_process_groups_reaped
        || !crate::config::valid_hex64(&evidence.evidence_sha256)
        || evidence.evidence_sha256 != evidence_digest(evidence)?
    {
        return Err(Error::new(
            "candidate shadow or dual-reader migration replay was incomplete",
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", test))]
fn evidence_digest(evidence: &CandidateShadowEvidence) -> Result<String> {
    let mut value = serde_json::to_value(evidence)?;
    value
        .as_object_mut()
        .ok_or_else(|| Error::new("candidate shadow evidence is not an object"))?
        .remove("evidence_sha256");
    Ok(crate::fs_guard::sha256(&crate::fs_guard::canonical_json(
        &value,
    )?))
}

#[cfg(not(target_os = "linux"))]
pub fn run(
    _config: &Config,
    _bundle: &Path,
    _replay_root: &Path,
) -> Result<CandidateShadowEvidence> {
    Err(Error::new(
        "candidate-coupled shadow replay requires the Linux appliance target",
    ))
}

#[cfg(target_os = "linux")]
fn assert_production_workspace_denied(config: &Config) -> Result<()> {
    if config.identities.builder_uid == config.identities.runtime_uid
        || config.identities.builder_gid == config.identities.runtime_gid
    {
        return Err(Error::new(
            "builder and runtime identities are not isolated",
        ));
    }
    let metadata = fs::symlink_metadata(&config.roots.workspace)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() == config.identities.builder_uid
        || metadata.gid() == config.identities.builder_gid
        || metadata.mode() & 0o002 != 0
    {
        return Err(Error::new(
            "production workspace is writable or addressable by the builder identity",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn probe_production_workspace_denial(config: &Config, transaction_root: &Path) -> Result<()> {
    let path = config.roots.workspace.join(".astrid-builder-denial-probe");
    if path.exists() || path.is_symlink() {
        return Err(Error::new("builder denial probe path already exists"));
    }
    let mut runner = crate::native::SystemRunner;
    let receipt = runner.run_candidate_monitored(
        config,
        transaction_root,
        &CommandSpec {
            label: "production-workspace-builder-denial-probe",
            executable: config.executables.invariant_runner.clone(),
            arguments: vec![
                "internal-probe-denied-write".to_owned(),
                "--path".to_owned(),
                path.display().to_string(),
            ],
            current_dir: transaction_root.to_path_buf(),
            environment: BTreeMap::new(),
            timeout: Duration::from_secs(10),
            run_as_uid: Some(config.identities.builder_uid),
            run_as_gid: Some(config.identities.builder_gid),
        },
        &mut || Ok(()),
    )?;
    require_success(&receipt)?;
    if path.exists() || path.is_symlink() {
        return Err(Error::new(
            "builder denial probe left a production workspace artifact",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn run_daemon_once(
    config: &Config,
    transaction_root: &Path,
    binary: &Path,
    cli: &Path,
    home: &Path,
    workspace: &Path,
    replay_daemon_sha256: Option<&str>,
) -> Result<Option<crate::kernel_replay::KernelIngressEvidence>> {
    let ready = home.join("run/system.ready");
    let mut child = spawn(
        config,
        transaction_root,
        binary,
        &["--workspace".to_owned(), workspace.display().to_string()],
        workspace,
        BTreeMap::from([
            ("ASTRID_HOME".to_owned(), home.display().to_string()),
            ("HOME".to_owned(), home.display().to_string()),
            ("LANG".to_owned(), "C.UTF-8".to_owned()),
            ("LC_ALL".to_owned(), "C.UTF-8".to_owned()),
            ("TZ".to_owned(), "UTC".to_owned()),
        ]),
    )?;
    let outcome = wait_for(&mut child, Duration::from_secs(30), || ready.is_file())
        .and_then(|()| verify_jit_capsule_loaded(config, transaction_root, cli, home, workspace))
        .and_then(|()| {
            replay_daemon_sha256
                .map(|digest| crate::kernel_replay::probe(home, digest))
                .transpose()
        });
    let termination = terminate(&mut child);
    let evidence = outcome?;
    termination?;
    Ok(evidence)
}

#[cfg(target_os = "linux")]
fn verify_jit_capsule_loaded(
    config: &Config,
    transaction_root: &Path,
    cli: &Path,
    home: &Path,
    workspace: &Path,
) -> Result<()> {
    let mut runner = crate::native::SystemRunner;
    let spec = CommandSpec {
        label: "immutable-component-model-jit-status",
        executable: crate::config::TrustedExecutable {
            path: cli.to_path_buf(),
            sha256: sha256_file(cli, MAX_BINARY_BYTES)?,
        },
        arguments: vec![
            "--format".to_owned(),
            "json".to_owned(),
            "status".to_owned(),
        ],
        current_dir: workspace.to_path_buf(),
        environment: BTreeMap::from([
            ("ASTRID_HOME".to_owned(), home.display().to_string()),
            ("HOME".to_owned(), home.display().to_string()),
            ("LANG".to_owned(), "C.UTF-8".to_owned()),
            ("LC_ALL".to_owned(), "C.UTF-8".to_owned()),
            ("TZ".to_owned(), "UTC".to_owned()),
        ]),
        timeout: Duration::from_secs(30),
        run_as_uid: Some(config.identities.builder_uid),
        run_as_gid: Some(config.identities.builder_gid),
    };
    let (receipt, output) =
        runner.run_candidate_capture(config, transaction_root, &spec, 128 * 1024)?;
    require_success(&receipt)?;
    let value: Value = serde_json::from_slice(&output)?;
    let loaded = value
        .pointer("/status/loaded_capsules")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("immutable JIT status lacks loaded capsule evidence"))?;
    if value.get("running").and_then(Value::as_bool) != Some(true)
        || !loaded
            .iter()
            .any(|capsule| capsule.as_str() == Some(JIT_CAPSULE_ID))
    {
        return Err(Error::new(
            "real Component Model capsule did not instantiate in immutable shadow",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_jit_capsule_fixture(config: &Config, bundle: &Path, home: &Path) -> Result<()> {
    let source = bundle.join("installed-capsules").join(JIT_CAPSULE_ID);
    let principal = home.join("home").join("default");
    let local = principal.join(".local");
    let capsules = local.join("capsules");
    for directory in [&home.join("home"), &principal, &local, &capsules] {
        prepare_owned_directory(
            directory,
            config.identities.builder_uid,
            config.identities.builder_gid,
        )?;
    }
    let destination = capsules.join(JIT_CAPSULE_ID);
    if destination.exists() || destination.is_symlink() {
        return Err(Error::new("immutable JIT capsule fixture already exists"));
    }
    let mut bytes = 0_u64;
    let mut entries = 0_usize;
    copy_jit_capsule_tree(config, &source, &destination, &mut bytes, &mut entries, 0)?;
    if entries == 0 || bytes == 0 || bytes > MAX_JIT_CAPSULE_BYTES {
        return Err(Error::new(
            "immutable JIT capsule fixture is empty or oversized",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn copy_jit_capsule_tree(
    config: &Config,
    source: &Path,
    destination: &Path,
    bytes: &mut u64,
    entries: &mut usize,
    depth: usize,
) -> Result<()> {
    if depth > 8 || entries.saturating_add(1) > 128 {
        return Err(Error::new(
            "immutable JIT capsule fixture exceeds tree bounds",
        ));
    }
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return Err(Error::new(
            "immutable JIT capsule fixture contains a symlink",
        ));
    }
    *entries = entries.saturating_add(1);
    if metadata.is_dir() {
        fs::create_dir(destination)?;
        set_owner(
            destination,
            config.identities.builder_uid,
            config.identities.builder_gid,
            0o500,
        )?;
        let mut children = fs::read_dir(source)?.collect::<std::result::Result<Vec<_>, _>>()?;
        children.sort_by_key(std::fs::DirEntry::file_name);
        for child in children {
            copy_jit_capsule_tree(
                config,
                &child.path(),
                &destination.join(child.file_name()),
                bytes,
                entries,
                depth.saturating_add(1),
            )?;
        }
        return Ok(());
    }
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(Error::new(
            "immutable JIT capsule fixture contains a special or linked file",
        ));
    }
    *bytes = bytes.saturating_add(metadata.len());
    if *bytes > MAX_JIT_CAPSULE_BYTES {
        return Err(Error::new(
            "immutable JIT capsule fixture exceeds byte bound",
        ));
    }
    let copied = fs::copy(source, destination)?;
    if copied != metadata.len() {
        return Err(Error::new(
            "immutable JIT capsule fixture copy was incomplete",
        ));
    }
    set_owner(
        destination,
        config.identities.builder_uid,
        config.identities.builder_gid,
        0o400,
    )
}

#[cfg(target_os = "linux")]
#[allow(clippy::too_many_arguments)]
fn spawn_edge(
    config: &Config,
    transaction_root: &Path,
    binary: &Path,
    cli: &Path,
    home: &Path,
    workspace: &Path,
    telemetry: SocketAddr,
    sensory: SocketAddr,
) -> Result<CandidateProcess> {
    spawn_reservoir_only_edge(
        config,
        transaction_root,
        binary,
        cli,
        home,
        workspace,
        telemetry,
        sensory,
        SHADOW_SEED,
        "immutable candidate shadow",
    )
}

/// Spawn an edge binary with every authority-bearing subsystem disabled.
///
/// This is crate-visible so the immutable probation challenge can execute the
/// exact prior root-owned generation as an independent reservoir reference.
/// The mutable candidate never receives a handle to that reference process or
/// its output.
#[cfg(target_os = "linux")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_reservoir_only_edge(
    config: &Config,
    transaction_root: &Path,
    binary: &Path,
    cli: &Path,
    home: &Path,
    workspace: &Path,
    telemetry: SocketAddr,
    sensory: SocketAddr,
    seed: u64,
    instance_name: &str,
) -> Result<CandidateProcess> {
    let socket = home.join("run/unavailable-system.sock");
    let token = home.join("run/unavailable-system.token");
    spawn(
        config,
        transaction_root,
        binary,
        &[
            "--instance-name".into(),
            instance_name.into(),
            "--telemetry-addr".into(),
            telemetry.to_string(),
            "--sensory-addr".into(),
            sensory.to_string(),
            "--astrid-socket".into(),
            socket.display().to_string(),
            "--astrid-token".into(),
            token.display().to_string(),
            "--workspace".into(),
            workspace.display().to_string(),
            "--astrid-cli".into(),
            cli.display().to_string(),
            "--local-model-id".into(),
            "immutable-shadow-no-model".into(),
            "--maintenance-lease-path".into(),
            home.join("no-maintenance-lease").display().to_string(),
            "--autonomy-enabled=false".into(),
            "--scheduled-introspection-enabled=false".into(),
            "--self-change-enabled=false".into(),
            "--perceptual-notebook-enabled=false".into(),
            "--spectral-enabled=false".into(),
            "--reservoir-tuning-enabled=false".into(),
            "--fill-target".into(),
            "0.68".into(),
            "--tick-hz".into(),
            "20".into(),
            "--seed".into(),
            seed.to_string(),
        ],
        workspace,
        BTreeMap::from([
            ("ASTRID_HOME".to_owned(), home.display().to_string()),
            ("HOME".to_owned(), home.display().to_string()),
            ("ASTRID_EDGE_AUDIO_DEVICE".to_owned(), "off".to_owned()),
            ("LANG".to_owned(), "C.UTF-8".to_owned()),
            ("LC_ALL".to_owned(), "C.UTF-8".to_owned()),
            ("TZ".to_owned(), "UTC".to_owned()),
        ]),
    )
}

#[cfg(target_os = "linux")]
fn run_reservoir_replay(
    child: &mut CandidateProcess,
    sensory: SocketAddr,
    workspace: &Path,
) -> Result<(String, Vec<f64>)> {
    let started = Instant::now();
    let mut stream = loop {
        if let Some(status) = child.try_wait()? {
            return Err(Error::new(format!(
                "candidate edge exited before replay: {status}"
            )));
        }
        match TcpStream::connect_timeout(&sensory, Duration::from_millis(200)) {
            Ok(stream) => break stream,
            Err(_) if started.elapsed() < Duration::from_secs(30) => {
                thread::sleep(Duration::from_millis(100));
            },
            Err(error) => {
                return Err(Error::new(format!(
                    "candidate sensory listener unavailable: {error}"
                )));
            },
        }
    };
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    websocket_handshake(&mut stream, sensory)?;
    let _hello = read_websocket_text(&mut stream, 64 * 1024)?;
    let mut input_bytes = Vec::new();
    for sample in 0..INPUT_SAMPLES {
        let sample_u32 = u32::try_from(sample).unwrap_or_default();
        let phase_sample = f32::from(u16::try_from(sample_u32).unwrap_or_default());
        let features = (0..48_u32)
            .map(|index| {
                let phase = phase_sample * 0.071
                    + f32::from(u16::try_from(index).unwrap_or_default()) * 0.113;
                phase.sin()
            })
            .collect::<Vec<_>>();
        let packet = canonical_json(&json!({
            "protocol": {"name":"astrid_minime","major":1,"minor":3},
            "kind": "semantic",
            "features": features,
            "ts_ms": sample_u32.saturating_mul(50),
        }))?;
        input_bytes.extend_from_slice(&packet);
        input_bytes.push(b'\n');
        write_websocket_text(&mut stream, &packet, sample_u32)?;
    }
    let state_path = workspace.join("runtime/spectral_state.json");
    let mut fills = Vec::new();
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(15))
        .ok_or_else(|| Error::new("candidate shadow deadline overflow"))?;
    let mut last_sequence = None;
    while fills.len() < TELEMETRY_SAMPLES && Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            return Err(Error::new(format!(
                "candidate edge exited during replay: {status}"
            )));
        }
        if state_path.is_file() {
            let value: Value = read_json(&state_path, 256 * 1024)?;
            let sequence = value.get("sequence").and_then(Value::as_u64);
            if sequence != last_sequence {
                // Startup snapshots may legitimately precede the first
                // semantic impulse. They are not replay evidence and do not
                // consume the bounded sample target.
                if value.get("semantic_fresh").and_then(Value::as_bool) != Some(true) {
                    thread::sleep(Duration::from_millis(100));
                    continue;
                }
                validate_state(&value)?;
                fills.push(
                    value
                        .get("fill_ratio")
                        .and_then(Value::as_f64)
                        .ok_or_else(|| Error::new("candidate shadow fill is absent"))?,
                );
                last_sequence = sequence;
            }
        }
        thread::sleep(Duration::from_millis(500));
    }
    Ok((sha256(&input_bytes), fills))
}

#[cfg(any(target_os = "linux", test))]
fn validate_state(value: &Value) -> Result<()> {
    let dimensions = value
        .pointer("/substrate/reservoir_dim")
        .and_then(Value::as_u64);
    let fill = value.get("fill_ratio").and_then(Value::as_f64);
    if dimensions != Some(128)
        || fill.is_none_or(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
        || value.get("semantic_fresh").and_then(Value::as_bool) != Some(true)
        || value.get("authority").and_then(Value::as_str)
            != Some("deterministic_machine_spectral_state_not_astrid_authorship_or_causal_proof")
    {
        return Err(Error::new(
            "candidate shadow spectral state failed immutable checks",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn spawn(
    config: &Config,
    transaction_root: &Path,
    binary: &Path,
    arguments: &[String],
    current_dir: &Path,
    environment: BTreeMap<String, String>,
) -> Result<CandidateProcess> {
    let executable = crate::config::TrustedExecutable {
        path: binary.to_path_buf(),
        sha256: sha256_file(binary, MAX_BINARY_BYTES)?,
    };
    let mut runner = crate::native::SystemRunner;
    runner.spawn_candidate(
        config,
        transaction_root,
        &CommandSpec {
            label: "candidate-shadow-process",
            executable,
            arguments: arguments.to_vec(),
            current_dir: current_dir.to_path_buf(),
            environment,
            timeout: Duration::from_secs(config.policy.command_timeout_seconds),
            run_as_uid: Some(config.identities.builder_uid),
            run_as_gid: Some(config.identities.builder_gid),
        },
    )
}

#[cfg(target_os = "linux")]
fn wait_for(
    child: &mut CandidateProcess,
    timeout: Duration,
    condition: impl Fn() -> bool,
) -> Result<()> {
    let started = Instant::now();
    loop {
        if condition() {
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            return Err(Error::new(format!(
                "candidate fixture exited before readiness: {status}"
            )));
        }
        if started.elapsed() >= timeout {
            return Err(Error::new("candidate fixture readiness timed out"));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn terminate(child: &mut CandidateProcess) -> Result<()> {
    child.terminate()
}

#[cfg(target_os = "linux")]
pub(crate) fn reserve_loopback_pair() -> Result<(SocketAddr, SocketAddr)> {
    let telemetry = TcpListener::bind("127.0.0.1:0")?;
    let sensory = TcpListener::bind("127.0.0.1:0")?;
    let telemetry_address = telemetry.local_addr()?;
    let sensory_address = sensory.local_addr()?;
    validate_distinct_addresses(telemetry_address, sensory_address)?;
    // Keep both reservations live until the pair is complete, then release
    // immediately before the single candidate edge spawn.
    drop(sensory);
    drop(telemetry);
    Ok((telemetry_address, sensory_address))
}

#[cfg(any(target_os = "linux", test))]
fn validate_distinct_addresses(telemetry: SocketAddr, sensory: SocketAddr) -> Result<()> {
    if telemetry == sensory || !telemetry.ip().is_loopback() || !sensory.ip().is_loopback() {
        return Err(Error::new(
            "shadow telemetry and sensory reservations are not distinct loopback sockets",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn websocket_handshake(stream: &mut TcpStream, address: SocketAddr) -> Result<()> {
    let request = format!(
        "GET / HTTP/1.1\r\nHost: {address}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    stream.write_all(request.as_bytes())?;
    let mut response = Vec::new();
    let mut byte = [0_u8; 1];
    while !response.ends_with(b"\r\n\r\n") && response.len() < 8 * 1024 {
        stream.read_exact(&mut byte)?;
        response.push(byte[0]);
    }
    let text = String::from_utf8_lossy(&response);
    let lower = text.to_ascii_lowercase();
    if !text.starts_with("HTTP/1.1 101 ")
        || !lower.contains("upgrade: websocket")
        || !lower.contains("sec-websocket-accept: s3pplmbitxaq9kygzzhzrbk+xoo=")
    {
        return Err(Error::new(
            "candidate sensory WebSocket handshake is invalid",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn write_websocket_text(
    stream: &mut TcpStream,
    payload: &[u8],
    sequence: u32,
) -> Result<()> {
    let mut frame = vec![0x81];
    let length = payload.len();
    if length <= 125 {
        frame.push(0x80 | u8::try_from(length).unwrap_or(125));
    } else if u16::try_from(length).is_ok() {
        frame.push(0x80 | 0x7e);
        frame.extend_from_slice(&u16::try_from(length).unwrap_or(u16::MAX).to_be_bytes());
    } else {
        return Err(Error::new("shadow sensory frame exceeds bounded size"));
    }
    let mask = sequence.to_be_bytes();
    frame.extend_from_slice(&mask);
    frame.extend(
        payload
            .iter()
            .enumerate()
            .map(|(index, byte)| byte ^ mask[index % 4]),
    );
    stream.write_all(&frame)?;
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn read_websocket_text(stream: &mut TcpStream, maximum: usize) -> Result<Vec<u8>> {
    let mut header = [0_u8; 2];
    stream.read_exact(&mut header)?;
    if header[0] & 0x0f != 1 || header[1] & 0x80 != 0 {
        return Err(Error::new(
            "shadow WebSocket server frame is not unmasked text",
        ));
    }
    let mut length = usize::from(header[1] & 0x7f);
    if length == 126 {
        let mut extended = [0_u8; 2];
        stream.read_exact(&mut extended)?;
        length = usize::from(u16::from_be_bytes(extended));
    } else if length == 127 {
        let mut extended = [0_u8; 8];
        stream.read_exact(&mut extended)?;
        length = usize::try_from(u64::from_be_bytes(extended))
            .map_err(|_| Error::new("shadow WebSocket frame length overflow"))?;
    }
    if length > maximum {
        return Err(Error::new("shadow WebSocket frame exceeds bound"));
    }
    let mut payload = vec![0_u8; length];
    stream.read_exact(&mut payload)?;
    Ok(payload)
}

#[cfg(target_os = "linux")]
pub(crate) fn prepare_owned_directory(path: &Path, uid: u32, gid: u32) -> Result<()> {
    if !path.exists() {
        fs::create_dir(path)?;
    }
    set_owner(path, uid, gid, 0o700)
}

#[cfg(target_os = "linux")]
fn set_owner(path: &Path, uid: u32, gid: u32, mode: u32) -> Result<()> {
    nix::unistd::chown(
        path,
        Some(nix::unistd::Uid::from_raw(uid)),
        Some(nix::unistd::Gid::from_raw(gid)),
    )
    .map_err(|error| Error::new(format!("cannot assign shadow identity: {error}")))?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn cleanup_runtime_endpoints(home: &Path) -> Result<()> {
    for name in ["system.ready", "system.sock", "system.token"] {
        let path = home.join("run").join(name);
        if path.exists() || path.is_symlink() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CandidateShadowEvidence, evidence_digest, validate_evidence, validate_state};
    use crate::fs_guard::canonical_json;
    use crate::kernel_replay::KernelIngressEvidence;
    use std::collections::BTreeMap;

    fn kernel_evidence(daemon_sha256: &str) -> KernelIngressEvidence {
        let mut evidence = KernelIngressEvidence {
            schema: "astrid.edge_rescue_helper.kernel_ingress_replay.v1",
            provenance: "immutable_public_protocol_regression_machine_evidence_not_astrid_authorship",
            daemon_sha256: daemon_sha256.to_owned(),
            invalid_token_rejected: true,
            incompatible_protocol_rejected: true,
            authenticated_observer: true,
            authenticated_emitter: true,
            producer_claim_overwritten: true,
            malformed_trace_rerooted: true,
            sensory_mirror_preserved_trace: true,
            forged_provider_metrics_removed: true,
            public_protocol_regression_only: true,
            grants_activation_authority: false,
            production_continuity_or_reservoir_admission: false,
            evidence_sha256: String::new(),
        };
        evidence.evidence_sha256 = crate::kernel_replay::evidence_digest(&evidence).unwrap();
        evidence
    }

    #[test]
    fn evidence_is_bounded_machine_only_and_digestable() {
        let mut evidence = CandidateShadowEvidence {
            schema: "astrid.edge_rescue_helper.candidate_shadow.v1",
            appliance_id: "avado-test".into(),
            provenance: "deterministic_machine_evidence_not_astrid_authorship",
            candidate_binary_sha256: BTreeMap::from([("astrid-daemon".into(), "a".repeat(64))]),
            previous_daemon_sha256: "b".repeat(64),
            kernel_ingress_replay: kernel_evidence(&"a".repeat(64)),
            deterministic_input_sha256: "c".repeat(64),
            input_samples: 128,
            reservoir_dimensions: 128,
            telemetry_samples: 4,
            fill_minimum: 0.67,
            fill_mean: 0.68,
            fill_maximum: 0.69,
            candidate_state_opened: true,
            previous_generation_reopened_candidate_state: true,
            wasm_jit_capsule_id: "astrid-capsule-edge-context",
            candidate_wasm_jit_loaded: true,
            previous_generation_wasm_jit_loaded: true,
            wasm_jit_authority: "immutable_status_query_after_real_component_model_instantiation_under_effective_unit",
            migration_authority: "candidate_write_then_exact_previous_generation_reopen_required",
            production_workspace_write_authority: false,
            production_workspace_path_exposed: false,
            production_workspace_write_probe_denied: true,
            continuity_or_reservoir_admission: false,
            candidate_process_groups_reaped: true,
            evidence_sha256: String::new(),
        };
        evidence.evidence_sha256 = evidence_digest(&evidence).unwrap();
        let bytes = canonical_json(&evidence).unwrap();
        assert!(bytes.len() < 4_096);
        assert!(
            String::from_utf8(bytes)
                .unwrap()
                .contains("not_astrid_authorship")
        );
        assert!(validate_evidence(&evidence).is_err());
    }

    #[test]
    fn destructive_or_forged_migration_and_reservoir_evidence_is_rejected() {
        let mut hashes = BTreeMap::new();
        for name in ["astrid", "astrid-daemon", "astrid-edge-runtime"] {
            hashes.insert(name.to_owned(), "a".repeat(64));
        }
        let mut evidence = CandidateShadowEvidence {
            schema: "astrid.edge_rescue_helper.candidate_shadow.v1",
            appliance_id: "avado-test".into(),
            provenance: "deterministic_machine_evidence_not_astrid_authorship",
            candidate_binary_sha256: hashes,
            previous_daemon_sha256: "b".repeat(64),
            kernel_ingress_replay: kernel_evidence(&"a".repeat(64)),
            deterministic_input_sha256: "c".repeat(64),
            input_samples: 128,
            reservoir_dimensions: 128,
            telemetry_samples: 4,
            fill_minimum: 0.67,
            fill_mean: 0.68,
            fill_maximum: 0.69,
            candidate_state_opened: true,
            previous_generation_reopened_candidate_state: true,
            wasm_jit_capsule_id: "astrid-capsule-edge-context",
            candidate_wasm_jit_loaded: true,
            previous_generation_wasm_jit_loaded: true,
            wasm_jit_authority: "immutable_status_query_after_real_component_model_instantiation_under_effective_unit",
            migration_authority: "candidate_write_then_exact_previous_generation_reopen_required",
            production_workspace_write_authority: false,
            production_workspace_path_exposed: false,
            production_workspace_write_probe_denied: true,
            continuity_or_reservoir_admission: false,
            candidate_process_groups_reaped: true,
            evidence_sha256: String::new(),
        };
        evidence.evidence_sha256 = evidence_digest(&evidence).unwrap();
        assert!(validate_evidence(&evidence).is_ok());
        evidence.previous_generation_reopened_candidate_state = false;
        assert!(validate_evidence(&evidence).is_err());
        evidence.previous_generation_reopened_candidate_state = true;
        evidence.continuity_or_reservoir_admission = true;
        assert!(validate_evidence(&evidence).is_err());

        let valid = serde_json::json!({
            "substrate": {"reservoir_dim": 128},
            "fill_ratio": 0.68,
            "semantic_fresh": true,
            "authority": "deterministic_machine_spectral_state_not_astrid_authorship_or_causal_proof",
        });
        assert!(validate_state(&valid).is_ok());
        let mut destructive = valid;
        destructive["substrate"]["reservoir_dim"] = serde_json::json!(64);
        assert!(validate_state(&destructive).is_err());
    }

    #[test]
    fn same_port_shadow_reservation_is_rejected() {
        let address = "127.0.0.1:43123".parse().unwrap();
        assert!(super::validate_distinct_addresses(address, address).is_err());
        let other = "127.0.0.1:43124".parse().unwrap();
        assert!(super::validate_distinct_addresses(address, other).is_ok());
    }
}
