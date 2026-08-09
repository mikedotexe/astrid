//! Immutable validation and transactional activation of candidate appliance profiles.
//!
//! Candidate profiles are never consumed directly by systemd.  This module
//! parses the exact appliance profile, rejects every unknown or authority-
//! bearing mutation, and projects only a narrow set of bounded operational
//! values into a root-owned environment file.  The projection changes in the
//! same crash-recoverable transaction as the generation pointer.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read as _;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{Config, valid_hex64, valid_identifier};
use crate::fs_guard::{
    atomic_write, canonical_json, ensure_within, read_json, read_regular, sha256,
};
use crate::profile_schema::{KNOWN_KEYS, MUTABLE_KEYS};
use crate::{Error, Result};

const RELEASE_MANIFEST_SCHEMA: &str = "astrid.edge_self_change.runtime_projections.v1";
const TRANSACTION_SCHEMA: &str = "astrid.edge_rescue_helper.profile_transaction.v1";
const PENDING_SCHEMA: &str = "astrid.edge_rescue_helper.profile_transaction_pending.v1";
const AUTHORITY: &str = "immutable_root_validated_profile_projection:v1";
const RELEASE_MANIFEST: &str = "metadata/runtime-projections.json";
const ACTIVE_PROFILE: &str = "active-profile.env";
const TRANSACTION_DIRECTORY: &str = "profile-transactions";
const MAX_PROFILE_BYTES: u64 = 128 * 1024;
const MAX_REPORT_BYTES: u64 = 4 * 1024 * 1024;

