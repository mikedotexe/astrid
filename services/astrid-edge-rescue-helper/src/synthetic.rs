//! Operator-only full lifecycle replay in a disjoint temporary appliance root.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::build::Builder;
use crate::config::Config;
use crate::fs_guard::{atomic_write, canonical_json, ensure_within, sha256, sha256_file};
use crate::generation::{self, validate_release_manifest};
use crate::invariant::{MUTABLE_UNIT_FRAGMENTS, normalized_system_unit};
use crate::manifest::SourceSnapshot;
use crate::native::{CommandReceipt, CommandSpec, NativeRunner};
use crate::transition::{
    MaintenanceLease, activate_inner, active_target, read_generation_binding,
    reconcile_active_generation_inner, rollback_inner,
};
use crate::unit_transaction::{PolicyDropin, UnitPolicy};
use crate::{Error, Result};

const MAX_RETAINED_RUNS: usize = 8;

#[derive(Debug, Serialize)]
#[allow(clippy::struct_excessive_bools)] // Each explicit false is an authority non-claim.
pub struct SyntheticLifecycleEvidence {
    pub schema: &'static str,
    pub provenance: &'static str,
    pub appliance_id: String,
    pub production_generation_before: String,
    pub production_binding_sha256_before: String,
    pub production_binding_sha256_after: String,
    pub production_active_link_before: String,
    pub production_active_link_after: String,
    pub synthetic_candidate_id: String,
    pub synthetic_build_id: String,
    pub synthetic_generation_id: String,
    pub model_service_receipts: Vec<CommandReceipt>,
    pub candidate_source_changed: bool,
    pub offline_build_and_package_gates_passed: bool,
    pub isolated_activation_passed: bool,
    pub isolated_rollback_passed: bool,
    pub link_first_crash_recovered: bool,
    pub binding_first_crash_recovered: bool,
    pub production_intent_created: bool,
    pub production_generation_switched: bool,
    pub continuity_or_reservoir_admission: bool,
    pub sandbox_root: String,
    pub evidence_sha256: String,
}

pub fn run(config: &Config) -> Result<SyntheticLifecycleEvidence> {
    if nix::unistd::geteuid().as_raw() != 0 {
        return Err(Error::new("synthetic lifecycle harness requires root"));
    }
    let sandbox_root = allocate_sandbox_root(config)?;
    run_inner(config, &sandbox_root)
}

fn allocate_sandbox_root(config: &Config) -> Result<PathBuf> {
    let candidate = fs::symlink_metadata(&config.roots.candidate_work)?;
    let builds = fs::symlink_metadata(&config.roots.build_store)?;
    if !candidate.is_dir()
        || candidate.file_type().is_symlink()
        || !builds.is_dir()
        || builds.file_type().is_symlink()
        || candidate.dev() != builds.dev()
    {
        return Err(Error::new(
            "synthetic harness requires candidate and build stores on one regular filesystem",
        ));
    }
    let harness = config.roots.candidate_work.join("synthetic-harness");
    let parent = harness.join("runs");
    ensure_within(&config.roots.candidate_work, &parent, false)?;
    for path in [&harness, &parent] {
        if !path.exists() {
            fs::create_dir(path)?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        }
        require_root_harness_directory(path, candidate.dev())?;
    }
    let retained = retained_run_count(&parent, 0, candidate.dev())?;
    if retained >= MAX_RETAINED_RUNS {
        return Err(Error::new(
            "synthetic lifecycle retention is full; operator review is required before removal",
        ));
    }
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error::new("system clock precedes Unix epoch"))?;
    let name = format!(
        "synthetic-{}-{}-{}",
        elapsed.as_secs(),
        elapsed.subsec_nanos(),
        std::process::id()
    );
    if !crate::config::valid_identifier(&name) {
        return Err(Error::new("generated synthetic run identifier is invalid"));
    }
    let sandbox = parent.join(name);
    ensure_within(&parent, &sandbox, false)?;
    if sandbox.exists() || sandbox.is_symlink() {
        return Err(Error::new(
            "generated synthetic lifecycle root already exists",
        ));
    }
    Ok(sandbox)
}

