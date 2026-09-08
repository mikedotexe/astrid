use anyhow::Result;
use astrid_source_study::{Catalog, Command, Reader};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::{self, Read as _};
use std::path::PathBuf;

#[derive(Deserialize)]
struct Request {
    #[serde(default)]
    roots: BTreeMap<String, PathBuf>,
    astrid_root: Option<PathBuf>,
    minime_root: Option<PathBuf>,
    state_directory: PathBuf,
    #[serde(flatten)]
    operation: Operation,
}
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
enum Operation {
    Prepare {
        action: String,
    },
    Delivered {
        page_id: String,
        request_json: String,
        response_json: String,
    },
}
fn run() -> Result<serde_json::Value> {
    let mut input = String::new();
    io::stdin()
        .take(16 * 1024 * 1024)
        .read_to_string(&mut input)?;
    let request: Request = serde_json::from_str(&input)?;
    let catalog = match (request.astrid_root, request.minime_root) {
        (Some(astrid), Some(minime)) => Catalog::installation(&astrid, &minime)?,
        (None, None) => Catalog::new(request.roots)?,
        _ => anyhow::bail!("both installation roots are required"),
    };
    let reader = Reader::new(catalog, request.state_directory);
    match request.operation {
        Operation::Prepare { action } => Ok(serde_json::to_value(
            reader.prepare(Command::parse(&action)?)?,
        )?),
        Operation::Delivered {
            page_id,
            request_json,
            response_json,
        } => Ok(serde_json::to_value(reader.delivered(
            &page_id,
            &request_json,
            &response_json,
        )?)?),
    }
}
fn main() {
    match run() {
        Ok(value) => println!("{value}"),
        Err(error) => {
            println!("{}", serde_json::json!({"error": format!("{error:#}")}));
            std::process::exit(1);
        },
    }
}
