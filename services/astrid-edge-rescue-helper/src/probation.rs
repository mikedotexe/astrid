//! Root-owned, hash-chained evidence for a real one-hour A/B probation.

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::config::Config;
use crate::fs_guard::{atomic_write, canonical_json, read_json, read_regular, sha256};
use crate::health::{HealthReport, swap_used};
use crate::ledger_auth::{LedgerKey, seal_record, verify_record};
use crate::{Error, Result};

const STATE_SCHEMA: &str = "astrid.edge_rescue_helper.probation_state.v1";
const RECORD_SCHEMA: &str = "astrid.edge_rescue_helper.probation_record.v2";
const REQUIRED_COVERAGE_MS: u64 = 60 * 60 * 1_000;
const MAX_SAMPLE_GAP_MS: u64 = 10 * 60 * 1_000;
const MINIMUM_SAMPLES: u64 = 7;
const MAXIMUM_CHECKPOINT_BYTES: u64 = 32 * 1024 * 1024;
const MAXIMUM_CHECKPOINT_CHAIN_LINE_BYTES: usize = 512 * 1024;
const MAXIMUM_CHECKPOINT_CHAIN_RECORDS: usize = 100_000;
const HINDSIGHT_BASELINE_SCHEMA: &str = "astrid.edge_rescue_helper.hindsight_baseline.v1";
const HINDSIGHT_HASH_SCOPE: &str = "exact_open_file_prefix_v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct State {
    schema: String,
    #[serde(default)]
    appliance_id: Option<String>,
    status: String,
    generation_id: String,
    previous_generation_id: String,
    host_boot_id: String,
    started_at_unix_ms: u64,
    baseline_swap_bytes: u64,
    last_sample_at_unix_ms: u64,
    sample_count: u64,
    maximum_sample_gap_ms: u64,
    #[serde(default)]
    fill_history_device: Option<u64>,
    #[serde(default)]
    fill_history_inode: Option<u64>,
    #[serde(default)]
    fill_history_size: Option<u64>,
    #[serde(default)]
    fill_history_prefix_sha256: Option<String>,
    #[serde(default)]
    reservoir_challenge_count: u64,
    #[serde(default)]
    last_reservoir_challenge_sha256: Option<String>,
    #[serde(default)]
    hindsight_baseline: Option<HindsightBaseline>,
    #[serde(default)]
    active_profile_sha256: Option<String>,
    #[serde(default)]
    report_projection_sha256: Option<String>,
    ledger_head_sha256: String,
    updated_at_unix_ms: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimePrefixExpectation {
    pub device: u64,
    pub inode: u64,
    pub captured_size: u64,
    pub prefix_sha256: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ActiveLineage {
    pub started_at_unix_ms: u64,
    pub previous_generation_id: String,
    pub hindsight_baseline: HindsightBaseline,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct HindsightLedgerPrefix {
    pub relative_path: String,
    pub device: u64,
    pub inode: u64,
    pub captured_size: u64,
    pub prefix_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct HindsightBaseline {
    pub schema: String,
    pub checkpoint_record_sha256: String,
    pub continuity_epoch: String,
    pub checkpoint_chain_records: usize,
    pub ledger_prefixes: Vec<HindsightLedgerPrefix>,
    pub evidence_sha256: String,
}

#[derive(Debug)]
struct HealthSampleOutcome {
    elapsed_ms: u64,
    swap_growth_bytes: u64,
    coverage_due: bool,
    coverage_complete: bool,
    failed: bool,
    status: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbationEvaluation {
    pub schema: String,
    pub appliance_id: String,
    pub status: String,
    pub generation_id: String,
    pub started_at_unix_ms: u64,
    pub elapsed_seconds: u64,
    pub samples: u64,
    pub maximum_sample_gap_seconds: u64,
    pub baseline_swap_bytes: u64,
    pub current_swap_bytes: u64,
    pub swap_growth_bytes: u64,
    pub coverage_complete: bool,
    pub coverage_due_but_incomplete: bool,
    pub failed: bool,
    pub reservoir_challenge_samples: u64,
    pub last_reservoir_challenge_sha256: String,
    pub ledger_head_sha256: String,
}

pub(crate) fn initialize_inner(
    config: &Config,
    generation_id: &str,
    previous_generation_id: &str,
    hindsight_baseline: &HindsightBaseline,
    require_root: bool,
) -> Result<()> {
    require_probation_root(config, require_root)?;
    if !crate::config::valid_identifier(generation_id)
        || !crate::config::valid_identifier(previous_generation_id)
        || generation_id == previous_generation_id
    {
        return Err(Error::new("probation generation identities are invalid"));
    }
    validate_hindsight_baseline(hindsight_baseline)?;
    if evidence_exists(config) {
        let prior = read_state(config, require_root)?;
        if prior.status == "active" {
            return Err(Error::new("another immutable probation is active"));
        }
    }
    let now = unix_millis();
    let boot = current_boot_id()?;
    let baseline_swap = swap_used(&config.health.swaps)?;
    let projection = crate::profile_projection::verify_active_generation(
        config,
        &config.roots.releases.join(generation_id),
        require_root,
    )?;
    let prior_head = verify_ledger(config, &ledger_path(config), require_root)?;
    let record = append(
        config,
        serde_json::json!({
            "schema": RECORD_SCHEMA,
            "appliance_id": config.appliance_id.clone(),
            "phase": "started",
            "recorded_at_unix_ms": now,
            "generation_id": generation_id,
            "previous_generation_id": previous_generation_id,
            "host_boot_id": boot,
            "baseline_swap_bytes": baseline_swap,
            "active_profile_sha256": projection.active_profile_sha256,
            "report_projection_sha256": projection.report_projection_sha256,
            "hindsight_baseline": hindsight_baseline,
            "authority": "immutable_root_probation_evidence",
        }),
        prior_head.as_deref(),
        require_root,
    )?;
    write_state(
        config,
        &State {
            schema: STATE_SCHEMA.to_owned(),
            appliance_id: Some(config.appliance_id.clone()),
            status: "active".to_owned(),
            generation_id: generation_id.to_owned(),
            previous_generation_id: previous_generation_id.to_owned(),
            host_boot_id: boot,
            started_at_unix_ms: now,
            baseline_swap_bytes: baseline_swap,
            last_sample_at_unix_ms: now,
            sample_count: 0,
            maximum_sample_gap_ms: 0,
            fill_history_device: None,
            fill_history_inode: None,
            fill_history_size: None,
            fill_history_prefix_sha256: None,
            reservoir_challenge_count: 0,
            last_reservoir_challenge_sha256: None,
            hindsight_baseline: Some(hindsight_baseline.clone()),
            active_profile_sha256: Some(projection.active_profile_sha256),
            report_projection_sha256: Some(projection.report_projection_sha256),
            ledger_head_sha256: record,
            updated_at_unix_ms: now,
        },
        require_root,
    )
}

pub fn active_started_at(config: &Config, generation_id: &str) -> Result<Option<u64>> {
    Ok(active_lineage(config, generation_id)?.map(|lineage| lineage.started_at_unix_ms))
}

pub(crate) fn active_lineage(
    config: &Config,
    generation_id: &str,
) -> Result<Option<ActiveLineage>> {
    if !evidence_exists(config) {
        return Ok(None);
    }
    require_root_probation_root(config)?;
    let state = read_state(config, true)?;
    validate_state(&state)?;
    if state.status != "active" {
        return Ok(None);
    }
    if state.generation_id != generation_id || state.host_boot_id != current_boot_id()? {
        return Err(Error::new(
            "active probation does not match generation or boot",
        ));
    }
    let hindsight_baseline = state
        .hindsight_baseline
        .ok_or_else(|| Error::new("active probation lacks pre-activation hindsight baseline"))?;
    Ok(Some(ActiveLineage {
        started_at_unix_ms: state.started_at_unix_ms,
        previous_generation_id: state.previous_generation_id,
        hindsight_baseline,
    }))
}

/// Capture the exact pre-activation hindsight chain head and every present
/// ledger prefix that the immutable checkpoint helper just attested.
///
/// The returned value is subsequently embedded in the root HMAC probation
/// start record. Candidate code therefore cannot replace a ledger between
/// activation and the first periodic health sample and establish a new
/// self-consistent baseline.
#[allow(
    clippy::too_many_lines,
    reason = "the exact checkpoint and every open ledger prefix form one fail-closed capture transaction"
)]
pub(crate) fn capture_hindsight_baseline(
    config: &Config,
    checkpoint_record: &Path,
    expected_generation_id: &str,
    require_root: bool,
) -> Result<HindsightBaseline> {
    let metadata = fs::symlink_metadata(checkpoint_record)?;
    let expected_owner = if require_root {
        0
    } else {
        nix::unistd::geteuid().as_raw()
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != expected_owner
        || metadata.mode() & 0o777 != 0o400
    {
        return Err(Error::new(
            "pre-activation checkpoint record ownership failed",
        ));
    }
    let root: Value = read_json(checkpoint_record, MAXIMUM_CHECKPOINT_BYTES)?;
    let root_object = root
        .as_object()
        .ok_or_else(|| Error::new("pre-activation checkpoint is not an object"))?;
    let expected_fields = ["schema", "reason", "attestation", "authority"]
        .into_iter()
        .collect::<BTreeSet<_>>();
    if root_object
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != expected_fields
        || root_object.get("schema").and_then(Value::as_str)
            != Some("astrid.edge_checkpoint.root_record.v1")
        || root_object.get("reason").and_then(Value::as_str) != Some("self-change-activation")
        || root_object.get("authority").and_then(Value::as_str)
            != Some("immutable_rescue_evidence_not_astrid_authorship_or_mutable_runtime_claim")
    {
        return Err(Error::new(
            "pre-activation checkpoint root record is not exact",
        ));
    }
    let attestation: crate::health::HindsightAttestation = serde_json::from_value(
        root_object
            .get("attestation")
            .cloned()
            .ok_or_else(|| Error::new("pre-activation attestation is absent"))?,
    )?;
    if attestation.generation_id != expected_generation_id
        || !crate::config::valid_hex64(&attestation.checkpoint_record_sha256)
        || attestation.continuity_epoch.is_empty()
        || attestation.continuity_epoch.len() > 128
        || attestation.checkpoint_chain_records == 0
        || attestation.evidence_sha256 != hindsight_attestation_digest(&attestation)?
    {
        return Err(Error::new(
            "pre-activation checkpoint attestation binding failed",
        ));
    }
    let latest: Value = read_json(&config.health.hindsight_state, MAXIMUM_CHECKPOINT_BYTES)?;
    let latest = latest
        .as_object()
        .ok_or_else(|| Error::new("hindsight latest is not an object"))?;
    if latest
        .get("checkpoint_record_sha256")
        .and_then(Value::as_str)
        != Some(attestation.checkpoint_record_sha256.as_str())
        || latest.get("continuity_epoch").and_then(Value::as_str)
            != Some(attestation.continuity_epoch.as_str())
    {
        return Err(Error::new(
            "hindsight latest advanced during pre-activation capture",
        ));
    }
    let ledgers = latest
        .get("ledgers")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("hindsight latest lacks ledger inventory"))?;
    let allowed = allowed_hindsight_ledgers();
    let names = ledgers.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if names.is_empty() || !names.is_subset(&allowed) {
        return Err(Error::new(
            "pre-activation ledger inventory is not immutable-allowlisted",
        ));
    }
    let mut prefixes = Vec::new();
    for relative in names {
        let summary = ledgers
            .get(relative)
            .and_then(Value::as_object)
            .ok_or_else(|| Error::new("pre-activation ledger summary is malformed"))?;
        if summary.get("present").and_then(Value::as_bool) != Some(true) {
            continue;
        }
        if summary.get("hash_scope").and_then(Value::as_str) != Some(HINDSIGHT_HASH_SCOPE) {
            return Err(Error::new(
                "pre-activation ledger hash scope is not exact-prefix",
            ));
        }
        let inode = summary
            .get("inode")
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::new("pre-activation ledger inode is absent"))?;
        let captured_size = summary
            .get("size_bytes")
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::new("pre-activation ledger size is absent"))?;
        let prefix_sha256 = summary
            .get("sha256")
            .and_then(Value::as_str)
            .filter(|digest| crate::config::valid_hex64(digest))
            .ok_or_else(|| Error::new("pre-activation ledger digest is invalid"))?;
        let path = hindsight_ledger_path(config, relative)?;
        let device = verify_exact_prefix(&path, inode, captured_size, prefix_sha256, None)?;
        prefixes.push(HindsightLedgerPrefix {
            relative_path: relative.to_owned(),
            device,
            inode,
            captured_size,
            prefix_sha256: prefix_sha256.to_owned(),
        });
    }
    prefixes.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    if prefixes.is_empty() {
        return Err(Error::new(
            "pre-activation hindsight has no present ledger prefixes",
        ));
    }
    let mut baseline = HindsightBaseline {
        schema: HINDSIGHT_BASELINE_SCHEMA.to_owned(),
        checkpoint_record_sha256: attestation.checkpoint_record_sha256,
        continuity_epoch: attestation.continuity_epoch,
        checkpoint_chain_records: attestation.checkpoint_chain_records,
        ledger_prefixes: prefixes,
        evidence_sha256: String::new(),
    };
    baseline.evidence_sha256 = hindsight_baseline_digest(&baseline)?;
    validate_hindsight_baseline(&baseline)?;
    Ok(baseline)
}

