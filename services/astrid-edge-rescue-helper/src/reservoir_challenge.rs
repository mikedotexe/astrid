//! Immutable, nonce-bound reservoir challenge used during A/B probation.
//!
//! Candidate-owned state files and telemetry are corroborative evidence only.
//! During probation the root helper runs the exact sealed active and previous
//! edge binaries without authority in separate disposable roots, gives both an
//! unpredictable semantic stimulus, and derives their spectral dynamics
//! itself. The real service is bound by PID/executable hash, but never receives
//! the synthetic stimulus: challenge traffic cannot enter its reservoir,
//! notebook, thread, prompt, artifacts, or public telemetry. The signed
//! probation ledger attests both bound response digests. A static `0.68`
//! file/WebSocket pair therefore cannot satisfy reservoir health.

#[cfg(any(target_os = "linux", test))]
use std::collections::BTreeSet;
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::io::Read;
#[cfg(target_os = "linux")]
use std::net::{SocketAddr, TcpStream};
#[cfg(any(target_os = "linux", test))]
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::thread;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

use serde::Serialize;
#[cfg(any(target_os = "linux", test))]
use serde_json::{Value, json};

use crate::config::Config;
#[cfg(any(target_os = "linux", test))]
use crate::fs_guard::{canonical_json, sha256};
#[cfg(target_os = "linux")]
use crate::fs_guard::{ensure_within, sha256_file};
#[cfg(target_os = "linux")]
use crate::native::CandidateProcess;
#[cfg(target_os = "linux")]
use crate::shadow::{
    prepare_owned_directory, read_websocket_text, reserve_loopback_pair, spawn_reservoir_only_edge,
    terminate, websocket_handshake, write_websocket_text,
};
use crate::{Error, Result};

#[cfg(any(target_os = "linux", test))]
#[cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, reason = "parsed only by the Linux live challenge")
)]
const RESERVOIR_DIMENSIONS: f64 = 128.0;
#[cfg(any(target_os = "linux", test))]
const MINIMUM_RESPONSE_SAMPLES: usize = 24;
#[cfg(any(target_os = "linux", test))]
const MAXIMUM_RESPONSE_SAMPLES: usize = 40;
#[cfg(any(target_os = "linux", test))]
const RECOVERY_SAMPLES: usize = 4;
#[cfg(target_os = "linux")]
const MAX_BINARY_BYTES: u64 = 512 * 1024 * 1024;
#[cfg(target_os = "linux")]
const MAX_PACKET_BYTES: usize = 128 * 1024;
#[cfg(any(target_os = "linux", test))]
const MINIMUM_DYNAMIC_SPAN: f64 = 1.0e-6;
#[cfg(any(target_os = "linux", test))]
const MINIMUM_SPECTRAL_PATH: f64 = 1.0e-5;
#[cfg(any(target_os = "linux", test))]
const MAXIMUM_CHALLENGE_MILLISECONDS: u64 = 60_000;

#[derive(Debug, Clone, Serialize)]
pub struct ProbationReservoirChallengeEvidence {
    pub schema: &'static str,
    pub appliance_id: String,
    pub provenance: &'static str,
    pub authority: &'static str,
    pub challenge_nonce_sha256: String,
    pub challenge_input_sha256: String,
    pub challenge_started_at_unix_ms: u64,
    pub challenge_completed_at_unix_ms: u64,
    pub active_generation_id: String,
    pub active_edge_binary_sha256: String,
    pub active_edge_main_pid: u64,
    pub live_reservoir_generation_id: String,
    pub live_telemetry_t_ms: u64,
    pub active_reservoir_generation_id: String,
    pub reference_generation_id: String,
    pub reference_edge_binary_sha256: String,
    pub reference_reservoir_generation_id: String,
    pub candidate_output_series_sha256: String,
    pub reference_output_series_sha256: String,
    pub candidate_bound_response_sha256: String,
    pub reference_bound_response_sha256: String,
    pub input_samples: usize,
    pub candidate_samples: usize,
    pub reference_samples: usize,
    pub candidate_first_sequence: u64,
    pub candidate_last_sequence: u64,
    pub reference_first_sequence: u64,
    pub reference_last_sequence: u64,
    pub candidate_fill_mean: f64,
    pub candidate_fill_minimum: f64,
    pub candidate_fill_maximum: f64,
    pub candidate_instantaneous_fill_span: f64,
    pub candidate_spectral_entropy_span: f64,
    pub candidate_spectral_path_mean: f64,
    pub candidate_unique_spectral_shapes: usize,
    pub candidate_input_response_correlation: f64,
    pub reference_instantaneous_fill_span: f64,
    pub reference_spectral_entropy_span: f64,
    pub reference_spectral_path_mean: f64,
    pub reference_unique_spectral_shapes: usize,
    pub reference_input_response_correlation: f64,
    pub candidate_reference_response_ratio: f64,
    pub challenge_passed: bool,
    pub continuity_or_reservoir_admission: bool,
    pub evidence_sha256: String,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Clone)]
struct ChallengePlan {
    nonce_sha256: String,
    input_sha256: String,
    packets: Vec<Vec<u8>>,
    input_energy: Vec<f64>,
    pacing_milliseconds: Vec<u64>,
    #[cfg_attr(
        not(target_os = "linux"),
        allow(dead_code, reason = "consumed only by Linux child processes")
    )]
    reference_seed: u64,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Clone, Serialize)]
struct SpectralObservation {
    sequence: u64,
    t_ms: u64,
    fill_ratio: f64,
    instantaneous_fill_ratio: f64,
    effective_dimensionality: f64,
    spectral_entropy: f64,
    exported_entropy: f64,
    normalized_eigenvalues: Vec<f64>,
    reservoir_generation_id: String,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Clone)]
struct ResponseSeries {
    challenge_input_sha256: String,
    observations: Vec<SpectralObservation>,
    input_energy: Vec<f64>,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Clone)]
#[cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, reason = "full metrics are consumed by the Linux challenge")
)]
struct ResponseMetrics {
    output_series_sha256: String,
    first_sequence: u64,
    last_sequence: u64,
    fill_mean: f64,
    fill_minimum: f64,
    fill_maximum: f64,
    instantaneous_fill_span: f64,
    spectral_entropy_span: f64,
    spectral_path_mean: f64,
    unique_spectral_shapes: usize,
    input_response_correlation: f64,
}