fn require_root_harness_directory(path: &Path, device: u64) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != 0
        || metadata.dev() != device
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new(
            "synthetic harness directory identity or filesystem is invalid",
        ));
    }
    Ok(())
}

fn retained_run_count(parent: &Path, owner: u32, device: u64) -> Result<usize> {
    let mut count = 0_usize;
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        let valid_name = entry.file_name().to_str().is_some_and(|name| {
            name.starts_with("synthetic-") && crate::config::valid_identifier(name)
        });
        if !valid_name
            || !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != owner
            || metadata.dev() != device
            || metadata.mode() & 0o077 != 0
        {
            return Err(Error::new(
                "synthetic harness retention contains an untrusted entry",
            ));
        }
        count = count
            .checked_add(1)
            .ok_or_else(|| Error::new("synthetic harness retention count overflow"))?;
    }
    Ok(count)
}

#[allow(clippy::too_many_lines)] // One disjoint transaction proves the complete A/B harness.
fn run_inner(config: &Config, sandbox_root: &Path) -> Result<SyntheticLifecycleEvidence> {
    let production_generation = read_generation_binding(config, true)?;
    let production_binding_before = sha256_file(&config.roots.generation_binding, 4_096)?;
    let production_link_before = fs::read_link(&config.roots.active_link)?;
    fs::create_dir(sandbox_root)?;
    fs::set_permissions(sandbox_root, fs::Permissions::from_mode(0o700))?;
    let sandbox = sandbox_config(config, sandbox_root);
    prepare_sandbox(&sandbox)?;
    let production_generation_root = active_target(config)?;
    let base = sandbox.roots.releases.join(&production_generation);
    copy_tree(
        &production_generation_root,
        &production_generation_root,
        &base,
    )?;
    fs::set_permissions(&base, fs::Permissions::from_mode(0o555))?;
    std::os::unix::fs::symlink(
        Path::new("releases").join(&production_generation),
        &sandbox.roots.active_link,
    )?;
    atomic_write(
        &sandbox.roots.generation_binding,
        format!("{production_generation}\n").as_bytes(),
        0o600,
        false,
    )?;
    let base_identity = validate_release_manifest(&sandbox, &base)?;
    let source =
        SourceSnapshot::load_for_generation(&sandbox, &base, base_identity.operator_initial)?;
    let (candidate, candidate_manifest) =
        source.create_synthetic_noop_candidate(&sandbox, &production_generation, unix_seconds())?;
    let build_manifest = sandbox_root.join("synthetic-build.json");
    let (build, model_service_receipts) =
        synthetic_build_with_model_handoff(config, &sandbox, &candidate_manifest, &build_manifest)?;
    let mut updater_runner = crate::native::SystemRunner;
    let candidate_generation = generation::install(&sandbox, &mut updater_runner, &build_manifest)?;
    let mut runner = install_fake_unit_boundary(&sandbox, &base)?;
    activate_inner(&sandbox, &mut runner, &candidate_generation, &base, false)?;
    let isolated_activation_passed = read_generation_binding(&sandbox, false)?
        == build.generation_id
        && active_target(&sandbox)? == candidate_generation.canonicalize()?;
    rollback_inner(&sandbox, &mut runner, &base, false)?;
    let isolated_rollback_passed = read_generation_binding(&sandbox, false)?
        == production_generation
        && active_target(&sandbox)? == base.canonicalize()?;

    replace_active_link(&sandbox, &candidate_generation)?;
    let _ = reconcile_active_generation_inner(&sandbox, &mut runner, false)?;
    let link_first_crash_recovered = read_generation_binding(&sandbox, false)?
        == production_generation
        && active_target(&sandbox)? == base.canonicalize()?;

    fs::write(
        &sandbox.roots.generation_binding,
        format!("{}\n", build.generation_id),
    )?;
    fs::set_permissions(
        &sandbox.roots.generation_binding,
        fs::Permissions::from_mode(0o600),
    )?;
    let _ = reconcile_active_generation_inner(&sandbox, &mut runner, false)?;
    let binding_first_crash_recovered = read_generation_binding(&sandbox, false)?
        == production_generation
        && active_target(&sandbox)? == base.canonicalize()?;

    let production_binding_after = sha256_file(&config.roots.generation_binding, 4_096)?;
    let production_link_after = fs::read_link(&config.roots.active_link)?;
    if production_binding_before != production_binding_after
        || production_link_before != production_link_after
    {
        return Err(Error::new(
            "synthetic lifecycle touched the production generation selection",
        ));
    }
    let mut evidence = SyntheticLifecycleEvidence {
        schema: "astrid.edge_rescue_helper.synthetic_lifecycle.v1",
        provenance: "operator_isolated_synthetic_machine_evidence_not_astrid_authorship",
        appliance_id: config.appliance_id.clone(),
        production_generation_before: production_generation,
        production_binding_sha256_before: production_binding_before,
        production_binding_sha256_after: production_binding_after,
        production_active_link_before: production_link_before.display().to_string(),
        production_active_link_after: production_link_after.display().to_string(),
        synthetic_candidate_id: candidate.candidate_id,
        synthetic_build_id: build.build_id,
        synthetic_generation_id: build.generation_id,
        model_service_receipts,
        candidate_source_changed: false,
        offline_build_and_package_gates_passed: true,
        isolated_activation_passed,
        isolated_rollback_passed,
        link_first_crash_recovered,
        binding_first_crash_recovered,
        production_intent_created: false,
        production_generation_switched: false,
        continuity_or_reservoir_admission: false,
        sandbox_root: sandbox_root.display().to_string(),
        evidence_sha256: String::new(),
    };
    validate_evidence(&evidence)?;
    evidence.evidence_sha256 = sha256(&canonical_json(&evidence)?);
    atomic_write(
        &sandbox_root.join("synthetic-lifecycle-receipt.json"),
        &canonical_json(&evidence)?,
        0o600,
        false,
    )?;
    Ok(evidence)
}

