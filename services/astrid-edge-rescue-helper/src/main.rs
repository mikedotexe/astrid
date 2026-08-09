use std::path::{Path, PathBuf};

use astrid_edge_rescue_helper::build::Builder;
use astrid_edge_rescue_helper::config::Config;
use astrid_edge_rescue_helper::fs_guard::canonical_json;
use astrid_edge_rescue_helper::generation;
use astrid_edge_rescue_helper::health;
use astrid_edge_rescue_helper::invariant;
use astrid_edge_rescue_helper::native::SystemRunner;
use astrid_edge_rescue_helper::transition;
use astrid_edge_rescue_helper::verify;
use astrid_edge_rescue_helper::{Error, ErrorKind, Result};
use serde::Serialize;

fn main() {
    if invoked_as_rustup_shim() {
        let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
        if arguments == ["target", "list", "--installed"] {
            println!("wasm32-wasip2");
            return;
        }
        eprintln!("astrid-edge-rescue-helper: unsupported fixed rustup shim invocation");
        std::process::exit(64);
    }
    if let Err(error) = run() {
        match error.kind() {
            ErrorKind::DeferredInfrastructure => {
                let receipt = serde_json::json!({
                    "schema": "astrid.edge_rescue_helper.result.v1",
                    "status": "deferred_infrastructure",
                    "reason": error.message(),
                    "retry_authority": "immutable_supervisor_may_retry_after_condition_clears"
                });
                let _ = output(&receipt);
                std::process::exit(75);
            },
            ErrorKind::CandidateRejected => {
                let receipt = serde_json::json!({
                    "schema": "astrid.edge_rescue_helper.result.v1",
                    "status": "candidate_rejected",
                    "reason": error.message(),
                    "retry_authority": "identical_candidate_hash_never_retried_automatically"
                });
                let _ = output(&receipt);
                std::process::exit(65);
            },
            ErrorKind::Terminal => {},
        }
        eprintln!("astrid-edge-rescue-helper: {error}");
        std::process::exit(1);
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the fixed root-helper CLI keeps every allowlisted profile in one auditable dispatch table"
)]
fn run() -> Result<()> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.first().and_then(|value| value.to_str()) == Some("internal-probe-denied-write") {
        let path = exact_path_option(&arguments[1..], "--path")?;
        if arguments.len() != 3 {
            return Err(usage());
        }
        return probe_denied_write(&path);
    }
    if arguments.first().and_then(|value| value.to_str()) == Some("internal-materialize-generation")
    {
        let source = exact_path_option(&arguments[1..], "--source")?;
        let destination = exact_path_option(&arguments[1..], "--destination")?;
        let expected = exact_string_option(&arguments[1..], "--expected-sha256")?;
        if arguments.len() != 7 {
            return Err(usage());
        }
        generation::materialize_generation(&source, &destination, &expected)?;
        return output(&serde_json::json!({
            "schema": "astrid.edge_rescue_helper.updater_materialization.v1",
            "status": "materialized",
            "bundle_sha256": expected,
        }));
    }
    if arguments.len() < 3 || arguments.first().and_then(|value| value.to_str()) != Some("--config")
    {
        return Err(usage());
    }
    let config_path = PathBuf::from(&arguments[1]);
    let command = arguments[2].to_str().ok_or_else(usage)?;
    let rest = &arguments[3..];
    let reflection_command = matches!(
        command,
        "reflection-prepare" | "reflection-cleanup" | "reflection-reconcile"
    );
    if reflection_command {
        return run_reflection(&config_path, command, rest);
    }
    let config = Config::from_root_owned_file(&config_path)?;
    let mut runner = SystemRunner;
    match command {
        "reconcile-storage-reserve" if rest.is_empty() => output(
            &transition::reconcile_storage_reserve(&config, &mut runner)?,
        ),
        "verify-install" if rest.is_empty() => {
            // The boot guard is the only entry point allowed to repair an
            // interrupted active-link/current-generation two-file switch.
            // Reconciliation is journal-bound and occurs before mutable units.
            let _ = transition::reconcile_active_generation(&config, &mut runner)?;
            output(&verify::verify(&config, &mut runner)?)
        },
        "verify-proc-isolation" if rest.is_empty() => output(&verify_proc_isolation(&config)?),
        "build" => {
            let candidate = exact_path_option(rest, "--candidate-manifest")?;
            let intent_envelope = exact_path_option(rest, "--intent-envelope")?;
            let handoff = exact_path_option(rest, "--model-handoff")?;
            let output_path = exact_path_option(rest, "--build-manifest")?;
            if rest.len() != 8 {
                return Err(usage());
            }
            generation::require_effective_uid(0, "root build orchestrator")?;
            output(&Builder::new(&config, &mut runner).build(
                &candidate,
                &intent_envelope,
                &handoff,
                &output_path,
            )?)
        },
        "install" => {
            let manifest = exact_path_option(rest, "--build-manifest")?;
            if rest.len() != 2 {
                return Err(usage());
            }
            let path = generation::install(&config, &mut runner, &manifest)?;
            output(
                &serde_json::json!({"schema":"astrid.edge_rescue_helper.install.v1","generation_dir":path}),
            )
        },
        "profile-bootstrap" => {
            let generation = exact_path_option(rest, "--generation-dir")?;
            if rest.len() != 2 {
                return Err(usage());
            }
            output(
                &astrid_edge_rescue_helper::profile_projection::bootstrap_active_generation(
                    &config,
                    &generation,
                )?,
            )
        },
        "activate" => {
            let generation = exact_path_option(rest, "--generation-dir")?;
            let previous = exact_path_option(rest, "--previous-generation-dir")?;
            if rest.len() != 4 {
                return Err(usage());
            }
            let receipts = transition::activate(&config, &mut runner, &generation, &previous)?;
            output(
                &serde_json::json!({"schema":"astrid.edge_rescue_helper.activation.v1","status":"probation_started","generation_dir":generation,"previous_generation_dir":previous,"receipts":receipts}),
            )
        },
        "rollback" => {
            let generation = exact_path_option(rest, "--generation-dir")?;
            if rest.len() != 2 {
                return Err(usage());
            }
            let receipts = transition::rollback(&config, &mut runner, &generation)?;
            output(
                &serde_json::json!({"schema":"astrid.edge_rescue_helper.rollback.v1","status":"restored","generation_dir":generation,"receipts":receipts}),
            )
        },
        "health" if rest.is_empty() => output(&health::check(&config, &mut runner)?),
        "retention" if rest.is_empty() => {
            output(&astrid_edge_rescue_helper::retention::prune(&config)?)
        },
        "synthetic-lifecycle" if rest.is_empty() => {
            output(&astrid_edge_rescue_helper::synthetic::run(&config)?)
        },
        "recover-model-after-build" if rest.is_empty() => {
            let receipts = astrid_edge_rescue_helper::model_service::recover_after_interruption(
                &config,
                &mut runner,
            )?;
            let status = if receipts.is_empty() {
                "not_needed"
            } else {
                "restored"
            };
            output(&serde_json::json!({
                "schema": "astrid.edge_rescue_helper.model_recovery.v1",
                "status": status,
                "receipts": receipts,
            }))
        },
        "recover-core-liveness" if rest.is_empty() => output(
            &astrid_edge_rescue_helper::core_liveness::recover_if_requested(&config, &mut runner)?,
        ),
        "verify-candidate" => {
            let source = exact_path_option(rest, "--source-root")?;
            let target = exact_string_option(rest, "--target")?;
            let evidence = exact_path_option(rest, "--evidence")?;
            if rest.len() != 6 {
                return Err(usage());
            }
            invariant::verify_candidate(&config, &source, &target, &evidence)?;
            output(&serde_json::json!({
                "schema":"astrid.edge_rescue_helper.internal_replay.v1",
                "kind":"candidate",
                "status":"verified"
            }))
        },
        "verify-package" => {
            let bundle = exact_path_option(rest, "--bundle-root")?;
            let source = exact_path_option(rest, "--source-root")?;
            let target = exact_string_option(rest, "--target")?;
            let evidence = exact_path_option(rest, "--evidence")?;
            if rest.len() != 8 {
                return Err(usage());
            }
            invariant::verify_package(&config, &bundle, &source, &target, &evidence)?;
            output(&serde_json::json!({
                "schema":"astrid.edge_rescue_helper.internal_replay.v1",
                "kind":"package",
                "status":"verified"
            }))
        },
        _ => Err(usage()),
    }
}

