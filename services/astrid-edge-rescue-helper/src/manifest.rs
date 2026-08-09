//! Signed source, candidate-patch, and supervisor Build-v1 schemas.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::config::{Config, valid_hex64, valid_identifier};
use crate::fs_guard::{
    MAX_JSON_BYTES, atomic_write, canonical_json, read_json, read_regular, sha256,
    validate_relative, validate_relative_signed,
};
use crate::{Error, Result};

pub const CANDIDATE_SCHEMA: &str = "astrid.edge_self_change.candidate.v1";
pub const PATCH_SCHEMA: &str = "astrid.edge_self_change.full_replacement_patch.v1";
const MAX_MUTABLE_FILE_BYTES: usize = 512 * 1024;
const MAX_LOCKFILE_LINES: usize = 20_000;
pub const BUILD_SCHEMA: &str = "astrid.edge_self_change.build.v1";
const SOURCE_SCHEMA: &str = "astrid.edge.self_change_source_bundle.v1";
const SIGNATURE_SCHEMA: &str = "astrid.edge.self_change_source_signature.v1";
const SOURCE_ID_SCHEMA: &str = "astrid.edge.self_change_source_identity.v1";
const DERIVED_SOURCE_SCHEMA: &str = "astrid.edge.self_change_generation_source.v1";
const DERIVED_SIGNATURE_SCHEMA: &str = "astrid.edge.self_change_generation_source_signature.v1";
const INSPECT_ONLY_ORIGIN: &str = "inspect_only_immutable_boundary";
const QUICKJS_KERNEL_PATH: &str = "source/crates/astrid-openclaw/kernel/engine.wasm";
const QUICKJS_KERNEL_HASH_PATH: &str = "source/crates/astrid-openclaw/kernel/engine.wasm.blake3";
const EDGE_CAPSULES: [&str; 20] = [
    "astrid-capsule-agents",
    "astrid-capsule-cli",
    "astrid-capsule-edge-context",
    "astrid-capsule-edge-introspector",
    "astrid-capsule-edge-spectral",
    "astrid-capsule-fs",
    "astrid-capsule-http",
    "astrid-capsule-memory",
    "astrid-capsule-shell",
    "astrid-capsule-skills",
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
const EDGE_STANDALONE_SERVICES: [&str; 7] = [
    "astrid-edge-checkpoint",
    "astrid-edge-presentation-broker",
    "astrid-edge-provider-broker",
    "astrid-edge-rescue-helper",
    "astrid-edge-runtime",
    "astrid-edge-steward-helper",
    "astrid-edge-web-broker",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub schema: String,
    pub candidate_id: String,
    pub base_generation: String,
    pub proposal_sha256: String,
    pub patch_sha256: String,
    pub changed_paths: Vec<String>,
    pub created_at: u64,
    pub privilege_envelope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchBundle {
    pub schema: String,
    pub candidate_id: String,
    pub source_id: String,
    pub base_generation: String,
    pub files: Vec<PatchFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchFile {
    pub path: String,
    pub source_sha256: String,
    pub content_sha256: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildV1 {
    pub schema: String,
    pub appliance_id: String,
    pub build_id: String,
    pub candidate_id: String,
    pub candidate_sha256: String,
    pub base_generation: String,
    pub generation_id: String,
    pub source_revision: String,
    pub bundle_sha256: String,
    pub tests_sha256: String,
    pub target: String,
    pub created_at: u64,
    pub privilege_envelope: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceManifest {
    schema: String,
    source_id: String,
    source_identity_sha256: String,
    repository_commit: String,
    git_object_format: String,
    rustc: Value,
    cargo_lock_version: u64,
    cargo_lock_sha256: String,
    vendor_packages: Vec<VendorPackage>,
    signature_mode: String,
    key_id: String,
    file_count: usize,
    uncompressed_bytes: u64,
    files: Vec<SourceFile>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceSignature {
    schema: String,
    mode: String,
    key_id: String,
    manifest_sha256: String,
    hmac_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceFile {
    path: String,
    origin: String,
    mode: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VendorPackage {
    directory: String,
    name: String,
    version: String,
    package_checksum: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SourceSnapshot {
    pub source_id: String,
    pub repository_commit: String,
    root: PathBuf,
    entries: BTreeMap<String, SourceFile>,
    vendor_root: PathBuf,
    vendor_entries: BTreeMap<String, SourceFile>,
    vendor_versions: BTreeMap<String, BTreeSet<Version>>,
    vendor_checksums: BTreeMap<(String, Version), Option<String>>,
    local_packages: BTreeSet<String>,
    vendor_attestation_sha256: String,
    lineage_base_generation: Option<String>,
    lineage_parent_source_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DerivedSourceManifest {
    schema: String,
    source_id: String,
    parent_source_id: String,
    base_generation: String,
    repository_commit: String,
    vendor_attestation_sha256: String,
    file_count: usize,
    uncompressed_bytes: u64,
    files: Vec<SourceFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DerivedSourceSignature {
    schema: String,
    mode: String,
    key_id: String,
    manifest_sha256: String,
    hmac_sha256: String,
}

#[derive(Debug, Serialize)]
struct DerivedSourceIdentity<'a> {
    schema: &'static str,
    parent_source_id: &'a str,
    base_generation: &'a str,
    repository_commit: &'a str,
    vendor_attestation_sha256: &'a str,
    files: &'a [SourceFile],
}

impl SourceSnapshot {
    pub(crate) fn source_id(&self) -> &str {
        &self.source_id
    }

    pub(crate) fn parent_source_id(&self) -> Option<&str> {
        self.lineage_parent_source_id.as_deref()
    }

    pub(crate) fn verified_source(&self, relative: &str) -> Result<(String, Vec<u8>)> {
        let relative = validate_relative_signed(relative)?;
        let key = format!("source/{}", relative.to_string_lossy().replace('\\', "/"));
        let entry = self
            .entries
            .get(&key)
            .ok_or_else(|| Error::new("generation diff path is absent from signed source"))?;
        Ok((entry.sha256.clone(), self.verified(entry)?))
    }

    /// Create the one permitted operator harness candidate: an exact no-op
    /// replacement of a mutable appliance profile inside an isolated store.
    pub fn create_synthetic_noop_candidate(
        &self,
        config: &Config,
        base_generation: &str,
        created_at: u64,
    ) -> Result<(Candidate, PathBuf)> {
        if !valid_identifier(base_generation) {
            return Err(Error::new("synthetic candidate base generation is invalid"));
        }
        let relative = "packaging/appliances/generic-cpu.env";
        let entry = self
            .entries
            .get(&format!("source/{relative}"))
            .ok_or_else(|| Error::new("signed synthetic profile fixture is absent"))?;
        if entry.origin != "mutable_appliance_profile" {
            return Err(Error::new(
                "synthetic fixture is not signed as a mutable appliance profile",
            ));
        }
        let content = String::from_utf8(self.verified(entry)?)
            .map_err(|_| Error::new("synthetic profile fixture is not UTF-8"))?;
        let proposal_sha256 = sha256(&canonical_json(&serde_json::json!({
            "schema": "astrid.edge_rescue_helper.synthetic_candidate.v1",
            "base_generation": base_generation,
            "source_id": self.source_id,
            "created_at": created_at,
            "authority": "operator_isolated_synthetic_non_authorizing",
        }))?);
        let candidate_id = format!("synthetic-{}", &proposal_sha256[..24]);
        let patch = PatchBundle {
            schema: PATCH_SCHEMA.to_owned(),
            candidate_id: candidate_id.clone(),
            source_id: self.source_id.clone(),
            base_generation: base_generation.to_owned(),
            files: vec![PatchFile {
                path: relative.to_owned(),
                source_sha256: entry.sha256.clone(),
                content_sha256: entry.sha256.clone(),
                content,
            }],
        };
        let patch_bytes = canonical_json(&patch)?;
        let patch_sha256 = sha256(&patch_bytes);
        let candidate = Candidate {
            schema: CANDIDATE_SCHEMA.to_owned(),
            candidate_id,
            base_generation: base_generation.to_owned(),
            proposal_sha256,
            patch_sha256: patch_sha256.clone(),
            changed_paths: vec![relative.to_owned()],
            created_at,
            privilege_envelope: "proposal-only:no-execution:v1".to_owned(),
        };
        candidate.validate(config)?;
        let patch_path = config
            .roots
            .candidate_store
            .join(format!("candidate-patch-{patch_sha256}.json"));
        atomic_write(&patch_path, &patch_bytes, 0o400, false)?;
        let manifest_path = config
            .roots
            .candidate_store
            .join(format!("{}.json", candidate.candidate_id));
        atomic_write(&manifest_path, &canonical_json(&candidate)?, 0o400, false)?;
        Ok((candidate, manifest_path))
    }

    #[allow(clippy::too_many_lines)] // Authentication and inventory construction are one atomic gate.
    pub fn load(config: &Config, verify_all_files: bool) -> Result<Self> {
        let manifest_bytes = read_regular(&config.source.manifest, MAX_JSON_BYTES)?;
        let manifest_value: Value = serde_json::from_slice(&manifest_bytes)?;
        let manifest: SourceManifest = serde_json::from_value(manifest_value.clone())?;
        let signature: SourceSignature = read_json(&config.source.signature, 16 * 1024)?;
        let key = source_signing_key(config)?;
        let canonical = canonical_json(&manifest_value)?;
        let key_id = &sha256(&key)[..16];
        let source_identity = serde_json::json!({
            "schema": SOURCE_ID_SCHEMA,
            "repository_commit": &manifest.repository_commit,
            "rustc": &manifest.rustc,
            "files": &manifest.files,
        });
        let source_identity_sha256 = sha256(&canonical_json(&source_identity)?);
        if manifest.schema != SOURCE_SCHEMA
            || signature.schema != SIGNATURE_SCHEMA
            || manifest.signature_mode != "hmac-sha256"
            || signature.mode != "hmac-sha256"
            || manifest.key_id != key_id
            || signature.key_id != key_id
            || signature.manifest_sha256 != sha256(&canonical)
            || !constant_time_equal(
                hmac_sha256(&key, &canonical).as_bytes(),
                signature.hmac_sha256.as_bytes(),
            )
            || manifest.file_count != manifest.files.len()
            || manifest.source_identity_sha256 != source_identity_sha256
            || manifest.source_id != format!("cpu-edge:{source_identity_sha256}")
            || manifest.repository_commit.len() < 7
            || !matches!(manifest.git_object_format.as_str(), "sha1" | "sha256")
            || !matches!(manifest.cargo_lock_version, 3 | 4)
            || !valid_hex64(&manifest.cargo_lock_sha256)
            || manifest.uncompressed_bytes == 0
            || manifest.rustc.is_null()
        {
            return Err(Error::new("signed source manifest authentication failed"));
        }
        let mut entries = BTreeMap::new();
        let mut vendor_entries = BTreeMap::new();
        let mut total_bytes = 0_u64;
        let mut previous_path: Option<String> = None;
        for file in manifest.files {
            validate_relative_signed(&file.path)?;
            total_bytes = total_bytes
                .checked_add(file.size)
                .ok_or_else(|| Error::new("signed source byte total overflow"))?;
            if !valid_hex64(&file.sha256)
                || !matches!(file.mode.as_str(), "0644" | "0755")
                || !valid_source_origin(&file.path, &file.origin)
                || previous_path
                    .as_deref()
                    .is_some_and(|previous| previous >= file.path.as_str())
            {
                return Err(Error::new("invalid or duplicate signed source inventory"));
            }
            previous_path = Some(file.path.clone());
            if file.path.starts_with("vendor/") {
                if vendor_entries.insert(file.path.clone(), file).is_some() {
                    return Err(Error::new("duplicate signed vendor inventory"));
                }
            } else if file.path.starts_with("source/")
                && entries.insert(file.path.clone(), file).is_some()
            {
                return Err(Error::new("duplicate signed source inventory"));
            }
        }
        if total_bytes != manifest.uncompressed_bytes
            || entries.is_empty()
            || vendor_entries.is_empty()
            || entries
                .get("source/Cargo.lock")
                .is_none_or(|entry| entry.sha256 != manifest.cargo_lock_sha256)
        {
            return Err(Error::new("signed source inventory totals are invalid"));
        }
        let mut vendor_versions = BTreeMap::<String, BTreeSet<Version>>::new();
        let mut vendor_checksums = BTreeMap::new();
        let mut vendor_directories = BTreeSet::new();
        for package in manifest.vendor_packages {
            validate_relative_signed(&package.directory)?;
            let version = Version::parse(&package.version)
                .map_err(|_| Error::new("signed vendor version is invalid"))?;
            if package.name.is_empty()
                || package
                    .package_checksum
                    .as_ref()
                    .is_some_and(|hash| !valid_hex64(hash))
                || !vendor_directories.insert(package.directory)
            {
                return Err(Error::new("signed vendor package is invalid"));
            }
            if vendor_checksums
                .insert(
                    (package.name.clone(), version.clone()),
                    package.package_checksum,
                )
                .is_some()
            {
                return Err(Error::new("duplicate signed vendor package identity"));
            }
            vendor_versions
                .entry(package.name)
                .or_default()
                .insert(version);
        }
        let inventoried_vendor_directories = vendor_entries
            .keys()
            .filter_map(|path| path.split('/').nth(1))
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        if vendor_directories != inventoried_vendor_directories {
            return Err(Error::new(
                "signed vendor packages and file inventory differ",
            ));
        }
        let mut snapshot = Self {
            source_id: manifest.source_id,
            repository_commit: manifest.repository_commit,
            root: config.source.root.clone(),
            entries,
            vendor_root: config.source.root.clone(),
            vendor_entries,
            vendor_versions,
            vendor_checksums,
            local_packages: BTreeSet::new(),
            vendor_attestation_sha256: String::new(),
            lineage_base_generation: None,
            lineage_parent_source_id: None,
        };
        Self::verify_root_layout(config)?;
        snapshot.vendor_attestation_sha256 = snapshot.verify_vendor_tree()?;
        snapshot.local_packages = snapshot.discover_local_packages()?;
        snapshot.verify_required_build_closure()?;
        if verify_all_files {
            for entry in snapshot.entries.values() {
                let _ = snapshot.verified(entry)?;
            }
        }
        Ok(snapshot)
    }

    /// Load the exact source snapshot bound into the active generation. Operator-created
    /// generations use the immutable signed bootstrap snapshot; every model candidate must carry
    /// a cumulative, independently authenticated snapshot in its generation payload.
    pub fn load_for_generation(
        config: &Config,
        generation_root: &Path,
        operator_initial: bool,
    ) -> Result<Self> {
        let trusted = Self::load(config, false)?;
        if operator_initial {
            if generation_root.join("source-snapshot").exists() {
                return Err(Error::new(
                    "operator initial generation unexpectedly contains a derived source snapshot",
                ));
            }
            return Ok(trusted);
        }
        Self::load_derived(config, &generation_root.join("source-snapshot"), trusted)
    }

    #[allow(clippy::too_many_lines)] // Signature, lineage, and exact-tree validation form one gate.
    fn load_derived(config: &Config, root: &Path, trusted: Self) -> Result<Self> {
        let metadata = fs::symlink_metadata(root)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() || metadata.mode() & 0o022 != 0 {
            return Err(Error::new(
                "generation source snapshot is linked or mutable",
            ));
        }
        let manifest_value: Value = read_json(&root.join("MANIFEST.json"), MAX_JSON_BYTES)?;
        let manifest: DerivedSourceManifest = serde_json::from_value(manifest_value.clone())?;
        let signature: DerivedSourceSignature =
            read_json(&root.join("MANIFEST.signature.json"), 16 * 1024)?;
        let key = source_signing_key(config)?;
        let canonical = canonical_json(&manifest_value)?;
        let key_id = &sha256(&key)[..16];
        let identity = DerivedSourceIdentity {
            schema: DERIVED_SOURCE_SCHEMA,
            parent_source_id: &manifest.parent_source_id,
            base_generation: &manifest.base_generation,
            repository_commit: &manifest.repository_commit,
            vendor_attestation_sha256: &manifest.vendor_attestation_sha256,
            files: &manifest.files,
        };
        let identity_sha256 = sha256(&canonical_json(&identity)?);
        if manifest.schema != DERIVED_SOURCE_SCHEMA
            || signature.schema != DERIVED_SIGNATURE_SCHEMA
            || signature.mode != "hmac-sha256"
            || signature.key_id != key_id
            || signature.manifest_sha256 != sha256(&canonical)
            || !constant_time_equal(
                hmac_sha256(&key, &canonical).as_bytes(),
                signature.hmac_sha256.as_bytes(),
            )
            || manifest.source_id != format!("cpu-edge:{identity_sha256}")
            || !manifest
                .parent_source_id
                .strip_prefix("cpu-edge:")
                .is_some_and(valid_hex64)
            || !valid_identifier(&manifest.base_generation)
            || manifest.repository_commit != trusted.repository_commit
            || manifest.vendor_attestation_sha256 != trusted.vendor_attestation_sha256
            || manifest.file_count != manifest.files.len()
            || manifest.files.is_empty()
            || manifest.files.len() > 50_000
        {
            return Err(Error::new(
                "generation source snapshot authentication or lineage failed",
            ));
        }
        let mut entries = BTreeMap::new();
        let mut total = 0_u64;
        let mut previous_path: Option<String> = None;
        for file in manifest.files {
            validate_relative_signed(&file.path)?;
            if !file.path.starts_with("source/")
                || !valid_hex64(&file.sha256)
                || !matches!(file.mode.as_str(), "0644" | "0755")
                || !valid_source_origin(&file.path, &file.origin)
                || previous_path
                    .as_deref()
                    .is_some_and(|previous| previous >= file.path.as_str())
                || entries.insert(file.path.clone(), file.clone()).is_some()
            {
                return Err(Error::new("generation source inventory is invalid"));
            }
            previous_path = Some(file.path.clone());
            total = total
                .checked_add(file.size)
                .ok_or_else(|| Error::new("generation source byte total overflow"))?;
        }
        let surface_matches = entries.len() == trusted.entries.len()
            && entries.iter().all(|(path, entry)| {
                trusted.entries.get(path).is_some_and(|baseline| {
                    baseline.origin == entry.origin
                        && baseline.mode == entry.mode
                        && (mutable_origin_matches(
                            path.strip_prefix("source/").unwrap_or_default(),
                            &baseline.origin,
                        ) || (baseline.size == entry.size && baseline.sha256 == entry.sha256))
                })
            });
        if total == 0
            || total > 4 * 1024 * 1024 * 1024_u64
            || total != manifest.uncompressed_bytes
            || !surface_matches
        {
            return Err(Error::new(
                "generation source totals or immutable surface identity failed",
            ));
        }
        let mut snapshot = Self {
            source_id: manifest.source_id,
            repository_commit: trusted.repository_commit,
            root: root.to_path_buf(),
            entries,
            vendor_root: trusted.vendor_root,
            vendor_entries: trusted.vendor_entries,
            vendor_versions: trusted.vendor_versions,
            vendor_checksums: trusted.vendor_checksums,
            local_packages: BTreeSet::new(),
            vendor_attestation_sha256: trusted.vendor_attestation_sha256,
            lineage_base_generation: Some(manifest.base_generation),
            lineage_parent_source_id: Some(manifest.parent_source_id),
        };
        snapshot.verify_source_tree_exact()?;
        snapshot.local_packages = snapshot.discover_local_packages()?;
        snapshot.verify_required_build_closure()?;
        for entry in snapshot
            .entries
            .values()
            .filter(|entry| entry.path.ends_with("Cargo.lock"))
        {
            let content = String::from_utf8(snapshot.verified(entry)?)
                .map_err(|_| Error::new("derived Cargo.lock is not UTF-8"))?;
            snapshot.validate_lockfile(&content)?;
        }
        Ok(snapshot)
    }

    pub fn validate_generation_snapshot(
        config: &Config,
        generation_root: &Path,
        expected_base_generation: &str,
    ) -> Result<String> {
        let snapshot = Self::validate_embedded_generation_snapshot(config, generation_root)?;
        if snapshot.lineage_base_generation.as_deref() != Some(expected_base_generation) {
            return Err(Error::new(
                "generation source snapshot is not bound to the exact base generation",
            ));
        }
        snapshot.verify_parent_source(config, expected_base_generation)?;
        Ok(snapshot.source_id)
    }

    pub fn validate_retained_generation_snapshot(
        config: &Config,
        generation_root: &Path,
        expected_base_generation: &str,
    ) -> Result<String> {
        let snapshot = Self::validate_embedded_generation_snapshot(config, generation_root)?;
        if snapshot.lineage_base_generation.as_deref() != Some(expected_base_generation) {
            return Err(Error::new(
                "retained generation source snapshot has wrong base generation lineage",
            ));
        }
        Ok(snapshot.source_id)
    }

    pub fn validate_snapshot_against_existing_base(
        config: &Config,
        generation_root: &Path,
    ) -> Result<String> {
        let snapshot = Self::validate_embedded_generation_snapshot(config, generation_root)?;
        let base = snapshot
            .lineage_base_generation
            .clone()
            .ok_or_else(|| Error::new("derived source snapshot lacks base generation lineage"))?;
        snapshot.verify_parent_source(config, &base)?;
        Ok(snapshot.source_id)
    }

    pub fn validate_embedded_generation_snapshot(
        config: &Config,
        generation_root: &Path,
    ) -> Result<Self> {
        Self::load_for_generation(config, generation_root, false)
    }

    fn verify_parent_source(&self, config: &Config, expected_base_generation: &str) -> Result<()> {
        let base_root = config.roots.releases.join(expected_base_generation);
        let base_is_derived = base_root.join("source-snapshot").is_dir();
        let base_snapshot = Self::load_for_generation(config, &base_root, !base_is_derived)?;
        if self.lineage_parent_source_id.as_deref() != Some(base_snapshot.source_id.as_str()) {
            return Err(Error::new(
                "generation source snapshot parent does not match the exact base source ID",
            ));
        }
        Ok(())
    }

    pub fn copy_source_tree(&self, destination: &Path) -> Result<()> {
        for entry in self
            .entries
            .values()
            .filter(|entry| entry.path.starts_with("source/"))
        {
            let relative = entry.path.strip_prefix("source/").unwrap_or_default();
            let output = destination.join(validate_relative_signed(relative)?);
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
            }
            let bytes = self.verified(entry)?;
            fs::write(&output, bytes)?;
            let mode = if entry.mode == "0755" { 0o755 } else { 0o644 };
            fs::set_permissions(output, fs::Permissions::from_mode(mode))?;
        }
        Ok(())
    }

    fn verify_root_layout(config: &Config) -> Result<()> {
        let expected_vendor = fs::canonicalize(config.source.root.join("vendor"))?;
        if fs::canonicalize(&config.source.vendor)? != expected_vendor {
            return Err(Error::new(
                "configured vendor root differs from the signed source-bundle vendor root",
            ));
        }
        Ok(())
    }

    /// Re-read every signed vendor byte and mode, reject missing/extra entries, and return an
    /// exact canonical attestation. Callers invoke this both before and after untrusted builds.
    pub fn verify_vendor_tree(&self) -> Result<String> {
        let mut actual = BTreeSet::new();
        collect_inventory_paths(
            &self.vendor_root.join("vendor"),
            &self.vendor_root,
            &mut actual,
        )?;
        let expected = self.vendor_entries.keys().cloned().collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(Error::new(
                "signed vendor membership changed or contains an unrecorded file",
            ));
        }
        let mut evidence = Vec::new();
        for (path, entry) in &self.vendor_entries {
            let absolute = self.vendor_root.join(validate_relative_signed(path)?);
            let metadata = fs::symlink_metadata(&absolute)?;
            let expected_mode = if entry.mode == "0755" { 0o755 } else { 0o644 };
            let bytes = read_regular(&absolute, entry.size)?;
            if metadata.mode() & 0o777 != expected_mode
                || metadata.len() != entry.size
                || sha256(&bytes) != entry.sha256
                || entry.origin != "operator_vendored_cargo"
            {
                return Err(Error::new("signed vendor byte or mode attestation failed"));
            }
            evidence.extend_from_slice(path.as_bytes());
            evidence.push(0);
            evidence.extend_from_slice(entry.mode.as_bytes());
            evidence.push(0);
            evidence.extend_from_slice(entry.sha256.as_bytes());
            evidence.push(b'\n');
        }
        Ok(sha256(&evidence))
    }

    fn verify_source_tree_exact(&self) -> Result<()> {
        let mut actual = BTreeSet::new();
        collect_inventory_paths(&self.root.join("source"), &self.root, &mut actual)?;
        let expected = self.entries.keys().cloned().collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(Error::new("generation source snapshot membership failed"));
        }
        for entry in self.entries.values() {
            let metadata = fs::symlink_metadata(self.root.join(&entry.path))?;
            let expected_mode = if entry.mode == "0755" { 0o555 } else { 0o444 };
            if metadata.mode() & 0o777 != expected_mode {
                return Err(Error::new("generation source snapshot mode failed"));
            }
            let _ = self.verified(entry)?;
        }
        Ok(())
    }

    /// Emit the cumulative source tree that results from this exact candidate. The immutable
    /// signing key authenticates lineage and inventory; no source body is written to public
    /// telemetry or candidate receipts.
    pub fn export_derived(
        &self,
        config: &Config,
        candidate: &Candidate,
        candidate_source: &Path,
        destination: &Path,
    ) -> Result<String> {
        if destination.exists() || destination.is_symlink() {
            return Err(Error::new(
                "derived source snapshot destination already exists",
            ));
        }
        fs::create_dir(destination)?;
        let source_output = destination.join("source");
        fs::create_dir(&source_output)?;
        let mut files = Vec::with_capacity(self.entries.len());
        let mut total = 0_u64;
        for entry in self.entries.values() {
            let relative = entry.path.strip_prefix("source/").ok_or_else(|| {
                Error::new("source snapshot inventory contains a non-source entry")
            })?;
            let input = candidate_source.join(validate_relative_signed(relative)?);
            let metadata = fs::symlink_metadata(&input)?;
            if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.nlink() != 1 {
                return Err(Error::new(
                    "candidate source contains linked or special content",
                ));
            }
            let bytes = read_regular(&input, entry.size.max(metadata.len()))?;
            let mode = if metadata.mode() & 0o111 != 0 {
                "0755"
            } else {
                "0644"
            };
            let derived = SourceFile {
                path: entry.path.clone(),
                origin: entry.origin.clone(),
                mode: mode.to_owned(),
                size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                sha256: sha256(&bytes),
            };
            total = total
                .checked_add(derived.size)
                .ok_or_else(|| Error::new("derived source byte total overflow"))?;
            let output = destination.join(validate_relative_signed(&derived.path)?);
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&output, &bytes)?;
            fs::set_permissions(
                &output,
                fs::Permissions::from_mode(if mode == "0755" { 0o755 } else { 0o644 }),
            )?;
            files.push(derived);
        }
        let identity = DerivedSourceIdentity {
            schema: DERIVED_SOURCE_SCHEMA,
            parent_source_id: &self.source_id,
            base_generation: &candidate.base_generation,
            repository_commit: &self.repository_commit,
            vendor_attestation_sha256: &self.vendor_attestation_sha256,
            files: &files,
        };
        let source_id = format!("cpu-edge:{}", sha256(&canonical_json(&identity)?));
        let manifest = DerivedSourceManifest {
            schema: DERIVED_SOURCE_SCHEMA.to_owned(),
            source_id: source_id.clone(),
            parent_source_id: self.source_id.clone(),
            base_generation: candidate.base_generation.clone(),
            repository_commit: self.repository_commit.clone(),
            vendor_attestation_sha256: self.vendor_attestation_sha256.clone(),
            file_count: files.len(),
            uncompressed_bytes: total,
            files,
        };
        let manifest_bytes = canonical_json(&manifest)?;
        let key = source_signing_key(config)?;
        let signature = DerivedSourceSignature {
            schema: DERIVED_SIGNATURE_SCHEMA.to_owned(),
            mode: "hmac-sha256".to_owned(),
            key_id: sha256(&key)[..16].to_owned(),
            manifest_sha256: sha256(&manifest_bytes),
            hmac_sha256: hmac_sha256(&key, &manifest_bytes),
        };
        fs::write(destination.join("MANIFEST.json"), &manifest_bytes)?;
        fs::write(
            destination.join("MANIFEST.signature.json"),
            canonical_json(&signature)?,
        )?;
        make_snapshot_read_only(destination)?;
        let derived = Self::load_derived(config, destination, self.clone())?;
        if derived.source_id != source_id {
            return Err(Error::new(
                "derived source snapshot self-verification failed",
            ));
        }
        Ok(source_id)
    }

    fn load_bound_patch(&self, config: &Config, candidate: &Candidate) -> Result<PatchBundle> {
        let patch_path = config
            .roots
            .candidate_store
            .join(format!("candidate-patch-{}.json", candidate.patch_sha256));
        let patch_bytes = read_regular(&patch_path, config.policy.maximum_candidate_bytes)?;
        if sha256(&canonical_json(&serde_json::from_slice::<Value>(
            &patch_bytes,
        )?)?)
            != candidate.patch_sha256
        {
            return Err(Error::new("candidate patch hash mismatch"));
        }
        let patch: PatchBundle = serde_json::from_slice(&patch_bytes)?;
        if patch.schema != PATCH_SCHEMA
            || patch.candidate_id != candidate.candidate_id
            || patch.source_id != self.source_id
            || patch.base_generation != candidate.base_generation
            || patch.files.is_empty()
            || patch.files.len() > config.policy.maximum_files
        {
            return Err(Error::new("candidate patch binding or bounds failed"));
        }
        Ok(patch)
    }

    pub fn validate_and_apply(
        &self,
        config: &Config,
        candidate: &Candidate,
        root: &Path,
    ) -> Result<PatchBundle> {
        candidate.validate(config)?;
        let patch = self.load_bound_patch(config, candidate)?;
        let paths = patch
            .files
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        if paths != candidate.changed_paths {
            return Err(Error::new(
                "candidate changed paths do not exactly match patch order",
            ));
        }
        let mut unique = BTreeSet::new();
        let mut changed_lines = 0_usize;
        let mut replacements = BTreeMap::new();
        for file in &patch.files {
            validate_mutable_path(&file.path)?;
            if !unique.insert(&file.path)
                || !valid_hex64(&file.source_sha256)
                || !valid_hex64(&file.content_sha256)
            {
                return Err(Error::new(
                    "duplicate path or malformed candidate file hash",
                ));
            }
            let entry = self
                .entries
                .get(&format!("source/{}", file.path))
                .ok_or_else(|| Error::new("candidate path is absent from signed source"))?;
            if !mutable_origin_matches(&file.path, &entry.origin) {
                return Err(Error::new(
                    "candidate path is signed as build-required or proposal-only, not mutable",
                ));
            }
            let original = self.verified(entry)?;
            if entry.sha256 != file.source_sha256
                || sha256(file.content.as_bytes()) != file.content_sha256
                || file.content.len() > MAX_MUTABLE_FILE_BYTES
                || candidate_text_is_ambiguous(&file.content)
            {
                return Err(Error::new(
                    "candidate replacement source/content hash failed",
                ));
            }
            if file.path.starts_with("capsules/astralis/astrid-capsule-")
                && file.path.ends_with("/Capsule.toml")
            {
                crate::invariant::validate_capsule_authority_update(
                    &file.path,
                    &original,
                    file.content.as_bytes(),
                )?;
            }
            let original_text = std::str::from_utf8(&original)
                .map_err(|_| Error::new("mutable source is not UTF-8"))?;
            let remaining = config
                .policy
                .maximum_changed_lines
                .saturating_sub(changed_lines);
            changed_lines = changed_lines.saturating_add(bounded_changed_lines(
                original_text,
                &file.content,
                remaining,
            ));
            replacements.insert(format!("source/{}", file.path), file.content.clone());
            validate_service_envelope(&file.path, &file.content, &config.roots.active_link)?;
            let output = root.join(validate_relative(&file.path)?);
            let metadata = fs::symlink_metadata(&output)?;
            if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.nlink() != 1 {
                return Err(Error::new("candidate destination is linked or special"));
            }
            fs::write(&output, file.content.as_bytes())?;
        }
        if changed_lines > config.policy.maximum_changed_lines {
            return Err(Error::new(
                "candidate exceeds 4,000 conservative changed lines",
            ));
        }
        self.validate_dependency_changes(&replacements)?;
        Ok(patch)
    }

    pub fn verify_post_build_tree(
        &self,
        root: &Path,
        patch: &PatchBundle,
        cargo_policy: &[u8],
    ) -> Result<()> {
        let replacements = patch
            .files
            .iter()
            .map(|file| (file.path.as_str(), file.content.as_bytes()))
            .collect::<BTreeMap<_, _>>();
        let mut expected_paths = BTreeSet::new();
        for entry in self
            .entries
            .values()
            .filter(|entry| entry.path.starts_with("source/"))
        {
            let relative = entry.path.strip_prefix("source/").unwrap_or_default();
            expected_paths.insert(relative.to_owned());
            let file_path = root.join(validate_relative_signed(relative)?);
            let metadata = fs::symlink_metadata(&file_path)?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.nlink() != 1
                || metadata.uid() != 0
                || metadata.mode() & 0o222 != 0
            {
                return Err(Error::new(
                    "post-build source is linked, mutable, or not root-owned",
                ));
            }
            let actual = read_regular(&file_path, 4 * 1024 * 1024 * 1024_u64)?;
            let expected = if relative == ".cargo/config.toml" {
                cargo_policy
            } else if let Some(replacement) = replacements.get(relative) {
                replacement
            } else {
                let baseline = self
                    .entries
                    .get(&entry.path)
                    .ok_or_else(|| Error::new("signed source inventory changed"))?;
                if sha256(&actual) == baseline.sha256 {
                    continue;
                }
                return Err(Error::new(
                    "undeclared source mutation detected after candidate tests",
                ));
            };
            if actual != expected {
                return Err(Error::new("declared source changed after candidate tests"));
            }
        }
        let mut actual_paths = BTreeSet::new();
        collect_regular_paths(root, root, &mut actual_paths)?;
        if actual_paths != expected_paths {
            return Err(Error::new("candidate test added or removed source files"));
        }
        Ok(())
    }

    fn discover_local_packages(&self) -> Result<BTreeSet<String>> {
        let mut result = BTreeSet::new();
        for entry in self
            .entries
            .values()
            .filter(|entry| entry.path.ends_with("Cargo.toml") && entry.path.starts_with("source/"))
        {
            let text = String::from_utf8(self.verified(entry)?)
                .map_err(|_| Error::new("source manifest is not UTF-8"))?;
            let document = text
                .parse::<toml::Value>()
                .map_err(|_| Error::new("source Cargo.toml is malformed"))?;
            if let Some(name) = document
                .get("package")
                .and_then(|value| value.get("name"))
                .and_then(toml::Value::as_str)
            {
                result.insert(name.to_owned());
            }
        }
        Ok(result)
    }

    fn validate_dependency_changes(&self, replacements: &BTreeMap<String, String>) -> Result<()> {
        for (path, content) in replacements {
            if path.ends_with("Cargo.toml") {
                let document = content
                    .parse::<toml::Value>()
                    .map_err(|_| Error::new("candidate Cargo.toml is malformed"))?;
                reject_manifest_source_overrides(&document)?;
                let mut tables = Vec::new();
                dependency_tables(&document, &mut tables);
                for table in tables {
                    for (alias, value) in table {
                        let dependency = dependency_identity(alias, value)?;
                        if let Some(relative) = dependency.path {
                            self.validate_local_dependency_path(path, dependency.name, relative)?;
                        } else if dependency.workspace {
                            if !self.local_packages.contains(dependency.name)
                                && !self.vendor_versions.contains_key(dependency.name)
                            {
                                return Err(Error::new(
                                    "candidate workspace dependency is absent from signed source",
                                ));
                            }
                        } else {
                            let requirement =
                                VersionReq::parse(dependency.version.ok_or_else(|| {
                                    Error::new("external dependency omitted version")
                                })?)
                                .map_err(|_| {
                                    Error::new("external dependency version is malformed")
                                })?;
                            if !self
                                .vendor_versions
                                .get(dependency.name)
                                .is_some_and(|versions| {
                                    versions.iter().any(|item| requirement.matches(item))
                                })
                            {
                                return Err(Error::new(
                                    "candidate dependency is absent from signed vendor",
                                ));
                            }
                        }
                    }
                }
            } else if path.ends_with("Cargo.lock") {
                self.validate_lockfile(content)?;
            }
        }
        Ok(())
    }

    fn validate_lockfile(&self, content: &str) -> Result<()> {
        if content.len() > MAX_MUTABLE_FILE_BYTES || content.lines().count() > MAX_LOCKFILE_LINES {
            return Err(Error::new("candidate Cargo.lock exceeds immutable bounds"));
        }
        let document = content
            .parse::<toml::Value>()
            .map_err(|_| Error::new("candidate Cargo.lock is malformed"))?;
        if !matches!(
            document.get("version").and_then(toml::Value::as_integer),
            Some(3 | 4)
        ) {
            return Err(Error::new("candidate Cargo.lock version is unsupported"));
        }
        let packages = document
            .get("package")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| Error::new("candidate Cargo.lock has no package inventory"))?;
        if packages.len() > 4_096 {
            return Err(Error::new(
                "candidate Cargo.lock package inventory is oversized",
            ));
        }
        let mut unique = BTreeSet::new();
        for package in packages {
            let name = package
                .get("name")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| Error::new("lock package omitted name"))?;
            let version = package
                .get("version")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| Error::new("lock package omitted version"))?;
            if let Some(source) = package.get("source") {
                let source = source
                    .as_str()
                    .ok_or_else(|| Error::new("lock package source is malformed"))?;
                if !(source.starts_with("registry+") || source.starts_with("sparse+")) {
                    return Err(Error::new(
                        "lock package source is outside the signed registry/vendor policy",
                    ));
                }
                let version = Version::parse(version)
                    .map_err(|_| Error::new("lock package version is malformed"))?;
                let checksum = package
                    .get("checksum")
                    .and_then(toml::Value::as_str)
                    .ok_or_else(|| Error::new("lock package omitted its signed checksum"))?;
                if !valid_hex64(checksum)
                    || self
                        .vendor_checksums
                        .get(&(name.to_owned(), version.clone()))
                        .and_then(Option::as_deref)
                        != Some(checksum)
                {
                    return Err(Error::new(
                        "lock package version or checksum is absent from signed vendor inventory",
                    ));
                }
                if !unique.insert((name.to_owned(), version, source.to_owned())) {
                    return Err(Error::new(
                        "candidate Cargo.lock repeats a package identity",
                    ));
                }
            } else if !self.local_packages.contains(name) {
                return Err(Error::new(
                    "lock package without a registry source is not signed local source",
                ));
            }
        }
        Ok(())
    }

    fn validate_local_dependency_path(
        &self,
        manifest_path: &str,
        package_name: &str,
        dependency_path: &str,
    ) -> Result<()> {
        let relative = validate_cargo_path(dependency_path)?;
        let manifest_directory = Path::new(manifest_path)
            .parent()
            .ok_or_else(|| Error::new("candidate Cargo.toml has no parent"))?;
        let joined = normalize_relative_join(manifest_directory, &relative)?;
        let dependency_manifest = format!("source/{}/Cargo.toml", joined.to_string_lossy());
        let entry = self
            .entries
            .get(&dependency_manifest)
            .ok_or_else(|| Error::new("candidate path dependency is absent from signed source"))?;
        let text = String::from_utf8(self.verified(entry)?)
            .map_err(|_| Error::new("signed path dependency manifest is not UTF-8"))?;
        let document = text
            .parse::<toml::Value>()
            .map_err(|_| Error::new("signed path dependency manifest is malformed"))?;
        let actual_name = document
            .get("package")
            .and_then(|value| value.get("name"))
            .and_then(toml::Value::as_str)
            .ok_or_else(|| Error::new("signed path dependency has no package identity"))?;
        if actual_name != package_name {
            return Err(Error::new(
                "candidate path dependency package alias does not match signed source",
            ));
        }
        Ok(())
    }

    fn verified(&self, entry: &SourceFile) -> Result<Vec<u8>> {
        let path = self.root.join(validate_relative_signed(&entry.path)?);
        let bytes = read_regular(&path, entry.size)?;
        if bytes.len() as u64 != entry.size || sha256(&bytes) != entry.sha256 {
            return Err(Error::new("signed source inventory changed"));
        }
        Ok(bytes)
    }

    fn verify_required_build_closure(&self) -> Result<()> {
        for capsule in EDGE_CAPSULES {
            let lock = format!("source/capsules/astralis/{capsule}/Cargo.lock");
            if !self.entries.contains_key(&lock) {
                return Err(Error::new(
                    "signed source omits a required edge capsule lockfile",
                ));
            }
        }
        for service in EDGE_STANDALONE_SERVICES {
            let lock = format!("source/services/{service}/Cargo.lock");
            if !self.entries.contains_key(&lock) {
                return Err(Error::new(
                    "signed source omits a required edge service lockfile",
                ));
            }
        }
        let kernel = self.verified(
            self.entries
                .get(QUICKJS_KERNEL_PATH)
                .ok_or_else(|| Error::new("signed source omits the QuickJS kernel"))?,
        )?;
        if kernel.len() < 8 || &kernel[..8] != b"\0asm\x01\0\0\0" {
            return Err(Error::new(
                "signed QuickJS kernel has an invalid WASM header",
            ));
        }
        let kernel_hash = self.verified(
            self.entries
                .get(QUICKJS_KERNEL_HASH_PATH)
                .ok_or_else(|| Error::new("signed source omits the QuickJS kernel hash"))?,
        )?;
        let kernel_hash = std::str::from_utf8(&kernel_hash)
            .map_err(|_| Error::new("QuickJS kernel BLAKE3 record is not UTF-8"))?;
        let trimmed = kernel_hash.strip_suffix('\n').unwrap_or(kernel_hash);
        let Some((digest, basename)) = trimmed.split_once("  ") else {
            return Err(Error::new("QuickJS kernel BLAKE3 record is malformed"));
        };
        if !valid_hex64(digest) || basename != "engine.wasm" {
            return Err(Error::new("QuickJS kernel BLAKE3 record is malformed"));
        }
        Ok(())
    }
}

fn collect_regular_paths(
    root: &Path,
    directory: &Path,
    output: &mut BTreeSet<String>,
) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::new("candidate source contains a symlink"));
        }
        if metadata.is_dir() {
            if metadata.uid() != 0 || metadata.mode() & 0o222 != 0 {
                return Err(Error::new(
                    "candidate source directory is mutable or not root-owned",
                ));
            }
            collect_regular_paths(root, &path, output)?;
        } else if metadata.is_file() && metadata.nlink() == 1 {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| Error::new("candidate source path escape"))?
                .to_string_lossy()
                .replace('\\', "/");
            validate_relative_signed(&relative)?;
            output.insert(relative);
        } else {
            return Err(Error::new(
                "candidate source contains a linked or special file",
            ));
        }
    }
    Ok(())
}

