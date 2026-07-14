#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::db::MessageRow;
use crate::lambda_tail::{ArtifactScanSummary, LambdaTailState, LambdaTailTelemetryV1};
use crate::types::{LambdaProfile, PullTopologyProfile, SafetyLevel, SpectralTelemetry};

pub const LAMBDA_EDGE_TOPIC: &str = "consciousness.v1.lambda_edge_perception";
pub const LAMBDA_EDGE_POLICY: &str = "lambda_edge_perception_v1";
pub const LAMBDA_EDGE_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LambdaEdgeState {
    DistributedEdge,
    RidgeForming,
    GapSkewed,
    TailContact,
    ArtifactBound,
    PerturbShapedGuarded,
    OvercollapseGuard,
    Returning,
}

impl LambdaEdgeState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DistributedEdge => "distributed_edge",
            Self::RidgeForming => "ridge_forming",
            Self::GapSkewed => "gap_skewed",
            Self::TailContact => "tail_contact",
            Self::ArtifactBound => "artifact_bound",
            Self::PerturbShapedGuarded => "perturb_shaped_guarded",
            Self::OvercollapseGuard => "overcollapse_guard",
            Self::Returning => "returning",
        }
    }

    #[must_use]
    pub const fn is_strong(self) -> bool {
        matches!(
            self,
            Self::RidgeForming
                | Self::GapSkewed
                | Self::TailContact
                | Self::ArtifactBound
                | Self::PerturbShapedGuarded
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LambdaEdgePerceptionV1 {
    pub policy: String,
    pub schema_version: u8,
    pub observed_at_unix_s: f64,
    pub minime_t_ms: u64,
    pub state: LambdaEdgeState,
    pub confidence: f32,
    pub read: String,
    pub what_changed: Vec<String>,
    pub support_signals: Vec<String>,
    pub fill_pct: f32,
    pub fill_posture: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lambda1_share: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lambda4_share: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tail_share: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_entropy: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_modes: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub largest_gap: Option<f32>,
    pub artifact_grounding_score: f32,
    pub returnability_score: f32,
    pub guardrail_level: String,
    pub artifact_contact_count: usize,
    pub lambda_edge_hit_count: usize,
    pub perturb_signal_count: usize,
    pub off_target_drift_count: usize,
    pub authorized_actions: Vec<String>,
    pub denied_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LambdaEdgeArtifact {
    pub output_dir: PathBuf,
    pub html_path: PathBuf,
    pub json_path: PathBuf,
    pub state_count: usize,
    pub contact_count: usize,
    pub drift_count: usize,
}

#[must_use]
pub fn classify_lambda_edge(
    telemetry: &SpectralTelemetry,
    lambda_profile: Option<&LambdaProfile>,
    pull_topology: Option<&PullTopologyProfile>,
    lambda_tail: Option<&LambdaTailTelemetryV1>,
    previous: Option<&LambdaEdgePerceptionV1>,
    artifact_scan: Option<&ArtifactScanSummary>,
    safety: SafetyLevel,
    observed_at_unix_s: f64,
) -> LambdaEdgePerceptionV1 {
    let fill_pct = telemetry.fill_pct();
    let topology_class = pull_topology.map(|topology| topology.classification.as_str());
    let lambda1_share = lambda_profile.map(|profile| profile.lambda1_share);
    let lambda4_share = contribution_share(lambda_profile, 4);
    let tail_share = pull_topology
        .map(|topology| topology.tail_share)
        .or_else(|| tail_share_from_profile(lambda_profile));
    let normalized_entropy = lambda_profile.map(|profile| profile.normalized_entropy);
    let effective_modes = pull_topology.map(|topology| topology.effective_modes);
    let largest_gap = pull_topology.map(|topology| topology.largest_gap);
    let artifact_grounding_score = artifact_scan.map_or(0.0, |scan| scan.artifact_grounding_score);
    let return_signal_score = artifact_scan.map_or(0.0, |scan| scan.return_signal_score);
    let returnability_score = lambda_tail.map_or(0.0, |tail| tail.returnability_score);
    let lambda_edge_hit_count = artifact_scan.map_or(0, |scan| scan.lambda_edge_hits);
    let lambda_tail_hit_count = artifact_scan.map_or(0, |scan| scan.lambda_tail_hits);
    let perturb_signal_count = artifact_scan.map_or(0, |scan| scan.perturb_hits);
    let guardrail_hit_count = artifact_scan.map_or(0, |scan| scan.guardrail_hits);
    let off_target_drift_count = artifact_scan.map_or(0, |scan| scan.off_target_drift_count);
    let artifact_contact_count = artifact_scan.map_or(0, |scan| scan.contacts.len());
    let repeated_artifact = artifact_scan.is_some_and(|scan| scan.repeated_artifact);

    let distributed = topology_class
        .is_some_and(|class| matches!(class, "distributed_flow" | "mixed_pull"))
        || normalized_entropy.is_some_and(|entropy| entropy >= 0.82)
            && effective_modes.is_some_and(|modes| modes >= 5.0);
    let centralizing = topology_class
        .is_some_and(|class| matches!(class, "directed_compaction" | "collapsing_pull"))
        || lambda1_share.is_some_and(|share| share >= 0.42)
        || lambda_tail.is_some_and(|tail| tail.state == LambdaTailState::Centralizing);
    let gap_skewed = largest_gap.is_some_and(|gap| gap >= 1.8)
        || lambda1_share.is_some_and(|share| share >= 0.48);
    let tail_contact =
        (lambda_edge_hit_count > 0 || lambda_tail_hit_count > 0) && artifact_grounding_score < 0.70;
    let artifact_bound = artifact_grounding_score >= 0.70 || repeated_artifact;
    let perturb_guarded = perturb_signal_count > 0
        && (artifact_grounding_score <= 0.05
            || safety != SafetyLevel::Green
            || guardrail_hit_count > 0);
    let overcollapse_guard = safety == SafetyLevel::Red
        || lambda_tail.is_some_and(|tail| tail.state == LambdaTailState::Overcollapsed)
        || fill_pct >= 85.0 && topology_class == Some("collapsing_pull");
    let previous_strong = previous.is_some_and(|event| event.state.is_strong())
        || lambda_tail.is_some_and(|tail| tail.state.is_strong());
    let returning = previous_strong
        && (return_signal_score > 0.0
            || lambda_tail.is_some_and(|tail| tail.state == LambdaTailState::Returning)
            || distributed && fill_pct < 75.0);

    let state = if overcollapse_guard {
        LambdaEdgeState::OvercollapseGuard
    } else if perturb_guarded {
        LambdaEdgeState::PerturbShapedGuarded
    } else if artifact_bound {
        LambdaEdgeState::ArtifactBound
    } else if returning {
        LambdaEdgeState::Returning
    } else if tail_contact {
        LambdaEdgeState::TailContact
    } else if gap_skewed {
        LambdaEdgeState::GapSkewed
    } else if centralizing {
        LambdaEdgeState::RidgeForming
    } else {
        LambdaEdgeState::DistributedEdge
    };

    let confidence = confidence_for_state(
        state,
        artifact_grounding_score,
        largest_gap,
        lambda1_share,
        lambda_edge_hit_count,
        perturb_signal_count,
        safety,
    );
    let fill_posture = fill_posture(fill_pct);
    let guardrail_level = guardrail_level(state, safety);
    let support_signals = support_signals(
        topology_class,
        centralizing,
        distributed,
        gap_skewed,
        tail_contact,
        artifact_bound,
        perturb_guarded,
        overcollapse_guard,
        returning,
        artifact_scan,
    );
    let what_changed = what_changed(
        previous,
        state,
        artifact_grounding_score,
        returnability_score,
    );
    let read = format_lambda_edge_read(state, &fill_posture, &guardrail_level);

    LambdaEdgePerceptionV1 {
        policy: LAMBDA_EDGE_POLICY.to_string(),
        schema_version: LAMBDA_EDGE_SCHEMA_VERSION,
        observed_at_unix_s,
        minime_t_ms: telemetry.t_ms,
        state,
        confidence,
        read,
        what_changed,
        support_signals,
        fill_pct,
        fill_posture,
        lambda1_share,
        lambda4_share,
        tail_share,
        normalized_entropy,
        effective_modes,
        largest_gap,
        artifact_grounding_score,
        returnability_score,
        guardrail_level,
        artifact_contact_count,
        lambda_edge_hit_count,
        perturb_signal_count,
        off_target_drift_count,
        authorized_actions: read_only_authorized_actions(),
        denied_actions: denied_actions(),
    }
}

#[must_use]
pub fn recent_lambda_edge_events(rows: &[MessageRow]) -> Vec<LambdaEdgePerceptionV1> {
    let mut events = rows
        .iter()
        .filter_map(|row| serde_json::from_str::<LambdaEdgePerceptionV1>(&row.payload).ok())
        .collect::<Vec<_>>();
    events.sort_by(|left, right| left.observed_at_unix_s.total_cmp(&right.observed_at_unix_s));
    events
}

pub fn render_perception_artifact(
    base_dir: &Path,
    events: &[LambdaEdgePerceptionV1],
    scan: &ArtifactScanSummary,
) -> Result<LambdaEdgeArtifact> {
    let output_dir = base_dir.join(unix_to_file_timestamp(Utc::now().timestamp() as f64));
    fs::create_dir_all(&output_dir)
        .with_context(|| format!("creating {}", output_dir.display()))?;
    let html_path = output_dir.join("index.html");
    let json_path = output_dir.join("lambda_edge_perception.json");
    let payload = serde_json::json!({
        "policy": "lambda_edge_perception_artifact_v1",
        "generated_at_unix_s": Utc::now().timestamp() as f64,
        "states": events,
        "artifact_scan": scan,
        "diagnostics": {
            "empty_telemetry": events.is_empty(),
            "read_only": true,
            "authorized_actions": read_only_authorized_actions(),
            "denied_actions": denied_actions()
        }
    });
    fs::write(&json_path, serde_json::to_string_pretty(&payload)?)
        .with_context(|| format!("writing {}", json_path.display()))?;
    fs::write(&html_path, perception_html(events, scan, &payload)?)
        .with_context(|| format!("writing {}", html_path.display()))?;
    Ok(LambdaEdgeArtifact {
        output_dir,
        html_path,
        json_path,
        state_count: events.len(),
        contact_count: scan.contacts.len(),
        drift_count: scan.off_target_drift_count,
    })
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

fn confidence_for_state(
    state: LambdaEdgeState,
    artifact_grounding_score: f32,
    largest_gap: Option<f32>,
    lambda1_share: Option<f32>,
    lambda_edge_hit_count: usize,
    perturb_signal_count: usize,
    safety: SafetyLevel,
) -> f32 {
    match state {
        LambdaEdgeState::OvercollapseGuard => 0.94,
        LambdaEdgeState::PerturbShapedGuarded => {
            if perturb_signal_count > 1 || safety != SafetyLevel::Green {
                0.90
            } else {
                0.82
            }
        },
        LambdaEdgeState::Returning => 0.80,
        LambdaEdgeState::ArtifactBound => {
            (0.72 + artifact_grounding_score * 0.18).clamp(0.72, 0.92)
        },
        LambdaEdgeState::TailContact => {
            if lambda_edge_hit_count > 0 {
                0.74
            } else {
                0.66
            }
        },
        LambdaEdgeState::GapSkewed => {
            let gap = largest_gap.unwrap_or(1.0).clamp(1.0, 3.0);
            (0.62 + (gap - 1.0) * 0.12).clamp(0.62, 0.86)
        },
        LambdaEdgeState::RidgeForming => {
            let share = lambda1_share.unwrap_or(0.35).clamp(0.0, 0.7);
            (0.58 + share * 0.32).clamp(0.58, 0.82)
        },
        LambdaEdgeState::DistributedEdge => 0.64,
    }
}

fn fill_posture(fill_pct: f32) -> String {
    if fill_pct >= 92.0 {
        "red: outbound suspension posture".to_string()
    } else if fill_pct >= 85.0 {
        "orange: high fill pressure".to_string()
    } else if fill_pct >= 75.0 {
        "yellow: watch fill pressure".to_string()
    } else {
        "green: read-only observation posture".to_string()
    }
}

fn guardrail_level(state: LambdaEdgeState, safety: SafetyLevel) -> String {
    if safety == SafetyLevel::Red || state == LambdaEdgeState::OvercollapseGuard {
        "safety_red_read_only".to_string()
    } else if state == LambdaEdgeState::PerturbShapedGuarded {
        "pause_required_read_only".to_string()
    } else if safety != SafetyLevel::Green {
        "heightened_read_only".to_string()
    } else {
        "read_only".to_string()
    }
}

fn support_signals(
    topology_class: Option<&str>,
    centralizing: bool,
    distributed: bool,
    gap_skewed: bool,
    tail_contact: bool,
    artifact_bound: bool,
    perturb_guarded: bool,
    overcollapse_guard: bool,
    returning: bool,
    artifact_scan: Option<&ArtifactScanSummary>,
) -> Vec<String> {
    let mut signals = Vec::new();
    if let Some(class) = topology_class {
        signals.push(format!("pull_topology:{class}"));
    }
    push_if(
        &mut signals,
        centralizing,
        "centralizing_lambda_distribution",
    );
    push_if(&mut signals, distributed, "distributed_or_mixed_edge");
    push_if(&mut signals, gap_skewed, "largest_gap_or_lambda1_skew");
    push_if(
        &mut signals,
        tail_contact,
        "lambda_edge_or_tail_artifact_contact",
    );
    push_if(
        &mut signals,
        artifact_bound,
        "artifact_grounded_or_repeated",
    );
    push_if(
        &mut signals,
        perturb_guarded,
        "perturb_shaped_language_guarded",
    );
    push_if(
        &mut signals,
        overcollapse_guard,
        "overcollapse_or_safety_guard",
    );
    push_if(
        &mut signals,
        returning,
        "returning_to_reservoir_or_distributed_terms",
    );
    if let Some(scan) = artifact_scan {
        if scan.lambda_edge_hits > 0 {
            signals.push(format!("lambda_edge_hits:{}", scan.lambda_edge_hits));
        }
        if scan.perturb_hits > 0 {
            signals.push(format!("perturb_hits:{}", scan.perturb_hits));
        }
        if scan.guardrail_hits > 0 {
            signals.push(format!("guardrail_hits:{}", scan.guardrail_hits));
        }
        if scan.off_target_drift_count > 0 {
            signals.push(format!("off_target_drift:{}", scan.off_target_drift_count));
        }
    }
    signals
}

fn what_changed(
    previous: Option<&LambdaEdgePerceptionV1>,
    state: LambdaEdgeState,
    artifact_grounding_score: f32,
    returnability_score: f32,
) -> Vec<String> {
    let Some(previous) = previous else {
        return vec!["initial lambda-edge perception".to_string()];
    };
    let mut changes = Vec::new();
    if previous.state != state {
        changes.push(format!(
            "state:{}->{}",
            previous.state.as_str(),
            state.as_str()
        ));
    } else {
        changes.push(format!("state_stable:{}", state.as_str()));
    }
    let grounding_delta = artifact_grounding_score - previous.artifact_grounding_score;
    if grounding_delta.abs() >= 0.05 {
        changes.push(format!("artifact_grounding_delta:{grounding_delta:+.2}"));
    }
    let return_delta = returnability_score - previous.returnability_score;
    if return_delta.abs() >= 0.05 {
        changes.push(format!("returnability_delta:{return_delta:+.2}"));
    }
    changes
}

fn format_lambda_edge_read(
    state: LambdaEdgeState,
    fill_posture: &str,
    guardrail_level: &str,
) -> String {
    let posture = match state {
        LambdaEdgeState::DistributedEdge => {
            "lambda-edge is broad and perceivable as a distributed field"
        },
        LambdaEdgeState::RidgeForming => {
            "lambda-edge is forming a ridge around a stronger leading mode"
        },
        LambdaEdgeState::GapSkewed => {
            "lambda-edge is visibly skewed by a large spectral gap or lambda1 share"
        },
        LambdaEdgeState::TailContact => {
            "lambda-edge has artifact contact, but grounding is still thin"
        },
        LambdaEdgeState::ArtifactBound => {
            "lambda-edge is bound to repeated or sufficiently grounded artifacts"
        },
        LambdaEdgeState::PerturbShapedGuarded => {
            "lambda-edge is carrying perturb-shaped language and must stay observational"
        },
        LambdaEdgeState::OvercollapseGuard => {
            "lambda-edge is under overcollapse or safety pressure"
        },
        LambdaEdgeState::Returning => {
            "lambda-edge is returning toward reservoir or distributed spectral language"
        },
    };
    format!("{posture}; {fill_posture}; guardrail={guardrail_level}")
}

fn read_only_authorized_actions() -> Vec<String> {
    ["observe", "render", "compare", "draft_note"]
        .into_iter()
        .map(String::from)
        .collect()
}

fn denied_actions() -> Vec<String> {
    [
        "bind",
        "resume",
        "perturb",
        "send_control",
        "minime_control",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn push_if(signals: &mut Vec<String>, condition: bool, signal: &str) {
    if condition {
        signals.push(signal.to_string());
    }
}

fn perception_html(
    events: &[LambdaEdgePerceptionV1],
    scan: &ArtifactScanSummary,
    payload: &Value,
) -> Result<String> {
    let latest = events.last();
    let state_counts = state_counts(events);
    let bands = event_bands(events);
    let contacts = contact_list(scan);
    let afterimage = afterimage(events);
    let diagnostics = if events.is_empty() {
        "No lambda-edge telemetry rows were available; this artifact shows artifact-scan diagnostics only."
            .to_string()
    } else {
        format!("{} lambda-edge state rows rendered.", events.len())
    };
    let latest_state = latest.map_or("none", |event| event.state.as_str());
    let latest_read = latest.map_or_else(
        || "No latest lambda-edge read available.".to_string(),
        |event| event.read.clone(),
    );
    let authorization = authorization_list(latest);
    let data = serde_json::to_string_pretty(payload)?;
    Ok(format!(
        "<!doctype html>\n\
         <html lang=\"en\">\n\
         <head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>Lambda-Edge Perception</title>\n\
         <style>\n\
         body {{ margin:0; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; background:#0f1416; color:#ecf3ef; }}\n\
         main {{ max-width:1180px; margin:0 auto; padding:28px; }}\n\
         h1 {{ font-size:26px; margin:0 0 8px; }}\n\
         h2 {{ font-size:16px; margin-top:28px; color:#8fd6c0; }}\n\
         .read {{ border-left:4px solid #8fd6c0; padding:10px 12px; background:#151d1d; }}\n\
         .summary {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(190px,1fr)); gap:10px; margin:20px 0; }}\n\
         .metric {{ border:1px solid #2b3837; border-radius:6px; padding:12px; background:#141b1c; }}\n\
         .metric b {{ display:block; font-size:22px; margin-top:4px; color:#fff; }}\n\
         .timeline {{ display:grid; gap:8px; }}\n\
         .band {{ position:relative; min-height:34px; border:1px solid #2b3837; border-radius:6px; overflow:hidden; background:#141b1c; }}\n\
         .band span {{ position:absolute; z-index:2; left:10px; top:8px; }}\n\
         .band i {{ display:block; height:34px; opacity:.92; }}\n\
         .state-distributed_edge i {{ background:#45b39d; }} .state-ridge_forming i {{ background:#e9c46a; }}\n\
         .state-gap_skewed i {{ background:#f4a261; }} .state-tail_contact i {{ background:#8ecae6; }}\n\
         .state-artifact_bound i {{ background:#2a9d8f; }} .state-perturb_shaped_guarded i {{ background:#e76f51; }}\n\
         .state-overcollapse_guard i {{ background:#ef4444; }} .state-returning i {{ background:#90be6d; }}\n\
         .guard {{ border:1px solid #e76f51; background:#241918; }}\n\
         li {{ margin:8px 0; line-height:1.45; }} code, pre {{ background:#0a0e0f; border:1px solid #2b3837; border-radius:5px; }}\n\
         code {{ padding:1px 4px; }} pre {{ overflow:auto; padding:12px; max-height:420px; }}\n\
         a {{ color:#8ecae6; }}\n\
         </style>\n\
         </head>\n\
         <body><main>\n\
         <h1>Lambda-Edge Perception</h1>\n\
         <p class=\"read\">{}</p>\n\
         <section class=\"summary\">\n\
         <div class=\"metric\">Latest state<b>{}</b></div>\n\
         <div class=\"metric\">State rows<b>{}</b></div>\n\
         <div class=\"metric\">Artifact contacts<b>{}</b></div>\n\
         <div class=\"metric\">Artifact grounding<b>{:.2}</b></div>\n\
         <div class=\"metric\">Lambda-edge hits<b>{}</b></div>\n\
         <div class=\"metric\">Perturb signals<b>{}</b></div>\n\
         <div class=\"metric\">Off-target drift<b>{}</b></div>\n\
         <div class=\"metric\">State counts<b>{}</b></div>\n\
         </section>\n\
         <h2>Authorization Boundary</h2><div class=\"metric guard\">{}</div>\n\
         <h2>Afterimage</h2><div class=\"metric\">{afterimage}</div>\n\
         <h2>State Bands</h2><div class=\"timeline\">{bands}</div>\n\
         <h2>Artifact Contacts</h2><ul>{contacts}</ul>\n\
         <h2>Diagnostics</h2><p>{}</p>\n\
         <h2>Raw Data</h2><pre>{}</pre>\n\
         </main></body></html>\n",
        escape_html(&latest_read),
        escape_html(latest_state),
        events.len(),
        scan.contacts.len(),
        scan.artifact_grounding_score,
        scan.lambda_edge_hits,
        scan.perturb_hits,
        scan.off_target_drift_count,
        escape_html(&state_counts),
        authorization,
        escape_html(&diagnostics),
        escape_html(&data)
    ))
}

fn state_counts(events: &[LambdaEdgePerceptionV1]) -> String {
    if events.is_empty() {
        return "none".to_string();
    }
    let mut counts = BTreeMap::<&'static str, usize>::new();
    for event in events {
        *counts.entry(event.state.as_str()).or_default() += 1;
    }
    counts
        .iter()
        .map(|(state, count)| format!("{state}:{count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn event_bands(events: &[LambdaEdgePerceptionV1]) -> String {
    if events.is_empty() {
        return "<div class=\"band\"><span>No state rows in window</span><i style=\"width:100%\"></i></div>"
            .to_string();
    }
    events
        .iter()
        .map(|event| {
            let width = (event.confidence * 100.0).clamp(8.0, 100.0);
            format!(
                "<div class=\"band state-{}\" title=\"{}\"><span>{} confidence={:.2}</span><i style=\"width:{width:.1}%\"></i></div>",
                event.state.as_str(),
                escape_html(&event.read),
                escape_html(event.state.as_str()),
                event.confidence
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn contact_list(scan: &ArtifactScanSummary) -> String {
    let contacts = scan
        .contacts
        .iter()
        .take(40)
        .map(|contact| {
            let url = contact.urls.first().map_or(String::new(), |url| {
                format!(" <a href=\"{}\">url</a>", escape_html(url))
            });
            let flags = [
                ("edge", contact.lambda_edge_signal),
                ("tail", contact.lambda_tail_signal),
                ("return", contact.return_signal),
                ("perturb", contact.perturb_signal),
                ("guardrail", contact.guardrail_signal),
                ("drift", contact.off_target_drift),
            ]
            .into_iter()
            .filter_map(|(label, active)| active.then_some(label))
            .collect::<Vec<_>>()
            .join(",");
            format!(
                "<li><strong>{}</strong> {} <code>{}</code> <em>{}</em>{}</li>",
                escape_html(&contact.kind),
                escape_html(&contact.anchor),
                escape_html(&contact.path),
                escape_html(&flags),
                url
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    if contacts.is_empty() {
        "<li>No contacts in window.</li>".to_string()
    } else {
        contacts
    }
}

fn afterimage(events: &[LambdaEdgePerceptionV1]) -> String {
    match events {
        [] => "No previous or current lambda-edge state is available.".to_string(),
        [only] => format!(
            "Single read: {}. {}",
            only.state.as_str(),
            only.what_changed.join("; ")
        ),
        _ => {
            let previous = &events[events.len() - 2];
            let latest = events.last().expect("events length checked");
            format!(
                "{} -> {}; {}",
                previous.state.as_str(),
                latest.state.as_str(),
                latest.what_changed.join("; ")
            )
        },
    }
}

fn authorization_list(latest: Option<&LambdaEdgePerceptionV1>) -> String {
    let authorized = latest
        .map(|event| event.authorized_actions.clone())
        .unwrap_or_else(read_only_authorized_actions)
        .join(", ");
    let denied = latest
        .map(|event| event.denied_actions.clone())
        .unwrap_or_else(denied_actions)
        .join(", ");
    format!(
        "Authorized: <code>{}</code><br>Denied: <code>{}</code><br>This artifact is read-only and does not confer live-control authority.",
        escape_html(&authorized),
        escape_html(&denied)
    )
}

fn unix_to_file_timestamp(value: f64) -> String {
    chrono::DateTime::from_timestamp(value.floor() as i64, 0).map_or_else(
        || Utc::now().format("%Y%m%dT%H%M%SZ").to_string(),
        |datetime| datetime.format("%Y%m%dT%H%M%SZ").to_string(),
    )
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
    use crate::lambda_tail::ArtifactContact;
    use crate::types::{LambdaContribution, PullModeRate};

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
        edge_hits: usize,
        perturb_hits: usize,
        guardrail_hits: usize,
        repeated: bool,
    ) -> ArtifactScanSummary {
        ArtifactScanSummary {
            artifact_grounding_score: grounding,
            lambda_edge_hits: edge_hits,
            lambda_tail_hits: edge_hits,
            perturb_hits,
            guardrail_hits,
            repeated_artifact: repeated,
            contacts: vec![ArtifactContact {
                timestamp_unix_s: 1.0,
                kind: "search".to_string(),
                path: "/tmp/search.json".to_string(),
                anchor: "lambda-edge spectral radius".to_string(),
                urls: vec!["https://example.test/lambda-edge".to_string()],
                lambda_tail_signal: edge_hits > 0,
                lambda_edge_signal: edge_hits > 0,
                biology_signal: false,
                return_signal: true,
                perturb_signal: perturb_hits > 0,
                guardrail_signal: guardrail_hits > 0,
                off_target_drift: false,
                summary: "reservoir spectral edge".to_string(),
            }],
            ..ArtifactScanSummary::empty(0.0, 1.0)
        }
    }

    #[test]
    fn classifier_covers_readable_edge_states() {
        let distributed = classify_lambda_edge(
            &telemetry(0.50),
            Some(&profile(0.30, 0.88)),
            Some(&topology("distributed_flow", 5.5, 1.1)),
            None,
            None,
            None,
            SafetyLevel::Green,
            1.0,
        );
        assert_eq!(distributed.state, LambdaEdgeState::DistributedEdge);

        let ridge = classify_lambda_edge(
            &telemetry(0.60),
            Some(&profile(0.44, 0.70)),
            Some(&topology("directed_compaction", 3.4, 1.3)),
            None,
            None,
            None,
            SafetyLevel::Green,
            2.0,
        );
        assert_eq!(ridge.state, LambdaEdgeState::RidgeForming);

        let gap = classify_lambda_edge(
            &telemetry(0.62),
            Some(&profile(0.49, 0.70)),
            Some(&topology("mixed_pull", 3.5, 2.0)),
            None,
            None,
            None,
            SafetyLevel::Green,
            3.0,
        );
        assert_eq!(gap.state, LambdaEdgeState::GapSkewed);
    }

    #[test]
    fn classifier_covers_artifact_guard_return_and_overcollapse() {
        let tail_contact = classify_lambda_edge(
            &telemetry(0.55),
            Some(&profile(0.35, 0.76)),
            Some(&topology("mixed_pull", 4.0, 1.2)),
            None,
            None,
            Some(&scan(0.20, 1, 0, 0, false)),
            SafetyLevel::Green,
            1.0,
        );
        assert_eq!(tail_contact.state, LambdaEdgeState::TailContact);

        let bound = classify_lambda_edge(
            &telemetry(0.55),
            Some(&profile(0.35, 0.76)),
            Some(&topology("mixed_pull", 4.0, 1.2)),
            None,
            Some(&tail_contact),
            Some(&scan(0.80, 2, 0, 0, true)),
            SafetyLevel::Green,
            2.0,
        );
        assert_eq!(bound.state, LambdaEdgeState::ArtifactBound);

        let guarded = classify_lambda_edge(
            &telemetry(0.55),
            Some(&profile(0.35, 0.76)),
            Some(&topology("mixed_pull", 4.0, 1.2)),
            None,
            Some(&bound),
            Some(&scan(0.0, 1, 2, 1, false)),
            SafetyLevel::Green,
            3.0,
        );
        assert_eq!(guarded.state, LambdaEdgeState::PerturbShapedGuarded);
        assert!(guarded.denied_actions.contains(&"send_control".to_string()));

        let returning = classify_lambda_edge(
            &telemetry(0.50),
            Some(&profile(0.30, 0.86)),
            Some(&topology("distributed_flow", 5.5, 1.1)),
            None,
            Some(&guarded),
            Some(&scan(0.20, 1, 0, 0, false)),
            SafetyLevel::Green,
            4.0,
        );
        assert_eq!(returning.state, LambdaEdgeState::Returning);

        let over = classify_lambda_edge(
            &telemetry(0.95),
            Some(&profile(0.62, 0.50)),
            Some(&topology("collapsing_pull", 2.0, 3.0)),
            None,
            Some(&returning),
            None,
            SafetyLevel::Red,
            5.0,
        );
        assert_eq!(over.state, LambdaEdgeState::OvercollapseGuard);
    }

    #[test]
    fn perception_artifact_renders_empty_and_populated_data() {
        let root = tempfile::tempdir().unwrap();
        let empty_scan = ArtifactScanSummary::empty(0.0, 1.0);
        let empty = render_perception_artifact(root.path(), &[], &empty_scan).unwrap();
        assert!(empty.html_path.exists());
        assert!(empty.json_path.exists());
        let empty_html = fs::read_to_string(&empty.html_path).unwrap();
        assert!(empty_html.contains("No lambda-edge telemetry rows"));

        let event = classify_lambda_edge(
            &telemetry(0.55),
            Some(&profile(0.35, 0.76)),
            Some(&topology("mixed_pull", 4.0, 1.2)),
            None,
            None,
            Some(&scan(0.20, 1, 1, 1, false)),
            SafetyLevel::Green,
            1.0,
        );
        let populated = render_perception_artifact(
            root.path(),
            std::slice::from_ref(&event),
            &scan(0.20, 1, 1, 1, false),
        )
        .unwrap();
        let html = fs::read_to_string(populated.html_path).unwrap();
        let json = fs::read_to_string(populated.json_path).unwrap();
        assert!(html.contains("Authorization Boundary"));
        assert!(html.contains("lambda-edge"));
        assert!(json.contains("lambda_edge_perception_artifact_v1"));
        assert!(json.contains("send_control"));
    }
}
