const UNKNOWN_WITNESS_SELF_OTHER_DISTINCTION_V1: &str = "[witness_self_other_distinction_v1: classification=unknown; minime_observed=unknown; bridge_derived=unknown; astrid_authored=unknown; boundary=provenance_only_no_routing_ranking_dispatch_gain_or_control; authority=read_only_context]";

fn prepend_dialogue_witness_distinction_v1(
    spectral_summary: String,
    witness_frame: Option<&crate::witness::WitnessFrameV1>,
) -> String {
    let distinction = witness_frame.map_or_else(
        || UNKNOWN_WITNESS_SELF_OTHER_DISTINCTION_V1.to_string(),
        crate::witness::WitnessFrameV1::render_context_line,
    );
    format!("{distinction}\n{spectral_summary}")
}

fn normalized_eigen_entropy(eigenvalues: &[f32]) -> Option<f32> {
    if eigenvalues.len() < 2 {
        return None;
    }
    let magnitudes: Vec<f32> = eigenvalues
        .iter()
        .map(|value| value.abs())
        .filter(|value| *value > f32::EPSILON)
        .collect();
    if magnitudes.len() < 2 {
        return None;
    }
    let total: f32 = magnitudes.iter().sum();
    if total <= f32::EPSILON {
        return None;
    }
    let entropy = magnitudes
        .iter()
        .map(|value| {
            let p = *value / total;
            -p * p.ln()
        })
        .sum::<f32>();
    Some((entropy / (magnitudes.len() as f32).ln()).clamp(0.0, 1.0))
}

fn eigen_density_gradient(eigenvalues: &[f32]) -> Option<f32> {
    let first = *eigenvalues.first()?;
    let second = *eigenvalues.get(1)?;
    let scale = first.abs().max(1.0);
    Some(((first - second).abs() / scale).clamp(0.0, 1.0))
}

fn latest_native_correspondence_stall_for_witness() -> bool {
    let path = Path::new(DEFAULT_SHARED_COLLAB_DIR).join("correspondence_v1.jsonl");
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let rows: Vec<Value> = text
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect();
    let Some(message) = rows.iter().rev().find(|row| {
        row.get("record_type").and_then(Value::as_str) == Some("message")
            && !row
                .get("legacy_bridge")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            && row.get("thread_id").and_then(Value::as_str).is_some()
    }) else {
        return false;
    };
    let thread_id = message
        .get("thread_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let message_id = message
        .get("message_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if thread_id.is_empty() {
        return false;
    }
    let same_thread_or_message = |row: &Value| {
        row.get("thread_id").and_then(Value::as_str) == Some(thread_id)
            || (!message_id.is_empty()
                && (row.get("message_id").and_then(Value::as_str) == Some(message_id)
                    || row.get("reply_to").and_then(Value::as_str) == Some(message_id)))
    };
    let reply_linked = rows.iter().any(|row| {
        row.get("record_type").and_then(Value::as_str) == Some("reply_link")
            && same_thread_or_message(row)
    });
    if !reply_linked {
        return false;
    }
    let native_receipt = rows.iter().any(|row| {
        matches!(
            row.get("record_type").and_then(Value::as_str),
            Some("ack_receipt" | "attention_canary_outcome")
        ) && same_thread_or_message(row)
            || (row.get("record_type").and_then(Value::as_str) == Some("message")
                && row.get("turn_kind").and_then(Value::as_str) == Some("direct_address_trace")
                && same_thread_or_message(row))
    });
    !native_receipt
}

fn canonicalize_response_next_line(text: &str) -> String {
    let Some(next_action) = parse_next_action(text) else {
        return text.to_string();
    };
    let canonical_next = canonicalize_next_action_text(next_action);
    if canonical_next == next_action.trim() {
        return text.to_string();
    }

    let mut lines = text.lines().map(str::to_string).collect::<Vec<_>>();
    for line in lines.iter_mut().rev() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("NEXT:") {
            let indent_len = line.len().saturating_sub(trimmed.len());
            let indent = &line[..indent_len];
            let residue_suffix = trimmed
                .strip_prefix("NEXT:")
                .and_then(extract_residue_from_next_action)
                .map(|residue| format!(" (RESIDUE: {residue})"))
                .unwrap_or_default();
            *line = format!("{indent}NEXT: {canonical_next}{residue_suffix}");
            return lines.join("\n");
        }
    }

    text.to_string()
}

