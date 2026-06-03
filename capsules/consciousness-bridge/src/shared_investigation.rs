//! Shared Investigation Object V1 sidecar visibility.
//!
//! The sidecar lives outside Astrid and Minime workspaces so either being can
//! cite it without owning the other's lifecycle.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::paths::bridge_paths;

#[derive(Debug, Clone)]
pub struct RenderedSharedInvestigation {
    pub output_dir: PathBuf,
    pub index_html: PathBuf,
    pub json_path: PathBuf,
    pub investigation: Value,
}

#[must_use]
pub fn root_dir() -> PathBuf {
    bridge_paths()
        .shared_collaborations_dir()
        .join("shared_investigations")
}

pub fn list() -> Result<Vec<Value>> {
    list_from_root(&root_dir())
}

pub fn list_from_root(root: &Path) -> Result<Vec<Value>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut rows = Vec::new();
    for entry in fs::read_dir(root).with_context(|| format!("read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path().join("investigation.json");
        if !path.exists() {
            continue;
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("read shared investigation {}", path.display()))?;
        let value = serde_json::from_str::<Value>(&raw)
            .with_context(|| format!("parse shared investigation {}", path.display()))?;
        rows.push(value);
    }
    rows.sort_by_key(|right| std::cmp::Reverse(sort_ts(right)));
    Ok(rows)
}

pub fn get(selector: Option<&str>) -> Result<Value> {
    get_from_root(&root_dir(), selector)
}

pub fn get_from_root(root: &Path, selector: Option<&str>) -> Result<Value> {
    let selector = selector.unwrap_or("latest").trim();
    let rows = list_from_root(root)?;
    if rows.is_empty() {
        anyhow::bail!("No shared investigations found under {}.", root.display());
    }
    if selector.is_empty() || matches!(selector, "latest" | "current") {
        return rows
            .into_iter()
            .next()
            .context("shared investigation list unexpectedly empty");
    }
    rows.into_iter()
        .find(|row| {
            row.get("id").and_then(Value::as_str) == Some(selector)
                || row
                    .get("title")
                    .and_then(Value::as_str)
                    .is_some_and(|title| title.eq_ignore_ascii_case(selector))
        })
        .with_context(|| format!("Shared investigation `{selector}` not found."))
}

pub fn render(
    selector: Option<&str>,
    output_base: Option<&Path>,
) -> Result<RenderedSharedInvestigation> {
    render_from_root(&root_dir(), selector, output_base)
}

pub fn render_from_root(
    root: &Path,
    selector: Option<&str>,
    output_base: Option<&Path>,
) -> Result<RenderedSharedInvestigation> {
    let investigation = get_from_root(root, selector)?;
    let id = investigation
        .get("id")
        .and_then(Value::as_str)
        .context("shared investigation missing id")?;
    let dir = root.join(id);
    let claims = read_jsonl(&dir.join("claims.jsonl"))?;
    let decisions = read_jsonl(&dir.join("decisions.jsonl"))?;
    let events = read_jsonl(&dir.join("events.jsonl"))?;
    let linked_experiments = linked_experiment_summaries(&investigation);
    let artifact_refs = artifact_refs(&claims, &decisions);
    let output_root = output_base.map_or_else(
        || {
            bridge_paths()
                .bridge_workspace()
                .join("diagnostics/shared_investigation")
        },
        Path::to_path_buf,
    );
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let output_dir = unique_dir(&output_root.join(format!("{}_{}", stamp, sanitize_for_path(id))));
    fs::create_dir_all(&output_dir)?;
    let json_path = output_dir.join("shared_investigation.json");
    let index_html = output_dir.join("index.html");
    let payload = json!({
        "schema_version": 1,
        "rendered_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "investigation": investigation,
        "events": events,
        "claims": claims,
        "decisions": decisions,
        "linked_experiments": linked_experiments,
        "artifact_refs": artifact_refs,
    });
    fs::write(&json_path, serde_json::to_string_pretty(&payload)?)?;
    fs::write(&index_html, render_html(&payload))?;
    Ok(RenderedSharedInvestigation {
        output_dir,
        index_html,
        json_path,
        investigation: payload,
    })
}

pub fn read_sidecar_jsonl(investigation_id: &str, filename: &str) -> Result<Vec<Value>> {
    read_jsonl(&root_dir().join(investigation_id).join(filename))
}

fn read_jsonl(path: &Path) -> Result<Vec<Value>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(raw
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect())
}