fn collect_inventory_paths(
    root: &Path,
    inventory_root: &Path,
    output: &mut BTreeSet<String>,
) -> Result<()> {
    fn visit(root: &Path, directory: &Path, output: &mut BTreeSet<String>) -> Result<()> {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(Error::new("signed inventory contains a symlink"));
            }
            if metadata.is_dir() {
                visit(root, &path, output)?;
            } else if metadata.is_file() && metadata.nlink() == 1 {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|_| Error::new("signed inventory path escape"))?
                    .to_string_lossy()
                    .replace('\\', "/");
                validate_relative_signed(&relative)?;
                if !output.insert(relative) {
                    return Err(Error::new("signed inventory path is duplicated"));
                }
            } else {
                return Err(Error::new(
                    "signed inventory contains a linked or special file",
                ));
            }
        }
        Ok(())
    }

    let metadata = fs::symlink_metadata(root)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Error::new("signed inventory root is unavailable or linked"));
    }
    visit(inventory_root, root, output)
}

fn make_snapshot_read_only(root: &Path) -> Result<()> {
    fn visit(path: &Path) -> Result<()> {
        for entry in fs::read_dir(path)? {
            let child = entry?.path();
            let metadata = fs::symlink_metadata(&child)?;
            if metadata.file_type().is_symlink() {
                return Err(Error::new("derived source snapshot contains a symlink"));
            }
            if metadata.is_dir() {
                visit(&child)?;
                fs::set_permissions(&child, fs::Permissions::from_mode(0o555))?;
            } else if metadata.is_file() && metadata.nlink() == 1 {
                let mode = if metadata.mode() & 0o111 != 0 {
                    0o555
                } else {
                    0o444
                };
                fs::set_permissions(&child, fs::Permissions::from_mode(mode))?;
            } else {
                return Err(Error::new(
                    "derived source snapshot contains special content",
                ));
            }
        }
        Ok(())
    }
    visit(root)?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o555))?;
    Ok(())
}