const REPORT_SCRIPTS: &[&str] = &[
    "scripts/astrid_at_a_glance.py",
    "scripts/report_edge_activity.py",
    "scripts/report_edge_appliance.py",
];
const NON_BROKER_REPORT_SCRIPTS: &[&str] = &[
    "scripts/edge_hindsight.py",
    "scripts/report_edge_appliance.sh",
    "scripts/report_edge_fleet_activity.py",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReportProjection {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReleaseProjectionManifest {
    pub schema: String,
    pub appliance_id: String,
    pub profile_source: String,
    pub profile_source_sha256: String,
    pub profile_projection_sha256: String,
    pub profile_mutated_by_candidate: bool,
    pub report_scripts: Vec<ReportProjection>,
    pub report_projection_sha256: String,
    pub reports_mutated_by_candidate: bool,
    pub authority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ProjectionSnapshot {
    generation_id: String,
    profile_source: String,
    profile_source_sha256: String,
    profile_projection_sha256: String,
    profile_projection_size: u64,
    report_projection_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct TransactionManifest {
    schema: String,
    transaction_id: String,
    target: ProjectionSnapshot,
    prior: ProjectionSnapshot,
    authority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Pending {
    schema: String,
    transaction_id: String,
    manifest_sha256: String,
    authority: String,
}

#[derive(Debug, Clone)]
pub struct PreparedProfileTransaction {
    pub transaction_id: String,
    pub target_generation_id: String,
    pub prior_generation_id: String,
    pub target_profile_sha256: String,
    pub prior_profile_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActiveProjectionEvidence {
    pub schema: &'static str,
    pub transaction_id: String,
    pub generation_id: String,
    pub active_profile_sha256: String,
    pub report_projection_sha256: String,
    pub profile_mutated_by_candidate: bool,
    pub reports_mutated_by_candidate: bool,
    pub authority: &'static str,
}

#[derive(Debug)]
struct ValidatedRelease {
    snapshot: ProjectionSnapshot,
    projection: Vec<u8>,
    manifest: ReleaseProjectionManifest,
}

/// Root-controlled environment file consumed after the operator profile.
#[must_use]
pub fn active_profile_path(config: &Config) -> PathBuf {
    config.roots.supervisor_state.join(ACTIVE_PROFILE)
}

/// Root-owned transaction directory used by the updater and boot guard.
#[must_use]
pub fn transaction_root_path(config: &Config) -> PathBuf {
    config.roots.state_snapshots.join(TRANSACTION_DIRECTORY)
}

/// Derive the exact canonical projection for immutable bootstrap/install code.
/// This performs all release/profile/report validation but never writes host state.
pub fn projection_bytes_for_generation(config: &Config, generation: &Path) -> Result<Vec<u8>> {
    Ok(validate_release(config, generation)?.projection)
}

/// Initialize the root-owned active profile from one already verified release.
///
/// This is intentionally create-once and idempotent: reinstalling the same
/// generation verifies the existing bytes, while any divergence or pending
/// transition fails closed instead of replacing operator-visible state.
pub fn bootstrap_active_generation(
    config: &Config,
    generation: &Path,
) -> Result<ActiveProjectionEvidence> {
    crate::generation::require_effective_uid(0, "root profile bootstrap")?;
    bootstrap_active_generation_inner(config, generation, true)
}

pub(crate) fn bootstrap_active_generation_inner(
    config: &Config,
    generation: &Path,
    require_root_owner: bool,
) -> Result<ActiveProjectionEvidence> {
    require_roots(config, require_root_owner)?;
    if pending_path(config).exists() || pending_path(config).is_symlink() {
        return Err(Error::new(
            "profile bootstrap is forbidden while a transaction is pending",
        ));
    }
    let release = validate_release(config, generation)?;
    let active = active_profile_path(config);
    match fs::symlink_metadata(&active) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            atomic_write(&active, &release.projection, 0o400, false)?;
        },
        Err(error) => return Err(error.into()),
        Ok(_) => {
            if read_active_profile(config, require_root_owner)? != release.projection {
                return Err(Error::new(
                    "existing active profile differs from bootstrap generation",
                ));
            }
        },
    }
    verify_active_generation(config, generation, require_root_owner)
}

/// Validate immutable generation projection metadata without consulting the
/// live profile pointer. Transition journals use this to bind both A/B slots.
pub(crate) fn generation_projection_evidence(
    config: &Config,
    generation: &Path,
) -> Result<ActiveProjectionEvidence> {
    Ok(evidence(&validate_release(config, generation)?, "none"))
}

/// Validate the candidate appliance profile before the expensive build runs.
pub fn validate_candidate_profile_source(
    config: &Config,
    source_root: &Path,
    prior_generation: &Path,
    changed_paths: &[String],
) -> Result<String> {
    reject_non_broker_report_mutation(changed_paths)?;
    let relative = profile_source(config);
    let target = parse_profile(
        config,
        &read_regular(&source_root.join(&relative), MAX_PROFILE_BYTES)?,
    )?;
    let prior = parse_profile(
        config,
        &read_regular(&prior_generation.join(&relative), MAX_PROFILE_BYTES)?,
    )?;
    let projection = validate_pair_and_project(config, &target, &prior)?;
    Ok(sha256(&projection))
}

/// Write immutable build-generated projection metadata into a candidate bundle.
pub fn write_release_projection_manifest(
    config: &Config,
    changed_paths: &[String],
    bundle: &Path,
    prior_generation: &Path,
) -> Result<ReleaseProjectionManifest> {
    reject_non_broker_report_mutation(changed_paths)?;
    let relative = profile_source(config);
    let target_bytes = read_regular(&bundle.join(&relative), MAX_PROFILE_BYTES)?;
    let target = parse_profile(config, &target_bytes)?;
    let prior = parse_profile(
        config,
        &read_regular(&prior_generation.join(&relative), MAX_PROFILE_BYTES)?,
    )?;
    let projection = validate_pair_and_project(config, &target, &prior)?;
    let report_scripts = report_inventory(bundle)?;
    let manifest = ReleaseProjectionManifest {
        schema: RELEASE_MANIFEST_SCHEMA.to_owned(),
        appliance_id: config.appliance_id.clone(),
        profile_source: relative.clone(),
        profile_source_sha256: sha256(&target_bytes),
        profile_projection_sha256: sha256(&projection),
        profile_mutated_by_candidate: changed_paths.iter().any(|path| path == &relative),
        report_projection_sha256: report_projection_digest(&report_scripts)?,
        reports_mutated_by_candidate: changed_paths
            .iter()
            .any(|path| REPORT_SCRIPTS.contains(&path.as_str())),
        report_scripts,
        authority: AUTHORITY.to_owned(),
    };
    let output = bundle.join(RELEASE_MANIFEST);
    if let Some(parent) = output.parent() {
        fs::create_dir(parent)?;
    }
    atomic_write(&output, &canonical_json(&manifest)?, 0o444, false)?;
    Ok(manifest)
}

pub(crate) fn prepare_for_transition(
    config: &Config,
    target_generation: &Path,
    prior_generation: &Path,
    require_root_owner: bool,
) -> Result<PreparedProfileTransaction> {
    require_roots(config, require_root_owner)?;
    if pending_path(config).exists() || pending_path(config).is_symlink() {
        return Err(Error::new(
            "another profile projection transaction remains pending",
        ));
    }
    let target = validate_release(config, target_generation)?;
    let prior = validate_release(config, prior_generation)?;
    let target_profile = parse_profile(
        config,
        &read_regular(
            &target_generation.join(&target.manifest.profile_source),
            MAX_PROFILE_BYTES,
        )?,
    )?;
    let prior_profile = parse_profile(
        config,
        &read_regular(
            &prior_generation.join(&prior.manifest.profile_source),
            MAX_PROFILE_BYTES,
        )?,
    )?;
    let target_projection = validate_pair_and_project(config, &target_profile, &prior_profile)?;
    if target_projection != target.projection {
        return Err(Error::new(
            "target profile projection changed after build validation",
        ));
    }
    if read_active_profile(config, require_root_owner)? != prior.projection {
        return Err(Error::new(
            "root-controlled active profile differs from selected prior generation",
        ));
    }
    let transaction_id = new_transaction_id(
        &target.snapshot.generation_id,
        &prior.snapshot.generation_id,
        &target.snapshot.profile_projection_sha256,
    )?;
    let root = transaction_root_path(config);
    let partial = root.join(format!(".{transaction_id}.partial"));
    let final_root = root.join(&transaction_id);
    ensure_within(&root, &partial, false)?;
    ensure_within(&root, &final_root, false)?;
    if partial.exists() || partial.is_symlink() || final_root.exists() || final_root.is_symlink() {
        return Err(Error::new("profile transaction identifier collision"));
    }
    fs::create_dir(&partial)?;
    fs::set_permissions(&partial, fs::Permissions::from_mode(0o700))?;
    let manifest = TransactionManifest {
        schema: TRANSACTION_SCHEMA.to_owned(),
        transaction_id: transaction_id.clone(),
        target: target.snapshot,
        prior: prior.snapshot,
        authority: AUTHORITY.to_owned(),
    };
    let manifest_bytes = canonical_json(&manifest)?;
    let result = (|| {
        atomic_write(&partial.join("prior.env"), &prior.projection, 0o400, false)?;
        atomic_write(
            &partial.join("target.env"),
            &target_projection,
            0o400,
            false,
        )?;
        atomic_write(
            &partial.join("manifest.json"),
            &manifest_bytes,
            0o400,
            false,
        )?;
        fs::set_permissions(&partial, fs::Permissions::from_mode(0o500))?;
        fs::rename(&partial, &final_root)?;
        File::open(&root)?.sync_all()?;
        let pending = Pending {
            schema: PENDING_SCHEMA.to_owned(),
            transaction_id: transaction_id.clone(),
            manifest_sha256: sha256(&manifest_bytes),
            authority: AUTHORITY.to_owned(),
        };
        atomic_write(
            &pending_path(config),
            &canonical_json(&pending)?,
            0o400,
            false,
        )?;
        Ok(PreparedProfileTransaction {
            transaction_id,
            target_generation_id: manifest.target.generation_id,
            prior_generation_id: manifest.prior.generation_id,
            target_profile_sha256: manifest.target.profile_projection_sha256,
            prior_profile_sha256: manifest.prior.profile_projection_sha256,
        })
    })();
    if result.is_err() && partial.exists() {
        let _ = fs::remove_dir_all(&partial);
    }
    result
}

pub(crate) fn apply_target_for_transition(
    config: &Config,
    transaction: &PreparedProfileTransaction,
    require_root_owner: bool,
) -> Result<ActiveProjectionEvidence> {
    apply_selected(config, transaction, true, require_root_owner)
}

pub(crate) fn restore_prior_for_transition(
    config: &Config,
    transaction: &PreparedProfileTransaction,
    require_root_owner: bool,
) -> Result<ActiveProjectionEvidence> {
    let evidence = apply_selected(config, transaction, false, require_root_owner)?;
    clear_pending(config)?;
    Ok(evidence)
}

pub(crate) fn commit_for_transition(
    config: &Config,
    transaction: &PreparedProfileTransaction,
    require_root_owner: bool,
) -> Result<()> {
    let (_, manifest) = load_pending(config, transaction, require_root_owner)?;
    let active = read_active_profile(config, require_root_owner)?;
    if sha256(&active) != manifest.target.profile_projection_sha256 {
        return Err(Error::new("active target profile differs at commit"));
    }
    clear_pending(config)
}

pub(crate) fn reconcile_for_transition(
    config: &Config,
    selected_generation: &Path,
    require_root_owner: bool,
) -> Result<ActiveProjectionEvidence> {
    require_roots(config, require_root_owner)?;
    let selected = validate_release(config, selected_generation)?;
    if !pending_path(config).exists() && !pending_path(config).is_symlink() {
        let active = read_active_profile(config, require_root_owner)?;
        if active != selected.projection {
            return Err(Error::new(
                "active profile does not match selected generation and no transaction can repair it",
            ));
        }
        return Ok(evidence(&selected, "none"));
    }
    let pending: Pending = read_owned_json(&pending_path(config), 16 * 1024, require_root_owner)?;
    validate_pending(&pending)?;
    let placeholder = PreparedProfileTransaction {
        transaction_id: pending.transaction_id,
        target_generation_id: String::new(),
        prior_generation_id: String::new(),
        target_profile_sha256: String::new(),
        prior_profile_sha256: String::new(),
    };
    let (_, manifest) = load_pending_loose(config, &placeholder, require_root_owner)?;
    let transaction = PreparedProfileTransaction {
        transaction_id: placeholder.transaction_id,
        target_generation_id: manifest.target.generation_id.clone(),
        prior_generation_id: manifest.prior.generation_id.clone(),
        target_profile_sha256: manifest.target.profile_projection_sha256.clone(),
        prior_profile_sha256: manifest.prior.profile_projection_sha256.clone(),
    };
    let target = selected.snapshot.generation_id == transaction.target_generation_id;
    if !target && selected.snapshot.generation_id != transaction.prior_generation_id {
        return Err(Error::new(
            "selected generation is absent from pending profile transaction",
        ));
    }
    let evidence = apply_selected(config, &transaction, target, require_root_owner)?;
    clear_pending(config)?;
    Ok(evidence)
}

pub(crate) fn verify_active_generation(
    config: &Config,
    generation: &Path,
    require_root_owner: bool,
) -> Result<ActiveProjectionEvidence> {
    require_roots(config, require_root_owner)?;
    let release = validate_release(config, generation)?;
    if read_active_profile(config, require_root_owner)? != release.projection {
        return Err(Error::new(
            "active root profile is not the selected generation projection",
        ));
    }
    Ok(evidence(&release, "none"))
}

fn apply_selected(
    config: &Config,
    transaction: &PreparedProfileTransaction,
    target: bool,
    require_root_owner: bool,
) -> Result<ActiveProjectionEvidence> {
    let (root, manifest) = load_pending(config, transaction, require_root_owner)?;
    let (snapshot, basename) = if target {
        (&manifest.target, "target.env")
    } else {
        (&manifest.prior, "prior.env")
    };
    let bytes = read_owned_regular(&root.join(basename), MAX_PROFILE_BYTES, require_root_owner)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != snapshot.profile_projection_size
        || sha256(&bytes) != snapshot.profile_projection_sha256
    {
        return Err(Error::new("sealed profile snapshot digest failed"));
    }
    atomic_write(&active_profile_path(config), &bytes, 0o400, true)?;
    if read_active_profile(config, require_root_owner)? != bytes {
        return Err(Error::new("atomic active profile verification failed"));
    }
    let generation = config.roots.releases.join(&snapshot.generation_id);
    let mut evidence = verify_active_generation(config, &generation, require_root_owner)?;
    evidence
        .transaction_id
        .clone_from(&transaction.transaction_id);
    Ok(evidence)
}

fn evidence(release: &ValidatedRelease, transaction_id: &str) -> ActiveProjectionEvidence {
    ActiveProjectionEvidence {
        schema: "astrid.edge_rescue_helper.active_projection_evidence.v1",
        transaction_id: transaction_id.to_owned(),
        generation_id: release.snapshot.generation_id.clone(),
        active_profile_sha256: release.snapshot.profile_projection_sha256.clone(),
        report_projection_sha256: release.snapshot.report_projection_sha256.clone(),
        profile_mutated_by_candidate: release.manifest.profile_mutated_by_candidate,
        reports_mutated_by_candidate: release.manifest.reports_mutated_by_candidate,
        authority: AUTHORITY,
    }
}

fn validate_release(config: &Config, generation: &Path) -> Result<ValidatedRelease> {
    ensure_within(&config.roots.releases, generation, true)?;
    let generation_id = generation
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| valid_identifier(name))
        .ok_or_else(|| Error::new("profile release generation ID is invalid"))?
        .to_owned();
    if generation.parent() != Some(config.roots.releases.as_path()) {
        return Err(Error::new("profile release is not one direct generation"));
    }
    let generation_manifest: serde_json::Value = read_json(
        &generation.join(".astrid-edge-generation.json"),
        16 * 1024 * 1024,
    )?;
    let is_initial = generation_manifest
        .get("schema")
        .and_then(serde_json::Value::as_str)
        == Some("astrid.edge_self_change.initial_generation.v1");
    let profile_source = profile_source(config);
    let source_bytes = read_regular(&generation.join(&profile_source), MAX_PROFILE_BYTES)?;
    let parsed = parse_profile(config, &source_bytes)?;
    let projection = project_profile(config, &parsed)?;
    let report_scripts = report_inventory(generation)?;
    let derived = ReleaseProjectionManifest {
        schema: RELEASE_MANIFEST_SCHEMA.to_owned(),
        appliance_id: config.appliance_id.clone(),
        profile_source: profile_source.clone(),
        profile_source_sha256: sha256(&source_bytes),
        profile_projection_sha256: sha256(&projection),
        profile_mutated_by_candidate: false,
        report_projection_sha256: report_projection_digest(&report_scripts)?,
        reports_mutated_by_candidate: false,
        report_scripts,
        authority: AUTHORITY.to_owned(),
    };
    let manifest = if is_initial {
        if generation.join(RELEASE_MANIFEST).exists() {
            return Err(Error::new(
                "operator initial generation may not claim candidate projections",
            ));
        }
        derived
    } else {
        let actual: ReleaseProjectionManifest =
            read_json(&generation.join(RELEASE_MANIFEST), 128 * 1024)?;
        if actual.schema != RELEASE_MANIFEST_SCHEMA
            || actual.appliance_id != config.appliance_id
            || actual.profile_source != derived.profile_source
            || actual.profile_source_sha256 != derived.profile_source_sha256
            || actual.profile_projection_sha256 != derived.profile_projection_sha256
            || actual.report_scripts != derived.report_scripts
            || actual.report_projection_sha256 != derived.report_projection_sha256
            || actual.authority != AUTHORITY
        {
            return Err(Error::new(
                "candidate runtime projection manifest does not match immutable release bytes",
            ));
        }
        actual
    };
    Ok(ValidatedRelease {
        snapshot: ProjectionSnapshot {
            generation_id,
            profile_source,
            profile_source_sha256: manifest.profile_source_sha256.clone(),
            profile_projection_sha256: manifest.profile_projection_sha256.clone(),
            profile_projection_size: u64::try_from(projection.len()).unwrap_or(u64::MAX),
            report_projection_sha256: manifest.report_projection_sha256.clone(),
        },
        projection,
        manifest,
    })
}

fn validate_pair_and_project(
    config: &Config,
    target: &BTreeMap<String, String>,
    prior: &BTreeMap<String, String>,
) -> Result<Vec<u8>> {
    validate_pair_and_project_for(is_icp(config), target, prior)
}

fn validate_pair_and_project_for(
    icp: bool,
    target: &BTreeMap<String, String>,
    prior: &BTreeMap<String, String>,
) -> Result<Vec<u8>> {
    if target.keys().ne(prior.keys()) {
        return Err(Error::new(
            "candidate profile changed the exact profile schema",
        ));
    }
    for key in target.keys().filter(|key| !is_mutable_key(key)) {
        if target.get(key) != prior.get(key) {
            return Err(Error::new(format!(
                "candidate profile attempted to change protected key: {key}"
            )));
        }
    }
    project_profile_for(icp, target)
}

fn project_profile(config: &Config, profile: &BTreeMap<String, String>) -> Result<Vec<u8>> {
    project_profile_for(is_icp(config), profile)
}

fn project_profile_for(icp: bool, profile: &BTreeMap<String, String>) -> Result<Vec<u8>> {
    let mut output = String::new();
    for key in MUTABLE_KEYS {
        let value = profile
            .get(*key)
            .ok_or_else(|| Error::new(format!("candidate profile omits mutable key: {key}")))?;
        validate_mutable_value(icp, key, value)?;
        output.push_str(key);
        output.push('=');
        output.push_str(value);
        output.push('\n');
    }
    Ok(output.into_bytes())
}

fn parse_profile(config: &Config, bytes: &[u8]) -> Result<BTreeMap<String, String>> {
    parse_profile_for(is_icp(config), bytes)
}

fn parse_profile_for(icp: bool, bytes: &[u8]) -> Result<BTreeMap<String, String>> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| Error::new("candidate appliance profile is not UTF-8"))?;
    if text.contains('\r') || text.contains('\0') {
        return Err(Error::new(
            "candidate appliance profile contains forbidden bytes",
        ));
    }
    let expected = expected_keys(icp);
    let mut values = BTreeMap::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.trim() != line || line.len() > 1_024 {
            return Err(Error::new("candidate appliance profile line is malformed"));
        }
        let (key, raw) = line
            .split_once('=')
            .ok_or_else(|| Error::new("candidate appliance profile assignment is malformed"))?;
        if !expected.contains(key) || !valid_env_key(key) || values.contains_key(key) {
            return Err(Error::new(
                "candidate appliance profile has an unknown or duplicate key",
            ));
        }
        let value = parse_env_value(raw)?;
        values.insert(key.to_owned(), value);
    }
    if values.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected {
        return Err(Error::new(
            "candidate appliance profile schema is incomplete",
        ));
    }
    Ok(values)
}

