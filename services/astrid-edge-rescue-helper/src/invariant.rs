//! Immutable candidate and package replay implemented by the rescue helper itself.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path};

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::fs_guard::{
    atomic_write, canonical_json, ensure_within, read_regular, sha256, validate_relative_signed,
};
use crate::{Error, Result};

pub(crate) const LOCAL_SOURCE_CAPSULES: &[&str] = &[
    "astrid-capsule-cli",
    "astrid-capsule-fs",
    "astrid-capsule-http",
    "astrid-capsule-shell",
    "astrid-capsule-skills",
    "astrid-capsule-agents",
    "astrid-capsule-memory",
    "astrid-capsule-edge-context",
    "astrid-capsule-edge-introspector",
    "astrid-capsule-edge-spectral",
];
pub(crate) const EXTERNAL_SOURCE_CAPSULES: &[&str] = &[
    "astrid-capsule-context-engine",
    "astrid-capsule-hook-bridge",
    "astrid-capsule-identity",
    "astrid-capsule-openai-compat",
    "astrid-capsule-prompt-builder",
    "astrid-capsule-react",
    "astrid-capsule-registry",
    "astrid-capsule-router",
    "astrid-capsule-session",
    "astrid-capsule-system",
];
/// The complete mutable CPU-edge capsule surface. The external-source half is
/// pinned and imported at bootstrap, but after authentication it is rebuilt
/// under exactly the same candidate and authority gates as repository-local
/// capsules; it is never a permanently carried binary baseline.
pub(crate) const REBUILDABLE_CAPSULES: &[&str] = &[
    "astrid-capsule-cli",
    "astrid-capsule-fs",
    "astrid-capsule-http",
    "astrid-capsule-shell",
    "astrid-capsule-skills",
    "astrid-capsule-agents",
    "astrid-capsule-memory",
    "astrid-capsule-edge-context",
    "astrid-capsule-edge-introspector",
    "astrid-capsule-edge-spectral",
    "astrid-capsule-context-engine",
    "astrid-capsule-hook-bridge",
    "astrid-capsule-identity",
    "astrid-capsule-openai-compat",
    "astrid-capsule-prompt-builder",
    "astrid-capsule-react",
    "astrid-capsule-registry",
    "astrid-capsule-router",
    "astrid-capsule-session",
    "astrid-capsule-system",
];
pub(crate) const ESSENTIAL_CAPSULES: &[&str] = &[
    "astrid-capsule-cli",
    "astrid-capsule-fs",
    "astrid-capsule-http",
    "astrid-capsule-shell",
    "astrid-capsule-skills",
    "astrid-capsule-agents",
    "astrid-capsule-memory",
    "astrid-capsule-edge-context",
    "astrid-capsule-edge-introspector",
    "astrid-capsule-edge-spectral",
    "astrid-capsule-context-engine",
    "astrid-capsule-hook-bridge",
    "astrid-capsule-identity",
    "astrid-capsule-openai-compat",
    "astrid-capsule-prompt-builder",
    "astrid-capsule-react",
    "astrid-capsule-registry",
    "astrid-capsule-router",
    "astrid-capsule-session",
    "astrid-capsule-system",
];
pub(crate) const ESSENTIAL_SCRIPTS: &[&str] = &[
    "warm_ollama_model.sh",
    "report_edge_appliance.py",
    "report_edge_appliance.sh",
    "report_edge_activity.py",
    "report_edge_fleet_activity.py",
    "edge_hindsight.py",
    "astrid_at_a_glance.py",
];

/// The complete model-authored systemd authority surface.  These are only the
/// six appliance base fragments.  Root boundary/profile drop-ins, the rescue
/// stack, broker, steward, generation guard, SSH, sudo, and host units are not
/// members of this set and can never be selected by the transactional unit
/// installer.
pub(crate) const MUTABLE_UNIT_FRAGMENTS: &[&str] = &[
    "ollama-cpu.service",
    "astrid-model-warmup.service",
    "astrid.service",
    "astrid-edge-runtime.service",
    "astrid-edge-hindsight.service",
    "astrid-edge-hindsight.timer",
];

fn validate_capsule_policy_surface() -> Result<()> {
    let partition = LOCAL_SOURCE_CAPSULES
        .iter()
        .chain(EXTERNAL_SOURCE_CAPSULES)
        .copied()
        .collect::<BTreeSet<_>>();
    let rebuildable = REBUILDABLE_CAPSULES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let essential = ESSENTIAL_CAPSULES.iter().copied().collect::<BTreeSet<_>>();
    let partition_size = LOCAL_SOURCE_CAPSULES
        .len()
        .checked_add(EXTERNAL_SOURCE_CAPSULES.len())
        .ok_or_else(|| Error::new("immutable capsule policy size overflow"))?;
    if partition.len() != partition_size || partition != rebuildable || rebuildable != essential {
        return Err(Error::new(
            "immutable capsule policy is not an exact twenty-capsule rebuildable partition",
        ));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct CandidateReplay {
    schema: &'static str,
    source_files: usize,
    source_tree_sha256: String,
    immutable_domains_absent: bool,
    unit_envelope_valid: bool,
    service_template_authority: &'static str,
    immutable_native_installer_replay: bool,
    migration_policy: &'static str,
    candidate_coupled_shadow_gate: &'static str,
}

#[derive(Debug, Serialize)]
struct PackageReplay {
    schema: &'static str,
    target: String,
    binaries: usize,
    capsules: usize,
    capsule_inventory_sha256: String,
    install_layout_valid: bool,
    installed_capsules_match_archives: bool,
    runtime_scripts: usize,
    service_template_authority: &'static str,
    native_install_replay: &'static str,
    source_bodies_retained: bool,
    installer_dry_run: InstallerDryRunEvidence,
    candidate_shadow: crate::shadow::CandidateShadowEvidence,
}

#[derive(Debug, Serialize)]
struct InstallerDryRunEvidence {
    provenance: &'static str,
    files: usize,
    bytes: u64,
    source_tree_sha256: String,
    installed_tree_sha256: String,
    host_paths_written: bool,
    evidence_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct CapsuleFile {
    path: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstalledCapsuleSet {
    schema: String,
    authority: String,
    capsules: Vec<InstalledCapsuleRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstalledCapsuleRecord {
    capsule_id: String,
    archive: String,
    archive_sha256: String,
    expanded_files: Vec<CapsuleFile>,
}

pub fn verify_candidate(
    config: &Config,
    source_root: &Path,
    target: &str,
    evidence: &Path,
) -> Result<()> {
    validate_internal_paths(config, source_root, evidence)?;
    if target != config.target {
        return Err(Error::new("invariant replay target differs from appliance"));
    }
    let mut files = Vec::new();
    walk_source(source_root, source_root, &mut files)?;
    files.sort();
    if files.is_empty() || files.len() > 50_000 {
        return Err(Error::new(
            "candidate source inventory is empty or oversized",
        ));
    }
    let denied = [
        "services/astrid-edge-steward-helper/",
        "services/astrid-edge-rescue-helper/",
        "services/astrid-edge-web-broker/",
        "services/astrid-edge-provider-broker/",
        "services/astrid-edge-presentation-broker/",
        "services/astrid-edge-checkpoint/",
        "scripts/edge_self_change/",
        "scripts/edge_self_change_supervisor.py",
        "scripts/install_edge_self_evolution_root.sh",
        "packaging/systemd/astrid-edge-self-change-",
        "packaging/systemd/astrid-edge-generation-guard",
        "packaging/systemd/astrid-edge-provider-",
        "packaging/systemd/astrid-edge-presentation-broker",
        "packaging/systemd/astrid-edge-web-broker-",
        "packaging/systemd/astrid-edge-steward.",
        "capsules/spectral-bridge/",
        "minime/",
    ];
    if files
        .iter()
        .any(|path| denied.iter().any(|prefix| path.starts_with(prefix)))
    {
        return Err(Error::new(
            "candidate source contains an immutable or peer domain",
        ));
    }
    let mut tree_hash_input = Vec::new();
    for path in &files {
        let bytes = read_regular(&source_root.join(path), 16 * 1024 * 1024)?;
        tree_hash_input.extend_from_slice(path.as_bytes());
        tree_hash_input.push(0);
        tree_hash_input.extend_from_slice(sha256(&bytes).as_bytes());
        if path.starts_with("packaging/systemd/")
            && Path::new(path)
                .extension()
                .is_some_and(|extension| extension == "service" || extension == "timer")
        {
            let text = String::from_utf8_lossy(&bytes);
            validate_unit(path, &text, &config.roots.active_link)?;
        }
    }
    let replay = CandidateReplay {
        schema: "astrid.edge_rescue_helper.candidate_replay.v1",
        source_files: files.len(),
        source_tree_sha256: sha256(&tree_hash_input),
        immutable_domains_absent: true,
        unit_envelope_valid: true,
        service_template_authority: "exact_six_base_fragments:immutable_root_transactional_units:v1",
        immutable_native_installer_replay: true,
        migration_policy: "additive_or_dual_readable_only;destructive_requires_operator",
        candidate_coupled_shadow_gate: "required_after_sealed_release_build_before_package_evidence_is_accepted",
    };
    atomic_write(evidence, &canonical_json(&replay)?, 0o400, false)
}

#[allow(clippy::too_many_lines)] // One pass binds the complete sealed package to all immutable gates.
pub fn verify_package(
    config: &Config,
    bundle_root: &Path,
    source_root: &Path,
    target: &str,
    evidence: &Path,
) -> Result<()> {
    validate_capsule_policy_surface()?;
    validate_internal_paths(config, source_root, evidence)?;
    ensure_within(&config.roots.candidate_work, bundle_root, true)?;
    if target != config.target {
        return Err(Error::new("package replay target differs from appliance"));
    }
    for binary in [
        "astrid",
        "astrid-daemon",
        "astrid-build",
        "astrid-edge-runtime",
    ] {
        validate_elf(&bundle_root.join(binary), target)?;
    }
    let capsule_root = bundle_root.join("capsules");
    let mut capsule_hashes = Vec::new();
    for capsule in ESSENTIAL_CAPSULES {
        let path = capsule_root.join(format!("{capsule}.capsule"));
        let _ = capsule_archive_inventory(&path)?;
        validate_packaged_capsule_manifest_binding(
            bundle_root,
            capsule,
            &bundle_root
                .join("installed-capsules")
                .join(capsule)
                .join("Capsule.toml"),
        )?;
        capsule_hashes.extend_from_slice(capsule.as_bytes());
        capsule_hashes.push(0);
        capsule_hashes
            .extend_from_slice(sha256(&read_regular(&path, 64 * 1024 * 1024)?).as_bytes());
    }
    let capsule_entries =
        fs::read_dir(&capsule_root)?.collect::<std::result::Result<Vec<_>, _>>()?;
    let actual_capsules = capsule_entries
        .iter()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "capsule")
        })
        .count();
    if actual_capsules != ESSENTIAL_CAPSULES.len()
        || capsule_entries.len() != ESSENTIAL_CAPSULES.len()
    {
        return Err(Error::new(
            "package contains missing or unapproved capsule archives",
        ));
    }
    validate_capsule_set(bundle_root)?;
    for script in ESSENTIAL_SCRIPTS {
        let path = bundle_root.join("scripts").join(script);
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.nlink() != 1
            || metadata.mode() & 0o111 == 0
        {
            return Err(Error::new(
                "package runtime/report script layout is incomplete",
            ));
        }
        if Path::new(script)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("py"))
        {
            validate_python_script(&path)?;
        } else {
            validate_shell_script(&path)?;
        }
    }
    validate_exact_top_level(bundle_root)?;
    validate_source_snapshot_layout(bundle_root)?;
    let _ = crate::manifest::SourceSnapshot::validate_snapshot_against_existing_base(
        config,
        bundle_root,
    )?;
    validate_packaged_units(bundle_root, config)?;
    let evidence_parent = evidence
        .parent()
        .ok_or_else(|| Error::new("package replay evidence has no parent"))?;
    let installer_dry_run =
        installer_dry_run(bundle_root, &evidence_parent.join("install-dry-run"))?;
    let shadow_root = evidence_parent.join("candidate-shadow-replay");
    let candidate_shadow = crate::shadow::run(config, bundle_root, &shadow_root)?;
    if shadow_root.exists() || shadow_root.is_symlink() {
        fs::remove_dir_all(&shadow_root)?;
    }
    let replay = PackageReplay {
        schema: "astrid.edge_rescue_helper.package_replay.v1",
        target: target.to_owned(),
        binaries: 4,
        capsules: actual_capsules,
        capsule_inventory_sha256: sha256(&capsule_hashes),
        install_layout_valid: bundle_root.join("packaging/systemd").is_dir()
            && bundle_root.join("packaging/appliances").is_dir()
            && bundle_root.join("source-snapshot").is_dir(),
        installed_capsules_match_archives: true,
        runtime_scripts: ESSENTIAL_SCRIPTS.len(),
        service_template_authority: "exact_six_base_fragments:immutable_root_transactional_units:v1",
        native_install_replay: "generation_and_unit_install_exact_copy_digest_snapshot_journal_and_rollback",
        source_bodies_retained: true,
        installer_dry_run,
        candidate_shadow,
    };
    if !replay.install_layout_valid {
        return Err(Error::new("package lacks fixed install/service layout"));
    }
    atomic_write(evidence, &canonical_json(&replay)?, 0o400, false)
}

fn installer_dry_run(bundle: &Path, destination: &Path) -> Result<InstallerDryRunEvidence> {
    if destination.exists() || destination.is_symlink() {
        return Err(Error::new("installer dry-run destination already exists"));
    }
    fs::create_dir(destination)?;
    fs::set_permissions(destination, fs::Permissions::from_mode(0o700))?;
    let source = tree_inventory(bundle)?;
    copy_tree_exact(bundle, bundle, destination)?;
    let installed = tree_inventory(destination)?;
    let result = if source == installed {
        let bytes = source.iter().try_fold(0_u64, |total, entry| {
            total
                .checked_add(entry.1)
                .ok_or_else(|| Error::new("installer replay byte count overflow"))
        })?;
        let source_hash = sha256(&canonical_json(&source)?);
        let installed_hash = sha256(&canonical_json(&installed)?);
        let mut evidence = InstallerDryRunEvidence {
            provenance: "immutable_package_install_dry_run_machine_evidence_not_astrid_authorship",
            files: source.len(),
            bytes,
            source_tree_sha256: source_hash,
            installed_tree_sha256: installed_hash,
            host_paths_written: false,
            evidence_sha256: String::new(),
        };
        evidence.evidence_sha256 = sha256(&canonical_json(&evidence)?);
        Ok(evidence)
    } else {
        Err(Error::new(
            "immutable installer dry-run changed packaged bytes or modes",
        ))
    };
    fs::remove_dir_all(destination)?;
    result
}

fn tree_inventory(root: &Path) -> Result<Vec<(String, u64, u32, String)>> {
    let mut paths = Vec::new();
    walk_source(root, root, &mut paths)?;
    paths.sort();
    paths
        .into_iter()
        .map(|relative| {
            let path = root.join(&relative);
            let metadata = fs::symlink_metadata(&path)?;
            let bytes = read_regular(&path, 512 * 1024 * 1024)?;
            Ok((
                relative,
                metadata.len(),
                metadata.mode() & 0o7777,
                sha256(&bytes),
            ))
        })
        .collect()
}

fn copy_tree_exact(root: &Path, current: &Path, destination: &Path) -> Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let source = entry.path();
        let metadata = fs::symlink_metadata(&source)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::new("installer replay rejects symlinks"));
        }
        let relative = source
            .strip_prefix(root)
            .map_err(|_| Error::new("installer replay path escaped bundle"))?;
        let target = destination.join(relative);
        if metadata.is_dir() {
            fs::create_dir(&target)?;
            fs::set_permissions(
                &target,
                fs::Permissions::from_mode(metadata.mode() & 0o7777),
            )?;
            copy_tree_exact(root, &source, destination)?;
        } else if metadata.is_file() && metadata.nlink() == 1 {
            let bytes = read_regular(&source, 512 * 1024 * 1024)?;
            atomic_write(&target, &bytes, metadata.mode() & 0o7777, false)?;
        } else {
            return Err(Error::new(
                "installer replay rejects linked or special package files",
            ));
        }
    }
    Ok(())
}

