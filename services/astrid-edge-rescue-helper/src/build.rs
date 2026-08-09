//! Confined candidate materialization, fixed offline gates, and immutable build evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::config::{Config, TrustedExecutable};
use crate::fs_guard::{
    atomic_write, canonical_json, ensure_within, make_read_only_tree, read_json, read_regular,
    sha256, validate_relative,
};
use crate::generation::{GenerationManifest, validate_release_manifest};
use crate::invariant::{ESSENTIAL_CAPSULES, ESSENTIAL_SCRIPTS, REBUILDABLE_CAPSULES};
use crate::manifest::{BuildV1, Candidate, PATCH_SCHEMA, PatchBundle, SourceSnapshot};
use crate::native::{
    CandidateRunner, CommandExecutionBoundary, CommandReceipt, CommandSpec, fixed_environment,
    reconcile_candidate_transients,
};
use crate::transition::{MaintenanceLease, active_target, read_generation_binding};
use crate::{Error, ErrorKind, Result};

const MAX_BUILD_FILE: u64 = 512 * 1024 * 1024;
const MAX_RETAINED_TERMINAL_BUILDS: usize = 4;
const SEALED_BUILD_ENTRIES: &[&str] = &[
    "build.json",
    "bundle",
    "bundle-inventory.json",
    "candidate-patch.json",
    "evidence.json",
];
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Impact {
    Core,
    Edge,
    Capsule,
    Python,
    Unit,
    Profile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecutionBoundary {
    CandidateTransient,
    TrustedHost,
}

#[derive(Debug, Clone)]
struct BuildStep {
    boundary: ExecutionBoundary,
    command: CommandSpec,
}

impl BuildStep {
    fn candidate(command: CommandSpec) -> Self {
        Self {
            boundary: ExecutionBoundary::CandidateTransient,
            command,
        }
    }

    fn trusted_host(command: CommandSpec) -> Self {
        Self {
            boundary: ExecutionBoundary::TrustedHost,
            command,
        }
    }
}

#[derive(Debug, Serialize)]
struct Evidence<'a> {
    schema: &'static str,
    candidate_id: &'a str,
    source_id: &'a str,
    source_revision: &'a str,
    commands: &'a [CommandReceipt],
    candidate_replay_sha256: String,
    package_replay_sha256: String,
    immutable_invariants: bool,
    offline_locked: bool,
    network_policy: &'static str,
}

#[derive(Debug, Serialize)]
struct BundleInventory {
    schema: &'static str,
    files: Vec<BundleFile>,
}

#[derive(Debug, Serialize)]
struct BundleFile {
    path: String,
    size: u64,
    mode: String,
    sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BaseBoundPatch {
    schema: String,
    candidate: Candidate,
    patch: PatchBundle,
}

pub struct Builder<'a, R: CandidateRunner> {
    config: &'a Config,
    runner: &'a mut R,
}

impl<'a, R: CandidateRunner> Builder<'a, R> {
    pub fn new(config: &'a Config, runner: &'a mut R) -> Self {
        Self { config, runner }
    }

    #[allow(clippy::too_many_lines)]
    pub fn build(
        &mut self,
        candidate_manifest: &Path,
        intent_envelope: &Path,
        model_handoff: &Path,
        output_manifest: &Path,
    ) -> Result<BuildV1> {
        self.build_inner(
            candidate_manifest,
            Some((intent_envelope, model_handoff)),
            output_manifest,
        )
    }

    /// Exercise the exact offline pipeline for the internally generated no-op
    /// candidate. This entry point accepts no model handoff and is never used
    /// for a production candidate.
    pub fn build_synthetic(
        &mut self,
        candidate_manifest: &Path,
        output_manifest: &Path,
    ) -> Result<BuildV1> {
        self.build_inner(candidate_manifest, None, output_manifest)
    }

    /// Run the immutable synthetic build while the caller holds the exact
    /// maintenance lease. Only the root-owned synthetic lifecycle harness uses
    /// this entry point, after it has drained work and unloaded the model.
    pub(crate) fn build_synthetic_under_maintenance(
        &mut self,
        candidate_manifest: &Path,
        output_manifest: &Path,
    ) -> Result<BuildV1> {
        self.build_transaction(candidate_manifest, None, output_manifest)
    }

