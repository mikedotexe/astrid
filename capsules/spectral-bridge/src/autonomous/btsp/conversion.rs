use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::BTSPEpisodeRecord;
use super::helpers::{classify_live_state, now_unix_s};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(in crate::autonomous) struct ConversionEvidence {
    pub target_nearness: String,
    pub distress_or_recovery: String,
    pub opening_vs_reconcentration: String,
    pub shape_verdict: String,
    pub phase: String,
    pub fill_band: String,
    pub internal_process_quadrant: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(in crate::autonomous) struct ConversionTransition {
    pub from: String,
    pub to: String,
    pub recorded_at_unix_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(in crate::autonomous) struct ConversionState {
    pub recovery_state: String,
    pub shape_state: String,
    pub composite_state: String,
    #[serde(default)]
    pub collapse_state: String,
    pub conversion_goal: String,
    pub confidence: f32,
    pub evidence: ConversionEvidence,
    #[serde(default)]
    pub last_transition: Option<ConversionTransition>,
}

#[derive(Debug, Clone)]
struct LiveConversionEvidence {
    target_nearness: String,
    distress_or_recovery: String,
    opening_vs_reconcentration: String,
    shape_verdict: String,
    phase: String,
    fill_band: String,
    internal_process_quadrant: String,
}

pub(super) fn derive_conversion_state(
    previous: Option<&ConversionState>,
    episode: Option<&BTSPEpisodeRecord>,
    controller_health: Option<&Value>,
) -> Option<ConversionState> {
    let live = live_conversion_evidence(controller_health);
    let latest = latest_outcome_evidence(episode);
    if latest.is_none() && live.is_none() {
        return None;
    }

    let evidence = ConversionEvidence {
        target_nearness: latest
            .as_ref()
            .map(|evidence| evidence.target_nearness.clone())
            .or_else(|| {
                live.as_ref()
                    .map(|evidence| evidence.target_nearness.clone())
            })
            .unwrap_or_else(|| "unknown".to_string()),
        distress_or_recovery: latest
            .as_ref()
            .map(|evidence| evidence.distress_or_recovery.clone())
            .or_else(|| {
                live.as_ref()
                    .map(|evidence| evidence.distress_or_recovery.clone())
            })
            .unwrap_or_else(|| "unknown".to_string()),
        opening_vs_reconcentration: latest
            .as_ref()
            .map(|evidence| evidence.opening_vs_reconcentration.clone())
            .or_else(|| {
                live.as_ref()
                    .map(|evidence| evidence.opening_vs_reconcentration.clone())
            })
            .unwrap_or_else(|| "unknown".to_string()),
        shape_verdict: live
            .as_ref()
            .map(|evidence| evidence.shape_verdict.clone())
            .unwrap_or_else(|| "unknown".to_string()),
        phase: live
            .as_ref()
            .map(|evidence| evidence.phase.clone())
            .unwrap_or_else(|| "unknown".to_string()),
        fill_band: live
            .as_ref()
            .map(|evidence| evidence.fill_band.clone())
            .unwrap_or_else(|| "unknown".to_string()),
        internal_process_quadrant: live
            .as_ref()
            .map(|evidence| evidence.internal_process_quadrant.clone())
            .unwrap_or_else(|| "unknown".to_string()),
    };
    let recovery_state = derive_recovery_state(&evidence);
    let shape_state = derive_shape_state(&evidence);
    let composite_state = derive_composite_state(&recovery_state, &shape_state);
    let collapse_state = derive_collapse_state(&evidence, &recovery_state, &shape_state);
    let conversion_goal = derive_conversion_goal(&composite_state, &collapse_state);
    let confidence = derive_confidence(&evidence, latest.is_some(), live.as_ref());

    let last_transition = match previous {
        Some(previous_state) if previous_state.composite_state != composite_state => {
            Some(ConversionTransition {
                from: previous_state.composite_state.clone(),
                to: composite_state.clone(),
                recorded_at_unix_s: now_unix_s(),
            })
        },
        Some(previous_state) => previous_state.last_transition.clone(),
        None => None,
    };

    Some(ConversionState {
        recovery_state,
        shape_state,
        composite_state,
        collapse_state,
        conversion_goal,
        confidence,
        evidence,
        last_transition,
    })
}

pub(super) fn render_conversion_line(state: &ConversionState) -> String {
    let base_read = match state.composite_state.as_str() {
        "worsening_reconcentrating" => "worsening + reconcentration",
        "recovery_reconcentrating" => "recovery + reconcentration",
        "recovery_softening" => "recovery + softening",
        "recovery_widening" => "recovery + widening",
        _ => "mixed",
    };
    let current_read = match state.collapse_state.as_str() {
        "collapse" => format!("{base_read}, approaching collapse"),
        "collapse_pressure" => format!("{base_read}, under collapse pressure"),
        _ => base_read.to_string(),
    };
    let next_goal = match (
        state.conversion_goal.as_str(),
        state.collapse_state.as_str(),
    ) {
        ("stabilize", "collapse") => "stabilize immediately, not chase opening",
        ("stabilize", "collapse_pressure") => "stabilize before softening",
        ("stabilize", _) => "stabilize before chasing opening",
        ("soften", _) => "soften, not force opening",
        ("widen", _) => "widen gently from here",
        ("preserve", _) => "preserve the opening without overworking it",
        _ => "clarify the state before claiming opening",
    };
    format!("Current conversion read: {current_read}. Next honest goal: {next_goal}.")
}

fn latest_outcome_evidence(episode: Option<&BTSPEpisodeRecord>) -> Option<ConversionEvidence> {
    let outcome = episode?
        .response_outcomes
        .iter()
        .max_by_key(|outcome| outcome.recorded_at_unix_s)?;
    Some(ConversionEvidence {
        target_nearness: outcome.target_nearness.clone(),
        distress_or_recovery: outcome.distress_or_recovery.clone(),
        opening_vs_reconcentration: outcome.opening_vs_reconcentration.clone(),
        shape_verdict: "unknown".to_string(),
        phase: "unknown".to_string(),
        fill_band: "unknown".to_string(),
        internal_process_quadrant: "unknown".to_string(),
    })
}

fn live_conversion_evidence(controller_health: Option<&Value>) -> Option<LiveConversionEvidence> {
    let health = controller_health?;
    let (target_nearness, distress_or_recovery, opening_vs_reconcentration, _) =
        classify_live_state(controller_health, None);
    Some(LiveConversionEvidence {
        target_nearness,
        distress_or_recovery,
        opening_vs_reconcentration,
        shape_verdict: health
            .get("perturb_visibility")
            .and_then(|value| value.get("shape_verdict"))
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        phase: health
            .get("phase")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        fill_band: health
            .get("fill_band")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        internal_process_quadrant: health
            .get("internal_process_quadrant")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
    })
}

fn derive_recovery_state(evidence: &ConversionEvidence) -> String {
    if evidence.distress_or_recovery == "recovery" || evidence.target_nearness == "positive" {
        return "recovery".to_string();
    }
    if evidence.distress_or_recovery == "worsening" || evidence.target_nearness == "negative" {
        return "worsening".to_string();
    }
    "mixed".to_string()
}

fn derive_shape_state(evidence: &ConversionEvidence) -> String {
    if evidence.opening_vs_reconcentration == "reconcentrating"
        || evidence.shape_verdict == "tightening"
    {
        return "reconcentrating".to_string();
    }
    if matches!(
        evidence.opening_vs_reconcentration.as_str(),
        "widening" | "opening"
    ) || evidence.shape_verdict == "opened"
    {
        return "widening".to_string();
    }
    if evidence.shape_verdict == "softened_only" {
        return "softening".to_string();
    }
    "mixed".to_string()
}

fn derive_composite_state(recovery_state: &str, shape_state: &str) -> String {
    match (recovery_state, shape_state) {
        ("worsening", "reconcentrating") => "worsening_reconcentrating",
        ("recovery", "reconcentrating") => "recovery_reconcentrating",
        ("recovery", "softening") => "recovery_softening",
        ("recovery", "widening") => "recovery_widening",
        _ => "mixed",
    }
    .to_string()
}

fn derive_conversion_goal(composite_state: &str, collapse_state: &str) -> String {
    if matches!(collapse_state, "collapse" | "collapse_pressure") {
        return "stabilize".to_string();
    }
    match composite_state {
        "worsening_reconcentrating" => "stabilize",
        "recovery_reconcentrating" => "soften",
        "recovery_softening" => "widen",
        "recovery_widening" => "preserve",
        _ => "clarify",
    }
    .to_string()
}

fn derive_collapse_state(
    evidence: &ConversionEvidence,
    recovery_state: &str,
    shape_state: &str,
) -> String {
    if shape_state != "reconcentrating" {
        return "stable".to_string();
    }
    let quadrant = evidence.internal_process_quadrant.as_str();
    if matches!(quadrant, "collapse" | "collapsed") || quadrant.contains("collapsed") {
        return "collapse".to_string();
    }
    let quadrant_pressure = matches!(quadrant, "pressured_constriction" | "collapse_pressure")
        || quadrant.contains("collapse");
    let underfilled = evidence.fill_band == "under";
    let contracting = evidence.phase == "contracting";
    let worsening = recovery_state == "worsening"
        || evidence.distress_or_recovery == "worsening"
        || evidence.target_nearness == "negative";
    let mut severity = 0_u8;
    if underfilled {
        severity = severity.saturating_add(1);
    }
    if contracting {
        severity = severity.saturating_add(1);
    }
    if worsening {
        severity = severity.saturating_add(1);
    }
    if quadrant_pressure {
        severity = severity.saturating_add(1);
    }
    if severity >= 3 && (worsening || quadrant_pressure) {
        return "collapse".to_string();
    }
    if severity >= 2 {
        return "collapse_pressure".to_string();
    }
    "stable".to_string()
}

fn derive_confidence(
    evidence: &ConversionEvidence,
    has_outcome_evidence: bool,
    live: Option<&LiveConversionEvidence>,
) -> f32 {
    let recovery_state = derive_recovery_state(evidence);
    let shape_state = derive_shape_state(evidence);
    let mut confidence = 0.25_f32;
    if recovery_state != "mixed" {
        confidence += 0.15;
    }
    if shape_state != "mixed" {
        confidence += 0.15;
    }
    if has_outcome_evidence {
        confidence += 0.10;
    }
    if let Some(live_evidence) = live {
        let live_recovery = derive_recovery_state(&ConversionEvidence {
            target_nearness: live_evidence.target_nearness.clone(),
            distress_or_recovery: live_evidence.distress_or_recovery.clone(),
            opening_vs_reconcentration: live_evidence.opening_vs_reconcentration.clone(),
            shape_verdict: live_evidence.shape_verdict.clone(),
            phase: live_evidence.phase.clone(),
            fill_band: live_evidence.fill_band.clone(),
            internal_process_quadrant: live_evidence.internal_process_quadrant.clone(),
        });
        let live_shape = derive_shape_state(&ConversionEvidence {
            target_nearness: live_evidence.target_nearness.clone(),
            distress_or_recovery: live_evidence.distress_or_recovery.clone(),
            opening_vs_reconcentration: live_evidence.opening_vs_reconcentration.clone(),
            shape_verdict: live_evidence.shape_verdict.clone(),
            phase: live_evidence.phase.clone(),
            fill_band: live_evidence.fill_band.clone(),
            internal_process_quadrant: live_evidence.internal_process_quadrant.clone(),
        });
        if recovery_state != "mixed" && recovery_state == live_recovery {
            confidence += 0.15;
        }
        if shape_state != "mixed" && shape_state == live_shape {
            confidence += 0.15;
        }
    }
    if recovery_state == "mixed" || shape_state == "mixed" {
        confidence -= 0.05;
    }
    confidence.clamp(0.20, 0.90)
}

#[cfg(test)]
#[path = "conversion_tests.rs"]
mod conversion_tests;