fn validate_internal_paths(config: &Config, source_root: &Path, evidence: &Path) -> Result<()> {
    ensure_within(&config.roots.candidate_work, source_root, true)?;
    ensure_within(&config.roots.candidate_work, evidence, false)?;
    if source_root.file_name().and_then(|name| name.to_str()) != Some("source") {
        return Err(Error::new(
            "internal replay source root is not the confined candidate source",
        ));
    }
    Ok(())
}

/// Prove that a candidate capsule manifest does not gain any authority beyond
/// the exact manifest authenticated in its base source snapshot.  Candidate
/// code may change freely inside the separately bounded source envelope, and a
/// manifest may remove declarations, but it may not add components,
/// capabilities, interfaces, tools, IPC routes, uplinks, or interceptors.
pub(crate) fn validate_capsule_authority_update(
    path: &str,
    base_bytes: &[u8],
    candidate_bytes: &[u8],
) -> Result<()> {
    const TOP_LEVEL: &[&str] = &[
        "package",
        "component",
        "imports",
        "exports",
        "capabilities",
        "env",
        "context_file",
        "command",
        "mcp_server",
        "skill",
        "uplink",
        "interceptor",
        "topic",
    ];
    let expected_name = Path::new(path)
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .filter(|name| name.starts_with("astrid-capsule-"))
        .ok_or_else(|| Error::new("capsule manifest path is outside an exact capsule slot"))?;
    if !path.starts_with("capsules/astralis/astrid-capsule-") || !path.ends_with("/Capsule.toml") {
        return Err(Error::new(
            "capsule manifest path is outside the mutable capsule tree",
        ));
    }
    let base_text = std::str::from_utf8(base_bytes)
        .map_err(|_| Error::new("base capsule manifest is not UTF-8"))?;
    let candidate_text = std::str::from_utf8(candidate_bytes)
        .map_err(|_| Error::new("candidate capsule manifest is not UTF-8"))?;
    let base: toml::Value =
        toml::from_str(base_text).map_err(|_| Error::new("base capsule manifest is malformed"))?;
    let candidate: toml::Value = toml::from_str(candidate_text)
        .map_err(|_| Error::new("candidate capsule manifest is malformed"))?;
    let base = base
        .as_table()
        .ok_or_else(|| Error::new("base capsule manifest is not a table"))?;
    let candidate = candidate
        .as_table()
        .ok_or_else(|| Error::new("candidate capsule manifest is not a table"))?;
    if base.keys().any(|key| !TOP_LEVEL.contains(&key.as_str()))
        || candidate
            .keys()
            .any(|key| !TOP_LEVEL.contains(&key.as_str()))
    {
        return Err(Error::new(
            "capsule manifest contains an authority-unknown top-level declaration",
        ));
    }
    for table in [base, candidate] {
        let name = table
            .get("package")
            .and_then(toml::Value::as_table)
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str);
        if name != Some(expected_name) {
            return Err(Error::new(
                "capsule package identity differs from its authenticated slot",
            ));
        }
    }
    validate_components_subset(base.get("component"), candidate.get("component"))?;
    for key in [
        "imports",
        "exports",
        "capabilities",
        "env",
        "context_file",
        "command",
        "mcp_server",
        "skill",
        "uplink",
        "interceptor",
        "topic",
    ] {
        let Some(proposed) = candidate.get(key) else {
            continue;
        };
        let existing = base
            .get(key)
            .ok_or_else(|| Error::new(format!("candidate capsule adds authority section {key}")))?;
        if !authority_value_is_subset(existing, proposed) {
            return Err(Error::new(format!(
                "candidate capsule widens or changes authenticated {key} authority"
            )));
        }
    }
    Ok(())
}