/// Run the immutable live/reference challenge for an active probation.
#[cfg(target_os = "linux")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn run(
    config: &Config,
    active_generation_id: &str,
    previous_generation_id: &str,
    active_edge_main_pid: u64,
    expected_active_reservoir_generation_id: &str,
    minimum_live_t_ms: u64,
) -> Result<ProbationReservoirChallengeEvidence> {
    if !crate::config::valid_identifier(active_generation_id)
        || !crate::config::valid_identifier(previous_generation_id)
        || active_generation_id == previous_generation_id
        || active_edge_main_pid == 0
        || expected_active_reservoir_generation_id.is_empty()
    {
        return Err(Error::new(
            "probation reservoir challenge lineage is invalid",
        ));
    }
    let active_edge = config
        .roots
        .releases
        .join(active_generation_id)
        .join("astrid-edge-runtime");
    let active_cli = config
        .roots
        .releases
        .join(active_generation_id)
        .join("astrid");
    let reference_root = config.roots.releases.join(previous_generation_id);
    let reference_edge = reference_root.join("astrid-edge-runtime");
    let reference_cli = reference_root.join("astrid");
    verify_process_binary(active_edge_main_pid, &active_edge)?;
    let active_binary_sha256 = sha256_file(&active_edge, MAX_BINARY_BYTES)?;
    let active_cli_sha256 = sha256_file(&active_cli, MAX_BINARY_BYTES)?;
    let reference_binary_sha256 = sha256_file(&reference_edge, MAX_BINARY_BYTES)?;
    let reference_cli_sha256 = sha256_file(&reference_cli, MAX_BINARY_BYTES)?;
    let plan = create_plan()?;
    let started_at = unix_millis();

    let replay_id = uuid::Uuid::new_v4();
    let (candidate_root, reference_process_root) =
        isolated_process_roots(&config.roots.candidate_work, replay_id);
    let instance_suffix = &plan.nonce_sha256[..16];
    let instance_name = format!("edge isolated {instance_suffix}");
    // Run the untrusted candidate to completion and prove its entire transient
    // cgroup and writable root are gone before the exact prior generation is
    // even materialized. Sharing a UID or loopback namespace is therefore not
    // a communication channel between the two measurements.
    let candidate_series = run_isolated_response(
        config,
        &candidate_root,
        &active_edge,
        &active_cli,
        &plan,
        0,
        &instance_name,
    )?;
    let reference_series = run_isolated_response(
        config,
        &reference_process_root,
        &reference_edge,
        &reference_cli,
        &plan,
        11,
        &instance_name,
    )?;

    verify_process_binary(active_edge_main_pid, &active_edge)?;
    if active_binary_sha256 != sha256_file(&active_edge, MAX_BINARY_BYTES)?
        || active_cli_sha256 != sha256_file(&active_cli, MAX_BINARY_BYTES)?
        || reference_binary_sha256 != sha256_file(&reference_edge, MAX_BINARY_BYTES)?
        || reference_cli_sha256 != sha256_file(&reference_cli, MAX_BINARY_BYTES)?
    {
        return Err(Error::new(
            "probation challenge binary changed during immutable observation",
        ));
    }
    let candidate_generation = candidate_series
        .observations
        .first()
        .map(|sample| sample.reservoir_generation_id.clone())
        .ok_or_else(|| Error::new("candidate challenge response is empty"))?;
    let reference_generation = reference_series
        .observations
        .first()
        .map(|sample| sample.reservoir_generation_id.clone())
        .ok_or_else(|| Error::new("reference challenge response is empty"))?;
    let candidate_metrics = evaluate_series(
        &candidate_series,
        expected_active_reservoir_generation_id,
        &plan.input_sha256,
    )?;
    let reference_metrics =
        evaluate_series(&reference_series, &reference_generation, &plan.input_sha256)?;
    let response_ratio = candidate_metrics.spectral_path_mean
        / reference_metrics
            .spectral_path_mean
            .max(MINIMUM_SPECTRAL_PATH);
    if !(0.02..=50.0).contains(&response_ratio)
        || !(0.65..=0.735).contains(&candidate_metrics.fill_mean)
        || candidate_metrics.output_series_sha256 == reference_metrics.output_series_sha256
    {
        return Err(Error::new(
            "candidate reservoir response diverged from immutable reference bounds",
        ));
    }
    let completed_at = unix_millis();
    if completed_at < started_at
        || completed_at.saturating_sub(started_at) > MAXIMUM_CHALLENGE_MILLISECONDS
    {
        return Err(Error::new(
            "probation reservoir challenge exceeded its immutable time window",
        ));
    }
    let candidate_bound = bound_response_digest(
        &config.appliance_id,
        &plan.nonce_sha256,
        &plan.input_sha256,
        active_generation_id,
        &active_binary_sha256,
        &candidate_metrics.output_series_sha256,
        started_at,
        completed_at,
    )?;
    let reference_bound = bound_response_digest(
        &config.appliance_id,
        &plan.nonce_sha256,
        &plan.input_sha256,
        previous_generation_id,
        &reference_binary_sha256,
        &reference_metrics.output_series_sha256,
        started_at,
        completed_at,
    )?;
    let mut evidence = ProbationReservoirChallengeEvidence {
        schema: "astrid.edge_rescue_helper.probation_reservoir_challenge.v1",
        appliance_id: config.appliance_id.clone(),
        provenance: "synthetic_root_test_machine_evidence_not_astrid_authorship",
        authority: "root_nonce_isolated_sealed_candidate_plus_exact_previous_generation_reference",
        challenge_nonce_sha256: plan.nonce_sha256,
        challenge_input_sha256: plan.input_sha256,
        challenge_started_at_unix_ms: started_at,
        challenge_completed_at_unix_ms: completed_at,
        active_generation_id: active_generation_id.to_owned(),
        active_edge_binary_sha256: active_binary_sha256,
        active_edge_main_pid,
        live_reservoir_generation_id: expected_active_reservoir_generation_id.to_owned(),
        live_telemetry_t_ms: minimum_live_t_ms,
        active_reservoir_generation_id: candidate_generation,
        reference_generation_id: previous_generation_id.to_owned(),
        reference_edge_binary_sha256: reference_binary_sha256,
        reference_reservoir_generation_id: reference_generation,
        candidate_output_series_sha256: candidate_metrics.output_series_sha256,
        reference_output_series_sha256: reference_metrics.output_series_sha256,
        candidate_bound_response_sha256: candidate_bound,
        reference_bound_response_sha256: reference_bound,
        input_samples: plan.packets.len(),
        candidate_samples: candidate_series.observations.len(),
        reference_samples: reference_series.observations.len(),
        candidate_first_sequence: candidate_metrics.first_sequence,
        candidate_last_sequence: candidate_metrics.last_sequence,
        reference_first_sequence: reference_metrics.first_sequence,
        reference_last_sequence: reference_metrics.last_sequence,
        candidate_fill_mean: candidate_metrics.fill_mean,
        candidate_fill_minimum: candidate_metrics.fill_minimum,
        candidate_fill_maximum: candidate_metrics.fill_maximum,
        candidate_instantaneous_fill_span: candidate_metrics.instantaneous_fill_span,
        candidate_spectral_entropy_span: candidate_metrics.spectral_entropy_span,
        candidate_spectral_path_mean: candidate_metrics.spectral_path_mean,
        candidate_unique_spectral_shapes: candidate_metrics.unique_spectral_shapes,
        candidate_input_response_correlation: candidate_metrics.input_response_correlation,
        reference_instantaneous_fill_span: reference_metrics.instantaneous_fill_span,
        reference_spectral_entropy_span: reference_metrics.spectral_entropy_span,
        reference_spectral_path_mean: reference_metrics.spectral_path_mean,
        reference_unique_spectral_shapes: reference_metrics.unique_spectral_shapes,
        reference_input_response_correlation: reference_metrics.input_response_correlation,
        candidate_reference_response_ratio: response_ratio,
        challenge_passed: true,
        continuity_or_reservoir_admission: false,
        evidence_sha256: String::new(),
    };
    evidence.evidence_sha256 = evidence_digest(&evidence)?;
    validate_evidence(&evidence)?;
    Ok(evidence)
}