fn parse_env_value(raw: &str) -> Result<String> {
    if raw.is_empty() || raw.len() > 512 {
        return Err(Error::new("candidate profile value is empty or oversized"));
    }
    let value = if let Some(inner) = raw.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        if inner.is_empty()
            || inner.contains(['"', '\\', '$', '`'])
            || inner.chars().any(char::is_control)
        {
            return Err(Error::new("quoted candidate profile value is unsafe"));
        }
        inner
    } else {
        if raw.contains('"')
            || !raw
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || "._:/-".contains(ch))
        {
            return Err(Error::new("candidate profile value is not a literal"));
        }
        raw
    };
    Ok(value.to_owned())
}

fn validate_mutable_value(icp: bool, key: &str, value: &str) -> Result<()> {
    let integer = |minimum: u64, maximum: u64| -> Result<()> {
        let parsed = value
            .parse::<u64>()
            .map_err(|_| Error::new(format!("mutable profile key is not numeric: {key}")))?;
        if !(minimum..=maximum).contains(&parsed) {
            return Err(Error::new(format!(
                "mutable profile key is outside bounds: {key}"
            )));
        }
        Ok(())
    };
    match key {
        "TOKIO_WORKER_THREADS" | "ASTRID_EDGE_AUTONOMY_CHAIN_SESSION_MAX_AUTHORED_TURNS" => {
            integer(1, 4)
        },
        "ASTRID_EDGE_TICK_HZ" => integer(10, 30),
        "ASTRID_EDGE_SPECTRAL_ENABLED"
        | "ASTRID_EDGE_AUTONOMY_ENABLED"
        | "ASTRID_EDGE_AUTONOMY_EVENT_DRIVEN"
        | "ASTRID_EDGE_AUTONOMY_JOURNAL_AUTHORED_TURNS"
        | "ASTRID_EDGE_PERCEPTUAL_NOTEBOOK_ENABLED" => boolean(value, key),
        "ASTRID_EDGE_SPECTRAL_ROLLUP_SECONDS" => integer(60, 900),
        "ASTRID_EDGE_AUTONOMY_INTERVAL_MINUTES" => integer(5, 120),
        "ASTRID_EDGE_AUTONOMY_EVENT_HEARTBEAT_MINUTES" => integer(15, 360),
        "ASTRID_EDGE_AUTONOMY_FOLLOW_UP_MINUTES" => integer(3, 60),
        "ASTRID_EDGE_AUTONOMY_MAX_CHAIN_STEPS"
        | "ASTRID_EDGE_AUTONOMY_SESSION_MAX_AUTHORED_TURNS" => integer(1, 8),
        "ASTRID_EDGE_AUTONOMY_INITIAL_DELAY_SECONDS" => integer(30, 1_800),
        "ASTRID_EDGE_AUTONOMY_QUIET_MINUTES" => integer(1, 60),
        "ASTRID_EDGE_AUTONOMY_MAX_TURNS_PER_DAY"
        | "ASTRID_EDGE_PERCEPTUAL_NOTEBOOK_MAX_PER_DAY" => integer(1, 96),
        "ASTRID_EDGE_AUTONOMY_TIMEOUT_SECONDS" => integer(120, 720),
        "ASTRID_EDGE_AUTONOMY_PROMPT_PROFILE" => one_of(value, key, &["compact", "detailed"]),
        "ASTRID_EDGE_AUTONOMY_PROMPT_MAX_CHARS" => integer(320, if icp { 900 } else { 1_200 }),
        "ASTRID_EDGE_PERCEPTUAL_NOTEBOOK_WARMUP_SECONDS" => integer(60, 1_800),
        "ASTRID_EDGE_PERCEPTUAL_NOTEBOOK_INTERVAL_SECONDS" => integer(300, 3_600),
        "ASTRID_EDGE_PERCEPTUAL_NOTEBOOK_HEARTBEAT_SECONDS" => integer(3_600, 86_400),
        "ASTRID_OLLAMA_KEEP_ALIVE" => one_of(value, key, &["30m", "1h", "2h", "4h"]),
        "ASTRID_OLLAMA_CONTEXT" => integer(1_024, if icp { 3_072 } else { 4_096 }),
        "ASTRID_OLLAMA_MAX_OUTPUT" => integer(64, if icp { 112 } else { 192 }),
        _ => Err(Error::new(
            "profile key is not in the mutable projection allowlist",
        )),
    }
}

