use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::attestation::HmacSigner;
use crate::config::Config;
use crate::util::{
    MAX_JSON_BYTES, bounded_text, canonical_json, read_stable_regular, sha256, validate_hex64,
    validate_identifier, validate_relative,
};
use crate::{Error, Result};

const MANIFEST_SCHEMA: &str = "astrid.edge.self_change_source_bundle.v1";
const SIGNATURE_SCHEMA: &str = "astrid.edge.self_change_source_signature.v1";
const SOURCE_ID_SCHEMA: &str = "astrid.edge.self_change_source_identity.v1";
const DERIVED_MANIFEST_SCHEMA: &str = "astrid.edge.self_change_generation_source.v1";
const DERIVED_SIGNATURE_SCHEMA: &str = "astrid.edge.self_change_generation_source_signature.v1";
const GENERATION_SCHEMA: &str = "astrid.edge_self_change.generation.v1";
const INITIAL_GENERATION_SCHEMA: &str = "astrid.edge_self_change.initial_generation.v1";
const INITIAL_GENERATION_AUTHORITY: &str =
    "operator_packaged_initial_generation_not_model_candidate";
const MAX_SOURCE_FILE: u64 = 512 * 1024;
const MAX_SEARCH_FILES: usize = 128;
const IMMUTABLE_SOURCE_PREFIXES: &[&str] = &[
    "source/services/astrid-edge-steward-helper/",
    "source/services/astrid-edge-rescue-helper/",
    "source/services/astrid-edge-web-broker/",
    "source/services/astrid-edge-checkpoint/",
    "source/packaging/systemd/astrid-edge-web-broker",
    "source/scripts/edge_self_change",
    "source/scripts/install_edge_self_evolution_root",
    "source/capsules/spectral-bridge/",
    "source/minime/",
];
const INSPECT_ONLY_ORIGIN: &str = "inspect_only_immutable_boundary";
const MUTABLE_UNIT_FRAGMENTS: &[&str] = &[
    "ollama-cpu.service",
    "astrid-model-warmup.service",
    "astrid.service",
    "astrid-edge-runtime.service",
    "astrid-edge-hindsight.service",
    "astrid-edge-hindsight.timer",
];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
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
    files: Vec<ManifestFile>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ManifestFile {
    path: String,
    origin: String,
    mode: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DerivedManifest {
    schema: String,
    source_id: String,
    parent_source_id: String,
    base_generation: String,
    repository_commit: String,
    vendor_attestation_sha256: String,
    file_count: usize,
    uncompressed_bytes: u64,
    files: Vec<ManifestFile>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DerivedSignature {
    schema: String,
    mode: String,
    key_id: String,
    manifest_sha256: String,
    hmac_sha256: String,
}

#[derive(Debug, Serialize)]
struct DerivedIdentity<'a> {
    schema: &'static str,
    parent_source_id: &'a str,
    base_generation: &'a str,
    repository_commit: &'a str,
    vendor_attestation_sha256: &'a str,
    files: &'a [ManifestFile],
}

#[derive(Debug, Serialize)]
struct SourceIdentity<'a> {
    schema: &'static str,
    repository_commit: &'a str,
    rustc: &'a Value,
    files: &'a [ManifestFile],
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct GenerationManifest {
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

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct VendorPackage {
    directory: String,
    name: String,
    version: String,
    package_checksum: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Signature {
    schema: String,
    mode: String,
    key_id: String,
    manifest_sha256: String,
    hmac_sha256: String,
}

#[derive(Debug, Clone)]
pub struct SourceSnapshot {
    root: PathBuf,
    pub source_id: String,
    pub repository_commit: String,
    entries: BTreeMap<String, SourceEntry>,
    vendor_versions: BTreeMap<String, Vec<Version>>,
    vendor_checksums: BTreeMap<(String, Version), Option<String>>,
    local_packages: BTreeSet<String>,
    vendor_attestation_sha256: String,
    lineage_base_generation: Option<String>,
    lineage_parent_source_id: Option<String>,
}

/// Exact signed generation/source binding used only to validate root-produced
/// metadata projections. It carries no path, write, build, or activation authority.
#[derive(Debug, Clone)]
pub(crate) struct GenerationEvidenceBinding {
    pub appliance_id: String,
    pub generation_id: String,
    pub build_id: String,
    pub candidate_id: String,
    pub candidate_sha256: String,
    pub base_generation: String,
    pub bundle_sha256: String,
    pub tests_sha256: String,
    pub target: String,
    pub source_id: String,
    pub parent_source_id: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceEntry {
    pub path: String,
    pub origin: String,
    pub mode: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceMatch {
    pub source_id: String,
    pub line: usize,
    pub excerpt: String,
}

impl SourceSnapshot {
    #[allow(clippy::too_many_lines)] // Signature and exact inventory construction are one gate.
    pub fn load(
        root: &Path,
        manifest_path: &Path,
        signature_path: &Path,
        key_path: &Path,
    ) -> Result<Self> {
        let manifest_bytes = read_stable_regular(manifest_path, MAX_JSON_BYTES)?;
        let signature_bytes = read_stable_regular(signature_path, 16 * 1024)?;
        let manifest_value: Value = serde_json::from_slice(&manifest_bytes)?;
        let manifest: Manifest = serde_json::from_value(manifest_value.clone())?;
        let signature: Signature = serde_json::from_slice(&signature_bytes)?;
        let signer = HmacSigner::from_file(key_path)?;
        let canonical = canonical_json(&manifest_value)?;
        let identity = SourceIdentity {
            schema: SOURCE_ID_SCHEMA,
            repository_commit: &manifest.repository_commit,
            rustc: &manifest.rustc,
            files: &manifest.files,
        };
        let source_identity_sha256 = sha256(&canonical_json(&identity)?);
        if manifest.schema != MANIFEST_SCHEMA
            || manifest.signature_mode != "hmac-sha256"
            || signature.schema != SIGNATURE_SCHEMA
            || signature.mode != "hmac-sha256"
            || signature.key_id != signer.key_id.trim_start_matches("hmac-sha256:")
            || manifest.key_id != signature.key_id
            || signature.manifest_sha256 != sha256(&canonical)
            || !signer.verify(&canonical, &signature.hmac_sha256)
            || manifest.file_count != manifest.files.len()
            || manifest.source_identity_sha256 != source_identity_sha256
            || manifest.source_id != format!("cpu-edge:{source_identity_sha256}")
            || manifest.repository_commit.len() < 7
            || manifest.git_object_format.is_empty()
            || manifest.cargo_lock_version == 0
            || validate_hex64(&manifest.cargo_lock_sha256, "cargo lock hash").is_err()
            || manifest.uncompressed_bytes == 0
            || manifest.rustc.is_null()
            || manifest.vendor_packages.is_empty()
        {
            return Err(Error::new("signed source snapshot verification failed"));
        }
        let root = fs::canonicalize(root)?;
        let metadata = fs::symlink_metadata(&root)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(Error::new("source snapshot root is not a directory"));
        }
        let mut entries = BTreeMap::new();
        let mut total_bytes = 0_u64;
        let mut previous_path: Option<String> = None;
        for record in manifest.files {
            validate_relative(&record.path, true)?;
            total_bytes = total_bytes
                .checked_add(record.size)
                .ok_or_else(|| Error::new("signed source byte total overflow"))?;
            if record.size > 4 * 1024 * 1024 * 1024_u64
                || validate_hex64(&record.sha256, "source inventory hash").is_err()
                || !matches!(record.mode.as_str(), "0644" | "0755")
                || record.origin.is_empty()
                || !service_role_is_valid(&record.path, &record.origin)
                || previous_path
                    .as_deref()
                    .is_some_and(|previous| previous >= record.path.as_str())
            {
                return Err(Error::new("invalid signed source inventory record"));
            }
            previous_path = Some(record.path.clone());
            let entry = SourceEntry {
                path: record.path.clone(),
                origin: record.origin,
                mode: record.mode,
                size: record.size,
                sha256: record.sha256,
            };
            if entries.insert(record.path, entry).is_some() {
                return Err(Error::new("duplicate signed source ID"));
            }
        }
        if total_bytes != manifest.uncompressed_bytes
            || entries
                .get("source/Cargo.lock")
                .is_some_and(|entry| entry.sha256 != manifest.cargo_lock_sha256)
        {
            return Err(Error::new("signed source inventory totals are invalid"));
        }
        let mut vendor_attestation = Vec::new();
        for entry in entries
            .values()
            .filter(|entry| entry.path.starts_with("vendor/"))
        {
            vendor_attestation.extend_from_slice(entry.path.as_bytes());
            vendor_attestation.push(0);
            vendor_attestation.extend_from_slice(entry.mode.as_bytes());
            vendor_attestation.push(0);
            vendor_attestation.extend_from_slice(entry.sha256.as_bytes());
            vendor_attestation.push(b'\n');
        }
        let vendor_attestation_sha256 = sha256(&vendor_attestation);
        let mut vendor_versions: BTreeMap<String, Vec<Version>> = BTreeMap::new();
        let mut vendor_checksums = BTreeMap::new();
        for package in manifest.vendor_packages {
            if package.directory.is_empty()
                || package.name.is_empty()
                || package
                    .package_checksum
                    .as_deref()
                    .is_some_and(|value| value.len() != 64)
            {
                return Err(Error::new("invalid signed vendor package inventory"));
            }
            let version = Version::parse(&package.version)
                .map_err(|_| Error::new("invalid signed vendor package version"))?;
            if let Some(checksum) = package.package_checksum.as_deref() {
                validate_hex64(checksum, "signed vendor package checksum")?;
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
                .push(version);
        }
        let mut snapshot = Self {
            root,
            source_id: manifest.source_id,
            repository_commit: manifest.repository_commit,
            entries,
            vendor_versions,
            vendor_checksums,
            local_packages: BTreeSet::new(),
            vendor_attestation_sha256,
            lineage_base_generation: None,
            lineage_parent_source_id: None,
        };
        snapshot.local_packages = snapshot.discover_local_packages()?;
        Ok(snapshot)
    }

    /// Resolve the source that belongs to the exact active generation.
    ///
    /// The immutable bootstrap snapshot is accepted only while the active release is an exact
    /// operator-authenticated initial generation. Every promoted candidate must carry its own
    /// cumulative signed snapshot; a missing or invalid snapshot never falls back to bootstrap.
    pub fn load_for_active_generation(config: &Config) -> Result<(Self, String)> {
        let bootstrap = Self::load(
            &config.source_root,
            &config.source_manifest,
            &config.source_signature,
            &config.source_signing_key,
        )?;
        if bootstrap.source_id != config.expected_source_id {
            return Err(Error::new(
                "signed bootstrap source does not match configured source identity",
            ));
        }
        let active = ActiveGeneration::resolve(config)?;
        let active_manifest = active.root.join(".astrid-edge-generation.json");
        require_trusted_regular(
            &active_manifest,
            active.trusted_uid,
            "active generation manifest",
        )?;
        let value: Value =
            serde_json::from_slice(&read_stable_regular(&active_manifest, 16 * 1024 * 1024)?)?;
        let snapshot = match value.get("schema").and_then(Value::as_str) {
            Some(INITIAL_GENERATION_SCHEMA) => {
                let manifest: InitialGenerationManifest = serde_json::from_value(value)?;
                validate_initial_generation(config, &active, &manifest)?;
                let derived = active.root.join("source-snapshot");
                if derived.exists() || derived.is_symlink() {
                    return Err(Error::new(
                        "operator initial generation unexpectedly contains source-snapshot",
                    ));
                }
                bootstrap
            },
            Some(GENERATION_SCHEMA) => {
                let manifest: GenerationManifest = serde_json::from_value(value)?;
                validate_candidate_generation(config, &active, &manifest)?;
                let snapshot = Self::load_derived(
                    &active.root.join("source-snapshot"),
                    &bootstrap,
                    &config.source_signing_key,
                    active.trusted_uid,
                )?;
                if snapshot.lineage_base_generation.as_deref()
                    != Some(manifest.base_generation.as_str())
                {
                    return Err(Error::new(
                        "active source lineage does not match generation base",
                    ));
                }
                let parent_source_id = source_id_for_parent_generation(
                    config,
                    &active,
                    &manifest.base_generation,
                    &bootstrap,
                )?;
                if snapshot.lineage_parent_source_id.as_deref() != Some(parent_source_id.as_str()) {
                    return Err(Error::new(
                        "active source parent does not match exact base generation source",
                    ));
                }
                snapshot
            },
            _ => return Err(Error::new("unsupported active generation manifest")),
        };
        active.verify_stable(config)?;
        Ok((snapshot, active.generation_id))
    }

    /// Authenticate the source and generation metadata for the active generation,
    /// its direct base, or one installed direct successor. This deliberately does
    /// not provide an arbitrary release-directory reader.
    #[allow(clippy::too_many_lines)] // Signed current/base/successor lineage is one trust gate.
    pub(crate) fn evidence_for_adjacent_generation(
        &self,
        config: &Config,
        active_generation: &str,
        requested_generation: &str,
    ) -> Result<Option<(Self, GenerationEvidenceBinding)>> {
        validate_identifier(requested_generation, "evidence generation ID")?;
        let active = ActiveGeneration::resolve(config)?;
        if active.generation_id != active_generation {
            return Err(Error::new(
                "active generation changed before evidence inspection",
            ));
        }
        let active_manifest_path = active.root.join(".astrid-edge-generation.json");
        require_trusted_regular(
            &active_manifest_path,
            active.trusted_uid,
            "active generation manifest",
        )?;
        let active_value: Value = serde_json::from_slice(&read_stable_regular(
            &active_manifest_path,
            16 * 1024 * 1024,
        )?)?;
        let active_candidate = match active_value.get("schema").and_then(Value::as_str) {
            Some(GENERATION_SCHEMA) => {
                let manifest: GenerationManifest = serde_json::from_value(active_value)?;
                validate_candidate_generation(config, &active, &manifest)?;
                Some(manifest)
            },
            Some(INITIAL_GENERATION_SCHEMA) => None,
            _ => return Err(Error::new("unsupported active generation manifest")),
        };

        let requested_root = active.releases_root.join(requested_generation);
        let requested = ActiveGeneration {
            generation_id: requested_generation.to_owned(),
            root: requested_root,
            releases_root: active.releases_root.clone(),
            trusted_uid: active.trusted_uid,
            root_device: 0,
            root_inode: 0,
        };
        require_trusted_directory(
            &requested.root,
            requested.trusted_uid,
            "evidence generation",
        )?;
        let requested_manifest_path = requested.root.join(".astrid-edge-generation.json");
        require_trusted_regular(
            &requested_manifest_path,
            requested.trusted_uid,
            "evidence generation manifest",
        )?;
        let requested_value: Value = serde_json::from_slice(&read_stable_regular(
            &requested_manifest_path,
            16 * 1024 * 1024,
        )?)?;
        let requested_manifest = match requested_value.get("schema").and_then(Value::as_str) {
            Some(GENERATION_SCHEMA) => {
                let manifest: GenerationManifest = serde_json::from_value(requested_value)?;
                validate_candidate_generation(config, &requested, &manifest)?;
                manifest
            },
            Some(INITIAL_GENERATION_SCHEMA) => {
                if requested_generation != active_generation
                    && active_candidate
                        .as_ref()
                        .map(|value| value.base_generation.as_str())
                        != Some(requested_generation)
                {
                    return Err(Error::new(
                        "generation evidence is stale or outside current lineage",
                    ));
                }
                active.verify_stable(config)?;
                return Ok(None);
            },
            _ => return Err(Error::new("unsupported evidence generation manifest")),
        };
        let adjacent = requested_generation == active_generation
            || active_candidate
                .as_ref()
                .map(|value| value.base_generation.as_str())
                == Some(requested_generation)
            || requested_manifest.base_generation == active_generation;
        if !adjacent {
            return Err(Error::new(
                "generation evidence is stale or outside current lineage",
            ));
        }

        let requested_snapshot = if requested_generation == active_generation {
            self.clone()
        } else {
            let bootstrap = Self::load(
                &config.source_root,
                &config.source_manifest,
                &config.source_signature,
                &config.source_signing_key,
            )?;
            if bootstrap.source_id != config.expected_source_id {
                return Err(Error::new(
                    "signed bootstrap source does not match configured source identity",
                ));
            }
            Self::load_derived(
                &requested.root.join("source-snapshot"),
                &bootstrap,
                &config.source_signing_key,
                requested.trusted_uid,
            )?
        };
        if requested_snapshot.lineage_base_generation.as_deref()
            != Some(requested_manifest.base_generation.as_str())
        {
            return Err(Error::new(
                "generation evidence source does not match its exact base",
            ));
        }
        let parent_source_id = source_id_for_parent_generation(
            config,
            &active,
            &requested_manifest.base_generation,
            &Self::load(
                &config.source_root,
                &config.source_manifest,
                &config.source_signature,
                &config.source_signing_key,
            )?,
        )?;
        if requested_snapshot.lineage_parent_source_id.as_deref() != Some(parent_source_id.as_str())
        {
            return Err(Error::new(
                "generation evidence parent source identity failed",
            ));
        }
        active.verify_stable(config)?;
        let binding = GenerationEvidenceBinding {
            appliance_id: requested_manifest.appliance_id,
            generation_id: requested_manifest.generation_id,
            build_id: requested_manifest.build_id,
            candidate_id: requested_manifest.candidate_id,
            candidate_sha256: requested_manifest.candidate_sha256,
            base_generation: requested_manifest.base_generation,
            bundle_sha256: requested_manifest.bundle_sha256,
            tests_sha256: requested_manifest.tests_sha256,
            target: requested_manifest.target,
            source_id: requested_snapshot.source_id.clone(),
            parent_source_id,
        };
        Ok(Some((requested_snapshot, binding)))
    }

    pub(crate) fn source_sha256(&self, path: &str) -> Option<&str> {
        self.entries.get(path).map(|entry| entry.sha256.as_str())
    }

    #[allow(clippy::too_many_lines)] // Authentication, lineage, and exact-tree checks are one gate.
    fn load_derived(
        root: &Path,
        bootstrap: &Self,
        key_path: &Path,
        trusted_uid: u32,
    ) -> Result<Self> {
        require_trusted_directory(root, trusted_uid, "generation source snapshot")?;
        require_trusted_regular(
            &root.join("MANIFEST.json"),
            trusted_uid,
            "generation source manifest",
        )?;
        require_trusted_regular(
            &root.join("MANIFEST.signature.json"),
            trusted_uid,
            "generation source signature",
        )?;
        let manifest_value: Value = serde_json::from_slice(&read_stable_regular(
            &root.join("MANIFEST.json"),
            MAX_JSON_BYTES,
        )?)?;
        let manifest: DerivedManifest = serde_json::from_value(manifest_value.clone())?;
        let signature: DerivedSignature = serde_json::from_slice(&read_stable_regular(
            &root.join("MANIFEST.signature.json"),
            16 * 1024,
        )?)?;
        let signer = HmacSigner::from_file(key_path)?;
        let canonical = canonical_json(&manifest_value)?;
        let identity = DerivedIdentity {
            schema: DERIVED_MANIFEST_SCHEMA,
            parent_source_id: &manifest.parent_source_id,
            base_generation: &manifest.base_generation,
            repository_commit: &manifest.repository_commit,
            vendor_attestation_sha256: &manifest.vendor_attestation_sha256,
            files: &manifest.files,
        };
        let identity_sha256 = sha256(&canonical_json(&identity)?);
        let parent_hash = manifest
            .parent_source_id
            .strip_prefix("cpu-edge:")
            .ok_or_else(|| Error::new("derived source parent has unsupported form"))?;
        if manifest.schema != DERIVED_MANIFEST_SCHEMA
            || signature.schema != DERIVED_SIGNATURE_SCHEMA
            || signature.mode != "hmac-sha256"
            || signature.key_id != signer.key_id.trim_start_matches("hmac-sha256:")
            || signature.manifest_sha256 != sha256(&canonical)
            || !signer.verify(&canonical, &signature.hmac_sha256)
            || manifest.source_id != format!("cpu-edge:{identity_sha256}")
            || validate_hex64(parent_hash, "derived parent source ID").is_err()
            || validate_identifier(&manifest.base_generation, "derived base generation").is_err()
            || manifest.repository_commit != bootstrap.repository_commit
            || manifest.vendor_attestation_sha256 != bootstrap.vendor_attestation_sha256
            || manifest.file_count != manifest.files.len()
            || manifest.files.is_empty()
            || manifest.files.len() > 50_000
        {
            return Err(Error::new(
                "generation source authentication or lineage failed",
            ));
        }
        let bootstrap_source = bootstrap
            .entries
            .iter()
            .filter(|(path, _)| path.starts_with("source/"))
            .collect::<BTreeMap<_, _>>();
        let mut entries = BTreeMap::new();
        let mut total = 0_u64;
        let mut previous_path: Option<String> = None;
        for record in manifest.files {
            validate_relative(&record.path, true)?;
            let baseline = bootstrap_source
                .get(&record.path)
                .ok_or_else(|| Error::new("generation source changed signed source surface"))?;
            if !record.path.starts_with("source/")
                || validate_hex64(&record.sha256, "generation source hash").is_err()
                || !matches!(record.mode.as_str(), "0644" | "0755")
                || record.origin != baseline.origin
                || record.mode != baseline.mode
                || previous_path
                    .as_deref()
                    .is_some_and(|previous| previous >= record.path.as_str())
            {
                return Err(Error::new("generation source inventory is invalid"));
            }
            previous_path = Some(record.path.clone());
            total = total
                .checked_add(record.size)
                .ok_or_else(|| Error::new("generation source byte total overflow"))?;
            let entry = SourceEntry {
                path: record.path.clone(),
                origin: record.origin,
                mode: record.mode,
                size: record.size,
                sha256: record.sha256,
            };
            if entries.insert(record.path, entry).is_some() {
                return Err(Error::new("duplicate generation source inventory"));
            }
        }
        let surface_matches = entries.len() == bootstrap_source.len();
        if !surface_matches
            || total == 0
            || total > 4 * 1024 * 1024 * 1024_u64
            || total != manifest.uncompressed_bytes
        {
            return Err(Error::new(
                "generation source totals or surface identity failed",
            ));
        }
        let mut snapshot = Self {
            root: root.to_path_buf(),
            source_id: manifest.source_id,
            repository_commit: bootstrap.repository_commit.clone(),
            entries,
            vendor_versions: bootstrap.vendor_versions.clone(),
            vendor_checksums: bootstrap.vendor_checksums.clone(),
            local_packages: BTreeSet::new(),
            vendor_attestation_sha256: bootstrap.vendor_attestation_sha256.clone(),
            lineage_base_generation: Some(manifest.base_generation),
            lineage_parent_source_id: Some(manifest.parent_source_id),
        };
        snapshot.verify_derived_tree(trusted_uid)?;
        snapshot.local_packages = snapshot.discover_local_packages()?;
        for entry in snapshot
            .entries
            .values()
            .filter(|entry| entry.path.ends_with("Cargo.lock"))
        {
            snapshot.validate_lockfile(&snapshot.full_text(entry)?)?;
        }
        Ok(snapshot)
    }

    fn verify_derived_tree(&self, trusted_uid: u32) -> Result<()> {
        let source_root = self.root.join("source");
        require_trusted_directory(&source_root, trusted_uid, "derived source directory")?;
        let mut actual = BTreeSet::new();
        collect_generation_files(&self.root, &source_root, trusted_uid, &mut actual)?;
        let expected = self.entries.keys().cloned().collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(Error::new(
                "generation source tree differs from signed membership",
            ));
        }
        for entry in self.entries.values() {
            let metadata = fs::symlink_metadata(self.root.join(&entry.path))?;
            let expected_mode = if entry.mode == "0755" { 0o555 } else { 0o444 };
            if metadata.mode() & 0o777 != expected_mode {
                return Err(Error::new("generation source mode differs from signature"));
            }
            let _ = self.verified_bytes(entry)?;
        }
        Ok(())
    }

    pub fn list(&self, prefix: &str, limit: usize) -> Result<Vec<SourceEntry>> {
        if prefix.len() > 160 || limit == 0 || limit > 50 {
            return Err(Error::new("invalid source listing bounds"));
        }
        Ok(self
            .entries
            .values()
            .filter(|entry| is_visible_source(entry) && entry.path.contains(prefix))
            .take(limit)
            .cloned()
            .collect())
    }

    pub fn read(
        &self,
        source_id: &str,
        expected_sha256: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> Result<String> {
        if limit == 0 || limit > 8_000 || offset > 512 * 1024 {
            return Err(Error::new("invalid source read bounds"));
        }
        let entry = self
            .entries
            .get(source_id)
            .filter(|entry| is_visible_source(entry))
            .ok_or_else(|| Error::new("unknown or non-readable signed source ID"))?;
        if expected_sha256.is_some_and(|expected| expected != entry.sha256) {
            return Err(Error::new("stale signed source hash"));
        }
        let data = self.verified_bytes(entry)?;
        let text = std::str::from_utf8(&data).map_err(|_| Error::new("binary source rejected"))?;
        Ok(text.chars().skip(offset).take(limit).collect())
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SourceMatch>> {
        let query = query.trim();
        if query.is_empty() || query.chars().count() > 160 || limit == 0 || limit > 20 {
            return Err(Error::new("invalid source search bounds"));
        }
        let query_lower = query.to_lowercase();
        let mut matches = Vec::new();
        for entry in self
            .entries
            .values()
            .filter(|entry| is_visible_source(entry))
            .take(MAX_SEARCH_FILES)
        {
            if entry.size > MAX_SOURCE_FILE {
                continue;
            }
            let data = self.verified_bytes(entry)?;
            let Ok(text) = std::str::from_utf8(&data) else {
                continue;
            };
            for (line_index, line) in text.lines().enumerate() {
                if line.to_lowercase().contains(&query_lower) {
                    matches.push(SourceMatch {
                        source_id: entry.path.clone(),
                        line: line_index.saturating_add(1),
                        excerpt: bounded_text(line.trim(), 240),
                    });
                    if matches.len() == limit {
                        return Ok(matches);
                    }
                }
            }
        }
        Ok(matches)
    }

    pub fn mutable_entry(&self, source_id: &str) -> Result<SourceEntry> {
        let entry = self
            .entries
            .get(source_id)
            .filter(|entry| is_mutable_source(entry))
            .ok_or_else(|| Error::new("source ID is outside the signed mutable surface"))?;
        let _ = self.verified_bytes(entry)?;
        Ok(entry.clone())
    }

    pub fn full_text(&self, entry: &SourceEntry) -> Result<String> {
        if entry.size > MAX_SOURCE_FILE {
            return Err(Error::new("source file exceeds candidate authoring bound"));
        }
        let bytes = self.verified_bytes(entry)?;
        if bytes.contains(&0) {
            return Err(Error::new("binary source rejected"));
        }
        String::from_utf8(bytes).map_err(|_| Error::new("non-UTF-8 source rejected"))
    }

    pub fn validate_dependency_changes(
        &self,
        replacements: &BTreeMap<String, String>,
    ) -> Result<()> {
        for (source_id, content) in replacements {
            if source_id.ends_with("Cargo.toml") {
                let document = content
                    .parse::<toml::Value>()
                    .map_err(|_| Error::new("candidate Cargo.toml is malformed"))?;
                self.validate_manifest_dependencies(&document)?;
            } else if source_id.ends_with("Cargo.lock") {
                self.validate_lockfile(content)?;
            }
        }
        Ok(())
    }

    fn discover_local_packages(&self) -> Result<BTreeSet<String>> {
        let mut packages = BTreeSet::new();
        for entry in self.entries.values().filter(|entry| {
            entry.path.ends_with("Cargo.toml")
                && matches!(
                    entry.origin.as_str(),
                    "build_required_manifest" | "mutable_build_manifest"
                )
        }) {
            let text = self.full_text(entry)?;
            let value = text
                .parse::<toml::Value>()
                .map_err(|_| Error::new("signed local Cargo.toml is malformed"))?;
            if let Some(name) = value
                .get("package")
                .and_then(|value| value.get("name"))
                .and_then(toml::Value::as_str)
            {
                packages.insert(name.to_owned());
            }
        }
        Ok(packages)
    }

    fn validate_manifest_dependencies(&self, document: &toml::Value) -> Result<()> {
        let mut tables = Vec::new();
        collect_dependency_tables(document, &mut tables);
        for table in tables {
            for (alias, specification) in table {
                let (name, version, local_or_workspace) =
                    dependency_identity(alias, specification)?;
                if local_or_workspace {
                    if !self.local_packages.contains(name)
                        && !self.vendor_versions.contains_key(name)
                    {
                        return Err(Error::new(
                            "candidate dependency is absent from signed source/vendor inventory",
                        ));
                    }
                    continue;
                }
                let versions = self.vendor_versions.get(name).ok_or_else(|| {
                    Error::new("candidate external dependency is not signed and vendored")
                })?;
                let requirement =
                    VersionReq::parse(version.ok_or_else(|| {
                        Error::new("external dependency omitted version constraint")
                    })?)
                    .map_err(|_| {
                        Error::new("candidate dependency version requirement is malformed")
                    })?;
                if !versions
                    .iter()
                    .any(|candidate| requirement.matches(candidate))
                {
                    return Err(Error::new(
                        "candidate dependency version is absent from signed vendor set",
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_lockfile(&self, content: &str) -> Result<()> {
        if content.len() > 512 * 1024 || content.lines().count() > 20_000 {
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
                validate_hex64(checksum, "lock package checksum")?;
                if self
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

    fn verified_bytes(&self, entry: &SourceEntry) -> Result<Vec<u8>> {
        let relative = validate_relative(&entry.path, true)?;
        let path = self.root.join(relative);
        let canonical_parent = path
            .parent()
            .ok_or_else(|| Error::new("source has no parent"))?
            .canonicalize()?;
        if !canonical_parent.starts_with(&self.root) {
            return Err(Error::new("source path escapes signed root"));
        }
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.nlink() != 1
            || metadata.len() != entry.size
        {
            return Err(Error::new("signed source became linked, special, or stale"));
        }
        let bytes = read_stable_regular(&path, entry.size)?;
        if sha256(&bytes) != entry.sha256 {
            return Err(Error::new("signed source content hash mismatch"));
        }
        Ok(bytes)
    }
}

#[derive(Debug)]
struct ActiveGeneration {
    generation_id: String,
    root: PathBuf,
    releases_root: PathBuf,
    trusted_uid: u32,
    root_device: u64,
    root_inode: u64,
}

impl ActiveGeneration {
    fn resolve(config: &Config) -> Result<Self> {
        let binding_metadata = fs::symlink_metadata(&config.current_generation)?;
        if !binding_metadata.is_file()
            || binding_metadata.file_type().is_symlink()
            || binding_metadata.nlink() != 1
            || binding_metadata.mode() & 0o022 != 0
        {
            return Err(Error::new(
                "current generation binding is linked, writable, or special",
            ));
        }
        let trusted_uid = binding_metadata.uid();
        let generation_id = read_generation_binding(config)?;
        let link_metadata = fs::symlink_metadata(&config.active_generation_link)?;
        if !link_metadata.file_type().is_symlink() || link_metadata.uid() != trusted_uid {
            return Err(Error::new(
                "active generation pointer is not trusted-owner symlink",
            ));
        }
        let release_parent = config
            .active_generation_link
            .parent()
            .ok_or_else(|| Error::new("active generation pointer has no parent"))?;
        let releases_root = release_parent.join("releases");
        require_trusted_directory(release_parent, trusted_uid, "release parent")?;
        require_trusted_directory(&releases_root, trusted_uid, "releases root")?;
        let expected_target = Path::new("releases").join(&generation_id);
        if fs::read_link(&config.active_generation_link)? != expected_target {
            return Err(Error::new(
                "active generation pointer and generation binding disagree",
            ));
        }
        let root = releases_root.join(&generation_id);
        require_trusted_directory(&root, trusted_uid, "active generation")?;
        let root_metadata = fs::symlink_metadata(&root)?;
        Ok(Self {
            generation_id,
            root,
            releases_root,
            trusted_uid,
            root_device: root_metadata.dev(),
            root_inode: root_metadata.ino(),
        })
    }

    fn verify_stable(&self, config: &Config) -> Result<()> {
        if read_generation_binding(config)? != self.generation_id
            || fs::read_link(&config.active_generation_link)?
                != Path::new("releases").join(&self.generation_id)
        {
            return Err(Error::new(
                "active generation changed while source was being verified",
            ));
        }
        let metadata = fs::symlink_metadata(&self.root)?;
        if metadata.dev() != self.root_device
            || metadata.ino() != self.root_inode
            || metadata.uid() != self.trusted_uid
            || !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.mode() & 0o022 != 0
        {
            return Err(Error::new(
                "active generation identity changed during source verification",
            ));
        }
        Ok(())
    }
}

fn read_generation_binding(config: &Config) -> Result<String> {
    let bytes = read_stable_regular(&config.current_generation, 256)?;
    let value = std::str::from_utf8(&bytes)
        .map_err(|_| Error::new("current generation binding is not UTF-8"))?
        .trim()
        .to_owned();
    validate_identifier(&value, "current generation")?;
    Ok(value)
}

fn require_trusted_directory(path: &Path, trusted_uid: u32, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != trusted_uid
        || metadata.mode() & 0o022 != 0
    {
        return Err(Error::new(format!(
            "{label} must be trusted-owner, non-linked, and non-writable"
        )));
    }
    Ok(())
}

fn require_trusted_regular(path: &Path, trusted_uid: u32, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != trusted_uid
        || metadata.mode() & 0o022 != 0
    {
        return Err(Error::new(format!(
            "{label} must be trusted-owner, non-linked, and non-writable"
        )));
    }
    Ok(())
}

fn validate_candidate_generation(
    config: &Config,
    active: &ActiveGeneration,
    manifest: &GenerationManifest,
) -> Result<()> {
    if manifest.schema != GENERATION_SCHEMA
        || manifest.appliance_id != config.appliance_id
        || manifest.generation_id != active.generation_id
        || manifest.target != config.target
        || manifest.base_generation == manifest.generation_id
        || validate_identifier(&manifest.generation_id, "generation ID").is_err()
        || validate_identifier(&manifest.build_id, "build ID").is_err()
        || validate_identifier(&manifest.candidate_id, "candidate ID").is_err()
        || validate_identifier(&manifest.base_generation, "base generation").is_err()
        || validate_hex64(&manifest.candidate_sha256, "candidate hash").is_err()
        || validate_hex64(&manifest.bundle_sha256, "bundle hash").is_err()
        || validate_hex64(&manifest.tests_sha256, "tests hash").is_err()
    {
        return Err(Error::new(
            "active candidate generation identity or target failed",
        ));
    }
    Ok(())
}

fn validate_initial_generation(
    config: &Config,
    active: &ActiveGeneration,
    manifest: &InitialGenerationManifest,
) -> Result<()> {
    if manifest.schema != INITIAL_GENERATION_SCHEMA
        || manifest.appliance_id != config.appliance_id
        || manifest.target != config.target
        || manifest.version.is_empty()
        || manifest.version.len() > 128
        || manifest.authority != INITIAL_GENERATION_AUTHORITY
        || manifest.inventory.is_empty()
        || manifest.inventory.len() > 50_000
    {
        return Err(Error::new(
            "operator initial generation authority or target failed",
        ));
    }
    let mut expected = BTreeSet::new();
    for item in &manifest.inventory {
        let relative = validate_relative(&item.path, true)?;
        if validate_hex64(&item.sha256, "initial generation hash").is_err()
            || !expected.insert(item.path.clone())
        {
            return Err(Error::new(
                "operator initial generation inventory is invalid",
            ));
        }
        let path = active.root.join(relative);
        require_trusted_regular(&path, active.trusted_uid, "initial generation member")?;
        let bytes = read_stable_regular(&path, 512 * 1024 * 1024)?;
        if bytes.len() as u64 != item.size || sha256(&bytes) != item.sha256 {
            return Err(Error::new(
                "operator initial generation inventory hash failed",
            ));
        }
    }
    let mut actual = BTreeSet::new();
    collect_generation_files(&active.root, &active.root, active.trusted_uid, &mut actual)?;
    actual.remove(".astrid-edge-generation.json");
    if actual != expected {
        return Err(Error::new(
            "operator initial generation inventory membership failed",
        ));
    }
    Ok(())
}

fn source_id_for_parent_generation(
    config: &Config,
    active: &ActiveGeneration,
    generation_id: &str,
    bootstrap: &SourceSnapshot,
) -> Result<String> {
    validate_identifier(generation_id, "base generation")?;
    let root = active.releases_root.join(generation_id);
    require_trusted_directory(&root, active.trusted_uid, "base generation")?;
    let parent = ActiveGeneration {
        generation_id: generation_id.to_owned(),
        root,
        releases_root: active.releases_root.clone(),
        trusted_uid: active.trusted_uid,
        root_device: 0,
        root_inode: 0,
    };
    let manifest_path = parent.root.join(".astrid-edge-generation.json");
    require_trusted_regular(
        &manifest_path,
        active.trusted_uid,
        "base generation manifest",
    )?;
    let value: Value =
        serde_json::from_slice(&read_stable_regular(&manifest_path, 16 * 1024 * 1024)?)?;
    match value.get("schema").and_then(Value::as_str) {
        Some(INITIAL_GENERATION_SCHEMA) => {
            let manifest: InitialGenerationManifest = serde_json::from_value(value)?;
            validate_initial_generation(config, &parent, &manifest)?;
            if parent.root.join("source-snapshot").exists()
                || parent.root.join("source-snapshot").is_symlink()
            {
                return Err(Error::new(
                    "operator base generation unexpectedly contains source-snapshot",
                ));
            }
            Ok(bootstrap.source_id.clone())
        },
        Some(GENERATION_SCHEMA) => {
            let manifest: GenerationManifest = serde_json::from_value(value)?;
            validate_candidate_generation(config, &parent, &manifest)?;
            let snapshot = SourceSnapshot::load_derived(
                &parent.root.join("source-snapshot"),
                bootstrap,
                &config.source_signing_key,
                active.trusted_uid,
            )?;
            if snapshot.lineage_base_generation.as_deref()
                != Some(manifest.base_generation.as_str())
            {
                return Err(Error::new(
                    "base generation source lineage does not match its manifest",
                ));
            }
            Ok(snapshot.source_id)
        },
        _ => Err(Error::new("unsupported base generation manifest")),
    }
}

fn collect_generation_files(
    root: &Path,
    directory: &Path,
    trusted_uid: u32,
    output: &mut BTreeSet<String>,
) -> Result<()> {
    require_trusted_directory(directory, trusted_uid, "generation directory")?;
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink()
            || metadata.uid() != trusted_uid
            || metadata.mode() & 0o022 != 0
        {
            return Err(Error::new(
                "generation inventory contains linked, mutable, or foreign-owned content",
            ));
        }
        if metadata.is_dir() {
            collect_generation_files(root, &path, trusted_uid, output)?;
        } else if metadata.is_file() && metadata.nlink() == 1 {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| Error::new("generation inventory path escaped root"))?
                .to_string_lossy()
                .replace('\\', "/");
            validate_relative(&relative, true)?;
            output.insert(relative);
        } else {
            return Err(Error::new(
                "generation inventory contains linked or special content",
            ));
        }
    }
    Ok(())
}

fn is_visible_source(entry: &SourceEntry) -> bool {
    entry.path.starts_with("source/")
        && if is_immutable_source_domain(&entry.path) {
            entry.origin == INSPECT_ONLY_ORIGIN && is_inspect_only_immutable_source(&entry.path)
        } else {
            entry.origin != INSPECT_ONLY_ORIGIN
        }
        && !entry
            .path
            .split('/')
            .any(|component| component.starts_with('.'))
        && entry.size <= MAX_SOURCE_FILE
}

fn is_mutable_source(entry: &SourceEntry) -> bool {
    if !is_visible_source(entry) {
        return false;
    }
    match entry.origin.as_str() {
        "mutable_core_source"
        | "mutable_edge_runtime"
        | "mutable_edge_capsule"
        | "mutable_capsule_manifest"
        | "mutable_edge_report"
        | "mutable_appliance_profile" => true,
        "operator_supplied_lockfile" | "mutable_build_manifest" => {
            mutable_build_manifest_path(&entry.path)
        },
        "mutable_astrid_service_template" => is_mutable_unit_source(&entry.path),
        _ => false,
    }
}

fn mutable_build_manifest_path(path: &str) -> bool {
    path == "source/Cargo.toml"
        || path == "source/Cargo.lock"
        || (path.starts_with("source/crates/") && path.ends_with("/Cargo.toml"))
        || matches!(
            path.strip_prefix("source/services/astrid-edge-runtime/"),
            Some("Cargo.toml" | "Cargo.lock")
        )
        || (path.starts_with("source/capsules/astralis/astrid-capsule-")
            && matches!(path.rsplit('/').next(), Some("Cargo.toml" | "Cargo.lock")))
}

fn service_role_is_valid(path: &str, origin: &str) -> bool {
    if is_inspect_only_immutable_source(path) {
        return origin == INSPECT_ONLY_ORIGIN;
    }
    if origin == INSPECT_ONLY_ORIGIN || is_immutable_source_domain(path) {
        return false;
    }
    if path.starts_with("source/packaging/systemd/") {
        return if is_mutable_unit_source(path) {
            origin == "mutable_astrid_service_template"
        } else {
            origin == "build_required_service_template"
        };
    }
    !matches!(
        origin,
        "mutable_astrid_service_template" | "build_required_service_template"
    )
}

fn is_mutable_unit_source(path: &str) -> bool {
    let Some(relative) = path.strip_prefix("source/packaging/systemd/") else {
        return false;
    };
    let name = relative.strip_prefix("icp/").unwrap_or(relative);
    !name.contains('/') && MUTABLE_UNIT_FRAGMENTS.contains(&name)
}

fn is_immutable_source_domain(path: &str) -> bool {
    IMMUTABLE_SOURCE_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
        || is_inspect_only_immutable_source(path)
}

fn is_inspect_only_immutable_source(path: &str) -> bool {
    for prefix in [
        "source/services/astrid-edge-steward-helper/",
        "source/services/astrid-edge-rescue-helper/",
        "source/services/astrid-edge-web-broker/",
        "source/services/astrid-edge-checkpoint/",
    ] {
        if let Some(relative) = path.strip_prefix(prefix) {
            return relative == "Cargo.toml"
                || (relative.starts_with("src/")
                    && Path::new(relative)
                        .extension()
                        .is_some_and(|value| value == "rs"));
        }
    }
    if path.starts_with("source/scripts/edge_self_change/") {
        return Path::new(path)
            .extension()
            .is_some_and(|value| value == "py");
    }
    if matches!(
        path,
        "source/scripts/build_edge_self_change_source_bundle.py"
            | "source/scripts/build_edge_self_change_supervisor_zipapp.py"
            | "source/scripts/build_edge_self_change_toolchain_bundle.py"
            | "source/scripts/edge_self_change_supervisor.py"
            | "source/scripts/install_edge_self_evolution_root.sh"
            | "source/scripts/test_build_edge_self_change_source_bundle.py"
            | "source/scripts/test_build_edge_self_change_supervisor_zipapp.py"
            | "source/scripts/test_build_edge_self_change_toolchain_bundle.py"
            | "source/scripts/test_edge_builder_store.py"
            | "source/scripts/test_edge_probation_health_systemd.py"
            | "source/scripts/test_edge_self_change_supervisor.py"
            | "source/scripts/test_install_edge_self_evolution_root.sh"
    ) || path == "source/docs/cpu-edge-self-evolution.md"
    {
        return true;
    }
    let Some(relative) = path.strip_prefix("source/packaging/systemd/") else {
        return false;
    };
    let allowed_suffix = [".service", ".timer", ".socket", ".conf", ".env", ".in"]
        .iter()
        .any(|suffix| relative.ends_with(suffix));
    let exact_root_script = matches!(
        relative,
        "root/astrid-edge-builder-store"
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
                    "edge-checkpoint",
                    "builder-store",
                    "generation-guard",
                ]
                .iter()
                .any(|marker| relative.contains(marker))))
}

fn collect_dependency_tables<'a>(
    value: &'a toml::Value,
    output: &mut Vec<&'a toml::map::Map<String, toml::Value>>,
) {
    let Some(table) = value.as_table() else {
        return;
    };
    for name in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(dependencies) = table.get(name).and_then(toml::Value::as_table) {
            output.push(dependencies);
        }
    }
    if let Some(workspace) = table.get("workspace").and_then(toml::Value::as_table)
        && let Some(dependencies) = workspace
            .get("dependencies")
            .and_then(toml::Value::as_table)
    {
        output.push(dependencies);
    }
    if let Some(targets) = table.get("target").and_then(toml::Value::as_table) {
        for target in targets.values() {
            collect_dependency_tables(target, output);
        }
    }
}

fn dependency_identity<'a>(
    alias: &'a str,
    value: &'a toml::Value,
) -> Result<(&'a str, Option<&'a str>, bool)> {
    match value {
        toml::Value::String(version) => Ok((alias, Some(version), false)),
        toml::Value::Table(table) => {
            if table.contains_key("git") || table.contains_key("registry") {
                return Err(Error::new(
                    "git and alternate-registry dependencies are forbidden",
                ));
            }
            let name = table
                .get("package")
                .and_then(toml::Value::as_str)
                .unwrap_or(alias);
            let version = table.get("version").and_then(toml::Value::as_str);
            let local = table.get("path").is_some()
                || table.get("workspace").and_then(toml::Value::as_bool) == Some(true);
            Ok((name, version, local))
        },
        _ => Err(Error::new("unsupported candidate dependency specification")),
    }
}

pub fn repository_path(source_id: &str) -> Result<String> {
    let path = source_id
        .strip_prefix("source/")
        .ok_or_else(|| Error::new("candidate source ID is not repository source"))?;
    validate_relative(path, false)?;
    Ok(path.to_owned())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::os::unix::fs::symlink;

    use semver::Version;

    use super::{
        INSPECT_ONLY_ORIGIN, SourceEntry, SourceSnapshot, is_immutable_source_domain,
        is_mutable_source, is_mutable_unit_source, repository_path, service_role_is_valid,
    };

    #[test]
    fn repository_paths_are_derived_only_from_source_ids() {
        assert_eq!(
            repository_path("source/crates/a/src/lib.rs").unwrap(),
            "crates/a/src/lib.rs"
        );
        assert!(repository_path("vendor/a/src/lib.rs").is_err());
        assert!(repository_path("source/../secret").is_err());
    }

    #[test]
    fn stale_signed_source_is_rejected_before_model_reads() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let path = "source/services/edge/src/lib.rs";
        let full = root.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(&full, b"changed").unwrap();
        let entry = SourceEntry {
            path: path.to_owned(),
            origin: "mutable_edge_runtime".to_owned(),
            mode: "0644".to_owned(),
            size: 7,
            sha256: "a".repeat(64),
        };
        let snapshot = SourceSnapshot {
            root,
            source_id: format!("cpu-edge:{}", "b".repeat(64)),
            repository_commit: "c".repeat(40),
            entries: BTreeMap::from([(path.to_owned(), entry)]),
            vendor_versions: BTreeMap::new(),
            vendor_checksums: BTreeMap::new(),
            local_packages: BTreeSet::new(),
            vendor_attestation_sha256: super::sha256(&[]),
            lineage_base_generation: None,
            lineage_parent_source_id: None,
        };
        assert!(snapshot.read(path, None, 0, 100).is_err());
    }

    #[test]
    fn linked_hidden_and_special_signed_source_ids_are_rejected() {
        for kind in ["symlink", "hardlink", "special"] {
            let temporary = tempfile::tempdir().unwrap();
            let root = temporary.path().canonicalize().unwrap();
            let path = "source/services/edge/src/lib.rs";
            let full = root.join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            let data = b"pub fn safe() {}\n";
            match kind {
                "symlink" => {
                    let target = root.join("target.rs");
                    fs::write(&target, data).unwrap();
                    symlink(&target, &full).unwrap();
                },
                "hardlink" => {
                    let target = root.join("target.rs");
                    fs::write(&target, data).unwrap();
                    fs::hard_link(&target, &full).unwrap();
                },
                "special" => fs::create_dir(&full).unwrap(),
                _ => unreachable!(),
            }
            let entry = SourceEntry {
                path: path.to_owned(),
                origin: "mutable_edge_runtime".to_owned(),
                mode: "0644".to_owned(),
                size: data.len() as u64,
                sha256: super::sha256(data),
            };
            let snapshot = SourceSnapshot {
                root,
                source_id: format!("cpu-edge:{}", "b".repeat(64)),
                repository_commit: "c".repeat(40),
                entries: BTreeMap::from([(path.to_owned(), entry)]),
                vendor_versions: BTreeMap::new(),
                vendor_checksums: BTreeMap::new(),
                local_packages: BTreeSet::new(),
                vendor_attestation_sha256: super::sha256(&[]),
                lineage_base_generation: None,
                lineage_parent_source_id: None,
            };
            assert!(snapshot.read(path, None, 0, 100).is_err());
        }

        let hidden = SourceEntry {
            path: "source/services/.hidden/lib.rs".to_owned(),
            origin: "mutable_edge_runtime".to_owned(),
            mode: "0644".to_owned(),
            size: 1,
            sha256: "a".repeat(64),
        };
        assert!(!super::is_visible_source(&hidden));
    }

    #[test]
    fn dependency_changes_require_exact_signed_vendor_availability() {
        let snapshot = SourceSnapshot {
            root: std::path::PathBuf::from("/not-read"),
            source_id: format!("cpu-edge:{}", "b".repeat(64)),
            repository_commit: "c".repeat(40),
            entries: BTreeMap::new(),
            vendor_versions: BTreeMap::from([(
                "serde".to_owned(),
                vec![Version::parse("1.0.200").unwrap()],
            )]),
            vendor_checksums: BTreeMap::from([(
                ("serde".to_owned(), Version::parse("1.0.200").unwrap()),
                Some("d".repeat(64)),
            )]),
            local_packages: BTreeSet::from(["astrid-local".to_owned()]),
            vendor_attestation_sha256: super::sha256(&[]),
            lineage_base_generation: None,
            lineage_parent_source_id: None,
        };
        assert!(
            snapshot
                .validate_dependency_changes(&BTreeMap::from([(
                    "source/Cargo.toml".to_owned(),
                    "[dependencies]\nserde = \"^1.0\"\nastrid-local = { path = \"crates/local\" }\n"
                        .to_owned()
                )]))
                .is_ok()
        );
        for denied in [
            "[dependencies]\nunknown = \"1\"\n",
            "[dependencies]\nserde = \"^2\"\n",
            "[dependencies]\nserde = { git = \"https://example.invalid/repo\" }\n",
        ] {
            assert!(
                snapshot
                    .validate_dependency_changes(&BTreeMap::from([(
                        "source/Cargo.toml".to_owned(),
                        denied.to_owned()
                    )]))
                    .is_err()
            );
        }

        let vendored_lock = format!(
            "version = 4\n\n[[package]]\nname = 'serde'\nversion = '1.0.200'\nsource = 'registry+https://github.com/rust-lang/crates.io-index'\nchecksum = '{}'\n",
            "d".repeat(64)
        );
        assert!(
            snapshot
                .validate_dependency_changes(&BTreeMap::from([(
                    "source/Cargo.lock".to_owned(),
                    vendored_lock.clone(),
                )]))
                .is_ok()
        );
        for denied_lock in [
            vendored_lock.replace("1.0.200", "1.0.201"),
            vendored_lock.replace(&"d".repeat(64), &"e".repeat(64)),
            vendored_lock.replace("registry+", "git+"),
        ] {
            assert!(
                snapshot
                    .validate_dependency_changes(&BTreeMap::from([(
                        "source/Cargo.lock".to_owned(),
                        denied_lock,
                    )]))
                    .is_err()
            );
        }
    }

    #[test]
    fn reviewed_manifests_and_operator_lock_are_in_mutable_surface() {
        for (path, origin) in [
            ("source/Cargo.toml", "mutable_build_manifest"),
            ("source/Cargo.lock", "mutable_build_manifest"),
            (
                "source/crates/astrid-kernel/Cargo.toml",
                "mutable_build_manifest",
            ),
            (
                "source/services/astrid-edge-runtime/Cargo.lock",
                "mutable_build_manifest",
            ),
            (
                "source/capsules/astralis/astrid-capsule-edge-context/Cargo.lock",
                "mutable_build_manifest",
            ),
        ] {
            assert!(is_mutable_source(&SourceEntry {
                path: path.to_owned(),
                origin: origin.to_owned(),
                mode: "0644".to_owned(),
                size: 10,
                sha256: "a".repeat(64),
            }));
        }
    }

    #[test]
    fn immutable_rescue_source_is_inspect_only_and_never_mutable() {
        for path in [
            "source/services/astrid-edge-steward-helper/src/main.rs",
            "source/services/astrid-edge-rescue-helper/src/main.rs",
            "source/services/astrid-edge-web-broker/src/main.rs",
            "source/services/astrid-edge-checkpoint/src/main.rs",
            "source/packaging/systemd/astrid-edge-web-broker-runtime.socket",
            "source/packaging/systemd/astrid-edge-web-broker-runtime.service",
            "source/packaging/systemd/astrid-edge-web-broker-steward.socket",
            "source/packaging/systemd/astrid-edge-web-broker-steward.service",
        ] {
            let entry = SourceEntry {
                path: path.to_owned(),
                origin: INSPECT_ONLY_ORIGIN.to_owned(),
                mode: "0644".to_owned(),
                size: 10,
                sha256: "a".repeat(64),
            };
            assert!(is_immutable_source_domain(path));
            assert!(super::is_visible_source(&entry));
            assert!(!is_mutable_source(&entry));

            let forged_mutable = SourceEntry {
                origin: "mutable_edge_runtime".to_owned(),
                ..entry
            };
            assert!(!super::is_visible_source(&forged_mutable));
            assert!(!service_role_is_valid(path, &forged_mutable.origin));
        }
    }

    #[test]
    fn only_six_semantically_verified_service_fragments_are_mutable() {
        for path in [
            "source/packaging/systemd/astrid.service",
            "source/packaging/systemd/astrid-edge-runtime.service",
            "source/packaging/systemd/icp/ollama-cpu.service",
            "source/packaging/systemd/icp/astrid-edge-hindsight.timer",
        ] {
            assert!(is_mutable_unit_source(path));
            assert!(is_mutable_source(&SourceEntry {
                path: path.to_owned(),
                origin: "mutable_astrid_service_template".to_owned(),
                mode: "0644".to_owned(),
                size: 10,
                sha256: "a".repeat(64),
            }));
            assert!(!is_mutable_source(&SourceEntry {
                path: path.to_owned(),
                origin: "build_required_service_template".to_owned(),
                mode: "0644".to_owned(),
                size: 10,
                sha256: "a".repeat(64),
            }));
        }
        for path in [
            "source/packaging/systemd/astrid-edge-steward.service",
            "source/packaging/systemd/astrid-edge-builder-store-verify.service.in",
            "source/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in",
            "source/packaging/systemd/icp/../astrid.service",
        ] {
            assert!(!is_mutable_unit_source(path));
        }
        assert!(service_role_is_valid(
            "source/packaging/systemd/astrid.service",
            "mutable_astrid_service_template"
        ));
        assert!(!service_role_is_valid(
            "source/packaging/systemd/astrid.service",
            "build_required_service_template"
        ));
        assert!(!service_role_is_valid(
            "source/packaging/systemd/astrid-edge-steward.service",
            "mutable_astrid_service_template"
        ));
        assert!(!service_role_is_valid(
            "source/packaging/systemd/astrid-edge-steward.service",
            "build_required_service_template"
        ));
        assert!(service_role_is_valid(
            "source/packaging/systemd/astrid-edge-steward.service",
            INSPECT_ONLY_ORIGIN
        ));
        assert!(!service_role_is_valid(
            "source/packaging/systemd/astrid-edge-steward.service",
            "mutable_core_source"
        ));
        assert!(!service_role_is_valid(
            "source/crates/astrid-kernel/src/lib.rs",
            "mutable_astrid_service_template"
        ));
    }
}
