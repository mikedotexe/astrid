//! Immutable state flush, hindsight attestation, and rollback-fixture helper.

use std::path::PathBuf;

use astrid_edge_checkpoint::{Error, Result, checkpoint, flush, snapshot};
use astrid_edge_rescue_helper::generation::require_effective_uid;

fn main() {
    if let Err(error) = run() {
        eprintln!("astrid-edge-checkpoint: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    let command = arguments
        .first()
        .and_then(|value| value.to_str())
        .ok_or_else(usage)?;
    let rest = &arguments[1..];
    match command {
        "flush" => {
            require_effective_uid(0, "immutable state flush")?;
            let workspace = exact_path(rest, "--workspace")?;
            let output = exact_path(rest, "--output")?;
            let generation_id = exact_string(rest, "--generation-id")?;
            exact_arity(rest, 6)?;
            flush::flush(&workspace, &output, &generation_id)
        },
        "checkpoint" => {
            require_effective_uid(0, "immutable hindsight checkpoint")?;
            let workspace = exact_path(rest, "--workspace")?;
            let output = exact_path(rest, "--output")?;
            let generation_id = exact_string(rest, "--generation-id")?;
            let reason = exact_string(rest, "--reason")?;
            let maximum_age = exact_u64(rest, "--maximum-age-seconds")?;
            exact_arity(rest, 10)?;
            checkpoint::record(&workspace, &output, &generation_id, &reason, maximum_age)
        },
        "verify-health" => {
            let workspace = exact_path(rest, "--workspace")?;
            let generation_id = exact_string(rest, "--generation-id")?;
            let maximum_age = exact_u64(rest, "--maximum-age-seconds")?;
            exact_arity(rest, 6)?;
            checkpoint::print_attestation(&workspace, &generation_id, maximum_age)
        },
        "snapshot" => {
            require_effective_uid(0, "immutable rollback snapshot")?;
            let workspace = exact_path(rest, "--workspace")?;
            let output = exact_path(rest, "--output")?;
            let generation_id = exact_string(rest, "--generation-id")?;
            let quiescence_record_sha256 = exact_string(rest, "--quiescence-record-sha256")?;
            if !has_flag(rest, "--require-dual-readable") || rest.len() != 9 {
                return Err(usage());
            }
            snapshot::create(
                &workspace,
                &output,
                &generation_id,
                &quiescence_record_sha256,
            )
        },
        "verify-snapshot" => {
            let snapshot = exact_path(rest, "--snapshot")?;
            let generation_id = exact_string(rest, "--generation-id")?;
            exact_arity(rest, 4)?;
            snapshot::verify(&snapshot, &generation_id).map(|_| ())
        },
        "restore" => {
            require_effective_uid(0, "immutable rollback state restore")?;
            let workspace = exact_path(rest, "--workspace")?;
            let snapshot = exact_path(rest, "--snapshot")?;
            let generation_id = exact_string(rest, "--generation-id")?;
            let transaction_id = exact_string(rest, "--transaction-id")?;
            exact_arity(rest, 8)?;
            snapshot::restore(&workspace, &snapshot, &generation_id, &transaction_id)
        },
        _ => Err(usage()),
    }
}

fn exact_path(arguments: &[std::ffi::OsString], name: &str) -> Result<PathBuf> {
    let value = exact_option(arguments, name)?;
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(Error::new("checkpoint paths must be absolute"));
    }
    Ok(path)
}

fn exact_string(arguments: &[std::ffi::OsString], name: &str) -> Result<String> {
    exact_option(arguments, name)?
        .into_string()
        .map_err(|_| Error::new("checkpoint option is not UTF-8"))
}

fn exact_u64(arguments: &[std::ffi::OsString], name: &str) -> Result<u64> {
    exact_string(arguments, name)?
        .parse()
        .map_err(|_| Error::new("checkpoint numeric option is malformed"))
}

fn exact_option(arguments: &[std::ffi::OsString], name: &str) -> Result<std::ffi::OsString> {
    let mut found = None;
    for pair in arguments.chunks(2) {
        if pair.len() == 2
            && pair[0].to_str() == Some(name)
            && found.replace(pair[1].clone()).is_some()
        {
            return Err(Error::new("duplicate checkpoint option"));
        }
    }
    found.ok_or_else(usage)
}

fn has_flag(arguments: &[std::ffi::OsString], name: &str) -> bool {
    arguments
        .iter()
        .filter(|value| value.to_str() == Some(name))
        .count()
        == 1
}

fn exact_arity(arguments: &[std::ffi::OsString], expected: usize) -> Result<()> {
    if arguments.len() != expected {
        return Err(usage());
    }
    Ok(())
}

fn usage() -> Error {
    Error::new(
        "usage: astrid-edge-checkpoint (flush|checkpoint|verify-health|snapshot|verify-snapshot|restore) with exact fixed options",
    )
}