fn run_reflection(config_path: &Path, command: &str, rest: &[std::ffi::OsString]) -> Result<()> {
    if !rest.is_empty() {
        return Err(usage());
    }
    let config = Config::from_root_owned_file_for_reflection(config_path)?;
    let result = match command {
        "reflection-prepare" => astrid_edge_rescue_helper::reflection::prepare(&config)?,
        "reflection-cleanup" => astrid_edge_rescue_helper::reflection::cleanup(&config)?,
        "reflection-reconcile" => astrid_edge_rescue_helper::reflection::reconcile(&config)?,
        _ => return Err(usage()),
    };
    output(&result)
}

fn probe_denied_write(path: &Path) -> Result<()> {
    if nix::unistd::geteuid().as_raw() == 0
        || path.file_name().and_then(|name| name.to_str()) != Some(".astrid-builder-denial-probe")
        || path.exists()
        || path.is_symlink()
    {
        return Err(Error::new("builder denial probe invocation is invalid"));
    }
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => Ok(()),
        Ok(_) => {
            let _ = std::fs::remove_file(path);
            Err(Error::new(
                "builder unexpectedly created a production workspace file",
            ))
        },
        Err(error) => Err(Error::new(format!(
            "builder denial probe failed ambiguously: {error}"
        ))),
    }
}

