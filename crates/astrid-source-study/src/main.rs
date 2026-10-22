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
    JournalRegistration {
        request_id: String,
        expected_revision: String,
        request: astrid_source_study::JournalRegistration,
    },
    WritingPolicy {
        action: String,
    },
    WritingRecovery {
        input_id: String,
        #[serde(default)]
        final_capacity: Option<astrid_source_study::FinalWritingCapacity>,
    },
    PreparationRevision,
    PrepareOnce {
        request_id: String,
        expected_revision: String,
        action: String,
    },
    Activity {
        request_id: String,
        expected_revision: Option<u64>,
        now_ms: u64,
        request: astrid_source_study::ActivityRequest,
    },
    AnalyzeResponse {
        text: String,
        #[serde(default)]
        private_writing: bool,
    },
    RecoverNavigation {
        action: String,
    },
    SourceActionRoute {
        action: String,
    },
    ResearchSourceRoute {
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
    if let Some(response) = stateless_response(&request.operation)? {
        return Ok(response);
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
        Operation::JournalRegistration {
            request_id,
            expected_revision,
            request,
        } => Ok(serde_json::to_value(reader.prepare_journal_once(
            &request_id,
            &expected_revision,
            &request,
        )?)?),
        Operation::WritingRecovery {
            input_id,
            final_capacity,
        } => writing_recovery_response(&reader, &input_id, final_capacity),
        Operation::PreparationRevision => {
            Ok(serde_json::json!({"revision": reader.preparation_revision()?}))
        },
        Operation::PrepareOnce {
            request_id,
            expected_revision,
            action,
        } => Ok(serde_json::to_value(reader.prepare_once(
            &request_id,
            &expected_revision,
            &action,
        )?)?),
        Operation::Activity {
            request_id,
            expected_revision,
            now_ms,
            request,
        } => Ok(serde_json::to_value(reader.activity(
            &request_id,
            expected_revision,
            now_ms,
            request,
        )?)?),
        Operation::AnalyzeResponse { .. }
        | Operation::SourceActionRoute { .. }
        | Operation::ResearchSourceRoute { .. }
        | Operation::RecoverNavigation { .. }
        | Operation::WritingPolicy { .. } => {
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

fn stateless_response(operation: &Operation) -> Result<Option<serde_json::Value>> {
    let response = match operation {
        Operation::SourceActionRoute { action } => {
            serde_json::json!({"route": astrid_source_study::source_action_route(action)})
        },
        Operation::ResearchSourceRoute { action } => {
            serde_json::json!({"canonical_action": astrid_source_study::research_source_route(action)})
        },
        Operation::WritingPolicy { action } => {
            serde_json::json!({"storage_only": astrid_source_study::writing_storage_only(action)})
        },
        Operation::AnalyzeResponse {
            text,
            private_writing,
        } => serde_json::to_value(astrid_source_study::response_choice::inspect_response(
            text,
            *private_writing,
        ))?,
        Operation::RecoverNavigation { action } => {
            serde_json::to_value(astrid_source_study::recover_local_navigation(action))?
        },
        _ => return Ok(None),
    };
    Ok(Some(response))
}

fn writing_recovery_response(
    reader: &Reader,
    input_id: &str,
    final_capacity: Option<astrid_source_study::FinalWritingCapacity>,
) -> Result<serde_json::Value> {
    let recovery = match final_capacity {
        Some(capacity) => reader.writing_capacity_recovery(input_id, capacity)?,
        None => reader.writing_recovery(input_id)?,
    };
    Ok(serde_json::to_value(recovery)?)
}
fn main() {
    match run() {
        Ok(value) => println!("{value}"),
        Err(error) => {
            println!(
                "{}",
                serde_json::json!({"error": format!("{error:#}"), "writing_recovery": error.downcast_ref::<astrid_source_study::WritingRecovery>()})
            );
            std::process::exit(1);
        },
    }
}