#[cfg(any(target_os = "linux", test))]
fn isolated_process_roots(candidate_work: &Path, replay_id: uuid::Uuid) -> (PathBuf, PathBuf) {
    (
        candidate_work.join(format!("probation-reservoir-candidate-{replay_id}")),
        candidate_work.join(format!("probation-reservoir-reference-{replay_id}")),
    )
}

#[cfg(not(target_os = "linux"))]
pub fn run(
    _config: &Config,
    _active_generation_id: &str,
    _previous_generation_id: &str,
    _active_edge_main_pid: u64,
    _expected_active_reservoir_generation_id: &str,
    _minimum_live_t_ms: u64,
) -> Result<ProbationReservoirChallengeEvidence> {
    Err(Error::new(
        "probation reservoir challenge requires the Linux appliance target",
    ))
}

#[cfg(target_os = "linux")]
#[allow(clippy::too_many_arguments)]
fn run_isolated_response(
    config: &Config,
    process_root: &Path,
    edge: &Path,
    cli: &Path,
    plan: &ChallengePlan,
    mask_rotation: u32,
    instance_name: &str,
) -> Result<ResponseSeries> {
    ensure_within(&config.roots.candidate_work, process_root, false)?;
    if process_root.parent() != Some(config.roots.candidate_work.as_path())
        || process_root.exists()
        || process_root.is_symlink()
    {
        return Err(Error::new(
            "probation reservoir process root is not a fresh exact transaction",
        ));
    }
    fs::create_dir(process_root)?;
    let setup = (|| {
        prepare_owned_directory(
            process_root,
            config.identities.builder_uid,
            config.identities.builder_gid,
        )?;
        let home = process_root.join("home");
        let workspace = process_root.join("workspace");
        prepare_owned_directory(
            &home,
            config.identities.builder_uid,
            config.identities.builder_gid,
        )?;
        prepare_owned_directory(
            &workspace,
            config.identities.builder_uid,
            config.identities.builder_gid,
        )?;
        Ok((home, workspace))
    })();
    let (home, workspace) = match setup {
        Ok(paths) => paths,
        Err(error) => {
            cleanup_replay_root(process_root)?;
            return Err(error);
        },
    };
    let (telemetry, sensory) = match reserve_loopback_pair() {
        Ok(addresses) => addresses,
        Err(error) => {
            cleanup_replay_root(process_root)?;
            return Err(error);
        },
    };
    let mut process = match spawn_reservoir_only_edge(
        config,
        process_root,
        edge,
        cli,
        &home,
        &workspace,
        telemetry,
        sensory,
        plan.reference_seed,
        instance_name,
    ) {
        Ok(process) => process,
        Err(error) => {
            cleanup_replay_root(process_root)?;
            return Err(error);
        },
    };
    let outcome = collect_isolated_response(&mut process, telemetry, sensory, plan, mask_rotation);
    let termination = terminate(&mut process);
    let cleanup = cleanup_replay_root(process_root);
    termination?;
    cleanup?;
    outcome
}

#[cfg(target_os = "linux")]
fn collect_isolated_response(
    process: &mut CandidateProcess,
    telemetry: SocketAddr,
    sensory: SocketAddr,
    plan: &ChallengePlan,
    mask_rotation: u32,
) -> Result<ResponseSeries> {
    let mut sensory_stream = connect_with_retry(sensory, Some(process))?;
    websocket_handshake(&mut sensory_stream, sensory)?;
    let _hello = read_websocket_text(&mut sensory_stream, 64 * 1024)?;
    // Acquire non-zero recurrent/covariance state before the randomized
    // stimulus. This does not run a model or authority-bearing subsystem.
    thread::sleep(Duration::from_millis(2_000));

    let mut telemetry_stream = connect_with_retry(telemetry, Some(process))?;
    websocket_handshake(&mut telemetry_stream, telemetry)?;
    let initial = read_observation(&mut telemetry_stream)?;
    let mut observations = Vec::with_capacity(plan.packets.len());
    let mut sequence = initial.sequence;
    let mut t_ms = initial.t_ms;
    for (index, ((packet, pacing), _energy)) in plan
        .packets
        .iter()
        .zip(&plan.pacing_milliseconds)
        .zip(&plan.input_energy)
        .enumerate()
    {
        let mask = u32::try_from(index)
            .unwrap_or(u32::MAX)
            .wrapping_add(u32::from(packet.first().copied().unwrap_or_default()))
            .rotate_left(mask_rotation);
        write_websocket_text(&mut sensory_stream, packet, mask)?;
        thread::sleep(Duration::from_millis(*pacing));
        let observation = read_new_observation(&mut telemetry_stream, sequence, t_ms)?;
        sequence = observation.sequence;
        t_ms = observation.t_ms;
        observations.push(observation);
    }
    Ok(ResponseSeries {
        challenge_input_sha256: plan.input_sha256.clone(),
        observations,
        input_energy: plan.input_energy.clone(),
    })
}

