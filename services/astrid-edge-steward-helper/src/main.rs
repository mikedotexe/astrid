#![deny(unsafe_code)]

use std::env;
use std::path::PathBuf;

use astrid_edge_steward_helper::{
    Config, RunRequest, run_once, scheduled_authorship_verifying_key_hex,
};

fn usage() -> &'static str {
    "usage: astrid-edge-steward-helper --config ABSOLUTE_JSON [--credential-directory ABS] [--print-scheduled-authorship-verifying-key]"
}

struct Arguments {
    config: PathBuf,
    credential_directory: Option<PathBuf>,
    print_scheduled_authorship_verifying_key: bool,
}

fn arguments() -> Result<Arguments, String> {
    let mut values = env::args().skip(1);
    let mut config = None;
    let mut credential_directory = None;
    let mut print_scheduled_authorship_verifying_key = false;
    while let Some(argument) = values.next() {
        match argument.as_str() {
            "--config" => config = values.next().map(PathBuf::from),
            "--credential-directory" => credential_directory = values.next().map(PathBuf::from),
            "--print-scheduled-authorship-verifying-key" => {
                print_scheduled_authorship_verifying_key = true;
            },
            "--help" | "-h" => return Err(usage().to_owned()),
            _ => return Err(format!("unsupported argument: {argument}\n{}", usage())),
        }
    }
    let config = config.ok_or_else(|| usage().to_owned())?;
    Ok(Arguments {
        config,
        credential_directory,
        print_scheduled_authorship_verifying_key,
    })
}

fn main() {
    let result = arguments()
        .map_err(astrid_edge_steward_helper::Error::new)
        .and_then(|arguments| {
            let config = Config::from_root_owned_file_with_credentials(
                &arguments.config,
                arguments.credential_directory.as_deref(),
            )?;
            if arguments.print_scheduled_authorship_verifying_key {
                println!("{}", scheduled_authorship_verifying_key_hex(&config)?);
                Ok(None)
            } else {
                run_once(&config, RunRequest::default()).map(Some)
            }
        });
    match result {
        Ok(Some(result)) => {
            println!(
                "{}",
                serde_json::to_string(&result).expect("result is serializable")
            );
        },
        Ok(None) => {},
        Err(error) => {
            eprintln!("astrid-edge-steward-helper: {error}");
            std::process::exit(1);
        },
    }
}