fn valid_source_origin(path: &str, origin: &str) -> bool {
    if path.starts_with("vendor/") {
        return origin == "operator_vendored_cargo";
    }
    if path == "rustc-version.txt" {
        return origin == "operator_supplied_toolchain_metadata";
    }
    if !path.starts_with("source/") {
        return false;
    }
    let source_path = path.strip_prefix("source/").unwrap_or_default();
    if is_inspect_only_immutable_source(source_path) {
        return origin == INSPECT_ONLY_ORIGIN;
    }
    if origin == INSPECT_ONLY_ORIGIN {
        return false;
    }
    if source_path.starts_with("packaging/systemd/") {
        return if crate::invariant::is_mutable_unit_path(source_path) {
            origin == "mutable_astrid_service_template"
        } else {
            origin == "build_required_service_template"
        };
    }
    if matches!(
        origin,
        "mutable_astrid_service_template" | "build_required_service_template"
    ) {
        return false;
    }
    matches!(
        origin,
        "mutable_build_manifest"
            | "mutable_core_source"
            | "mutable_edge_runtime"
            | "mutable_edge_capsule"
            | "mutable_capsule_manifest"
            | "mutable_edge_report"
            | "mutable_appliance_profile"
            | "build_required_manifest"
            | "build_required_immutable"
            | "build_required_runtime_script"
    )
}

