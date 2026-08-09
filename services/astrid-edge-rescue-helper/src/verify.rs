//! Installation attestation for authority separation, source inventory, and Rust pinning.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::time::Duration;

use serde::Serialize;

use crate::config::{Config, RUST_COMMIT, RUST_RELEASE};
use crate::manifest::SourceSnapshot;
use crate::native::{CommandSpec, NativeRunner, require_success};
use crate::{Error, Result};
use crate::{
    generation::validate_release_manifest,
    transition::{active_target, read_generation_binding},
};

#[derive(Debug, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct InstallVerification {
    pub schema: &'static str,
    pub source_id: String,
    pub repository_commit: String,
    pub rust_release: &'static str,
    pub rust_commit: &'static str,
    pub authority_roots_separate: bool,
    pub source_inventory_valid: bool,
    pub trusted_executables_valid: bool,
    pub active_unit_layout_valid: bool,
    pub active_generation_manifest_valid: bool,
    pub generation_binding_matches_active_link: bool,
    pub bounded_storage_valid: bool,
}

pub fn verify<R: NativeRunner>(config: &Config, runner: &mut R) -> Result<InstallVerification> {
    config.validate()?;
    for executable in config.executables.all() {
        executable.verify()?;
    }
    verify_directory(&config.source.root, 0, false, "source root")?;
    verify_directory(
        &config.roots.candidate_store,
        config.identities.steward_uid,
        true,
        "candidate store",
    )?;
    let _storage = crate::storage::verify(config, true)?;
    verify_directory(&config.roots.candidate_work, 0, true, "candidate work")?;
    verify_directory(&config.roots.build_store, 0, true, "build store")?;
    verify_directory(&config.roots.releases, 0, true, "release root")?;
    verify_directory(&config.roots.state_snapshots, 0, false, "state snapshots")?;
    verify_directory(
        &crate::generation::updater_staging_root(config)?,
        config.identities.updater_uid,
        true,
        "updater generation staging",
    )?;
    verify_directory(
        &config.roots.workspace,
        config.identities.runtime_uid,
        true,
        "runtime workspace",
    )?;
    let snapshot = SourceSnapshot::load(config, true)?;
    let binding = read_generation_binding(config, true)?;
    let bound_generation = config.roots.releases.join(&binding);
    let identity = validate_release_manifest(config, &bound_generation)?;
    if identity.generation_id != binding
        || active_target(config)? != fs::canonicalize(&bound_generation)?
    {
        return Err(Error::new(
            "active generation link, binding, and validated manifest disagree",
        ));
    }
    verify_active_unit_layout(config, runner)?;
    let spec = CommandSpec {
        label: "rustc-version",
        executable: config.executables.rustc.clone(),
        arguments: vec!["-Vv".to_owned()],
        current_dir: config.roots.workspace.clone(),
        environment: BTreeMap::from([("LANG".to_owned(), "C.UTF-8".to_owned())]),
        timeout: Duration::from_secs(30),
        run_as_uid: None,
        run_as_gid: None,
    };
    let (receipt, output) = runner.run_capture(&spec, 8 * 1024)?;
    require_success(&receipt)?;
    let output =
        std::str::from_utf8(&output).map_err(|_| Error::new("rustc identity is not UTF-8"))?;
    if !output
        .lines()
        .next()
        .is_some_and(|line| line.starts_with(&format!("rustc {RUST_RELEASE} ")))
        || !output
            .lines()
            .any(|line| line == format!("commit-hash: {RUST_COMMIT}"))
        || !output
            .lines()
            .any(|line| line == format!("release: {RUST_RELEASE}"))
        || !output
            .lines()
            .any(|line| line == format!("host: {}", config.target))
    {
        return Err(Error::new("toolchain is not exact pinned Rust 1.94.1"));
    }
    Ok(InstallVerification {
        schema: "astrid.edge_rescue_helper.install_verification.v1",
        source_id: snapshot.source_id,
        repository_commit: snapshot.repository_commit,
        rust_release: RUST_RELEASE,
        rust_commit: RUST_COMMIT,
        authority_roots_separate: true,
        source_inventory_valid: true,
        trusted_executables_valid: true,
        active_unit_layout_valid: true,
        active_generation_manifest_valid: true,
        generation_binding_matches_active_link: true,
        bounded_storage_valid: true,
    })
}

fn verify_active_unit_layout<R: NativeRunner>(config: &Config, runner: &mut R) -> Result<()> {
    for (service, executable) in [
        (&config.services.core, "astrid-daemon"),
        (&config.services.warmup, "scripts/warm_ollama_model.sh"),
        (&config.services.edge, "astrid-edge-runtime"),
    ] {
        let spec = CommandSpec {
            label: "systemd-execstart-layout",
            executable: config.executables.systemctl.clone(),
            arguments: vec![
                "show".to_owned(),
                "--property=ExecStart".to_owned(),
                "--value".to_owned(),
                service.clone(),
            ],
            current_dir: config.roots.workspace.clone(),
            environment: BTreeMap::from([("LANG".to_owned(), "C.UTF-8".to_owned())]),
            timeout: Duration::from_secs(30),
            run_as_uid: None,
            run_as_gid: None,
        };
        let (receipt, output) = runner.run_capture(&spec, 16 * 1024)?;
        require_success(&receipt)?;
        let output = std::str::from_utf8(&output)
            .map_err(|_| Error::new("systemd ExecStart metadata is not UTF-8"))?;
        let expected = config.roots.active_link.join(executable);
        if !output.contains(&expected.display().to_string()) {
            return Err(Error::new(format!(
                "{service} ExecStart does not traverse the active generation"
            )));
        }
    }
    Ok(())
}

fn verify_directory(path: &std::path::Path, uid: u32, writable: bool, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != uid
        || (!writable && metadata.mode() & 0o022 != 0)
        || (writable && metadata.mode() & 0o002 != 0)
    {
        return Err(Error::new(format!("{label} ownership or mode failed")));
    }
    Ok(())
}