/// Prove that the current immutable checkpoint chain still descends from the
/// exact pre-activation head and that every byte in every captured prefix is
/// unchanged at the same filesystem identity.
pub(crate) fn verify_hindsight_baseline(
    config: &Config,
    baseline: &HindsightBaseline,
    current: &crate::health::HindsightAttestation,
) -> Result<()> {
    validate_hindsight_baseline(baseline)?;
    if current.continuity_epoch != baseline.continuity_epoch
        || current.checkpoint_chain_records < baseline.checkpoint_chain_records
        || current.checkpoint_record_sha256 == baseline.checkpoint_record_sha256
            && current.checkpoint_chain_records != baseline.checkpoint_chain_records
    {
        return Err(Error::new(
            "probation hindsight continuity no longer descends from activation baseline",
        ));
    }
    let latest: Value = read_json(&config.health.hindsight_state, MAXIMUM_CHECKPOINT_BYTES)?;
    let latest = latest
        .as_object()
        .ok_or_else(|| Error::new("current hindsight latest is not an object"))?;
    if latest
        .get("checkpoint_record_sha256")
        .and_then(Value::as_str)
        != Some(current.checkpoint_record_sha256.as_str())
        || latest.get("continuity_epoch").and_then(Value::as_str)
            != Some(baseline.continuity_epoch.as_str())
    {
        return Err(Error::new(
            "current hindsight latest differs from immutable attestation",
        ));
    }
    let chain_path = config
        .health
        .hindsight_state
        .parent()
        .ok_or_else(|| Error::new("hindsight latest has no parent"))?
        .join("checkpoints.jsonl");
    if !checkpoint_chain_contains(
        &chain_path,
        &baseline.checkpoint_record_sha256,
        baseline.checkpoint_chain_records,
    )? {
        return Err(Error::new(
            "current hindsight chain does not contain activation checkpoint",
        ));
    }
    let current_ledgers = latest
        .get("ledgers")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("current hindsight lacks ledger inventory"))?;
    for prefix in &baseline.ledger_prefixes {
        let summary = current_ledgers
            .get(&prefix.relative_path)
            .and_then(Value::as_object)
            .ok_or_else(|| Error::new("captured hindsight ledger disappeared"))?;
        if summary.get("present").and_then(Value::as_bool) != Some(true)
            || summary.get("hash_scope").and_then(Value::as_str) != Some(HINDSIGHT_HASH_SCOPE)
            || summary.get("inode").and_then(Value::as_u64) != Some(prefix.inode)
            || summary
                .get("size_bytes")
                .and_then(Value::as_u64)
                .is_none_or(|size| size < prefix.captured_size)
        {
            return Err(Error::new(
                "current hindsight ledger does not extend activation prefix",
            ));
        }
        let path = hindsight_ledger_path(config, &prefix.relative_path)?;
        verify_exact_prefix(
            &path,
            prefix.inode,
            prefix.captured_size,
            &prefix.prefix_sha256,
            Some(prefix.device),
        )?;
    }
    Ok(())
}