fn validate_components_subset(
    base: Option<&toml::Value>,
    candidate: Option<&toml::Value>,
) -> Result<()> {
    const COMPONENT_KEYS: &[&str] = &[
        "id",
        "file",
        "entrypoint",
        "hash",
        "type",
        "link",
        "capabilities",
    ];
    let base = base
        .and_then(toml::Value::as_array)
        .ok_or_else(|| Error::new("base capsule has no component array"))?;
    let candidate = candidate
        .and_then(toml::Value::as_array)
        .ok_or_else(|| Error::new("candidate capsule has no component array"))?;
    if candidate.is_empty() || candidate.len() > base.len() {
        return Err(Error::new(
            "candidate capsule component count widens or removes execution identity",
        ));
    }
    let mut used = vec![false; base.len()];
    for proposed in candidate {
        let proposed = proposed
            .as_table()
            .ok_or_else(|| Error::new("candidate capsule component is not a table"))?;
        if proposed
            .keys()
            .any(|key| !COMPONENT_KEYS.contains(&key.as_str()))
        {
            return Err(Error::new(
                "candidate capsule component contains unknown authority metadata",
            ));
        }
        let id = proposed
            .get("id")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| Error::new("candidate capsule component has no exact ID"))?;
        let Some((index, existing)) = base.iter().enumerate().find_map(|(index, item)| {
            let table = item.as_table()?;
            (table.get("id").and_then(toml::Value::as_str) == Some(id) && !used[index])
                .then_some((index, table))
        }) else {
            return Err(Error::new("candidate capsule adds a component identity"));
        };
        used[index] = true;
        if component_entrypoint(proposed).is_none()
            || component_entrypoint(proposed) != component_entrypoint(existing)
            || proposed
                .get("type")
                .and_then(toml::Value::as_str)
                .unwrap_or_default()
                != existing
                    .get("type")
                    .and_then(toml::Value::as_str)
                    .unwrap_or_default()
        {
            return Err(Error::new(
                "candidate capsule changes a component entry point or type",
            ));
        }
        for key in ["hash", "link", "capabilities"] {
            let Some(value) = proposed.get(key) else {
                continue;
            };
            let Some(bound) = existing.get(key) else {
                return Err(Error::new(format!(
                    "candidate capsule component adds {key} authority"
                )));
            };
            if !authority_value_is_subset(bound, value) {
                return Err(Error::new(format!(
                    "candidate capsule component widens or changes {key} authority"
                )));
            }
        }
    }
    Ok(())
}

fn component_entrypoint(table: &toml::map::Map<String, toml::Value>) -> Option<&str> {
    let file = table.get("file").and_then(toml::Value::as_str);
    let alias = table.get("entrypoint").and_then(toml::Value::as_str);
    (file.is_some() ^ alias.is_some()).then_some(file.or(alias).unwrap_or_default())
}

fn authority_value_is_subset(base: &toml::Value, candidate: &toml::Value) -> bool {
    match (base, candidate) {
        (toml::Value::Boolean(existing), toml::Value::Boolean(proposed)) => !*proposed || *existing,
        (toml::Value::Array(existing), toml::Value::Array(proposed)) => {
            if proposed.len() > existing.len() {
                return false;
            }
            let mut used = vec![false; existing.len()];
            proposed.iter().all(|value| {
                let Some(index) = existing.iter().enumerate().position(|(index, bound)| {
                    !used[index] && authority_value_is_subset(bound, value)
                }) else {
                    return false;
                };
                used[index] = true;
                true
            })
        },
        (toml::Value::Table(existing), toml::Value::Table(proposed)) => {
            proposed.iter().all(|(key, value)| {
                existing
                    .get(key)
                    .is_some_and(|bound| authority_value_is_subset(bound, value))
            })
        },
        _ => base == candidate,
    }
}

fn validate_packaged_capsule_manifest_binding(
    bundle: &Path,
    capsule: &str,
    installed_manifest: &Path,
) -> Result<()> {
    let relative = format!("capsules/astralis/{capsule}/Capsule.toml");
    let source = read_regular(
        &bundle.join("source-snapshot/source").join(&relative),
        1024 * 1024,
    )?;
    let installed = read_regular(installed_manifest, 1024 * 1024)?;
    validate_capsule_authority_update(&relative, &source, &installed)?;
    if installed != source {
        return Err(Error::new(
            "packaged capsule manifest differs from authenticated candidate source",
        ));
    }
    Ok(())
}

fn validate_exact_top_level(root: &Path) -> Result<()> {
    let expected = [
        "astrid",
        "astrid-build",
        "astrid-daemon",
        "astrid-edge-runtime",
        "capsules",
        "installed-capsules",
        "packaging",
        "scripts",
        "source-snapshot",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    let actual = fs::read_dir(root)?
        .map(|entry| {
            entry.map_err(Error::from).and_then(|entry| {
                entry
                    .file_name()
                    .into_string()
                    .map_err(|_| Error::new("package top-level name is not UTF-8"))
            })
        })
        .collect::<Result<BTreeSet<_>>>()?;
    if actual != expected {
        return Err(Error::new("package top-level layout is not exact"));
    }
    Ok(())
}

fn validate_source_snapshot_layout(bundle: &Path) -> Result<()> {
    let root = bundle.join("source-snapshot");
    for relative in ["MANIFEST.json", "MANIFEST.signature.json"] {
        let metadata = fs::symlink_metadata(root.join(relative))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.nlink() != 1 {
            return Err(Error::new("package source snapshot metadata is invalid"));
        }
    }
    let source = root.join("source");
    if !source.is_dir() || fs::read_dir(source)?.next().is_none() {
        return Err(Error::new("package cumulative source snapshot is empty"));
    }
    Ok(())
}

fn validate_packaged_units(bundle: &Path, config: &Config) -> Result<()> {
    let root = bundle.join("packaging/systemd");
    let required = [
        "astrid.service",
        "astrid-model-warmup.service",
        "astrid-edge-runtime.service",
        "astrid-edge-hindsight.service",
        "astrid-edge-hindsight.timer",
        "ollama-cpu.service",
        "icp/astrid.service",
        "icp/astrid-model-warmup.service",
        "icp/astrid-edge-runtime.service",
        "icp/astrid-edge-hindsight.service",
        "icp/astrid-edge-hindsight.timer",
        "icp/ollama-cpu.service",
    ];
    for relative in required {
        let bytes = read_regular(&root.join(relative), 1024 * 1024)?;
        let text =
            std::str::from_utf8(&bytes).map_err(|_| Error::new("packaged unit is not UTF-8"))?;
        validate_unit(
            &format!("packaging/systemd/{relative}"),
            text,
            &config.roots.active_link,
        )?;
    }
    for profile in [
        "avado-i3-16g.env",
        "avado-i3-16g.edge-context.json",
        "icp-j3455-8g.env",
        "icp-j3455-8g.edge-context.json",
        "generic-cpu.env",
        "generic-cpu.edge-context.json",
    ] {
        let path = bundle.join("packaging/appliances").join(profile);
        let bytes = read_regular(&path, 1024 * 1024)?;
        if Path::new(profile).extension() == Some(std::ffi::OsStr::new("env")) {
            validate_reflection_scheduler_profile(&bytes)?;
        }
    }
    Ok(())
}

/// The dedicated two-hour steward is part of the immutable rescue authority,
/// even though the rest of an appliance profile is candidate-mutable.  Every
/// selectable generation must describe that exact authority honestly and
/// must keep the legacy in-runtime scheduler disabled.
fn validate_reflection_scheduler_profile(bytes: &[u8]) -> Result<()> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| Error::new("packaged appliance profile is not UTF-8"))?;
    let expected = BTreeMap::from([
        ("ASTRID_EDGE_SCHEDULED_INTROSPECTION_ENABLED", "false"),
        ("ASTRID_EDGE_DEDICATED_STEWARD_ENABLED", "true"),
        ("ASTRID_EDGE_DEDICATED_STEWARD_INTERVAL_MINUTES", "120"),
    ]);
    let mut observed = BTreeMap::<&str, Vec<&str>>::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(Error::new("packaged appliance profile line is malformed"));
        };
        if expected.contains_key(key) {
            observed.entry(key).or_default().push(value);
        }
    }
    if expected.iter().any(|(key, value)| {
        observed
            .get(key)
            .is_none_or(|values| values.as_slice() != [*value])
    }) {
        return Err(Error::new(
            "appliance profile may not alter the immutable reflection scheduler",
        ));
    }
    Ok(())
}

fn validate_python_script(path: &Path) -> Result<()> {
    let bytes = read_regular(path, 16 * 1024 * 1024)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| Error::new("packaged Python script is not UTF-8"))?;
    if !text.starts_with("#!/usr/bin/env python3\n")
        || text.contains('\0')
        || text.lines().count() < 5
    {
        return Err(Error::new(
            "packaged Python script failed immutable fixture checks",
        ));
    }
    Ok(())
}

fn validate_shell_script(path: &Path) -> Result<()> {
    let bytes = read_regular(path, 1024 * 1024)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| Error::new("packaged shell script is not UTF-8"))?;
    if !(text.starts_with("#!/bin/sh\n") || text.starts_with("#!/usr/bin/env bash\n"))
        || text.contains(['\0', '\r'])
        || text.contains("eval ")
    {
        return Err(Error::new(
            "packaged shell script failed immutable fixture checks",
        ));
    }
    Ok(())
}