    #[allow(clippy::too_many_lines)]
    fn build_inner(
        &mut self,
        candidate_manifest: &Path,
        authored_handoff: Option<(&Path, &Path)>,
        output_manifest: &Path,
    ) -> Result<BuildV1> {
        let lease = MaintenanceLease::acquire_for(
            self.config,
            "candidate_build",
            self.config.policy.pipeline_timeout_seconds,
        )?;
        let _drain = lease.wait_for_exact_drain(self.config)?;
        let stop = crate::model_service::stop(self.config, self.runner);
        if let Err(stop_error) = stop {
            let _ = crate::model_service::restore(self.config, self.runner);
            return Err(stop_error);
        }
        let transaction =
            self.build_transaction(candidate_manifest, authored_handoff, output_manifest);
        let restoration = crate::model_service::restore(self.config, self.runner);
        match (transaction, restoration) {
            (Ok(build), Ok(_)) => Ok(build),
            (Err(build_error), Ok(_)) => Err(build_error),
            (Ok(_), Err(restore_error)) => Err(restore_error),
            (Err(build_error), Err(restore_error)) => Err(Error::new(format!(
                "candidate build failed and model restoration also failed: {}; {}",
                build_error.message(),
                restore_error.message()
            ))),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn build_transaction(
        &mut self,
        candidate_manifest: &Path,
        authored_handoff: Option<(&Path, &Path)>,
        output_manifest: &Path,
    ) -> Result<BuildV1> {
        let pipeline_started = Instant::now();
        self.preflight()?;
        let candidate = Candidate::from_file(candidate_manifest, self.config)?;
        if let Some((intent_envelope, model_handoff)) = authored_handoff {
            crate::handoff::verify(self.config, model_handoff, intent_envelope, &candidate)?;
        } else if !candidate.candidate_id.starts_with("synthetic-") {
            return Err(Error::new(
                "non-authored build entry accepts only an immutable synthetic candidate",
            ));
        }
        let (base_generation, base_identity) = self.validate_base_generation(&candidate)?;
        let candidate_digest = candidate.digest()?;
        let build_id = format!("build-{}", &candidate_digest[..20]);
        let generation_id = format!("gen-{}", &candidate_digest[..24]);
        let scratch = self.config.roots.candidate_work.join(&build_id);
        let final_build = self.config.roots.build_store.join(&build_id);
        ensure_within(&self.config.roots.candidate_work, &scratch, false)?;
        ensure_within(&self.config.roots.build_store, &final_build, false)?;
        if scratch.exists() || scratch.is_symlink() {
            cleanup_completed_scratch(self.config, &scratch)?;
        }
        if final_build.exists() || final_build.is_symlink() {
            let recovered = verify_sealed_build(
                self.config,
                &final_build,
                Some((&candidate, &candidate_digest)),
            )?;
            write_or_verify_manifest(output_manifest, &canonical_json(&recovered)?)?;
            return Ok(recovered);
        }
        fs::create_dir(&scratch)?;
        prepare_builder_traversal_directory(&scratch, self.config.identities.builder_gid)?;
        let source = scratch.join("source");
        fs::create_dir(&source)?;
        let snapshot = SourceSnapshot::load_for_generation(
            self.config,
            &base_generation,
            base_identity.operator_initial,
        )?;
        let vendor_before = snapshot.verify_vendor_tree()?;
        snapshot.copy_source_tree(&source)?;
        let patch = match snapshot.validate_and_apply(self.config, &candidate, &source) {
            Ok(patch) => patch,
            Err(error) => {
                return Err(candidate_rejected_after_cleanup(
                    self.config,
                    &scratch,
                    "candidate patch validation",
                    &error,
                ));
            },
        };
        if let Err(error) = crate::profile_projection::validate_candidate_profile_source(
            self.config,
            &source,
            &base_generation,
            &candidate.changed_paths,
        ) {
            return Err(candidate_rejected_after_cleanup(
                self.config,
                &scratch,
                "candidate service/profile envelope",
                &error,
            ));
        }
        if authored_handoff.is_none()
            && patch
                .files
                .iter()
                .any(|file| file.source_sha256 != file.content_sha256)
        {
            return Err(Error::new(
                "operator synthetic candidate attempted a source mutation",
            ));
        }
        let derived_snapshot = scratch.join("source-snapshot");
        let _derived_source_id =
            snapshot.export_derived(self.config, &candidate, &source, &derived_snapshot)?;
        let cargo_policy = self.write_cargo_policy(&source, &scratch)?;
        make_read_only_tree(&source)?;
        let impacts = classify(&candidate.changed_paths);
        let capsules_to_build = selected_capsules(&candidate.changed_paths);
        let capsule_targets = prepare_capsule_targets(
            &source,
            &capsules_to_build,
            self.config.identities.builder_uid,
            self.config.identities.builder_gid,
        )?;
        let tool_shims = self.prepare_tool_shims(&scratch)?;
        let mut receipts = Vec::new();
        let mut source_reverified = false;
        for spec in
            self.command_plan(&source, &scratch, &impacts, &capsules_to_build, &tool_shims)?
        {
            if pipeline_started.elapsed()
                >= Duration::from_secs(self.config.policy.pipeline_timeout_seconds)
            {
                if receipts.is_empty() {
                    let error = Error::deferred(
                        "candidate build deferred by immutable pipeline wall-time gate",
                    );
                    cleanup_unstarted_scratch(self.config, &scratch)?;
                    return Err(error);
                }
                return Err(Error::new(
                    "candidate build exceeded immutable pipeline wall-time after execution began",
                ));
            }
            if let Err(error) = Self::resource_gate(self.config) {
                if receipts.is_empty() {
                    if error.kind() == ErrorKind::DeferredInfrastructure {
                        cleanup_unstarted_scratch(self.config, &scratch)?;
                    }
                    return Err(error);
                }
                if error.kind() == ErrorKind::DeferredInfrastructure {
                    cleanup_completed_scratch(self.config, &scratch).map_err(|cleanup| {
                        Error::new(format!(
                            "candidate resource deferral could not securely discard scratch: {cleanup}"
                        ))
                    })?;
                }
                return Err(error);
            }
            if spec.boundary == ExecutionBoundary::TrustedHost && !source_reverified {
                cleanup_capsule_targets(&capsule_targets)?;
                snapshot.verify_post_build_tree(&source, &patch, &cargo_policy)?;
                source_reverified = true;
            }
            let receipt = self.run_with_health(&spec, &scratch)?;
            if let Err(error) = require_candidate_gate_success(&receipt) {
                return Err(candidate_rejected_after_cleanup(
                    self.config,
                    &scratch,
                    "fixed build gate",
                    &error,
                ));
            }
            receipts.push(receipt);
        }
        let vendor_after = snapshot.verify_vendor_tree()?;
        if vendor_after != vendor_before {
            return Err(Error::new(
                "signed vendor attestation changed across candidate execution",
            ));
        }
        let bundle = scratch.join("bundle");
        fs::create_dir(&bundle)?;
        Self::assemble_bundle(
            &source,
            &scratch.join("target"),
            &base_generation,
            &capsules_to_build,
            &derived_snapshot,
            &bundle,
        )?;
        crate::profile_projection::write_release_projection_manifest(
            self.config,
            &candidate.changed_paths,
            &bundle,
            &base_generation,
        )?;
        // Nothing below this point is a compiler. `assemble_bundle` copied candidate outputs into
        // root/helper-owned directories with non-writable modes. Execute fixtures only from that
        // sealed copy: a candidate `--help` path cannot rewrite a sibling binary, capsule archive,
        // or its own bytes between verification and packaging.
        for spec in self.candidate_binary_fixtures(&bundle, &scratch)? {
            let receipt = self.run_with_health(&spec, &scratch)?;
            if let Err(error) = require_candidate_gate_success(&receipt) {
                return Err(candidate_rejected_after_cleanup(
                    self.config,
                    &scratch,
                    "candidate executable fixture",
                    &error,
                ));
            }
            receipts.push(receipt);
        }
        let capsule_home = scratch.join("capsule-home");
        fs::create_dir(&capsule_home)?;
        prepare_builder_directory(
            &capsule_home,
            self.config.identities.builder_uid,
            self.config.identities.builder_gid,
        )?;
        for spec in self.capsule_install_fixtures(&bundle, &capsule_home, &scratch)? {
            let receipt = self.run_with_health(&spec, &scratch)?;
            if let Err(error) = require_candidate_gate_success(&receipt) {
                return Err(candidate_rejected_after_cleanup(
                    self.config,
                    &scratch,
                    "candidate capsule fixture",
                    &error,
                ));
            }
            receipts.push(receipt);
        }
        crate::invariant::finalize_installed_capsules(
            &bundle,
            &capsule_home.join("home/default/.local/capsules"),
        )?;
        fs::remove_dir_all(&capsule_home)?;
        if snapshot.verify_vendor_tree()? != vendor_before {
            return Err(Error::new(
                "signed vendor attestation changed during candidate fixture execution",
            ));
        }
        let package_receipt = self.run_with_health(
            &BuildStep::trusted_host(CommandSpec {
                label: "immutable-package-replay",
                executable: self.config.executables.package_verifier.clone(),
                arguments: vec![
                    "verify-package".into(),
                    "--bundle-root".into(),
                    bundle.display().to_string(),
                    "--source-root".into(),
                    source.display().to_string(),
                    "--target".into(),
                    self.config.target.clone(),
                    "--evidence".into(),
                    scratch.join("package-replay.json").display().to_string(),
                ],
                current_dir: scratch.clone(),
                environment: BTreeMap::new(),
                timeout: Duration::from_secs(self.config.policy.command_timeout_seconds),
                run_as_uid: None,
                run_as_gid: None,
            }),
            &scratch,
        )?;
        if let Err(error) = require_candidate_gate_success(&package_receipt) {
            return Err(candidate_rejected_after_cleanup(
                self.config,
                &scratch,
                "immutable package replay",
                &error,
            ));
        }
        receipts.push(package_receipt);
        let candidate_replay_sha256 =
            crate::fs_guard::sha256_file(&scratch.join("invariant-replay.json"), 1024 * 1024)?;
        let package_replay_sha256 =
            crate::fs_guard::sha256_file(&scratch.join("package-replay.json"), 1024 * 1024)?;
        let evidence = Evidence {
            schema: "astrid.edge_rescue_helper.build_evidence.v1",
            candidate_id: &candidate.candidate_id,
            source_id: &snapshot.source_id,
            source_revision: &snapshot.repository_commit,
            commands: &receipts,
            candidate_replay_sha256,
            package_replay_sha256,
            immutable_invariants: true,
            offline_locked: true,
            network_policy: "private-network-none:v1",
        };
        let evidence_bytes = canonical_json(&evidence)?;
        let tests_sha256 = sha256(&evidence_bytes);
        let inventory = inventory(&bundle)?;
        let inventory_bytes = canonical_json(&inventory)?;
        let bundle_sha256 = sha256(&inventory_bytes);
        let build = BuildV1 {
            schema: "astrid.edge_self_change.build.v1".to_owned(),
            appliance_id: self.config.appliance_id.clone(),
            build_id,
            candidate_id: candidate.candidate_id.clone(),
            candidate_sha256: candidate_digest.clone(),
            base_generation: candidate.base_generation.clone(),
            generation_id,
            source_revision: snapshot.repository_commit,
            bundle_sha256,
            tests_sha256,
            target: self.config.target.clone(),
            created_at: unix_seconds(),
            privilege_envelope: "offline-build-sandbox:no-host-state:v1".to_owned(),
        };
        build.validate(self.config)?;
        let build_bytes = canonical_json(&build)?;
        if patch.files.len() != candidate.changed_paths.len() {
            return Err(Error::new(
                "candidate patch file count changed during build",
            ));
        }
        let raw_patch_bytes = canonical_json(&patch)?;
        if sha256(&raw_patch_bytes) != candidate.patch_sha256 {
            return Err(Error::new(
                "candidate patch changed before sealed evidence publication",
            ));
        }
        let patch_bytes = canonical_json(&BaseBoundPatch {
            schema: "astrid.edge_rescue_helper.base_bound_patch.v1".to_owned(),
            candidate: candidate.clone(),
            patch,
        })?;
        let sealed = scratch.join("sealed");
        fs::create_dir(&sealed)?;
        fs::set_permissions(&sealed, fs::Permissions::from_mode(0o700))?;
        fs::rename(&bundle, sealed.join("bundle"))?;
        for (name, bytes) in [
            ("build.json", build_bytes.as_slice()),
            ("bundle-inventory.json", inventory_bytes.as_slice()),
            ("candidate-patch.json", patch_bytes.as_slice()),
            ("evidence.json", evidence_bytes.as_slice()),
        ] {
            atomic_write(&sealed.join(name), bytes, 0o400, false)?;
        }
        make_read_only_tree(&sealed)?;
        let sealed_build =
            verify_sealed_build(self.config, &sealed, Some((&candidate, &candidate_digest)))?;
        if canonical_json(&sealed_build)? != build_bytes {
            return Err(Error::new("sealed Build v1 changed before publication"));
        }
        fs::rename(&sealed, &final_build)?;
        fs::File::open(&self.config.roots.build_store)?.sync_all()?;
        cleanup_completed_scratch(self.config, &scratch)?;
        write_or_verify_manifest(output_manifest, &build_bytes)?;
        Ok(build)
    }

    fn preflight(&self) -> Result<()> {
        let _ = crate::storage::verify(self.config, false)?;
        for root in [&self.config.source.root, &self.config.roots.candidate_store] {
            let metadata = fs::symlink_metadata(root)?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(Error::new(
                    "immutable build input root is unavailable or linked",
                ));
            }
        }
        for root in [
            &self.config.roots.candidate_work,
            &self.config.roots.build_store,
        ] {
            let metadata = fs::symlink_metadata(root)?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(Error::new("builder writable root is unavailable or linked"));
            }
        }
        if !same_filesystem(
            &self.config.roots.candidate_work,
            &self.config.roots.build_store,
        )? {
            return Err(Error::new(
                "candidate work and build store must share the bounded builder filesystem",
            ));
        }
        gc_terminal_builds(self.config, None)?;
        let available = fs2::available_space(&self.config.roots.candidate_work)?;
        if available < self.config.policy.minimum_free_disk_bytes {
            return Err(Error::deferred("insufficient free disk for confined build"));
        }
        Ok(())
    }