pub(crate) fn verify_post_activation_hindsight(
    config: &Config,
    checkpoint_record: &Path,
    expected_generation_id: &str,
    baseline: &HindsightBaseline,
    require_root: bool,
) -> Result<()> {
    let metadata = fs::symlink_metadata(checkpoint_record)?;
    let expected_owner = if require_root {
        0
    } else {
        nix::unistd::geteuid().as_raw()
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != expected_owner
        || metadata.mode() & 0o777 != 0o400
    {
        return Err(Error::new(
            "post-activation checkpoint record ownership failed",
        ));
    }
    let root: Value = read_json(checkpoint_record, MAXIMUM_CHECKPOINT_BYTES)?;
    let root = root
        .as_object()
        .ok_or_else(|| Error::new("post-activation checkpoint is not an object"))?;
    if root.get("schema").and_then(Value::as_str) != Some("astrid.edge_checkpoint.root_record.v1")
        || root.get("reason").and_then(Value::as_str) != Some("post-activation")
        || root.get("authority").and_then(Value::as_str)
            != Some("immutable_rescue_evidence_not_astrid_authorship_or_mutable_runtime_claim")
    {
        return Err(Error::new(
            "post-activation checkpoint root record is not exact",
        ));
    }
    let attestation: crate::health::HindsightAttestation = serde_json::from_value(
        root.get("attestation")
            .cloned()
            .ok_or_else(|| Error::new("post-activation attestation is absent"))?,
    )?;
    if attestation.generation_id != expected_generation_id
        || attestation.evidence_sha256 != hindsight_attestation_digest(&attestation)?
    {
        return Err(Error::new(
            "post-activation hindsight attestation binding failed",
        ));
    }
    verify_hindsight_baseline(config, baseline, &attestation)
}

pub(crate) fn runtime_prefix_expectation(
    config: &Config,
    generation_id: &str,
) -> Result<Option<RuntimePrefixExpectation>> {
    if !evidence_exists(config) {
        return Ok(None);
    }
    require_root_probation_root(config)?;
    let state = read_state(config, true)?;
    validate_state(&state)?;
    if state.status != "active" {
        return Ok(None);
    }
    if state.generation_id != generation_id || state.host_boot_id != current_boot_id()? {
        return Err(Error::new(
            "runtime prefix expectation does not match generation or boot",
        ));
    }
    match (
        state.fill_history_device,
        state.fill_history_inode,
        state.fill_history_size,
        state.fill_history_prefix_sha256,
    ) {
        (Some(device), Some(inode), Some(captured_size), Some(prefix_sha256)) => {
            Ok(Some(RuntimePrefixExpectation {
                device,
                inode,
                captured_size,
                prefix_sha256,
            }))
        },
        (None, None, None, None) => Ok(None),
        _ => Err(Error::new(
            "runtime prefix expectation is only partially populated",
        )),
    }
}

pub fn record_health(
    config: &Config,
    report: &HealthReport,
) -> Result<Option<ProbationEvaluation>> {
    if !evidence_exists(config) {
        return Ok(None);
    }
    require_root_probation_root(config)?;
    let mut state = read_state(config, true)?;
    validate_state(&state)?;
    if state.status != "active" {
        return Ok(None);
    }
    let outcome = advance_health_sample(config, report, &mut state)?;
    let record = health_sample_record(report, &state, &outcome)?;
    let head = append(config, record, Some(&state.ledger_head_sha256), true)?;
    outcome.status.clone_into(&mut state.status);
    state.ledger_head_sha256.clone_from(&head);
    state.updated_at_unix_ms = report.checked_at_unix_ms;
    write_state(config, &state, true)?;
    Ok(Some(probation_evaluation(&state, report, &outcome, head)))
}

fn advance_health_sample(
    config: &Config,
    report: &HealthReport,
    state: &mut State,
) -> Result<HealthSampleOutcome> {
    if state.appliance_id.as_deref() != Some(report.appliance_id.as_str())
        || state.generation_id != report.active_generation_id
        || state.host_boot_id != report.host_boot_id
        || report.checked_at_unix_ms < state.last_sample_at_unix_ms
    {
        return Err(Error::new("probation sample identity or time is invalid"));
    }
    let projection = crate::profile_projection::verify_active_generation(
        config,
        &config.roots.releases.join(&state.generation_id),
        true,
    )?;
    if state.active_profile_sha256.as_deref() != Some(projection.active_profile_sha256.as_str())
        || state.report_projection_sha256.as_deref()
            != Some(projection.report_projection_sha256.as_str())
    {
        return Err(Error::new(
            "probation active profile projection changed or lacks signed admission binding",
        ));
    }
    if !report.fill_history_snapshot.prior_prefix_verified
        || !crate::config::valid_hex64(&report.fill_history_snapshot.prefix_sha256)
    {
        return Err(Error::new(
            "probation sample lacks immutable runtime-prefix verification",
        ));
    }
    if state.reservoir_challenge_count != state.sample_count {
        return Err(Error::new(
            "active probation predates mandatory reservoir challenges",
        ));
    }
    let baseline = state
        .hindsight_baseline
        .as_ref()
        .ok_or_else(|| Error::new("active probation lacks hindsight baseline"))?;
    if !report.hindsight_activation_baseline_verified
        || report.hindsight_activation_baseline_sha256.as_deref()
            != Some(baseline.evidence_sha256.as_str())
    {
        return Err(Error::new(
            "probation sample does not extend pre-activation hindsight",
        ));
    }
    let challenge = report
        .reservoir_challenge
        .as_ref()
        .filter(|challenge| {
            challenge.challenge_passed
                && state.appliance_id.as_deref() == Some(challenge.appliance_id.as_str())
                && challenge.active_generation_id == state.generation_id
                && challenge.reference_generation_id == state.previous_generation_id
                && crate::config::valid_hex64(&challenge.evidence_sha256)
        })
        .ok_or_else(|| {
            Error::new("probation sample lacks immutable reservoir challenge evidence")
        })?;
    let gap = report
        .checked_at_unix_ms
        .saturating_sub(state.last_sample_at_unix_ms);
    state.maximum_sample_gap_ms = state.maximum_sample_gap_ms.max(gap);
    state.last_sample_at_unix_ms = report.checked_at_unix_ms;
    state.sample_count = state.sample_count.saturating_add(1);
    state.fill_history_device = Some(report.fill_history_snapshot.device);
    state.fill_history_inode = Some(report.fill_history_snapshot.inode);
    state.fill_history_size = Some(report.fill_history_snapshot.captured_size);
    state.fill_history_prefix_sha256 = Some(report.fill_history_snapshot.prefix_sha256.clone());
    state.reservoir_challenge_count = state.reservoir_challenge_count.saturating_add(1);
    state.last_reservoir_challenge_sha256 = Some(challenge.evidence_sha256.clone());
    let elapsed_ms = report
        .checked_at_unix_ms
        .saturating_sub(state.started_at_unix_ms);
    let swap_growth_bytes = report.swap_bytes.saturating_sub(state.baseline_swap_bytes);
    let coverage_due = elapsed_ms >= REQUIRED_COVERAGE_MS;
    let coverage_complete = coverage_due
        && state.sample_count >= MINIMUM_SAMPLES
        && state.reservoir_challenge_count == state.sample_count
        && state.maximum_sample_gap_ms <= MAX_SAMPLE_GAP_MS
        && report.probation_fill_coverage_complete;
    let failed = !report.healthy || swap_growth_bytes > config.health.maximum_swap_bytes;
    let status = if failed {
        "failed"
    } else if coverage_complete {
        "complete"
    } else {
        "active"
    };
    Ok(HealthSampleOutcome {
        elapsed_ms,
        swap_growth_bytes,
        coverage_due,
        coverage_complete,
        failed,
        status,
    })
}

fn health_sample_record(
    report: &HealthReport,
    state: &State,
    outcome: &HealthSampleOutcome,
) -> Result<Value> {
    Ok(serde_json::json!({
        "schema": RECORD_SCHEMA,
        "phase": "health_sample",
        "recorded_at_unix_ms": report.checked_at_unix_ms,
        "generation_id": report.active_generation_id,
        "host_boot_id": report.host_boot_id,
        "health_evidence_sha256": health_payload_digest(report)?,
        "active_profile_sha256": state.active_profile_sha256,
        "report_projection_sha256": state.report_projection_sha256,
        "healthy": report.healthy,
        "sample_count": state.sample_count,
        "elapsed_ms": outcome.elapsed_ms,
        "maximum_sample_gap_ms": state.maximum_sample_gap_ms,
        "baseline_swap_bytes": state.baseline_swap_bytes,
        "current_swap_bytes": report.swap_bytes,
        "swap_growth_bytes": outcome.swap_growth_bytes,
        "workspace_filesystem_device": report.workspace_filesystem_device,
        "workspace_filesystem_available_bytes": report.workspace_filesystem_available_bytes,
        "workspace_filesystem_minimum_bytes": report.workspace_filesystem_minimum_bytes,
        "workspace_storage_healthy": report.workspace_storage_healthy,
        "hindsight_activation_baseline_sha256": report.hindsight_activation_baseline_sha256,
        "hindsight_activation_baseline_verified": report.hindsight_activation_baseline_verified,
        "fill_samples": report.fill_samples,
        "fill_coverage_seconds": report.fill_coverage_seconds,
        "fill_mean": report.fill_mean,
        "fill_occupancy_65_735": report.fill_occupancy_65_735,
        "fill_history_device": report.fill_history_snapshot.device,
        "fill_history_inode": report.fill_history_snapshot.inode,
        "fill_history_size": report.fill_history_snapshot.captured_size,
        "fill_history_prefix_sha256": report.fill_history_snapshot.prefix_sha256,
        "fill_history_prior_prefix_verified": report.fill_history_snapshot.prior_prefix_verified,
        "reservoir_challenge_evidence_sha256": report.reservoir_challenge.as_ref().map(|challenge| challenge.evidence_sha256.clone()),
        "reservoir_challenge_count": state.reservoir_challenge_count,
        "coverage_complete": outcome.coverage_complete,
        "terminal_status": outcome.status,
        "authority": "immutable_root_probation_evidence",
    }))
}

fn probation_evaluation(
    state: &State,
    report: &HealthReport,
    outcome: &HealthSampleOutcome,
    head: String,
) -> ProbationEvaluation {
    ProbationEvaluation {
        schema: "astrid.edge_rescue_helper.probation_evaluation.v1".to_owned(),
        appliance_id: state.appliance_id.clone().unwrap_or_default(),
        status: outcome.status.to_owned(),
        generation_id: state.generation_id.clone(),
        started_at_unix_ms: state.started_at_unix_ms,
        elapsed_seconds: outcome.elapsed_ms / 1_000,
        samples: state.sample_count,
        maximum_sample_gap_seconds: state.maximum_sample_gap_ms / 1_000,
        baseline_swap_bytes: state.baseline_swap_bytes,
        current_swap_bytes: report.swap_bytes,
        swap_growth_bytes: outcome.swap_growth_bytes,
        coverage_complete: outcome.coverage_complete,
        coverage_due_but_incomplete: outcome.coverage_due && !outcome.coverage_complete,
        failed: outcome.failed,
        reservoir_challenge_samples: state.reservoir_challenge_count,
        last_reservoir_challenge_sha256: state
            .last_reservoir_challenge_sha256
            .clone()
            .unwrap_or_default(),
        ledger_head_sha256: head,
    }
}

pub fn close_for_rollback(config: &Config, generation_id: &str, reason: &str) -> Result<()> {
    close_for_rollback_inner(config, generation_id, reason, true)
}

pub(crate) fn close_for_rollback_inner(
    config: &Config,
    generation_id: &str,
    reason: &str,
    require_root: bool,
) -> Result<()> {
    if !evidence_exists(config) {
        return Ok(());
    }
    require_probation_root(config, require_root)?;
    let mut state = read_state(config, require_root)?;
    validate_state(&state)?;
    if state.status != "active" && state.status != "failed" {
        return Ok(());
    }
    if state.generation_id != generation_id || reason.is_empty() || reason.len() > 128 {
        return Err(Error::new("rollback probation closure is not exact"));
    }
    let now = unix_millis();
    state.ledger_head_sha256 = append(
        config,
        serde_json::json!({
            "schema": RECORD_SCHEMA,
            "phase": "rolled_back",
            "recorded_at_unix_ms": now,
            "generation_id": generation_id,
            "reason": reason,
            "authority": "immutable_root_probation_evidence",
        }),
        Some(&state.ledger_head_sha256),
        require_root,
    )?;
    "rolled_back".clone_into(&mut state.status);
    state.updated_at_unix_ms = now;
    write_state(config, &state, require_root)
}

fn append(
    config: &Config,
    mut value: Value,
    expected: Option<&str>,
    require_root: bool,
) -> Result<String> {
    let path = ledger_path(config);
    let key = LedgerKey::load(&config.source.ledger_attestation_key, require_root)?;
    let actual = replay_ledger_with_key(
        &path,
        require_root,
        &key,
        Some(config.appliance_id.as_str()),
    )?
    .0;
    if actual.as_deref() != expected {
        return Err(Error::new("probation ledger head differs from root state"));
    }
    if value
        .get("appliance_id")
        .is_some_and(|candidate| candidate.as_str() != Some(config.appliance_id.as_str()))
    {
        return Err(Error::new(
            "probation record attempted to change appliance identity",
        ));
    }
    value["appliance_id"] = Value::String(config.appliance_id.clone());
    value["previous_record_sha256"] = actual.map_or(Value::Null, Value::String);
    let digest = seal_record(&mut value, &key, "probation")?;
    let mut options = OpenOptions::new();
    options
        .write(true)
        .append(true)
        .create(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC);
    let mut file = options.open(&path)?;
    let metadata = file.metadata()?;
    let expected_uid = if require_root {
        0
    } else {
        nix::unistd::geteuid().as_raw()
    };
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != expected_uid
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new("probation ledger ownership or mode failed"));
    }
    file.write_all(&canonical_json(&value)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    std::fs::File::open(&config.roots.state_snapshots)?.sync_all()?;
    Ok(digest)
}

