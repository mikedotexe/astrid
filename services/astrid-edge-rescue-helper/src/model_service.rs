//! Fixed model-service stop and restoration used only under immutable maintenance.

use std::collections::BTreeMap;
use std::time::Duration;

use crate::config::Config;
use crate::native::{CommandReceipt, CommandSpec, NativeRunner, require_success};
use crate::transition;
use crate::{Error, Result};

const MODEL_UNIT: &str = "ollama-cpu.service";

pub(crate) fn stop<R: NativeRunner>(config: &Config, runner: &mut R) -> Result<CommandReceipt> {
    invoke(config, runner, "build-model-stop", &["stop", MODEL_UNIT])
}

pub(crate) fn restore<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
) -> Result<Vec<CommandReceipt>> {
    Ok(vec![
        invoke(config, runner, "build-model-start", &["start", MODEL_UNIT])?,
        invoke(
            config,
            runner,
            "build-model-warmup",
            &["restart", config.services.warmup.as_str()],
        )?,
    ])
}

/// Fixed `ExecStopPost` recovery. The orphaned lease is removed only after
/// the immutable mutex proves that no build helper still owns the envelope.
pub fn recover_after_interruption<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
) -> Result<Vec<CommandReceipt>> {
    if nix::unistd::geteuid().as_raw() != 0 {
        return Err(Error::new("model build-envelope recovery requires root"));
    }
    if transition::remove_orphaned_build_lease(config)? {
        restore(config, runner)
    } else {
        Ok(Vec::new())
    }
}

fn invoke<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    label: &'static str,
    arguments: &[&str],
) -> Result<CommandReceipt> {
    let receipt = runner.run(&CommandSpec {
        label,
        executable: config.executables.systemctl.clone(),
        arguments: arguments.iter().map(|item| (*item).to_owned()).collect(),
        current_dir: config.roots.workspace.clone(),
        environment: BTreeMap::from([
            ("LANG".to_owned(), "C.UTF-8".to_owned()),
            ("LC_ALL".to_owned(), "C.UTF-8".to_owned()),
            ("TZ".to_owned(), "UTC".to_owned()),
        ]),
        timeout: Duration::from_secs(config.policy.command_timeout_seconds.min(900)),
        run_as_uid: None,
        run_as_gid: None,
    })?;
    require_success(&receipt)?;
    Ok(receipt)
}