fn walk_source(root: &Path, directory: &Path, output: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::new("candidate replay found a symlink"));
        }
        if metadata.is_dir() {
            walk_source(root, &path, output)?;
        } else if metadata.is_file() && metadata.nlink() == 1 {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| Error::new("candidate replay path escape"))?
                .to_string_lossy()
                .replace('\\', "/");
            validate_relative_signed(&relative)?;
            output.push(relative);
        } else {
            return Err(Error::new(
                "candidate replay found a linked or special file",
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // One-pass semantic parsing keeps reset/duplicate state explicit.
pub(crate) fn validate_unit(path: &str, text: &str, active_link: &Path) -> Result<()> {
    if text.contains(['\0', '\r', '\t']) || text.lines().any(|line| line.trim_end().ends_with('\\'))
    {
        return Err(Error::new(
            "unit contains binary, tab, carriage-return, or continuation syntax",
        ));
    }
    let service = path.ends_with(".service");
    let mut section = "";
    let mut exec_start = Vec::new();
    let mut sensitive = BTreeSet::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len().saturating_sub(1)];
            if !matches!(section, "Unit" | "Service" | "Timer" | "Install") {
                return Err(Error::new("unit contains an unsupported section"));
            }
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| Error::new("unit directive lacks an exact key/value separator"))?;
        if key.is_empty()
            || !key.bytes().all(|byte| byte.is_ascii_alphanumeric())
            || section.is_empty()
        {
            return Err(Error::new("unit directive key or section is malformed"));
        }
        if key.starts_with("Exec") && key != "ExecStart" {
            return Err(Error::new(
                "unit pre/post/condition/reload/stop execution is forbidden",
            ));
        }
        if key == "ReadWritePaths"
            && path.ends_with("astrid-edge-hindsight.service")
            && value == "%h/.astrid/operator/hindsight"
        {
            continue;
        }
        if matches!(
            key,
            "LoadCredential"
                | "LoadCredentialEncrypted"
                | "SetCredential"
                | "SetCredentialEncrypted"
                | "ImportCredential"
                | "PassEnvironment"
                | "UnsetEnvironment"
        ) {
            return Err(Error::new(
                "unit credential or ambient environment injection is forbidden",
            ));
        }
        if matches!(key, "User" | "Group") {
            return Err(Error::new(
                "proposal-only base templates may not select an execution identity",
            ));
        }
        if key == "ExecStart" {
            if section != "Service"
                || value.is_empty()
                || value.starts_with(['-', '+', '!', '@', ':'])
                || value.contains([';', '|', '&', '`', '\n'])
            {
                return Err(Error::new("unit ExecStart syntax or prefix is forbidden"));
            }
            exec_start.push(value.to_owned());
        }
        if matches!(key, "ExecStart" | "EnvironmentFile" | "WorkingDirectory")
            && !sensitive.insert((section.to_owned(), key.to_owned()))
        {
            return Err(Error::new(
                "unit repeats a sensitive reset-capable directive",
            ));
        }
        if key == "Environment" && !allowed_environment(path, value) {
            return Err(Error::new(
                "unit environment assignment is outside the exact template",
            ));
        }
        if key == "EnvironmentFile" && !allowed_environment_file(path, value) {
            return Err(Error::new(
                "unit EnvironmentFile is outside the exact appliance profile",
            ));
        }
        if matches!(
            key,
            "RootDirectory"
                | "RootImage"
                | "BindPaths"
                | "ReadWritePaths"
                | "AmbientCapabilities"
                | "CapabilityBoundingSet"
                | "Delegate"
        ) {
            return Err(Error::new(
                "proposal-only unit attempts to define immutable privilege boundaries",
            ));
        }
    }
    if service {
        let expected = expected_exec_start(path, active_link)?;
        if exec_start.len() != 1 || !expected.contains(&exec_start[0]) {
            return Err(Error::new(
                "unit ExecStart is not one exact reviewed appliance/generation command",
            ));
        }
    } else if !exec_start.is_empty() {
        return Err(Error::new(
            "timer unit unexpectedly contains executable content",
        ));
    }
    if is_mutable_unit_path(path) {
        validate_installable_unit_semantics(path, text)?;
    }
    Ok(())
}

/// Return whether `path` is one of the exact AVADO/ICP base fragments which
/// may pass from an authored patch into the immutable root installer.
#[must_use]
pub(crate) fn is_mutable_unit_path(path: &str) -> bool {
    let Some(relative) = path.strip_prefix("packaging/systemd/") else {
        return false;
    };
    let name = relative.strip_prefix("icp/").unwrap_or(relative);
    !name.contains('/') && MUTABLE_UNIT_FRAGMENTS.contains(&name)
}

/// Validate and normalize one candidate base fragment for the system manager.
/// The bootstrap migration changes only `default.target` to
/// `multi-user.target`; reproducing that deterministic translation prevents a
/// candidate from gaining an enablement or install-target side channel.
pub(crate) fn normalized_system_unit(
    path: &str,
    text: &str,
    active_link: &Path,
) -> Result<Vec<u8>> {
    if !is_mutable_unit_path(path) {
        return Err(Error::new("unit is outside the exact mutable fragment set"));
    }
    validate_unit(path, text, active_link)?;
    let timer = Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("timer"));
    let mut replacements = 0_usize;
    let mut output = String::with_capacity(text.len().saturating_add(16));
    for line in text.lines() {
        if !timer && line.trim() == "WantedBy=default.target" {
            output.push_str("WantedBy=multi-user.target\n");
            replacements = replacements.saturating_add(1);
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    if (!timer && replacements != 1) || (timer && replacements != 0) {
        return Err(Error::new("unit install-target normalization is not exact"));
    }
    validate_unit(path, &output, active_link)?;
    Ok(output.into_bytes())
}

#[allow(clippy::too_many_lines)]
fn validate_installable_unit_semantics(path: &str, text: &str) -> Result<()> {
    let timer = Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("timer"));
    let mut section = "";
    let mut sections = BTreeSet::new();
    let mut singleton = BTreeSet::new();
    let mut exec_start = 0_usize;
    let mut wanted_by = 0_usize;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len().saturating_sub(1)];
            sections.insert(section.to_owned());
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| Error::new("unit directive lacks key/value syntax"))?;
        let repeatable = section == "Service" && key == "Environment"
            || section == "Unit"
                && matches!(key, "Wants" | "Requires" | "BindsTo" | "After" | "Before");
        if !repeatable && !singleton.insert((section.to_owned(), key.to_owned())) {
            return Err(Error::new("installable unit repeats a singleton directive"));
        }
        let valid = match (section, key) {
            ("Unit", "Description") => safe_description(value),
            ("Unit", "Documentation") => matches!(
                value,
                "https://github.com/unicity-astrid/astrid"
                    | "https://docs.ollama.com/api"
                    | "https://docs.ollama.com/linux"
            ),
            ("Unit", "Wants" | "Requires" | "BindsTo" | "After" | "Before") => {
                allowed_unit_dependencies(value)
            },
            ("Unit", "ConditionPathExists") => value == "%h/.config/astrid/edge-appliance.env",
            ("Unit", "ConditionPathIsDirectory") => value == "%h/.astrid/home/default/edge",
            ("Unit", "StartLimitIntervalSec") => bounded_integer(value, 10, 600),
            ("Unit", "StartLimitBurst") => bounded_integer(value, 1, 20),
            ("Service", "Type") => matches!(value, "simple" | "oneshot"),
            ("Service", "WorkingDirectory") => value == "%h",
            ("Service", "Environment") => allowed_environment(path, value),
            ("Service", "EnvironmentFile") => allowed_environment_file(path, value),
            ("Service", "ExecStart") => {
                exec_start = exec_start.saturating_add(1);
                true
            },
            ("Service", "Restart") => value == "on-failure",
            ("Service", "RestartSec") => bounded_duration(value, 1, 60),
            ("Service", "TimeoutStartSec" | "TimeoutStopSec") => bounded_duration(value, 10, 900),
            ("Service", "KillSignal") => value == "SIGTERM",
            ("Service", "UMask") => value == "0077",
            ("Service", "LimitNOFILE") => bounded_integer(value, 1_024, 65_536),
            ("Service", "TasksMax") => bounded_integer(value, 64, 1_024),
            ("Service", "RemainAfterExit" | "NoNewPrivileges" | "PrivateTmp") => value == "yes",
            ("Service", "Nice") => bounded_integer(value, 0, 19),
            ("Service", "IOSchedulingClass") => value == "idle",
            ("Service", "ProtectSystem") => value == "strict",
            ("Service", "ProtectHome") => value == "read-only",
            ("Service", "ReadWritePaths") => {
                path.ends_with("astrid-edge-hindsight.service")
                    && value == "%h/.astrid/operator/hindsight"
            },
            ("Timer", "OnBootSec") => bounded_duration(value, 30, 1_800),
            ("Timer", "OnUnitActiveSec") => bounded_duration(value, 300, 86_400),
            ("Timer", "RandomizedDelaySec") => bounded_duration(value, 0, 300),
            ("Timer", "AccuracySec") => bounded_duration(value, 1, 300),
            ("Timer", "Persistent") => value == "true",
            ("Timer", "Unit") => value == "astrid-edge-hindsight.service",
            ("Install", "WantedBy") => {
                wanted_by = wanted_by.saturating_add(1);
                if timer {
                    value == "timers.target"
                } else {
                    matches!(value, "default.target" | "multi-user.target")
                }
            },
            _ => false,
        };
        if !valid {
            return Err(Error::new(format!(
                "installable unit directive is outside policy: {section}.{key}"
            )));
        }
    }
    let required = if timer {
        BTreeSet::from(["Unit".to_owned(), "Timer".to_owned(), "Install".to_owned()])
    } else {
        BTreeSet::from([
            "Unit".to_owned(),
            "Service".to_owned(),
            "Install".to_owned(),
        ])
    };
    if sections != required
        || wanted_by != 1
        || (timer && exec_start != 0)
        || (!timer && exec_start != 1)
    {
        return Err(Error::new(
            "installable unit section or terminal directive set is incomplete",
        ));
    }
    Ok(())
}

fn safe_description(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() || byte == b' ')
}

fn allowed_unit_dependencies(value: &str) -> bool {
    let allowed = [
        "network-online.target",
        "ollama-cpu.service",
        "astrid-model-warmup.service",
        "astrid.service",
        "astrid-edge-runtime.service",
        "astrid-edge-hindsight.service",
        "astrid-edge-hindsight.timer",
    ];
    let values = value.split_ascii_whitespace().collect::<Vec<_>>();
    !values.is_empty()
        && values.len() <= allowed.len()
        && values.iter().all(|item| allowed.contains(item))
}

fn bounded_integer(value: &str, minimum: u64, maximum: u64) -> bool {
    value
        .parse::<u64>()
        .is_ok_and(|parsed| (minimum..=maximum).contains(&parsed))
}

fn bounded_duration(value: &str, minimum_seconds: u64, maximum_seconds: u64) -> bool {
    let (number, multiplier) = if let Some(number) = value.strip_suffix("min") {
        (number, 60_u64)
    } else if let Some(number) = value.strip_suffix('h') {
        (number, 3_600_u64)
    } else if let Some(number) = value.strip_suffix('s') {
        (number, 1_u64)
    } else {
        (value, 1_u64)
    };
    number
        .parse::<u64>()
        .ok()
        .and_then(|number| number.checked_mul(multiplier))
        .is_some_and(|seconds| (minimum_seconds..=maximum_seconds).contains(&seconds))
}

