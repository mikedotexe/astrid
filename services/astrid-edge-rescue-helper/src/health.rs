//! Immutable probation health gates over live services and independently verified state.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::fs_guard::{canonical_json, read_json, read_regular, sha256, sha256_file};
use crate::health_ledger::tail_fills;
use crate::health_telemetry::{DirectHostSample, RuntimeStateReference, observe_live_telemetry};
use crate::native::{CommandSpec, NativeRunner, require_success};
use crate::reservoir_challenge::ProbationReservoirChallengeEvidence;
use crate::transition::read_generation_binding;
use crate::{Error, Result, probation, reservoir_challenge};

const FUTURE_SKEW_MS: u64 = 5 * 60 * 1_000;
const HOUR_MS: u64 = 60 * 60 * 1_000;
const PROVIDER_CANARY_MAXIMUM_AGE_SECONDS: u64 = 2 * 60 * 60;
const FIVE_SECOND_SAMPLES_PER_HOUR: usize = 720;
const PROBATION_FILL_MINIMUM_SAMPLES: usize = 648;

#[derive(Debug, Clone, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct HealthReport {
    pub schema: &'static str,
    pub appliance_id: String,
    pub checked_at_unix_ms: u64,
    pub host_boot_id: String,
    pub active_generation_id: String,
    pub healthy: bool,
    pub services: Vec<ServiceHealth>,
    pub cognition_graph: CognitionGraphHealth,
    pub live_telemetry: LiveTelemetryHealth,
    pub sensor_schema: String,
    pub sensor_recorded_at_unix_ms: u64,
    pub sensor_age_seconds: u64,
    pub sensor_hash_verified: bool,
    pub sensor_fresh: bool,
    pub audio_fresh: bool,
    pub audio_source: String,
    pub aux_fresh: bool,
    pub aux_source: String,
    pub hindsight: HindsightAttestation,
    pub hindsight_activation_baseline_sha256: Option<String>,
    pub hindsight_activation_baseline_verified: bool,
    pub available_ram_bytes: u64,
    pub swap_bytes: u64,
    pub workspace_filesystem_device: u64,
    pub workspace_filesystem_available_bytes: u64,
    pub workspace_filesystem_minimum_bytes: u64,
    pub workspace_storage_healthy: bool,
    pub bounded_storage: crate::storage::StorageAttestation,
    pub thermal_celsius: f64,
    pub fill_samples: usize,
    pub fill_expected_samples: usize,
    pub fill_coverage_seconds: u64,
    pub fill_max_gap_seconds: f64,
    pub fill_mean: f64,
    pub fill_occupancy_65_735: f64,
    pub probation_fill_coverage_complete: bool,
    pub fill_history_snapshot: RuntimeLedgerSnapshot,
    pub reservoir_challenge: Option<ProbationReservoirChallengeEvidence>,
    pub probation: Option<probation::ProbationEvaluation>,
    pub evidence_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceHealth {
    pub unit: String,
    pub active_state: String,
    pub sub_state: String,
    pub restarts: u64,
    pub main_pid: u64,
    pub control_group: String,
    pub proc_cgroup_verified: bool,
    pub executable_path: Option<String>,
    pub active_generation_executable_verified: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CognitionGraphHealth {
    pub loaded_capsules: Vec<String>,
    pub loaded_capsule_count: usize,
    pub exact_twenty_verified: bool,
    pub required_cognition_path: Vec<&'static str>,
    pub missing_cognition_path: Vec<&'static str>,
    pub declared_edges: Vec<CognitionDeclaredEdge>,
    pub declared_interfaces: Vec<CognitionDeclaredInterface>,
    pub manifest_sha256: BTreeMap<String, String>,
    pub declared_graph_sha256: String,
    pub exact_declared_graph_verified: bool,
    pub missing_declared_edges: Vec<String>,
    pub missing_declared_interfaces: Vec<String>,
    pub provider_gateway: ProviderGatewayHealth,
    pub status_sha256: String,
    pub authority: &'static str,
    pub residual_limitation: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct CognitionDeclaredEdge {
    pub from_capsule: &'static str,
    pub publish: &'static str,
    pub to_capsule: &'static str,
    pub subscribe: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct CognitionDeclaredInterface {
    pub provider_capsule: &'static str,
    pub export: &'static str,
    pub consumer_capsule: &'static str,
    pub import: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderGatewayHealth {
    pub model: String,
    pub completed_at_unix_ms: u64,
    pub age_seconds: u64,
    pub elapsed_ms: u64,
    pub gateway_wire_sha256: String,
    pub provider_body_sha256: String,
    pub model_response_sha256: String,
    pub model_response_bytes: u64,
    pub canonical_response_verified: bool,
    pub fresh: bool,
    pub receipt_sha256: String,
    pub authority: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderCanaryReceiptV3 {
    schema: String,
    model: String,
    status: String,
    completed_at_unix_ms: u64,
    elapsed_ms: u64,
    keep_alive: String,
    gateway_wire_sha256: String,
    provider_body_sha256: String,
    model_response_sha256: String,
    model_response_bytes: u64,
    canonical_response_verified: bool,
    authority: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveTelemetryHealth {
    pub endpoint: String,
    pub protocol_major: u64,
    pub protocol_minor: u64,
    pub t_ms: u64,
    pub fill_ratio: f64,
    pub snapshot_generation_id: String,
    pub state_correlation: LiveStateCorrelation,
    pub audio: LiveAudioHealth,
    pub auxiliary: LiveAuxiliaryHealth,
    pub observed_sha256: String,
    pub authority: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveStateCorrelation {
    pub generation_matches: bool,
    pub fill_delta: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveAudioHealth {
    pub audio_fresh: bool,
    pub audio_source: String,
    pub audio_rms: f64,
    pub audio_policy_verified: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveAuxiliaryHealth {
    pub aux_fresh: bool,
    pub aux_source: String,
    pub host_memory_delta: f64,
    pub host_thermal_delta: f64,
    pub host_aux_crosscheck_verified: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeLedgerSnapshot {
    pub device: u64,
    pub inode: u64,
    pub captured_size: u64,
    pub prefix_sha256: String,
    pub prior_prefix_verified: bool,
    pub continuity_status: &'static str,
    pub authority: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HindsightAttestation {
    pub schema: String,
    pub checked_at_unix_ms: u64,
    pub generation_id: String,
    pub host_boot_id: String,
    pub checkpoint_recorded_at_unix_ms: u64,
    pub checkpoint_age_seconds: u64,
    pub continuity_epoch: String,
    pub checkpoint_record_sha256: String,
    pub checkpoint_chain_records: usize,
    pub ledger_prefixes_verified: usize,
    pub ledger_prefix_bytes_verified: u64,
    pub operator_database_quick_check: String,
    pub operator_database_schema_version: u64,
    pub operator_database_sha256: String,
    pub authority: String,
    pub evidence_sha256: String,
}

#[allow(clippy::too_many_lines)]
pub fn check<R: NativeRunner>(config: &Config, runner: &mut R) -> Result<HealthReport> {
    let checked_at = unix_millis();
    let host_boot_id = current_boot_id()?;
    let active_generation_id = read_generation_binding(config, true)?;
    let bounded_storage = crate::storage::verify(config, false)?;
    let mut services = Vec::new();
    for (unit, expected_binary) in [
        (&config.services.core, Some("astrid-daemon")),
        (&config.services.warmup, None),
        (&config.services.edge, Some("astrid-edge-runtime")),
    ] {
        let active = property(config, runner, unit, "ActiveState")?;
        let sub = property(config, runner, unit, "SubState")?;
        let restarts = property(config, runner, unit, "NRestarts")?
            .parse::<u64>()
            .map_err(|_| Error::new("systemd NRestarts is malformed"))?;
        let main_pid = property(config, runner, unit, "MainPID")?
            .parse::<u64>()
            .map_err(|_| Error::new("systemd MainPID is malformed"))?;
        let control_group = property(config, runner, unit, "ControlGroup")?;
        let (proc_cgroup_verified, executable_path, active_generation_executable_verified) =
            if let Some(binary) = expected_binary {
                verify_live_process_binding(
                    main_pid,
                    &control_group,
                    &config.roots.active_link,
                    binary,
                )?
            } else {
                (main_pid == 0 && sub == "exited", None, true)
            };
        services.push(ServiceHealth {
            unit: unit.clone(),
            active_state: active,
            sub_state: sub,
            restarts,
            main_pid,
            control_group,
            proc_cgroup_verified,
            executable_path,
            active_generation_executable_verified,
        });
    }
    let cognition_graph = verify_cognition_graph(config, runner)?;
    let sensor: Value = read_json(&config.health.sensor_state, 256 * 1024)?;
    let sensor_schema = sensor
        .get("schema")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("sensor state schema is absent"))?
        .to_owned();
    if !matches!(
        sensor_schema.as_str(),
        "astrid_edge_spectral_state_v1" | "astrid_edge_spectral_state_v2"
    ) {
        return Err(Error::new("sensor state schema is unsupported"));
    }
    let sensor_recorded_at = sensor
        .get("recorded_at_unix_ms")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("sensor state timestamp is absent"))?;
    let sensor_fresh = sensor_recorded_at > 0
        && sensor_recorded_at <= checked_at.saturating_add(FUTURE_SKEW_MS)
        && checked_at.saturating_sub(sensor_recorded_at)
            <= config.health.maximum_age_seconds.saturating_mul(1_000);
    let sensor_hash_verified = verify_sensor_hash(&sensor, &sensor_schema)?;
    let audio_fresh = sensor
        .get("audio_fresh")
        .and_then(Value::as_bool)
        .ok_or_else(|| Error::new("sensor audio freshness is absent"))?;
    let audio_source = bounded_string(&sensor, "audio_source")?;
    let aux_fresh = sensor
        .get("aux_fresh")
        .and_then(Value::as_bool)
        .ok_or_else(|| Error::new("sensor auxiliary freshness is absent"))?;
    let aux_source = bounded_string(&sensor, "aux_source")?;
    let sensor_generation_id = bounded_string(&sensor, "generation_id")?;
    let sensor_fill = sensor
        .get("fill_ratio")
        .and_then(Value::as_f64)
        .filter(|fill| fill.is_finite() && (0.0..=1.0).contains(fill))
        .ok_or_else(|| Error::new("sensor fill is absent or invalid"))?;
    if sensor_schema == "astrid_edge_spectral_state_v2"
        && (!sensor_hash_verified
            || sensor
                .get("generation_id")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty))
    {
        return Err(Error::new(
            "v2 sensor state lacks exact hash or reservoir generation",
        ));
    }
    let hindsight = independently_verify_hindsight(
        config,
        runner,
        &active_generation_id,
        &host_boot_id,
        checked_at,
    )?;
    let (total_ram_bytes, available_ram_bytes) = mem_total_available(&config.health.meminfo)?;
    let swap_bytes = swap_used(&config.health.swaps)?;
    let (workspace_filesystem_device, workspace_filesystem_available_bytes) =
        workspace_storage(&config.roots.workspace)?;
    let workspace_storage_healthy =
        workspace_filesystem_available_bytes >= config.policy.minimum_free_disk_bytes;
    let thermal_celsius = temperature(&config.health.thermal_celsius)?;
    let live_telemetry = observe_live_telemetry(
        config,
        &RuntimeStateReference {
            generation_id: &sensor_generation_id,
            fill_ratio: sensor_fill,
        },
        &DirectHostSample {
            total_ram_bytes,
            available_ram_bytes,
            thermal_celsius,
        },
    )?;
    let probation_lineage = probation::active_lineage(config, &active_generation_id)?;
    let hindsight_activation_baseline_verified = if let Some(lineage) = &probation_lineage {
        probation::verify_hindsight_baseline(config, &lineage.hindsight_baseline, &hindsight)?;
        true
    } else {
        false
    };
    let hindsight_activation_baseline_sha256 = probation_lineage
        .as_ref()
        .map(|lineage| lineage.hindsight_baseline.evidence_sha256.clone());
    let probation_started_at = probation_lineage
        .as_ref()
        .map(|lineage| lineage.started_at_unix_ms);
    let since = probation_started_at
        .unwrap_or_else(|| checked_at.saturating_sub(HOUR_MS))
        .max(checked_at.saturating_sub(HOUR_MS));
    let prefix_expectation = probation::runtime_prefix_expectation(config, &active_generation_id)?;
    let (fills, fill_history_snapshot) = tail_fills(
        &config.health.fill_history,
        since,
        checked_at,
        config.identities.runtime_uid,
        prefix_expectation.as_ref(),
    )?;
    let fill_samples = fills.len();
    let fill_expected_samples = if probation_started_at.is_some() {
        usize::try_from(checked_at.saturating_sub(since) / 5_000)
            .unwrap_or(FIVE_SECOND_SAMPLES_PER_HOUR)
            .min(FIVE_SECOND_SAMPLES_PER_HOUR)
    } else {
        config.health.minimum_fill_samples
    };
    let fill_mean = mean(fills.iter().map(|sample| sample.fill))?;
    let occupied = fills
        .iter()
        .filter(|sample| (0.65..=0.735).contains(&sample.fill))
        .count();
    let fill_occupancy_65_735 = ratio(occupied, fill_samples)?;
    let fill_coverage_seconds = fills.first().zip(fills.last()).map_or(0, |(first, last)| {
        last.recorded_at_unix_ms
            .saturating_sub(first.recorded_at_unix_ms)
            / 1_000
    });
    let fill_max_gap_seconds = fills
        .windows(2)
        .map(|pair| {
            let milliseconds = pair[1]
                .recorded_at_unix_ms
                .saturating_sub(pair[0].recorded_at_unix_ms);
            let milliseconds = u32::try_from(milliseconds).unwrap_or(u32::MAX);
            f64::from(milliseconds) / 1_000.0
        })
        .fold(0.0_f64, f64::max);
    let probation_elapsed =
        probation_started_at.is_some_and(|started| checked_at.saturating_sub(started) >= HOUR_MS);
    let probation_fill_coverage_complete = !probation_elapsed
        || (fill_samples >= PROBATION_FILL_MINIMUM_SAMPLES
            && fill_coverage_seconds >= 57 * 60
            && fill_max_gap_seconds <= 20.0);
    let reservoir_challenge = if let Some(lineage) = &probation_lineage {
        let edge_main_pid = services
            .iter()
            .find(|service| service.unit == config.services.edge)
            .map(|service| service.main_pid)
            .ok_or_else(|| Error::new("edge service health identity is absent"))?;
        Some(reservoir_challenge::run(
            config,
            &active_generation_id,
            &lineage.previous_generation_id,
            edge_main_pid,
            &live_telemetry.snapshot_generation_id,
            live_telemetry.t_ms,
        )?)
    } else {
        None
    };
    let healthy = services.iter().all(|service| {
        service.active_state == "active"
            && matches!(service.sub_state.as_str(), "running" | "exited")
            && service.restarts == 0
            && service.proc_cgroup_verified
            && service.active_generation_executable_verified
    }) && cognition_graph.exact_twenty_verified
        && cognition_graph.missing_cognition_path.is_empty()
        && cognition_graph.exact_declared_graph_verified
        && cognition_graph.provider_gateway.canonical_response_verified
        && cognition_graph.provider_gateway.fresh
        && sensor_fresh
        && sensor_hash_verified
        && live_telemetry.state_correlation.generation_matches
        && live_telemetry.state_correlation.fill_delta <= 0.05
        && live_telemetry.audio.audio_policy_verified
        && live_telemetry.auxiliary.aux_fresh
        && live_telemetry.auxiliary.host_aux_crosscheck_verified
        && audio_fresh == live_telemetry.audio.audio_fresh
        && audio_source == live_telemetry.audio.audio_source
        && aux_fresh == live_telemetry.auxiliary.aux_fresh
        && aux_source == live_telemetry.auxiliary.aux_source
        && fill_history_snapshot.prior_prefix_verified
        && aux_fresh
        && hindsight.operator_database_quick_check == "ok"
        && hindsight.generation_id == active_generation_id
        && hindsight.host_boot_id == host_boot_id
        && (probation_lineage.is_none() || hindsight_activation_baseline_verified)
        && available_ram_bytes >= config.health.minimum_available_ram_bytes
        && swap_bytes <= config.health.maximum_swap_bytes
        && workspace_storage_healthy
        && thermal_celsius <= config.health.maximum_thermal_celsius
        && fill_samples >= config.health.minimum_fill_samples
        && fill_samples >= fill_expected_samples.min(config.health.minimum_fill_samples)
        && (0.67..=0.70).contains(&fill_mean)
        && fill_occupancy_65_735 >= 0.90
        && probation_fill_coverage_complete
        && probation_lineage.is_none() == reservoir_challenge.is_none()
        && reservoir_challenge
            .as_ref()
            .is_none_or(|challenge| challenge.challenge_passed);
    let mut report = HealthReport {
        schema: "astrid.edge_rescue_helper.health.v2",
        appliance_id: config.appliance_id.clone(),
        checked_at_unix_ms: checked_at,
        host_boot_id,
        active_generation_id,
        healthy,
        services,
        cognition_graph,
        live_telemetry,
        sensor_schema,
        sensor_recorded_at_unix_ms: sensor_recorded_at,
        sensor_age_seconds: checked_at.saturating_sub(sensor_recorded_at) / 1_000,
        sensor_hash_verified,
        sensor_fresh,
        audio_fresh,
        audio_source,
        aux_fresh,
        aux_source,
        hindsight,
        hindsight_activation_baseline_sha256,
        hindsight_activation_baseline_verified,
        available_ram_bytes,
        swap_bytes,
        workspace_filesystem_device,
        workspace_filesystem_available_bytes,
        workspace_filesystem_minimum_bytes: config.policy.minimum_free_disk_bytes,
        workspace_storage_healthy,
        bounded_storage,
        thermal_celsius,
        fill_samples,
        fill_expected_samples,
        fill_coverage_seconds,
        fill_max_gap_seconds,
        fill_mean,
        fill_occupancy_65_735,
        probation_fill_coverage_complete,
        fill_history_snapshot,
        reservoir_challenge,
        probation: None,
        evidence_sha256: String::new(),
    };
    report.probation = probation::record_health(config, &report)?;
    report.evidence_sha256 = report_digest(&report)?;
    if !healthy || report.probation.as_ref().is_some_and(|state| state.failed) {
        return Err(Error::new(serde_json::to_string(&report)?));
    }
    Ok(report)
}

fn verify_cognition_graph<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
) -> Result<CognitionGraphHealth> {
    let astrid_home = config
        .roots
        .workspace
        .ancestors()
        .nth(3)
        .ok_or_else(|| Error::new("workspace state root is unavailable"))?;
    let executable_path = config.roots.active_link.join("astrid");
    let executable = crate::config::TrustedExecutable {
        path: executable_path.clone(),
        sha256: sha256_file(&executable_path, 512 * 1024 * 1024)?,
    };
    let spec = CommandSpec {
        label: "immutable-cognition-graph-status",
        executable,
        arguments: vec![
            "--format".to_owned(),
            "json".to_owned(),
            "status".to_owned(),
        ],
        current_dir: config.roots.workspace.clone(),
        environment: BTreeMap::from([
            ("ASTRID_HOME".to_owned(), astrid_home.display().to_string()),
            ("HOME".to_owned(), astrid_home.display().to_string()),
            ("LANG".to_owned(), "C.UTF-8".to_owned()),
            ("LC_ALL".to_owned(), "C.UTF-8".to_owned()),
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
            ("TZ".to_owned(), "UTC".to_owned()),
        ]),
        timeout: Duration::from_secs(30),
        run_as_uid: Some(config.identities.runtime_uid),
        run_as_gid: Some(config.identities.runtime_gid),
    };
    let (receipt, output) = runner.run_capture(&spec, 256 * 1024)?;
    require_success(&receipt)?;
    let status = parse_cognition_graph_status(&output)?;
    let declared = verify_declared_cognition_graph(&config.roots.active_link)?;
    let provider_gateway = verify_provider_gateway_canary(config, unix_millis())?;
    Ok(CognitionGraphHealth {
        loaded_capsules: status.loaded_capsules,
        loaded_capsule_count: status.loaded_capsule_count,
        exact_twenty_verified: status.exact_twenty_verified,
        required_cognition_path: status.required_cognition_path,
        missing_cognition_path: status.missing_cognition_path,
        declared_edges: DECLARED_COGNITION_EDGES.to_vec(),
        declared_interfaces: DECLARED_COGNITION_INTERFACES.to_vec(),
        manifest_sha256: declared.manifest_sha256,
        declared_graph_sha256: declared.declared_graph_sha256,
        exact_declared_graph_verified: declared.missing_edges.is_empty()
            && declared.missing_interfaces.is_empty(),
        missing_declared_edges: declared.missing_edges,
        missing_declared_interfaces: declared.missing_interfaces,
        provider_gateway,
        status_sha256: sha256(&output),
        authority: "live_exact_twenty_plus_root_owned_declared_edges_plus_provider_canary:v1",
        residual_limitation: "no_semantic_react_round_trip_was_executed_because_user_prompt_ingress_writes_session_activity_and_reservoir_state",
    })
}

struct CognitionGraphStatus {
    loaded_capsules: Vec<String>,
    loaded_capsule_count: usize,
    exact_twenty_verified: bool,
    required_cognition_path: Vec<&'static str>,
    missing_cognition_path: Vec<&'static str>,
}

#[derive(Debug)]
struct DeclaredGraphVerification {
    manifest_sha256: BTreeMap<String, String>,
    declared_graph_sha256: String,
    missing_edges: Vec<String>,
    missing_interfaces: Vec<String>,
}

const DECLARED_COGNITION_EDGES: &[CognitionDeclaredEdge] = &[
    CognitionDeclaredEdge {
        from_capsule: "astrid-capsule-react",
        publish: "spark.v1.request.build",
        to_capsule: "astrid-capsule-identity",
        subscribe: "spark.v1.request.build",
    },
    CognitionDeclaredEdge {
        from_capsule: "astrid-capsule-identity",
        publish: "spark.v1.response.ready",
        to_capsule: "astrid-capsule-react",
        subscribe: "spark.v1.response.ready",
    },
    CognitionDeclaredEdge {
        from_capsule: "astrid-capsule-react",
        publish: "prompt_builder.v1.assemble",
        to_capsule: "astrid-capsule-prompt-builder",
        subscribe: "prompt_builder.v1.*",
    },
    CognitionDeclaredEdge {
        from_capsule: "astrid-capsule-prompt-builder",
        publish: "session.v1.request.get_messages",
        to_capsule: "astrid-capsule-session",
        subscribe: "session.v1.request.get_messages",
    },
    CognitionDeclaredEdge {
        from_capsule: "astrid-capsule-session",
        publish: "session.v1.response.get_messages.*",
        to_capsule: "astrid-capsule-prompt-builder",
        subscribe: "session.v1.response.get_messages.*",
    },
    CognitionDeclaredEdge {
        from_capsule: "astrid-capsule-prompt-builder",
        publish: "prompt_builder.v1.response.*",
        to_capsule: "astrid-capsule-react",
        subscribe: "prompt_builder.v1.response.assemble",
    },
    CognitionDeclaredEdge {
        from_capsule: "astrid-capsule-react",
        publish: "context_engine.v1.compact",
        to_capsule: "astrid-capsule-context-engine",
        subscribe: "context_engine.v1.compact",
    },
    CognitionDeclaredEdge {
        from_capsule: "astrid-capsule-context-engine",
        publish: "context_engine.v1.response.compact",
        to_capsule: "astrid-capsule-react",
        subscribe: "context_engine.v1.response.compact",
    },
    CognitionDeclaredEdge {
        from_capsule: "astrid-capsule-registry",
        publish: "llm.v1.request.describe",
        to_capsule: "astrid-capsule-openai-compat",
        subscribe: "llm.v1.request.describe",
    },
    CognitionDeclaredEdge {
        from_capsule: "astrid-capsule-openai-compat",
        publish: "llm.v1.response.describe.*",
        to_capsule: "astrid-capsule-registry",
        subscribe: "llm.v1.response.describe",
    },
    CognitionDeclaredEdge {
        from_capsule: "astrid-capsule-react",
        publish: "llm.v1.request.generate.*",
        to_capsule: "astrid-capsule-openai-compat",
        subscribe: "llm.v1.request.generate.openai-compat",
    },
    CognitionDeclaredEdge {
        from_capsule: "astrid-capsule-openai-compat",
        publish: "llm.v1.stream.openai-compat",
        to_capsule: "astrid-capsule-react",
        subscribe: "llm.v1.stream.*",
    },
    CognitionDeclaredEdge {
        from_capsule: "astrid-capsule-react",
        publish: "tool.v1.request.execute",
        to_capsule: "astrid-capsule-router",
        subscribe: "tool.v1.request.execute",
    },
    CognitionDeclaredEdge {
        from_capsule: "astrid-capsule-router",
        publish: "tool.v1.execute.*",
        to_capsule: "astrid-capsule-system",
        subscribe: "tool.v1.execute.system_status",
    },
    CognitionDeclaredEdge {
        from_capsule: "astrid-capsule-system",
        publish: "tool.v1.execute.*.result",
        to_capsule: "astrid-capsule-router",
        subscribe: "tool.v1.execute.*.result",
    },
    CognitionDeclaredEdge {
        from_capsule: "astrid-capsule-router",
        publish: "tool.v1.execute.result",
        to_capsule: "astrid-capsule-react",
        subscribe: "tool.v1.execute.result",
    },
    CognitionDeclaredEdge {
        from_capsule: "astrid-capsule-react",
        publish: "session.v1.request.get_messages",
        to_capsule: "astrid-capsule-session",
        subscribe: "session.v1.request.get_messages",
    },
    CognitionDeclaredEdge {
        from_capsule: "astrid-capsule-session",
        publish: "session.v1.response.get_messages.*",
        to_capsule: "astrid-capsule-react",
        subscribe: "session.v1.response.get_messages.*",
    },
];

const DECLARED_COGNITION_INTERFACES: &[CognitionDeclaredInterface] = &[
    CognitionDeclaredInterface {
        provider_capsule: "astrid-capsule-identity",
        export: "astrid:spark",
        consumer_capsule: "astrid-capsule-react",
        import: "astrid:spark",
    },
    CognitionDeclaredInterface {
        provider_capsule: "astrid-capsule-session",
        export: "astrid:session",
        consumer_capsule: "astrid-capsule-react",
        import: "astrid:session",
    },
    CognitionDeclaredInterface {
        provider_capsule: "astrid-capsule-context-engine",
        export: "astrid:context",
        consumer_capsule: "astrid-capsule-react",
        import: "astrid:context",
    },
    CognitionDeclaredInterface {
        provider_capsule: "astrid-capsule-openai-compat",
        export: "astrid:llm",
        consumer_capsule: "astrid-capsule-react",
        import: "astrid:llm",
    },
];

const REQUIRED_COGNITION_HANDLERS: &[(&str, &str, &str)] = &[
    (
        "astrid-capsule-identity",
        "spark.v1.request.build",
        "handle_build_request",
    ),
    (
        "astrid-capsule-react",
        "spark.v1.response.ready",
        "handle_identity_response",
    ),
    (
        "astrid-capsule-session",
        "session.v1.request.get_messages",
        "handle_get_messages",
    ),
    (
        "astrid-capsule-react",
        "prompt_builder.v1.response.assemble",
        "handle_prompt_response",
    ),
    (
        "astrid-capsule-openai-compat",
        "llm.v1.request.generate.openai-compat",
        "handle_llm_request",
    ),
    (
        "astrid-capsule-react",
        "llm.v1.stream.*",
        "handle_llm_stream",
    ),
    (
        "astrid-capsule-router",
        "tool.v1.request.execute",
        "handle_execute_request",
    ),
    (
        "astrid-capsule-system",
        "tool.v1.execute.system_status",
        "tool_execute_system_status",
    ),
    (
        "astrid-capsule-router",
        "tool.v1.execute.*.result",
        "handle_execute_result",
    ),
    (
        "astrid-capsule-react",
        "tool.v1.execute.result",
        "handle_tool_result",
    ),
];

fn parse_cognition_graph_status(output: &[u8]) -> Result<CognitionGraphStatus> {
    const EXPECTED: &[&str] = crate::invariant::ESSENTIAL_CAPSULES;
    let value: Value = serde_json::from_slice(output)
        .map_err(|_| Error::new("Astrid structured status is malformed"))?;
    let object = value
        .as_object()
        .ok_or_else(|| Error::new("Astrid structured status is not an object"))?;
    if object.len() != 2
        || !object.contains_key("running")
        || !object.contains_key("status")
        || object.get("running").and_then(Value::as_bool) != Some(true)
    {
        return Err(Error::new("Astrid structured status envelope is not exact"));
    }
    let status = object
        .get("status")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("Astrid structured status has no status object"))?;
    let loaded = status
        .get("loaded_capsules")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("Astrid structured status has no capsule graph"))?;
    let mut loaded_capsules = loaded
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .filter(|name| !name.is_empty() && name.len() <= 128)
                .map(str::to_owned)
                .ok_or_else(|| Error::new("Astrid capsule graph contains an invalid identity"))
        })
        .collect::<Result<Vec<_>>>()?;
    let unique = loaded_capsules
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    if unique.len() != loaded_capsules.len() {
        return Err(Error::new(
            "Astrid capsule graph contains duplicate identities",
        ));
    }
    loaded_capsules.sort();
    let expected = EXPECTED
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let observed = loaded_capsules
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let required_cognition_path = vec![
        "astrid-capsule-system",
        "astrid-capsule-session",
        "astrid-capsule-context-engine",
        "astrid-capsule-prompt-builder",
        "astrid-capsule-router",
        "astrid-capsule-react",
        "astrid-capsule-openai-compat",
    ];
    let missing_cognition_path = required_cognition_path
        .iter()
        .copied()
        .filter(|name| !observed.contains(name))
        .collect();
    Ok(CognitionGraphStatus {
        loaded_capsule_count: loaded_capsules.len(),
        exact_twenty_verified: observed == expected && loaded_capsules.len() == 20,
        required_cognition_path,
        missing_cognition_path,
        loaded_capsules,
    })
}

fn verify_declared_cognition_graph(active_generation: &Path) -> Result<DeclaredGraphVerification> {
    let mut required_capsules = BTreeSet::new();
    for edge in DECLARED_COGNITION_EDGES {
        required_capsules.insert(edge.from_capsule);
        required_capsules.insert(edge.to_capsule);
    }
    for interface in DECLARED_COGNITION_INTERFACES {
        required_capsules.insert(interface.provider_capsule);
        required_capsules.insert(interface.consumer_capsule);
    }
    let expected_uid = nix::unistd::geteuid().as_raw();
    let mut manifests = BTreeMap::new();
    let mut manifest_sha256 = BTreeMap::new();
    for capsule in required_capsules {
        let path = active_generation
            .join("installed-capsules")
            .join(capsule)
            .join("Capsule.toml");
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.nlink() != 1
            || metadata.uid() != expected_uid
            || metadata.permissions().mode() & 0o022 != 0
        {
            return Err(Error::new(format!(
                "cognition manifest is not root-controlled: {}",
                path.display()
            )));
        }
        let body = read_regular(&path, 1024 * 1024)?;
        let text = std::str::from_utf8(&body)
            .map_err(|_| Error::new(format!("cognition manifest is not UTF-8: {capsule}")))?;
        let value: toml::Value = toml::from_str(text)
            .map_err(|_| Error::new(format!("cognition manifest is malformed: {capsule}")))?;
        if manifest_string(&value, "package", "name") != Some(capsule) {
            return Err(Error::new(format!(
                "cognition manifest package identity differs: {capsule}"
            )));
        }
        manifest_sha256.insert(capsule.to_owned(), sha256(&body));
        manifests.insert(capsule.to_owned(), value);
    }
    evaluate_declared_cognition_graph(&manifests, manifest_sha256)
}

fn evaluate_declared_cognition_graph(
    manifests: &BTreeMap<String, toml::Value>,
    manifest_sha256: BTreeMap<String, String>,
) -> Result<DeclaredGraphVerification> {
    let mut missing_edges = Vec::new();
    for edge in DECLARED_COGNITION_EDGES {
        let publisher = manifests.get(edge.from_capsule);
        let subscriber = manifests.get(edge.to_capsule);
        if publisher.is_none_or(|value| !manifest_table_contains(value, "publish", edge.publish))
            || subscriber
                .is_none_or(|value| !manifest_table_contains(value, "subscribe", edge.subscribe))
        {
            missing_edges.push(format!(
                "{}:{} -> {}:{}",
                edge.from_capsule, edge.publish, edge.to_capsule, edge.subscribe
            ));
        }
    }
    for (capsule, topic, handler) in REQUIRED_COGNITION_HANDLERS {
        if manifests
            .get(*capsule)
            .and_then(|value| manifest_table_entry_string(value, "subscribe", topic, "handler"))
            != Some(*handler)
        {
            missing_edges.push(format!("{capsule}:{topic} handler={handler}"));
        }
    }
    let mut missing_interfaces = Vec::new();
    for interface in DECLARED_COGNITION_INTERFACES {
        let provider = manifests.get(interface.provider_capsule);
        let consumer = manifests.get(interface.consumer_capsule);
        if provider
            .and_then(|value| manifest_string(value, "exports", interface.export))
            .is_none()
            || consumer
                .and_then(|value| manifest_string(value, "imports", interface.import))
                .is_none()
        {
            missing_interfaces.push(format!(
                "{}:{} -> {}:{}",
                interface.provider_capsule,
                interface.export,
                interface.consumer_capsule,
                interface.import
            ));
        }
    }
    missing_edges.sort();
    missing_edges.dedup();
    missing_interfaces.sort();
    let declared_graph_sha256 = sha256(&canonical_json(&serde_json::json!({
        "schema": "astrid.edge.cognition_declared_graph.v1",
        "manifests": manifest_sha256,
        "edges": DECLARED_COGNITION_EDGES,
        "interfaces": DECLARED_COGNITION_INTERFACES,
        "missing_edges": missing_edges,
        "missing_interfaces": missing_interfaces,
    }))?);
    Ok(DeclaredGraphVerification {
        manifest_sha256,
        declared_graph_sha256,
        missing_edges,
        missing_interfaces,
    })
}

fn manifest_table_contains(value: &toml::Value, table: &str, key: &str) -> bool {
    value
        .get(table)
        .and_then(toml::Value::as_table)
        .is_some_and(|entries| entries.contains_key(key))
}

fn manifest_string<'a>(value: &'a toml::Value, table: &str, key: &str) -> Option<&'a str> {
    value
        .get(table)
        .and_then(toml::Value::as_table)
        .and_then(|entries| entries.get(key))
        .and_then(toml::Value::as_str)
}

fn manifest_table_entry_string<'a>(
    value: &'a toml::Value,
    table: &str,
    entry: &str,
    key: &str,
) -> Option<&'a str> {
    value
        .get(table)
        .and_then(toml::Value::as_table)
        .and_then(|entries| entries.get(entry))
        .and_then(toml::Value::as_table)
        .and_then(|entry| entry.get(key))
        .and_then(toml::Value::as_str)
}