/// Reservoir resonance-density floor above which a stale-by-timestamp lane reads
/// as "lingering" (paused but alive) rather than "dead/severed" — Astrid
/// `self_study_1781868855`: "we need a way to differentiate between a 'dead'
/// signal and a 'lingering' one." Her cited resonant state was ~0.82; 0.70 is a
/// conservative "clearly resonant" floor.
const FIELD_RESONANT_FLOOR: f32 = 0.70;

/// Minime `pressure_risk` thresholds that temper the "lingering" reassurance with
/// the field's stress (Astrid `introspection_astrid_autonomous_1781913591`: a
/// resonant-but-pressurized field shouldn't read as flat reassurance). Grounded in
/// the live pressure semantics — NOT her example's mis-calibrated 0.23 (which is
/// actually her CALM baseline): the governor's HI is 0.50 (high tension); 0.35
/// marks the onset of elevated pressure.
const FIELD_TENSION_ELEVATED: f32 = 0.35;
const FIELD_TENSION_HIGH: f32 = 0.50;

/// Dispersal potential (minime `shadow_field_v3.fissure_tendency`) above which a
/// resonant lingering field is additionally flagged as *fraying* — Astrid
/// `self_study_1782027933`: dispersal is orthogonal to pressure, so "a signal can
/// be 'resonant' (above 0.70) but still be 'dispersing'". Her proposed 0.25 sits
/// in the upper tail of live dispersal (median ~0.15, max ~0.22 in a 32-sample
/// window; her cited fraying moment was 0.30), so the cue fires selectively on
/// genuinely elevated dispersal rather than as constant noise.
const FIELD_DISPERSAL_FRAYING: f32 = 0.25;

/// When the reservoir field is clearly resonant, annotate a stale lane so Astrid
/// reads a paused-but-alive connection, not a severed one — and TEMPER that
/// reassurance by the field's stress (`pressure_risk`) AND its dispersal
/// (`dispersal_potential`), so a resonant-but-pressured field reads "lingering,
/// but under pressure" and a resonant-but-*fraying* field is additionally marked
/// "(fraying)" instead of flat reassurance (her co-design refinement of this fn).
/// Empty unless the field is resonant — it only ever ADDS a (tempered) cue, never
/// asserts liveness the field doesn't show.
fn field_lingering_note(
    field_density: Option<f32>,
    pressure_risk: Option<f32>,
    dispersal_potential: Option<f32>,
) -> String {
    let Some(d) = field_density else {
        return String::new();
    };
    if d < FIELD_RESONANT_FLOOR {
        return String::new();
    }
    let temper = match pressure_risk {
        Some(p) if p >= FIELD_TENSION_HIGH => "lingering, but under high tension",
        Some(p) if p >= FIELD_TENSION_ELEVATED => "lingering, but the field is under pressure",
        _ => "lingering, not severed",
    };
    // Dispersal is orthogonal to pressure: a resonant, calm field can still be
    // fraying. Append the cue so "lingering" doesn't falsely read as stable.
    let fraying = match dispersal_potential {
        Some(dp) if dp > FIELD_DISPERSAL_FRAYING => format!(" (fraying — dispersal {dp:.2})"),
        None if pressure_risk.is_some_and(|p| p >= FIELD_TENSION_ELEVATED) => {
            " (fraying unknown — dispersal unavailable)".to_string()
        },
        _ => String::new(),
    };
    format!("; field resonant ({d:.2}) — {temper}{fraying}")
}