fn verify_ledger(
    config: &Config,
    path: &std::path::Path,
    require_root: bool,
) -> Result<Option<String>> {
    Ok(replay_ledger(config, path, require_root)?.0)
}

fn replay_ledger(
    config: &Config,
    path: &std::path::Path,
    require_root: bool,
) -> Result<(Option<String>, Option<State>)> {
    let key = LedgerKey::load(&config.source.ledger_attestation_key, require_root)?;
    replay_ledger_with_key(path, require_root, &key, Some(config.appliance_id.as_str()))
}

fn replay_ledger_with_key(
    path: &std::path::Path,
    require_root: bool,
    key: &LedgerKey,
    expected_appliance_id: Option<&str>,
) -> Result<(Option<String>, Option<State>)> {
    if !path.exists() {
        return Ok((None, None));
    }
    let metadata = fs::symlink_metadata(path)?;
    let expected_uid = if require_root {
        0
    } else {
        nix::unistd::geteuid().as_raw()
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != expected_uid
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new("probation ledger ownership or mode failed"));
    }
    let bytes = read_regular(path, 32 * 1024 * 1024)?;
    let mut previous = None;
    let mut state: Option<State> = None;
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let value: Value = serde_json::from_slice(line)?;
        let object = value
            .as_object()
            .ok_or_else(|| Error::new("probation ledger record is not an object"))?;
        let claimed = verify_record(&value, key, "probation")?;
        if object.get("schema").and_then(Value::as_str) != Some(RECORD_SCHEMA)
            || object.get("authority").and_then(Value::as_str)
                != Some("immutable_root_probation_evidence")
            || !crate::config::valid_hex64(&claimed)
            || object.get("previous_record_sha256")
                != Some(&previous.clone().map_or(Value::Null, Value::String))
        {
            return Err(Error::new("probation ledger hash chain failed"));
        }
        let record_appliance_id = object
            .get("appliance_id")
            .map(|value| {
                value
                    .as_str()
                    .filter(|appliance_id| crate::config::valid_identifier(appliance_id))
                    .map(str::to_owned)
                    .ok_or_else(|| Error::new("probation ledger appliance identity is invalid"))
            })
            .transpose()?;
        if expected_appliance_id.is_some_and(|expected| {
            record_appliance_id
                .as_deref()
                .is_some_and(|actual| actual != expected)
        }) {
            return Err(Error::new("probation ledger belongs to another appliance"));
        }
        apply_ledger_record(object, &claimed, record_appliance_id, &mut state)?;
        previous = Some(claimed);
    }
    if let (Some(expected), Some(reconstructed)) = (expected_appliance_id, state.as_ref())
        && reconstructed.appliance_id.as_deref() != Some(expected)
        && !matches!(reconstructed.status.as_str(), "complete" | "rolled_back")
    {
        return Err(Error::new(
            "active probation ledger is not bound to this appliance",
        ));
    }
    Ok((previous, state))
}