fn boolean(value: &str, key: &str) -> Result<()> {
    one_of(value, key, &["true", "false"])
}

fn one_of(value: &str, key: &str, allowed: &[&str]) -> Result<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(Error::new(format!(
            "mutable profile key has forbidden value: {key}"
        )))
    }
}

fn expected_keys(icp: bool) -> BTreeSet<&'static str> {
    KNOWN_KEYS
        .iter()
        .copied()
        .filter(|key| icp || *key != "ASTRID_EDGE_AUDIO_DEVICE")
        .collect()
}

fn is_mutable_key(key: &str) -> bool {
    MUTABLE_KEYS.contains(&key)
}

fn valid_env_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 128
        && key
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_uppercase() || ch.is_ascii_digit())
}

fn is_icp(config: &Config) -> bool {
    config.appliance_id.starts_with("icp")
}

fn profile_source(config: &Config) -> String {
    format!(
        "packaging/appliances/{}",
        if is_icp(config) {
            "icp-j3455-8g.env"
        } else {
            "avado-i3-16g.env"
        }
    )
}

fn report_inventory(root: &Path) -> Result<Vec<ReportProjection>> {
    REPORT_SCRIPTS
        .iter()
        .map(|relative| {
            let bytes = read_regular(&root.join(relative), MAX_REPORT_BYTES)?;
            Ok(ReportProjection {
                path: (*relative).to_owned(),
                size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                sha256: sha256(&bytes),
            })
        })
        .collect()
}