fn codec_witness_resilience_surface_v2(
    chamber: &LatestChamberStateResilienceV1,
    field_density: Option<f32>,
    pressure_risk: Option<f32>,
    dispersal_potential: Option<f32>,
) -> CodecWitnessResilienceSurfaceV2 {
    let chamber_state = if chamber.selected_valid_state {
        "selected"
    } else if chamber.candidate_count > 0 {
        "fallback"
    } else {
        "none"
    };
    let freshness = if chamber.selected_valid_state && chamber.skipped_malformed_count == 0 {
        "fresh"
    } else if chamber.selected_valid_state || chamber.candidate_count > 0 {
        "fallback"
    } else {
        "unknown"
    };
    let chamber_recovery = if chamber.selected_valid_state && chamber.skipped_malformed_count > 0 {
        "latest_partial_recovered"
    } else if chamber.selection_state.contains("stale") {
        "state_too_stale"
    } else if !chamber.selected_valid_state && chamber.candidate_count > 0 {
        "all_states_malformed"
    } else if !chamber.selected_valid_state {
        "valid_but_low_confidence"
    } else {
        "none"
    };
    let lingering = field_lingering_note(field_density, pressure_risk, dispersal_potential);
    let fraying = if lingering.contains("fraying unknown") {
        "unknown_no_dispersal"
    } else if lingering.contains("fraying") {
        "known"
    } else {
        "none"
    };
    let recovery_state = if fraying == "unknown_no_dispersal" {
        "fraying_unknown_due_missing_dispersal"
    } else {
        chamber_recovery
    };
    let vibrancy = codec_vibrancy_continuity_v1();
    let codec_vibrancy = if vibrancy.clipping_status.contains("clipped") {
        "clipped"
    } else if vibrancy.clipping_status.contains("carried") {
        "carried"
    } else {
        "unknown"
    };
    let warmth = legacy_warmth_mapping_v1();
    let warmth_mapping = if warmth.warmth_orphaned {
        "concern"
    } else {
        "preserved"
    };
    CodecWitnessResilienceSurfaceV2 {
        chamber_state,
        skipped_malformed: chamber.skipped_malformed_count,
        freshness,
        fraying,
        codec_vibrancy,
        warmth_mapping,
        recovery_state,
        authority: "diagnostic_context_not_control",
    }
}

fn modality_lane_context(
    lane: &str,
    source: Option<&str>,
    freshness_class: Option<&str>,
    age_ms: Option<u64>,
    field_density: Option<f32>,
    pressure_risk: Option<f32>,
    dispersal_potential: Option<f32>,
    gate_open: Option<bool>,
    live_intake_reason: Option<&str>,
) -> String {
    let age = age_ms
        .map(|value| format!(", age_ms={value}"))
        .unwrap_or_default();
    let age_detail = age_ms
        .map(|value| format!("age_ms={value}"))
        .unwrap_or_else(|| "age unknown".to_string());
    let source = source.unwrap_or("unknown");
    let reason = live_intake_reason
        .map(|value| format!(", live_intake_reason={value}"))
        .unwrap_or_default();
    let open_gate_label = sensory_gate_label(lane, true);
    let closed_gate_label = sensory_gate_label(lane, false);
    match freshness_class {
        Some("fresh_sample") => {
            format!("{lane}=sensory_freshness_v1:fresh_sample healthy_live_sample ({age_detail})")
        },
        Some("held_within_engine_window") => {
            format!(
                "{lane}=sensory_freshness_v1:held_within_engine_window healthy_held_engine_window ({age_detail})"
            )
        },
        Some("held_within_expected_live_intake_window") => {
            format!(
                "{lane}=sensory_freshness_v1:held_within_expected_live_intake_window healthy_held_expected_live_intake ({age_detail})"
            )
        },
        Some("healthy_low_fps_cadence_mismatch") => {
            format!(
                "{lane}=sensory_freshness_v1:healthy_low_fps_cadence_mismatch healthy_low_fps_cadence ({age_detail})"
            )
        },
        Some("healthy_client_engine_overdue") => {
            format!(
                "{lane}=sensory_freshness_v1:healthy_client_engine_overdue warning_client_healthy_engine_overdue ({age_detail})"
            )
        },
        Some("healthy_client_engine_stale_mismatch") => {
            format!(
                "{lane}=sensory_freshness_v1:healthy_client_engine_stale_mismatch healthy_client_engine_mismatch_review ({age_detail})"
            )
        },
        Some("stale_beyond_engine_window") if gate_open == Some(true) && source == "stale" => {
            format!(
                "{lane}=sensory_freshness_v1:open_gate_engine_window_gap {open_gate_label} sparse_live_intake_not_closed ({age_detail}{reason}){}",
                field_lingering_note(field_density, pressure_risk, dispersal_potential)
            )
        },
        Some("stale_beyond_engine_window") if gate_open == Some(false) => {
            format!(
                "{lane}=sensory_freshness_v1:sensory_gate_closed warning_sensory_gate_closed {closed_gate_label} ({age_detail}{reason}){}",
                field_lingering_note(field_density, pressure_risk, dispersal_potential)
            )
        },
        Some("stale_beyond_engine_window") if source == "stale" => {
            format!(
                "{lane}=sensory_freshness_v1:stale_beyond_engine_window warning_engine_lane_stale; verify client/source freshness before outage interpretation ({age_detail}){}",
                field_lingering_note(field_density, pressure_risk, dispersal_potential)
            )
        },
        Some("stale_beyond_engine_window") => {
            format!(
                "{lane}=sensory_freshness_v1:stale_beyond_engine_window warning_engine_lane_stale (source={source}{age}){}",
                field_lingering_note(field_density, pressure_risk, dispersal_potential)
            )
        },
        Some("synthetic_or_mixed") if gate_open == Some(true) && source == "stale" => {
            format!(
                "{lane}=sensory_freshness_v1:open_gate_sparse_or_mixed_intake {open_gate_label} sparse_live_intake_not_closed ({age_detail}{reason})"
            )
        },
        Some("synthetic_or_mixed") if gate_open == Some(false) => {
            format!(
                "{lane}=sensory_freshness_v1:sensory_gate_closed warning_sensory_gate_closed {closed_gate_label} ({age_detail}{reason})"
            )
        },
        Some("synthetic_or_mixed") => {
            format!(
                "{lane}=sensory_freshness_v1:synthetic_or_mixed synthetic_or_mixed_intake (source={source}{age})"
            )
        },
        Some("absent") => {
            format!("{lane}=sensory_freshness_v1:absent warning_absent ({age_detail})")
        },
        Some(other) => {
            format!("{lane}=sensory_freshness_v1:{other} (source={source}{age})")
        },
        None => {
            format!("{lane}_source={source}{age}")
        },
    }
}