fn verify_provider_gateway_canary(
    config: &Config,
    checked_at: u64,
) -> Result<ProviderGatewayHealth> {
    let path = &config.health.model_warmup_receipt;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != config.health.model_warmup_uid
        || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err(Error::new(
            "provider canary receipt identity or permissions are invalid",
        ));
    }
    let bytes = read_regular(path, 64 * 1024)?;
    let receipt: ProviderCanaryReceiptV3 = serde_json::from_slice(&bytes)?;
    let age_seconds = checked_at.saturating_sub(receipt.completed_at_unix_ms) / 1_000;
    let timestamp_valid = receipt.completed_at_unix_ms > 0
        && receipt.completed_at_unix_ms <= checked_at.saturating_add(FUTURE_SKEW_MS);
    let hashes_valid = [
        &receipt.gateway_wire_sha256,
        &receipt.provider_body_sha256,
        &receipt.model_response_sha256,
    ]
    .iter()
    .all(|digest| valid_lower_hex64(digest));
    let canonical_response_verified = receipt.schema == "astrid_edge_model_warmup_v3"
        && receipt.model == config.model
        && receipt.status == "loaded_and_canary_verified_via_immutable_provider_gateway"
        && matches!(receipt.keep_alive.as_str(), "2h" | "120m")
        && receipt.elapsed_ms <= 660_000
        && (1..=8).contains(&receipt.model_response_bytes)
        && receipt.canonical_response_verified
        && hashes_valid
        && receipt.authority
            == "immutable_non_authored_non_continuity_non_reservoir_provider_canary";
    Ok(ProviderGatewayHealth {
        model: receipt.model,
        completed_at_unix_ms: receipt.completed_at_unix_ms,
        age_seconds,
        elapsed_ms: receipt.elapsed_ms,
        gateway_wire_sha256: receipt.gateway_wire_sha256,
        provider_body_sha256: receipt.provider_body_sha256,
        model_response_sha256: receipt.model_response_sha256,
        model_response_bytes: receipt.model_response_bytes,
        canonical_response_verified,
        fresh: timestamp_valid && age_seconds <= PROVIDER_CANARY_MAXIMUM_AGE_SECONDS,
        receipt_sha256: sha256(&bytes),
        authority: receipt.authority,
    })
}

