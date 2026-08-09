use std::path::PathBuf;

use astrid_edge_web_broker::{Config, Error, Result, initialize_response_keypair, run};

const USAGE: &str = "usage: astrid-edge-web-broker --config ABSOLUTE_JSON | --key-init --signing-seed ABSOLUTE_PATH --verify-key ABSOLUTE_PATH";

fn main() {
    if let Err(error) = execute() {
        eprintln!("astrid-edge-web-broker: {error}");
        std::process::exit(1);
    }
}

fn execute() -> Result<()> {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    match arguments.next().as_deref() {
        Some(value) if value == std::ffi::OsStr::new("--config") => {
            let path = arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| Error::new("configuration path is required"))?;
            if arguments.next().is_some() {
                return Err(Error::new("unexpected command-line argument"));
            }
            let config = Config::from_root_owned_file(&path)?;
            run(&config)
        },
        Some(value) if value == std::ffi::OsStr::new("--key-init") => {
            if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--signing-seed")) {
                return Err(Error::new(USAGE));
            }
            let signing_seed = arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| Error::new(USAGE))?;
            if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--verify-key")) {
                return Err(Error::new(USAGE));
            }
            let verify_key = arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| Error::new(USAGE))?;
            if arguments.next().is_some() {
                return Err(Error::new(USAGE));
            }
            let result = initialize_response_keypair(&signing_seed, &verify_key)?;
            println!(
                "{}",
                serde_json::to_string(&result).map_err(|error| Error::new(error.to_string()))?
            );
            Ok(())
        },
        _ => Err(Error::new(USAGE)),
    }
}
