//! Immutable generation staging and exact supervisor generation manifests.

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::build::bundle_digest;
use crate::config::Config;
use crate::fs_guard::{
    atomic_write, canonical_json, ensure_within, make_read_only_tree, read_json, read_regular,
    sha256, validate_relative, validate_relative_signed,
};
use crate::invariant::ESSENTIAL_SCRIPTS;
use crate::manifest::BuildV1;
use crate::native::{CommandSpec, NativeRunner, require_success};
use crate::{Error, Result};

#[path = "introspection_evidence.rs"]
mod introspection_evidence;

pub const GENERATION_SCHEMA: &str = "astrid.edge_self_change.generation.v1";
pub const INITIAL_GENERATION_SCHEMA: &str = "astrid.edge_self_change.initial_generation.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GenerationManifest {
    pub schema: String,
    pub appliance_id: String,
    pub generation_id: String,
    pub build_id: String,
    pub candidate_id: String,
    pub candidate_sha256: String,
    pub base_generation: String,
    pub bundle_sha256: String,
    pub tests_sha256: String,
    pub target: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct InitialGenerationManifest {
    schema: String,
    appliance_id: String,
    version: String,
    target: String,
    inventory: Vec<InitialGenerationFile>,
    authority: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct InitialGenerationFile {
    path: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseIdentity {
    pub appliance_id: String,
    pub generation_id: String,
    pub target: String,
    pub operator_initial: bool,
}

impl From<&BuildV1> for GenerationManifest {
    fn from(build: &BuildV1) -> Self {
        Self {
            schema: GENERATION_SCHEMA.to_owned(),
            appliance_id: build.appliance_id.clone(),
            generation_id: build.generation_id.clone(),
            build_id: build.build_id.clone(),
            candidate_id: build.candidate_id.clone(),
            candidate_sha256: build.candidate_sha256.clone(),
            base_generation: build.base_generation.clone(),
            bundle_sha256: build.bundle_sha256.clone(),
            tests_sha256: build.tests_sha256.clone(),
            target: build.target.clone(),
        }
    }
}

pub fn install<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    build_manifest: &Path,
) -> Result<PathBuf> {
    require_effective_uid(0, "install")?;
    let build: BuildV1 = read_json(build_manifest, 256 * 1024)?;
    build.validate(config)?;
    let artifact = config.roots.build_store.join(&build.build_id);
    ensure_within(&config.roots.build_store, &artifact, true)?;
    let stored: BuildV1 = read_json(&artifact.join("build.json"), 256 * 1024)?;
    if canonical_json(&stored)? != canonical_json(&build)? {
        return Err(Error::new(
            "build manifest differs from immutable builder artifact",
        ));
    }
    let bundle = artifact.join("bundle");
    if bundle_digest(&bundle)? != build.bundle_sha256 {
        return Err(Error::new("builder bundle inventory digest failed"));
    }
    if sha256(&read_regular(
        &artifact.join("evidence.json"),
        16 * 1024 * 1024,
    )?) != build.tests_sha256
    {
        return Err(Error::new("builder test evidence digest failed"));
    }
    let final_generation = config.roots.releases.join(&build.generation_id);
    let partial = config
        .roots
        .releases
        .join(format!(".{}.partial", build.generation_id));
    ensure_within(&config.roots.releases, &final_generation, false)?;
    ensure_within(&config.roots.releases, &partial, false)?;
    if final_generation.exists()
        || final_generation.is_symlink()
        || partial.exists()
        || partial.is_symlink()
    {
        return Err(Error::new(
            "generation already exists; install is never replayed",
        ));
    }
    let updater_staging = updater_staging_root(config)?;
    require_updater_staging_root(config, &updater_staging)?;
    let materialized =
        updater_staging.join(format!(".generation-{}.materializing", build.generation_id));
    ensure_within(&updater_staging, &materialized, false)?;
    if materialized.exists() || materialized.is_symlink() {
        return Err(Error::new(
            "updater materialization already exists; install is never replayed",
        ));
    }
    let spec = updater_materialization_spec(
        &config.executables.invariant_runner,
        config.identities.updater_uid,
        config.identities.updater_gid,
        &bundle,
        &materialized,
        &build.bundle_sha256,
        &updater_staging,
        config.policy.command_timeout_seconds,
    );
    let receipt = match runner.run(&spec) {
        Ok(receipt) => receipt,
        Err(error) => {
            let _ = cleanup_materialized(config, &materialized);
            return Err(error);
        },
    };
    if let Err(error) = require_success(&receipt) {
        let _ = cleanup_materialized(config, &materialized);
        return Err(error);
    }
    validate_materialized_tree(config, &materialized, &build.bundle_sha256)?;
    fs::create_dir(&partial)?;
    if let Err(error) = copy_tree(&materialized, &partial) {
        let _ = cleanup_materialized(config, &materialized);
        return Err(error);
    }
    cleanup_materialized(config, &materialized)?;
    if bundle_digest(&partial)? != build.bundle_sha256 {
        return Err(Error::new(
            "staged generation payload differs from verified builder bundle",
        ));
    }
    let generation = GenerationManifest::from(&build);
    atomic_write(
        &partial.join(".astrid-edge-generation.json"),
        &canonical_json(&generation)?,
        0o400,
        false,
    )?;
    // The payload was verified above; the generation manifest is intentionally outside the
    // bundle digest because the supervisor validates it as a separate exact record.
    make_read_only_tree(&partial)?;
    fs::rename(&partial, &final_generation)?;
    fs::File::open(&config.roots.releases)?.sync_all()?;
    validate_generation(config, &build, &final_generation)?;
    introspection_evidence::publish(config, &artifact, &final_generation)?;
    Ok(final_generation)
}

#[allow(clippy::too_many_arguments)]
fn updater_materialization_spec(
    executable: &crate::config::TrustedExecutable,
    uid: u32,
    gid: u32,
    source: &Path,
    destination: &Path,
    expected_sha256: &str,
    current_dir: &Path,
    timeout_seconds: u64,
) -> CommandSpec {
    CommandSpec {
        label: "updater-materialize-generation",
        executable: executable.clone(),
        arguments: vec![
            "internal-materialize-generation".to_owned(),
            "--source".to_owned(),
            source.to_string_lossy().into_owned(),
            "--destination".to_owned(),
            destination.to_string_lossy().into_owned(),
            "--expected-sha256".to_owned(),
            expected_sha256.to_owned(),
        ],
        current_dir: current_dir.to_path_buf(),
        environment: std::collections::BTreeMap::new(),
        timeout: Duration::from_secs(timeout_seconds),
        run_as_uid: Some(uid),
        run_as_gid: Some(gid),
    }
}

pub fn materialize_generation(
    source: &Path,
    destination: &Path,
    expected_sha256: &str,
) -> Result<()> {
    let updater_uid = effective_uid()?;
    if updater_uid == 0 || !crate::config::valid_hex64(expected_sha256) {
        return Err(Error::new(
            "generation materialization requires a non-root updater and exact digest",
        ));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| Error::new("generation materialization parent is absent"))?;
    let parent_metadata = fs::symlink_metadata(parent)?;
    let destination_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if !parent_metadata.is_dir()
        || parent_metadata.file_type().is_symlink()
        || parent_metadata.uid() != updater_uid
        || parent_metadata.mode() & 0o077 != 0
        || !destination_name.starts_with(".generation-")
        || !destination_name.ends_with(".materializing")
        || destination.exists()
        || destination.is_symlink()
    {
        return Err(Error::new(
            "generation materialization destination is outside the updater staging root",
        ));
    }
    let source_metadata = fs::symlink_metadata(source)?;
    if !source_metadata.is_dir()
        || source_metadata.file_type().is_symlink()
        || source_metadata.uid() != 0
        || source_metadata.mode() & 0o022 != 0
        || bundle_digest(source)? != expected_sha256
    {
        return Err(Error::new(
            "generation materialization source is not the exact immutable builder bundle",
        ));
    }
    fs::create_dir(destination)?;
    fs::set_permissions(destination, fs::Permissions::from_mode(0o700))?;
    if let Err(error) = copy_tree(source, destination) {
        let _ = fs::remove_dir_all(destination);
        return Err(error);
    }
    if bundle_digest(destination)? != expected_sha256 {
        return Err(Error::new("updater materialization digest failed"));
    }
    fs::File::open(destination)?.sync_all()?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

pub fn updater_staging_root(config: &Config) -> Result<PathBuf> {
    let parent = config
        .roots
        .state_snapshots
        .parent()
        .ok_or_else(|| Error::new("updater state root is absent"))?;
    if config
        .roots
        .state_snapshots
        .file_name()
        .and_then(|name| name.to_str())
        != Some("snapshots")
    {
        return Err(Error::new(
            "updater snapshot root has an unsupported layout",
        ));
    }
    Ok(parent.join("generation-staging"))
}

fn require_updater_staging_root(config: &Config, root: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(root)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.nlink() < 2
        || metadata.uid() != config.identities.updater_uid
        || metadata.gid() != config.identities.updater_gid
        || metadata.mode() & 0o777 != 0o700
    {
        return Err(Error::new(
            "updater generation staging root ownership or mode failed",
        ));
    }
    Ok(())
}

fn validate_materialized_tree(config: &Config, root: &Path, expected_sha256: &str) -> Result<()> {
    fn visit(path: &Path, uid: u32, gid: u32) -> Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink()
            || metadata.uid() != uid
            || metadata.gid() != gid
            || metadata.mode() & 0o022 != 0
        {
            return Err(Error::new(
                "updater materialization contains linked, foreign, or writable content",
            ));
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(path)? {
                visit(&entry?.path(), uid, gid)?;
            }
        } else if !metadata.is_file() || metadata.nlink() != 1 {
            return Err(Error::new(
                "updater materialization contains linked or special content",
            ));
        }
        Ok(())
    }

    require_updater_staging_root(config, &updater_staging_root(config)?)?;
    ensure_within(&updater_staging_root(config)?, root, true)?;
    visit(
        root,
        config.identities.updater_uid,
        config.identities.updater_gid,
    )?;
    if bundle_digest(root)? != expected_sha256 {
        return Err(Error::new("updater materialization digest failed"));
    }
    Ok(())
}

fn cleanup_materialized(config: &Config, root: &Path) -> Result<()> {
    let staging = updater_staging_root(config)?;
    ensure_within(&staging, root, true)?;
    validate_materialized_tree(config, root, &bundle_digest(root)?)?;
    fs::remove_dir_all(root)?;
    fs::File::open(staging)?.sync_all()?;
    Ok(())
}

pub fn validate_generation(config: &Config, build: &BuildV1, path: &Path) -> Result<()> {
    ensure_within(&config.roots.releases, path, true)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || metadata.mode() & 0o222 != 0 {
        return Err(Error::new(
            "generation is not an immutable regular directory",
        ));
    }
    let actual: GenerationManifest =
        read_json(&path.join(".astrid-edge-generation.json"), 64 * 1024)?;
    if actual != GenerationManifest::from(build) {
        return Err(Error::new(
            "generation manifest does not bind exact Build v1",
        ));
    }
    if bundle_digest(path)? != build.bundle_sha256 {
        return Err(Error::new(
            "generation payload digest differs from Build v1",
        ));
    }
    validate_release_tree(path, true)?;
    validate_required_layout(path, &build.target)?;
    let _ = crate::manifest::SourceSnapshot::validate_generation_snapshot(
        config,
        path,
        &build.base_generation,
    )?;
    Ok(())
}

pub fn validate_release_manifest(config: &Config, path: &Path) -> Result<ReleaseIdentity> {
    validate_release_manifest_inner(config, path, true)
}

pub(crate) fn validate_release_manifest_inner(
    config: &Config,
    path: &Path,
    require_root_owner: bool,
) -> Result<ReleaseIdentity> {
    ensure_within(&config.roots.releases, path, true)?;
    let generation_id = path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| crate::config::valid_identifier(value))
        .ok_or_else(|| Error::new("release directory has invalid generation ID"))?
        .to_owned();
    let manifest_path = path.join(".astrid-edge-generation.json");
    let value: Value = read_json(&manifest_path, 16 * 1024 * 1024)?;
    match value.get("schema").and_then(Value::as_str) {
        Some(GENERATION_SCHEMA) => {
            let manifest: GenerationManifest = serde_json::from_value(value)?;
            if candidate_manifest_identity_error(
                &manifest,
                &generation_id,
                &config.appliance_id,
                &config.target,
            )
            .is_some()
            {
                return Err(Error::new("candidate generation identity or target failed"));
            }
            if bundle_digest(path)? != manifest.bundle_sha256 {
                return Err(Error::new("candidate generation payload digest failed"));
            }
            validate_release_tree(path, require_root_owner)?;
            validate_required_layout(path, &manifest.target)?;
            let _ = crate::manifest::SourceSnapshot::validate_retained_generation_snapshot(
                config,
                path,
                &manifest.base_generation,
            )?;
            Ok(ReleaseIdentity {
                appliance_id: manifest.appliance_id,
                generation_id,
                target: manifest.target,
                operator_initial: false,
            })
        },
        Some(INITIAL_GENERATION_SCHEMA) => {
            let manifest: InitialGenerationManifest = serde_json::from_value(value)?;
            validate_initial_manifest(path, &manifest, config, require_root_owner)?;
            Ok(ReleaseIdentity {
                appliance_id: manifest.appliance_id,
                generation_id,
                target: manifest.target,
                operator_initial: true,
            })
        },
        _ => Err(Error::new("unsupported release generation manifest")),
    }
}