#[cfg(target_os = "linux")]
fn connect_once(address: SocketAddr) -> Result<TcpStream> {
    if !address.ip().is_loopback() {
        return Err(Error::new("reservoir challenge endpoint is not loopback"));
    }
    let stream = TcpStream::connect_timeout(&address, Duration::from_secs(3))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    Ok(stream)
}

#[cfg(target_os = "linux")]
fn connect_with_retry(
    address: SocketAddr,
    child: Option<&mut CandidateProcess>,
) -> Result<TcpStream> {
    let started = Instant::now();
    let mut child = child;
    loop {
        if let Some(child) = child.as_deref_mut()
            && let Some(status) = child.try_wait()?
        {
            return Err(Error::new(format!(
                "isolated reservoir exited before challenge: {status}"
            )));
        }
        match connect_once(address) {
            Ok(stream) => return Ok(stream),
            Err(_) if started.elapsed() < Duration::from_secs(30) => {
                thread::sleep(Duration::from_millis(100));
            },
            Err(error) => return Err(error),
        }
    }
}

#[cfg(target_os = "linux")]
fn read_new_observation(
    stream: &mut TcpStream,
    previous_sequence: u64,
    previous_t_ms: u64,
) -> Result<SpectralObservation> {
    for _ in 0..8 {
        let observation = read_observation(stream)?;
        if observation.sequence > previous_sequence && observation.t_ms > previous_t_ms {
            return Ok(observation);
        }
    }
    Err(Error::new(
        "reservoir challenge telemetry did not advance within bound",
    ))
}

#[cfg(target_os = "linux")]
fn read_observation(stream: &mut TcpStream) -> Result<SpectralObservation> {
    let bytes = read_websocket_text(stream, MAX_PACKET_BYTES)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    parse_observation(&value)
}

#[cfg(any(target_os = "linux", test))]
#[cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, reason = "live packet parser is exercised on Linux")
)]
#[allow(
    clippy::too_many_lines,
    reason = "one exact wire-packet parser validates all mutually dependent spectral fields"
)]
fn parse_observation(value: &Value) -> Result<SpectralObservation> {
    if value.pointer("/protocol/name").and_then(Value::as_str) != Some("astrid_minime")
        || value.pointer("/protocol/major").and_then(Value::as_u64) != Some(1)
        || value.pointer("/protocol/minor").and_then(Value::as_u64) != Some(0)
        || value
            .pointer("/spectral_substrate_v1/substrate_kind")
            .and_then(Value::as_str)
            != Some("cpu_edge_covariance_effective_rank")
        || value
            .pointer("/edge_runtime_v1/kind")
            .and_then(Value::as_str)
            != Some("cpu_effective_rank_esn")
        || value
            .pointer("/edge_runtime_v1/reservoir_dim")
            .and_then(Value::as_u64)
            != Some(128)
        || value
            .pointer("/edge_runtime_v1/semantic_fresh")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err(Error::new(
            "challenge telemetry identity or semantic response is not exact",
        ));
    }
    let sequence = exact_u64(value, "/edge_runtime_v1/snapshot_sequence")?;
    let t_ms = exact_u64(value, "/t_ms")?;
    let fill_ratio = exact_f64(value, "/fill_ratio", 0.0, 1.0)?;
    let instantaneous_fill_ratio =
        exact_f64(value, "/edge_runtime_v1/instantaneous_fill_ratio", 0.0, 1.0)?;
    let target = exact_f64(value, "/edge_runtime_v1/fill_target", 0.0, 1.0)?;
    let effective_dimensionality = exact_f64(
        value,
        "/spectral_denominator_v1/effective_dimensionality",
        0.0,
        RESERVOIR_DIMENSIONS,
    )?;
    let spectral_entropy = exact_f64(value, "/spectral_denominator_v1/spectral_entropy", 0.0, 1.0)?;
    let denominator_fill = exact_f64(
        value,
        "/spectral_denominator_v1/instantaneous_fill_ratio",
        0.0,
        1.0,
    )?;
    let expected_fill = effective_dimensionality / RESERVOIR_DIMENSIONS;
    let entropy_fill = (spectral_entropy * RESERVOIR_DIMENSIONS.ln()).exp() / RESERVOIR_DIMENSIONS;
    if (target - 0.68).abs() > 1.0e-6
        || (instantaneous_fill_ratio - denominator_fill).abs() > 1.0e-6
        || (instantaneous_fill_ratio - expected_fill).abs() > 1.0e-4
        || (instantaneous_fill_ratio - entropy_fill).abs() > 2.0e-3
    {
        return Err(Error::new(
            "challenge telemetry spectral denominator is internally inconsistent",
        ));
    }
    let eigenvalues = value
        .get("eigenvalues")
        .and_then(Value::as_array)
        .filter(|values| (8..=32).contains(&values.len()))
        .ok_or_else(|| Error::new("challenge telemetry spectrum is absent or oversized"))?;
    let mut spectrum = Vec::with_capacity(eigenvalues.len());
    for eigenvalue in eigenvalues {
        let number = eigenvalue
            .as_f64()
            .filter(|number| number.is_finite() && *number >= 0.0)
            .ok_or_else(|| Error::new("challenge telemetry eigenvalue is invalid"))?;
        if spectrum
            .last()
            .is_some_and(|previous: &f64| number > *previous + 1.0e-8)
        {
            return Err(Error::new(
                "challenge telemetry eigenvalues are not ordered",
            ));
        }
        spectrum.push(number);
    }
    let total = spectrum.iter().sum::<f64>();
    if !total.is_finite() || total <= f64::EPSILON {
        return Err(Error::new(
            "challenge telemetry spectrum has no positive energy",
        ));
    }
    let normalized = spectrum
        .iter()
        .map(|value| *value / total)
        .collect::<Vec<_>>();
    let entropy_nats = normalized
        .iter()
        .filter(|share| **share > f64::EPSILON)
        .map(|share| -*share * share.ln())
        .sum::<f64>();
    let count = u32::try_from(normalized.len())
        .map_err(|_| Error::new("challenge spectrum count overflow"))?;
    let exported_entropy = entropy_nats / f64::from(count).ln();
    let reservoir_generation_id = value
        .pointer("/edge_runtime_v1/snapshot_generation_id")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty() && text.len() <= 128 && !text.chars().any(char::is_control))
        .ok_or_else(|| Error::new("challenge reservoir generation is invalid"))?
        .to_owned();
    Ok(SpectralObservation {
        sequence,
        t_ms,
        fill_ratio,
        instantaneous_fill_ratio,
        effective_dimensionality,
        spectral_entropy,
        exported_entropy,
        normalized_eigenvalues: normalized,
        reservoir_generation_id,
    })
}

