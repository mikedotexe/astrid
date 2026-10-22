//! Offline operator preparation only. No installation, service, or fetch command.
#![deny(unsafe_code)]

mod private_fs;
mod provision;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use serde_json::json;

type Result<T> = std::result::Result<T, &'static str>;

#[derive(Parser)]
#[command(about = "Prepare public-web authority; never install or fetch")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a dedicated issuer in a new private directory. Never replaces one.
    InitIssuer {
        #[arg(long)]
        issuer_dir: PathBuf,
    },
    /// Prepare a 365-day search/page grant from reviewed operator JSON.
    Prepare {
        #[arg(long)]
        issuer_dir: PathBuf,
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Renew only the exact signed approval, preserving its prior-floor binding.
    RenewApproval {
        #[arg(long)]
        issuer_dir: PathBuf,
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Prepare a revoked successor against an explicitly reviewed installed head.
    PrepareRevoke {
        #[arg(long)]
        issuer_dir: PathBuf,
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Command::InitIssuer { issuer_dir } => provision::init_issuer(&issuer_dir),
        Command::Prepare {
            issuer_dir,
            request,
            out,
        } => provision::prepare(&issuer_dir, &request, &out),
        Command::RenewApproval {
            issuer_dir,
            request,
            out,
        } => provision::renew(&issuer_dir, &request, &out),
        Command::PrepareRevoke {
            issuer_dir,
            request,
            out,
        } => provision::revoke(&issuer_dir, &request, &out),
    };
    match result {
        Ok(receipt) => {
            println!("{receipt}");
            ExitCode::SUCCESS
        },
        Err(code) => {
            eprintln!("{}", json!({"complete": false, "code": code}));
            ExitCode::FAILURE
        },
    }
}