fn is_inspect_only_immutable_source(path: &str) -> bool {
    for prefix in [
        "services/astrid-edge-steward-helper/",
        "services/astrid-edge-rescue-helper/",
        "services/astrid-edge-web-broker/",
        "services/astrid-edge-provider-broker/",
        "services/astrid-edge-presentation-broker/",
        "services/astrid-edge-checkpoint/",
    ] {
        if let Some(relative) = path.strip_prefix(prefix) {
            return matches!(relative, "Cargo.toml" | "Cargo.lock")
                || (relative.starts_with("src/")
                    && Path::new(relative)
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("rs")));
        }
    }
    if path.starts_with("scripts/edge_self_change/") {
        return Path::new(path)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("py"));
    }
    if matches!(
        path,
        "scripts/build_edge_self_change_source_bundle.py"
            | "scripts/build_edge_self_change_supervisor_zipapp.py"
            | "scripts/build_edge_self_change_toolchain_bundle.py"
            | "scripts/astrid_train.py"
            | "scripts/edge_audio_feeder.py"
            | "scripts/edge_hindsight.py"
            | "scripts/edge_self_change_supervisor.py"
            | "scripts/install_edge_self_evolution_root.sh"
            | "scripts/test_build_edge_self_change_source_bundle.py"
            | "scripts/test_build_edge_self_change_supervisor_zipapp.py"
            | "scripts/test_build_edge_self_change_toolchain_bundle.py"
            | "scripts/test_edge_audio_feeder.py"
            | "scripts/test_edge_builder_store.py"
            | "scripts/test_edge_probation_health_systemd.py"
            | "scripts/test_edge_self_change_supervisor.py"
            | "scripts/test_edge_state_store.py"
            | "scripts/test_install_edge_self_evolution_root.sh"
    ) || matches!(
        path,
        "docs/cpu-edge-self-evolution.md"
            | "packaging/headless/edge-audio-feeder.json.in"
            | "packaging/headless/edge-hindsight-writer.json.in"
    ) {
        return true;
    }
    let Some(relative) = path.strip_prefix("packaging/systemd/") else {
        return false;
    };
    let allowed_suffix = [".service", ".timer", ".socket", ".conf", ".env", ".in"]
        .iter()
        .any(|suffix| relative.ends_with(suffix));
    let exact_root_script = matches!(
        relative,
        "root/astrid-edge-builder-store"
            | "root/astrid-edge-state-store"
            | "root/astrid-edge-self-evolution-control"
            | "root/migrate-edge-user-services-to-system"
    );
    exact_root_script
        || (allowed_suffix
            && (relative.starts_with("root/")
                || [
                    "self-change",
                    "edge-steward",
                    "edge-web-broker",
                    "edge-provider",
                    "edge-presentation-broker",
                    "edge-checkpoint",
                    "builder-store",
                    "state-store",
                    "audio-feeder",
                    "generation-guard",
                    "core-liveness",
                ]
                .iter()
                .any(|marker| relative.contains(marker))))
}