    fn run_with_health(&mut self, step: &BuildStep, scratch: &Path) -> Result<CommandReceipt> {
        let config = self.config;
        if step.boundary == ExecutionBoundary::TrustedHost {
            match reconcile_candidate_transients(config) {
                Ok(false) => {},
                Ok(true) => {
                    cleanup_completed_scratch(config, scratch).map_err(|cleanup| {
                        Error::new(format!(
                            "pre-invariant candidate cleanup could not securely discard scratch: {cleanup}"
                        ))
                    })?;
                    return Err(Error::new(
                        "candidate transient activity crossed into a trusted-host invariant boundary",
                    ));
                },
                Err(error) => {
                    cleanup_completed_scratch(config, scratch).map_err(|cleanup| {
                        Error::new(format!(
                            "pre-invariant reconciliation and scratch cleanup failed: {error}; {cleanup}"
                        ))
                    })?;
                    return Err(Error::new(format!(
                        "cannot prove a clean candidate boundary before trusted-host invariants: {error}"
                    )));
                },
            }
        }
        let outcome = match step.boundary {
            ExecutionBoundary::CandidateTransient => {
                self.runner
                    .run_candidate_monitored(config, scratch, &step.command, &mut || {
                        Self::resource_gate(config)
                    })
            },
            ExecutionBoundary::TrustedHost => self
                .runner
                .run_monitored(&step.command, &mut || Self::resource_gate(config)),
        };
        let outcome = if step.boundary == ExecutionBoundary::TrustedHost {
            enforce_trusted_host_quiescence(outcome, reconcile_candidate_transients(config))
        } else {
            outcome
        };
        match outcome {
            Ok(receipt)
                if receipt.execution_boundary
                    != match step.boundary {
                        ExecutionBoundary::CandidateTransient => {
                            CommandExecutionBoundary::CandidateTransient
                        },
                        ExecutionBoundary::TrustedHost => CommandExecutionBoundary::TrustedHost,
                    } =>
            {
                cleanup_completed_scratch(config, scratch).map_err(|cleanup| {
                    Error::new(format!(
                        "execution-boundary mismatch could not securely discard scratch: {cleanup}"
                    ))
                })?;
                Err(Error::new(
                    "native runner returned a receipt from the wrong execution boundary",
                ))
            },
            Err(error) if error.kind() == ErrorKind::DeferredInfrastructure => {
                cleanup_completed_scratch(config, scratch).map_err(|cleanup| {
                    Error::new(format!(
                        "candidate health abort could not securely discard scratch: {cleanup}"
                    ))
                })?;
                Err(error)
            },
            Err(error) if step.boundary == ExecutionBoundary::TrustedHost => {
                cleanup_completed_scratch(config, scratch).map_err(|cleanup| {
                    Error::new(format!(
                        "trusted-host invariant failure could not securely discard scratch: {cleanup}"
                    ))
                })?;
                Err(error)
            },
            outcome => outcome,
        }
    }

    fn resource_gate(config: &Config) -> Result<()> {
        let thermal = read_regular(&config.health.thermal_celsius, 128)?;
        let thermal = String::from_utf8_lossy(&thermal)
            .trim()
            .parse::<f64>()
            .map_err(|_| Error::new("thermal sensor is malformed"))?;
        if !thermal.is_finite() || thermal > config.health.maximum_thermal_celsius {
            return Err(Error::deferred("build deferred by immutable thermal gate"));
        }
        if fs2::available_space(&config.roots.candidate_work)?
            < config.policy.minimum_free_disk_bytes
        {
            return Err(Error::deferred("build deferred by immutable disk gate"));
        }
        if crate::health::mem_available(&config.health.meminfo)?
            < config.health.minimum_available_ram_bytes
        {
            return Err(Error::deferred("build deferred by immutable RAM gate"));
        }
        if crate::health::swap_used(&config.health.swaps)? > config.health.maximum_swap_bytes {
            return Err(Error::deferred("build deferred by immutable swap gate"));
        }
        Ok(())
    }

    fn validate_base_generation(
        &self,
        candidate: &Candidate,
    ) -> Result<(PathBuf, crate::generation::ReleaseIdentity)> {
        if read_generation_binding(self.config, true)? != candidate.base_generation {
            return Err(Error::new(
                "candidate base generation differs from immutable supervisor state",
            ));
        }
        let base = self.config.roots.releases.join(&candidate.base_generation);
        let identity = validate_release_manifest(self.config, &base)?;
        if identity.generation_id != candidate.base_generation
            || active_target(self.config)? != fs::canonicalize(&base)?
        {
            return Err(Error::new(
                "candidate base generation differs from active immutable release",
            ));
        }
        Ok((base, identity))
    }