fn valid_lower_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn independently_verify_hindsight<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    generation_id: &str,
    boot_id: &str,
    checked_at: u64,
) -> Result<HindsightAttestation> {
    let spec = CommandSpec {
        label: "immutable-hindsight-verification",
        executable: config.executables.checkpoint.clone(),
        arguments: vec![
            "verify-health".into(),
            "--workspace".into(),
            config.roots.workspace.display().to_string(),
            "--generation-id".into(),
            generation_id.to_owned(),
            "--maximum-age-seconds".into(),
            config.health.maximum_age_seconds.to_string(),
        ],
        current_dir: config.roots.state_snapshots.clone(),
        environment: BTreeMap::from([
            ("LANG".to_owned(), "C.UTF-8".to_owned()),
            ("TZ".to_owned(), "UTC".to_owned()),
        ]),
        timeout: Duration::from_secs(300),
        run_as_uid: None,
        run_as_gid: None,
    };
    let (receipt, bytes) = runner.run_capture(&spec, 64 * 1024)?;
    require_success(&receipt)?;
    let value: HindsightAttestation = serde_json::from_slice(&bytes)?;
    if value.schema != "astrid.edge_checkpoint.hindsight_attestation.v1"
        || value.generation_id != generation_id
        || value.host_boot_id != boot_id
        || value.checked_at_unix_ms > checked_at.saturating_add(FUTURE_SKEW_MS)
        || checked_at.saturating_sub(value.checked_at_unix_ms) > 300_000
        || value.operator_database_quick_check != "ok"
        || value.ledger_prefixes_verified == 0
        || value.evidence_sha256 != hindsight_digest(&value)?
    {
        return Err(Error::new("immutable hindsight attestation failed"));
    }
    Ok(value)
}