fn validate_initial_manifest(
    path: &Path,
    manifest: &InitialGenerationManifest,
    config: &Config,
    require_root_owner: bool,
) -> Result<()> {
    if initial_manifest_identity_error(manifest, &config.appliance_id, &config.target).is_some()
        || manifest.version.is_empty()
        || manifest.version.len() > 128
        || manifest.authority != "operator_packaged_initial_generation_not_model_candidate"
        || manifest.inventory.is_empty()
        || manifest.inventory.len() > 50_000
    {
        return Err(Error::new(
            "operator initial-generation authority or bounds failed",
        ));
    }
    let mut expected = std::collections::BTreeSet::new();
    for item in &manifest.inventory {
        let relative = validate_relative_signed(&item.path)?;
        if !crate::config::valid_hex64(&item.sha256) || !expected.insert(item.path.clone()) {
            return Err(Error::new(
                "operator initial-generation inventory is invalid",
            ));
        }
        let file = path.join(relative);
        let bytes = read_regular(&file, 512 * 1024 * 1024)?;
        if bytes.len() as u64 != item.size || sha256(&bytes) != item.sha256 {
            return Err(Error::new("operator initial-generation file hash failed"));
        }
    }
    let mut actual = std::collections::BTreeSet::new();
    collect_release_files(path, path, &mut actual)?;
    actual.remove(".astrid-edge-generation.json");
    if actual != expected {
        return Err(Error::new(
            "operator initial-generation inventory membership failed",
        ));
    }
    validate_release_tree(path, require_root_owner)?;
    validate_required_layout(path, &manifest.target)
}