    fn prepare_tool_shims(&self, scratch: &Path) -> Result<PathBuf> {
        let directory = scratch.join("tool-shims");
        fs::create_dir(&directory)?;
        let shim = directory.join("rustup");
        let bytes = read_regular(
            &self.config.executables.invariant_runner.path,
            256 * 1024 * 1024,
        )?;
        if sha256(&bytes) != self.config.executables.invariant_runner.sha256 {
            return Err(Error::new("pinned rescue-helper rustup shim digest failed"));
        }
        atomic_write(&shim, &bytes, 0o555, false)?;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o555))?;
        Ok(directory)
    }

    fn write_cargo_policy(&self, source: &Path, scratch: &Path) -> Result<Vec<u8>> {
        let cargo_home = scratch.join("cargo-home");
        fs::create_dir(&cargo_home)?;
        let config_dir = source.join(".cargo");
        fs::create_dir_all(&config_dir)?;
        let vendor = self
            .config
            .source
            .vendor
            .display()
            .to_string()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        let policy = format!(
            "[net]\noffline = true\ngit-fetch-with-cli = false\n[source.crates-io]\nreplace-with = \"astrid-signed-vendor\"\n[source.astrid-signed-vendor]\ndirectory = \"{vendor}\"\n"
        );
        let path = config_dir.join("config.toml");
        if path.exists() {
            fs::remove_file(&path)?;
        }
        atomic_write(&path, policy.as_bytes(), 0o400, false)?;
        Ok(policy.into_bytes())
    }

    #[allow(clippy::too_many_lines)]
    fn command_plan(
        &self,
        source: &Path,
        scratch: &Path,
        impacts: &BTreeSet<Impact>,
        capsules_to_build: &BTreeSet<String>,
        tool_shims: &Path,
    ) -> Result<Vec<BuildStep>> {
        let target = scratch.join("target");
        fs::create_dir(&target)?;
        let cargo_home = scratch.join("cargo-home");
        prepare_builder_directory(
            &target,
            self.config.identities.builder_uid,
            self.config.identities.builder_gid,
        )?;
        prepare_builder_directory(
            &cargo_home,
            self.config.identities.builder_uid,
            self.config.identities.builder_gid,
        )?;
        let environment = fixed_environment(
            &cargo_home,
            &target,
            &self.config.executables.rustc.path,
            &self.config.executables.rustfmt.path,
            self.config.policy.build_workers,
        );
        let timeout = Duration::from_secs(self.config.policy.command_timeout_seconds);
        let cargo = |label, arguments: &[&str], current_dir: &Path| {
            BuildStep::candidate(CommandSpec {
                label,
                executable: self.config.executables.cargo.clone(),
                arguments: arguments.iter().map(|value| (*value).to_owned()).collect(),
                current_dir: current_dir.to_path_buf(),
                environment: environment.clone(),
                timeout,
                run_as_uid: Some(self.config.identities.builder_uid),
                run_as_gid: Some(self.config.identities.builder_gid),
            })
        };
        let mut plan = vec![cargo(
            "cargo-metadata",
            &["metadata", "--offline", "--locked", "--format-version", "1"],
            source,
        )];
        if impacts.contains(&Impact::Core) {
            plan.push(cargo(
                "core-fmt",
                &["fmt", "--all", "--", "--check"],
                source,
            ));
            plan.push(cargo(
                "core-clippy",
                &[
                    "clippy",
                    "--workspace",
                    "--all-features",
                    "--offline",
                    "--locked",
                    "--",
                    "-D",
                    "warnings",
                ],
                source,
            ));
            plan.push(cargo(
                "core-tests",
                &[
                    "test",
                    "--workspace",
                    "--exclude",
                    "astrid-openclaw",
                    "--all-features",
                    "--offline",
                    "--locked",
                ],
                source,
            ));
            plan.push(cargo(
                "openclaw-compile-tests-no-run",
                &[
                    "test",
                    "--package",
                    "astrid-openclaw",
                    "--all-features",
                    "--tests",
                    "--no-run",
                    "--offline",
                    "--locked",
                ],
                source,
            ));
            plan.push(cargo(
                "openclaw-deterministic-tests",
                &[
                    "test",
                    "--package",
                    "astrid-openclaw",
                    "--test",
                    "e2e_plugin",
                    "--test",
                    "manifest_roundtrip",
                    "--offline",
                    "--locked",
                ],
                source,
            ));
        }
        let edge_manifest = source.join("services/astrid-edge-runtime/Cargo.toml");
        if impacts.contains(&Impact::Edge) {
            let manifest = edge_manifest.display().to_string();
            plan.push(cargo_owned(
                "edge-fmt",
                &self.config.executables.cargo,
                vec![
                    "fmt".into(),
                    "--manifest-path".into(),
                    manifest.clone(),
                    "--".into(),
                    "--check".into(),
                ],
                source,
                &environment,
                timeout,
                self.config.identities.builder_uid,
                self.config.identities.builder_gid,
            ));
            plan.push(cargo_owned(
                "edge-clippy",
                &self.config.executables.cargo,
                vec![
                    "clippy".into(),
                    "--manifest-path".into(),
                    manifest.clone(),
                    "--all-features".into(),
                    "--offline".into(),
                    "--locked".into(),
                    "--".into(),
                    "-D".into(),
                    "warnings".into(),
                ],
                source,
                &environment,
                timeout,
                self.config.identities.builder_uid,
                self.config.identities.builder_gid,
            ));
            plan.push(cargo_owned(
                "edge-tests",
                &self.config.executables.cargo,
                vec![
                    "test".into(),
                    "--manifest-path".into(),
                    manifest,
                    "--all-features".into(),
                    "--offline".into(),
                    "--locked".into(),
                ],
                source,
                &environment,
                timeout,
                self.config.identities.builder_uid,
                self.config.identities.builder_gid,
            ));
        }
        for capsule in capsules_to_build {
            let labels = capsule_command_labels(capsule)?;
            let capsule_root = source.join("capsules/astralis").join(capsule);
            let manifest = capsule_root.join("Cargo.toml").display().to_string();
            let mut capsule_environment = environment.clone();
            capsule_environment.insert(
                "CARGO_TARGET_DIR".to_owned(),
                capsule_root.join("target").display().to_string(),
            );
            let cargo_parent = self
                .config
                .executables
                .cargo
                .path
                .parent()
                .ok_or_else(|| Error::new("pinned Cargo executable has no parent"))?;
            capsule_environment.insert(
                "PATH".to_owned(),
                format!("{}:{}", tool_shims.display(), cargo_parent.display()),
            );
            plan.push(cargo_owned(
                labels.clippy,
                &self.config.executables.cargo,
                vec![
                    "clippy".into(),
                    "--manifest-path".into(),
                    manifest.clone(),
                    "--all-targets".into(),
                    "--all-features".into(),
                    "--offline".into(),
                    "--locked".into(),
                    "--".into(),
                    "-D".into(),
                    "warnings".into(),
                ],
                source,
                &capsule_environment,
                timeout,
                self.config.identities.builder_uid,
                self.config.identities.builder_gid,
            ));
            plan.push(cargo_owned(
                labels.test,
                &self.config.executables.cargo,
                vec![
                    "test".into(),
                    "--manifest-path".into(),
                    manifest.clone(),
                    "--offline".into(),
                    "--locked".into(),
                ],
                source,
                &capsule_environment,
                timeout,
                self.config.identities.builder_uid,
                self.config.identities.builder_gid,
            ));
            plan.push(cargo_owned(
                labels.wasip2_build,
                &self.config.executables.cargo,
                vec![
                    "build".into(),
                    "--manifest-path".into(),
                    manifest,
                    "--target".into(),
                    "wasm32-wasip2".into(),
                    "--release".into(),
                    "--offline".into(),
                    "--locked".into(),
                ],
                source,
                &capsule_environment,
                timeout,
                self.config.identities.builder_uid,
                self.config.identities.builder_gid,
            ));
            plan.push(BuildStep::candidate(CommandSpec {
                label: labels.package,
                executable: self.config.executables.capsule_builder.clone(),
                arguments: vec![
                    capsule_root.display().to_string(),
                    "--output".into(),
                    target.join("capsule-archives").display().to_string(),
                    "--type".into(),
                    "rust-component".into(),
                ],
                current_dir: source.to_path_buf(),
                environment: capsule_environment,
                timeout,
                run_as_uid: Some(self.config.identities.builder_uid),
                run_as_gid: Some(self.config.identities.builder_gid),
            }));
        }
        if impacts.iter().any(|impact| {
            matches!(
                impact,
                Impact::Core | Impact::Edge | Impact::Python | Impact::Profile | Impact::Unit
            )
        }) {
            // Run only the mutable, appliance-facing report suites.  The
            // signed source snapshot also contains inspect-only rescue-policy
            // tests so Astrid can understand the immutable boundary, but
            // those operator-side tools may require a newer Python than an
            // appliance provides (ICP intentionally remains on Python 3.10).
            // Discovering every `test_*.py` would therefore make an unrelated
            // immutable inspection tool a permanent build veto.  The fixed
            // list below is part of the rescue policy and cannot be widened or
            // replaced by candidate-authored commands.
            for (label, pattern) in [
                ("python-hindsight-tests", "test_edge_hindsight.py"),
                (
                    "python-activity-report-tests",
                    "test_report_edge_activity.py",
                ),
                (
                    "python-appliance-report-tests",
                    "test_report_edge_appliance.py",
                ),
            ] {
                plan.push(BuildStep::candidate(CommandSpec {
                    label,
                    executable: self.config.executables.python.clone(),
                    arguments: vec![
                        "-I".into(),
                        "-m".into(),
                        "unittest".into(),
                        "discover".into(),
                        "-s".into(),
                        "scripts".into(),
                        "-p".into(),
                        pattern.into(),
                    ],
                    current_dir: source.to_path_buf(),
                    environment: BTreeMap::from([
                        ("PYTHONDONTWRITEBYTECODE".to_owned(), "1".to_owned()),
                        ("PYTHONHASHSEED".to_owned(), "0".to_owned()),
                        ("LANG".to_owned(), "C.UTF-8".to_owned()),
                    ]),
                    timeout,
                    run_as_uid: Some(self.config.identities.builder_uid),
                    run_as_gid: Some(self.config.identities.builder_gid),
                }));
            }
        }
        for unit in signed_unit_files(source)? {
            plan.push(BuildStep::candidate(CommandSpec {
                label: "systemd-verify",
                executable: self.config.executables.systemd_analyze.clone(),
                arguments: vec!["verify".to_owned(), unit.display().to_string()],
                current_dir: source.to_path_buf(),
                environment: BTreeMap::from([("LANG".to_owned(), "C.UTF-8".to_owned())]),
                timeout,
                run_as_uid: Some(self.config.identities.builder_uid),
                run_as_gid: Some(self.config.identities.builder_gid),
            }));
        }
        // Release artifacts are complete even when a candidate touches only reports/profiles.
        plan.push(cargo(
            "core-release",
            &[
                "build",
                "--release",
                "--offline",
                "--locked",
                "--package",
                "astrid",
                "--bin",
                "astrid",
                "--bin",
                "astrid-daemon",
                "--bin",
                "astrid-build",
            ],
            source,
        ));
        plan.push(cargo_owned(
            "edge-release",
            &self.config.executables.cargo,
            vec![
                "build".into(),
                "--manifest-path".into(),
                edge_manifest.display().to_string(),
                "--release".into(),
                "--offline".into(),
                "--locked".into(),
            ],
            source,
            &environment,
            timeout,
            self.config.identities.builder_uid,
            self.config.identities.builder_gid,
        ));
        plan.push(BuildStep::trusted_host(CommandSpec {
            label: "immutable-invariants",
            executable: self.config.executables.invariant_runner.clone(),
            arguments: vec![
                "verify-candidate".into(),
                "--source-root".into(),
                source.display().to_string(),
                "--target".into(),
                self.config.target.clone(),
                "--evidence".into(),
                scratch.join("invariant-replay.json").display().to_string(),
            ],
            current_dir: scratch.to_path_buf(),
            environment: BTreeMap::new(),
            timeout,
            run_as_uid: None,
            run_as_gid: None,
        }));
        validate_offline_command_plan(&plan)?;
        Ok(plan)
    }

    fn candidate_binary_fixtures(
        &self,
        binary_root: &Path,
        scratch: &Path,
    ) -> Result<Vec<BuildStep>> {
        let mut result = Vec::new();
        for (label, name) in [
            ("fixture-astrid-help", "astrid"),
            ("fixture-daemon-help", "astrid-daemon"),
            ("fixture-build-help", "astrid-build"),
            ("fixture-edge-help", "astrid-edge-runtime"),
        ] {
            let path = binary_root.join(name);
            let metadata = fs::symlink_metadata(&path)?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.nlink() != 1
                || metadata.uid() != nix::unistd::geteuid().as_raw()
                || metadata.mode() & 0o111 == 0
                || metadata.mode() & 0o222 != 0
            {
                return Err(Error::new(
                    "candidate executable fixture is linked, mutable, non-executable, or has wrong owner",
                ));
            }
            result.push(BuildStep::candidate(CommandSpec {
                label,
                executable: TrustedExecutable {
                    path: path.clone(),
                    sha256: crate::fs_guard::sha256_file(&path, MAX_BUILD_FILE)?,
                },
                arguments: vec!["--help".to_owned()],
                current_dir: scratch.to_path_buf(),
                environment: BTreeMap::from([
                    ("LANG".to_owned(), "C.UTF-8".to_owned()),
                    ("LC_ALL".to_owned(), "C.UTF-8".to_owned()),
                    ("TZ".to_owned(), "UTC".to_owned()),
                ]),
                timeout: Duration::from_secs(30),
                run_as_uid: Some(self.config.identities.builder_uid),
                run_as_gid: Some(self.config.identities.builder_gid),
            }));
        }
        Ok(result)
    }

    fn capsule_install_fixtures(
        &self,
        bundle: &Path,
        capsule_home: &Path,
        scratch: &Path,
    ) -> Result<Vec<BuildStep>> {
        let astrid = bundle.join("astrid");
        let executable = TrustedExecutable {
            path: astrid.clone(),
            sha256: crate::fs_guard::sha256_file(&astrid, MAX_BUILD_FILE)?,
        };
        Ok(ESSENTIAL_CAPSULES
            .iter()
            .map(|capsule| {
                BuildStep::candidate(CommandSpec {
                    label: "fixture-capsule-install",
                    executable: executable.clone(),
                    arguments: vec![
                        "capsule".to_owned(),
                        "install".to_owned(),
                        bundle
                            .join("capsules")
                            .join(format!("{capsule}.capsule"))
                            .display()
                            .to_string(),
                    ],
                    current_dir: scratch.to_path_buf(),
                    environment: BTreeMap::from([
                        ("HOME".to_owned(), capsule_home.display().to_string()),
                        ("ASTRID_HOME".to_owned(), capsule_home.display().to_string()),
                        ("LANG".to_owned(), "C.UTF-8".to_owned()),
                        ("LC_ALL".to_owned(), "C.UTF-8".to_owned()),
                        ("TZ".to_owned(), "UTC".to_owned()),
                    ]),
                    timeout: Duration::from_secs(60),
                    run_as_uid: Some(self.config.identities.builder_uid),
                    run_as_gid: Some(self.config.identities.builder_gid),
                })
            })
            .collect())
    }

    fn assemble_bundle(
        source: &Path,
        target: &Path,
        base_generation: &Path,
        capsules_to_build: &BTreeSet<String>,
        derived_snapshot: &Path,
        bundle: &Path,
    ) -> Result<()> {
        for name in [
            "astrid",
            "astrid-daemon",
            "astrid-build",
            "astrid-edge-runtime",
        ] {
            let built = target.join("release").join(name);
            copy_output(&built, &bundle.join(name), 0o555)?;
        }
        let capsule_output = bundle.join("capsules");
        fs::create_dir(&capsule_output)?;
        for capsule in ESSENTIAL_CAPSULES {
            let archive = format!("{capsule}.capsule");
            let source_archive = if capsules_to_build.contains(*capsule) {
                target.join("capsule-archives").join(&archive)
            } else {
                base_generation.join("capsules").join(&archive)
            };
            let destination = capsule_output.join(archive);
            copy_output(&source_archive, &destination, 0o444)?;
        }
        let scripts = bundle.join("scripts");
        fs::create_dir(&scripts)?;
        for script in ESSENTIAL_SCRIPTS {
            let input = source.join("scripts").join(script);
            copy_output(&input, &scripts.join(script), 0o555)?;
        }
        copy_release_tree(
            &source.join("packaging/appliances"),
            &bundle.join("packaging/appliances"),
        )?;
        copy_release_tree(
            &source.join("packaging/systemd"),
            &bundle.join("packaging/systemd"),
        )?;
        copy_release_tree(derived_snapshot, &bundle.join("source-snapshot"))?;
        Ok(())
    }
}