fn property<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    unit: &str,
    property: &str,
) -> Result<String> {
    let spec = CommandSpec {
        label: "systemd-health",
        executable: config.executables.systemctl.clone(),
        arguments: vec![
            "show".into(),
            unit.to_owned(),
            format!("--property={property}"),
            "--value".into(),
        ],
        current_dir: config.roots.state_snapshots.clone(),
        environment: BTreeMap::from([("LANG".to_owned(), "C.UTF-8".to_owned())]),
        timeout: Duration::from_secs(30),
        run_as_uid: None,
        run_as_gid: None,
    };
    let (receipt, output) = runner.run_capture(&spec, 4_096)?;
    require_success(&receipt)?;
    let value = std::str::from_utf8(&output)
        .map_err(|_| Error::new("systemd property is not UTF-8"))?
        .trim();
    if value.is_empty() || value.len() > 128 {
        return Err(Error::new("systemd property is empty or oversized"));
    }
    Ok(value.to_owned())
}

fn verify_live_process_binding(
    main_pid: u64,
    control_group: &str,
    active_link: &Path,
    binary: &str,
) -> Result<(bool, Option<String>, bool)> {
    if main_pid == 0
        || control_group.is_empty()
        || control_group.len() > 256
        || !control_group.starts_with('/')
    {
        return Err(Error::new("live service process identity is absent"));
    }
    let proc_root = Path::new("/proc").join(main_pid.to_string());
    let cgroups = String::from_utf8(read_proc_or_regular(&proc_root.join("cgroup"), 64 * 1024)?)
        .map_err(|_| Error::new("live process cgroup is not UTF-8"))?;
    let proc_cgroup_verified = cgroups.lines().any(|line| {
        line.splitn(3, ':')
            .nth(2)
            .is_some_and(|observed| observed == control_group)
    });
    let observed = std::fs::canonicalize(proc_root.join("exe"))?;
    let expected = std::fs::canonicalize(active_link.join(binary))?;
    let executable_path = observed
        .to_str()
        .filter(|path| path.len() <= 4_096)
        .ok_or_else(|| Error::new("live executable path is not bounded UTF-8"))?
        .to_owned();
    Ok((
        proc_cgroup_verified,
        Some(executable_path),
        observed == expected,
    ))
}