fn report_projection_digest(reports: &[ReportProjection]) -> Result<String> {
    Ok(sha256(&canonical_json(&reports.to_vec())?))
}

fn reject_non_broker_report_mutation(changed_paths: &[String]) -> Result<()> {
    if changed_paths
        .iter()
        .any(|path| NON_BROKER_REPORT_SCRIPTS.contains(&path.as_str()))
    {
        return Err(Error::new(
            "candidate changed a report script with no immutable presentation-broker route",
        ));
    }
    Ok(())
}

fn load_pending(
    config: &Config,
    expected: &PreparedProfileTransaction,
    require_root_owner: bool,
) -> Result<(PathBuf, TransactionManifest)> {
    let result = load_pending_loose(config, expected, require_root_owner)?;
    if result.1.target.generation_id != expected.target_generation_id
        || result.1.prior.generation_id != expected.prior_generation_id
        || result.1.target.profile_projection_sha256 != expected.target_profile_sha256
        || result.1.prior.profile_projection_sha256 != expected.prior_profile_sha256
    {
        return Err(Error::new("pending profile transaction identity changed"));
    }
    Ok(result)
}

fn load_pending_loose(
    config: &Config,
    expected: &PreparedProfileTransaction,
    require_root_owner: bool,
) -> Result<(PathBuf, TransactionManifest)> {
    let pending: Pending = read_owned_json(&pending_path(config), 16 * 1024, require_root_owner)?;
    validate_pending(&pending)?;
    if pending.transaction_id != expected.transaction_id {
        return Err(Error::new("pending profile transaction pointer differs"));
    }
    let root = transaction_root_path(config).join(&pending.transaction_id);
    ensure_within(&transaction_root_path(config), &root, true)?;
    require_private_directory(&root, require_root_owner, 0o500)?;
    let manifest_bytes =
        read_owned_regular(&root.join("manifest.json"), 64 * 1024, require_root_owner)?;
    if sha256(&manifest_bytes) != pending.manifest_sha256 {
        return Err(Error::new(
            "pending profile transaction manifest digest failed",
        ));
    }
    let manifest: TransactionManifest = serde_json::from_slice(&manifest_bytes)?;
    validate_transaction_manifest(&manifest, &pending.transaction_id)?;
    Ok((root, manifest))
}

