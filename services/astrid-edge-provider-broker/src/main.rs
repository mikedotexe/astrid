use std::path::PathBuf;

use astrid_edge_provider_broker::{Config, run, run_warmup_client};

fn main() {
    if let Err(error) = command() {
        eprintln!("astrid-edge-provider-broker: {error}");
        std::process::exit(1);
    }
}

fn command() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let command = arguments.next().ok_or("missing command")?;
    if command == "--help" || command == "-h" {
        println!(
            "usage: astrid-edge-provider-broker serve --config ABS --client CLIENT --credential-directory ABS\n       astrid-edge-provider-broker warmup --config ABS --key ABS --receipt ABS"
        );
        ensure_end(&mut arguments)?;
    } else if command == "serve" {
        let config = expect_flag(&mut arguments, "--config")?;
        let client = expect_string_flag(&mut arguments, "--client")?;
        let credentials = expect_flag(&mut arguments, "--credential-directory")?;
        ensure_end(&mut arguments)?;
        run(&Config::from_file(&config)?, &client, &credentials)?;
    } else if command == "warmup" {
        let config = expect_flag(&mut arguments, "--config")?;
        let key = expect_flag(&mut arguments, "--key")?;
        let receipt = expect_flag(&mut arguments, "--receipt")?;
        ensure_end(&mut arguments)?;
        let config = Config::from_file(&config)?;
        run_warmup_client(&config, &key, &receipt)?;
    } else {
        return Err("unsupported command".into());
    }
    Ok(())
}

fn expect_string_flag(
    arguments: &mut impl Iterator<Item = std::ffi::OsString>,
    expected: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let value = expect_flag(arguments, expected)?;
    value
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{expected} is not UTF-8").into())
}

fn expect_flag(
    arguments: &mut impl Iterator<Item = std::ffi::OsString>,
    expected: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new(expected)) {
        return Err(format!("expected {expected}").into());
    }
    arguments
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing value for {expected}").into())
}

fn ensure_end(
    arguments: &mut impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), Box<dyn std::error::Error>> {
    if arguments.next().is_some() {
        return Err("unexpected trailing arguments".into());
    }
    Ok(())
}