#[allow(clippy::too_many_arguments)]
fn verify_sensor_hash(sensor: &Value, schema: &str) -> Result<bool> {
    if schema == "astrid_edge_spectral_state_v1" {
        return Ok(false);
    }
    let claimed = sensor
        .get("record_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("v2 sensor record hash is absent"))?;
    let mut payload = sensor.clone();
    payload
        .as_object_mut()
        .ok_or_else(|| Error::new("sensor state is not an object"))?
        .remove("record_sha256");
    Ok(claimed.len() == 64 && sha256(&canonical_json(&payload)?) == claimed)
}

fn mem_total_available(path: &Path) -> Result<(u64, u64)> {
    let text = String::from_utf8(read_proc_or_regular(path, 128 * 1024)?)
        .map_err(|_| Error::new("meminfo is not UTF-8"))?;
    let mut total_kib: Option<u64> = None;
    let mut available_kib: Option<u64> = None;
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        match fields.next() {
            Some("MemTotal:") => total_kib = fields.next().and_then(|value| value.parse().ok()),
            Some("MemAvailable:") => {
                available_kib = fields.next().and_then(|value| value.parse().ok());
            },
            _ => {},
        }
    }
    let total = total_kib
        .filter(|value: &u64| *value > 0)
        .ok_or_else(|| Error::new("meminfo lacks valid MemTotal"))?;
    let available = available_kib
        .filter(|value| *value <= total)
        .ok_or_else(|| Error::new("meminfo lacks valid MemAvailable"))?;
    Ok((total.saturating_mul(1_024), available.saturating_mul(1_024)))
}