fn synthetic_build_with_model_handoff(
    authority_config: &Config,
    build_config: &Config,
    candidate_manifest: &Path,
    build_manifest: &Path,
) -> Result<(crate::manifest::BuildV1, Vec<CommandReceipt>)> {
    let lease = MaintenanceLease::acquire_for(
        authority_config,
        "operator_synthetic_lifecycle",
        authority_config.policy.pipeline_timeout_seconds,
    )?;
    let _drain = lease.wait_for_exact_drain(authority_config)?;
    let mut runner = crate::native::SystemRunner;
    let mut receipts = Vec::new();
    let stop = crate::model_service::stop(authority_config, &mut runner);
    let stop_receipt = match stop {
        Ok(receipt) => receipt,
        Err(stop_error) => {
            let _ = crate::model_service::restore(authority_config, &mut runner);
            return Err(stop_error);
        },
    };
    receipts.push(stop_receipt);
    let build = Builder::new(build_config, &mut runner)
        .build_synthetic_under_maintenance(candidate_manifest, build_manifest);
    let restoration = crate::model_service::restore(authority_config, &mut runner);
    if let Ok(restored) = &restoration {
        receipts.extend(restored.iter().cloned());
    }
    match (build, restoration) {
        (Ok(build), Ok(_)) => Ok((build, receipts)),
        (Err(build_error), Ok(_)) => Err(build_error),
        (Ok(_), Err(restore_error)) => Err(restore_error),
        (Err(build_error), Err(restore_error)) => Err(Error::new(format!(
            "synthetic build failed and model restoration also failed: {}; {}",
            build_error.message(),
            restore_error.message()
        ))),
    }
}