impl Candidate {
    pub fn from_file(path: &Path, config: &Config) -> Result<Self> {
        let value: Value = read_json(path, config.policy.maximum_candidate_bytes)?;
        let candidate: Self = serde_json::from_value(value.clone())?;
        candidate.validate(config)?;
        if canonical_json(&candidate)? != canonical_json(&value)? {
            return Err(Error::new(
                "candidate manifest is not exact canonical schema",
            ));
        }
        Ok(candidate)
    }

    pub fn validate(&self, config: &Config) -> Result<()> {
        if self.schema != CANDIDATE_SCHEMA
            || !valid_identifier(&self.candidate_id)
            || !valid_identifier(&self.base_generation)
            || !valid_hex64(&self.proposal_sha256)
            || !valid_hex64(&self.patch_sha256)
            || self.changed_paths.is_empty()
            || self.changed_paths.len() > config.policy.maximum_files
            || self.privilege_envelope != "proposal-only:no-execution:v1"
        {
            return Err(Error::new("candidate manifest authority or bounds failed"));
        }
        let mut paths = BTreeSet::new();
        for path in &self.changed_paths {
            validate_mutable_path(path)?;
            if !paths.insert(path) {
                return Err(Error::new("candidate changed path is duplicated"));
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<String> {
        Ok(sha256(&canonical_json(self)?))
    }
}

impl BuildV1 {
    pub fn validate(&self, config: &Config) -> Result<()> {
        if self.schema != BUILD_SCHEMA
            || self.appliance_id != config.appliance_id
            || !valid_identifier(&self.build_id)
            || !valid_identifier(&self.candidate_id)
            || !valid_hex64(&self.candidate_sha256)
            || !valid_identifier(&self.base_generation)
            || !valid_identifier(&self.generation_id)
            || self.source_revision.len() < 7
            || !self
                .source_revision
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            || !valid_hex64(&self.bundle_sha256)
            || !valid_hex64(&self.tests_sha256)
            || self.target != config.target
            || self.privilege_envelope != "offline-build-sandbox:no-host-state:v1"
        {
            return Err(Error::new(
                "build manifest is not exact supervisor Build v1",
            ));
        }
        Ok(())
    }
}

pub fn validate_mutable_path(path: &str) -> Result<()> {
    let relative = validate_relative(path)?;
    let normalized = relative.to_string_lossy();
    let mutable_capsule = EDGE_CAPSULES
        .iter()
        .any(|capsule| normalized.starts_with(&format!("capsules/astralis/{capsule}/")));
    let allowed = normalized == "Cargo.toml"
        || normalized == "Cargo.lock"
        || normalized.starts_with("crates/")
        || normalized.starts_with("services/astrid-edge-runtime/")
        || mutable_capsule
        || normalized.starts_with("packaging/appliances/")
        || crate::invariant::is_mutable_unit_path(&normalized)
        || (normalized.starts_with("scripts/") && !normalized[8..].contains('/'));
    let denied = normalized.starts_with("capsules/spectral-bridge/")
        || normalized.starts_with("minime/")
        || normalized.starts_with("services/astrid-edge-steward-helper/")
        || normalized.starts_with("services/astrid-edge-rescue-helper/")
        || normalized.starts_with("services/astrid-edge-web-broker/")
        || normalized.starts_with("services/astrid-edge-provider-broker/")
        || normalized.starts_with("services/astrid-edge-presentation-broker/")
        || normalized.starts_with("services/astrid-edge-checkpoint/")
        || normalized.starts_with("scripts/edge_self_change")
        || normalized.starts_with("scripts/install_edge_self_evolution_root");
    if !allowed || denied {
        return Err(Error::new(
            "candidate path is outside mutable CPU-edge surface",
        ));
    }
    Ok(())
}

fn validate_service_envelope(path: &str, content: &str, active_link: &Path) -> Result<()> {
    if !path.starts_with("packaging/systemd/") {
        return Ok(());
    }
    if !crate::invariant::is_mutable_unit_path(path) {
        return Err(Error::new(
            "service template is outside the exact transactional fragment set",
        ));
    }
    crate::invariant::validate_unit(path, content, active_link)
}

fn mutable_origin_matches(path: &str, origin: &str) -> bool {
    if path.starts_with("services/astrid-edge-steward-helper/")
        || path.starts_with("services/astrid-edge-rescue-helper/")
        || path.starts_with("services/astrid-edge-web-broker/")
        || path.starts_with("services/astrid-edge-provider-broker/")
        || path.starts_with("services/astrid-edge-presentation-broker/")
        || path.starts_with("services/astrid-edge-checkpoint/")
    {
        return false;
    }
    let rust_source = || {
        Path::new(path)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
    };
    let capsule_source = || {
        Path::new(path).extension().is_some_and(|extension| {
            ["rs", "md", "json", "toml", "txt"]
                .iter()
                .any(|allowed| extension.eq_ignore_ascii_case(allowed))
        })
    };
    match origin {
        "mutable_build_manifest" => {
            path == "Cargo.toml"
                || path == "Cargo.lock"
                || path.ends_with("/Cargo.toml")
                || (path.ends_with("/Cargo.lock")
                    && (path.starts_with("services/astrid-edge-runtime/")
                        || path.starts_with("capsules/astralis/astrid-capsule-")))
        },
        "mutable_core_source" => path.starts_with("crates/") && rust_source(),
        "mutable_edge_runtime" => {
            path.starts_with("services/astrid-edge-runtime/") && rust_source()
        },
        "mutable_edge_capsule" => {
            path.starts_with("capsules/astralis/astrid-capsule-") && capsule_source()
        },
        "mutable_capsule_manifest" => {
            path.starts_with("capsules/astralis/astrid-capsule-") && path.ends_with("Capsule.toml")
        },
        "mutable_edge_report" => path.starts_with("scripts/") && !path[8..].contains('/'),
        "mutable_appliance_profile" => path.starts_with("packaging/appliances/"),
        "mutable_astrid_service_template" => crate::invariant::is_mutable_unit_path(path),
        _ => false,
    }
}

fn candidate_text_is_ambiguous(content: &str) -> bool {
    content.chars().any(|character| {
        (character.is_control() && !matches!(character, '\n' | '\t'))
            || matches!(character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
    })
}

/// Bounded Myers line edit distance. Insertions and deletions count one line;
/// a replacement therefore counts two. Work stops at `limit + 1`.
pub(crate) fn bounded_changed_lines(original: &str, replacement: &str, limit: usize) -> usize {
    if original == replacement {
        return 0;
    }
    let before = original.lines().collect::<Vec<_>>();
    let after = replacement.lines().collect::<Vec<_>>();
    let prefix = before
        .iter()
        .zip(&after)
        .take_while(|(left, right)| left == right)
        .count();
    let mut before_end = before.len();
    let mut after_end = after.len();
    while before_end > prefix
        && after_end > prefix
        && before[before_end.saturating_sub(1)] == after[after_end.saturating_sub(1)]
    {
        before_end = before_end.saturating_sub(1);
        after_end = after_end.saturating_sub(1);
    }
    let before = &before[prefix..before_end];
    let after = &after[prefix..after_end];
    if before.len().abs_diff(after.len()) > limit {
        return limit.saturating_add(1);
    }
    let maximum = before.len().saturating_add(after.len()).min(limit);
    let offset = maximum.saturating_add(1);
    let mut furthest = vec![0_usize; maximum.saturating_mul(2).saturating_add(3)];
    for distance in 0..=maximum {
        let distance_signed = isize::try_from(distance).unwrap_or(isize::MAX);
        let minimum_diagonal = distance_signed.checked_neg().unwrap_or(isize::MIN);
        let mut diagonal = minimum_diagonal;
        while diagonal <= distance_signed {
            let index_signed =
                diagonal.saturating_add(isize::try_from(offset).unwrap_or(isize::MAX));
            let Ok(index) = usize::try_from(index_signed) else {
                return limit.saturating_add(1);
            };
            let mut x = if diagonal == minimum_diagonal
                || (diagonal != distance_signed
                    && furthest[index.saturating_sub(1)] < furthest[index.saturating_add(1)])
            {
                furthest[index.saturating_add(1)]
            } else {
                furthest[index.saturating_sub(1)].saturating_add(1)
            };
            let Some(mut y) = isize::try_from(x)
                .ok()
                .and_then(|value| value.checked_sub(diagonal))
                .and_then(|value| usize::try_from(value).ok())
            else {
                return limit.saturating_add(1);
            };
            while x < before.len() && y < after.len() && before[x] == after[y] {
                x = x.saturating_add(1);
                y = y.saturating_add(1);
            }
            furthest[index] = x;
            if x == before.len() && y == after.len() {
                return distance;
            }
            diagonal = diagonal.saturating_add(2);
        }
    }
    limit.saturating_add(1)
}

fn dependency_tables<'a>(
    value: &'a toml::Value,
    output: &mut Vec<&'a toml::map::Map<String, toml::Value>>,
) {
    let Some(table) = value.as_table() else {
        return;
    };
    for name in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(item) = table.get(name).and_then(toml::Value::as_table) {
            output.push(item);
        }
    }
    if let Some(item) = table
        .get("workspace")
        .and_then(toml::Value::as_table)
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table)
    {
        output.push(item);
    }
    if let Some(targets) = table.get("target").and_then(toml::Value::as_table) {
        for target in targets.values() {
            dependency_tables(target, output);
        }
    }
}

struct DependencyIdentity<'a> {
    name: &'a str,
    version: Option<&'a str>,
    path: Option<&'a str>,
    workspace: bool,
}