pub(crate) fn mem_available(path: &Path) -> Result<u64> {
    Ok(mem_total_available(path)?.1)
}

pub(crate) fn swap_used(path: &Path) -> Result<u64> {
    let text = String::from_utf8(read_proc_or_regular(path, 128 * 1024)?)
        .map_err(|_| Error::new("swaps is not UTF-8"))?;
    let mut kib = 0_u64;
    for line in text.lines().skip(1) {
        let used = line
            .split_whitespace()
            .nth(3)
            .ok_or_else(|| Error::new("swap row is malformed"))?
            .parse::<u64>()
            .map_err(|_| Error::new("swap used is malformed"))?;
        kib = kib.saturating_add(used);
    }
    Ok(kib.saturating_mul(1024))
}

fn workspace_storage(path: &Path) -> Result<(u64, u64)> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Error::new(
            "workspace storage target is not a real directory",
        ));
    }
    let canonical = std::fs::canonicalize(path)?;
    if canonical != path {
        return Err(Error::new(
            "workspace storage target is not an exact canonical path",
        ));
    }
    let filesystem = nix::sys::statvfs::statvfs(path)
        .map_err(|error| Error::new(format!("workspace statvfs failed: {error}")))?;
    let blocks = u128::from(filesystem.blocks_available());
    let fragment_size = u128::from(filesystem.fragment_size());
    let available = blocks
        .checked_mul(fragment_size)
        .ok_or_else(|| Error::new("workspace available-byte count overflow"))?;
    let available = u64::try_from(available)
        .map_err(|_| Error::new("workspace available-byte count exceeds u64"))?;
    Ok((metadata.dev(), available))
}