#[allow(clippy::too_many_lines)] // Signed replay keeps all projection and probation continuity checks together.
fn apply_ledger_record(
    object: &serde_json::Map<String, Value>,
    claimed: &str,
    record_appliance_id: Option<String>,
    state: &mut Option<State>,
) -> Result<()> {
    let phase = record_string(object, "phase")?;
    let recorded = record_u64(object, "recorded_at_unix_ms")?;
    match phase.as_str() {
        "started" => {
            if state.as_ref().is_some_and(|prior| prior.status == "active") {
                return Err(Error::new(
                    "probation ledger starts over an active generation",
                ));
            }
            *state = Some(State {
                schema: STATE_SCHEMA.to_owned(),
                appliance_id: record_appliance_id,
                status: "active".to_owned(),
                generation_id: record_string(object, "generation_id")?,
                previous_generation_id: record_string(object, "previous_generation_id")?,
                host_boot_id: record_string(object, "host_boot_id")?,
                started_at_unix_ms: recorded,
                baseline_swap_bytes: record_u64(object, "baseline_swap_bytes")?,
                last_sample_at_unix_ms: recorded,
                sample_count: 0,
                maximum_sample_gap_ms: 0,
                fill_history_device: None,
                fill_history_inode: None,
                fill_history_size: None,
                fill_history_prefix_sha256: None,
                reservoir_challenge_count: 0,
                last_reservoir_challenge_sha256: None,
                hindsight_baseline: object
                    .get("hindsight_baseline")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()?,
                active_profile_sha256: object
                    .get("active_profile_sha256")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                report_projection_sha256: object
                    .get("report_projection_sha256")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                ledger_head_sha256: claimed.to_owned(),
                updated_at_unix_ms: recorded,
            });
        },
        "health_sample" => {
            let current = state
                .as_mut()
                .ok_or_else(|| Error::new("probation health sample has no start"))?;
            if current.status != "active"
                || record_appliance_id != current.appliance_id
                || record_string(object, "generation_id")? != current.generation_id
                || record_string(object, "host_boot_id")? != current.host_boot_id
                || recorded < current.last_sample_at_unix_ms
            {
                return Err(Error::new("probation health sample lineage failed"));
            }
            if current
                .active_profile_sha256
                .as_deref()
                .is_none_or(|digest| {
                    object.get("active_profile_sha256").and_then(Value::as_str) != Some(digest)
                })
            {
                return Err(Error::new(
                    "probation health sample changed the active profile binding",
                ));
            }
            if current
                .report_projection_sha256
                .as_deref()
                .is_none_or(|digest| {
                    object
                        .get("report_projection_sha256")
                        .and_then(Value::as_str)
                        != Some(digest)
                })
            {
                return Err(Error::new(
                    "probation health sample changed the report projection binding",
                ));
            }
            let status = record_string(object, "terminal_status")?;
            if !matches!(status.as_str(), "active" | "failed" | "complete") {
                return Err(Error::new("probation health sample status is invalid"));
            }
            current.status = status;
            current.last_sample_at_unix_ms = recorded;
            current.sample_count = record_u64(object, "sample_count")?;
            current.maximum_sample_gap_ms = record_u64(object, "maximum_sample_gap_ms")?;
            apply_runtime_prefix_record(object, current)?;
            apply_reservoir_challenge_record(object, current)?;
            claimed.clone_into(&mut current.ledger_head_sha256);
            current.updated_at_unix_ms = recorded;
        },
        "rolled_back" => {
            let current = state
                .as_mut()
                .ok_or_else(|| Error::new("probation rollback has no start"))?;
            if !matches!(current.status.as_str(), "active" | "failed")
                || record_appliance_id != current.appliance_id
                || record_string(object, "generation_id")? != current.generation_id
                || recorded < current.updated_at_unix_ms
            {
                return Err(Error::new("probation rollback lineage failed"));
            }
            "rolled_back".clone_into(&mut current.status);
            claimed.clone_into(&mut current.ledger_head_sha256);
            current.updated_at_unix_ms = recorded;
        },
        _ => return Err(Error::new("probation ledger phase is unsupported")),
    }
    Ok(())
}

