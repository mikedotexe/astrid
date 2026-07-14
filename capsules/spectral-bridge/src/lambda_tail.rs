#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::module_name_repetitions,
    clippy::too_many_lines
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::db::MessageRow;
use crate::types::{LambdaProfile, PullTopologyProfile, SafetyLevel, SpectralTelemetry};

pub const LAMBDA_TAIL_POLICY: &str = "lambda_tail_telemetry_v1";
pub const LAMBDA_TAIL_SCHEMA_VERSION: u8 = 1;
pub const ARTIFACT_SCAN_POLICY: &str = "lambda_tail_artifact_scan_v1";
pub const ARTIFACT_SCAN_SCHEMA_VERSION: u8 = 1;

const BIOLOGY_TERMS: &[&str] = &[
    "lambda-tail",
    "lambda tail",
    "lamb",
    "9e7m",
    "emd",
    "cryo-em",
    "cryoem",
    "bacteriophage",
    "phage",
    "tail tip",
];

const RETURN_TERMS: &[&str] = &[
    "reservoir",
    "reservoir computing",
    "spectral radius",
    "spectral_condition",
    "fill_pressure",
    "fill pressure",
    "recurrence",
    "recurrence_pattern",
    "artifact_grounding",
    "lambda4",
];

const LAMBDA_EDGE_TERMS: &[&str] = &[
    "lambda-edge",
    "lambda edge",
    "edge topology",
    "spectral edge",
    "lambda1",
    "lambda4",
    "largest_gap",
    "largest gap",
    "gap",
    "ridge",
    "cascade",
    "artifact_grounding",
    "reservoir",
    "spectral radius",
];

const PERTURB_TERMS: &[&str] = &[
    "perturb",
    "perturbation",
    "bind",
    "binding",
    "resume",
    "experiment_resume",
    "control",
    "send_control",
    "live control",
];