/// Procfs reports regular pseudo-files with a metadata length of zero. Keep
/// the strict stable-file reader everywhere else, but read configured procfs
/// snapshots through a separately bounded path. No appliance-owned mutable
/// file receives this exception.
fn read_proc_or_regular(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    if !path.starts_with("/proc") {
        return read_regular(path, maximum);
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(Error::new(
            "procfs health input is not a regular pseudo-file",
        ));
    }
    let mut bytes = Vec::new();
    File::open(path)?
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(Error::new("procfs health input exceeds bound"));
    }
    Ok(bytes)
}

fn temperature(path: &Path) -> Result<f64> {
    let value = String::from_utf8_lossy(&read_regular(path, 128)?)
        .trim()
        .parse::<f64>()
        .map_err(|_| Error::new("thermal state is malformed"))?;
    let value = if value > 1_000.0 {
        value / 1_000.0
    } else {
        value
    };
    if !value.is_finite() || !(0.0..=150.0).contains(&value) {
        return Err(Error::new("thermal state is non-finite or implausible"));
    }
    Ok(value)
}

fn bounded_string(value: &Value, key: &str) -> Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty() && text.len() <= 256)
        .map(str::to_owned)
        .ok_or_else(|| Error::new(format!("sensor string is absent: {key}")))
}

fn mean(values: impl Iterator<Item = f64>) -> Result<f64> {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        return Ok(f64::NAN);
    }
    let count = u32::try_from(values.len()).map_err(|_| Error::new("mean count overflow"))?;
    Ok(values.iter().sum::<f64>() / f64::from(count))
}

fn ratio(numerator: usize, denominator: usize) -> Result<f64> {
    if denominator == 0 {
        return Ok(0.0);
    }
    let numerator = u32::try_from(numerator).map_err(|_| Error::new("ratio overflow"))?;
    let denominator = u32::try_from(denominator).map_err(|_| Error::new("ratio overflow"))?;
    Ok(f64::from(numerator) / f64::from(denominator))
}

fn current_boot_id() -> Result<String> {
    let value = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
    let value = value.trim().to_ascii_lowercase();
    if value.len() != 36
        || value.bytes().enumerate().any(|(index, byte)| {
            matches!(index, 8 | 13 | 18 | 23) != (byte == b'-')
                || (!matches!(index, 8 | 13 | 18 | 23) && !byte.is_ascii_hexdigit())
        })
    {
        return Err(Error::new("kernel boot identity is malformed"));
    }
    Ok(value)
}

fn report_digest(report: &HealthReport) -> Result<String> {
    let mut value = serde_json::to_value(report)?;
    value
        .as_object_mut()
        .ok_or_else(|| Error::new("health report serialization failed"))?
        .remove("evidence_sha256");
    Ok(sha256(&canonical_json(&value)?))
}