fn linked_experiment_summaries(investigation: &Value) -> Vec<Value> {
    investigation
        .get("participants")
        .and_then(Value::as_array)
        .map(|participants| {
            participants
                .iter()
                .map(linked_experiment_summary)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn linked_experiment_summary(participant: &Value) -> Value {
    let being = participant
        .get("being")
        .and_then(Value::as_str)
        .unwrap_or("peer");
    let experiment_id = participant
        .get("experiment_id")
        .and_then(Value::as_str)
        .unwrap_or("unlinked");
    let thread_id = participant.get("thread_id").and_then(Value::as_str);
    let workspace = participant.get("workspace").and_then(Value::as_str);
    let Some((workspace, thread_id)) = workspace.zip(thread_id) else {
        return json!({
            "being": being,
            "experiment_id": experiment_id,
            "status": "missing_snapshot",
            "note": "participant does not include workspace/thread_id",
        });
    };
    let path = Path::new(workspace)
        .join("action_threads")
        .join("threads")
        .join(thread_id)
        .join("experiments.jsonl");
    let latest = read_jsonl(&path)
        .unwrap_or_default()
        .into_iter()
        .rev()
        .find(|row| row.get("experiment_id").and_then(Value::as_str) == Some(experiment_id));
    latest.map_or_else(
        || {
            json!({
                "being": being,
                "experiment_id": experiment_id,
                "thread_id": thread_id,
                "source_path": path.display().to_string(),
                "status": "missing_snapshot",
            })
        },
        |row| {
            json!({
                "being": being,
                "experiment_id": experiment_id,
                "thread_id": thread_id,
                "source_path": path.display().to_string(),
                "status": row.get("status").cloned().unwrap_or(Value::Null),
                "title": row.get("title").cloned().unwrap_or(Value::Null),
                "planned_next": row.get("planned_next").cloned().unwrap_or(Value::Null),
                "updated_at": row.get("updated_at").cloned().unwrap_or(Value::Null),
            })
        },
    )
}

fn artifact_refs(claims: &[Value], decisions: &[Value]) -> Vec<String> {
    let mut refs = Vec::<String>::new();
    for row in claims.iter().chain(decisions.iter()) {
        for key in ["source_refs", "sources", "artifact_refs"] {
            if let Some(values) = row.get(key).and_then(Value::as_array) {
                for value in values {
                    if let Some(text) = value.as_str()
                        && !refs.iter().any(|existing| existing == text)
                    {
                        refs.push(text.to_string());
                    }
                }
            }
        }
    }
    refs
}

fn render_html(payload: &Value) -> String {
    let investigation = &payload["investigation"];
    let id = investigation
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let title = investigation
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Shared Investigation");
    let question = investigation
        .get("shared_question")
        .and_then(Value::as_str)
        .unwrap_or("(no question)");
    let authority = investigation
        .get("authority_boundary")
        .and_then(Value::as_str)
        .unwrap_or("read-only shared continuity");
    let status = investigation
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("active");
    let participant_rows = rows_html(
        payload["linked_experiments"]
            .as_array()
            .into_iter()
            .flatten(),
        |row| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                html_escape(row.get("being").and_then(Value::as_str).unwrap_or("peer")),
                html_escape(
                    row.get("experiment_id")
                        .and_then(Value::as_str)
                        .unwrap_or("unlinked")
                ),
                html_escape(
                    row.get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                ),
                html_escape(
                    row.get("planned_next")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                )
            )
        },
    );
    let claim_rows = rows_html(
        payload["claims"].as_array().into_iter().flatten().take(8),
        |row| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                html_escape(row.get("stance").and_then(Value::as_str).unwrap_or("hold")),
                html_escape(row.get("lane").and_then(Value::as_str).unwrap_or("native")),
                html_escape(row.get("claim").and_then(Value::as_str).unwrap_or(""))
            )
        },
    );
    let decision_rows = rows_html(
        payload["decisions"]
            .as_array()
            .into_iter()
            .flatten()
            .take(8),
        |row| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                html_escape(
                    row.get("decision")
                        .and_then(Value::as_str)
                        .unwrap_or("hold")
                ),
                html_escape(row.get("reason").and_then(Value::as_str).unwrap_or("")),
                html_escape(row.get("created_at").and_then(Value::as_str).unwrap_or(""))
            )
        },
    );
    let refs = payload["artifact_refs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|value| format!("<li>{}</li>", html_escape(value)))
        .collect::<Vec<_>>()
        .join("");
    let refs = if refs.is_empty() {
        "<li>No artifact refs recorded yet.</li>".to_string()
    } else {
        refs
    };
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
:root {{ color-scheme: light dark; font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }}
body {{ margin: 0; background: #f7f7f4; color: #1e2422; }}
main {{ max-width: 1120px; margin: 0 auto; padding: 32px 20px 48px; }}
header {{ border-bottom: 1px solid #c9d1cc; padding-bottom: 18px; }}
h1 {{ font-size: 2rem; margin: 0 0 8px; letter-spacing: 0; }}
h2 {{ font-size: 1.05rem; margin: 28px 0 10px; letter-spacing: 0; }}
.pill {{ display: inline-flex; align-items: center; min-height: 24px; padding: 0 10px; border: 1px solid #7a8b84; border-radius: 999px; font-size: .82rem; }}
.question {{ font-size: 1.08rem; line-height: 1.5; max-width: 820px; }}
.boundary {{ background: #fff6d8; border-left: 4px solid #b88a15; padding: 12px 14px; margin-top: 16px; }}
table {{ border-collapse: collapse; width: 100%; background: #ffffff; }}
th, td {{ border: 1px solid #d6ddd9; padding: 9px 10px; text-align: left; vertical-align: top; font-size: .92rem; }}
th {{ background: #eef2ef; }}
ul {{ background: #ffffff; border: 1px solid #d6ddd9; margin: 0; padding: 12px 12px 12px 28px; }}
@media (prefers-color-scheme: dark) {{
  body {{ background: #141716; color: #edf2ef; }}
  header {{ border-color: #38433f; }}
  .boundary {{ background: #332b16; border-color: #d6a72b; }}
  table, ul {{ background: #1b201e; }}
  th, td, ul {{ border-color: #37423e; }}
  th {{ background: #24302b; }}
}}
</style>
</head>
<body>
<main>
<header>
<span class="pill">{status}</span>
<h1>{title}</h1>
<p>{id}</p>
<p class="question">{question}</p>
<section class="boundary"><strong>Authority Boundary</strong><br>{authority}</section>
</header>
<section>
<h2>Linked Experiments</h2>
<table><thead><tr><th>Being</th><th>Experiment</th><th>Status</th><th>Planned Return</th></tr></thead><tbody>{participant_rows}</tbody></table>
</section>
<section>
<h2>Recent Claims</h2>
<table><thead><tr><th>Stance</th><th>Lane</th><th>Claim</th></tr></thead><tbody>{claim_rows}</tbody></table>
</section>
<section>
<h2>Recent Decisions</h2>
<table><thead><tr><th>Decision</th><th>Reason</th><th>Created</th></tr></thead><tbody>{decision_rows}</tbody></table>
</section>
<section>
<h2>Artifact Refs</h2>
<ul>{refs}</ul>
</section>
</main>
</body>
</html>"#,
        title = html_escape(title),
        id = html_escape(id),
        status = html_escape(status),
        question = html_escape(question),
        authority = html_escape(authority),
        participant_rows = participant_rows,
        claim_rows = claim_rows,
        decision_rows = decision_rows,
        refs = refs,
    )
}

fn rows_html<'a, I, F>(rows: I, render: F) -> String
where
    I: Iterator<Item = &'a Value>,
    F: Fn(&'a Value) -> String,
{
    let html = rows.map(render).collect::<Vec<_>>().join("");
    if html.is_empty() {
        "<tr><td colspan=\"4\">No rows yet.</td></tr>".to_string()
    } else {
        html
    }
}

fn sort_ts(row: &Value) -> u64 {
    row.get("updated_t_ms")
        .or_else(|| row.get("created_t_ms"))
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

fn unique_dir(base: &Path) -> PathBuf {
    if !base.exists() {
        return base.to_path_buf();
    }
    for index in 2..1000_u16 {
        let candidate = base.with_file_name(format!(
            "{}-{index}",
            base.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("shared_investigation")
        ));
        if !candidate.exists() {
            return candidate;
        }
    }
    base.with_file_name(format!(
        "{}-{}",
        base.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("shared_investigation"),
        chrono::Utc::now().timestamp_millis()
    ))
}

fn sanitize_for_path(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '-' | '_' | ' ') && !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_matches('_');
    if out.is_empty() {
        "shared_investigation".to_string()
    } else {
        out.to_string()
    }
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