fn expected_exec_start(path: &str, active_link: &Path) -> Result<BTreeSet<String>> {
    let name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::new("unit source path has no UTF-8 basename"))?;
    let icp = path.contains("/icp/");
    let mut result = BTreeSet::new();
    match (name, icp) {
        ("astrid.service", false) => {
            result.insert("%h/.astrid/bin/astrid-daemon --workspace %h".to_owned());
            result.insert(format!(
                "{} --workspace %h",
                active_link.join("astrid-daemon").display()
            ));
        },
        ("astrid.service", true) => {
            result.insert(
                "%h/.astrid-icp/state/bin/astrid-daemon --workspace %h/.astrid-icp/workspace"
                    .to_owned(),
            );
            result.insert(format!(
                "{} --workspace %h/.astrid-icp/workspace",
                active_link.join("astrid-daemon").display()
            ));
        },
        ("astrid-edge-runtime.service", false) => {
            result.insert("%h/.astrid/bin/astrid-edge-runtime".to_owned());
            result.insert(
                active_link
                    .join("astrid-edge-runtime")
                    .display()
                    .to_string(),
            );
        },
        ("astrid-edge-runtime.service", true) => {
            result.insert("%h/.astrid-icp/state/bin/astrid-edge-runtime".to_owned());
            result.insert(
                active_link
                    .join("astrid-edge-runtime")
                    .display()
                    .to_string(),
            );
        },
        ("astrid-model-warmup.service", false) => {
            result.insert("%h/.astrid/bin/warm-ollama-model".to_owned());
            result.insert(
                active_link
                    .join("scripts/warm_ollama_model.sh")
                    .display()
                    .to_string(),
            );
        },
        ("astrid-model-warmup.service", true) => {
            result.insert("%h/.astrid-icp/state/bin/warm-ollama-model".to_owned());
            result.insert(
                active_link
                    .join("scripts/warm_ollama_model.sh")
                    .display()
                    .to_string(),
            );
        },
        ("astrid-edge-hindsight.service", false) => {
            result.insert(
                "%h/.astrid/bin/edge-hindsight record --workspace %h/.astrid/home/default/edge --state-root %h/.astrid"
                    .to_owned(),
            );
            result.insert(format!(
                "{} record --workspace %h/.astrid/home/default/edge --state-root %h/.astrid",
                active_link.join("scripts/edge_hindsight.py").display()
            ));
        },
        ("astrid-edge-hindsight.service", true) => {
            result.insert(
                "%h/.astrid-icp/state/bin/edge-hindsight record --workspace %h/.astrid-icp/state/home/default/edge --state-root %h/.astrid-icp/state"
                    .to_owned(),
            );
            result.insert(format!(
                "{} record --workspace %h/.astrid-icp/state/home/default/edge --state-root %h/.astrid-icp/state",
                active_link.join("scripts/edge_hindsight.py").display()
            ));
        },
        ("ollama-cpu.service", false) => {
            result.insert("%h/.local/bin/ollama serve".to_owned());
        },
        ("ollama-cpu.service", true) => {
            result.insert("%h/.astrid-icp/ollama/runtime/bin/ollama serve".to_owned());
        },
        _ => {
            return Err(Error::new(
                "unrecognized proposal-only Astrid service template",
            ));
        },
    }
    Ok(result)
}

fn allowed_environment_file(path: &str, value: &str) -> bool {
    let expected = "%h/.config/astrid/edge-appliance.env";
    (value == expected || value == format!("-{expected}"))
        && matches!(
            Path::new(path).file_name().and_then(|name| name.to_str()),
            Some("astrid.service" | "astrid-model-warmup.service" | "astrid-edge-runtime.service")
        )
}

fn allowed_environment(path: &str, value: &str) -> bool {
    let icp = path.contains("/icp/");
    let common = [
        "RUST_BACKTRACE=1",
        "OLLAMA_HOST=127.0.0.1:11434",
        "OLLAMA_NUM_PARALLEL=1",
        "OLLAMA_MAX_LOADED_MODELS=1",
        "OLLAMA_KEEP_ALIVE=2h",
    ];
    common.contains(&value)
        || (!icp
            && matches!(
                value,
                "ASTRID_HOME=%h/.astrid"
                    | "PATH=%h/.astrid/bin:%h/.local/bin:/usr/local/bin:/usr/bin:/bin"
                    | "OLLAMA_MODELS=%h/.local/share/ollama/models"
                    | "OLLAMA_CONTEXT_LENGTH=4096"
            ))
        || (icp
            && matches!(
                value,
                "ASTRID_HOME=%h/.astrid-icp/state"
                    | "TMPDIR=%h/.astrid-icp/tmp"
                    | "PATH=%h/.astrid-icp/state/bin:%h/.astrid-icp/ollama/runtime/bin:/usr/local/bin:/usr/bin:/bin"
                    | "TOKIO_WORKER_THREADS=4"
                    | "ASTRID_LOCAL_HTTP_ALLOWLIST=astrid-capsule-openai-compat@127.0.0.1:11434"
                    | "OLLAMA_MODELS=%h/.astrid-icp/ollama/models"
                    | "OLLAMA_CONTEXT_LENGTH=2048"
                    | "OLLAMA_LLM_LIBRARY=sse42"
            ))
}

fn validate_elf(path: &Path, target: &str) -> Result<()> {
    let bytes = read_regular(path, 512 * 1024 * 1024)?;
    if bytes.len() < 20 || &bytes[..4] != b"\x7fELF" || bytes[4] != 2 || bytes[5] != 1 {
        return Err(Error::new("package binary is not little-endian ELF64"));
    }
    let machine = u16::from_le_bytes([bytes[18], bytes[19]]);
    let expected = if target.starts_with("x86_64") {
        62
    } else {
        183
    };
    if machine != expected {
        return Err(Error::new("package ELF architecture differs from target"));
    }
    Ok(())
}

pub(crate) fn validate_release_architecture(release: &Path, target: &str) -> Result<()> {
    for binary in [
        "astrid",
        "astrid-daemon",
        "astrid-build",
        "astrid-edge-runtime",
    ] {
        validate_elf(&release.join(binary), target)?;
    }
    Ok(())
}

fn capsule_archive_inventory(path: &Path) -> Result<Vec<CapsuleFile>> {
    let bytes = read_regular(path, 64 * 1024 * 1024)?;
    let mut archive = tar::Archive::new(GzDecoder::new(bytes.as_slice()));
    let mut names = BTreeSet::new();
    let mut files = Vec::new();
    let mut component_wasm = 0_usize;
    let mut total = 0_u64;
    for entry in archive
        .entries()
        .map_err(|_| Error::new("capsule archive is malformed"))?
    {
        let entry = entry.map_err(|_| Error::new("capsule archive entry is malformed"))?;
        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_dir()) {
            return Err(Error::new(
                "capsule archive contains a link or special entry",
            ));
        }
        let path = entry
            .path()
            .map_err(|_| Error::new("capsule archive path is malformed"))?;
        if path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(Error::new("capsule archive path escapes its root"));
        }
        let name = path.to_string_lossy().replace('\\', "/");
        if !names.insert(name.clone()) {
            return Err(Error::new("capsule archive contains duplicate entries"));
        }
        if kind.is_dir() {
            continue;
        }
        let mut sink = Vec::new();
        entry.take(16 * 1024 * 1024 + 1).read_to_end(&mut sink)?;
        if sink.len() > 16 * 1024 * 1024 {
            return Err(Error::new("capsule archive entry is oversized"));
        }
        total = total
            .checked_add(u64::try_from(sink.len()).unwrap_or(u64::MAX))
            .ok_or_else(|| Error::new("capsule archive total size overflow"))?;
        if total > 64 * 1024 * 1024 || files.len() >= 256 {
            return Err(Error::new("capsule archive inventory exceeds bounds"));
        }
        if Path::new(&name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("wasm"))
        {
            if sink.len() < 8 || &sink[..8] != b"\0asm\x0d\0\x01\0" {
                return Err(Error::new(
                    "capsule archive WASM is not a Component Model binary",
                ));
            }
            component_wasm = component_wasm.saturating_add(1);
        }
        files.push(CapsuleFile {
            path: name,
            size: u64::try_from(sink.len()).unwrap_or(u64::MAX),
            sha256: sha256(&sink),
        });
    }
    if !names.contains("Capsule.toml") || component_wasm != 1 {
        return Err(Error::new(
            "capsule archive lacks an exact single WASM component and manifest",
        ));
    }
    files.sort();
    Ok(files)
}

/// Finalize the unprivileged candidate-CLI fixture into a deterministic installed tree. Every
/// archive member is replayed byte-for-byte, CLI metadata is normalized, and an immutable exact
/// archive/expanded inventory is written before the tree enters the candidate bundle.
pub(crate) fn finalize_installed_capsules(bundle: &Path, fixture_root: &Path) -> Result<()> {
    validate_capsule_policy_surface()?;
    let fixture_metadata = fs::symlink_metadata(fixture_root)?;
    if !fixture_metadata.is_dir() || fixture_metadata.file_type().is_symlink() {
        return Err(Error::new(
            "candidate capsule fixture root is absent or linked",
        ));
    }
    let directories = fs::read_dir(fixture_root)?.collect::<std::result::Result<Vec<_>, _>>()?;
    if directories.len() != ESSENTIAL_CAPSULES.len()
        || directories.iter().any(|entry| {
            !entry.path().is_dir()
                || !ESSENTIAL_CAPSULES.contains(&entry.file_name().to_string_lossy().as_ref())
        })
    {
        return Err(Error::new(
            "candidate capsule fixture does not contain the exact twenty capsule directories",
        ));
    }
    let mut records = Vec::new();
    for capsule in ESSENTIAL_CAPSULES {
        let archive_relative = format!("capsules/{capsule}.capsule");
        let archive_path = bundle.join(&archive_relative);
        let target = fixture_root.join(capsule);
        let _ = collect_expanded_capsule_inventory(&target, false)?;
        let component_blake3 = replay_archive_members(&archive_path, &target).map_err(|error| {
            Error::new(format!(
                "candidate capsule archive replay failed for {capsule}: {error}"
            ))
        })?;
        validate_packaged_capsule_manifest_binding(bundle, capsule, &target.join("Capsule.toml"))?;
        normalize_capsule_meta(
            &target.join("meta.json"),
            &target.join("Capsule.toml"),
            capsule,
            &component_blake3,
        )
        .map_err(|error| {
            Error::new(format!(
                "candidate capsule metadata normalization failed for {capsule}: {error}"
            ))
        })?;
        let expanded_files = collect_expanded_capsule_inventory(&target, false)?;
        records.push(InstalledCapsuleRecord {
            capsule_id: (*capsule).to_owned(),
            archive: archive_relative,
            archive_sha256: sha256(&read_regular(&archive_path, 64 * 1024 * 1024)?),
            expanded_files,
        });
    }
    let set = InstalledCapsuleSet {
        schema: "astrid.edge.installed_capsules.v1".to_owned(),
        authority: "deterministic_expansion_of_candidate_archive_fixtures".to_owned(),
        capsules: records,
    };
    let bytes = [canonical_json(&set)?, b"\n".to_vec()].concat();
    atomic_write(&fixture_root.join("CAPSULES.json"), &bytes, 0o444, false)?;
    let destination = bundle.join("installed-capsules");
    if destination.exists() || destination.is_symlink() {
        return Err(Error::new(
            "candidate bundle installed capsule target already exists",
        ));
    }
    fs::rename(fixture_root, &destination)
        .map_err(|error| Error::new(format!("candidate capsule publish failed: {error}")))?;
    // macOS requires write permission on a directory being renamed, while Linux ordinarily only
    // checks its parents. Publish first and seal immediately; no candidate process runs across
    // this root-owned finalization interval.
    make_expansion_read_only(&destination).map_err(|error| {
        Error::new(format!(
            "candidate capsule expansion sealing failed: {error}"
        ))
    })?;
    validate_capsule_set(bundle)
}

