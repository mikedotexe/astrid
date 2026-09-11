use anyhow::{Context as _, Result};
use astrid_source_study::{Catalog, Reader};
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
    state_directory: Option<PathBuf>,
    runtime_workspace: Option<PathBuf>,
    being: Option<String>,
    #[serde(flatten)]
    operation: Operation,
}
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
enum Operation {
    AnalyzeResponse {
        text: String,
        #[serde(default)]
        private_writing: bool,
    },
    RecoverNavigation {
        action: String,
    },
    Prepare {
        action: String,
    },
    Delivered {
        page_id: String,
        request_json: String,
        response_json: String,
    },
    NavigationDelivered {
        navigation_id: String,
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
    if let Operation::AnalyzeResponse {
        text,
        private_writing,
    } = &request.operation
    {
        return Ok(serde_json::to_value(
            astrid_source_study::response_choice::inspect_response(text, *private_writing),
        )?);
    }
    if let Operation::RecoverNavigation { action } = &request.operation {
        return Ok(serde_json::to_value(
            astrid_source_study::recover_local_navigation(action),
        )?);
    }
    let catalog = match (request.astrid_root, request.minime_root) {
        (Some(astrid), Some(minime)) => Catalog::installation(&astrid, &minime)?,
        (None, None) => Catalog::new(request.roots)?,
        _ => anyhow::bail!("both installation roots are required"),
    };
    let mut reader = Reader::new(
        catalog,
        request
            .state_directory
            .context("state_directory is required for reader operations")?,
    );
    if let (Some(workspace), Some(being)) = (request.runtime_workspace, request.being) {
        reader = reader.with_runtime_workspace(workspace, &being);
    }
    match request.operation {
        Operation::AnalyzeResponse { .. } | Operation::RecoverNavigation { .. } => {
            unreachable!("handled before constructing a reader")
        },
        Operation::Prepare { action } => Ok(serde_json::to_value(reader.prepare_action(&action)?)?),
        Operation::Delivered {
            page_id,
            request_json,
            response_json,
        } => Ok(serde_json::to_value(reader.delivered(
            &page_id,
            &request_json,
            &response_json,
        )?)?),
        Operation::NavigationDelivered {
            navigation_id,
            request_json,
            response_json,
        } => Ok(serde_json::to_value(reader.navigation_delivered(
            &navigation_id,
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