fn initial_manifest_identity_error(
    manifest: &InitialGenerationManifest,
    appliance_id: &str,
    target: &str,
) -> Option<&'static str> {
    (manifest.schema != INITIAL_GENERATION_SCHEMA)
        .then_some("schema")
        .or_else(|| (manifest.appliance_id != appliance_id).then_some("appliance_id"))
        .or_else(|| (manifest.target != target).then_some("target"))
}

fn candidate_manifest_identity_error(
    manifest: &GenerationManifest,
    generation_id: &str,
    appliance_id: &str,
    target: &str,
) -> Option<&'static str> {
    (manifest.schema != GENERATION_SCHEMA)
        .then_some("schema")
        .or_else(|| (manifest.appliance_id != appliance_id).then_some("appliance_id"))
        .or_else(|| (manifest.generation_id != generation_id).then_some("generation_id"))
        .or_else(|| (manifest.target != target).then_some("target"))
}

fn collect_release_files(
    root: &Path,
    directory: &Path,
    output: &mut std::collections::BTreeSet<String>,
) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::new("release inventory contains a symlink"));
        }
        if metadata.is_dir() {
            collect_release_files(root, &path, output)?;
        } else if metadata.is_file() && metadata.nlink() == 1 {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| Error::new("release inventory path escape"))?
                .to_string_lossy()
                .replace('\\', "/");
            validate_relative_signed(&relative)?;
            output.insert(relative);
        } else {
            return Err(Error::new(
                "release inventory contains a linked or special file",
            ));
        }
    }
    Ok(())
}