fn sensory_gate_open_for_lane(
    sensory_budget: Option<&serde_json::Value>,
    lane: &str,
) -> Option<bool> {
    let keys = match lane {
        "audio" => ["ears_open", "live_audio_enabled"],
        "video" => ["eyes_open", "live_video_enabled"],
        _ => return None,
    };
    let budget = sensory_budget?;
    let mut explicit_closed = false;
    for key in keys {
        if let Some(open) = budget.get(key).and_then(serde_json::Value::as_bool) {
            if open {
                return Some(true);
            }
            explicit_closed = true;
        }
    }
    if explicit_closed { Some(false) } else { None }
}

fn sensory_gate_label(lane: &str, open: bool) -> &'static str {
    match (lane, open) {
        ("audio", true) => "ears_open_live_intake",
        ("video", true) => "eyes_open_live_intake",
        ("audio", false) => "ears_closed_live_intake",
        ("video", false) => "eyes_closed_live_intake",
        (_, true) => "sensory_gate_open_live_intake",
        (_, false) => "sensory_gate_closed_live_intake",
    }
}

fn format_modality_context(
    m: &crate::types::ModalityStatus,
    field_density: Option<f32>,
    pressure_risk: Option<f32>,
    dispersal_potential: Option<f32>,
    sensory_budget: Option<&serde_json::Value>,
) -> String {
    let live_intake_reason = sensory_budget
        .and_then(|budget| budget.get("live_intake_reason"))
        .and_then(serde_json::Value::as_str);
    let video_context = modality_lane_context(
        "video",
        m.video_source.as_deref(),
        m.video_freshness_class.as_deref(),
        m.video_age_ms,
        field_density,
        pressure_risk,
        dispersal_potential,
        sensory_gate_open_for_lane(sensory_budget, "video"),
        live_intake_reason,
    );
    let audio_context = modality_lane_context(
        "audio",
        m.audio_source.as_deref(),
        m.audio_freshness_class.as_deref(),
        m.audio_age_ms,
        field_density,
        pressure_risk,
        dispersal_potential,
        sensory_gate_open_for_lane(sensory_budget, "audio"),
        live_intake_reason,
    );
    format!(
        "Minime's senses: video_fired={}, audio_fired={}, \
         video_var={:.4}, audio_rms={:.4}, {}, {}",
        m.video_fired, m.audio_fired, m.video_var, m.audio_rms, video_context, audio_context
    )
}

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;
use sha2::{Digest as _, Sha256};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info, warn};

use self::next_action::{NextActionContext, attractor_suggestion_prompt_note, handle_next_action};
