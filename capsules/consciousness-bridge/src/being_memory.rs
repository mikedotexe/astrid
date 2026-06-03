//! Read-only visibility for being-owned memory and authority consequences.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::paths::bridge_paths;

#[derive(Debug, Clone)]
pub struct RenderedBeingMemory {
    pub output_dir: PathBuf,
    pub index_html: PathBuf,
    pub json_path: PathBuf,
    pub status: Value,
}

pub fn status() -> Result<Value> {
    status_from_paths(
        bridge_paths().minime_workspace(),
        bridge_paths().bridge_workspace(),
    )
}

pub fn status_from_paths(minime_workspace: &Path, bridge_workspace: &Path) -> Result<Value> {
    Ok(json!({
        "schema_version": 1,
        "policy": "being_memory_visibility_v1",
        "authority_boundary": authority_boundary(),
        "systems": {
            "minime": being_status("minime", minime_workspace)?,
            "astrid": being_status("astrid", bridge_workspace)?,
        }
    }))
}

pub fn render(output_base: Option<&Path>) -> Result<RenderedBeingMemory> {
    render_status_to_base(status()?, output_base)
}

pub fn render_status_to_base(
    status: Value,
    output_base: Option<&Path>,
) -> Result<RenderedBeingMemory> {
    let output_root = output_base.map_or_else(
        || {
            bridge_paths()
                .bridge_workspace()
                .join("diagnostics/being_memory")
        },
        Path::to_path_buf,
    );
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let output_dir = unique_dir(&output_root.join(stamp));
    fs::create_dir_all(&output_dir)?;
    let json_path = output_dir.join("being_memory.json");
    let index_html = output_dir.join("index.html");
    fs::write(&json_path, serde_json::to_string_pretty(&status)?)?;
    fs::write(&index_html, render_html(&status))?;
    Ok(RenderedBeingMemory {
        output_dir,
        index_html,
        json_path,
        status,
    })
}

fn being_status(being: &str, workspace: &Path) -> Result<Value> {
    let root = workspace.join("action_threads/threads");
    let mut memory_rows = Vec::new();
    let mut consequence_rows = Vec::new();
    let mut thread_refs = Vec::new();
    if root.exists() {
        for entry in fs::read_dir(&root).with_context(|| format!("read {}", root.display()))? {
            let thread_dir = entry?.path();
            let memory_path = thread_dir.join("being_memory.jsonl");
            let gate_path = thread_dir.join("authority_gate.jsonl");
            let memory = read_schema_jsonl(&memory_path, "being_memory_v1");
            let consequences = read_schema_jsonl(&gate_path, "authority_consequence_v1");
            if !memory.is_empty() || !consequences.is_empty() {
                thread_refs.push(thread_dir.display().to_string());
            }
            memory_rows.extend(memory);
            consequence_rows.extend(consequences);
        }
    }
    Ok(json!({
        "being": being,
        "workspace": workspace,
        "thread_refs": thread_refs,
        "memory_count": memory_rows.len(),
        "draft_count": memory_rows.iter().filter(|row| row.get("record_type").and_then(Value::as_str) == Some("draft")).count(),
        "consequence_count": consequence_rows.len(),
        "latest_memory": latest(&memory_rows),
        "latest_authority_draft": memory_rows.iter().rev().find(|row| row.get("card_type").and_then(Value::as_str) == Some("authority_request_draft")).cloned(),
        "latest_consequence": latest(&consequence_rows),
        "recent_memory": recent(memory_rows, 8),
        "recent_consequences": recent(consequence_rows, 8),
        "authority_boundary": authority_boundary(),
    }))
}

fn read_schema_jsonl(path: &Path, schema: &str) -> Vec<Value> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    raw.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|row| row.get("record_schema").and_then(Value::as_str) == Some(schema))
        .collect()
}

fn latest(rows: &[Value]) -> Value {
    rows.last().cloned().unwrap_or(Value::Null)
}

fn recent(mut rows: Vec<Value>, limit: usize) -> Vec<Value> {
    if rows.len() > limit {
        rows = rows.split_off(rows.len().saturating_sub(limit));
    }
    rows
}

fn render_html(status: &Value) -> String {
    let pretty = html_escape(&serde_json::to_string_pretty(status).unwrap_or_default());
    let mut cards = String::new();
    if let Some(systems) = status.get("systems").and_then(Value::as_object) {
        for (name, system) in systems {
            let memory_count = system
                .get("memory_count")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let draft_count = system
                .get("draft_count")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let consequence_count = system
                .get("consequence_count")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let latest = system
                .get("latest_memory")
                .and_then(|row| row.get("summary"))
                .and_then(Value::as_str)
                .unwrap_or("none");
            let _ = write!(
                cards,
                "<section><h2>{}</h2><p><strong>Memory:</strong> {} card(s), {} draft(s)</p><p><strong>Consequences:</strong> {}</p><p><strong>Latest:</strong> {}</p></section>",
                html_escape(name),
                memory_count,
                draft_count,
                consequence_count,
                html_escape(latest)
            );
        }
    }
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Being Memory</title><style>body{{font-family:system-ui,sans-serif;margin:32px;line-height:1.45}}section{{border:1px solid #bbb;padding:16px;margin:12px 0;border-radius:8px}}pre{{white-space:pre-wrap}}</style></head><body><h1>Being Memory V1</h1><p><strong>Authority Boundary:</strong> {}</p>{}<h2>Raw JSON</h2><pre>{}</pre></body></html>",
        html_escape(authority_boundary()),
        cards,
        pretty
    )
}

fn html_escape(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn unique_dir(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    for idx in 2_u32..1000 {
        let candidate = path.with_file_name(format!(
            "{}_{idx}",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("render")
        ));
        if !candidate.exists() {
            return candidate;
        }
    }
    path.with_file_name(format!(
        "{}_{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("render"),
        chrono::Utc::now().timestamp()
    ))
}

fn authority_boundary() -> &'static str {
    "Being memory is local, cite-backed, and read-only from the bridge; it never grants bind, resume, perturb, live control, authority approval, execution, or peer mutation."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reads_memory_and_consequence_sidecars() {
        let temp = tempfile::tempdir().unwrap();
        let minime = temp.path().join("minime_workspace");
        let astrid = temp.path().join("astrid_workspace");
        let thread = minime.join("action_threads/threads/th_test");
        fs::create_dir_all(&thread).unwrap();
        fs::write(
            thread.join("being_memory.jsonl"),
            serde_json::to_string(&json!({
                "record_schema": "being_memory_v1",
                "record_type": "card",
                "summary": "remember this",
            }))
            .unwrap()
                + "\n",
        )
        .unwrap();
        fs::write(
            thread.join("authority_gate.jsonl"),
            serde_json::to_string(&json!({
                "record_schema": "authority_consequence_v1",
                "record_type": "consequence",
                "consequence_status": "blocked",
            }))
            .unwrap()
                + "\n",
        )
        .unwrap();
        let status = status_from_paths(&minime, &astrid).unwrap();
        assert_eq!(status["systems"]["minime"]["memory_count"], 1);
        assert_eq!(status["systems"]["minime"]["consequence_count"], 1);
    }
}