#[cfg(any(target_os = "linux", test))]
fn evaluate_series(
    series: &ResponseSeries,
    expected_reservoir_generation_id: &str,
    expected_input_sha256: &str,
) -> Result<ResponseMetrics> {
    let sample_count = series.observations.len();
    if series.challenge_input_sha256 != expected_input_sha256
        || !crate::config::valid_hex64(expected_input_sha256)
        || !(MINIMUM_RESPONSE_SAMPLES..=MAXIMUM_RESPONSE_SAMPLES).contains(&sample_count)
        || series.input_energy.len() != sample_count
    {
        return Err(Error::new(
            "reservoir challenge response is short or not nonce-input bound",
        ));
    }
    for pair in series.observations.windows(2) {
        if pair[1].sequence <= pair[0].sequence || pair[1].t_ms <= pair[0].t_ms {
            return Err(Error::new(
                "reservoir challenge response is stale or replayed",
            ));
        }
    }
    if series.observations.iter().any(|sample| {
        sample.reservoir_generation_id != expected_reservoir_generation_id
            || !sample.fill_ratio.is_finite()
            || !sample.instantaneous_fill_ratio.is_finite()
            || !sample.spectral_entropy.is_finite()
            || !sample.exported_entropy.is_finite()
            || !sample.effective_dimensionality.is_finite()
    }) {
        return Err(Error::new(
            "reservoir challenge response changed identity or contains invalid values",
        ));
    }
    let fills = series
        .observations
        .iter()
        .map(|sample| sample.fill_ratio)
        .collect::<Vec<_>>();
    let instantaneous = series
        .observations
        .iter()
        .map(|sample| sample.instantaneous_fill_ratio)
        .collect::<Vec<_>>();
    let entropies = series
        .observations
        .iter()
        .map(|sample| sample.spectral_entropy)
        .collect::<Vec<_>>();
    let fill_minimum = finite_min(&fills)?;
    let fill_maximum = finite_max(&fills)?;
    let instantaneous_span = finite_max(&instantaneous)? - finite_min(&instantaneous)?;
    let entropy_span = finite_max(&entropies)? - finite_min(&entropies)?;
    let spectral_deltas = series
        .observations
        .windows(2)
        .map(|pair| spectral_distance(&pair[0], &pair[1]))
        .collect::<Result<Vec<_>>>()?;
    let denominator = f64::from(
        u32::try_from(spectral_deltas.len())
            .map_err(|_| Error::new("challenge response count overflow"))?,
    );
    let spectral_path_mean = spectral_deltas.iter().sum::<f64>() / denominator;
    let unique_spectral_shapes = series
        .observations
        .iter()
        .map(spectral_shape_key)
        .collect::<Result<BTreeSet<_>>>()?
        .len();
    let input_response_correlation =
        maximum_lag_correlation(&series.input_energy[1..], &spectral_deltas, 3)?;
    if instantaneous_span < MINIMUM_DYNAMIC_SPAN
        || entropy_span < MINIMUM_DYNAMIC_SPAN
        || spectral_path_mean < MINIMUM_SPECTRAL_PATH
        || unique_spectral_shapes < 4
        || input_response_correlation < 0.005
    {
        return Err(Error::new(
            "reservoir challenge response is constant or not input-responsive",
        ));
    }
    let output_series_sha256 = sha256(&canonical_json(&series.observations)?);
    let count =
        f64::from(u32::try_from(fills.len()).map_err(|_| Error::new("fill count overflow"))?);
    Ok(ResponseMetrics {
        output_series_sha256,
        first_sequence: series.observations[0].sequence,
        last_sequence: series.observations[sample_count.saturating_sub(1)].sequence,
        fill_mean: fills.iter().sum::<f64>() / count,
        fill_minimum,
        fill_maximum,
        instantaneous_fill_span: instantaneous_span,
        spectral_entropy_span: entropy_span,
        spectral_path_mean,
        unique_spectral_shapes,
        input_response_correlation,
    })
}

#[cfg(any(target_os = "linux", test))]
fn spectral_distance(left: &SpectralObservation, right: &SpectralObservation) -> Result<f64> {
    if left.normalized_eigenvalues.len() != right.normalized_eigenvalues.len() {
        return Err(Error::new(
            "challenge spectrum width changed during response",
        ));
    }
    let shape = left
        .normalized_eigenvalues
        .iter()
        .zip(&right.normalized_eigenvalues)
        .map(|(left, right)| (*left - *right).abs())
        .sum::<f64>();
    Ok(shape
        + (left.instantaneous_fill_ratio - right.instantaneous_fill_ratio).abs()
        + (left.exported_entropy - right.exported_entropy).abs())
}

#[cfg(any(target_os = "linux", test))]
fn spectral_shape_key(sample: &SpectralObservation) -> Result<String> {
    let quantized = sample
        .normalized_eigenvalues
        .iter()
        .take(8)
        .map(|value| format!("{value:.7}"))
        .collect::<Vec<_>>();
    Ok(sha256(&canonical_json(&quantized)?))
}