fn validate_required_layout(path: &Path, target: &str) -> Result<()> {
    for binary in [
        "astrid",
        "astrid-daemon",
        "astrid-build",
        "astrid-edge-runtime",
    ] {
        let metadata = fs::symlink_metadata(path.join(binary))?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.nlink() != 1
            || metadata.mode() & 0o111 == 0
        {
            return Err(Error::new("release lacks a valid required executable"));
        }
    }
    for script in ESSENTIAL_SCRIPTS {
        let metadata = fs::symlink_metadata(path.join("scripts").join(script))?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.nlink() != 1
            || metadata.mode() & 0o111 == 0
        {
            return Err(Error::new(
                "release lacks a required executable runtime/report script",
            ));
        }
    }
    crate::invariant::validate_release_architecture(path, target)?;
    crate::invariant::validate_installed_capsule_set(path)
}

fn validate_release_tree(path: &Path, require_root_owner: bool) -> Result<()> {
    fn visit(directory: &Path, require_root_owner: bool) -> Result<()> {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink()
                || metadata.mode() & 0o022 != 0
                || (require_root_owner && metadata.uid() != 0)
            {
                return Err(Error::new(
                    "release contains a linked, mutable, or non-root-owned entry",
                ));
            }
            if metadata.is_dir() {
                visit(&path, require_root_owner)?;
            } else if !metadata.is_file() || metadata.nlink() != 1 {
                return Err(Error::new("release contains a linked or special entry"));
            }
        }
        Ok(())
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.mode() & 0o022 != 0
        || (require_root_owner && metadata.uid() != 0)
    {
        return Err(Error::new("release root ownership or mode failed"));
    }
    visit(path, require_root_owner)
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let input = entry.path();
        let relative = input
            .strip_prefix(source)
            .map_err(|_| Error::new("copy source escape"))?;
        let relative = validate_relative(&relative.to_string_lossy().replace('\\', "/"))?;
        let output = destination.join(relative);
        let metadata = fs::symlink_metadata(&input)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::new("builder artifact contains a symlink"));
        }
        if metadata.is_dir() {
            fs::create_dir(&output)?;
            copy_tree(&input, &output)?;
        } else if metadata.is_file() && metadata.nlink() == 1 {
            let bytes = read_regular(&input, 512 * 1024 * 1024)?;
            atomic_write(&output, &bytes, metadata.mode() & 0o777, false)?;
        } else {
            return Err(Error::new(
                "builder artifact contains a linked or special file",
            ));
        }
    }
    Ok(())
}