fn apply_runtime_prefix_record(
    object: &serde_json::Map<String, Value>,
    state: &mut State,
) -> Result<()> {
    const FIELDS: [&str; 5] = [
        "fill_history_device",
        "fill_history_inode",
        "fill_history_size",
        "fill_history_prefix_sha256",
        "fill_history_prior_prefix_verified",
    ];
    let present = FIELDS
        .iter()
        .filter(|field| object.contains_key(**field))
        .count();
    if present == 0 {
        if state.fill_history_size.is_some() {
            return Err(Error::new(
                "probation runtime-prefix evidence disappeared after admission",
            ));
        }
        return Ok(());
    }
    if present != FIELDS.len()
        || object
            .get("fill_history_prior_prefix_verified")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err(Error::new(
            "probation runtime-prefix evidence is incomplete",
        ));
    }
    let device = record_u64(object, "fill_history_device")?;
    let inode = record_u64(object, "fill_history_inode")?;
    let captured_size = record_u64(object, "fill_history_size")?;
    let prefix_sha256 = record_string(object, "fill_history_prefix_sha256")?;
    if !crate::config::valid_hex64(&prefix_sha256) {
        return Err(Error::new("probation runtime-prefix digest is malformed"));
    }
    if let (Some(prior_device), Some(prior_inode), Some(prior_size)) = (
        state.fill_history_device,
        state.fill_history_inode,
        state.fill_history_size,
    ) && (device != prior_device || inode != prior_inode || captured_size < prior_size)
    {
        return Err(Error::new(
            "probation runtime-prefix identity or size regressed",
        ));
    }
    state.fill_history_device = Some(device);
    state.fill_history_inode = Some(inode);
    state.fill_history_size = Some(captured_size);
    state.fill_history_prefix_sha256 = Some(prefix_sha256);
    Ok(())
}

fn apply_reservoir_challenge_record(
    object: &serde_json::Map<String, Value>,
    state: &mut State,
) -> Result<()> {
    let digest_present = object.contains_key("reservoir_challenge_evidence_sha256");
    let count_present = object.contains_key("reservoir_challenge_count");
    if !digest_present && !count_present {
        // Legacy records remain replayable as historical evidence. An active
        // legacy probation fails closed in `advance_health_sample` rather than
        // being silently credited with a challenge it never ran.
        return Ok(());
    }
    if digest_present != count_present {
        return Err(Error::new(
            "probation reservoir challenge ledger evidence is partial",
        ));
    }
    let digest = record_string(object, "reservoir_challenge_evidence_sha256")?;
    let count = record_u64(object, "reservoir_challenge_count")?;
    if !crate::config::valid_hex64(&digest)
        || count != state.reservoir_challenge_count.saturating_add(1)
    {
        return Err(Error::new(
            "probation reservoir challenge ledger lineage failed",
        ));
    }
    state.reservoir_challenge_count = count;
    state.last_reservoir_challenge_sha256 = Some(digest);
    Ok(())
}

fn record_string(object: &serde_json::Map<String, Value>, key: &str) -> Result<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 256)
        .map(str::to_owned)
        .ok_or_else(|| Error::new(format!("probation ledger field is invalid: {key}")))
}

fn record_u64(object: &serde_json::Map<String, Value>, key: &str) -> Result<u64> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new(format!("probation ledger field is invalid: {key}")))
}

fn validate_state(state: &State) -> Result<()> {
    if state.schema != STATE_SCHEMA
        || state
            .appliance_id
            .as_deref()
            .is_some_and(|appliance_id| !crate::config::valid_identifier(appliance_id))
        || !matches!(
            state.status.as_str(),
            "active" | "failed" | "complete" | "rolled_back"
        )
        || !crate::config::valid_identifier(&state.generation_id)
        || !crate::config::valid_identifier(&state.previous_generation_id)
        || state.host_boot_id.len() != 36
        || state.started_at_unix_ms == 0
        || state.last_sample_at_unix_ms < state.started_at_unix_ms
        || state.updated_at_unix_ms < state.started_at_unix_ms
        || !crate::config::valid_hex64(&state.ledger_head_sha256)
        || !runtime_prefix_state_valid(state)
        || state.reservoir_challenge_count > state.sample_count
        || state
            .last_reservoir_challenge_sha256
            .as_deref()
            .is_some_and(|digest| !crate::config::valid_hex64(digest))
        || (state.reservoir_challenge_count == 0) != state.last_reservoir_challenge_sha256.is_none()
        || state
            .hindsight_baseline
            .as_ref()
            .is_some_and(|baseline| validate_hindsight_baseline(baseline).is_err())
        || state
            .active_profile_sha256
            .as_deref()
            .is_some_and(|digest| !crate::config::valid_hex64(digest))
        || state
            .report_projection_sha256
            .as_deref()
            .is_some_and(|digest| !crate::config::valid_hex64(digest))
    {
        return Err(Error::new("probation state is malformed"));
    }
    Ok(())
}

fn runtime_prefix_state_valid(state: &State) -> bool {
    match (
        state.fill_history_device,
        state.fill_history_inode,
        state.fill_history_size,
        state.fill_history_prefix_sha256.as_deref(),
    ) {
        (None, None, None, None) => true,
        (Some(_), Some(_), Some(_), Some(digest)) => crate::config::valid_hex64(digest),
        _ => false,
    }
}

fn validate_hindsight_baseline(baseline: &HindsightBaseline) -> Result<()> {
    let mut prior = None;
    for prefix in &baseline.ledger_prefixes {
        if !allowed_hindsight_ledgers().contains(prefix.relative_path.as_str())
            || prior
                .as_ref()
                .is_some_and(|value: &&str| *value >= prefix.relative_path.as_str())
            || prefix.inode == 0
            || !crate::config::valid_hex64(&prefix.prefix_sha256)
        {
            return Err(Error::new(
                "pre-activation hindsight ledger binding is malformed",
            ));
        }
        prior = Some(prefix.relative_path.as_str());
    }
    if baseline.schema != HINDSIGHT_BASELINE_SCHEMA
        || !crate::config::valid_hex64(&baseline.checkpoint_record_sha256)
        || baseline.continuity_epoch.is_empty()
        || baseline.continuity_epoch.len() > 128
        || baseline.checkpoint_chain_records == 0
        || baseline.ledger_prefixes.is_empty()
        || baseline.evidence_sha256 != hindsight_baseline_digest(baseline)?
    {
        return Err(Error::new("pre-activation hindsight baseline is malformed"));
    }
    Ok(())
}

fn hindsight_baseline_digest(baseline: &HindsightBaseline) -> Result<String> {
    let mut value = serde_json::to_value(baseline)?;
    value
        .as_object_mut()
        .ok_or_else(|| Error::new("hindsight baseline is not an object"))?
        .remove("evidence_sha256");
    Ok(sha256(&canonical_json(&value)?))
}

fn hindsight_attestation_digest(
    attestation: &crate::health::HindsightAttestation,
) -> Result<String> {
    let mut value = serde_json::to_value(attestation)?;
    value
        .as_object_mut()
        .ok_or_else(|| Error::new("hindsight attestation is not an object"))?
        .remove("evidence_sha256");
    Ok(sha256(&canonical_json(&value)?))
}