fn sandbox_config(config: &Config, root: &Path) -> Config {
    let mut value = config.clone();
    value.roots.supervisor_state = root.join("supervisor");
    value.roots.candidate_store = root.join("candidates");
    value.roots.model_handoff_root = root.join("model-handoff");
    value.roots.model_handoff_ledger = root.join("model-handoff/receipts.jsonl");
    value.roots.candidate_work = root.join("work");
    value.roots.build_store = root.join("builds");
    value.roots.releases = root.join("releases");
    value.roots.active_link = root.join("current");
    value.roots.generation_binding = root.join("current-generation");
    value.roots.maintenance_lease = root.join("maintenance.json");
    value.roots.maintenance_mutex = root.join("maintenance.lock");
    value.roots.state_snapshots = root.join("snapshots");
    value.roots.workspace = root.join("workspace");
    value.roots.system_unit_root = root.join("systemd");
    value.roots.unit_policy = root.join("supervisor/unit-policy.json");
    value.roots.unit_transactions = root.join("snapshots/unit-transactions");
    value.drain.autonomy_state = root.join("workspace/autonomous/state.json");
    value.drain.model_lock = root.join("model.lock");
    value.drain.maintenance_edge_acknowledgement = root.join("workspace/edge-ack.json");
    value.drain.maintenance_core_acknowledgement = root.join("core-ack.json");
    value.drain.activity_ledgers = vec![root.join("workspace/actions/receipts.jsonl")];
    value
}