fn validate_offline_command_plan(plan: &[BuildStep]) -> Result<()> {
    let mut labels = BTreeSet::new();
    for step in plan {
        let spec = &step.command;
        if !labels.insert(spec.label) {
            return Err(Error::new("fixed build plan repeats a command label"));
        }
        validate_offline_command(spec)?;
    }
    validate_openclaw_offline_coverage(plan)
}

fn validate_offline_command(spec: &CommandSpec) -> Result<()> {
    let executable = spec
        .executable
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if matches!(executable, "npm" | "npx" | "node" | "git" | "curl" | "wget")
        || spec.arguments.iter().any(|argument| {
            matches!(
                argument.as_str(),
                "install" | "fetch" | "search" | "publish" | "update"
            )
        })
        || spec.environment.keys().any(|key| {
            let upper = key.to_ascii_uppercase();
            upper.contains("PROXY") || upper.contains("REGISTRY") || upper.starts_with("NPM_")
        })
    {
        return Err(Error::new(
            "fixed build plan contains an external dependency acquisition surface",
        ));
    }
    if executable != "cargo" {
        return Ok(());
    }
    if spec
        .environment
        .get("CARGO_NET_OFFLINE")
        .map(String::as_str)
        != Some("true")
    {
        return Err(Error::new(
            "Cargo command is missing immutable offline mode",
        ));
    }
    let dependency_capable = spec
        .arguments
        .iter()
        .any(|argument| matches!(argument.as_str(), "metadata" | "clippy" | "test" | "build"));
    if dependency_capable
        && (!spec
            .arguments
            .iter()
            .any(|argument| argument == "--offline")
            || !spec.arguments.iter().any(|argument| argument == "--locked"))
    {
        return Err(Error::new(
            "dependency-capable Cargo command is not offline and locked",
        ));
    }
    Ok(())
}