fn validate_transaction_manifest(
    manifest: &TransactionManifest,
    transaction_id: &str,
) -> Result<()> {
    if manifest.schema != TRANSACTION_SCHEMA
        || manifest.transaction_id != transaction_id
        || manifest.authority != AUTHORITY
        || manifest.target.generation_id == manifest.prior.generation_id
        || !valid_snapshot(&manifest.target)
        || !valid_snapshot(&manifest.prior)
    {
        return Err(Error::new("profile transaction manifest is invalid"));
    }
    Ok(())
}

fn valid_snapshot(snapshot: &ProjectionSnapshot) -> bool {
    valid_identifier(&snapshot.generation_id)
        && snapshot.profile_source.starts_with("packaging/appliances/")
        && valid_hex64(&snapshot.profile_source_sha256)
        && valid_hex64(&snapshot.profile_projection_sha256)
        && snapshot.profile_projection_size > 0
        && snapshot.profile_projection_size <= MAX_PROFILE_BYTES
        && valid_hex64(&snapshot.report_projection_sha256)
}

fn validate_pending(pending: &Pending) -> Result<()> {
    if pending.schema != PENDING_SCHEMA
        || !valid_identifier(&pending.transaction_id)
        || !valid_hex64(&pending.manifest_sha256)
        || pending.authority != AUTHORITY
    {
        return Err(Error::new("pending profile transaction pointer is invalid"));
    }
    Ok(())
}