fn hindsight_digest(attestation: &HindsightAttestation) -> Result<String> {
    let mut value = serde_json::to_value(attestation)?;
    value
        .as_object_mut()
        .ok_or_else(|| Error::new("hindsight attestation serialization failed"))?
        .remove("evidence_sha256");
    Ok(sha256(&canonical_json(&value)?))
}

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
        DECLARED_COGNITION_EDGES, DECLARED_COGNITION_INTERFACES, REQUIRED_COGNITION_HANDLERS,
        evaluate_declared_cognition_graph, mem_available, parse_cognition_graph_status, swap_used,
        tail_fills, verify_sensor_hash, workspace_storage,
    };
    use crate::invariant::ESSENTIAL_CAPSULES;
    use crate::probation::RuntimePrefixExpectation;
    use serde_json::json;
    use std::fs;
    use std::io::{Seek, SeekFrom, Write};
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

    fn status(capsules: &[&str]) -> Vec<u8> {
        serde_json::to_vec(&json!({"running": true, "status": {"loaded_capsules": capsules}}))
            .unwrap()
    }

    #[test]
    fn cognition_graph_requires_exact_twenty_functional_capsules() {
        let graph = parse_cognition_graph_status(&status(ESSENTIAL_CAPSULES)).unwrap();
        assert!(graph.exact_twenty_verified);
        assert!(graph.missing_cognition_path.is_empty());
        assert_eq!(graph.required_cognition_path.len(), 7);

        let ten = parse_cognition_graph_status(&status(&ESSENTIAL_CAPSULES[..10])).unwrap();
        assert!(!ten.exact_twenty_verified);
        assert!(
            ten.missing_cognition_path
                .contains(&"astrid-capsule-openai-compat")
        );
        assert!(ten.missing_cognition_path.contains(&"astrid-capsule-react"));

        let mut substituted = ESSENTIAL_CAPSULES.to_vec();
        substituted[10] = "unapproved-provider-lookalike";
        let graph = parse_cognition_graph_status(&status(&substituted)).unwrap();
        assert!(!graph.exact_twenty_verified);
        assert!(
            graph
                .missing_cognition_path
                .contains(&"astrid-capsule-context-engine")
        );
        assert!(
            !graph
                .missing_cognition_path
                .contains(&"astrid-capsule-openai-compat")
        );
        assert!(
            parse_cognition_graph_status(
                br#"{"running":true,"status":{"loaded_capsules":["duplicate","duplicate"]}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn cognition_graph_requires_every_exact_declared_edge_and_interface() {
        fn table_mut<'a>(value: &'a mut toml::Value, name: &str) -> &'a mut toml::Table {
            let root = value.as_table_mut().unwrap();
            root.entry(name.to_owned())
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .unwrap()
        }

        let mut names = std::collections::BTreeSet::new();
        for edge in DECLARED_COGNITION_EDGES {
            names.insert(edge.from_capsule);
            names.insert(edge.to_capsule);
        }
        for interface in DECLARED_COGNITION_INTERFACES {
            names.insert(interface.provider_capsule);
            names.insert(interface.consumer_capsule);
        }
        let mut manifests = names
            .into_iter()
            .map(|name| (name.to_owned(), toml::Value::Table(toml::Table::new())))
            .collect::<std::collections::BTreeMap<_, _>>();
        for edge in DECLARED_COGNITION_EDGES {
            table_mut(manifests.get_mut(edge.from_capsule).unwrap(), "publish").insert(
                edge.publish.to_owned(),
                toml::Value::Table(toml::Table::new()),
            );
            table_mut(manifests.get_mut(edge.to_capsule).unwrap(), "subscribe").insert(
                edge.subscribe.to_owned(),
                toml::Value::Table(toml::Table::new()),
            );
        }
        for (capsule, topic, handler) in REQUIRED_COGNITION_HANDLERS {
            table_mut(manifests.get_mut(*capsule).unwrap(), "subscribe")
                .get_mut(*topic)
                .unwrap()
                .as_table_mut()
                .unwrap()
                .insert(
                    "handler".to_owned(),
                    toml::Value::String((*handler).to_owned()),
                );
        }
        for interface in DECLARED_COGNITION_INTERFACES {
            table_mut(
                manifests.get_mut(interface.provider_capsule).unwrap(),
                "exports",
            )
            .insert(
                interface.export.to_owned(),
                toml::Value::String("1.0.0".to_owned()),
            );
            table_mut(
                manifests.get_mut(interface.consumer_capsule).unwrap(),
                "imports",
            )
            .insert(
                interface.import.to_owned(),
                toml::Value::String("^1.0".to_owned()),
            );
        }
        let hashes = manifests
            .keys()
            .map(|name| (name.clone(), "a".repeat(64)))
            .collect();
        let verified = evaluate_declared_cognition_graph(&manifests, hashes).unwrap();
        assert!(verified.missing_edges.is_empty());
        assert!(verified.missing_interfaces.is_empty());

        table_mut(
            manifests.get_mut("astrid-capsule-openai-compat").unwrap(),
            "subscribe",
        )
        .remove("llm.v1.request.generate.openai-compat");
        let hashes = manifests
            .keys()
            .map(|name| (name.clone(), "b".repeat(64)))
            .collect();
        let rejected = evaluate_declared_cognition_graph(&manifests, hashes).unwrap();
        assert!(
            rejected
                .missing_edges
                .iter()
                .any(|edge| edge.contains("llm.v1.request.generate.openai-compat"))
        );
    }

    #[test]
    fn resource_and_timestamped_fill_fixtures_are_deterministic() {
        let temp = tempfile::tempdir().unwrap();
        let mem = temp.path().join("meminfo");
        fs::write(&mem, "MemTotal: 8000000 kB\nMemAvailable: 3000000 kB\n").unwrap();
        assert_eq!(mem_available(&mem).unwrap(), 3_072_000_000);
        let swaps = temp.path().join("swaps");
        fs::write(
            &swaps,
            "Filename Type Size Used Priority\n/swap file 1000 12 -2\n",
        )
        .unwrap();
        assert_eq!(swap_used(&swaps).unwrap(), 12 * 1024);
        let fill = temp.path().join("fill.jsonl");
        fs::write(
            &fill,
            "{\"recorded_at_unix_ms\":1000,\"fill_ratio\":0.68}\n{\"recorded_at_unix_ms\":6000,\"fill_ratio\":0.69}\n",
        )
        .unwrap();
        fs::set_permissions(&fill, fs::Permissions::from_mode(0o600)).unwrap();
        let (values, snapshot) =
            tail_fills(&fill, 0, 10_000, nix::unistd::geteuid().as_raw(), None).unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[1].recorded_at_unix_ms, 6_000);
        assert_eq!(
            snapshot.continuity_status,
            "migration_baseline_no_prior_continuity_claim"
        );
    }

    #[test]
    fn workspace_storage_is_derived_from_the_exact_backing_filesystem() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().canonicalize().unwrap();
        let (device, available) = workspace_storage(&workspace).unwrap();
        assert_eq!(device, fs::metadata(&workspace).unwrap().dev());
        assert!(available > 0);

        let linked = temp.path().with_extension("linked-workspace");
        symlink(&workspace, &linked).unwrap();
        assert!(workspace_storage(&linked).is_err());
        fs::remove_file(linked).unwrap();
    }

    #[test]
    fn v2_sensor_hash_is_exact_and_v1_is_explicitly_legacy() {
        let mut value = json!({
            "schema":"astrid_edge_spectral_state_v2",
            "recorded_at_unix_ms":1,
            "generation_id":"reservoir-1"
        });
        let digest = crate::fs_guard::sha256(&crate::fs_guard::canonical_json(&value).unwrap());
        value["record_sha256"] = json!(digest);
        assert!(verify_sensor_hash(&value, "astrid_edge_spectral_state_v2").unwrap());
        value["generation_id"] = json!("tampered");
        assert!(!verify_sensor_hash(&value, "astrid_edge_spectral_state_v2").unwrap());
        assert!(!verify_sensor_hash(&value, "astrid_edge_spectral_state_v1").unwrap());
    }

    #[test]
    fn runtime_fill_prefix_accepts_append_and_rejects_mutation() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("fill.jsonl");
        fs::write(
            &path,
            "{\"recorded_at_unix_ms\":1000,\"fill_ratio\":0.68}\n",
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let (_, baseline) =
            tail_fills(&path, 0, 20_000, nix::unistd::geteuid().as_raw(), None).unwrap();
        let expected = RuntimePrefixExpectation {
            device: baseline.device,
            inode: baseline.inode,
            captured_size: baseline.captured_size,
            prefix_sha256: baseline.prefix_sha256,
        };
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"{\"recorded_at_unix_ms\":6000,\"fill_ratio\":0.69}\n")
            .unwrap();
        file.sync_all().unwrap();
        let (values, appended) = tail_fills(
            &path,
            0,
            20_000,
            nix::unistd::geteuid().as_raw(),
            Some(&expected),
        )
        .unwrap();
        assert_eq!(values.len(), 2);
        assert!(appended.prior_prefix_verified);
        assert_eq!(appended.continuity_status, "append_only_prefix_verified");

        let mut file = fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(b"X").unwrap();
        file.sync_all().unwrap();
        assert!(
            tail_fills(
                &path,
                0,
                20_000,
                nix::unistd::geteuid().as_raw(),
                Some(&expected),
            )
            .is_err()
        );
    }

    #[test]
    fn runtime_fill_prefix_rejects_truncation_and_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("fill.jsonl");
        fs::write(
            &path,
            "{\"recorded_at_unix_ms\":1000,\"fill_ratio\":0.68}\n",
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let (_, baseline) =
            tail_fills(&path, 0, 20_000, nix::unistd::geteuid().as_raw(), None).unwrap();
        let expected = RuntimePrefixExpectation {
            device: baseline.device,
            inode: baseline.inode,
            captured_size: baseline.captured_size,
            prefix_sha256: baseline.prefix_sha256,
        };
        fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        assert!(
            tail_fills(
                &path,
                0,
                20_000,
                nix::unistd::geteuid().as_raw(),
                Some(&expected),
            )
            .is_err()
        );

        fs::remove_file(&path).unwrap();
        fs::write(
            &path,
            "{\"recorded_at_unix_ms\":1000,\"fill_ratio\":0.68}\n",
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(
            tail_fills(
                &path,
                0,
                20_000,
                nix::unistd::geteuid().as_raw(),
                Some(&expected),
            )
            .is_err()
        );
    }
}