fn validate_openclaw_offline_coverage(plan: &[BuildStep]) -> Result<()> {
    let core = plan
        .iter()
        .map(|step| &step.command)
        .find(|spec| spec.label == "core-tests");
    let compile = plan
        .iter()
        .map(|step| &step.command)
        .find(|spec| spec.label == "openclaw-compile-tests-no-run");
    let deterministic = plan
        .iter()
        .map(|step| &step.command)
        .find(|spec| spec.label == "openclaw-deterministic-tests");
    if let Some(core) = core {
        let excluded = core
            .arguments
            .windows(2)
            .any(|pair| pair == ["--exclude", "astrid-openclaw"]);
        if !excluded || compile.is_none() || deterministic.is_none() {
            return Err(Error::new(
                "OpenClaw network-acquiring tests are not replaced by fixed offline coverage",
            ));
        }
    }
    if let Some(compile) = compile
        && (!compile
            .arguments
            .iter()
            .any(|argument| argument == "--no-run")
            || !compile
                .arguments
                .iter()
                .any(|argument| argument == "--tests"))
    {
        return Err(Error::new(
            "OpenClaw compile coverage may execute integration tests",
        ));
    }
    if let Some(deterministic) = deterministic {
        let tests = deterministic
            .arguments
            .windows(2)
            .filter_map(|pair| (pair[0] == "--test").then_some(pair[1].as_str()))
            .collect::<BTreeSet<_>>();
        if tests != BTreeSet::from(["e2e_plugin", "manifest_roundtrip"])
            || deterministic
                .arguments
                .iter()
                .any(|argument| argument == "pipeline_e2e")
        {
            return Err(Error::new(
                "OpenClaw runtime coverage is outside the reviewed non-installing set",
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cargo_owned(
    label: &'static str,
    executable: &TrustedExecutable,
    arguments: Vec<String>,
    current_dir: &Path,
    environment: &BTreeMap<String, String>,
    timeout: Duration,
    uid: u32,
    gid: u32,
) -> BuildStep {
    BuildStep::candidate(CommandSpec {
        label,
        executable: executable.clone(),
        arguments,
        current_dir: current_dir.to_path_buf(),
        environment: environment.clone(),
        timeout,
        run_as_uid: Some(uid),
        run_as_gid: Some(gid),
    })
}

fn prepare_builder_directory(path: &Path, uid: u32, gid: u32) -> Result<()> {
    std::os::unix::fs::chown(path, Some(uid), Some(gid))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn prepare_builder_traversal_directory(path: &Path, gid: u32) -> Result<()> {
    std::os::unix::fs::chown(path, Some(nix::unistd::geteuid().as_raw()), Some(gid))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o710))?;
    Ok(())
}

fn classify(paths: &[String]) -> BTreeSet<Impact> {
    let mut impacts = BTreeSet::new();
    for path in paths {
        if path == "Cargo.toml" || path.starts_with("crates/") {
            impacts.insert(Impact::Core);
        }
        if path.starts_with("services/astrid-edge-runtime/") {
            impacts.insert(Impact::Edge);
        }
        if path.starts_with("capsules/astralis/") {
            impacts.insert(Impact::Capsule);
        }
        if path.starts_with("scripts/") {
            impacts.insert(Impact::Python);
        }
        if path.starts_with("packaging/systemd/") {
            impacts.insert(Impact::Unit);
        }
        if path.starts_with("packaging/appliances/") {
            impacts.insert(Impact::Profile);
        }
    }
    impacts
}

fn selected_capsules(paths: &[String]) -> BTreeSet<String> {
    if paths
        .iter()
        .any(|path| path == "Cargo.toml" || path.starts_with("crates/"))
    {
        return REBUILDABLE_CAPSULES
            .iter()
            .map(|capsule| (*capsule).to_owned())
            .collect();
    }
    REBUILDABLE_CAPSULES
        .iter()
        .filter(|capsule| {
            let prefix = format!("capsules/astralis/{capsule}/");
            paths.iter().any(|path| path.starts_with(&prefix))
        })
        .map(|capsule| (*capsule).to_owned())
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct CapsuleCommandLabels {
    clippy: &'static str,
    test: &'static str,
    wasip2_build: &'static str,
    package: &'static str,
}

/// Fixed labels keep build evidence deterministic and prevent candidate path
/// text from entering command identities. Every essential capsule owns four
/// unique labels, so a core or multi-capsule candidate remains executable.
#[allow(clippy::too_many_lines)] // Deliberately centralized immutable command-label registry.
fn capsule_command_labels(capsule: &str) -> Result<CapsuleCommandLabels> {
    let labels = match capsule {
        "astrid-capsule-cli" => CapsuleCommandLabels {
            clippy: "capsule-cli-clippy",
            test: "capsule-cli-test",
            wasip2_build: "capsule-cli-wasip2-build",
            package: "capsule-cli-package",
        },
        "astrid-capsule-fs" => CapsuleCommandLabels {
            clippy: "capsule-fs-clippy",
            test: "capsule-fs-test",
            wasip2_build: "capsule-fs-wasip2-build",
            package: "capsule-fs-package",
        },
        "astrid-capsule-http" => CapsuleCommandLabels {
            clippy: "capsule-http-clippy",
            test: "capsule-http-test",
            wasip2_build: "capsule-http-wasip2-build",
            package: "capsule-http-package",
        },
        "astrid-capsule-shell" => CapsuleCommandLabels {
            clippy: "capsule-shell-clippy",
            test: "capsule-shell-test",
            wasip2_build: "capsule-shell-wasip2-build",
            package: "capsule-shell-package",
        },
        "astrid-capsule-skills" => CapsuleCommandLabels {
            clippy: "capsule-skills-clippy",
            test: "capsule-skills-test",
            wasip2_build: "capsule-skills-wasip2-build",
            package: "capsule-skills-package",
        },
        "astrid-capsule-agents" => CapsuleCommandLabels {
            clippy: "capsule-agents-clippy",
            test: "capsule-agents-test",
            wasip2_build: "capsule-agents-wasip2-build",
            package: "capsule-agents-package",
        },
        "astrid-capsule-memory" => CapsuleCommandLabels {
            clippy: "capsule-memory-clippy",
            test: "capsule-memory-test",
            wasip2_build: "capsule-memory-wasip2-build",
            package: "capsule-memory-package",
        },
        "astrid-capsule-edge-context" => CapsuleCommandLabels {
            clippy: "capsule-edge-context-clippy",
            test: "capsule-edge-context-test",
            wasip2_build: "capsule-edge-context-wasip2-build",
            package: "capsule-edge-context-package",
        },
        "astrid-capsule-edge-introspector" => CapsuleCommandLabels {
            clippy: "capsule-edge-introspector-clippy",
            test: "capsule-edge-introspector-test",
            wasip2_build: "capsule-edge-introspector-wasip2-build",
            package: "capsule-edge-introspector-package",
        },
        "astrid-capsule-edge-spectral" => CapsuleCommandLabels {
            clippy: "capsule-edge-spectral-clippy",
            test: "capsule-edge-spectral-test",
            wasip2_build: "capsule-edge-spectral-wasip2-build",
            package: "capsule-edge-spectral-package",
        },
        "astrid-capsule-context-engine" => CapsuleCommandLabels {
            clippy: "capsule-context-engine-clippy",
            test: "capsule-context-engine-test",
            wasip2_build: "capsule-context-engine-wasip2-build",
            package: "capsule-context-engine-package",
        },
        "astrid-capsule-hook-bridge" => CapsuleCommandLabels {
            clippy: "capsule-hook-bridge-clippy",
            test: "capsule-hook-bridge-test",
            wasip2_build: "capsule-hook-bridge-wasip2-build",
            package: "capsule-hook-bridge-package",
        },
        "astrid-capsule-identity" => CapsuleCommandLabels {
            clippy: "capsule-identity-clippy",
            test: "capsule-identity-test",
            wasip2_build: "capsule-identity-wasip2-build",
            package: "capsule-identity-package",
        },
        "astrid-capsule-openai-compat" => CapsuleCommandLabels {
            clippy: "capsule-openai-compat-clippy",
            test: "capsule-openai-compat-test",
            wasip2_build: "capsule-openai-compat-wasip2-build",
            package: "capsule-openai-compat-package",
        },
        "astrid-capsule-prompt-builder" => CapsuleCommandLabels {
            clippy: "capsule-prompt-builder-clippy",
            test: "capsule-prompt-builder-test",
            wasip2_build: "capsule-prompt-builder-wasip2-build",
            package: "capsule-prompt-builder-package",
        },
        "astrid-capsule-react" => CapsuleCommandLabels {
            clippy: "capsule-react-clippy",
            test: "capsule-react-test",
            wasip2_build: "capsule-react-wasip2-build",
            package: "capsule-react-package",
        },
        "astrid-capsule-registry" => CapsuleCommandLabels {
            clippy: "capsule-registry-clippy",
            test: "capsule-registry-test",
            wasip2_build: "capsule-registry-wasip2-build",
            package: "capsule-registry-package",
        },
        "astrid-capsule-router" => CapsuleCommandLabels {
            clippy: "capsule-router-clippy",
            test: "capsule-router-test",
            wasip2_build: "capsule-router-wasip2-build",
            package: "capsule-router-package",
        },
        "astrid-capsule-session" => CapsuleCommandLabels {
            clippy: "capsule-session-clippy",
            test: "capsule-session-test",
            wasip2_build: "capsule-session-wasip2-build",
            package: "capsule-session-package",
        },
        "astrid-capsule-system" => CapsuleCommandLabels {
            clippy: "capsule-system-clippy",
            test: "capsule-system-test",
            wasip2_build: "capsule-system-wasip2-build",
            package: "capsule-system-package",
        },
        _ => {
            return Err(Error::new(
                "capsule command labels are not immutable-allowlisted",
            ));
        },
    };
    Ok(labels)
}

fn prepare_capsule_targets(
    source: &Path,
    capsules: &BTreeSet<String>,
    uid: u32,
    gid: u32,
) -> Result<Vec<PathBuf>> {
    let mut targets = Vec::new();
    for capsule in capsules {
        let target = source
            .join("capsules/astralis")
            .join(capsule)
            .join("target");
        if target.exists() || target.is_symlink() {
            return Err(Error::new("capsule-local build target already exists"));
        }
        fs::create_dir(&target)?;
        prepare_builder_directory(&target, uid, gid)?;
        targets.push(target);
    }
    Ok(targets)
}

fn cleanup_capsule_targets(targets: &[PathBuf]) -> Result<()> {
    for target in targets {
        let metadata = fs::symlink_metadata(target)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(Error::new("candidate replaced a capsule-local target root"));
        }
        fs::remove_dir_all(target)?;
    }
    Ok(())
}

fn cleanup_unstarted_scratch(config: &Config, scratch: &Path) -> Result<()> {
    fn verify(path: &Path, allowed_uids: &[u32]) -> Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink()
            || (!metadata.is_dir() && (!metadata.is_file() || metadata.nlink() != 1))
            || !allowed_uids.contains(&metadata.uid())
        {
            return Err(Error::new(
                "deferred scratch cleanup found linked, special, or foreign-owned content",
            ));
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(path)? {
                verify(&entry?.path(), allowed_uids)?;
            }
        }
        Ok(())
    }

    ensure_within(&config.roots.candidate_work, scratch, true)?;
    if scratch.parent() != Some(config.roots.candidate_work.as_path())
        || !scratch
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("build-") && crate::config::valid_identifier(name))
    {
        return Err(Error::new(
            "deferred scratch cleanup target is not an exact build root",
        ));
    }
    verify(
        scratch,
        &[
            0,
            config.identities.builder_uid,
            nix::unistd::geteuid().as_raw(),
        ],
    )?;
    fs::remove_dir_all(scratch)?;
    fs::File::open(&config.roots.candidate_work)?.sync_all()?;
    Ok(())
}

fn require_candidate_gate_success(receipt: &CommandReceipt) -> Result<()> {
    if !receipt.timed_out && receipt.exit_code == Some(0) {
        return Ok(());
    }
    if receipt.timed_out || receipt.exit_code.is_some() {
        return Err(Error::candidate_rejected(format!(
            "fixed candidate gate rejected: {}",
            receipt.label
        )));
    }
    Err(Error::new(format!(
        "fixed native gate ended without an exit status: {}",
        receipt.label
    )))
}

fn enforce_trusted_host_quiescence(
    outcome: Result<CommandReceipt>,
    cleanup: Result<bool>,
) -> Result<CommandReceipt> {
    match cleanup {
        Err(error) => Err(Error::new(format!(
            "trusted-host invariant exit could not prove candidate transient cleanup: {error}"
        ))),
        Ok(true) => Err(Error::new(
            "trusted-host invariant left candidate transient activity behind",
        )),
        Ok(false) => outcome,
    }
}

fn candidate_rejected_after_cleanup(
    config: &Config,
    scratch: &Path,
    phase: &str,
    rejection: &Error,
) -> Error {
    if let Err(cleanup) = cleanup_completed_scratch(config, scratch) {
        return Error::new(format!(
            "candidate rejection cleanup failed after {phase}: {}",
            cleanup.message()
        ));
    }
    if rejection.kind() == ErrorKind::CandidateRejected {
        return rejection.clone();
    }
    Error::candidate_rejected(format!("{phase}: {}", rejection.message()))
}

fn cleanup_completed_scratch(config: &Config, scratch: &Path) -> Result<()> {
    ensure_within(&config.roots.candidate_work, scratch, true)?;
    if scratch.parent() != Some(config.roots.candidate_work.as_path())
        || !scratch
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("build-") && crate::config::valid_identifier(name))
    {
        return Err(Error::new(
            "completed scratch cleanup target is not an exact build root",
        ));
    }
    let metadata = fs::symlink_metadata(scratch)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != nix::unistd::geteuid().as_raw()
    {
        return Err(Error::new(
            "completed scratch cleanup root is linked, special, or foreign-owned",
        ));
    }
    // The exact top-level scratch root is owned by the helper and mode 0710, so the dropped
    // builder cannot exchange it. `remove_dir_all` on Unix does not follow interior symlinks;
    // accepting them here avoids a candidate-created dangling link becoming a permanent disk DoS.
    fs::remove_dir_all(scratch)?;
    fs::File::open(&config.roots.candidate_work)?.sync_all()?;
    Ok(())
}

fn write_or_verify_manifest(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() || path.is_symlink() {
        if read_regular(path, 256 * 1024)? != bytes {
            return Err(Error::new(
                "existing build output manifest differs from recovered sealed build",
            ));
        }
        return Ok(());
    }
    atomic_write(path, bytes, 0o400, false)
}

pub(crate) fn verify_sealed_build(
    config: &Config,
    root: &Path,
    expected: Option<(&Candidate, &str)>,
) -> Result<BuildV1> {
    let metadata = fs::symlink_metadata(root)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != nix::unistd::geteuid().as_raw()
        || metadata.mode() & 0o222 != 0
    {
        return Err(Error::new(
            "sealed build root is linked, writable, or foreign-owned",
        ));
    }
    let mut names = fs::read_dir(root)?
        .map(|entry| entry.map(|value| value.file_name().to_string_lossy().into_owned()))
        .collect::<std::io::Result<Vec<_>>>()?;
    names.sort();
    if names != SEALED_BUILD_ENTRIES {
        return Err(Error::new(
            "sealed build contains unexpected or missing top-level content",
        ));
    }
    verify_immutable_tree(root, nix::unistd::geteuid().as_raw())?;
    let build: BuildV1 = read_json(&root.join("build.json"), 256 * 1024)?;
    build.validate(config)?;
    let basename = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if basename != "sealed" && basename != build.build_id {
        return Err(Error::new(
            "sealed build directory does not bind Build v1 ID",
        ));
    }
    if bundle_digest(&root.join("bundle"))? != build.bundle_sha256 {
        return Err(Error::new("sealed bundle digest differs from Build v1"));
    }
    let inventory_bytes = read_regular(&root.join("bundle-inventory.json"), 32 * 1024 * 1024)?;
    let inventory_value: serde_json::Value = serde_json::from_slice(&inventory_bytes)?;
    if canonical_json(&inventory_value)? != inventory_bytes
        || sha256(&inventory_bytes) != build.bundle_sha256
    {
        return Err(Error::new(
            "sealed bundle inventory is non-canonical or differs from Build v1",
        ));
    }
    if sha256(&read_regular(
        &root.join("evidence.json"),
        16 * 1024 * 1024,
    )?) != build.tests_sha256
    {
        return Err(Error::new("sealed test evidence differs from Build v1"));
    }
    let patch_bytes = read_regular(&root.join("candidate-patch.json"), 32 * 1024 * 1024)?;
    let bound: BaseBoundPatch = serde_json::from_slice(&patch_bytes)?;
    if canonical_json(&bound)? != patch_bytes
        || bound.schema != "astrid.edge_rescue_helper.base_bound_patch.v1"
        || bound.candidate.schema != crate::manifest::CANDIDATE_SCHEMA
        || bound.patch.schema != PATCH_SCHEMA
        || bound.candidate.digest()? != build.candidate_sha256
        || bound.candidate.candidate_id != build.candidate_id
        || bound.candidate.base_generation != build.base_generation
        || bound.patch.candidate_id != build.candidate_id
        || bound.patch.base_generation != build.base_generation
        || sha256(&canonical_json(&bound.patch)?) != bound.candidate.patch_sha256
        || bound
            .patch
            .files
            .iter()
            .map(|file| file.path.as_str())
            .ne(bound.candidate.changed_paths.iter().map(String::as_str))
    {
        return Err(Error::new("sealed candidate patch binding failed"));
    }
    bound.candidate.validate(config)?;
    if let Some((candidate, digest)) = expected
        && (canonical_json(candidate)? != canonical_json(&bound.candidate)?
            || digest != build.candidate_sha256)
    {
        return Err(Error::new(
            "recovered sealed build differs from the exact requested candidate",
        ));
    }
    Ok(build)
}

fn verify_immutable_tree(path: &Path, owner: u32) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || metadata.uid() != owner
        || metadata.mode() & 0o222 != 0
        || (!metadata.is_dir() && (!metadata.is_file() || metadata.nlink() != 1))
    {
        return Err(Error::new(
            "sealed build contains linked, writable, special, or foreign-owned content",
        ));
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            verify_immutable_tree(&entry?.path(), owner)?;
        }
    }
    Ok(())
}