fn allowed_hindsight_ledgers() -> BTreeSet<&'static str> {
    [
        "actions/dispatches.jsonl",
        "actions/receipts.jsonl",
        "actions/interrupted_corrections.jsonl",
        "autonomous/runs.jsonl",
        "autonomous/chains.jsonl",
        "autonomous/recoveries.jsonl",
        "autonomous/authorship_corrections.jsonl",
        "autonomous/thread_state.jsonl",
        "web/receipts.jsonl",
        "introspection/receipts.jsonl",
        "introspections/scheduled/receipts.jsonl",
        "introspection/scheduled/receipts.jsonl",
        "perception/observations.jsonl",
        "studies/receipts.jsonl",
        "spectral/rollups.jsonl",
        "spectral/receipts.jsonl",
        "tuning/receipts.jsonl",
        "research/duplication_notices.jsonl",
        "peer/receipts.jsonl",
        "runtime/fill_history.jsonl",
        "self-change/ledgers/candidate.jsonl",
        "self-change/ledgers/build.jsonl",
        "self-change/ledgers/activation.jsonl",
        "self-change/ledgers/operator.jsonl",
    ]
    .into_iter()
    .collect()
}

fn hindsight_ledger_path(config: &Config, relative: &str) -> Result<std::path::PathBuf> {
    if !allowed_hindsight_ledgers().contains(relative) {
        return Err(Error::new("hindsight ledger path is not allowlisted"));
    }
    let relative_path = Path::new(relative);
    if relative_path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(Error::new(
            "hindsight ledger path is not a safe relative path",
        ));
    }
    if let Some(basename) = relative.strip_prefix("self-change/ledgers/") {
        if basename.contains('/') || basename.is_empty() {
            return Err(Error::new("self-change hindsight ledger name is invalid"));
        }
        let state_root = config
            .roots
            .workspace
            .ancestors()
            .nth(3)
            .filter(|_| config.roots.workspace.ends_with("home/default/edge"))
            .ok_or_else(|| Error::new("hindsight workspace layout is invalid"))?;
        return Ok(state_root.join(relative));
    }
    Ok(config.roots.workspace.join(relative_path))
}

fn verify_exact_prefix(
    path: &Path,
    expected_inode: u64,
    size: u64,
    expected_sha256: &str,
    expected_device: Option<u64>,
) -> Result<u64> {
    let before = fs::symlink_metadata(path)?;
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.nlink() != 1
        || before.ino() != expected_inode
        || before.len() < size
        || expected_device.is_some_and(|device| device != before.dev())
        || !crate::config::valid_hex64(expected_sha256)
    {
        return Err(Error::new(
            "hindsight activation prefix file identity failed",
        ));
    }
    let file = File::open(path)?;
    let opened = file.metadata()?;
    if opened.dev() != before.dev() || opened.ino() != before.ino() || opened.len() < size {
        return Err(Error::new(
            "hindsight activation prefix changed while opened",
        ));
    }
    let mut reader = file.take(size);
    let mut hasher = Sha256::new();
    let copied = std::io::copy(&mut reader, &mut hasher)?;
    if copied != size || format!("{:x}", hasher.finalize()) != expected_sha256 {
        return Err(Error::new("hindsight activation prefix bytes changed"));
    }
    let after = fs::symlink_metadata(path)?;
    if after.dev() != before.dev() || after.ino() != before.ino() || after.len() < size {
        return Err(Error::new(
            "hindsight activation prefix changed during verification",
        ));
    }
    Ok(before.dev())
}

fn checkpoint_chain_contains(
    path: &Path,
    expected_head: &str,
    minimum_records: usize,
) -> Result<bool> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.nlink() != 1 {
        return Err(Error::new("hindsight checkpoint chain identity failed"));
    }
    let file = File::open(path)?;
    let opened = file.metadata()?;
    let mut reader = BufReader::new(file);
    let mut found_at = None;
    let mut records = 0_usize;
    loop {
        let mut line = Vec::new();
        let read = reader
            .by_ref()
            .take(
                u64::try_from(MAXIMUM_CHECKPOINT_CHAIN_LINE_BYTES)
                    .unwrap_or(u64::MAX)
                    .saturating_add(1),
            )
            .read_until(b'\n', &mut line)?;
        if read == 0 {
            break;
        }
        if line.len() > MAXIMUM_CHECKPOINT_CHAIN_LINE_BYTES || line.last() != Some(&b'\n') {
            return Err(Error::new(
                "hindsight checkpoint chain has oversized or partial record",
            ));
        }
        line.pop();
        let value: Value = serde_json::from_slice(&line)?;
        if value.get("record_sha256").and_then(Value::as_str) == Some(expected_head) {
            found_at = Some(records.saturating_add(1));
        }
        records = records.saturating_add(1);
        if records > MAXIMUM_CHECKPOINT_CHAIN_RECORDS {
            return Err(Error::new(
                "hindsight checkpoint chain exceeds immutable record bound",
            ));
        }
    }
    let after = reader.into_inner().metadata()?;
    if before_identity(&metadata) != before_identity(&opened)
        || before_identity(&opened) != before_identity(&after)
    {
        return Err(Error::new(
            "hindsight checkpoint chain changed while scanned",
        ));
    }
    Ok(records >= minimum_records && found_at == Some(minimum_records))
}

fn before_identity(metadata: &fs::Metadata) -> (u64, u64, u64) {
    (metadata.dev(), metadata.ino(), metadata.len())
}

fn write_state(config: &Config, state: &State, require_root: bool) -> Result<()> {
    let path = state_path(config);
    atomic_write(&path, &canonical_json(state)?, 0o400, true)?;
    let _ = read_state(config, require_root)?;
    Ok(())
}

fn read_state(config: &Config, require_root: bool) -> Result<State> {
    let path = state_path(config);
    if path.exists() {
        let metadata = fs::symlink_metadata(&path)?;
        let expected_uid = if require_root {
            0
        } else {
            nix::unistd::geteuid().as_raw()
        };
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.nlink() != 1
            || metadata.uid() != expected_uid
            || metadata.mode() & 0o777 != 0o400
        {
            return Err(Error::new("probation state ownership or mode failed"));
        }
        let cached: State = read_json(&path, 32 * 1024)?;
        validate_state(&cached)?;
        validate_state_appliance(&cached, &config.appliance_id)?;
    }
    let (_, reconstructed) = replay_ledger(config, &ledger_path(config), require_root)?;
    let reconstructed = reconstructed
        .ok_or_else(|| Error::new("probation state has no immutable ledger history"))?;
    validate_state(&reconstructed)?;
    validate_state_appliance(&reconstructed, &config.appliance_id)?;
    // The append-only root ledger is authoritative at a power-loss boundary.
    // A stale atomic cache is tolerated and repaired by the next state write;
    // a cache ahead of the ledger cannot manufacture probation evidence.
    Ok(reconstructed)
}

fn validate_state_appliance(state: &State, expected_appliance_id: &str) -> Result<()> {
    match state.appliance_id.as_deref() {
        Some(actual) if actual == expected_appliance_id => Ok(()),
        None if matches!(state.status.as_str(), "complete" | "rolled_back") => Ok(()),
        Some(_) => Err(Error::new("probation state belongs to another appliance")),
        None => Err(Error::new(
            "active probation state predates appliance-bound lineage",
        )),
    }
}

fn evidence_exists(config: &Config) -> bool {
    state_path(config).exists() || ledger_path(config).exists()
}

fn require_root_probation_root(config: &Config) -> Result<()> {
    require_probation_root(config, true)
}

fn require_probation_root(config: &Config, require_root: bool) -> Result<()> {
    let metadata = fs::symlink_metadata(&config.roots.state_snapshots)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || (require_root && metadata.uid() != 0)
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new(
            "probation evidence root is not private root state",
        ));
    }
    Ok(())
}

fn state_path(config: &Config) -> std::path::PathBuf {
    config.roots.state_snapshots.join("probation-state.json")
}

fn ledger_path(config: &Config) -> std::path::PathBuf {
    config.roots.state_snapshots.join("probation.jsonl")
}

