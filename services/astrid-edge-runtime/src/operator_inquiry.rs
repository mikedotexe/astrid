//! Isolated operator acceptance harness for the public evidence route.
//!
//! Outputs live under the operator state tree and never enter Astrid's owned
//! workspace, prompt continuity, reservoir, or authorship records.

use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use uuid::Uuid;

use crate::{actions, config::Config, ipc, trace::IpcTraceContextV1};

const HARNESS_SCHEMA: &str = "astrid_edge_operator_inquiry_harness_v1";

pub async fn run(config: &Config, question: &str) -> Result<Value> {
    let question = question.trim();
    let timestamp = unix_millis();
    let root = operator_root(config)?
        .join("inquiry-harness")
        .join(format!("run_{timestamp}_{}", Uuid::new_v4().simple()));
    owner_directory(&root)?;
    let receipt_path = root.join("web_receipts.jsonl");
    let trace = IpcTraceContextV1::root(Uuid::new_v4(), ipc::operator_inquiry_session_id(), None);

    let candidates =
        ipc::execute_operator_inquiry_search(config, question, &receipt_path, &trace).await?;
    persist_search_candidates(&root, question, &candidates, &trace)?;
    let (selected_result, fetch_failures) =
        fetch_first_readable(config, &candidates, &receipt_path, &trace).await;

    let status = if let Some(source) = selected_result.as_ref() {
        persist_verified_source(&root, question, source, &trace)?;
        "completed_with_verified_source"
    } else if candidates
        .iter()
        .any(|candidate| candidate.relevance_score_millis >= 120)
    {
        "useful_candidate_fetch_failed"
    } else {
        "no_useful_evidence"
    };

    let result = json!({
        "schema": HARNESS_SCHEMA,
        "completed_at_unix_ms": unix_millis(),
        "status": status,
        "question": question,
        "candidate_count": candidates.len(),
        "fetch_failures": fetch_failures,
        "output_directory": root,
        "web_receipts": receipt_path,
        "trace": trace,
        "isolation": {
            "astrid_workspace_written": false,
            "thread_updated": false,
            "reservoir_admitted": false,
            "astrid_authorship_claimed": false,
        },
        "authority": "read_only_operator_acceptance_harness_not_astrid_activity",
    });
    owner_write_json(&root.join("result.json"), &result)?;
    Ok(result)
}

fn persist_search_candidates(
    root: &Path,
    question: &str,
    candidates: &[ipc::SelectedSearchResult],
    trace: &IpcTraceContextV1,
) -> Result<()> {
    let candidates = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            json!({
                "rank": index.saturating_add(1),
                "title": candidate.title,
                "url": candidate.url,
                "source_class": candidate.source_class,
                "relevance_score_millis": candidate.relevance_score_millis,
            })
        })
        .collect::<Vec<_>>();
    owner_write_json(
        &root.join("search_candidates.json"),
        &json!({
            "schema": HARNESS_SCHEMA,
            "question": question,
            "candidates": candidates,
            "authority": "operator_harness_search_not_astrid_research_or_authorship",
            "trace": trace,
        }),
    )
}

async fn fetch_first_readable(
    config: &Config,
    candidates: &[ipc::SelectedSearchResult],
    receipt_path: &Path,
    trace: &IpcTraceContextV1,
) -> (Option<ipc::PublicSourceEvidence>, Vec<String>) {
    let mut failures = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate.relevance_score_millis < 120 || is_pdf_url(&candidate.url) {
            continue;
        }
        let result_id = u8::try_from(index.saturating_add(1)).unwrap_or(u8::MAX);
        match ipc::execute_operator_inquiry_fetch(config, candidate, result_id, receipt_path, trace)
            .await
        {
            Ok(source) => return (Some(source), failures),
            Err(error) => failures.push(format!("result {result_id}: {error}")),
        }
    }
    (None, failures)
}

fn persist_verified_source(
    root: &Path,
    question: &str,
    source: &ipc::PublicSourceEvidence,
    trace: &IpcTraceContextV1,
) -> Result<()> {
    let (excerpt, extraction) = actions::readable_source_excerpt(&source.body, 8_000);
    let source_content = format!(
        "# Operator inquiry source\n\n\
         Question: {}\n\
         Title: {}\n\
         Canonical URL: {}\n\
         Retrieved at Unix ms: {}\n\
         Source class: {}\n\
         Relevance score: {:.3}\n\
         HTTP status: {}\n\
         Bounded body SHA-256: {}\n\
         Extraction: {extraction}\n\
         Authority: operator harness evidence; not Astrid research, memory, or authorship\n\n\
         ## Bounded untrusted readable source excerpt\n\n{excerpt}\n",
        one_line(question, 240),
        one_line(&source.title, 300),
        one_line(&source.url, 2_048),
        source.retrieved_at_unix_ms,
        source.source_class,
        source.relevance_score,
        source.status,
        source.body_sha256,
    );
    owner_write_new(&root.join("verified_source.md"), source_content.as_bytes())?;
    let source_sha256 = format!("{:x}", Sha256::digest(source_content.as_bytes()));
    owner_write_json(
        &root.join("synthesis_binding.json"),
        &json!({
            "schema": "astrid_edge_operator_synthesis_binding_v1",
            "evidence_id": "verified_source.md",
            "evidence_sha256": source_sha256,
            "binding_valid": true,
            "claim": "The operator harness verified exact source-hash citation binding.",
            "authority": "deterministic_operator_harness_binding_not_astrid_synthesis_or_authorship",
            "trace_id": trace.trace_id,
        }),
    )
}

fn operator_root(config: &Config) -> Result<PathBuf> {
    let run_directory = config
        .astrid_socket
        .parent()
        .context("Astrid socket has no run directory")?;
    let state_root = run_directory
        .parent()
        .context("Astrid run directory has no state root")?;
    Ok(state_root.join("operator"))
}

fn owner_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("create {}", path.display()))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn owner_write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("create {}", path.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn owner_write_json(path: &Path, value: &Value) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    owner_write_new(path, &bytes)
}

fn one_line(value: &str, maximum: usize) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(maximum)
        .collect()
}

fn is_pdf_url(url: &str) -> bool {
    url.split(['?', '#'])
        .next()
        .and_then(|value| Path::new(value).extension())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::is_pdf_url;

    #[test]
    fn unsupported_pdf_candidates_are_never_selected_for_fetch() {
        assert!(is_pdf_url("https://example.org/paper.PDF?download=1"));
        assert!(!is_pdf_url("https://example.org/article"));
    }
}