fn gc_terminal_builds(config: &Config, preserve: Option<&str>) -> Result<()> {
    recover_gc_tombstones(&config.roots.build_store)?;
    let mut terminal = Vec::new();
    for entry in fs::read_dir(&config.roots.build_store)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| Error::new("build store contains a non-UTF-8 entry"))?;
        if !name.starts_with("build-") || !crate::config::valid_identifier(name) {
            return Err(Error::new("build store contains an unexpected entry"));
        }
        let build = verify_sealed_build(config, &path, None)?;
        let release = config.roots.releases.join(&build.generation_id);
        if !release.exists() {
            continue;
        }
        let generation: GenerationManifest =
            read_json(&release.join(".astrid-edge-generation.json"), 256 * 1024)?;
        if generation != GenerationManifest::from(&build) {
            return Err(Error::new(
                "terminal build and installed generation binding differ",
            ));
        }
        terminal.push((build.created_at, build.build_id, path));
    }
    terminal.sort_by(|left, right| (left.0, &left.1).cmp(&(right.0, &right.1)));
    let selected = terminal_gc_selection(
        terminal
            .iter()
            .map(|(created_at, build_id, _)| (*created_at, build_id.clone()))
            .collect(),
        MAX_RETAINED_TERMINAL_BUILDS,
        preserve,
    );
    for (_, build_id, path) in terminal {
        if !selected.contains(&build_id) {
            continue;
        }
        let tombstone = config.roots.build_store.join(format!(".gc-{build_id}"));
        ensure_within(&config.roots.build_store, &tombstone, false)?;
        if tombstone.exists() || tombstone.is_symlink() {
            return Err(Error::new("terminal build GC tombstone already exists"));
        }
        fs::rename(&path, &tombstone)?;
        fs::File::open(&config.roots.build_store)?.sync_all()?;
        remove_gc_tombstone(&config.roots.build_store, &tombstone)?;
    }
    fs::File::open(&config.roots.build_store)?.sync_all()?;
    Ok(())
}

fn recover_gc_tombstones(build_store: &Path) -> Result<()> {
    for entry in fs::read_dir(build_store)? {
        let path = entry?.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(".gc-build-"))
        {
            remove_gc_tombstone(build_store, &path)?;
        }
    }
    Ok(())
}

fn remove_gc_tombstone(build_store: &Path, path: &Path) -> Result<()> {
    ensure_within(build_store, path, true)?;
    let valid = path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix(".gc-"))
        .is_some_and(|name| name.starts_with("build-") && crate::config::valid_identifier(name));
    if path.parent() != Some(build_store) || !valid {
        return Err(Error::new("terminal build GC tombstone name is invalid"));
    }
    make_tree_removable(path, nix::unistd::geteuid().as_raw())?;
    fs::remove_dir_all(path)?;
    fs::File::open(build_store)?.sync_all()?;
    Ok(())
}

fn terminal_gc_selection(
    mut terminal: Vec<(u64, String)>,
    retain: usize,
    preserve: Option<&str>,
) -> BTreeSet<String> {
    terminal.sort();
    let removable = terminal.len().saturating_sub(retain);
    terminal
        .into_iter()
        .take(removable)
        .filter_map(|(_, build_id)| (preserve != Some(build_id.as_str())).then_some(build_id))
        .collect()
}

fn same_filesystem(left: &Path, right: &Path) -> Result<bool> {
    Ok(fs::metadata(left)?.dev() == fs::metadata(right)?.dev())
}

fn make_tree_removable(path: &Path, owner: u32) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || metadata.uid() != owner
        || (!metadata.is_dir() && (!metadata.is_file() || metadata.nlink() != 1))
    {
        return Err(Error::new(
            "terminal build GC found linked, special, or foreign-owned content",
        ));
    }
    if metadata.is_dir() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        for entry in fs::read_dir(path)? {
            make_tree_removable(&entry?.path(), owner)?;
        }
    } else {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn signed_unit_files(source: &Path) -> Result<Vec<PathBuf>> {
    fn visit(directory: &Path, result: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(Error::new("systemd source contains symlink"));
            }
            if metadata.is_dir() {
                visit(&path, result)?;
            } else if metadata.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension == "service" || extension == "timer")
            {
                result.push(path);
            }
        }
        Ok(())
    }

    let root = source.join("packaging/systemd");
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    visit(&root, &mut result)?;
    result.sort();
    Ok(result)
}

fn copy_output(source: &Path, destination: &Path, mode: u32) -> Result<()> {
    let bytes = read_regular(source, MAX_BUILD_FILE)?;
    atomic_write(destination, &bytes, mode, false)
}

fn copy_release_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let input = entry.path();
        let output = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&input)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::new("release input contains symlink"));
        }
        if metadata.is_dir() {
            copy_release_tree(&input, &output)?;
        } else if metadata.is_file() && metadata.nlink() == 1 && metadata.len() <= 16 * 1024 * 1024
        {
            let mode = if metadata.mode() & 0o111 != 0 {
                0o555
            } else {
                0o444
            };
            atomic_write(&output, &read_regular(&input, metadata.len())?, mode, false)?;
        } else {
            return Err(Error::new(
                "release input contains linked, special, or oversized file",
            ));
        }
    }
    Ok(())
}