#[cfg(any(target_os = "linux", test))]
fn maximum_lag_correlation(inputs: &[f64], outputs: &[f64], maximum_lag: usize) -> Result<f64> {
    if inputs.len() != outputs.len() || inputs.len() < 8 {
        return Err(Error::new("challenge correlation coverage is incomplete"));
    }
    let mut maximum = 0.0_f64;
    for lag in 0..=maximum_lag {
        if inputs.len().saturating_sub(lag) < 8 {
            continue;
        }
        let left = &inputs[..inputs.len().saturating_sub(lag)];
        let right = &outputs[lag..];
        maximum = maximum.max(pearson(left, right)?.abs());
    }
    Ok(maximum)
}

#[cfg(any(target_os = "linux", test))]
fn pearson(left: &[f64], right: &[f64]) -> Result<f64> {
    if left.len() != right.len() || left.is_empty() {
        return Err(Error::new("challenge correlation shape is invalid"));
    }
    let count =
        f64::from(u32::try_from(left.len()).map_err(|_| Error::new("correlation count overflow"))?);
    let left_mean = left.iter().sum::<f64>() / count;
    let right_mean = right.iter().sum::<f64>() / count;
    let mut covariance = 0.0_f64;
    let mut left_variance = 0.0_f64;
    let mut right_variance = 0.0_f64;
    for (left, right) in left.iter().zip(right) {
        let left_delta = *left - left_mean;
        let right_delta = *right - right_mean;
        covariance += left_delta * right_delta;
        left_variance += left_delta * left_delta;
        right_variance += right_delta * right_delta;
    }
    let denominator = (left_variance * right_variance).sqrt();
    if !denominator.is_finite() || denominator <= f64::EPSILON {
        return Ok(0.0);
    }
    Ok((covariance / denominator).clamp(-1.0, 1.0))
}

#[cfg(any(target_os = "linux", test))]
fn finite_min(values: &[f64]) -> Result<f64> {
    let value = values.iter().copied().fold(f64::INFINITY, f64::min);
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| Error::new("challenge minimum is non-finite"))
}

#[cfg(any(target_os = "linux", test))]
fn finite_max(values: &[f64]) -> Result<f64> {
    let value = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| Error::new("challenge maximum is non-finite"))
}

#[cfg(target_os = "linux")]
fn create_plan() -> Result<ChallengePlan> {
    let mut nonce = [0_u8; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut nonce)?;
    create_plan_from_nonce(nonce)
}

#[cfg(any(target_os = "linux", test))]
fn create_plan_from_nonce(nonce: [u8; 32]) -> Result<ChallengePlan> {
    let mut seed_bytes = [0_u8; 8];
    seed_bytes.copy_from_slice(&nonce[..8]);
    let mut rng = XorShift64::new(u64::from_le_bytes(seed_bytes));
    let dynamic_samples = MINIMUM_RESPONSE_SAMPLES
        .saturating_sub(RECOVERY_SAMPLES)
        .saturating_add(usize::from(nonce[8] % 9));
    let total_samples = dynamic_samples.saturating_add(RECOVERY_SAMPLES);
    let mut packets = Vec::with_capacity(total_samples);
    let mut input_energy = Vec::with_capacity(total_samples);
    let mut pacing_milliseconds = Vec::with_capacity(total_samples);
    let mut input_bytes = Vec::new();
    for sample in 0..total_samples {
        let recovery = sample >= dynamic_samples;
        let amplitude_index = usize::try_from(rng.next_u64() % 4).unwrap_or_default();
        let amplitude = [0.06_f64, 0.14, 0.24, 0.34][amplitude_index];
        let features = (0..48)
            .map(|_| {
                if recovery {
                    0.0_f64
                } else {
                    (rng.next_unit() * 2.0 - 1.0) * amplitude
                }
            })
            .collect::<Vec<_>>();
        let count = f64::from(
            u32::try_from(features.len())
                .map_err(|_| Error::new("challenge feature count overflow"))?,
        );
        let energy = (features.iter().map(|value| value * value).sum::<f64>() / count).sqrt();
        let packet = canonical_json(&json!({
            "protocol": {"name":"astrid_minime","major":1,"minor":3},
            "kind": "semantic",
            "features": features,
            "ts_ms": u64::try_from(sample).unwrap_or(u64::MAX).saturating_mul(71),
        }))?;
        if packet.len() > 8 * 1024 {
            return Err(Error::new("challenge semantic packet exceeds bound"));
        }
        input_bytes.extend_from_slice(&packet);
        input_bytes.push(b'\n');
        packets.push(packet);
        input_energy.push(energy);
        pacing_milliseconds.push(55_u64.saturating_add(rng.next_u64() % 41));
    }
    Ok(ChallengePlan {
        nonce_sha256: sha256(&nonce),
        input_sha256: sha256(&input_bytes),
        packets,
        input_energy,
        pacing_milliseconds,
        reference_seed: rng.next_u64().max(1),
    })
}

#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Clone, Copy)]
struct XorShift64(u64);

#[cfg(any(target_os = "linux", test))]
impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        value
    }

    fn next_unit(&mut self) -> f64 {
        let upper = u32::try_from(self.next_u64() >> 32).unwrap_or(u32::MAX);
        f64::from(upper) / f64::from(u32::MAX)
    }
}

#[cfg(any(target_os = "linux", test))]
#[allow(
    clippy::too_many_arguments,
    reason = "all response-lineage fields are explicit inputs to one canonical digest"
)]
fn bound_response_digest(
    appliance_id: &str,
    nonce_sha256: &str,
    input_sha256: &str,
    generation_id: &str,
    binary_sha256: &str,
    output_series_sha256: &str,
    started_at_unix_ms: u64,
    completed_at_unix_ms: u64,
) -> Result<String> {
    Ok(sha256(&canonical_json(&serde_json::json!({
        "schema": "astrid.edge_rescue_helper.bound_reservoir_response.v1",
        "appliance_id": appliance_id,
        "challenge_nonce_sha256": nonce_sha256,
        "challenge_input_sha256": input_sha256,
        "generation_id": generation_id,
        "binary_sha256": binary_sha256,
        "output_series_sha256": output_series_sha256,
        "started_at_unix_ms": started_at_unix_ms,
        "completed_at_unix_ms": completed_at_unix_ms,
    }))?))
}

#[cfg(any(target_os = "linux", test))]
fn evidence_digest(evidence: &ProbationReservoirChallengeEvidence) -> Result<String> {
    let mut value = serde_json::to_value(evidence)?;
    value
        .as_object_mut()
        .ok_or_else(|| Error::new("challenge evidence is not an object"))?
        .remove("evidence_sha256");
    Ok(sha256(&canonical_json(&value)?))
}