fn replay_archive_members(archive_path: &Path, target: &Path) -> Result<String> {
    let expected = capsule_archive_inventory(archive_path)?;
    let bytes = read_regular(archive_path, 64 * 1024 * 1024)?;
    let mut archive = tar::Archive::new(GzDecoder::new(bytes.as_slice()));
    let mut component_blake3 = None;
    for item in archive
        .entries()
        .map_err(|_| Error::new("capsule archive is malformed"))?
    {
        let item = item.map_err(|_| Error::new("capsule archive entry is malformed"))?;
        if !item.header().entry_type().is_file() {
            continue;
        }
        let path = item
            .path()
            .map_err(|_| Error::new("capsule archive path is malformed"))?;
        let relative = path.to_string_lossy().replace('\\', "/");
        let output = target.join(validate_relative_signed(&relative)?);
        let mut content = Vec::new();
        item.take(16 * 1024 * 1024 + 1).read_to_end(&mut content)?;
        if content.len() > 16 * 1024 * 1024 {
            return Err(Error::new("capsule archive entry is oversized"));
        }
        if Path::new(&relative)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("wasm"))
            && component_blake3
                .replace(blake3::hash(&content).to_hex().to_string())
                .is_some()
        {
            return Err(Error::new(
                "capsule archive contains multiple Component payloads",
            ));
        }
        if output.exists() || output.is_symlink() {
            if read_regular(&output, 16 * 1024 * 1024)? != content {
                return Err(Error::new(
                    "candidate capsule fixture changed an archive-owned file",
                ));
            }
            continue;
        }
        if let Some(parent) = output.parent() {
            ensure_regular_directories(target, parent)?;
        }
        atomic_write(&output, &content, 0o444, false)?;
    }
    let actual = collect_expanded_capsule_inventory(target, false)?;
    if expected.iter().any(|item| !actual.contains(item)) {
        return Err(Error::new(
            "candidate installed capsule omits an exact archive member",
        ));
    }
    component_blake3.ok_or_else(|| Error::new("capsule archive has no Component identity"))
}

fn ensure_regular_directories(root: &Path, destination: &Path) -> Result<()> {
    let relative = destination
        .strip_prefix(root)
        .map_err(|_| Error::new("capsule expansion parent escapes capsule root"))?;
    let mut cursor = root.to_path_buf();
    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(Error::new("capsule expansion parent is noncanonical"));
        }
        cursor.push(component);
        if cursor.exists() {
            let metadata = fs::symlink_metadata(&cursor)?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(Error::new(
                    "capsule expansion parent is linked or non-directory",
                ));
            }
        } else {
            fs::create_dir(&cursor)?;
        }
    }
    Ok(())
}

fn normalize_capsule_meta(
    path: &Path,
    manifest_path: &Path,
    expected_capsule_id: &str,
    expected_component_blake3: &str,
) -> Result<()> {
    let manifest_text = String::from_utf8(read_regular(manifest_path, 1024 * 1024)?)
        .map_err(|_| Error::new("candidate capsule manifest is not UTF-8"))?;
    let manifest = manifest_text
        .parse::<toml::Value>()
        .map_err(|_| Error::new("candidate capsule manifest is malformed"))?;
    let package = manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| Error::new("candidate capsule manifest lacks package identity"))?;
    let manifest_name = package
        .get("name")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| Error::new("candidate capsule manifest lacks package name"))?;
    let manifest_version = package
        .get("version")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| Error::new("candidate capsule manifest lacks package version"))?;
    if manifest_name != expected_capsule_id
        || manifest_version.is_empty()
        || manifest_version.len() > 128
    {
        return Err(Error::new(
            "candidate capsule manifest identity differs from exact package slot",
        ));
    }
    let mut value: Value = serde_json::from_slice(&read_regular(path, 1024 * 1024)?)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| Error::new("candidate capsule meta.json is not an object"))?;
    let allowed = [
        "version",
        "installed_at",
        "updated_at",
        "source",
        "imports",
        "exports",
        "topics",
        "wasm_hash",
        "wit_files",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if object.keys().any(|key| !allowed.contains(key.as_str()))
        || object.get("version").and_then(Value::as_str) != Some(manifest_version)
        || object
            .get("imports")
            .is_some_and(|value| !value.is_object())
        || object
            .get("exports")
            .is_some_and(|value| !value.is_object())
        || object.get("topics").is_some_and(|value| !value.is_array())
        || object
            .get("wit_files")
            .is_some_and(|value| !value.is_object())
    {
        return Err(Error::new(
            "candidate capsule meta.json has an unsupported shape",
        ));
    }
    let wasm_hash = object
        .get("wasm_hash")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("candidate capsule meta.json lacks WASM identity"))?;
    if wasm_hash.len() != 64
        || !wasm_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || wasm_hash != expected_component_blake3
    {
        return Err(Error::new(
            "candidate capsule WASM identity is malformed or differs from exact archive BLAKE3",
        ));
    }
    object.insert(
        "installed_at".to_owned(),
        Value::String("1970-01-01T00:00:00+00:00".to_owned()),
    );
    object.insert(
        "updated_at".to_owned(),
        Value::String("1970-01-01T00:00:00+00:00".to_owned()),
    );
    object.remove("source");
    let bytes = [canonical_json(&value)?, b"\n".to_vec()].concat();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    atomic_write(path, &bytes, 0o444, true)
}

fn collect_expanded_capsule_inventory(root: &Path, read_only: bool) -> Result<Vec<CapsuleFile>> {
    fn visit(
        root: &Path,
        directory: &Path,
        read_only: bool,
        output: &mut Vec<CapsuleFile>,
    ) -> Result<()> {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() || (read_only && metadata.mode() & 0o222 != 0) {
                return Err(Error::new(
                    "installed capsule contains linked or mutable content",
                ));
            }
            if metadata.is_dir() {
                visit(root, &path, read_only, output)?;
            } else if metadata.is_file() && metadata.nlink() == 1 {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|_| Error::new("installed capsule path escape"))?
                    .to_string_lossy()
                    .replace('\\', "/");
                validate_relative_signed(&relative)?;
                let bytes = read_regular(&path, 16 * 1024 * 1024)?;
                output.push(CapsuleFile {
                    path: relative,
                    size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                    sha256: sha256(&bytes),
                });
            } else {
                return Err(Error::new(
                    "installed capsule contains linked or special content",
                ));
            }
        }
        Ok(())
    }
    let metadata = fs::symlink_metadata(root)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || (read_only && metadata.mode() & 0o222 != 0)
    {
        return Err(Error::new("installed capsule root is linked or mutable"));
    }
    let mut files = Vec::new();
    visit(root, root, read_only, &mut files)?;
    files.sort();
    if files.is_empty() || files.len() > 256 {
        return Err(Error::new(
            "installed capsule inventory is empty or oversized",
        ));
    }
    Ok(files)
}

fn make_expansion_read_only(root: &Path) -> Result<()> {
    fn visit(path: &Path) -> Result<()> {
        for entry in fs::read_dir(path)? {
            let child = entry?.path();
            let metadata = fs::symlink_metadata(&child)?;
            if metadata.file_type().is_symlink() {
                return Err(Error::new("capsule expansion contains a symlink"));
            }
            if metadata.is_dir() {
                visit(&child)?;
                std::os::unix::fs::chown(
                    &child,
                    Some(nix::unistd::geteuid().as_raw()),
                    Some(nix::unistd::getegid().as_raw()),
                )?;
                fs::set_permissions(&child, fs::Permissions::from_mode(0o555))?;
            } else if metadata.is_file() && metadata.nlink() == 1 {
                std::os::unix::fs::chown(
                    &child,
                    Some(nix::unistd::geteuid().as_raw()),
                    Some(nix::unistd::getegid().as_raw()),
                )?;
                fs::set_permissions(&child, fs::Permissions::from_mode(0o444))?;
            } else {
                return Err(Error::new("capsule expansion contains special content"));
            }
        }
        Ok(())
    }
    visit(root)?;
    std::os::unix::fs::chown(
        root,
        Some(nix::unistd::geteuid().as_raw()),
        Some(nix::unistd::getegid().as_raw()),
    )?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o555))?;
    Ok(())
}

pub(crate) fn validate_installed_capsule_set(release: &Path) -> Result<()> {
    validate_capsule_set(release)
}