fn require_roots(config: &Config, require_root_owner: bool) -> Result<()> {
    require_private_directory(&config.roots.supervisor_state, require_root_owner, 0o700)?;
    require_private_directory(&transaction_root_path(config), require_root_owner, 0o700)
}

fn require_private_directory(path: &Path, require_root_owner: bool, exact_mode: u32) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != expected_uid(require_root_owner)
        || metadata.mode() & 0o777 != exact_mode
    {
        return Err(Error::new(format!(
            "profile transaction directory identity failed: {}",
            path.display()
        )));
    }
    Ok(())
}

fn read_active_profile(config: &Config, require_root_owner: bool) -> Result<Vec<u8>> {
    read_owned_regular(
        &active_profile_path(config),
        MAX_PROFILE_BYTES,
        require_root_owner,
    )
}

fn read_owned_json<T: serde::de::DeserializeOwned>(
    path: &Path,
    maximum: u64,
    require_root_owner: bool,
) -> Result<T> {
    serde_json::from_slice(&read_owned_regular(path, maximum, require_root_owner)?)
        .map_err(Into::into)
}

fn read_owned_regular(path: &Path, maximum: u64, require_root_owner: bool) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != expected_uid(require_root_owner)
        || metadata.mode() & 0o022 != 0
    {
        return Err(Error::new(format!(
            "profile transaction file identity failed: {}",
            path.display()
        )));
    }
    read_regular(path, maximum)
}

fn expected_uid(require_root_owner: bool) -> u32 {
    if require_root_owner {
        0
    } else {
        nix::unistd::geteuid().as_raw()
    }
}

fn pending_path(config: &Config) -> PathBuf {
    transaction_root_path(config).join("pending.json")
}

fn clear_pending(config: &Config) -> Result<()> {
    fs::remove_file(pending_path(config))?;
    File::open(transaction_root_path(config))?.sync_all()?;
    Ok(())
}

fn new_transaction_id(target: &str, prior: &str, projection_sha256: &str) -> Result<String> {
    let mut entropy = [0_u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut entropy)?;
    let digest = sha256(&canonical_json(&serde_json::json!({
        "target": target,
        "prior": prior,
        "projection_sha256": projection_sha256,
        "entropy_sha256": sha256(&entropy),
    }))?);
    Ok(format!("profile-{}", &digest[..24]))
}

#[cfg(test)]
#[path = "profile_projection_tests.rs"]
mod tests;