fn inventory(root: &Path) -> Result<BundleInventory> {
    fn visit(root: &Path, directory: &Path, output: &mut Vec<BundleFile>) -> Result<()> {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(Error::new("bundle contains symlink"));
            }
            if metadata.is_dir() {
                visit(root, &path, output)?;
            } else if metadata.is_file() && metadata.nlink() == 1 {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|_| Error::new("bundle path escape"))?
                    .to_string_lossy()
                    .replace('\\', "/");
                if relative == ".astrid-edge-generation.json" {
                    continue;
                }
                validate_relative(&relative)?;
                output.push(BundleFile {
                    path: relative,
                    size: metadata.len(),
                    mode: format!("{:04o}", metadata.mode() & 0o777),
                    sha256: sha256(&read_regular(&path, MAX_BUILD_FILE)?),
                });
            } else {
                return Err(Error::new("bundle contains linked or special file"));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    visit(root, root, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(BundleInventory {
        schema: "astrid.edge_rescue_helper.bundle_inventory.v1",
        files,
    })
}

pub(crate) fn bundle_digest(root: &Path) -> Result<String> {
    Ok(sha256(&canonical_json(&inventory(root)?)?))
}

fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::{
        BuildStep, ExecutionBoundary, Impact, capsule_command_labels, classify,
        enforce_trusted_host_quiescence, recover_gc_tombstones, require_candidate_gate_success,
        same_filesystem, selected_capsules, terminal_gc_selection, validate_offline_command_plan,
        write_or_verify_manifest,
    };
    use crate::config::TrustedExecutable;
    use crate::invariant::REBUILDABLE_CAPSULES;
    use crate::native::{CommandReceipt, CommandSpec};
    use crate::{Error, ErrorKind};
    use std::collections::BTreeMap;
    use std::fs;
    #[cfg(target_os = "linux")]
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::Duration;

    fn cargo_spec(label: &'static str, arguments: &[&str]) -> BuildStep {
        BuildStep::candidate(CommandSpec {
            label,
            executable: TrustedExecutable {
                path: PathBuf::from("/opt/astrid-edge/toolchain/bin/cargo"),
                sha256: "a".repeat(64),
            },
            arguments: arguments
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect(),
            current_dir: PathBuf::from("/var/lib/astrid-edge-builder/work/build-fixture/source"),
            environment: BTreeMap::from([
                ("CARGO_NET_OFFLINE".to_owned(), "true".to_owned()),
                (
                    "CARGO_HOME".to_owned(),
                    "/var/lib/astrid-edge-builder/work/build-fixture/empty-cargo-home".to_owned(),
                ),
            ]),
            timeout: Duration::from_secs(60),
            run_as_uid: Some(981),
            run_as_gid: Some(981),
        })
    }

    #[test]
    fn clean_offline_plan_excludes_network_acquiring_openclaw_tests() {
        let accepted = vec![
            cargo_spec(
                "core-tests",
                &[
                    "test",
                    "--workspace",
                    "--exclude",
                    "astrid-openclaw",
                    "--offline",
                    "--locked",
                ],
            ),
            cargo_spec(
                "openclaw-compile-tests-no-run",
                &[
                    "test",
                    "--package",
                    "astrid-openclaw",
                    "--tests",
                    "--no-run",
                    "--offline",
                    "--locked",
                ],
            ),
            cargo_spec(
                "openclaw-deterministic-tests",
                &[
                    "test",
                    "--package",
                    "astrid-openclaw",
                    "--test",
                    "e2e_plugin",
                    "--test",
                    "manifest_roundtrip",
                    "--offline",
                    "--locked",
                ],
            ),
        ];
        assert!(validate_offline_command_plan(&accepted).is_ok());

        let mut broad = accepted.clone();
        broad[0].command.arguments = vec![
            "test".into(),
            "--workspace".into(),
            "--offline".into(),
            "--locked".into(),
        ];
        assert!(validate_offline_command_plan(&broad).is_err());

        let mut npm = accepted.clone();
        npm.push(BuildStep::candidate(CommandSpec {
            label: "forbidden-npm",
            executable: TrustedExecutable {
                path: PathBuf::from("/usr/bin/npm"),
                sha256: "b".repeat(64),
            },
            arguments: vec!["install".into()],
            current_dir: PathBuf::from("/var/lib/astrid-edge-builder/work/build-fixture"),
            environment: BTreeMap::new(),
            timeout: Duration::from_secs(60),
            run_as_uid: Some(981),
            run_as_gid: Some(981),
        }));
        assert!(validate_offline_command_plan(&npm).is_err());
        assert!(
            accepted
                .iter()
                .all(|step| step.boundary == ExecutionBoundary::CandidateTransient)
        );
        let trusted = BuildStep::trusted_host(accepted[0].command.clone());
        assert_eq!(trusted.boundary, ExecutionBoundary::TrustedHost);
    }

    #[test]
    fn fixed_gate_failure_is_a_terminal_candidate_rejection_not_rescue() {
        let rejected = require_candidate_gate_success(&CommandReceipt {
            label: "core-tests".to_owned(),
            execution_boundary: crate::native::CommandExecutionBoundary::CandidateTransient,
            executable_sha256: "a".repeat(64),
            argv_sha256: "b".repeat(64),
            exit_code: Some(1),
            timed_out: false,
            duration_ms: 10,
        })
        .unwrap_err();
        assert_eq!(rejected.kind(), ErrorKind::CandidateRejected);

        let ambiguous = require_candidate_gate_success(&CommandReceipt {
            label: "core-tests".to_owned(),
            execution_boundary: crate::native::CommandExecutionBoundary::CandidateTransient,
            executable_sha256: "a".repeat(64),
            argv_sha256: "b".repeat(64),
            exit_code: None,
            timed_out: false,
            duration_ms: 10,
        })
        .unwrap_err();
        assert_eq!(ambiguous.kind(), ErrorKind::Terminal);
    }

    #[test]
    fn monitor_abort_mid_shadow_requires_a_proven_zero_transient_boundary() {
        let abort = Err(Error::deferred("synthetic thermal abort during shadow"));
        let preserved = enforce_trusted_host_quiescence(abort, Ok(false)).unwrap_err();
        assert_eq!(preserved.kind(), ErrorKind::DeferredInfrastructure);

        let abort = Err(Error::deferred("synthetic thermal abort during shadow"));
        let leaked = enforce_trusted_host_quiescence(abort, Ok(true)).unwrap_err();
        assert_eq!(leaked.kind(), ErrorKind::Terminal);
        assert!(
            leaked
                .message()
                .contains("left candidate transient activity")
        );

        let abort = Err(Error::deferred("synthetic thermal abort during shadow"));
        let unproven =
            enforce_trusted_host_quiescence(abort, Err(Error::new("system manager unavailable")))
                .unwrap_err();
        assert_eq!(unproven.kind(), ErrorKind::Terminal);
        assert!(unproven.message().contains("could not prove"));
    }

    #[test]
    fn impacted_components_are_selected_without_model_commands() {
        let impacts = classify(&[
            "crates/astrid-kernel/src/lib.rs".to_owned(),
            "scripts/report_edge_activity.py".to_owned(),
            "packaging/systemd/astrid.service".to_owned(),
        ]);
        assert!(impacts.contains(&Impact::Core));
        assert!(impacts.contains(&Impact::Python));
        assert!(impacts.contains(&Impact::Unit));
        assert!(!impacts.contains(&Impact::Edge));
    }

    #[test]
    fn multi_capsule_plan_has_fixed_unique_labels_and_strict_offline_clippy() {
        let selected = selected_capsules(&[
            "capsules/astralis/astrid-capsule-edge-context/src/lib.rs".to_owned(),
            "capsules/astralis/astrid-capsule-edge-spectral/src/lib.rs".to_owned(),
        ]);
        assert_eq!(selected.len(), 2);
        let mut labels = std::collections::BTreeSet::new();
        let mut clippy_specs = Vec::new();
        for capsule in &selected {
            let fixed = capsule_command_labels(capsule).unwrap();
            assert!(labels.insert(fixed.clippy));
            assert!(labels.insert(fixed.test));
            assert!(labels.insert(fixed.wasip2_build));
            assert!(labels.insert(fixed.package));
            clippy_specs.push(cargo_spec(
                fixed.clippy,
                &[
                    "clippy",
                    "--manifest-path",
                    "capsule/Cargo.toml",
                    "--all-targets",
                    "--all-features",
                    "--offline",
                    "--locked",
                    "--",
                    "-D",
                    "warnings",
                ],
            ));
        }
        assert_eq!(labels.len(), 8);
        assert!(validate_offline_command_plan(&clippy_specs).is_ok());

        let core_selected = selected_capsules(&["crates/astrid-kernel/src/lib.rs".to_owned()]);
        assert_eq!(core_selected.len(), REBUILDABLE_CAPSULES.len());
        assert!(
            core_selected
                .iter()
                .all(|capsule| REBUILDABLE_CAPSULES.contains(&capsule.as_str()))
        );
        assert!(core_selected.contains("astrid-capsule-openai-compat"));

        let all_labels = REBUILDABLE_CAPSULES
            .iter()
            .flat_map(|capsule| {
                let labels = capsule_command_labels(capsule).unwrap();
                [
                    labels.clippy,
                    labels.test,
                    labels.wasip2_build,
                    labels.package,
                ]
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(all_labels.len(), REBUILDABLE_CAPSULES.len() * 4);
        assert!(capsule_command_labels("model-supplied-capsule").is_err());
    }

    #[test]
    fn terminal_gc_is_bounded_deterministic_and_preserves_requested_build() {
        let terminal = (0..7)
            .map(|index| (index, format!("build-{index}")))
            .collect();
        let selected = terminal_gc_selection(terminal, 4, Some("build-1"));
        assert_eq!(
            selected,
            ["build-0".to_owned(), "build-2".to_owned()]
                .into_iter()
                .collect()
        );
    }

    #[test]
    fn power_loss_manifest_replay_is_idempotent_but_never_replaces() {
        let temporary = tempfile::tempdir().unwrap();
        let manifest = temporary.path().join("build.json");
        write_or_verify_manifest(&manifest, br#"{"schema":"test"}"#).unwrap();
        write_or_verify_manifest(&manifest, br#"{"schema":"test"}"#).unwrap();
        assert!(write_or_verify_manifest(&manifest, br#"{"schema":"changed"}"#).is_err());
        assert_eq!(fs::read(manifest).unwrap(), br#"{"schema":"test"}"#);
    }

    #[test]
    fn power_loss_gc_tombstone_is_finished_without_touching_other_content() {
        let temporary = tempfile::tempdir().unwrap();
        let store = temporary.path().canonicalize().unwrap().join("builds");
        let tombstone = store.join(".gc-build-abc");
        let preserved = store.join("build-new");
        fs::create_dir(&store).unwrap();
        fs::create_dir(&tombstone).unwrap();
        fs::write(tombstone.join("evidence"), b"old").unwrap();
        fs::create_dir(&preserved).unwrap();
        fs::set_permissions(
            tombstone.join("evidence"),
            fs::Permissions::from_mode(0o400),
        )
        .unwrap();
        fs::set_permissions(&tombstone, fs::Permissions::from_mode(0o500)).unwrap();
        recover_gc_tombstones(&store).unwrap();
        assert!(!tombstone.exists());
        assert!(preserved.exists());
    }

    #[test]
    fn work_and_build_store_require_one_filesystem() {
        let temporary = tempfile::tempdir().unwrap();
        let work = temporary.path().join("work");
        let builds = temporary.path().join("builds");
        fs::create_dir(&work).unwrap();
        fs::create_dir(&builds).unwrap();
        assert!(same_filesystem(&work, &builds).unwrap());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cross_filesystem_builder_roots_are_rejected_when_available() {
        let temporary = tempfile::tempdir().unwrap();
        let shared_memory = std::path::Path::new("/dev/shm");
        if shared_memory.is_dir()
            && fs::metadata(temporary.path()).unwrap().dev()
                != fs::metadata(shared_memory).unwrap().dev()
        {
            assert!(!same_filesystem(temporary.path(), shared_memory).unwrap());
        }
    }
}