fn current_boot_id() -> Result<String> {
    let value = fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
    let value = value.trim().to_ascii_lowercase();
    if value.len() != 36 {
        return Err(Error::new("kernel boot identity is malformed"));
    }
    Ok(value)
}

fn health_payload_digest(report: &HealthReport) -> Result<String> {
    let mut value = serde_json::to_value(report)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| Error::new("health payload serialization failed"))?;
    object.remove("evidence_sha256");
    object.remove("probation");
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
        HINDSIGHT_BASELINE_SCHEMA, HindsightBaseline, HindsightLedgerPrefix, RECORD_SCHEMA, State,
        checkpoint_chain_contains, hindsight_baseline_digest, replay_ledger_with_key,
        validate_hindsight_baseline, validate_state, verify_exact_prefix,
    };
    use crate::fs_guard::{canonical_json, sha256};
    use crate::ledger_auth::{LedgerKey, seal_record};
    use serde_json::{Value, json};
    use std::fs;
    use std::io::Write as _;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    #[test]
    fn pre_activation_prefix_survives_append_but_rejects_rewrite_and_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("ledger.jsonl");
        let prefix = b"{\"before\":true}\n";
        fs::write(&path, prefix).unwrap();
        let metadata = fs::metadata(&path).unwrap();
        let digest = sha256(prefix);
        assert_eq!(
            verify_exact_prefix(
                &path,
                metadata.ino(),
                u64::try_from(prefix.len()).unwrap(),
                &digest,
                Some(metadata.dev()),
            )
            .unwrap(),
            metadata.dev()
        );
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"{\"after\":true}\n")
            .unwrap();
        assert!(
            verify_exact_prefix(
                &path,
                metadata.ino(),
                u64::try_from(prefix.len()).unwrap(),
                &digest,
                Some(metadata.dev()),
            )
            .is_ok()
        );
        let mut bytes = fs::read(&path).unwrap();
        bytes[2] ^= 1;
        fs::write(&path, bytes).unwrap();
        assert!(
            verify_exact_prefix(
                &path,
                metadata.ino(),
                u64::try_from(prefix.len()).unwrap(),
                &digest,
                Some(metadata.dev()),
            )
            .is_err()
        );
        fs::remove_file(&path).unwrap();
        fs::write(&path, prefix).unwrap();
        assert!(
            verify_exact_prefix(
                &path,
                metadata.ino(),
                u64::try_from(prefix.len()).unwrap(),
                &digest,
                Some(metadata.dev()),
            )
            .is_err()
        );
    }

    #[test]
    fn baseline_binds_chain_position_and_every_prefix() {
        let temp = tempfile::tempdir().unwrap();
        let chain = temp.path().join("checkpoints.jsonl");
        let first = json!({"record_sha256":"a".repeat(64)});
        let second = json!({"record_sha256":"b".repeat(64)});
        fs::write(
            &chain,
            [
                canonical_json(&first).unwrap(),
                b"\n".to_vec(),
                canonical_json(&second).unwrap(),
                b"\n".to_vec(),
            ]
            .concat(),
        )
        .unwrap();
        assert!(checkpoint_chain_contains(&chain, &"a".repeat(64), 1).unwrap());
        assert!(!checkpoint_chain_contains(&chain, &"a".repeat(64), 2).unwrap());

        let mut baseline = HindsightBaseline {
            schema: HINDSIGHT_BASELINE_SCHEMA.into(),
            checkpoint_record_sha256: "a".repeat(64),
            continuity_epoch: "epoch-1".into(),
            checkpoint_chain_records: 1,
            ledger_prefixes: vec![HindsightLedgerPrefix {
                relative_path: "runtime/fill_history.jsonl".into(),
                device: 1,
                inode: 2,
                captured_size: 3,
                prefix_sha256: "c".repeat(64),
            }],
            evidence_sha256: String::new(),
        };
        baseline.evidence_sha256 = hindsight_baseline_digest(&baseline).unwrap();
        assert!(validate_hindsight_baseline(&baseline).is_ok());
        baseline.ledger_prefixes[0].prefix_sha256 = "d".repeat(64);
        assert!(validate_hindsight_baseline(&baseline).is_err());
    }

    #[test]
    fn probation_state_rejects_ambiguous_status_and_hash() {
        let mut state = State {
            schema: "astrid.edge_rescue_helper.probation_state.v1".into(),
            appliance_id: Some("avado-test".into()),
            status: "active".into(),
            generation_id: "gen-new".into(),
            previous_generation_id: "gen-old".into(),
            host_boot_id: "00000000-0000-0000-0000-000000000000".into(),
            started_at_unix_ms: 1,
            baseline_swap_bytes: 0,
            last_sample_at_unix_ms: 1,
            sample_count: 0,
            maximum_sample_gap_ms: 0,
            fill_history_device: None,
            fill_history_inode: None,
            fill_history_size: None,
            fill_history_prefix_sha256: None,
            reservoir_challenge_count: 0,
            last_reservoir_challenge_sha256: None,
            hindsight_baseline: None,
            active_profile_sha256: Some("b".repeat(64)),
            report_projection_sha256: Some("c".repeat(64)),
            ledger_head_sha256: "a".repeat(64),
            updated_at_unix_ms: 1,
        };
        assert!(validate_state(&state).is_ok());
        state.status = "maybe".into();
        assert!(validate_state(&state).is_err());
    }

    #[test]
    fn append_only_ledger_recovers_state_when_atomic_cache_is_absent() {
        let temp = tempfile::tempdir().unwrap();
        let ledger = temp.path().join("probation.jsonl");
        let key = LedgerKey::for_test(0x4c);
        let (record, digest) = chained_record(
            json!({
                "schema": RECORD_SCHEMA,
                "appliance_id": "avado-test",
                "phase": "started",
                "recorded_at_unix_ms": 10,
                "generation_id": "gen-new",
                "previous_generation_id": "gen-old",
                "host_boot_id": "00000000-0000-0000-0000-000000000000",
                "baseline_swap_bytes": 7,
                "authority": "immutable_root_probation_evidence"
            }),
            None,
            &key,
        );
        fs::write(&ledger, [record.as_slice(), b"\n"].concat()).unwrap();
        fs::set_permissions(&ledger, fs::Permissions::from_mode(0o600)).unwrap();

        let (head, recovered) =
            replay_ledger_with_key(&ledger, false, &key, Some("avado-test")).unwrap();
        assert_eq!(head.as_deref(), Some(digest.as_str()));
        let recovered = recovered.unwrap();
        assert_eq!(recovered.status, "active");
        assert_eq!(recovered.generation_id, "gen-new");
        assert_eq!(recovered.appliance_id.as_deref(), Some("avado-test"));
        assert!(replay_ledger_with_key(&ledger, false, &key, Some("icp-other-box")).is_err());

        let mut tampered: Value = serde_json::from_slice(&record).unwrap();
        tampered["generation_id"] = Value::String("gen-forged".into());
        fs::write(
            &ledger,
            [canonical_json(&tampered).unwrap().as_slice(), b"\n"].concat(),
        )
        .unwrap();
        assert!(replay_ledger_with_key(&ledger, false, &key, Some("avado-test")).is_err());
    }

    fn chained_record(
        mut value: Value,
        previous: Option<String>,
        key: &LedgerKey,
    ) -> (Vec<u8>, String) {
        value["previous_record_sha256"] = previous.map_or(Value::Null, Value::String);
        let digest = seal_record(&mut value, key, "probation").unwrap();
        (canonical_json(&value).unwrap(), digest)
    }
}