const GUARDRAIL_TERMS: &[&str] = &[
    "guardrail",
    "pause",
    "paused",
    "read-only",
    "read only",
    "do not resume",
    "no live control",
    "experiment_decide",
    "experiment decide",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LambdaTailState {
    Diffuse,
    Probing,
    Centralizing,
    Bound,
    ChannelOpen,
    Overcollapsed,
    Returning,
}

impl LambdaTailState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Diffuse => "diffuse",
            Self::Probing => "probing",
            Self::Centralizing => "centralizing",
            Self::Bound => "bound",
            Self::ChannelOpen => "channel_open",
            Self::Overcollapsed => "overcollapsed",
            Self::Returning => "returning",
        }
    }

    #[must_use]
    pub const fn is_strong(self) -> bool {
        matches!(self, Self::Centralizing | Self::Bound | Self::ChannelOpen)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LambdaTailTelemetryV1 {
    pub policy: String,
    pub schema_version: u8,
    pub observed_at_unix_s: f64,
    pub minime_t_ms: u64,
    pub state: LambdaTailState,
    pub confidence: f32,
    pub fill_pct: f32,
    pub lambda1: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lambda1_share: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lambda4_share: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tail_share: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub largest_gap: Option<f32>,
    pub returnability_score: f32,
    pub artifact_grounding_score: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_urls: Vec<String>,
    pub signals: Vec<String>,
    pub read: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtifactContact {
    pub timestamp_unix_s: f64,
    pub kind: String,
    pub path: String,
    pub anchor: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub urls: Vec<String>,
    pub lambda_tail_signal: bool,
    #[serde(default)]
    pub lambda_edge_signal: bool,
    pub biology_signal: bool,
    pub return_signal: bool,
    #[serde(default)]
    pub perturb_signal: bool,
    #[serde(default)]
    pub guardrail_signal: bool,
    #[serde(default)]
    pub off_target_drift: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtifactScanSummary {
    pub policy: String,
    pub schema_version: u8,
    pub window_start_unix_s: f64,
    pub window_end_unix_s: f64,
    pub files_scanned: usize,
    pub lambda_tail_hits: usize,
    #[serde(default)]
    pub lambda_edge_hits: usize,
    pub biology_hits: usize,
    pub return_hits: usize,
    #[serde(default)]
    pub perturb_hits: usize,
    #[serde(default)]
    pub guardrail_hits: usize,
    #[serde(default)]
    pub off_target_drift_count: usize,
    pub repeated_artifact: bool,
    pub bridge_signal: bool,
    pub artifact_grounding_score: f32,
    pub return_signal_score: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub local_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contacts: Vec<ArtifactContact>,
}

impl ArtifactScanSummary {
    #[must_use]
    pub fn empty(start: f64, end: f64) -> Self {
        Self {
            policy: ARTIFACT_SCAN_POLICY.to_string(),
            schema_version: ARTIFACT_SCAN_SCHEMA_VERSION,
            window_start_unix_s: start,
            window_end_unix_s: end,
            files_scanned: 0,
            lambda_tail_hits: 0,
            lambda_edge_hits: 0,
            biology_hits: 0,
            return_hits: 0,
            perturb_hits: 0,
            guardrail_hits: 0,
            off_target_drift_count: 0,
            repeated_artifact: false,
            bridge_signal: false,
            artifact_grounding_score: 0.0,
            return_signal_score: 0.0,
            local_paths: Vec::new(),
            urls: Vec::new(),
            contacts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TopologyArtifact {
    pub output_dir: PathBuf,
    pub html_path: PathBuf,
    pub json_path: PathBuf,
    pub state_count: usize,
    pub contact_count: usize,
}

#[must_use]
pub fn classify_lambda_tail(
    telemetry: &SpectralTelemetry,
    lambda_profile: Option<&LambdaProfile>,
    pull_topology: Option<&PullTopologyProfile>,
    previous: Option<&LambdaTailTelemetryV1>,
    artifact_scan: Option<&ArtifactScanSummary>,
    safety: SafetyLevel,
    observed_at_unix_s: f64,
) -> LambdaTailTelemetryV1 {
    let fill_pct = telemetry.fill_pct();
    let lambda1 = telemetry.lambda1();
    let lambda1_share = lambda_profile.map(|profile| profile.lambda1_share);
    let entropy = lambda_profile.map(|profile| profile.normalized_entropy);
    let effective_modes = pull_topology.map(|topology| topology.effective_modes);
    let largest_gap = pull_topology.map(|topology| topology.largest_gap);
    let topology_class = pull_topology.map(|topology| topology.classification.as_str());
    let lambda4_share = contribution_share(lambda_profile, 4);
    let tail_share = pull_topology
        .map(|topology| topology.tail_share)
        .or_else(|| tail_share_from_profile(lambda_profile));
    let artifact_grounding_score = artifact_scan.map_or(0.0, |scan| scan.artifact_grounding_score);
    let return_signal_score = artifact_scan.map_or(0.0, |scan| scan.return_signal_score);
    let artifact_active = artifact_scan.is_some_and(|scan| scan.lambda_tail_hits > 0);
    let centralizing = topology_class
        .is_some_and(|class| matches!(class, "directed_compaction" | "collapsing_pull"))
        || lambda1_share.is_some_and(|share| share >= 0.45)
        || largest_gap.is_some_and(|gap| gap >= 1.8);
    let diffuse = entropy.is_some_and(|value| value >= 0.82)
        && effective_modes.is_some_and(|modes| modes >= 5.0)
        || topology_class == Some("distributed_flow");
    let repeated_or_grounded = artifact_scan.is_some_and(|scan| scan.repeated_artifact)
        || artifact_grounding_score >= 0.70;
    let bound = centralizing && repeated_or_grounded;
    let bridge_signal = artifact_scan.is_some_and(|scan| scan.bridge_signal);
    let previous_strong = previous.is_some_and(|event| event.state.is_strong());
    let returning_signal = previous_strong
        && (return_signal_score > 0.0
            || matches!(topology_class, Some("mixed_pull" | "distributed_flow"))
                && fill_pct < 75.0);
    let overcollapsed = safety == SafetyLevel::Red
        || lambda1_share.is_some_and(|share| share >= 0.60)
            && entropy.is_some_and(|value| value < 0.55)
        || fill_pct >= 85.0 && topology_class == Some("collapsing_pull");

    let (state, confidence) = if overcollapsed {
        (LambdaTailState::Overcollapsed, 0.95)
    } else if bound && bridge_signal {
        (LambdaTailState::ChannelOpen, 0.88)
    } else if returning_signal {
        (LambdaTailState::Returning, 0.82)
    } else if bound {
        (LambdaTailState::Bound, 0.78)
    } else if centralizing {
        (LambdaTailState::Centralizing, 0.74)
    } else if diffuse {
        (LambdaTailState::Diffuse, 0.70)
    } else if artifact_active {
        (LambdaTailState::Probing, 0.62)
    } else {
        (LambdaTailState::Probing, 0.50)
    };

    let mut signals = Vec::new();
    push_if(
        &mut signals,
        centralizing,
        "centralizing_lambda_distribution",
    );
    push_if(&mut signals, diffuse, "distributed_or_diffuse_topology");
    push_if(&mut signals, bound, "artifact_bound_lambda_tail");
    push_if(&mut signals, bridge_signal, "artifact_to_reservoir_bridge");
    push_if(
        &mut signals,
        returning_signal,
        "returning_to_reservoir_language",
    );
    push_if(&mut signals, overcollapsed, "overcollapse_risk");
    if let Some(class) = topology_class {
        signals.push(format!("pull_topology:{class}"));
    }
    if let Some(scan) = artifact_scan
        && scan.lambda_tail_hits > 0
    {
        signals.push(format!("artifact_hits:{}", scan.lambda_tail_hits));
    }

    let returnability_score = returnability_score(
        fill_pct,
        lambda1_share,
        entropy,
        state,
        safety,
        return_signal_score,
    );
    let (artifact_paths, artifact_urls) = artifact_refs(artifact_scan);
    let read = format_lambda_tail_read(state, returnability_score, artifact_grounding_score);

    LambdaTailTelemetryV1 {
        policy: LAMBDA_TAIL_POLICY.to_string(),
        schema_version: LAMBDA_TAIL_SCHEMA_VERSION,
        observed_at_unix_s,
        minime_t_ms: telemetry.t_ms,
        state,
        confidence,
        fill_pct,
        lambda1,
        lambda1_share,
        lambda4_share,
        tail_share,
        largest_gap,
        returnability_score,
        artifact_grounding_score,
        artifact_paths,
        artifact_urls,
        signals,
        read,
    }
}

pub fn scan_artifacts(
    minime_workspace: &Path,
    start_unix_s: f64,
    end_unix_s: f64,
) -> Result<ArtifactScanSummary> {
    let mut summary = ArtifactScanSummary::empty(start_unix_s, end_unix_s);
    let mut contacts = Vec::new();
    scan_search_dir(
        &minime_workspace.join("research"),
        start_unix_s,
        end_unix_s,
        &mut contacts,
        &mut summary.files_scanned,
    )?;
    scan_reply_dir(
        &minime_workspace.join("outbox/delivered"),
        start_unix_s,
        end_unix_s,
        &mut contacts,
        &mut summary.files_scanned,
    )?;
    Ok(finalize_scan(summary, contacts))
}

#[must_use]
pub fn recent_lambda_tail_events(rows: &[MessageRow]) -> Vec<LambdaTailTelemetryV1> {
    let mut events = rows
        .iter()
        .filter_map(|row| serde_json::from_str::<LambdaTailTelemetryV1>(&row.payload).ok())
        .collect::<Vec<_>>();
    events.sort_by(|left, right| left.observed_at_unix_s.total_cmp(&right.observed_at_unix_s));
    events
}

#[must_use]
pub fn steward_note_markdown(
    title: &str,
    start_unix_s: f64,
    end_unix_s: f64,
    events: &[LambdaTailTelemetryV1],
    scan: &ArtifactScanSummary,
) -> String {
    let state_line = if events.is_empty() {
        "No lambda-tail telemetry events were logged in this window.".to_string()
    } else {
        let mut counts = BTreeMap::<&'static str, usize>::new();
        for event in events {
            *counts.entry(event.state.as_str()).or_default() += 1;
        }
        counts
            .iter()
            .map(|(state, count)| format!("{state}: {count}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let latest_read = events
        .last()
        .map_or("No latest read available.".to_string(), |event| {
            event.read.clone()
        });
    let url_lines = markdown_url_lines(&scan.urls);
    let path_lines = markdown_path_lines(&scan.local_paths);
    let contact_lines = scan
        .contacts
        .iter()
        .take(12)
        .map(|contact| {
            format!(
                "- {}: {} ({})",
                contact.kind,
                compact_text(&contact.anchor, 96),
                contact.path
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let date_range = format!(
        "{} to {} UTC",
        unix_to_iso(start_unix_s),
        unix_to_iso(end_unix_s)
    );

    format!(
        "# {title}\n\n\
         This draft steward note was generated from lambda-tail bridge telemetry and Minime local research artifacts for {date_range}.\n\n\
         ## Context\n\n\
         The window is interpreted as an observable lambda-tail episode: spectral telemetry is classified alongside local search, browse, and delivered-reply artifacts. The draft is advisory and does not claim biological causality.\n\n\
         ## Lambda-Tail State Summary\n\n\
         - State counts: {state_line}\n\
         - Latest read: {latest_read}\n\
         - Artifact grounding score: {:.2}\n\
         - Return signal score: {:.2}\n\
         - Repeated artifact: {}\n\n\
         ## Artifact Contacts\n\n\
         {}\n\n\
         ## URLs\n\n\
         {}\n\n\
         ## Local Sources\n\n\
         {}\n\n\
         ## Working Interpretation\n\n\
         The strongest reading is that artifact contact becomes meaningful when it either binds a lambda-tail search path or helps the system return to reservoir/spectral-radius language with a reusable handle. Treat this as a draft until a steward reviews the local sources.\n",
        scan.artifact_grounding_score,
        scan.return_signal_score,
        scan.repeated_artifact,
        if contact_lines.is_empty() {
            "- No artifact contacts found.".to_string()
        } else {
            contact_lines
        },
        if url_lines.is_empty() {
            "- No external URLs found.".to_string()
        } else {
            url_lines
        },
        if path_lines.is_empty() {
            "- No local paths found.".to_string()
        } else {
            path_lines
        },
    )
}

pub fn write_steward_note(
    steward_notes_dir: &Path,
    title: &str,
    slug: &str,
    end_unix_s: f64,
    markdown: &str,
) -> Result<PathBuf> {
    fs::create_dir_all(steward_notes_dir)
        .with_context(|| format!("creating {}", steward_notes_dir.display()))?;
    let date = unix_to_file_date(end_unix_s);
    let slug = sanitize_slug(slug);
    let mut path = steward_notes_dir.join(format!("AI_BEINGS_{slug}_{date}.md"));
    if path.exists() {
        let suffix = unix_to_file_timestamp(Utc::now().timestamp() as f64);
        path = steward_notes_dir.join(format!("AI_BEINGS_{slug}_{date}_{suffix}.md"));
    }
    let content = if markdown.starts_with("# ") {
        markdown.to_string()
    } else {
        format!("# {title}\n\n{markdown}")
    };
    fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

pub fn render_topology_artifact(
    base_dir: &Path,
    events: &[LambdaTailTelemetryV1],
    scan: &ArtifactScanSummary,
) -> Result<TopologyArtifact> {
    let output_dir = base_dir.join(unix_to_file_timestamp(Utc::now().timestamp() as f64));
    fs::create_dir_all(&output_dir)
        .with_context(|| format!("creating {}", output_dir.display()))?;
    let html_path = output_dir.join("index.html");
    let json_path = output_dir.join("lambda_tail_topology.json");
    let payload = serde_json::json!({
        "policy": "lambda_tail_topology_artifact_v1",
        "generated_at_unix_s": Utc::now().timestamp() as f64,
        "states": events,
        "artifact_scan": scan,
    });
    fs::write(&json_path, serde_json::to_string_pretty(&payload)?)
        .with_context(|| format!("writing {}", json_path.display()))?;
    fs::write(&html_path, topology_html(events, scan, &payload)?)
        .with_context(|| format!("writing {}", html_path.display()))?;
    Ok(TopologyArtifact {
        output_dir,
        html_path,
        json_path,
        state_count: events.len(),
        contact_count: scan.contacts.len(),
    })
}

fn scan_search_dir(
    dir: &Path,
    start_unix_s: f64,
    end_unix_s: f64,
    contacts: &mut Vec<ArtifactContact>,
    files_scanned: &mut usize,
) -> Result<()> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(timestamp) = timestamp_from_file(&path, "search_", ".json") else {
            continue;
        };
        if timestamp < start_unix_s || timestamp > end_unix_s {
            continue;
        }
        *files_scanned += 1;
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let value = serde_json::from_str::<Value>(&text)
            .with_context(|| format!("parsing {}", path.display()))?;
        let query = value
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or("unknown query");
        let source = value
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("search");
        let summary = value
            .get("meaning_summary")
            .and_then(Value::as_str)
            .unwrap_or("");
        let urls = urls_from_value(&value);
        let combined = format!(
            "{query}\n{summary}\n{}\n{}",
            urls.join("\n"),
            value.get("results").and_then(Value::as_str).unwrap_or("")
        );
        contacts.push(contact_from_text(
            timestamp, source, &path, query, urls, summary, &combined,
        ));
    }
    Ok(())
}

fn scan_reply_dir(
    dir: &Path,
    start_unix_s: f64,
    end_unix_s: f64,
    contacts: &mut Vec<ArtifactContact>,
    files_scanned: &mut usize,
) -> Result<()> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(timestamp) = timestamp_from_file(&path, "reply_", ".txt") else {
            continue;
        };
        if timestamp < start_unix_s || timestamp > end_unix_s {
            continue;
        }
        *files_scanned += 1;
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let anchor = reply_anchor(&text);
        let summary = reply_summary(&text);
        contacts.push(contact_from_text(
            timestamp,
            "reply",
            &path,
            &anchor,
            Vec::new(),
            &summary,
            &text,
        ));
    }
    Ok(())
}

fn finalize_scan(
    mut summary: ArtifactScanSummary,
    mut contacts: Vec<ArtifactContact>,
) -> ArtifactScanSummary {
    contacts.sort_by(|left, right| left.timestamp_unix_s.total_cmp(&right.timestamp_unix_s));
    let mut artifact_counts = BTreeMap::<String, usize>::new();
    let mut paths = BTreeSet::<String>::new();
    let mut urls = BTreeSet::<String>::new();
    for contact in &contacts {
        if contact.lambda_tail_signal {
            summary.lambda_tail_hits += 1;
        }
        if contact.lambda_edge_signal {
            summary.lambda_edge_hits += 1;
        }
        if contact.biology_signal {
            summary.biology_hits += 1;
        }
        if contact.return_signal {
            summary.return_hits += 1;
        }
        if contact.perturb_signal {
            summary.perturb_hits += 1;
        }
        if contact.guardrail_signal {
            summary.guardrail_hits += 1;
        }
        if contact.off_target_drift {
            summary.off_target_drift_count += 1;
        }
        let key = contact
            .urls
            .first()
            .cloned()
            .unwrap_or_else(|| contact.anchor.to_ascii_lowercase());
        *artifact_counts.entry(key).or_default() += 1;
        paths.insert(contact.path.clone());
        for url in &contact.urls {
            urls.insert(url.clone());
        }
    }
    summary.repeated_artifact = artifact_counts.values().any(|count| *count >= 2);
    summary.bridge_signal = summary.lambda_tail_hits > 0 && summary.return_hits > 0;
    summary.artifact_grounding_score = artifact_grounding_score(
        summary.lambda_tail_hits,
        summary.biology_hits,
        summary.return_hits,
        summary.repeated_artifact,
        urls.len(),
    );
    summary.return_signal_score = (summary.return_hits as f32 / 2.0).clamp(0.0, 1.0);
    summary.local_paths = paths.into_iter().collect();
    summary.urls = urls.into_iter().collect();
    summary.contacts = contacts;
    summary
}

fn contact_from_text(
    timestamp_unix_s: f64,
    kind: &str,
    path: &Path,
    anchor: &str,
    urls: Vec<String>,
    summary: &str,
    combined: &str,
) -> ArtifactContact {
    let lower = combined.to_ascii_lowercase();
    let anchor_lower = anchor.to_ascii_lowercase();
    let url_summary_lower = format!("{}\n{}", urls.join("\n"), summary).to_ascii_lowercase();
    let lambda_tail_signal = contains_any(&lower, BIOLOGY_TERMS) || lower.contains("lambda4");
    let lambda_edge_signal = contains_any(&lower, LAMBDA_EDGE_TERMS);
    let biology_signal = contains_any(&lower, BIOLOGY_TERMS);
    let return_signal = contains_any(&lower, RETURN_TERMS);
    let perturb_signal = contains_any(&lower, PERTURB_TERMS);
    let guardrail_signal = contains_any(&lower, GUARDRAIL_TERMS);
    let query_is_edge_or_return =
        contains_any(&anchor_lower, LAMBDA_EDGE_TERMS) || contains_any(&anchor_lower, RETURN_TERMS);
    let result_has_edge_or_return = contains_any(&url_summary_lower, LAMBDA_EDGE_TERMS)
        || contains_any(&url_summary_lower, RETURN_TERMS)
        || contains_any(&url_summary_lower, BIOLOGY_TERMS);
    let off_target_drift =
        query_is_edge_or_return && !urls.is_empty() && !result_has_edge_or_return;
    ArtifactContact {
        timestamp_unix_s,
        kind: kind.to_string(),
        path: path.display().to_string(),
        anchor: compact_text(anchor, 120),
        urls,
        lambda_tail_signal,
        lambda_edge_signal,
        biology_signal,
        return_signal,
        perturb_signal,
        guardrail_signal,
        off_target_drift,
        summary: compact_text(summary, 240),
    }
}

fn artifact_grounding_score(
    lambda_tail_hits: usize,
    biology_hits: usize,
    return_hits: usize,
    repeated_artifact: bool,
    url_count: usize,
) -> f32 {
    let hit_score = (lambda_tail_hits as f32 / 4.0).clamp(0.0, 0.35);
    let biology_score = (biology_hits as f32 / 2.0).clamp(0.0, 0.20);
    let return_score = (return_hits as f32 / 2.0).clamp(0.0, 0.20);
    let repeat_score = if repeated_artifact { 0.20 } else { 0.0 };
    let url_score = (url_count as f32 / 5.0).clamp(0.0, 0.05);
    (hit_score + biology_score + return_score + repeat_score + url_score).clamp(0.0, 1.0)
}

fn returnability_score(
    fill_pct: f32,
    lambda1_share: Option<f32>,
    entropy: Option<f32>,
    state: LambdaTailState,
    safety: SafetyLevel,
    return_signal_score: f32,
) -> f32 {
    if safety == SafetyLevel::Red || state == LambdaTailState::Overcollapsed {
        return 0.10;
    }
    let fill_pressure = (fill_pct / 100.0).clamp(0.0, 1.0);
    let dominance = lambda1_share.unwrap_or(0.35).clamp(0.0, 1.0);
    let entropy_loss = 1.0 - entropy.unwrap_or(0.70).clamp(0.0, 1.0);
    let state_bonus = match state {
        LambdaTailState::Diffuse => 0.18,
        LambdaTailState::Returning => 0.22,
        LambdaTailState::ChannelOpen => 0.12,
        LambdaTailState::Probing => 0.08,
        LambdaTailState::Bound => 0.02,
        LambdaTailState::Centralizing | LambdaTailState::Overcollapsed => 0.0,
    };
    (1.0 - fill_pressure * 0.35 - dominance * 0.30 - entropy_loss * 0.20
        + return_signal_score * 0.15
        + state_bonus)
        .clamp(0.0, 1.0)
}

fn contribution_share(lambda_profile: Option<&LambdaProfile>, index: usize) -> Option<f32> {
    lambda_profile.and_then(|profile| {
        profile
            .contributions
            .iter()
            .find(|contribution| contribution.index == index)
            .map(|contribution| contribution.share)
    })
}

fn tail_share_from_profile(lambda_profile: Option<&LambdaProfile>) -> Option<f32> {
    lambda_profile.map(|profile| {
        profile
            .contributions
            .iter()
            .filter(|contribution| contribution.index >= 4)
            .map(|contribution| contribution.share)
            .sum::<f32>()
    })
}

fn artifact_refs(artifact_scan: Option<&ArtifactScanSummary>) -> (Vec<String>, Vec<String>) {
    artifact_scan.map_or_else(
        || (Vec::new(), Vec::new()),
        |scan| {
            (
                scan.local_paths.iter().take(12).cloned().collect(),
                scan.urls.iter().take(12).cloned().collect(),
            )
        },
    )
}

fn format_lambda_tail_read(
    state: LambdaTailState,
    returnability_score: f32,
    artifact_grounding_score: f32,
) -> String {
    let posture = match state {
        LambdaTailState::Diffuse => "tail remains broadly distributed",
        LambdaTailState::Probing => "tail is probing without a decisive binding event",
        LambdaTailState::Centralizing => "tail is narrowing around a stronger lambda edge",
        LambdaTailState::Bound => "tail appears bound to a repeated artifact or search target",
        LambdaTailState::ChannelOpen => {
            "artifact binding is opening a return path into spectral-reservoir language"
        },
        LambdaTailState::Overcollapsed => "tail is overcollapsed or safety pressure is high",
        LambdaTailState::Returning => {
            "tail is returning from artifact contact toward reservoir terms"
        },
    };
    format!(
        "{posture}; returnability={returnability_score:.2}; artifact_grounding={artifact_grounding_score:.2}"
    )
}

fn push_if(signals: &mut Vec<String>, condition: bool, signal: &str) {
    if condition {
        signals.push(signal.to_string());
    }
}

fn urls_from_value(value: &Value) -> Vec<String> {
    let mut urls = BTreeSet::new();
    if let Some(items) = value.get("urls").and_then(Value::as_array) {
        for item in items {
            if let Some(url) = item.as_str() {
                urls.insert(url.to_string());
            }
        }
    }
    if let Some(hits) = value.get("hits").and_then(Value::as_array) {
        for hit in hits {
            if let Some(url) = hit.get("url").and_then(Value::as_str) {
                urls.insert(url.to_string());
            }
        }
    }
    urls.into_iter().collect()
}

fn contains_any(haystack_lower: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack_lower.contains(needle))
}

fn timestamp_from_file(path: &Path, prefix: &str, suffix: &str) -> Option<f64> {
    let name = path.file_name()?.to_string_lossy();
    let raw = name.strip_prefix(prefix)?.strip_suffix(suffix)?;
    parse_dash_timestamp(raw)
}

fn parse_dash_timestamp(raw: &str) -> Option<f64> {
    let (date, time) = raw.split_once('T')?;
    let normalized = format!("{date}T{}", time.replace('-', ":"));
    let parsed = NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%dT%H:%M:%S").ok()?;
    Some(parsed.and_utc().timestamp() as f64)
}

fn unix_to_iso(value: f64) -> String {
    chrono::DateTime::from_timestamp(value.floor() as i64, 0).map_or_else(
        || "unknown-time".to_string(),
        |datetime| datetime.format("%Y-%m-%d %H:%M:%S").to_string(),
    )
}

fn unix_to_file_date(value: f64) -> String {
    chrono::DateTime::from_timestamp(value.floor() as i64, 0).map_or_else(
        || Utc::now().format("%Y_%m_%d").to_string(),
        |datetime| datetime.format("%Y_%m_%d").to_string(),
    )
}

fn unix_to_file_timestamp(value: f64) -> String {
    chrono::DateTime::from_timestamp(value.floor() as i64, 0).map_or_else(
        || Utc::now().format("%Y%m%dT%H%M%SZ").to_string(),
        |datetime| datetime.format("%Y%m%dT%H%M%SZ").to_string(),
    )
}

fn sanitize_slug(slug: &str) -> String {
    let mut out = String::new();
    for ch in slug.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
        } else if matches!(ch, '-' | '_' | ' ') && !out.ends_with('_') {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "LAMBDA_TAIL_DETOUR".to_string()
    } else {
        trimmed
    }
}

fn compact_text(text: &str, max_len: usize) -> String {
    let trimmed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if trimmed.len() <= max_len {
        return trimmed;
    }
    let cut = trimmed
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= max_len)
        .last()
        .unwrap_or(max_len);
    format!("{}...", trimmed[..cut].trim_end())
}

fn reply_anchor(text: &str) -> String {
    text.lines()
        .find(|line| line.contains("NEXT:") || line.contains("Core Experiment"))
        .map_or_else(
            || "delivered reply".to_string(),
            |line| compact_text(line, 120),
        )
}

fn reply_summary(text: &str) -> String {
    let lower_lines = text
        .lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            contains_any(&lower, BIOLOGY_TERMS)
                || contains_any(&lower, RETURN_TERMS)
                || contains_any(&lower, LAMBDA_EDGE_TERMS)
                || contains_any(&lower, PERTURB_TERMS)
                || contains_any(&lower, GUARDRAIL_TERMS)
        })
        .take(3)
        .map(|line| compact_text(line, 120))
        .collect::<Vec<_>>();
    if lower_lines.is_empty() {
        "Delivered reply in the scanned window.".to_string()
    } else {
        lower_lines.join(" ")
    }
}

fn markdown_url_lines(urls: &[String]) -> String {
    urls.iter()
        .take(24)
        .map(|url| format!("- [{url}]({url})"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn markdown_path_lines(paths: &[String]) -> String {
    paths
        .iter()
        .take(32)
        .map(|path| format!("- `{path}`"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn topology_html(
    events: &[LambdaTailTelemetryV1],
    scan: &ArtifactScanSummary,
    payload: &Value,
) -> Result<String> {
    let bars = events
        .iter()
        .map(|event| {
            let width = (event.returnability_score * 100.0).clamp(4.0, 100.0);
            format!(
                "<div class=\"band state-{}\" title=\"{}\"><span>{}</span><i style=\"width:{width:.1}%\"></i></div>",
                event.state.as_str(),
                escape_html(&event.read),
                escape_html(event.state.as_str())
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let contacts = scan
        .contacts
        .iter()
        .take(40)
        .map(|contact| {
            let url = contact.urls.first().map_or(String::new(), |url| {
                format!(" <a href=\"{}\">url</a>", escape_html(url))
            });
            format!(
                "<li><strong>{}</strong> {} <code>{}</code>{}</li>",
                escape_html(&contact.kind),
                escape_html(&contact.anchor),
                escape_html(&contact.path),
                url
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let data = serde_json::to_string_pretty(payload)?;
    Ok(format!(
        "<!doctype html>\n\
         <html lang=\"en\">\n\
         <head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>Lambda-Tail Topology</title>\n\
         <style>\n\
         body {{ margin:0; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; background:#101418; color:#e8edf2; }}\n\
         main {{ max-width:1100px; margin:0 auto; padding:28px; }}\n\
         h1 {{ font-size:26px; margin:0 0 8px; }}\n\
         h2 {{ font-size:16px; margin-top:28px; color:#a8d5ff; }}\n\
         .summary {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(190px,1fr)); gap:10px; margin:20px 0; }}\n\
         .metric {{ border:1px solid #2c3844; border-radius:6px; padding:12px; background:#151b21; }}\n\
         .metric b {{ display:block; font-size:22px; margin-top:4px; }}\n\
         .timeline {{ display:grid; gap:8px; }}\n\
         .band {{ position:relative; height:34px; border:1px solid #2c3844; border-radius:6px; overflow:hidden; background:#151b21; }}\n\
         .band span {{ position:absolute; z-index:2; left:10px; top:8px; }}\n\
         .band i {{ display:block; height:100%; opacity:.9; }}\n\
         .state-diffuse i {{ background:#45b39d; }} .state-probing i {{ background:#f4d35e; }}\n\
         .state-centralizing i {{ background:#f59e0b; }} .state-bound i {{ background:#e76f51; }}\n\
         .state-channel_open i {{ background:#8ecae6; }} .state-overcollapsed i {{ background:#ef4444; }}\n\
         .state-returning i {{ background:#90be6d; }}\n\
         li {{ margin:8px 0; line-height:1.45; }} code, pre {{ background:#0b0f13; border:1px solid #2c3844; border-radius:5px; }}\n\
         code {{ padding:1px 4px; }} pre {{ overflow:auto; padding:12px; max-height:420px; }}\n\
         a {{ color:#8ecae6; }}\n\
         </style>\n\
         </head>\n\
         <body><main>\n\
         <h1>Lambda-Tail Topology</h1>\n\
         <p>Static bridge artifact from recent lambda-tail telemetry and local artifact contact.</p>\n\
         <section class=\"summary\">\n\
         <div class=\"metric\">States<b>{}</b></div>\n\
         <div class=\"metric\">Contacts<b>{}</b></div>\n\
         <div class=\"metric\">Artifact grounding<b>{:.2}</b></div>\n\
         <div class=\"metric\">Return signal<b>{:.2}</b></div>\n\
         </section>\n\
         <h2>State Bands</h2><div class=\"timeline\">{bars}</div>\n\
         <h2>Artifact Contacts</h2><ul>{}</ul>\n\
         <h2>Raw Data</h2><pre>{}</pre>\n\
         </main></body></html>\n",
        events.len(),
        scan.contacts.len(),
        scan.artifact_grounding_score,
        scan.return_signal_score,
        if contacts.is_empty() {
            "<li>No contacts in window.</li>".to_string()
        } else {
            contacts
        },
        escape_html(&data)
    ))
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        LambdaContribution, LambdaProfile, PullModeRate, PullTopologyProfile, SafetyLevel,
        SpectralTelemetry,
    };

    fn telemetry(fill_ratio: f32) -> SpectralTelemetry {
        SpectralTelemetry {
            t_ms: 1,
            eigenvalues: vec![6.0, 2.0, 1.0, 0.5],
            fill_ratio,
            active_mode_count: None,
            active_mode_energy_ratio: None,
            lambda1_rel: None,
            modalities: None,
            neural: None,
            alert: None,
            spectral_fingerprint: None,
            spectral_fingerprint_v1: None,
            spectral_denominator_v1: None,
            effective_dimensionality: None,
            distinguishability_loss: None,
            esn_leak: None,
            esn_leak_override_v1: None,
            structural_entropy: None,
            resonance_density_v1: None,
            pressure_source_v1: None,
            inhabitable_fluctuation_v1: None,
            spectral_glimpse_12d: None,
            eigenvector_field: None,
            stable_core: None,
            semantic: None,
            semantic_energy_v1: None,
            transition_event: None,
            transition_event_v1: None,
            selected_memory_id: None,
            selected_memory_role: None,
            ising_shadow: None,
            shadow_field_v2: None,
            shadow_field_v3: None,
            shadow_influence_response_v3: None,
            residual_deformation_trace_v1: None,
        }
    }

    fn profile(lambda1_share: f32, entropy: f32) -> LambdaProfile {
        LambdaProfile {
            total_energy: 1.0,
            normalized_entropy: entropy,
            lambda1_share,
            lambda1_to_lambda2: None,
            lambda2_to_lambda3: None,
            effective_modes_90: 4,
            skew_read: "test".to_string(),
            contributions: vec![
                LambdaContribution {
                    index: 1,
                    value: lambda1_share,
                    share: lambda1_share,
                    cumulative_share: lambda1_share,
                    ratio_to_next: None,
                    outlier: false,
                },
                LambdaContribution {
                    index: 4,
                    value: 0.20,
                    share: 0.20,
                    cumulative_share: 0.80,
                    ratio_to_next: None,
                    outlier: false,
                },
            ],
        }
    }

    fn topology(
        classification: &str,
        effective_modes: f32,
        largest_gap: f32,
    ) -> PullTopologyProfile {
        PullTopologyProfile {
            classification: classification.to_string(),
            topology_index: 0.5,
            entropy_deficit: 0.2,
            effective_modes,
            lambda1_share: 0.4,
            shoulder_share: 0.3,
            tail_share: 0.2,
            largest_gap_from: 1,
            largest_gap,
            rate_available: false,
            core_rate: 0.0,
            shoulder_rate: 0.0,
            tail_rate: 0.0,
            read: "test".to_string(),
            mode_rates: vec![PullModeRate {
                index: 1,
                share: 0.4,
                log_rate: None,
                weighted_rate: None,
            }],
        }
    }

    fn scan(
        grounding: f32,
        return_score: f32,
        repeated: bool,
        bridge: bool,
    ) -> ArtifactScanSummary {
        ArtifactScanSummary {
            artifact_grounding_score: grounding,
            return_signal_score: return_score,
            repeated_artifact: repeated,
            bridge_signal: bridge,
            lambda_tail_hits: 2,
            biology_hits: 1,
            return_hits: usize::from(return_score > 0.0),
            ..ArtifactScanSummary::empty(0.0, 1.0)
        }
    }

    #[test]
    fn classifier_covers_diffuse_and_centralizing() {
        let diffuse = classify_lambda_tail(
            &telemetry(0.50),
            Some(&profile(0.30, 0.86)),
            Some(&topology("distributed_flow", 5.5, 1.1)),
            None,
            None,
            SafetyLevel::Green,
            1.0,
        );
        assert_eq!(diffuse.state, LambdaTailState::Diffuse);

        let centralizing = classify_lambda_tail(
            &telemetry(0.60),
            Some(&profile(0.48, 0.70)),
            Some(&topology("directed_compaction", 3.5, 2.0)),
            None,
            None,
            SafetyLevel::Green,
            1.0,
        );
        assert_eq!(centralizing.state, LambdaTailState::Centralizing);
    }

    #[test]
    fn classifier_covers_bound_channel_open_overcollapsed_and_returning() {
        let base = classify_lambda_tail(
            &telemetry(0.65),
            Some(&profile(0.50, 0.70)),
            Some(&topology("directed_compaction", 3.0, 2.0)),
            None,
            Some(&scan(0.75, 0.0, true, false)),
            SafetyLevel::Green,
            1.0,
        );
        assert_eq!(base.state, LambdaTailState::Bound);

        let channel = classify_lambda_tail(
            &telemetry(0.65),
            Some(&profile(0.50, 0.70)),
            Some(&topology("directed_compaction", 3.0, 2.0)),
            Some(&base),
            Some(&scan(0.75, 0.7, true, true)),
            SafetyLevel::Green,
            2.0,
        );
        assert_eq!(channel.state, LambdaTailState::ChannelOpen);

        let returning = classify_lambda_tail(
            &telemetry(0.60),
            Some(&profile(0.35, 0.78)),
            Some(&topology("mixed_pull", 4.5, 1.2)),
            Some(&channel),
            Some(&scan(0.30, 0.8, false, false)),
            SafetyLevel::Green,
            3.0,
        );
        assert_eq!(returning.state, LambdaTailState::Returning);

        let over = classify_lambda_tail(
            &telemetry(0.95),
            Some(&profile(0.62, 0.50)),
            Some(&topology("collapsing_pull", 2.0, 3.0)),
            Some(&returning),
            None,
            SafetyLevel::Red,
            4.0,
        );
        assert_eq!(over.state, LambdaTailState::Overcollapsed);
    }

    #[test]
    fn classifier_marks_probing_when_only_artifact_is_active() {
        let event = classify_lambda_tail(
            &telemetry(0.50),
            Some(&profile(0.35, 0.70)),
            Some(&topology("mixed_pull", 4.0, 1.2)),
            None,
            Some(&scan(0.30, 0.0, false, false)),
            SafetyLevel::Green,
            1.0,
        );
        assert_eq!(event.state, LambdaTailState::Probing);
    }

    #[test]
    fn artifact_scan_detects_may_22_style_biology_detour() {
        let root = tempfile::tempdir().unwrap();
        let research = root.path().join("research");
        let replies = root.path().join("outbox/delivered");
        fs::create_dir_all(&research).unwrap();
        fs::create_dir_all(&replies).unwrap();
        fs::write(
            research.join("search_2026-05-22T04-12-32.json"),
            serde_json::json!({
                "timestamp": "2026-05-22T04-12-32",
                "query": "BROWSE: https://www.nature.com/articles/s41467-024-48686-3",
                "source": "browse",
                "urls": ["https://www.nature.com/articles/s41467-024-48686-3"],
                "meaning_summary": "lambda tail-LamB conformational changes",
                "results": "LamB 9E7M EMD cryo-EM bacteriophage tail tip"
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            replies.join("reply_2026-05-22T05-08-55.txt"),
            "Core Experiment: tracing a lambda-tail spectral drift. SEARCH reservoir computing spectral radius. artifact_grounding recurrence_pattern",
        )
        .unwrap();
        let start = parse_dash_timestamp("2026-05-22T04-00-00").unwrap();
        let end = parse_dash_timestamp("2026-05-22T05-10-00").unwrap();
        let scan = scan_artifacts(root.path(), start, end).unwrap();
        assert_eq!(scan.lambda_tail_hits, 2);
        assert!(scan.lambda_edge_hits >= 1);
        assert!(scan.biology_hits >= 1);
        assert!(scan.return_hits >= 1);
        assert!(scan.bridge_signal);
        assert!(scan.artifact_grounding_score >= 0.70);
    }

    #[test]
    fn artifact_scan_detects_lambda_edge_and_off_target_drift() {
        let root = tempfile::tempdir().unwrap();
        let research = root.path().join("research");
        fs::create_dir_all(&research).unwrap();
        fs::write(
            research.join("search_2026-05-22T11-31-56.json"),
            serde_json::json!({
                "timestamp": "2026-05-22T11-31-56",
                "query": "reservoir computing lambda-edge spectral radius",
                "source": "browse",
                "urls": ["https://www.tdk.com/en/news_center/press/20251002_01.html"],
                "meaning_summary": "TDK press release about electronic components",
                "results": "product announcement"
            })
            .to_string(),
        )
        .unwrap();
        let start = parse_dash_timestamp("2026-05-22T11-30-00").unwrap();
        let end = parse_dash_timestamp("2026-05-22T11-33-00").unwrap();
        let scan = scan_artifacts(root.path(), start, end).unwrap();
        assert_eq!(scan.lambda_edge_hits, 1);
        assert_eq!(scan.return_hits, 1);
        assert_eq!(scan.off_target_drift_count, 1);
        assert!(scan.contacts[0].off_target_drift);
    }

    #[test]
    fn steward_note_and_topology_artifact_render() {
        let root = tempfile::tempdir().unwrap();
        let event = classify_lambda_tail(
            &telemetry(0.50),
            Some(&profile(0.30, 0.86)),
            Some(&topology("distributed_flow", 5.5, 1.1)),
            None,
            None,
            SafetyLevel::Green,
            1.0,
        );
        let scan = ArtifactScanSummary {
            local_paths: vec!["/tmp/search.json".to_string()],
            urls: vec!["https://www.rcsb.org/structure/9E7M".to_string()],
            contacts: vec![ArtifactContact {
                timestamp_unix_s: 1.0,
                kind: "browse".to_string(),
                path: "/tmp/search.json".to_string(),
                anchor: "BROWSE 9E7M".to_string(),
                urls: vec!["https://www.rcsb.org/structure/9E7M".to_string()],
                lambda_tail_signal: true,
                lambda_edge_signal: false,
                biology_signal: true,
                return_signal: false,
                perturb_signal: false,
                guardrail_signal: false,
                off_target_drift: false,
                summary: "RCSB 9E7M".to_string(),
            }],
            ..ArtifactScanSummary::empty(0.0, 2.0)
        };
        let md = steward_note_markdown("Test Note", 0.0, 2.0, std::slice::from_ref(&event), &scan);
        assert!(md.contains("# Test Note"));
        assert!(md.contains("https://www.rcsb.org/structure/9E7M"));
        assert!(md.contains("/tmp/search.json"));

        let note_dir = root.path().join("notes");
        let first = write_steward_note(&note_dir, "Test Note", "lambda tail", 2.0, &md).unwrap();
        let second = write_steward_note(&note_dir, "Test Note", "lambda tail", 2.0, &md).unwrap();
        assert_ne!(first, second);
        assert!(first.exists());
        assert!(second.exists());

        let artifact = render_topology_artifact(root.path(), &[event], &scan).unwrap();
        assert!(artifact.html_path.exists());
        assert!(artifact.json_path.exists());
        let html = fs::read_to_string(artifact.html_path).unwrap();
        assert!(html.contains("Lambda-Tail Topology"));
    }
}