fn dependency_identity<'a>(
    alias: &'a str,
    value: &'a toml::Value,
) -> Result<DependencyIdentity<'a>> {
    match value {
        toml::Value::String(version) => Ok(DependencyIdentity {
            name: alias,
            version: Some(version),
            path: None,
            workspace: false,
        }),
        toml::Value::Table(table) => {
            if [
                "git",
                "registry",
                "registry-index",
                "source",
                "branch",
                "tag",
                "rev",
            ]
            .iter()
            .any(|key| table.contains_key(*key))
            {
                return Err(Error::new(
                    "git, registry, or revision dependency source is forbidden",
                ));
            }
            let path = table
                .get("path")
                .map(|value| {
                    value
                        .as_str()
                        .ok_or_else(|| Error::new("candidate dependency path is not a string"))
                })
                .transpose()?;
            let workspace = table
                .get("workspace")
                .is_some_and(|value| value.as_bool() == Some(true));
            if table.get("workspace").is_some() && !workspace {
                return Err(Error::new(
                    "candidate workspace dependency marker is invalid",
                ));
            }
            if path.is_some() && workspace {
                return Err(Error::new(
                    "candidate dependency cannot combine path and workspace sources",
                ));
            }
            if table.get("package").is_some_and(|value| !value.is_str())
                || table.get("version").is_some_and(|value| !value.is_str())
                || (workspace && table.contains_key("version"))
            {
                return Err(Error::new(
                    "candidate dependency source identity is malformed or ambiguous",
                ));
            }
            Ok(DependencyIdentity {
                name: table
                    .get("package")
                    .and_then(toml::Value::as_str)
                    .unwrap_or(alias),
                version: table.get("version").and_then(toml::Value::as_str),
                path,
                workspace,
            })
        },
        _ => Err(Error::new("unsupported dependency declaration")),
    }
}