#[cfg(any(target_os = "linux", test))]
fn validate_evidence(evidence: &ProbationReservoirChallengeEvidence) -> Result<()> {
    let expected_candidate_bound = bound_response_digest(
        &evidence.appliance_id,
        &evidence.challenge_nonce_sha256,
        &evidence.challenge_input_sha256,
        &evidence.active_generation_id,
        &evidence.active_edge_binary_sha256,
        &evidence.candidate_output_series_sha256,
        evidence.challenge_started_at_unix_ms,
        evidence.challenge_completed_at_unix_ms,
    )?;
    let expected_reference_bound = bound_response_digest(
        &evidence.appliance_id,
        &evidence.challenge_nonce_sha256,
        &evidence.challenge_input_sha256,
        &evidence.reference_generation_id,
        &evidence.reference_edge_binary_sha256,
        &evidence.reference_output_series_sha256,
        evidence.challenge_started_at_unix_ms,
        evidence.challenge_completed_at_unix_ms,
    )?;
    if evidence.schema != "astrid.edge_rescue_helper.probation_reservoir_challenge.v1"
        || evidence.provenance != "synthetic_root_test_machine_evidence_not_astrid_authorship"
        || evidence.authority
            != "root_nonce_isolated_sealed_candidate_plus_exact_previous_generation_reference"
        || !crate::config::valid_identifier(&evidence.appliance_id)
        || !crate::config::valid_hex64(&evidence.challenge_nonce_sha256)
        || !crate::config::valid_hex64(&evidence.challenge_input_sha256)
        || !crate::config::valid_hex64(&evidence.active_edge_binary_sha256)
        || !crate::config::valid_hex64(&evidence.reference_edge_binary_sha256)
        || !crate::config::valid_hex64(&evidence.candidate_output_series_sha256)
        || !crate::config::valid_hex64(&evidence.reference_output_series_sha256)
        || evidence.candidate_bound_response_sha256 != expected_candidate_bound
        || evidence.reference_bound_response_sha256 != expected_reference_bound
        || evidence.challenge_completed_at_unix_ms < evidence.challenge_started_at_unix_ms
        || evidence
            .challenge_completed_at_unix_ms
            .saturating_sub(evidence.challenge_started_at_unix_ms)
            > MAXIMUM_CHALLENGE_MILLISECONDS
        || evidence.active_generation_id == evidence.reference_generation_id
        || evidence.active_edge_main_pid == 0
        || evidence.live_reservoir_generation_id.is_empty()
        || evidence.live_reservoir_generation_id.len() > 128
        || evidence.live_telemetry_t_ms == 0
        || evidence.input_samples < MINIMUM_RESPONSE_SAMPLES
        || evidence.input_samples > MAXIMUM_RESPONSE_SAMPLES
        || evidence.candidate_samples != evidence.input_samples
        || evidence.reference_samples != evidence.input_samples
        || evidence.candidate_last_sequence <= evidence.candidate_first_sequence
        || evidence.reference_last_sequence <= evidence.reference_first_sequence
        || !evidence.challenge_passed
        || evidence.continuity_or_reservoir_admission
        || evidence.evidence_sha256 != evidence_digest(evidence)?
    {
        return Err(Error::new(
            "probation reservoir challenge evidence binding is invalid",
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", test))]
#[cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, reason = "live packet parser is exercised on Linux")
)]
fn exact_u64(value: &Value, pointer: &str) -> Result<u64> {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new(format!("challenge telemetry integer is absent: {pointer}")))
}

#[cfg(any(target_os = "linux", test))]
#[cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, reason = "live packet parser is exercised on Linux")
)]
fn exact_f64(value: &Value, pointer: &str, minimum: f64, maximum: f64) -> Result<f64> {
    value
        .pointer(pointer)
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite() && (minimum..=maximum).contains(number))
        .ok_or_else(|| Error::new(format!("challenge telemetry number is invalid: {pointer}")))
}