pub fn effective_uid() -> Result<u32> {
    let status = fs::read_to_string("/proc/self/status")?;
    let line = status
        .lines()
        .find(|line| line.starts_with("Uid:"))
        .ok_or_else(|| Error::new("cannot determine effective uid"))?;
    line.split_whitespace()
        .nth(2)
        .ok_or_else(|| Error::new("cannot parse effective uid"))?
        .parse::<u32>()
        .map_err(|_| Error::new("cannot parse effective uid"))
}

pub fn require_effective_uid(expected: u32, operation: &str) -> Result<()> {
    if effective_uid()? != expected {
        return Err(Error::new(format!(
            "{operation} requires exact configured privilege identity"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        GENERATION_SCHEMA, GenerationManifest, INITIAL_GENERATION_SCHEMA, InitialGenerationFile,
        InitialGenerationManifest, candidate_manifest_identity_error,
        initial_manifest_identity_error, updater_materialization_spec,
    };
    use crate::config::TrustedExecutable;
    use crate::manifest::BuildV1;

    #[test]
    fn generation_manifest_is_exact_supervisor_shape() {
        let build = BuildV1 {
            schema: "astrid.edge_self_change.build.v1".into(),
            appliance_id: "avado-test".into(),
            build_id: "build-a".into(),
            candidate_id: "candidate-a".into(),
            candidate_sha256: "a".repeat(64),
            base_generation: "gen-old".into(),
            generation_id: "gen-new".into(),
            source_revision: "abcdef1".into(),
            bundle_sha256: "b".repeat(64),
            tests_sha256: "c".repeat(64),
            target: "x86_64-unknown-linux-gnu".into(),
            created_at: 1,
            privilege_envelope: "offline-build-sandbox:no-host-state:v1".into(),
        };
        let generation = GenerationManifest::from(&build);
        assert_eq!(generation.schema, GENERATION_SCHEMA);
        assert_eq!(generation.appliance_id, "avado-test");
        assert_eq!(generation.build_id, "build-a");
    }

    #[test]
    fn generation_lineage_rejects_an_otherwise_identical_cross_appliance_release() {
        let build = BuildV1 {
            schema: "astrid.edge_self_change.build.v1".into(),
            appliance_id: "avado-test".into(),
            build_id: "build-a".into(),
            candidate_id: "candidate-a".into(),
            candidate_sha256: "a".repeat(64),
            base_generation: "gen-old".into(),
            generation_id: "gen-new".into(),
            source_revision: "abcdef1".into(),
            bundle_sha256: "b".repeat(64),
            tests_sha256: "c".repeat(64),
            target: "x86_64-unknown-linux-gnu".into(),
            created_at: 1,
            privilege_envelope: "offline-build-sandbox:no-host-state:v1".into(),
        };
        let local = GenerationManifest::from(&build);
        assert_eq!(
            candidate_manifest_identity_error(
                &local,
                "gen-new",
                "avado-test",
                "x86_64-unknown-linux-gnu"
            ),
            None
        );
        let mut foreign = local.clone();
        foreign.appliance_id = "icp-other-box".into();
        assert_eq!(
            candidate_manifest_identity_error(
                &foreign,
                "gen-new",
                "avado-test",
                "x86_64-unknown-linux-gnu"
            ),
            Some("appliance_id")
        );

        let initial = InitialGenerationManifest {
            schema: INITIAL_GENERATION_SCHEMA.into(),
            appliance_id: "avado-test".into(),
            version: "cpu-edge.3".into(),
            target: "x86_64-unknown-linux-gnu".into(),
            inventory: vec![InitialGenerationFile {
                path: "astrid".into(),
                size: 1,
                sha256: "d".repeat(64),
            }],
            authority: "operator_packaged_initial_generation_not_model_candidate".into(),
        };
        assert_eq!(
            initial_manifest_identity_error(&initial, "avado-test", "x86_64-unknown-linux-gnu"),
            None
        );
        let mut foreign_initial = initial.clone();
        foreign_initial.appliance_id = "icp-other-box".into();
        assert_eq!(
            initial_manifest_identity_error(
                &foreign_initial,
                "avado-test",
                "x86_64-unknown-linux-gnu"
            ),
            Some("appliance_id")
        );
    }

    #[test]
    fn updater_materialization_runs_only_as_the_separate_updater_identity() {
        let spec = updater_materialization_spec(
            &TrustedExecutable {
                path: "/usr/libexec/astrid/astrid-edge-rescue-helper".into(),
                sha256: "a".repeat(64),
            },
            982,
            983,
            std::path::Path::new("/var/lib/astrid-edge-builder/builds/build-a/bundle"),
            std::path::Path::new(
                "/var/lib/astrid-edge-updater/generation-staging/.generation-gen-new.materializing",
            ),
            &"b".repeat(64),
            std::path::Path::new("/var/lib/astrid-edge-updater/generation-staging"),
            3_600,
        );
        assert_eq!(spec.run_as_uid, Some(982));
        assert_eq!(spec.run_as_gid, Some(983));
        assert!(spec.environment.is_empty());
        assert_eq!(spec.label, "updater-materialize-generation");
        assert_eq!(spec.arguments[0], "internal-materialize-generation");
        assert_eq!(spec.arguments[6], "b".repeat(64));
    }
}