fn prepare_sandbox(config: &Config) -> Result<()> {
    let updater_staging = generation::updater_staging_root(config)?;
    for path in [
        &config.roots.supervisor_state,
        &config.roots.candidate_store,
        &config.roots.model_handoff_root,
        &config.roots.candidate_work,
        &config.roots.build_store,
        &config.roots.releases,
        &config.roots.state_snapshots,
        &config.roots.workspace,
        &config.roots.system_unit_root,
        &config.roots.unit_transactions,
        &updater_staging,
    ] {
        fs::create_dir_all(path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    std::os::unix::fs::chown(
        &updater_staging,
        Some(config.identities.updater_uid),
        Some(config.identities.updater_gid),
    )?;
    fs::create_dir_all(config.roots.workspace.join("autonomous"))?;
    fs::create_dir_all(config.roots.workspace.join("actions"))?;
    fs::write(&config.drain.autonomy_state, b"{}\n")?;
    fs::write(&config.drain.activity_ledgers[0], b"")?;
    Ok(())
}

fn install_fake_unit_boundary(config: &Config, base: &Path) -> Result<FakeServiceRunner> {
    let icp = config.appliance_id.starts_with("icp");
    let mut policy_dropins = Vec::new();
    let mut effective_dropins = BTreeMap::new();
    for unit in MUTABLE_UNIT_FRAGMENTS {
        let logical = if icp {
            format!("packaging/systemd/icp/{unit}")
        } else {
            format!("packaging/systemd/{unit}")
        };
        let text = fs::read_to_string(base.join(&logical))?;
        let installed = normalized_system_unit(&logical, &text, &config.roots.active_link)?;
        atomic_write(
            &config.roots.system_unit_root.join(unit),
            &installed,
            0o644,
            false,
        )?;
        let dropin_root = config.roots.system_unit_root.join(format!("{unit}.d"));
        fs::create_dir(&dropin_root)?;
        let boundary = dropin_root.join("90-root-runtime-boundary.conf");
        fs::write(&boundary, b"[Service]\nNoNewPrivileges=yes\n")?;
        let mut paths = vec![boundary.display().to_string()];
        policy_dropins.push(policy_dropin(config, &boundary)?);
        if *unit == "astrid-edge-runtime.service" {
            let root_boundary = dropin_root.join("60-self-evolution-root.conf");
            fs::write(&root_boundary, b"[Service]\nProtectSystem=strict\n")?;
            paths.push(root_boundary.display().to_string());
            policy_dropins.push(policy_dropin(config, &root_boundary)?);
        }
        effective_dropins.insert((*unit).to_owned(), paths);
    }
    policy_dropins.sort();
    let policy = UnitPolicy {
        schema: "astrid.edge_rescue_helper.unit_policy.v1".to_owned(),
        authority: "operator_bootstrap_reviewed_immutable_dropins".to_owned(),
        system_unit_root: config.roots.system_unit_root.display().to_string(),
        mutable_fragments: MUTABLE_UNIT_FRAGMENTS
            .iter()
            .map(|unit| (*unit).to_owned())
            .collect(),
        immutable_dropins: policy_dropins,
    };
    atomic_write(
        &config.roots.unit_policy,
        &canonical_json(&policy)?,
        0o600,
        false,
    )?;
    Ok(FakeServiceRunner {
        system_root: config.roots.system_unit_root.clone(),
        dropins: effective_dropins,
    })
}

fn policy_dropin(config: &Config, path: &Path) -> Result<PolicyDropin> {
    let relative = path
        .strip_prefix(&config.roots.system_unit_root)
        .map_err(|_| Error::new("synthetic drop-in escaped system root"))?;
    let bytes = fs::read(path)?;
    Ok(PolicyDropin {
        path: relative.to_string_lossy().to_string(),
        size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        sha256: sha256(&bytes),
    })
}

struct FakeServiceRunner {
    system_root: PathBuf,
    dropins: BTreeMap<String, Vec<String>>,
}

impl NativeRunner for FakeServiceRunner {
    fn run(&mut self, spec: &CommandSpec) -> Result<CommandReceipt> {
        Ok(receipt(spec))
    }

    fn run_capture(
        &mut self,
        spec: &CommandSpec,
        _maximum: u64,
    ) -> Result<(CommandReceipt, Vec<u8>)> {
        let unit = spec.arguments.get(1).cloned().unwrap_or_default();
        let output = if spec
            .arguments
            .iter()
            .any(|argument| argument == "--property=FragmentPath")
        {
            format!("{}\n", self.system_root.join(&unit).display())
        } else {
            format!(
                "{}\n",
                self.dropins
                    .get(&unit)
                    .cloned()
                    .unwrap_or_default()
                    .join(" ")
            )
        };
        Ok((receipt(spec), output.into_bytes()))
    }
}

fn receipt(spec: &CommandSpec) -> CommandReceipt {
    CommandReceipt {
        label: spec.label.to_owned(),
        execution_boundary: crate::native::CommandExecutionBoundary::TrustedHost,
        executable_sha256: spec.executable.sha256.clone(),
        argv_sha256: "0".repeat(64),
        exit_code: Some(0),
        timed_out: false,
        duration_ms: 1,
    }
}

fn copy_tree(root: &Path, current: &Path, destination: &Path) -> Result<()> {
    if current == root {
        fs::create_dir(destination)?;
    }
    for entry in fs::read_dir(current)? {
        let source = entry?.path();
        let metadata = fs::symlink_metadata(&source)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::new("synthetic base generation contains a link"));
        }
        let target = destination.join(
            source
                .strip_prefix(root)
                .map_err(|_| Error::new("synthetic generation copy escaped base"))?,
        );
        if metadata.is_dir() {
            fs::create_dir(&target)?;
            copy_tree(root, &source, destination)?;
            fs::set_permissions(
                &target,
                fs::Permissions::from_mode(metadata.mode() & 0o7777),
            )?;
        } else if metadata.is_file() && metadata.nlink() == 1 {
            fs::copy(&source, &target)?;
            fs::set_permissions(
                &target,
                fs::Permissions::from_mode(metadata.mode() & 0o7777),
            )?;
        } else {
            return Err(Error::new("synthetic generation copy found a special file"));
        }
    }
    Ok(())
}

fn replace_active_link(config: &Config, generation: &Path) -> Result<()> {
    ensure_within(&config.roots.releases, generation, true)?;
    let name = generation
        .file_name()
        .ok_or_else(|| Error::new("synthetic generation has no name"))?;
    let temporary = config.roots.active_link.with_extension("synthetic-next");
    if temporary.exists() || temporary.is_symlink() {
        fs::remove_file(&temporary)?;
    }
    std::os::unix::fs::symlink(Path::new("releases").join(name), &temporary)?;
    fs::rename(&temporary, &config.roots.active_link)?;
    Ok(())
}