fn reject_manifest_source_overrides(document: &toml::Value) -> Result<()> {
    let Some(root) = document.as_table() else {
        return Err(Error::new("candidate Cargo.toml root is not a table"));
    };
    for key in ["patch", "replace", "source"] {
        if root.contains_key(key) {
            return Err(Error::new(
                "candidate Cargo source/patch/replace override is forbidden",
            ));
        }
    }
    Ok(())
}

fn validate_cargo_path(value: &str) -> Result<PathBuf> {
    if value.is_empty()
        || value.contains(['\\', '\0'])
        || value.starts_with('/')
        || value.split('/').any(|part| part.is_empty() || part == ".")
    {
        return Err(Error::new(
            "candidate path dependency is not canonical relative source",
        ));
    }
    Ok(PathBuf::from(value))
}

fn normalize_relative_join(base: &Path, relative: &Path) -> Result<PathBuf> {
    let mut output = base
        .components()
        .map(|item| item.as_os_str().to_owned())
        .collect::<Vec<_>>();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(item) => output.push(item.to_owned()),
            std::path::Component::ParentDir => {
                if output.pop().is_none() {
                    return Err(Error::new(
                        "candidate path dependency escapes signed source",
                    ));
                }
            },
            _ => return Err(Error::new("candidate path dependency is noncanonical")),
        }
    }
    let joined = output.into_iter().collect::<PathBuf>();
    let text = joined.to_string_lossy().replace('\\', "/");
    // `base` is repository-relative (for example `crates/astrid-kernel`), not rooted beneath
    // the source-bundle's outer `source/` directory.  Requiring a literal `source/` prefix here
    // would reject every legitimate local dependency.  The stack discipline above is the escape
    // check; the caller adds the one trusted `source/` inventory prefix after normalization.
    validate_relative_signed(&text)
}

