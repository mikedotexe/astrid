//! Explicit helper: no default live paths, key generation, or service mode.
#![deny(unsafe_code)]

use std::io::Read as _;
use std::process::ExitCode;

use astrid_minime_protocol::public_web::{PublicWebMutationV2, PublicWebOwnerV2};
use astrid_public_web::{
    PublicWebBundle, PublicWebConfig, PublicWebErrorCode as E, PublicWebRequest, fetch,
    load_approval, mutate, preview, preview_mutation, status,
};
use serde_json::{Value, json};

#[tokio::main]
async fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()).await {
        Ok(value) => {
            println!("{value}");
            ExitCode::SUCCESS
        },
        Err(value) => {
            println!("{value}");
            ExitCode::FAILURE
        },
    }
}

fn failure(code: E) -> Value {
    json!({"complete":false,"code":code})
}

fn is_read_command(args: &[String]) -> bool {
    args.len() == 3 && matches!(args[0].as_str(), "preview" | "status") && args[1] == "--config"
}

fn is_fetch_command(args: &[String]) -> bool {
    args.len() == 5 && args[0] == "fetch" && args[1] == "--config" && args[3] == "--runtime-owner"
}

fn is_mutation_command(args: &[String]) -> bool {
    (args.len() == 9 || (args.len() == 10 && args[9] == "--dry-run"))
        && matches!(args[0].as_str(), "install" | "revoke")
        && args[1] == "--config"
        && args[3] == "--runtime-owner"
        && args[5] == "--bundle"
        && args[7] == "--approval"
}

async fn run(args: Vec<String>) -> Result<Value, Value> {
    if args.first().is_some_and(|arg| arg == "brave-check") {
        if args.len() != 7
            || args[1] != "--credential-file"
            || args[3] != "--account-directory"
            || args[5] != "--account-id"
        {
            return Err(failure(E::InvalidRequest));
        }
        let config = astrid_public_web::BraveSearchConfig {
            credential_file: (&args[2]).into(),
            account_directory: (&args[4]).into(),
        };
        let check = astrid_public_web::check_brave_connection(&config, &args[6])
            .await
            .map_err(|error| {
                serde_json::to_value(error).unwrap_or_else(|_| failure(E::InvalidRequest))
            })?;
        return serde_json::to_value(check).map_err(|_| failure(E::InvalidRequest));
    }
    if args.first().is_some_and(|arg| {
        matches!(
            arg.as_str(),
            "search-account-init" | "search-account-status"
        )
    }) {
        return account_command(&args);
    }
    if !is_read_command(&args) && !is_fetch_command(&args) && !is_mutation_command(&args) {
        return Err(failure(E::InvalidRequest));
    }
    let config = PublicWebConfig::load(std::path::Path::new(&args[2])).map_err(failure)?;
    match args[0].as_str() {
        "preview" => {
            return serde_json::to_value(preview(&config).map_err(failure)?)
                .map_err(|_| failure(E::InvalidRequest));
        },
        "status" => {
            return serde_json::to_value(status(&config).map_err(failure)?)
                .map_err(|_| failure(E::InvalidRequest));
        },
        _ => {},
    }
    let runtime_owner: PublicWebOwnerV2 =
        serde_json::from_str(&args[4]).map_err(|_| failure(E::OwnerMismatch))?;
    if args[0] == "fetch" {
        let mut wire = Vec::new();
        if std::io::stdin()
            .take(16_385)
            .read_to_end(&mut wire)
            .is_err()
            || wire.len() > 16_384
        {
            return Err(failure(E::InvalidRequest));
        }
        let request: PublicWebRequest =
            serde_json::from_slice(&wire).map_err(|_| failure(E::InvalidRequest))?;
        return match fetch(&config, &runtime_owner, &request).await {
            Ok(response) => serde_json::to_value(response).map_err(|_| failure(E::InvalidRequest)),
            Err(error) => {
                Err(serde_json::to_value(error).unwrap_or_else(|_| failure(E::InvalidRequest)))
            },
        };
    }
    let bundle = PublicWebBundle::load(std::path::Path::new(&args[6])).map_err(failure)?;
    let approval = load_approval(std::path::Path::new(&args[8])).map_err(failure)?;
    let operation = if args[0] == "install" {
        PublicWebMutationV2::Install
    } else {
        PublicWebMutationV2::Revoke
    };
    if approval.operation != operation {
        return Err(failure(E::InvalidApproval));
    }
    let result = if args.len() == 10 {
        preview_mutation(&config, &runtime_owner, &bundle, &approval)
    } else {
        mutate(&config, &runtime_owner, &bundle, &approval)
    }
    .map_err(failure)?;
    serde_json::to_value(result).map_err(|_| failure(E::InvalidRequest))
}

fn account_command(args: &[String]) -> Result<Value, Value> {
    if args.len() < 5 || args[1] != "--directory" || args[3] != "--account-id" {
        return Err(failure(E::InvalidRequest));
    }
    let path = std::path::Path::new(&args[2]);
    let status =
        if args[0] == "search-account-init" && args.len() == 7 && args[5] == "--limit-microusd" {
            let limit = args[6].parse().map_err(|_| failure(E::InvalidRequest))?;
            astrid_public_web::initialize_search_account(path, &args[4], limit)
        } else if args[0] == "search-account-status" && args.len() == 5 {
            astrid_public_web::search_account_status(path, &args[4])
        } else {
            return Err(failure(E::InvalidRequest));
        }
        .map_err(failure)?;
    serde_json::to_value(status).map_err(|_| failure(E::InvalidRequest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_requires_explicit_identity_paths_approval_and_operation() {
        for raw in [
            "",
            "install",
            "revoke --config p",
            "fetch --config p",
            "fetch --config p --runtime-owner o --fallback",
            "install --config p --runtime-owner o --bundle b --approval a --force",
        ] {
            let args = raw
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            assert!(
                !is_read_command(&args) && !is_fetch_command(&args) && !is_mutation_command(&args)
            );
        }
        for raw in [
            "status --config p",
            "preview --config p",
            "fetch --config p --runtime-owner o",
            "install --config p --runtime-owner o --bundle b --approval a --dry-run",
            "revoke --config p --runtime-owner o --bundle b --approval a",
        ] {
            let args = raw
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            assert!(
                is_read_command(&args) || is_fetch_command(&args) || is_mutation_command(&args)
            );
        }
    }
}