fn validate_capsule_set(release: &Path) -> Result<()> {
    let archive_root = release.join("capsules");
    let installed_root = release.join("installed-capsules");
    let archive_entries =
        fs::read_dir(&archive_root)?.collect::<std::result::Result<Vec<_>, _>>()?;
    if archive_entries.len() != ESSENTIAL_CAPSULES.len() {
        return Err(Error::new(
            "release does not contain the exact twenty capsule archives",
        ));
    }
    let value: Value = serde_json::from_slice(&read_regular(
        &installed_root.join("CAPSULES.json"),
        4 * 1024 * 1024,
    )?)?;
    let set: InstalledCapsuleSet = serde_json::from_value(value.clone())?;
    if canonical_json(&set)? != canonical_json(&value)?
        || set.schema != "astrid.edge.installed_capsules.v1"
        || !matches!(
            set.authority.as_str(),
            "deterministic_expansion_of_operator_packaged_archives"
                | "deterministic_expansion_of_candidate_archive_fixtures"
        )
        || set.capsules.len() != ESSENTIAL_CAPSULES.len()
    {
        return Err(Error::new("installed capsule set manifest is invalid"));
    }
    let entries = fs::read_dir(&installed_root)?.collect::<std::result::Result<Vec<_>, _>>()?;
    if entries.len() != ESSENTIAL_CAPSULES.len().saturating_add(1) {
        return Err(Error::new("installed capsule root membership is not exact"));
    }
    for (index, capsule) in ESSENTIAL_CAPSULES.iter().enumerate() {
        let record = set
            .capsules
            .get(index)
            .ok_or_else(|| Error::new("installed capsule set record is missing"))?;
        let archive_relative = format!("capsules/{capsule}.capsule");
        let archive_path = release.join(&archive_relative);
        let archive_inventory = capsule_archive_inventory(&archive_path)?;
        let expanded = collect_expanded_capsule_inventory(&installed_root.join(capsule), true)?;
        let meta = expanded
            .iter()
            .find(|file| file.path == "meta.json")
            .ok_or_else(|| Error::new("installed capsule lacks normalized metadata"))?;
        let mut exact_expansion = archive_inventory.clone();
        exact_expansion.push(meta.clone());
        exact_expansion.sort();
        if record.capsule_id != *capsule
            || record.archive != archive_relative
            || record.archive_sha256 != sha256(&read_regular(&archive_path, 64 * 1024 * 1024)?)
            || record.expanded_files != expanded
            || expanded != exact_expansion
        {
            return Err(Error::new(
                "installed capsule identity does not match archive and expanded inventory",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::{
        ESSENTIAL_CAPSULES, ESSENTIAL_SCRIPTS, EXTERNAL_SOURCE_CAPSULES, capsule_archive_inventory,
        finalize_installed_capsules, installer_dry_run, validate_capsule_authority_update,
        validate_capsule_set, validate_elf, validate_reflection_scheduler_profile, validate_unit,
    };

    fn append_archive_file(
        archive: &mut tar::Builder<GzEncoder<fs::File>>,
        path: &str,
        bytes: &[u8],
    ) {
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o644);
        header.set_size(u64::try_from(bytes.len()).unwrap());
        header.set_cksum();
        archive.append_data(&mut header, path, bytes).unwrap();
    }

    #[test]
    fn immutable_installer_dry_run_copies_exactly_and_rejects_links() {
        let temporary = tempfile::tempdir().unwrap();
        let bundle = temporary.path().join("bundle");
        fs::create_dir(&bundle).unwrap();
        fs::write(bundle.join("artifact"), b"exact").unwrap();
        let evidence = installer_dry_run(&bundle, &temporary.path().join("install")).unwrap();
        assert_eq!(evidence.source_tree_sha256, evidence.installed_tree_sha256);
        assert!(!evidence.host_paths_written);
        std::os::unix::fs::symlink("artifact", bundle.join("escape")).unwrap();
        assert!(installer_dry_run(&bundle, &temporary.path().join("linked-install")).is_err());
    }

    fn write_capsule_archive_with_manifest(path: &Path, manifest: &str, extra_component: bool) {
        let file = fs::File::create(path).unwrap();
        let encoder = GzEncoder::new(file, Compression::default());
        let mut archive = tar::Builder::new(encoder);
        append_archive_file(&mut archive, "Capsule.toml", manifest.as_bytes());
        append_archive_file(&mut archive, "component.wasm", b"\0asm\x0d\0\x01\0");
        if extra_component {
            append_archive_file(&mut archive, "second.wasm", b"\0asm\x0d\0\x01\0");
        }
        archive.finish().unwrap();
    }

    fn write_capsule_archive(path: &Path, capsule: &str, extra_component: bool) {
        let manifest = format!(
            "[package]\nname = \"{capsule}\"\nversion = \"0.1.0\"\n\n[[component]]\nid = \"main\"\nfile = \"component.wasm\"\ntype = \"executable\"\n"
        );
        write_capsule_archive_with_manifest(path, &manifest, extra_component);
    }

    fn create_capsule_fixture(root: &Path) -> (PathBuf, PathBuf) {
        let bundle = root.join("bundle");
        let archives = bundle.join("capsules");
        let source = bundle.join("source-snapshot/source/capsules/astralis");
        let fixtures = root.join("fixtures");
        fs::create_dir_all(&archives).unwrap();
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&fixtures).unwrap();
        for capsule in ESSENTIAL_CAPSULES {
            let manifest = format!(
                "[package]\nname = \"{capsule}\"\nversion = \"0.1.0\"\n\n[[component]]\nid = \"main\"\nfile = \"component.wasm\"\ntype = \"executable\"\n"
            );
            write_capsule_archive(&archives.join(format!("{capsule}.capsule")), capsule, false);
            let source_capsule = source.join(capsule);
            fs::create_dir(&source_capsule).unwrap();
            fs::write(source_capsule.join("Capsule.toml"), &manifest).unwrap();
            let installed = fixtures.join(capsule);
            fs::create_dir(&installed).unwrap();
            fs::write(installed.join("Capsule.toml"), manifest).unwrap();
            fs::write(
                installed.join("meta.json"),
                format!(
                    "{{\"version\":\"0.1.0\",\"installed_at\":\"later\",\"updated_at\":\"later\",\"source\":\"untrusted\",\"wasm_hash\":\"{}\"}}\n",
                    blake3::hash(b"\0asm\x0d\0\x01\0").to_hex()
                ),
            )
            .unwrap();
        }
        (bundle, fixtures)
    }

    #[test]
    fn unit_escape_is_rejected() {
        let active = Path::new("/opt/astrid-edge/current");
        assert!(
            validate_unit(
                "packaging/systemd/astrid.service",
                "[Service]\nExecStartPre=+/usr/bin/sh -c nope\nExecStart=/opt/astrid-edge/current/astrid-daemon --workspace %h\n",
                active,
            )
            .is_err()
        );
        let valid = include_str!("../../../packaging/systemd/astrid.service").replace(
            "%h/.astrid/bin/astrid-daemon --workspace %h",
            "/opt/astrid-edge/current/astrid-daemon --workspace %h",
        );
        assert!(validate_unit("packaging/systemd/astrid.service", &valid, active).is_ok());
        assert!(
            validate_unit(
                "packaging/systemd/astrid.service",
                "[Service]\nLoadCredential=x:/etc/shadow\nExecStart=/opt/astrid-edge/current/astrid-daemon --workspace %h\n",
                active,
            )
            .is_err()
        );
        for forbidden in [
            "ExecStartPre=/usr/bin/true",
            "ExecStartPost=/usr/bin/true",
            "ExecStart=+/opt/astrid-edge/current/astrid-daemon --workspace %h",
            "ExecStart=/usr/bin/env sh -c /opt/astrid-edge/current/astrid-daemon",
            "EnvironmentFile=",
            "PassEnvironment=LD_PRELOAD",
            "User=root",
            "BindPaths=/:/host",
        ] {
            let unit = format!(
                "[Service]\n{forbidden}\nExecStart=/opt/astrid-edge/current/astrid-daemon --workspace %h\n"
            );
            assert!(
                validate_unit("packaging/systemd/astrid.service", &unit, active).is_err(),
                "forbidden directive passed semantic validation: {forbidden}"
            );
        }
    }

    #[test]
    fn checked_in_avado_and_icp_templates_match_exact_semantic_contract() {
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let active = Path::new("/opt/astrid-edge/current");
        for relative in [
            "packaging/systemd/astrid.service",
            "packaging/systemd/astrid-model-warmup.service",
            "packaging/systemd/astrid-edge-runtime.service",
            "packaging/systemd/astrid-edge-hindsight.service",
            "packaging/systemd/astrid-edge-hindsight.timer",
            "packaging/systemd/ollama-cpu.service",
            "packaging/systemd/icp/astrid.service",
            "packaging/systemd/icp/astrid-model-warmup.service",
            "packaging/systemd/icp/astrid-edge-runtime.service",
            "packaging/systemd/icp/astrid-edge-hindsight.service",
            "packaging/systemd/icp/astrid-edge-hindsight.timer",
            "packaging/systemd/icp/ollama-cpu.service",
        ] {
            let text = std::fs::read_to_string(repository.join(relative)).unwrap();
            validate_unit(relative, &text, active)
                .unwrap_or_else(|error| panic!("{relative}: {error}"));
        }
        for relative in [
            "packaging/appliances/avado-i3-16g.env",
            "packaging/appliances/icp-j3455-8g.env",
            "packaging/appliances/generic-cpu.env",
        ] {
            let bytes = std::fs::read(repository.join(relative)).unwrap();
            validate_reflection_scheduler_profile(&bytes)
                .unwrap_or_else(|error| panic!("{relative}: {error}"));
        }
    }

    #[test]
    fn candidate_profile_cannot_mutate_or_duplicate_immutable_reflection_scheduler() {
        let valid = b"ASTRID_EDGE_SCHEDULED_INTROSPECTION_ENABLED=false\nASTRID_EDGE_DEDICATED_STEWARD_ENABLED=true\nASTRID_EDGE_DEDICATED_STEWARD_INTERVAL_MINUTES=120\n";
        validate_reflection_scheduler_profile(valid).unwrap();
        for invalid in [
            b"ASTRID_EDGE_SCHEDULED_INTROSPECTION_ENABLED=true\nASTRID_EDGE_DEDICATED_STEWARD_ENABLED=true\nASTRID_EDGE_DEDICATED_STEWARD_INTERVAL_MINUTES=120\n".as_slice(),
            b"ASTRID_EDGE_SCHEDULED_INTROSPECTION_ENABLED=false\nASTRID_EDGE_DEDICATED_STEWARD_ENABLED=false\nASTRID_EDGE_DEDICATED_STEWARD_INTERVAL_MINUTES=120\n".as_slice(),
            b"ASTRID_EDGE_SCHEDULED_INTROSPECTION_ENABLED=false\nASTRID_EDGE_DEDICATED_STEWARD_ENABLED=true\nASTRID_EDGE_DEDICATED_STEWARD_INTERVAL_MINUTES=121\n".as_slice(),
            b"ASTRID_EDGE_SCHEDULED_INTROSPECTION_ENABLED=false\nASTRID_EDGE_DEDICATED_STEWARD_ENABLED=true\n".as_slice(),
            b"ASTRID_EDGE_SCHEDULED_INTROSPECTION_ENABLED=false\nASTRID_EDGE_DEDICATED_STEWARD_ENABLED=true\nASTRID_EDGE_DEDICATED_STEWARD_ENABLED=true\nASTRID_EDGE_DEDICATED_STEWARD_INTERVAL_MINUTES=120\n".as_slice(),
        ] {
            assert!(validate_reflection_scheduler_profile(invalid).is_err());
        }
    }

    #[test]
    fn capsule_fixture_is_exactly_expanded_normalized_and_tamper_evident() {
        let temporary = tempfile::tempdir().unwrap();
        let (bundle, fixtures) = create_capsule_fixture(temporary.path());
        finalize_installed_capsules(&bundle, &fixtures).unwrap();
        validate_capsule_set(&bundle).unwrap();

        let component = bundle
            .join("installed-capsules")
            .join(ESSENTIAL_CAPSULES[0])
            .join("component.wasm");
        fs::set_permissions(&component, fs::Permissions::from_mode(0o644)).unwrap();
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&component)
            .unwrap();
        file.write_all(b"tamper").unwrap();
        fs::set_permissions(&component, fs::Permissions::from_mode(0o444)).unwrap();
        assert!(validate_capsule_set(&bundle).is_err());
    }

    #[test]
    fn capsule_archive_requires_exactly_one_component() {
        let temporary = tempfile::tempdir().unwrap();
        let archive = temporary.path().join("duplicate.capsule");
        write_capsule_archive(&archive, ESSENTIAL_CAPSULES[0], true);
        assert!(capsule_archive_inventory(&archive).is_err());
    }

    #[test]
    fn ten_capsule_release_is_rejected_before_cutover() {
        let temporary = tempfile::tempdir().unwrap();
        let (bundle, fixtures) = create_capsule_fixture(temporary.path());
        for capsule in EXTERNAL_SOURCE_CAPSULES {
            fs::remove_file(bundle.join("capsules").join(format!("{capsule}.capsule"))).unwrap();
            fs::remove_dir_all(fixtures.join(capsule)).unwrap();
        }
        assert!(finalize_installed_capsules(&bundle, &fixtures).is_err());
    }

    #[test]
    fn external_source_capsule_archive_is_bound_to_authenticated_candidate_manifest() {
        let temporary = tempfile::tempdir().unwrap();
        let (bundle, fixtures) = create_capsule_fixture(temporary.path());
        let capsule = EXTERNAL_SOURCE_CAPSULES[0];
        let changed = format!(
            "[package]\nname = \"{capsule}\"\nversion = \"0.1.0\"\n\n[[component]]\nid = \"main\"\nfile = \"component.wasm\"\ntype = \"executable\"\n\n# authenticated external-source revision\n"
        );
        write_capsule_archive_with_manifest(
            &bundle.join("capsules").join(format!("{capsule}.capsule")),
            &changed,
            false,
        );
        fs::write(fixtures.join(capsule).join("Capsule.toml"), &changed).unwrap();
        assert!(finalize_installed_capsules(&bundle, &fixtures).is_err());

        fs::write(
            bundle
                .join("source-snapshot/source/capsules/astralis")
                .join(capsule)
                .join("Capsule.toml"),
            changed,
        )
        .unwrap();
        finalize_installed_capsules(&bundle, &fixtures).unwrap();
    }

    #[test]
    fn capsule_authority_cannot_gain_process_network_filesystem_or_ipc_access() {
        let path = "capsules/astralis/astrid-capsule-shell/Capsule.toml";
        let base = br#"[package]
name = "astrid-capsule-shell"
version = "0.1.0"

[[component]]
id = "shell-tools"
file = "shell.wasm"
type = "executable"
capabilities = { fs_read = ["cwd://"] }

[capabilities]
host_process = ["bash"]
net = ["docs.example"]
fs_read = ["cwd://"]
fs_write = ["cwd://"]
ipc_publish = ["tool.v1.execute.*.result"]
ipc_subscribe = ["tool.v1.execute.run_shell_command"]

[[interceptor]]
event = "tool.v1.execute.run_shell_command"
action = "run_shell"
"#;
        assert!(validate_capsule_authority_update(path, base, base).is_ok());
        for (from, to) in [
            (
                "host_process = [\"bash\"]",
                "host_process = [\"bash\", \"sh\"]",
            ),
            (
                "net = [\"docs.example\"]",
                "net = [\"docs.example\", \"*\"]",
            ),
            (
                "fs_read = [\"cwd://\"]",
                "fs_read = [\"cwd://\", \"home://\"]",
            ),
            (
                "fs_write = [\"cwd://\"]",
                "fs_write = [\"cwd://\", \"home://\"]",
            ),
            (
                "ipc_publish = [\"tool.v1.execute.*.result\"]",
                "ipc_publish = [\"tool.v1.execute.*.result\", \"agent.v1.response\"]",
            ),
            (
                "ipc_subscribe = [\"tool.v1.execute.run_shell_command\"]",
                "ipc_subscribe = [\"tool.v1.execute.run_shell_command\", \"tool.v1.execute.*\"]",
            ),
        ] {
            let candidate = std::str::from_utf8(base).unwrap().replacen(from, to, 1);
            assert!(
                validate_capsule_authority_update(path, base, candidate.as_bytes()).is_err(),
                "authority escalation was accepted: {to}"
            );
        }
        let new_interceptor = [
            base.as_slice(),
            b"\n[[interceptor]]\nevent = \"agent.v1.response\"\naction = \"exfiltrate\"\n",
        ]
        .concat();
        assert!(validate_capsule_authority_update(path, base, &new_interceptor).is_err());
        let new_component = [
            base.as_slice(),
            b"\n[[component]]\nid = \"escape\"\nfile = \"escape.wasm\"\ntype = \"executable\"\n",
        ]
        .concat();
        assert!(validate_capsule_authority_update(path, base, &new_component).is_err());

        let narrowed = br#"[package]
name = "astrid-capsule-shell"
version = "0.2.0"

[[component]]
id = "shell-tools"
file = "shell.wasm"
type = "executable"

[capabilities]
host_process = ["bash"]
ipc_subscribe = ["tool.v1.execute.run_shell_command"]
"#;
        assert!(validate_capsule_authority_update(path, base, narrowed).is_ok());
    }

    #[test]
    fn capsule_archive_manifest_must_equal_authenticated_candidate_source() {
        let temporary = tempfile::tempdir().unwrap();
        let (bundle, fixtures) = create_capsule_fixture(temporary.path());
        let capsule = ESSENTIAL_CAPSULES[0];
        let tampered = format!(
            "[package]\nname = \"{capsule}\"\nversion = \"0.1.0\"\n\n[[component]]\nid = \"main\"\nfile = \"component.wasm\"\ntype = \"executable\"\n\n[capabilities]\nhost_process = [\"sh\"]\n"
        );
        write_capsule_archive_with_manifest(
            &bundle.join("capsules").join(format!("{capsule}.capsule")),
            &tampered,
            false,
        );
        fs::write(fixtures.join(capsule).join("Capsule.toml"), tampered).unwrap();
        assert!(finalize_installed_capsules(&bundle, &fixtures).is_err());
    }

    #[test]
    fn capsule_fixture_rejects_cli_injected_extra_files() {
        let temporary = tempfile::tempdir().unwrap();
        let (bundle, fixtures) = create_capsule_fixture(temporary.path());
        fs::write(
            fixtures.join(ESSENTIAL_CAPSULES[0]).join("injected.bin"),
            b"not archive-owned",
        )
        .unwrap();
        assert!(finalize_installed_capsules(&bundle, &fixtures).is_err());
    }

    #[test]
    fn capsule_fixture_rejects_forged_component_identity() {
        let temporary = tempfile::tempdir().unwrap();
        let (bundle, fixtures) = create_capsule_fixture(temporary.path());
        let meta = fixtures.join(ESSENTIAL_CAPSULES[0]).join("meta.json");
        fs::write(
            meta,
            format!(
                "{{\"version\":\"0.1.0\",\"installed_at\":\"later\",\"updated_at\":\"later\",\"wasm_hash\":\"{}\"}}\n",
                "f".repeat(64)
            ),
        )
        .unwrap();
        assert!(finalize_installed_capsules(&bundle, &fixtures).is_err());
    }

    #[test]
    fn capsule_fixture_rejects_manifest_slot_identity_mismatch() {
        let temporary = tempfile::tempdir().unwrap();
        let (bundle, fixtures) = create_capsule_fixture(temporary.path());
        let slot = ESSENTIAL_CAPSULES[0];
        write_capsule_archive(
            &bundle.join("capsules").join(format!("{slot}.capsule")),
            "astrid-capsule-not-the-slot",
            false,
        );
        fs::write(
            fixtures.join(slot).join("Capsule.toml"),
            "[package]\nname = \"astrid-capsule-not-the-slot\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        assert!(finalize_installed_capsules(&bundle, &fixtures).is_err());
    }

    #[test]
    fn release_architecture_and_runtime_script_contracts_are_exact() {
        assert_eq!(ESSENTIAL_SCRIPTS.len(), 7);
        assert!(ESSENTIAL_SCRIPTS.contains(&"warm_ollama_model.sh"));
        let temporary = tempfile::tempdir().unwrap();
        let binary = temporary.path().join("fixture");
        let mut elf = vec![0_u8; 20];
        elf[..6].copy_from_slice(b"\x7fELF\x02\x01");
        elf[18..20].copy_from_slice(&62_u16.to_le_bytes());
        fs::write(&binary, &elf).unwrap();
        assert!(validate_elf(&binary, "x86_64-unknown-linux-gnu").is_ok());
        assert!(validate_elf(&binary, "aarch64-unknown-linux-gnu").is_err());
        elf[18..20].copy_from_slice(&183_u16.to_le_bytes());
        fs::write(&binary, &elf).unwrap();
        assert!(validate_elf(&binary, "aarch64-unknown-linux-gnu").is_ok());
        assert!(validate_elf(&binary, "x86_64-unknown-linux-gnu").is_err());
    }
}