fn source_signing_key(config: &Config) -> Result<Vec<u8>> {
    let key = read_regular(&config.source.signing_key, 4_096)?;
    if key.len() < 32 {
        return Err(Error::new(
            "immutable source signing key is shorter than 256 bits",
        ));
    }
    Ok(key)
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> String {
    let mut normalized = [0_u8; 64];
    if key.len() > 64 {
        normalized[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        normalized[..key.len()].copy_from_slice(key);
    }
    let mut inner_pad = [0x36_u8; 64];
    let mut outer_pad = [0x5c_u8; 64];
    for index in 0..64 {
        inner_pad[index] ^= normalized[index];
        outer_pad[index] ^= normalized[index];
    }
    let inner = Sha256::new()
        .chain_update(inner_pad)
        .chain_update(message)
        .finalize();
    format!(
        "{:x}",
        Sha256::new()
            .chain_update(outer_pad)
            .chain_update(inner)
            .finalize()
    )
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
            == 0
}

#[cfg(test)]
mod tests {
    use super::{
        EDGE_CAPSULES, EDGE_STANDALONE_SERVICES, INSPECT_ONLY_ORIGIN, QUICKJS_KERNEL_HASH_PATH,
        QUICKJS_KERNEL_PATH, SourceSnapshot, bounded_changed_lines, mutable_origin_matches,
        normalize_relative_join, valid_source_origin, validate_cargo_path, validate_mutable_path,
        validate_service_envelope,
    };
    use semver::Version;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn immutable_changed_line_metric_matches_bounded_edit_semantics() {
        assert_eq!(bounded_changed_lines("a\n", "b\n", 4_000), 2);
        assert_eq!(bounded_changed_lines("a\nb\n", "a\nx\nb\n", 4_000), 1);
        assert_eq!(bounded_changed_lines("a\nx\nb\n", "a\nb\n", 4_000), 1);
        assert_eq!(bounded_changed_lines("a\nb\n", "b\na\n", 4_000), 2);
        let large = "stable\n".repeat(12_000);
        let changed = large.replacen("stable\n", "changed\n", 1);
        assert_eq!(bounded_changed_lines(&large, &changed, 4_000), 2);
        assert_eq!(
            bounded_changed_lines(&large, &"different\n".repeat(12_000), 4_000),
            4_001
        );
    }

    #[test]
    fn mutable_surface_excludes_rescue_and_peer_domains() {
        assert!(validate_mutable_path("Cargo.toml").is_ok());
        assert!(validate_mutable_path("Cargo.lock").is_ok());
        assert!(mutable_origin_matches(
            "Cargo.lock",
            "mutable_build_manifest"
        ));
        assert!(validate_mutable_path("crates/astrid-kernel/src/lib.rs").is_ok());
        assert!(validate_mutable_path("crates/astrid-build/src/lib.rs").is_ok());
        assert!(validate_mutable_path("crates/astrid-openclaw/src/lib.rs").is_ok());
        assert!(validate_mutable_path("crates/astrid-prelude/src/lib.rs").is_ok());
        assert!(validate_mutable_path("crates/astrid-minime-protocol/src/lib.rs").is_ok());
        assert!(validate_mutable_path("crates/astrid-integration-tests/src/lib.rs").is_ok());
        assert!(validate_mutable_path("crates/astrid-test/src/lib.rs").is_ok());
        assert!(validate_mutable_path("minime/src/lib.rs").is_err());
        assert!(validate_mutable_path("services/astrid-edge-runtime/src/main.rs").is_ok());
        for external_capsule_source in [
            "capsules/astralis/astrid-capsule-react/src/lib.rs",
            "capsules/astralis/astrid-capsule-system/src/skills/capsule-development/SKILL.md",
            "capsules/astralis/astrid-capsule-openai-compat/Capsule.toml",
            "capsules/astralis/astrid-capsule-session/Cargo.lock",
        ] {
            assert!(validate_mutable_path(external_capsule_source).is_ok());
        }
        assert!(mutable_origin_matches(
            "capsules/astralis/astrid-capsule-system/src/skills/capsule-development/SKILL.md",
            "mutable_edge_capsule"
        ));
        for mutable_core_authority in [
            "crates/astrid-events/src/bus.rs",
            "crates/astrid-kernel/src/maintenance.rs",
            "crates/astrid-kernel/src/socket_bridge.rs",
        ] {
            assert!(validate_mutable_path(mutable_core_authority).is_ok());
            assert!(mutable_origin_matches(
                mutable_core_authority,
                "mutable_core_source"
            ));
        }
        for mutable_runtime_authority in [
            "services/astrid-edge-runtime/src/config.rs",
            "services/astrid-edge-runtime/src/ipc.rs",
            "services/astrid-edge-runtime/src/maintenance.rs",
            "services/astrid-edge-runtime/src/self_change.rs",
            "services/astrid-edge-runtime/src/self_change/state.rs",
        ] {
            assert!(validate_mutable_path(mutable_runtime_authority).is_ok());
            assert!(mutable_origin_matches(
                mutable_runtime_authority,
                "mutable_edge_runtime"
            ));
        }
        assert!(validate_mutable_path("services/astrid-edge-steward-helper/src/main.rs").is_err());
        assert!(validate_mutable_path("services/astrid-edge-rescue-helper/src/main.rs").is_err());
        assert!(validate_mutable_path("services/astrid-edge-web-broker/src/main.rs").is_err());
        assert!(validate_mutable_path("services/astrid-edge-provider-broker/src/main.rs").is_err());
        assert!(
            validate_mutable_path("services/astrid-edge-presentation-broker/src/main.rs").is_err()
        );
        assert!(validate_mutable_path("services/astrid-edge-checkpoint/src/main.rs").is_err());
        assert!(validate_mutable_path("capsules/spectral-bridge/src/main.rs").is_err());
        assert!(validate_mutable_path("../operator/.ssh/id_ed25519").is_err());
        assert!(validate_mutable_path("packaging/systemd/astrid.service").is_ok());
        assert!(validate_mutable_path("packaging/systemd/ssh.service").is_err());
        assert!(mutable_origin_matches(
            "crates/astrid-kernel/src/lib.rs",
            "mutable_core_source"
        ));
        assert!(!mutable_origin_matches(
            "crates/astrid-kernel/src/lib.rs",
            "build_required_immutable"
        ));
        assert!(mutable_origin_matches(
            "packaging/systemd/astrid.service",
            "mutable_astrid_service_template"
        ));
        assert!(!mutable_origin_matches(
            "packaging/systemd/astrid.service",
            "build_required_service_template"
        ));
        assert!(!mutable_origin_matches(
            "packaging/systemd/astrid-edge-steward.service",
            "mutable_astrid_service_template"
        ));
    }

    #[test]
    fn mutable_service_origin_is_bound_to_only_the_exact_six_fragments() {
        assert!(valid_source_origin(
            "source/packaging/systemd/astrid.service",
            "mutable_astrid_service_template"
        ));
        assert!(!valid_source_origin(
            "source/packaging/systemd/astrid.service",
            "build_required_service_template"
        ));
        assert!(!valid_source_origin(
            "source/packaging/systemd/astrid-edge-steward.service",
            "mutable_astrid_service_template"
        ));
        assert!(!valid_source_origin(
            "source/packaging/systemd/astrid-edge-steward.service",
            "build_required_service_template"
        ));
        assert!(valid_source_origin(
            "source/packaging/systemd/astrid-edge-steward.service",
            INSPECT_ONLY_ORIGIN
        ));
        assert!(!valid_source_origin(
            "source/packaging/systemd/astrid-edge-steward.service",
            "mutable_core_source"
        ));
        assert!(!valid_source_origin(
            "source/crates/astrid-kernel/src/lib.rs",
            "mutable_astrid_service_template"
        ));
        assert!(valid_source_origin(
            "source/services/astrid-edge-rescue-helper/src/main.rs",
            INSPECT_ONLY_ORIGIN
        ));
        assert!(valid_source_origin(
            "source/services/astrid-edge-provider-broker/src/main.rs",
            INSPECT_ONLY_ORIGIN
        ));
        assert!(valid_source_origin(
            "source/services/astrid-edge-presentation-broker/src/main.rs",
            INSPECT_ONLY_ORIGIN
        ));
        assert!(valid_source_origin(
            "source/packaging/systemd/astrid-edge-presentation-broker.socket.in",
            INSPECT_ONLY_ORIGIN
        ));
        assert!(valid_source_origin(
            "source/scripts/astrid_train.py",
            INSPECT_ONLY_ORIGIN
        ));
        assert!(!valid_source_origin(
            "source/scripts/astrid_train.py",
            "mutable_edge_report"
        ));
        assert!(!valid_source_origin(
            "source/services/astrid-edge-rescue-helper/src/main.rs",
            "mutable_edge_runtime"
        ));
        assert!(!mutable_origin_matches(
            "services/astrid-edge-rescue-helper/src/main.rs",
            INSPECT_ONLY_ORIGIN
        ));
        for service in EDGE_STANDALONE_SERVICES {
            let path = format!("source/services/{service}/Cargo.lock");
            let expected = if service == "astrid-edge-runtime" {
                "mutable_build_manifest"
            } else {
                INSPECT_ONLY_ORIGIN
            };
            assert!(valid_source_origin(&path, expected), "{path}");
        }
        for capsule in EDGE_CAPSULES {
            let path = format!("source/capsules/astralis/{capsule}/Cargo.lock");
            assert!(
                valid_source_origin(&path, "mutable_build_manifest"),
                "{path}"
            );
        }
        assert!(valid_source_origin(
            QUICKJS_KERNEL_PATH,
            "build_required_immutable"
        ));
        assert!(valid_source_origin(
            QUICKJS_KERNEL_HASH_PATH,
            "build_required_immutable"
        ));
    }

    #[test]
    fn privilege_expanding_units_are_rejected() {
        assert!(
            validate_service_envelope(
                "packaging/systemd/astrid.service",
                "[Service]\nUser=root\n",
                std::path::Path::new("/opt/astrid-edge/current"),
            )
            .is_err()
        );
        assert!(
            validate_service_envelope(
                "packaging/systemd/astrid.service",
                "[Service]\nNoNewPrivileges=true\nPrivateNetwork=true\n",
                std::path::Path::new("/opt/astrid-edge/current"),
            )
            .is_err()
        );
    }

    #[test]
    fn dependency_changes_must_resolve_inside_signed_vendor() {
        let mut vendor_versions = BTreeMap::new();
        vendor_versions.insert(
            "serde".to_owned(),
            BTreeSet::from([Version::parse("1.0.200").unwrap()]),
        );
        let snapshot = SourceSnapshot {
            source_id: "cpu-edge:test".into(),
            repository_commit: "abcdef1".into(),
            root: tempfile::tempdir().unwrap().keep(),
            entries: BTreeMap::new(),
            vendor_root: tempfile::tempdir().unwrap().keep(),
            vendor_entries: BTreeMap::new(),
            vendor_versions,
            vendor_checksums: BTreeMap::from([(
                ("serde".to_owned(), Version::parse("1.0.200").unwrap()),
                Some("d".repeat(64)),
            )]),
            local_packages: BTreeSet::new(),
            vendor_attestation_sha256: "0".repeat(64),
            lineage_base_generation: None,
            lineage_parent_source_id: None,
        };
        assert!(
            snapshot
                .validate_dependency_changes(&BTreeMap::from([(
                    "source/Cargo.toml".into(),
                    "[package]\nname='x'\nversion='0.1.0'\n[dependencies]\nserde='1'\n".into(),
                )]))
                .is_ok()
        );
        assert!(
            snapshot
                .validate_dependency_changes(&BTreeMap::from([(
                    "source/Cargo.toml".into(),
                    "[package]\nname='x'\nversion='0.1.0'\n[dependencies]\nunknown='1'\n".into(),
                )]))
                .is_err()
        );
        assert!(
            snapshot
                .validate_dependency_changes(&BTreeMap::from([(
                    "source/Cargo.toml".into(),
                    "[package]\nname='x'\nversion='0.1.0'\n[patch.crates-io]\nserde='1'\n".into(),
                )]))
                .is_err()
        );
        assert!(
            snapshot
                .validate_dependency_changes(&BTreeMap::from([(
                    "source/Cargo.toml".into(),
                    "[package]\nname='x'\nversion='0.1.0'\n[dependencies]\nserde={git='https://example.invalid/x'}\n".into(),
                )]))
                .is_err()
        );
        let vendored_lock = format!(
            "version = 4\n\n[[package]]\nname = 'serde'\nversion = '1.0.200'\nsource = 'registry+https://github.com/rust-lang/crates.io-index'\nchecksum = '{}'\n",
            "d".repeat(64)
        );
        assert!(
            snapshot
                .validate_dependency_changes(&BTreeMap::from([(
                    "source/Cargo.lock".into(),
                    vendored_lock.clone(),
                )]))
                .is_ok()
        );
        assert!(
            snapshot
                .validate_dependency_changes(&BTreeMap::from([(
                    "source/Cargo.lock".into(),
                    vendored_lock.replace("1.0.200", "1.0.201"),
                )]))
                .is_err()
        );
        assert!(
            snapshot
                .validate_dependency_changes(&BTreeMap::from([(
                    "source/Cargo.lock".into(),
                    vendored_lock.replace(&"d".repeat(64), &"e".repeat(64)),
                )]))
                .is_err()
        );
    }

    #[test]
    fn local_dependency_paths_are_repository_relative_and_cannot_escape() {
        let relative = validate_cargo_path("../astrid-types").unwrap();
        assert_eq!(
            normalize_relative_join(std::path::Path::new("crates/astrid-events"), &relative)
                .unwrap(),
            std::path::PathBuf::from("crates/astrid-types")
        );
        let root_relative = validate_cargo_path("crates/astrid-types").unwrap();
        assert_eq!(
            normalize_relative_join(std::path::Path::new(""), &root_relative).unwrap(),
            std::path::PathBuf::from("crates/astrid-types")
        );
        let escaping = validate_cargo_path("../../../operator-secrets").unwrap();
        assert!(
            normalize_relative_join(std::path::Path::new("crates/astrid-events"), &escaping)
                .is_err()
        );
        assert!(validate_cargo_path("./astrid-types").is_err());
        assert!(validate_cargo_path("crates//astrid-types").is_err());
    }
}