#[cfg(target_os = "linux")]
fn verify_process_binary(pid: u64, expected: &Path) -> Result<()> {
    let observed = fs::canonicalize(Path::new("/proc").join(pid.to_string()).join("exe"))?;
    let expected = fs::canonicalize(expected)?;
    if observed != expected {
        return Err(Error::new(
            "live reservoir process is not the sealed active binary",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn cleanup_replay_root(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Error::new("probation reservoir replay root changed type"));
    }
    fs::remove_dir_all(path)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::{
        ChallengePlan, ProbationReservoirChallengeEvidence, ResponseSeries, SpectralObservation,
        bound_response_digest, create_plan_from_nonce, evaluate_series, evidence_digest,
        isolated_process_roots, validate_evidence,
    };
    use crate::fs_guard::{canonical_json, sha256};

    fn observation(index: usize, dynamic: bool) -> SpectralObservation {
        let index_u32 = u32::try_from(index).unwrap();
        let phase = f64::from(index_u32) * 0.31;
        let entropy = if dynamic {
            0.914 + 0.002 * phase.sin()
        } else {
            0.914
        };
        let effective = (entropy * 128.0_f64.ln()).exp();
        let mut raw = (0..16_u32)
            .map(|mode| {
                let base = 17.0 - f64::from(mode);
                if dynamic {
                    base * (1.0 + 0.025 * (phase + f64::from(mode) * 0.17).sin())
                } else {
                    base
                }
            })
            .collect::<Vec<_>>();
        raw.sort_by(|left, right| right.total_cmp(left));
        let total = raw.iter().sum::<f64>();
        let normalized = raw.iter().map(|value| *value / total).collect::<Vec<_>>();
        let exported_entropy = -normalized
            .iter()
            .map(|share| *share * share.ln())
            .sum::<f64>()
            / 16.0_f64.ln();
        SpectralObservation {
            sequence: 1_000 + u64::from(index_u32),
            t_ms: 10_000_u64.saturating_add(u64::from(index_u32).saturating_mul(71)),
            fill_ratio: 0.68,
            instantaneous_fill_ratio: effective / 128.0,
            effective_dimensionality: effective,
            spectral_entropy: entropy,
            exported_entropy,
            normalized_eigenvalues: normalized,
            reservoir_generation_id: "reservoir-live".into(),
        }
    }

    fn response(plan: &ChallengePlan, dynamic: bool) -> ResponseSeries {
        ResponseSeries {
            challenge_input_sha256: plan.input_sha256.clone(),
            observations: (0..plan.packets.len())
                .map(|index| observation(index, dynamic))
                .collect(),
            input_energy: plan.input_energy.clone(),
        }
    }

    #[test]
    fn forged_constant_fill_and_matching_static_spectrum_are_rejected() {
        let plan = create_plan_from_nonce([7; 32]).unwrap();
        let forged = response(&plan, false);
        assert!(evaluate_series(&forged, "reservoir-live", &plan.input_sha256).is_err());
    }

    #[test]
    fn stale_short_and_prior_challenge_replays_are_rejected() {
        let plan = create_plan_from_nonce([9; 32]).unwrap();
        let mut stale = response(&plan, true);
        stale.observations[5].sequence = stale.observations[4].sequence;
        assert!(evaluate_series(&stale, "reservoir-live", &plan.input_sha256).is_err());

        let mut short = response(&plan, true);
        short.observations.truncate(8);
        short.input_energy.truncate(8);
        assert!(evaluate_series(&short, "reservoir-live", &plan.input_sha256).is_err());

        let other = create_plan_from_nonce([10; 32]).unwrap();
        let replay = response(&plan, true);
        assert!(evaluate_series(&replay, "reservoir-live", &other.input_sha256).is_err());
    }

    #[test]
    fn evidence_is_bound_to_nonce_generation_binary_series_and_window() {
        let started = 1_000;
        let completed = 2_000;
        let nonce = "a".repeat(64);
        let input = "b".repeat(64);
        let active_binary = "c".repeat(64);
        let reference_binary = "d".repeat(64);
        let candidate_output = "e".repeat(64);
        let reference_output = "f".repeat(64);
        let mut evidence = ProbationReservoirChallengeEvidence {
            schema: "astrid.edge_rescue_helper.probation_reservoir_challenge.v1",
            appliance_id: "avado-test".into(),
            provenance: "synthetic_root_test_machine_evidence_not_astrid_authorship",
            authority: "root_nonce_isolated_sealed_candidate_plus_exact_previous_generation_reference",
            challenge_nonce_sha256: nonce.clone(),
            challenge_input_sha256: input.clone(),
            challenge_started_at_unix_ms: started,
            challenge_completed_at_unix_ms: completed,
            active_generation_id: "gen-new".into(),
            active_edge_binary_sha256: active_binary.clone(),
            active_edge_main_pid: 42,
            live_reservoir_generation_id: "reservoir-production".into(),
            live_telemetry_t_ms: 900,
            active_reservoir_generation_id: "reservoir-new".into(),
            reference_generation_id: "gen-old".into(),
            reference_edge_binary_sha256: reference_binary.clone(),
            reference_reservoir_generation_id: "reservoir-old".into(),
            candidate_output_series_sha256: candidate_output.clone(),
            reference_output_series_sha256: reference_output.clone(),
            candidate_bound_response_sha256: bound_response_digest(
                "avado-test",
                &nonce,
                &input,
                "gen-new",
                &active_binary,
                &candidate_output,
                started,
                completed,
            )
            .unwrap(),
            reference_bound_response_sha256: bound_response_digest(
                "avado-test",
                &nonce,
                &input,
                "gen-old",
                &reference_binary,
                &reference_output,
                started,
                completed,
            )
            .unwrap(),
            input_samples: 24,
            candidate_samples: 24,
            reference_samples: 24,
            candidate_first_sequence: 1,
            candidate_last_sequence: 24,
            reference_first_sequence: 2,
            reference_last_sequence: 25,
            candidate_fill_mean: 0.68,
            candidate_fill_minimum: 0.67,
            candidate_fill_maximum: 0.69,
            candidate_instantaneous_fill_span: 0.01,
            candidate_spectral_entropy_span: 0.01,
            candidate_spectral_path_mean: 0.01,
            candidate_unique_spectral_shapes: 24,
            candidate_input_response_correlation: 0.2,
            reference_instantaneous_fill_span: 0.01,
            reference_spectral_entropy_span: 0.01,
            reference_spectral_path_mean: 0.01,
            reference_unique_spectral_shapes: 24,
            reference_input_response_correlation: 0.2,
            candidate_reference_response_ratio: 1.0,
            challenge_passed: true,
            continuity_or_reservoir_admission: false,
            evidence_sha256: String::new(),
        };
        evidence.evidence_sha256 = evidence_digest(&evidence).unwrap();
        assert!(validate_evidence(&evidence).is_ok());

        let prior = evidence.clone();
        evidence.challenge_nonce_sha256 = "0".repeat(64);
        evidence.evidence_sha256 = evidence_digest(&evidence).unwrap();
        assert!(validate_evidence(&evidence).is_err());
        assert_ne!(
            sha256(&canonical_json(&prior).unwrap()),
            sha256(&canonical_json(&evidence).unwrap())
        );
    }

    #[test]
    fn nonce_changes_shape_pacing_and_input_binding() {
        let first = create_plan_from_nonce([1; 32]).unwrap();
        let second = create_plan_from_nonce([2; 32]).unwrap();
        assert_ne!(first.nonce_sha256, second.nonce_sha256);
        assert_ne!(first.input_sha256, second.input_sha256);
        assert_ne!(first.pacing_milliseconds, second.pacing_milliseconds);
        assert!((24..=36).contains(&first.packets.len()));
    }

    #[test]
    fn candidate_and_reference_use_distinct_direct_transaction_roots() {
        let work = std::path::Path::new("/var/lib/astrid-edge-builder/work");
        let replay_id = uuid::Uuid::parse_str("018f5f64-8a21-7b4d-a746-91b40ecdc2c2").unwrap();
        let (candidate, reference) = isolated_process_roots(work, replay_id);
        assert_ne!(candidate, reference);
        assert_eq!(candidate.parent(), Some(work));
        assert_eq!(reference.parent(), Some(work));
        assert!(
            candidate
                .ends_with("probation-reservoir-candidate-018f5f64-8a21-7b4d-a746-91b40ecdc2c2")
        );
        assert!(
            reference
                .ends_with("probation-reservoir-reference-018f5f64-8a21-7b4d-a746-91b40ecdc2c2")
        );
    }
}