fn validate_evidence(evidence: &SyntheticLifecycleEvidence) -> Result<()> {
    let expected_model_labels = [
        "build-model-stop",
        "build-model-start",
        "build-model-warmup",
    ];
    if evidence.candidate_source_changed
        || !evidence.offline_build_and_package_gates_passed
        || !evidence.isolated_activation_passed
        || !evidence.isolated_rollback_passed
        || !evidence.link_first_crash_recovered
        || !evidence.binding_first_crash_recovered
        || evidence.production_intent_created
        || evidence.production_generation_switched
        || evidence.continuity_or_reservoir_admission
        || evidence.model_service_receipts.len() != 3
        || evidence
            .model_service_receipts
            .iter()
            .zip(expected_model_labels)
            .any(|(receipt, label)| {
                receipt.label != label
                    || receipt.timed_out
                    || receipt.exit_code != Some(0)
                    || !crate::config::valid_hex64(&receipt.executable_sha256)
                    || !crate::config::valid_hex64(&receipt.argv_sha256)
            })
        || evidence.production_binding_sha256_before != evidence.production_binding_sha256_after
        || evidence.production_active_link_before != evidence.production_active_link_after
    {
        return Err(Error::new("synthetic lifecycle evidence failed closed"));
    }
    Ok(())
}

fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

    use super::{SyntheticLifecycleEvidence, retained_run_count, validate_evidence};

    fn evidence() -> SyntheticLifecycleEvidence {
        SyntheticLifecycleEvidence {
            schema: "astrid.edge_rescue_helper.synthetic_lifecycle.v1",
            provenance: "operator_isolated_synthetic_machine_evidence_not_astrid_authorship",
            appliance_id: "test".into(),
            production_generation_before: "gen-old".into(),
            production_binding_sha256_before: "a".repeat(64),
            production_binding_sha256_after: "a".repeat(64),
            production_active_link_before: "releases/gen-old".into(),
            production_active_link_after: "releases/gen-old".into(),
            synthetic_candidate_id: "synthetic-a".into(),
            synthetic_build_id: "build-a".into(),
            synthetic_generation_id: "gen-a".into(),
            model_service_receipts: [
                "build-model-stop",
                "build-model-start",
                "build-model-warmup",
            ]
            .into_iter()
            .map(|label| crate::native::CommandReceipt {
                label: label.into(),
                execution_boundary: crate::native::CommandExecutionBoundary::TrustedHost,
                executable_sha256: "b".repeat(64),
                argv_sha256: "c".repeat(64),
                exit_code: Some(0),
                timed_out: false,
                duration_ms: 1,
            })
            .collect(),
            candidate_source_changed: false,
            offline_build_and_package_gates_passed: true,
            isolated_activation_passed: true,
            isolated_rollback_passed: true,
            link_first_crash_recovered: true,
            binding_first_crash_recovered: true,
            production_intent_created: false,
            production_generation_switched: false,
            continuity_or_reservoir_admission: false,
            sandbox_root: "/isolated".into(),
            evidence_sha256: String::new(),
        }
    }

    #[test]
    fn synthetic_evidence_cannot_claim_any_production_authority() {
        let mut value = evidence();
        assert!(validate_evidence(&value).is_ok());
        value.production_generation_switched = true;
        assert!(validate_evidence(&value).is_err());
        value.production_generation_switched = false;
        value.continuity_or_reservoir_admission = true;
        assert!(validate_evidence(&value).is_err());
    }

    #[test]
    fn retained_runs_are_exact_direct_owner_only_directories() {
        let temporary = tempfile::tempdir().unwrap();
        let parent = temporary.path();
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).unwrap();
        let metadata = fs::metadata(parent).unwrap();
        let run = parent.join("synthetic-1-2-3");
        fs::create_dir(&run).unwrap();
        fs::set_permissions(&run, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            retained_run_count(parent, metadata.uid(), metadata.dev()).unwrap(),
            1
        );
        fs::write(parent.join("unexpected"), b"not a retained run").unwrap();
        assert!(retained_run_count(parent, metadata.uid(), metadata.dev()).is_err());
    }
}