fn verify_proc_isolation(config: &Config) -> Result<serde_json::Value> {
    use std::process::{Command, Stdio};

    generation::require_effective_uid(0, "root process-isolation verifier")?;
    if config.identities.builder_uid == 0 || config.identities.builder_gid == 0 {
        return Err(Error::new(
            "builder process-isolation identity is privileged",
        ));
    }
    let mut foreign = Command::new("/usr/bin/sleep")
        .arg("10")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| Error::new(format!("cannot create foreign process probe: {error}")))?;
    let pid = foreign.id();
    let result = (|| {
        for leaf in ["cmdline", "environ"] {
            let path = format!("/proc/{pid}/{leaf}");
            if !Path::new(&path).is_file() {
                return Err(Error::new("root foreign-process probe disappeared"));
            }
            let status = Command::new("/usr/bin/setpriv")
                .arg(format!("--reuid={}", config.identities.builder_uid))
                .arg(format!("--regid={}", config.identities.builder_gid))
                .arg("--clear-groups")
                .arg("--bounding-set=-all")
                .arg("/usr/bin/test")
                .arg("!")
                .arg("-r")
                .arg(&path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map_err(|error| {
                    Error::new(format!("cannot execute process-isolation probe: {error}"))
                })?;
            if !status.success() {
                return Err(Error::new(
                    "builder can read unrelated root process metadata",
                ));
            }
        }
        Ok(())
    })();
    let _ = foreign.kill();
    let reaped = foreign
        .wait()
        .map_err(|error| Error::new(format!("cannot reap foreign process probe: {error}")))?;
    result?;
    if reaped.success() {
        return Err(Error::new("foreign process probe ended before cleanup"));
    }
    Ok(serde_json::json!({
        "schema": "astrid.edge_rescue_helper.proc_isolation.v1",
        "status": "verified",
        "builder_visibility": "unrelated_process_cmdline_and_environ_hidden",
        "aggregate_proc": "available_for_fixed_compiler_resource_discovery"
    }))
}

fn invoked_as_rustup_shim() -> bool {
    std::env::args_os()
        .next()
        .and_then(|path| {
            PathBuf::from(path)
                .file_name()
                .map(std::ffi::OsStr::to_owned)
        })
        .is_some_and(|name| name == "rustup")
}

fn exact_path_option(arguments: &[std::ffi::OsString], name: &str) -> Result<PathBuf> {
    let mut values = arguments.chunks_exact(2);
    let mut result = None;
    for pair in &mut values {
        let key = pair[0].to_str().ok_or_else(usage)?;
        if key == name {
            if result.is_some() {
                return Err(Error::new("duplicate CLI option"));
            }
            result = Some(PathBuf::from(&pair[1]));
        }
    }
    if !values.remainder().is_empty() {
        return Err(usage());
    }
    let path = result.ok_or_else(usage)?;
    if !path.is_absolute() {
        return Err(Error::new("CLI paths must be absolute"));
    }
    Ok(path)
}

fn exact_string_option(arguments: &[std::ffi::OsString], name: &str) -> Result<String> {
    let mut values = arguments.chunks_exact(2);
    let mut result = None;
    for pair in &mut values {
        let key = pair[0].to_str().ok_or_else(usage)?;
        if key == name {
            if result.is_some() {
                return Err(Error::new("duplicate CLI option"));
            }
            result = Some(pair[1].to_str().ok_or_else(usage)?.to_owned());
        }
    }
    if !values.remainder().is_empty() {
        return Err(usage());
    }
    result.ok_or_else(usage)
}

fn output<T: Serialize>(value: &T) -> Result<()> {
    let mut bytes = canonical_json(value)?;
    bytes.push(b'\n');
    std::io::Write::write_all(&mut std::io::stdout().lock(), &bytes)?;
    Ok(())
}

fn usage() -> Error {
    Error::new(
        "usage: astrid-edge-rescue-helper --config ABSOLUTE (reconcile-storage-reserve | verify-install | verify-proc-isolation | reflection-prepare | reflection-cleanup | reflection-reconcile | profile-bootstrap --generation-dir ABSOLUTE | build --candidate-manifest ABSOLUTE --intent-envelope ABSOLUTE --model-handoff ABSOLUTE --build-manifest ABSOLUTE | install --build-manifest ABSOLUTE | activate --generation-dir ABSOLUTE --previous-generation-dir ABSOLUTE | rollback --generation-dir ABSOLUTE | health | retention | synthetic-lifecycle | recover-model-after-build | recover-core-liveness); verify-candidate and verify-package are fixed internal replay commands",
    )
}

#[allow(dead_code)]
fn _absolute(path: &Path) -> bool {
    path.is_absolute()
}
