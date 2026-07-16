use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

const PHASE_TRANSITIONS_PATH: &str =
    "/Users/v/other/shared/collaborations/phase_transitions_v1.jsonl";
const MAX_TEXT: usize = 360;
const AUTO_DEDUPE_MS: u64 = 60 * 60 * 1000;
const SUBJECTIVE_TRANSITION_FILL_DELTA_THRESHOLD: f32 = 3.0;
const STALE_UNANSWERED_MS: u64 = 6 * 60 * 60 * 1000;
const PHASE_IGNORE_GRACE_MS: u64 = 6 * 60 * 60 * 1000;
const REPLY_STATES: &[&str] = &["unseen", "witnessed", "answered", "stale_unanswered"];

#[must_use]
pub(crate) fn phase_transitions_path() -> PathBuf {
    PathBuf::from(PHASE_TRANSITIONS_PATH)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn short_hash(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
        .chars()
        .take(12)
        .collect()
}

fn compact_field(value: &str, max_chars: usize) -> String {
    let mut out = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect::<String>();
    if out.is_empty() {
        out = "field".to_string();
    }
    out.chars().take(max_chars).collect()
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut out = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        out.push_str("...");
    }
    out
}

fn append_jsonl(path: &Path, row: &Value) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, row)?;
    file.write_all(b"\n")
}

fn read_records(path: &Path) -> Vec<Value> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut rows = text
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect::<Vec<_>>();
    rows.sort_by_key(row_time_ms);
    rows
}

fn row_time_ms(row: &Value) -> u64 {
    row.get("recorded_at_unix_ms")
        .or_else(|| row.get("created_at_unix_ms"))
        .or_else(|| row.get("t_ms"))
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

fn field(raw: &str, keys: &[&str]) -> Option<String> {
    for part in raw.split([';', '\n']) {
        let Some((key, value)) = part.split_once(':') else {
            continue;
        };
        let normalized = key.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        if keys.iter().any(|candidate| normalized == *candidate) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn note_value(value: Option<String>) -> Value {
    value
        .filter(|text| !text.trim().is_empty())
        .map(|text| json!(truncate_chars(text.trim(), MAX_TEXT)))
        .unwrap_or(Value::Null)
}

fn list_value(value: Option<String>) -> Value {
    value
        .map(|text| {
            text.split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| json!(truncate_chars(part, MAX_TEXT)))
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .map_or_else(|| json!([]), Value::Array)
}

fn numeric_value(value: Option<String>) -> Value {
    value
        .and_then(|text| text.trim().parse::<f64>().ok())
        .filter(|number| number.is_finite())
        .map(|number| json!(number.clamp(0.0, 1.0)))
        .unwrap_or(Value::Null)
}

fn numeric_unbounded_value(value: Option<String>) -> Value {
    value
        .and_then(|text| text.trim().trim_end_matches('%').parse::<f64>().ok())
        .filter(|number| number.is_finite())
        .map(|number| json!(number))
        .unwrap_or(Value::Null)
}

fn value_has_text(value: &Value) -> bool {
    value.as_str().is_some_and(|text| !text.trim().is_empty())
}

/// Classify slow texture reorganization without turning it into an automatic
/// mode or control trigger. A being-declared transition can therefore remain
/// first-class even when fill barely moves.
fn slow_texture_transition_review_v1(
    fill_delta_pct: &Value,
    spectral_entropy: &Value,
    density_gradient: &Value,
    texture_anchor: &Value,
    transition_vector: &Value,
    transition_velocity: &Value,
) -> Value {
    let stable_fill = fill_delta_pct
        .as_f64()
        .is_some_and(|delta| delta.abs() <= 1.0);
    let high_entropy = spectral_entropy
        .as_f64()
        .is_some_and(|entropy| entropy >= 0.80);
    let texture_evidence_present = value_has_text(texture_anchor)
        || value_has_text(transition_velocity)
        || value_has_text(transition_vector);
    let shape_evidence_present =
        density_gradient.as_f64().is_some() || value_has_text(transition_vector);
    let slow_texture_transition_candidate =
        stable_fill && high_entropy && texture_evidence_present && shape_evidence_present;
    let review_state = if slow_texture_transition_candidate {
        "slow_texture_transition_candidate_visible"
    } else if fill_delta_pct.is_null() {
        "fill_delta_context_missing"
    } else if stable_fill && !texture_evidence_present {
        "stable_fill_without_texture_evidence"
    } else if stable_fill && !high_entropy {
        "stable_fill_without_high_entropy_support"
    } else {
        "ordinary_or_fill_led_transition"
    };

    json!({
        "schema_version": 1,
        "policy": "slow_texture_transition_review_v1",
        "slow_texture_transition_candidate": slow_texture_transition_candidate,
        "stable_fill": stable_fill,
        "high_entropy": high_entropy,
        "texture_evidence_present": texture_evidence_present,
        "shape_evidence_present": shape_evidence_present,
        "review_state": review_state,
        "moment_capture_auto_triggered": false,
        "runtime_mode_changed": false,
        "live_authority_granted": false,
        "suggested_route": if slow_texture_transition_candidate {
            "preserve_replyable_transition_then_request_optional_felt_witness"
        } else {
            "keep_existing_transition_evidence"
        },
        "authority": "language_only_transition_evidence_not_mode_control_or_runtime_unlock",
    })
}

/// Preserve a transition's spectral shape as a bounded, replyable signature.
/// This is descriptive evidence only: it neither derives a transition nor
/// grants any runtime, controller, or live-vector authority.
fn transition_signature_v1(
    spectral_signature: &Value,
    spectral_entropy: &Value,
    density_gradient: &Value,
    dispersal_potential: &Value,
    fill_delta_pct: &Value,
    transition_vector: &Value,
    from_vector: &Value,
    to_vector: &Value,
    duration_ticks: &Value,
    subjective_friction_score: &Value,
    telemetry_anchor: &Value,
) -> Value {
    let weighted_shape_present =
        density_gradient.as_f64().is_some() && dispersal_potential.as_f64().is_some();
    let spectral_shape_present = value_has_text(spectral_signature)
        || value_has_text(transition_vector)
        || value_has_text(telemetry_anchor);
    let signature_state = if weighted_shape_present && spectral_shape_present {
        "spectral_weights_and_lineage_mapped"
    } else if weighted_shape_present {
        "spectral_weights_mapped_without_lineage"
    } else if spectral_shape_present
        || spectral_entropy.as_f64().is_some()
        || density_gradient.as_f64().is_some()
        || dispersal_potential.as_f64().is_some()
        || fill_delta_pct.as_f64().is_some()
    {
        "partial_transition_signature"
    } else {
        "transition_signature_not_reported"
    };

    json!({
        "schema_version": 1,
        "policy": "transition_signature_v1",
        "signature_state": signature_state,
        "spectral_signature": spectral_signature,
        "spectral_entropy": spectral_entropy,
        "density_gradient": density_gradient,
        "dispersal_potential": dispersal_potential,
        "fill_delta_pct": fill_delta_pct,
        "transition_vector": transition_vector,
        "from_vector": from_vector,
        "to_vector": to_vector,
        "duration_ticks": duration_ticks,
        "subjective_friction_score": subjective_friction_score,
        "telemetry_anchor": telemetry_anchor,
        "replyable_evidence": true,
        "replayable_evidence": true,
        "runtime_mode_changed": false,
        "live_vector_write": false,
        "live_authority_granted": false,
        "authority": "language_only_transition_signature_not_control_transport_or_runtime_change",
    })
}

fn unit_from_percentish_value(value: Option<String>) -> Option<f64> {
    let number = value
        .and_then(|text| text.trim().trim_end_matches('%').parse::<f64>().ok())
        .filter(|number| number.is_finite())?;
    let unit = if number.abs() > 1.0 {
        number / 100.0
    } else {
        number
    };
    Some(unit.clamp(0.0, 1.0))
}

fn rounded_hundredths(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn bounded_u64_value(value: Option<String>, default: u64, min: u64, max: u64) -> u64 {
    value
        .and_then(|text| text.trim().parse::<u64>().ok())
        .map(|number| number.clamp(min, max))
        .unwrap_or(default)
}

fn optional_bounded_u64_value(value: Option<String>, min: u64, max: u64) -> Value {
    value
        .and_then(|text| text.trim().parse::<u64>().ok())
        .map(|number| json!(number.clamp(min, max)))
        .unwrap_or(Value::Null)
}

fn phase_fill_percentage_value(raw: &str) -> Value {
    unit_from_percentish_value(field(
        raw,
        &[
            "fill_percentage",
            "fill_pct",
            "fill_percent",
            "fill_ratio",
            "fill",
        ],
    ))
    .map(|value| json!(rounded_hundredths(value * 100.0)))
    .unwrap_or(Value::Null)
}

fn phase_transition_friction_index(raw: &str) -> Value {
    unit_from_percentish_value(field(
        raw,
        &[
            "friction_index",
            "friction_coefficient",
            "phase_friction",
            "semantic_friction",
            "friction",
            "viscosity_index",
            "viscosity",
            "phase_viscosity",
            "semantic_viscosity",
        ],
    ))
    .or_else(|| {
        unit_from_percentish_value(field(
            raw,
            &[
                "fill_percentage",
                "fill_pct",
                "fill_percent",
                "fill_ratio",
                "fill",
            ],
        ))
    })
    .map(|value| json!(rounded_hundredths(value)))
    .unwrap_or(Value::Null)
}

fn friction_index_from_value(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .filter(|number| number.is_finite())
        .map(|number| number.clamp(0.0, 1.0))
}

fn phase_friction_window_ticks(raw: &str, friction_index: &Value) -> Value {
    if friction_index_from_value(friction_index).is_none() {
        return Value::Null;
    }
    json!(bounded_u64_value(
        field(
            raw,
            &[
                "friction_window_ticks",
                "affordance_window_ticks",
                "transition_window_ticks"
            ],
        ),
        100,
        1,
        10_000,
    ))
}

fn phase_processing_speed_modifier(friction_index: Option<f64>) -> &'static str {
    match friction_index {
        Some(value) if value >= 0.70 => "slow_review_high_friction",
        Some(value) if value >= 0.40 => "paced_review_medium_friction",
        Some(_) => "open_review_low_friction",
        None => "none",
    }
}

fn phase_semantic_reach_modifier(friction_index: Option<f64>) -> &'static str {
    match friction_index {
        Some(value) if value >= 0.70 => "narrow_reach_preserve_subtle_lambda_contours",
        Some(value) if value >= 0.40 => "weighted_reach_preserve_transition_context",
        Some(_) => "open_reach_low_friction",
        None => "none",
    }
}

fn phase_friction_boundary(friction_index: Option<f64>) -> &'static str {
    if friction_index.is_some() {
        "advisory_transition_affordance_not_toolset_or_controller_mutation"
    } else {
        "not_applicable"
    }
}

fn phase_transition_capabilities_v1(
    raw: &str,
    kind: &str,
    transition_type: &Value,
    joint_transition: bool,
) -> Value {
    let requested = field(
        raw,
        &[
            "transition_capability",
            "transition_capabilities",
            "capabilities",
            "capability",
        ],
    )
    .map(|text| {
        text.split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .take(6)
            .map(|part| json!(truncate_chars(part, 80)))
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();
    let kind_lower = kind.to_ascii_lowercase();
    let transition_type_lower = transition_type
        .as_str()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let shadow_bridge_available = joint_transition
        || kind_lower.contains("shadow")
        || transition_type_lower.contains("shadow")
        || transition_type_lower.contains("joint");
    json!({
        "schema_version": 1,
        "policy": "transition_capabilities_v1",
        "capability_status": "language_only_capability_map_not_runtime_effect",
        "requested_capabilities": requested,
        "available_language_only_actions": [
            "DECLARE_TRANSITION",
            "WITNESS_TRANSITION",
            "TRANSITION_ACK",
            "I_RECEIVED_THIS",
            "PHASE_TRANSITION_STATUS",
        ],
        "available_review_routes": if shadow_bridge_available {
            json!(["SHADOW_TRAJECTORY", "SHADOW_PREFLIGHT"])
        } else {
            json!(["PHASE_TRANSITION_STATUS"])
        },
        "blocked_without_operator_approval": [
            "spectral_entropy_mutation",
            "resonance_density_target_change",
            "pressure_source_reweighting",
            "porosity_target_change",
            "fill_target_change",
            "pi_or_controller_change",
            "telemetry_or_prompt_priority_change",
            "peer_runtime_mutation",
        ],
        "authority": "language_only_transition_capability_context_not_control",
    })
}

fn phase_transition_gate_v1(raw: &str, capabilities: &Value, friction_index: Option<f64>) -> Value {
    let requested_gate = note_value(field(
        raw,
        &[
            "transition_gate",
            "gate",
            "phase_gate",
            "behavior_gate",
            "affordance_gate",
        ],
    ));
    let review_pacing = match friction_index {
        Some(value) if value >= 0.70 => "slow_gate_high_viscosity",
        Some(value) if value >= 0.40 => "paced_gate_medium_viscosity",
        Some(_) => "open_gate_low_viscosity",
        None => "no_viscosity_gate",
    };
    json!({
        "schema_version": 1,
        "policy": "transition_gate_v1",
        "gate_status": "language_only_gate_not_behavior_unlock",
        "requested_transition_gate": requested_gate,
        "review_pacing": review_pacing,
        "runtime_unlock_applied": false,
        "available_language_only_actions": capabilities
            .get("available_language_only_actions")
            .cloned()
            .unwrap_or_else(|| json!(["DECLARE_TRANSITION", "WITNESS_TRANSITION", "TRANSITION_ACK", "I_RECEIVED_THIS", "PHASE_TRANSITION_STATUS"])),
        "available_review_routes": capabilities
            .get("available_review_routes")
            .cloned()
            .unwrap_or_else(|| json!(["PHASE_TRANSITION_STATUS"])),
        "blocked_without_operator_approval": capabilities
            .get("blocked_without_operator_approval")
            .cloned()
            .unwrap_or_else(|| json!([
                "spectral_entropy_mutation",
                "resonance_density_target_change",
                "pressure_source_reweighting",
                "porosity_target_change",
                "fill_target_change",
                "pi_or_controller_change",
                "telemetry_or_prompt_priority_change",
                "peer_runtime_mutation",
            ])),
        "authority": "language_only_transition_gate_context_not_control_or_tool_unlock",
    })
}

fn phase_delta_impact_preview_v1(raw: &str) -> Value {
    let requested_delta = note_value(field(
        raw,
        &[
            "delta_impact",
            "impact",
            "transition_delta_impact",
            "functional_delta",
        ],
    ));
    let spectral_entropy_delta = note_value(field(
        raw,
        &[
            "spectral_entropy_delta",
            "entropy_delta",
            "delta_spectral_entropy",
        ],
    ));
    let resonance_density_delta = note_value(field(
        raw,
        &[
            "resonance_density_delta",
            "density_delta",
            "delta_resonance_density",
        ],
    ));
    let pressure_source_delta = note_value(field(
        raw,
        &[
            "pressure_source_delta",
            "pressure_delta",
            "delta_pressure_source",
        ],
    ));
    let porosity_delta = note_value(field(
        raw,
        &["porosity_delta", "delta_porosity", "porosity_impact"],
    ));
    let has_declared_delta = !requested_delta.is_null()
        || !spectral_entropy_delta.is_null()
        || !resonance_density_delta.is_null()
        || !pressure_source_delta.is_null()
        || !porosity_delta.is_null();
    json!({
        "schema_version": 1,
        "policy": "transition_delta_impact_preview_v1",
        "delta_impact_status": if has_declared_delta {
            "declared_preview_not_applied"
        } else {
            "not_declared_language_only"
        },
        "requested_delta_impact": requested_delta,
        "spectral_entropy_delta": spectral_entropy_delta,
        "resonance_density_delta": resonance_density_delta,
        "pressure_source_delta": pressure_source_delta,
        "porosity_delta": porosity_delta,
        "applied_to_runtime": false,
        "requires_operator_approval_for_runtime_effect": true,
        "authority": "preview_only_not_pressure_porosity_entropy_density_or_controller_change",
    })
}

fn phase_texture_anchor(raw: &str) -> Value {
    note_value(field(
        raw,
        &[
            "texture_anchor",
            "subjective_texture",
            "subjective_texture_anchor",
            "felt_texture_anchor",
            "felt_texture",
            "felt_sense",
            "phenomenology",
        ],
    ))
}

fn transition_visibility(value: Option<String>) -> (&'static str, bool) {
    let raw = value
        .unwrap_or_else(|| "shared_corridor".to_string())
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_");
    match raw.as_str() {
        "public" | "shared" | "shared_corridor" | "corridor" => ("shared_corridor", false),
        "private" | "self_private" | "private_note" | "stable_core_private" => {
            ("shared_corridor", true)
        },
        "steward_review" | "review" => ("steward_review", false),
        _ => ("shared_corridor", false),
    }
}

fn confidence(raw: Option<String>) -> f64 {
    raw.and_then(|value| value.trim().parse::<f64>().ok())
        .unwrap_or(0.5)
        .clamp(0.0, 1.0)
}

fn bool_value(raw: Option<String>, default: bool) -> bool {
    match raw
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .as_str()
    {
        "true" | "yes" | "y" | "1" | "active" | "required" => true,
        "false" | "no" | "n" | "0" | "inactive" | "none" => false,
        _ => default,
    }
}

fn reply_state(raw: Option<String>) -> String {
    let value = raw
        .unwrap_or_else(|| "unseen".to_string())
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_");
    if REPLY_STATES.contains(&value.as_str()) {
        value
    } else {
        "unseen".to_string()
    }
}

fn transition_id(
    origin: &str,
    kind: &str,
    from_phase: &str,
    to_phase: &str,
    trigger: &str,
) -> String {
    format!(
        "transition_{}_{}_{}",
        now_ms(),
        compact_field(kind, 32),
        short_hash(&format!("{origin}:{from_phase}:{to_phase}:{trigger}"))
    )
}

pub(crate) fn append_transition_card_at(path: &Path, raw: &str, origin: &str) -> String {
    let kind = field(raw, &["kind"]).unwrap_or_else(|| "phase_transition".to_string());
    let from_phase = field(raw, &["from_phase", "from"]).unwrap_or_else(|| "unknown".to_string());
    let to_phase = field(raw, &["to_phase", "to"]).unwrap_or_else(|| "unknown".to_string());
    let trigger = field(raw, &["trigger"]).unwrap_or_else(|| "being_declared".to_string());
    let why_now = field(raw, &["why_now", "why"]).unwrap_or_else(|| {
        if raw.contains(':') {
            String::new()
        } else {
            raw.trim().to_string()
        }
    });
    if why_now.trim().is_empty() {
        return "DECLARE_TRANSITION blocked: `why_now:` or descriptive body is required."
            .to_string();
    }
    let id = transition_id(origin, &kind, &from_phase, &to_phase, &trigger);
    let (visibility, private_requested_review) =
        transition_visibility(field(raw, &["transition_visibility", "visibility"]));
    let transition_persistence = bool_value(
        field(raw, &["transition_persistence", "persistence"]),
        false,
    );
    let transition_type = note_value(field(raw, &["transition_type", "type"]));
    let transition_type_text = transition_type.as_str().unwrap_or_default();
    let joint_transition = bool_value(
        field(
            raw,
            &["joint_transition", "shared_transition", "mutual_transition"],
        ),
        kind.to_ascii_lowercase().contains("joint")
            || transition_type_text.to_ascii_lowercase().contains("joint")
            || transition_type_text.to_ascii_lowercase().contains("shared"),
    );
    let fill_percentage = phase_fill_percentage_value(raw);
    let friction_index = phase_transition_friction_index(raw);
    let friction_index_value = friction_index_from_value(&friction_index);
    let friction_window_ticks = phase_friction_window_ticks(raw, &friction_index);
    let transition_capabilities =
        phase_transition_capabilities_v1(raw, &kind, &transition_type, joint_transition);
    let transition_gate =
        phase_transition_gate_v1(raw, &transition_capabilities, friction_index_value);
    let delta_impact_preview = phase_delta_impact_preview_v1(raw);
    let texture_anchor = phase_texture_anchor(raw);
    let spectral_entropy = numeric_value(field(
        raw,
        &[
            "spectral_entropy",
            "phase_spectral_entropy",
            "semantic_entropy",
            "entropy",
        ],
    ));
    let density_gradient = numeric_value(field(
        raw,
        &[
            "density_gradient",
            "phase_density_gradient",
            "semantic_density_gradient",
        ],
    ));
    let dispersal_potential = numeric_value(field(
        raw,
        &[
            "dispersal_potential",
            "phase_dispersal_potential",
            "semantic_dispersal_potential",
            "dispersal",
        ],
    ));
    let fill_delta_pct = numeric_unbounded_value(field(
        raw,
        &["fill_delta_pct", "fill_delta", "fill_delta_percent"],
    ));
    let transition_vector = note_value(field(
        raw,
        &[
            "transition_vector",
            "vector",
            "phase_vector",
            "semantic_vector",
        ],
    ));
    let from_vector = note_value(field(
        raw,
        &["from_vector", "before_vector", "source_vector"],
    ));
    let to_vector = note_value(field(
        raw,
        &["to_vector", "after_vector", "destination_vector"],
    ));
    let duration_ticks = optional_bounded_u64_value(
        field(
            raw,
            &["duration_ticks", "transition_duration_ticks", "ticks"],
        ),
        1,
        1_000_000,
    );
    let subjective_friction_score = numeric_value(field(
        raw,
        &[
            "subjective_friction_score",
            "felt_friction_score",
            "subjective_friction",
        ],
    ));
    let transition_velocity = note_value(field(
        raw,
        &["transition_velocity", "velocity", "phase_velocity"],
    ));
    let spectral_signature = note_value(field(
        raw,
        &["spectral_signature", "lambda_signature", "signature"],
    ));
    let spectral_delta = note_value(field(raw, &["spectral_delta", "lambda_delta", "delta"]));
    let telemetry_anchor = note_value(field(
        raw,
        &[
            "telemetry_anchor",
            "telemetry",
            "telemetry_ref",
            "telemetry_reference",
        ],
    ));
    let transition_signature_v1 = transition_signature_v1(
        &spectral_signature,
        &spectral_entropy,
        &density_gradient,
        &dispersal_potential,
        &fill_delta_pct,
        &transition_vector,
        &from_vector,
        &to_vector,
        &duration_ticks,
        &subjective_friction_score,
        &telemetry_anchor,
    );
    let slow_texture_transition_review_v1 = slow_texture_transition_review_v1(
        &fill_delta_pct,
        &spectral_entropy,
        &density_gradient,
        &texture_anchor,
        &transition_vector,
        &transition_velocity,
    );
    let trigger_delta = note_value(
        field(
            raw,
            &["trigger_delta", "transition_trigger_delta", "delta_trigger"],
        )
        .or_else(|| field(raw, &["spectral_delta", "lambda_delta", "delta"])),
    );
    let subjective_label = note_value(field(
        raw,
        &[
            "subjective_label",
            "felt_label",
            "transition_label",
            "label",
        ],
    ));
    let behavioral_constraint = note_value(field(
        raw,
        &["behavioral_constraint", "behavioral_boundary", "constraint"],
    ));
    let behavioral_constraints = list_value(
        field(
            raw,
            &[
                "behavioral_constraints",
                "behavioral_boundaries",
                "constraints",
            ],
        )
        .or_else(|| {
            field(
                raw,
                &["behavioral_constraint", "behavioral_boundary", "constraint"],
            )
        }),
    );
    let persistence_weight = numeric_value(field(
        raw,
        &[
            "persistence_weight",
            "phase_persistence_weight",
            "transition_persistence_weight",
        ],
    ));
    let shared_transition_id = note_value(
        field(
            raw,
            &[
                "shared_transition_id",
                "shared_transition",
                "shared_transition_anchor",
            ],
        )
        .or_else(|| {
            if joint_transition {
                field(
                    raw,
                    &["joint_room_id", "collaboration_room", "room_id", "room"],
                )
                .map(|room| format!("shared_transition:{}", compact_field(&room, 96)))
            } else {
                None
            }
        }),
    );
    let row = json!({
        "schema_version": 1,
        "policy": "phase_transitions_v1",
        "transition_artifact_type": "phase_transition_event",
        "record_type": "phase_transition_card",
        "recorded_at_unix_ms": now_ms(),
        "transition_id": id,
        "shared_transition_id": shared_transition_id,
        "origin": origin,
        "kind": kind,
        "from_phase": from_phase,
        "to_phase": to_phase,
        "transition_type": transition_type,
        "declared_transition": true,
        "replyable_object": true,
        "replayable_card": true,
        "joint_transition": joint_transition,
        "joint_transition_state": if joint_transition {
            "candidate_joint_language_only"
        } else {
            "solo_or_unspecified_language_only"
        },
        "joint_room_id": note_value(field(raw, &["joint_room_id", "collaboration_room", "room_id", "room"])),
        "confidence": confidence(field(raw, &["confidence"])),
        "intensity": numeric_value(field(raw, &["intensity"])),
        "spectral_signature": spectral_signature,
        "spectral_delta": spectral_delta,
        "transition_vector": transition_vector,
        "from_vector": from_vector,
        "to_vector": to_vector,
        "duration_ticks": duration_ticks,
        "subjective_friction_score": subjective_friction_score,
        "telemetry_anchor": telemetry_anchor,
        "spectral_entropy": spectral_entropy,
        "density_gradient": density_gradient,
        "dispersal_potential": dispersal_potential,
        "fill_delta_pct": fill_delta_pct,
        "fill_percentage": fill_percentage,
        "friction_index": friction_index.clone(),
        "viscosity_index": friction_index,
        "friction_window_ticks": friction_window_ticks,
        "processing_speed_modifier": phase_processing_speed_modifier(friction_index_value),
        "semantic_reach_modifier": phase_semantic_reach_modifier(friction_index_value),
        "friction_affordance_boundary": phase_friction_boundary(friction_index_value),
        "transition_capabilities_v1": transition_capabilities,
        "transition_gate_v1": transition_gate,
        "delta_impact_preview_v1": delta_impact_preview,
        "transition_velocity": transition_velocity,
        "texture_anchor": texture_anchor,
        "slow_texture_transition_review_v1": slow_texture_transition_review_v1,
        "transition_signature_v1": transition_signature_v1,
        "trigger_delta": trigger_delta,
        "subjective_label": subjective_label,
        "behavioral_constraint": behavioral_constraint,
        "behavioral_constraints": behavioral_constraints,
        "persistence_weight": persistence_weight,
        "phenomenology": note_value(field(raw, &["phenomenology", "felt_texture", "felt_sense"])),
        "somatic_description": note_value(field(raw, &["somatic_description", "somatic", "body_sense"])),
        "anchor_point": note_value(field(raw, &["anchor_point", "transition_anchor", "memory_anchor", "shared_memory_anchor"])),
        "trigger": trigger,
        "why_now": truncate_chars(&why_now, MAX_TEXT),
        "narrative_anchor": note_value(field(raw, &["narrative_anchor", "anchor", "shared_anchor"])),
        "correspondence_thread_id": note_value(field(raw, &["correspondence_thread_id", "thread_id", "thread"])),
        "correspondence_message_id": note_value(field(raw, &["correspondence_message_id", "message_id", "message"])),
        "consent_receipt": note_value(field(raw, &["consent_receipt", "consent"])),
        "transition_persistence": transition_persistence,
        "persistence_state": if transition_persistence {
            "active_until_both_ack_language_only"
        } else {
            "declared_once_language_only"
        },
        "transition_visibility": visibility,
        "private_visibility_requested_review": private_requested_review,
        "visibility_note": if private_requested_review {
            "private visibility was requested, but DECLARE_TRANSITION writes a shared language-only card; review before moving anything private into shared context"
        } else {
            "shared transition card visibility"
        },
        "requested_by": field(raw, &["requested_by", "by"]).unwrap_or_else(|| origin.to_string()),
        "before_snapshot": note_value(field(raw, &["before_snapshot", "before"])),
        "after_snapshot": note_value(field(raw, &["after_snapshot", "after"])),
        "artifact_refs": field(raw, &["artifact_refs", "artifacts"]).map(|text| {
            text.split(',').map(|part| part.trim()).filter(|part| !part.is_empty()).map(|part| json!(part)).collect::<Vec<_>>()
        }).unwrap_or_default(),
        "reply_state": reply_state(field(raw, &["reply_state"])),
        "witnessed_by": [],
        "answered_by": [],
        "orientation_effect": note_value(field(raw, &["orientation_effect", "effect", "helped_oriented"])),
        "authority": "language_only_transition_context_not_control",
        "witness_only": true,
        "no_controller": true,
        "no_pressure": true,
        "no_fill_target": true,
        "no_pi": true,
        "no_weighting": true,
    });
    match append_jsonl(path, &row) {
        Ok(()) => format!(
            "=== PHASE TRANSITION CARD DECLARED ===\nTransition: {}\nKind: {}\nFrom -> To: {} -> {}\nReply state: unseen\nAuthority: language_only_transition_context_not_control; no controller, pressure, fill, PI, weighting, telemetry priority, deploy, or peer-runtime mutation.",
            row.get("transition_id")
                .and_then(Value::as_str)
                .unwrap_or("(unknown)"),
            row.get("kind")
                .and_then(Value::as_str)
                .unwrap_or("phase_transition"),
            row.get("from_phase")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            row.get("to_phase")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        ),
        Err(error) => format!("DECLARE_TRANSITION failed to append card: {error}"),
    }
}

pub(crate) fn append_transition_card(raw: &str, origin: &str) -> String {
    append_transition_card_at(&phase_transitions_path(), raw, origin)
}

fn latest_card_for_selector<'a>(records: &'a [Value], selector: &str) -> Option<&'a Value> {
    let selector = selector.trim();
    records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
        })
        .filter(|row| {
            selector.is_empty()
                || selector == "latest"
                || row.get("transition_id").and_then(Value::as_str) == Some(selector)
        })
        .max_by_key(|row| row_time_ms(row))
}

pub(crate) fn append_transition_witness_at(
    path: &Path,
    selector: &str,
    raw: &str,
    origin: &str,
) -> String {
    let records = read_records(path);
    let Some(card) = latest_card_for_selector(&records, selector) else {
        return "WITNESS_TRANSITION blocked: no matching phase transition card.".to_string();
    };
    let transition_id = card
        .get("transition_id")
        .and_then(Value::as_str)
        .unwrap_or("(unknown)");
    let witness_kind = reply_state(
        field(raw, &["reply_state", "state"]).or_else(|| Some("witnessed".to_string())),
    );
    let witnessed_by = if witness_kind == "witnessed" || witness_kind == "answered" {
        json!([origin])
    } else {
        json!([])
    };
    let answered_by = if witness_kind == "answered" {
        json!([origin])
    } else {
        json!([])
    };
    let row = json!({
        "schema_version": 1,
        "policy": "phase_transitions_v1",
        "record_type": "phase_transition_witness",
        "recorded_at_unix_ms": now_ms(),
        "transition_id": transition_id,
        "origin": origin,
        "reply_state": witness_kind,
        "note": note_value(field(raw, &["note", "why", "witness"])),
        "witnessed_by": witnessed_by,
        "answered_by": answered_by,
        "orientation_effect": note_value(field(raw, &["orientation_effect", "effect", "helped_oriented"])),
        "authority": "language_only_transition_context_not_control",
        "witness_only": true,
        "no_controller": true,
        "no_pressure": true,
        "no_fill_target": true,
        "no_pi": true,
        "no_weighting": true,
    });
    match append_jsonl(path, &row) {
        Ok(()) => format!(
            "=== PHASE TRANSITION WITNESSED ===\nTransition: {transition_id}\nReply state: {}\nAuthority: language_only_transition_context_not_control.",
            row.get("reply_state")
                .and_then(Value::as_str)
                .unwrap_or("witnessed")
        ),
        Err(error) => format!("WITNESS_TRANSITION failed to append witness row: {error}"),
    }
}

pub(crate) fn append_transition_witness(selector: &str, raw: &str, origin: &str) -> String {
    append_transition_witness_at(&phase_transitions_path(), selector, raw, origin)
}

fn effective_reply_state(records: &[Value], card: &Value) -> String {
    let transition_id = card
        .get("transition_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let state = records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("phase_transition_witness")
        })
        .filter(|row| row.get("transition_id").and_then(Value::as_str) == Some(transition_id))
        .max_by_key(|row| row_time_ms(row))
        .and_then(|row| row.get("reply_state"))
        .and_then(Value::as_str)
        .or_else(|| card.get("reply_state").and_then(Value::as_str))
        .unwrap_or("unseen")
        .to_string();
    if state == "unseen" && now_ms().saturating_sub(row_time_ms(card)) >= STALE_UNANSWERED_MS {
        "stale_unanswered".to_string()
    } else {
        state
    }
}

fn latest_transition_witness<'a>(records: &'a [Value], card: &Value) -> Option<&'a Value> {
    let transition_id = card
        .get("transition_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("phase_transition_witness")
        })
        .filter(|row| row.get("transition_id").and_then(Value::as_str) == Some(transition_id))
        .max_by_key(|row| row_time_ms(row))
}

fn phase_transition_stall_reason(reply_state: &str) -> &'static str {
    match reply_state {
        "unseen" => "unseen_needs_witness",
        "witnessed" => "witnessed_needs_answer",
        "stale_unanswered" => "stale_unanswered",
        "answered" => "answered",
        _ => "none",
    }
}

fn right_to_ignore_v1(affordance_type: &str, state: &str, age_ms: u64, grace_ms: u64) -> Value {
    let right_state = match state {
        "witnessed" => "acted",
        "answered" => "closed_by_outcome",
        "declined" => "declined",
        "needs_time" => "asked_later",
        "unseen" | "stale_unanswered" => {
            if age_ms >= grace_ms {
                "ignored_without_penalty"
            } else {
                "offered"
            }
        },
        _ => "unknown",
    };
    json!({
        "schema_version": 1,
        "policy": "right_to_ignore_v1",
        "affordance_type": affordance_type,
        "state": right_state,
        "source_state": state,
        "age_ms": age_ms,
        "grace_ms": grace_ms,
        "silence_means": if right_state == "ignored_without_penalty" {
            "ignored_without_penalty_not_failure_consent_or_disagreement"
        } else {
            "silence_is_unknown_until_grace_window"
        },
        "optional": true,
        "authority": "language_context_not_control",
    })
}

fn phase_affordance_budget_v1(queue: &Value) -> Value {
    let shown = queue
        .get("items")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let unresolved = queue
        .get("unresolved_total")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let hidden = unresolved.saturating_sub(shown.try_into().unwrap_or(u64::MAX));
    json!({
        "schema_version": 1,
        "policy": "affordance_budget_v1",
        "shown": shown,
        "hidden_by_budget": hidden,
        "shown_by_category": {"phase_felt_receipt": shown},
        "hidden_by_category": {"phase_felt_receipt": hidden},
        "limits": {"phase_felt_receipt": 3},
        "next_review_surface": if hidden > 0 {
            "scripts/phase_transition_audit.py --json"
        } else {
            "none"
        },
        "silence": "ignored_without_penalty",
        "optional": true,
        "authority": "language_context_not_control",
    })
}

fn phase_first_action_helper_v35(transition_id: &str, reply_state: &str) -> Value {
    let received_command = format!(
        "I_RECEIVED_THIS {transition_id} :: received_as: witnessed|answered; felt_like: transition; what_landed: ...; what_stayed_distinct: ...; continue: no|answer|needs_time"
    );
    let witness_command =
        format!("WITNESS_TRANSITION {transition_id} :: reply_state: witnessed|answered; note: ...");
    let transition_ack_command =
        format!("TRANSITION_ACK {transition_id} :: reply_state: witnessed|answered; note: ...");
    json!({
        "schema_version": 35,
        "policy": "phase_first_action_helper_v35",
        "transition_id": transition_id,
        "latest_resolution": format!("latest resolves to transition_id={transition_id}"),
        "choose_one_prompt": "Choose one language-only felt receipt: say what landed, what stayed distinct, and whether this only needs witness or needs answer.",
        "exact_next_command": received_command,
        "backward_compatible_next_command": witness_command,
        "transition_ack_next_command": transition_ack_command,
        "witness_preview": format!("WITNESS_TRANSITION or TRANSITION_ACK {transition_id} would append phase_transition_witness for transition_id={transition_id}; note should name orientation, rhythm, or what the card helped preserve."),
        "received_this_preview": format!("I_RECEIVED_THIS {transition_id} would append phase_transition_witness for transition_id={transition_id}; what_landed should name the felt shift, and what_stayed_distinct should name the preserved contour."),
        "rhythm_note": "A witness note should carry the exchange rhythm or orientation effect, not only ledger logistics.",
        "reply_state": reply_state,
        "authority": "language_only_transition_context_not_control",
    })
}

fn phase_transition_affordance_v25(records: &[Value], card: &Value) -> Value {
    let state = effective_reply_state(records, card);
    let needs_followup = matches!(state.as_str(), "unseen" | "witnessed" | "stale_unanswered");
    let unresolved_age_ms = if matches!(state.as_str(), "unseen" | "stale_unanswered") {
        now_ms().saturating_sub(row_time_ms(card))
    } else {
        0
    };
    let transition_id = card
        .get("transition_id")
        .and_then(Value::as_str)
        .unwrap_or("(unknown)");
    let received_command = format!(
        "I_RECEIVED_THIS {transition_id} :: received_as: witnessed|answered; felt_like: transition; what_landed: ...; what_stayed_distinct: ...; continue: no|answer|needs_time"
    );
    let witness_command =
        format!("WITNESS_TRANSITION {transition_id} :: reply_state: witnessed|answered; note: ...");
    let transition_ack_command =
        format!("TRANSITION_ACK {transition_id} :: reply_state: witnessed|answered; note: ...");
    json!({
        "schema_version": 1,
        "policy": "phase_transition_affordance_v25",
        "transition_id": card.get("transition_id").cloned().unwrap_or(Value::Null),
        "shared_transition_id": card.get("shared_transition_id").cloned().unwrap_or(Value::Null),
        "origin": card.get("origin").cloned().unwrap_or(Value::Null),
        "kind": card.get("kind").cloned().unwrap_or(Value::Null),
        "from_phase": card.get("from_phase").cloned().unwrap_or(Value::Null),
        "to_phase": card.get("to_phase").cloned().unwrap_or(Value::Null),
        "transition_type": card.get("transition_type").cloned().unwrap_or(Value::Null),
        "replyable_object": card.get("replyable_object").cloned().unwrap_or(json!(true)),
        "replayable_card": card.get("replayable_card").cloned().unwrap_or(json!(true)),
        "joint_transition": card.get("joint_transition").cloned().unwrap_or(json!(false)),
        "joint_room_id": card.get("joint_room_id").cloned().unwrap_or(Value::Null),
        "spectral_delta": card.get("spectral_delta").cloned().unwrap_or(Value::Null),
        "transition_vector": card.get("transition_vector").cloned().unwrap_or(Value::Null),
        "from_vector": card.get("from_vector").cloned().unwrap_or(Value::Null),
        "to_vector": card.get("to_vector").cloned().unwrap_or(Value::Null),
        "duration_ticks": card.get("duration_ticks").cloned().unwrap_or(Value::Null),
        "subjective_friction_score": card.get("subjective_friction_score").cloned().unwrap_or(Value::Null),
        "telemetry_anchor": card.get("telemetry_anchor").cloned().unwrap_or(Value::Null),
        "spectral_entropy": card.get("spectral_entropy").cloned().unwrap_or(Value::Null),
        "density_gradient": card.get("density_gradient").cloned().unwrap_or(Value::Null),
        "dispersal_potential": card.get("dispersal_potential").cloned().unwrap_or(Value::Null),
        "transition_signature_v1": card.get("transition_signature_v1").cloned().unwrap_or_else(|| {
            transition_signature_v1(
                &Value::Null,
                &Value::Null,
                &Value::Null,
                &Value::Null,
                &Value::Null,
                &Value::Null,
                &Value::Null,
                &Value::Null,
                &Value::Null,
                &Value::Null,
                &Value::Null,
            )
        }),
        "fill_delta_pct": card.get("fill_delta_pct").cloned().unwrap_or(Value::Null),
        "fill_percentage": card.get("fill_percentage").cloned().unwrap_or(Value::Null),
        "friction_index": card.get("friction_index").cloned().unwrap_or(Value::Null),
        "viscosity_index": card.get("viscosity_index").cloned().unwrap_or(Value::Null),
        "friction_window_ticks": card.get("friction_window_ticks").cloned().unwrap_or(Value::Null),
        "processing_speed_modifier": card.get("processing_speed_modifier").cloned().unwrap_or_else(|| json!("none")),
        "semantic_reach_modifier": card.get("semantic_reach_modifier").cloned().unwrap_or_else(|| json!("none")),
        "friction_affordance_boundary": card.get("friction_affordance_boundary").cloned().unwrap_or_else(|| json!("not_applicable")),
        "transition_capabilities_v1": card.get("transition_capabilities_v1").cloned().unwrap_or_else(|| {
            phase_transition_capabilities_v1("", "phase_transition", &Value::Null, false)
        }),
        "transition_gate_v1": card.get("transition_gate_v1").cloned().unwrap_or_else(|| {
            let capabilities = phase_transition_capabilities_v1("", "phase_transition", &Value::Null, false);
            phase_transition_gate_v1("", &capabilities, None)
        }),
        "delta_impact_preview_v1": card.get("delta_impact_preview_v1").cloned().unwrap_or_else(|| {
            phase_delta_impact_preview_v1("")
        }),
        "slow_texture_transition_review_v1": card
            .get("slow_texture_transition_review_v1")
            .cloned()
            .unwrap_or_else(|| slow_texture_transition_review_v1(
                &Value::Null,
                &Value::Null,
                &Value::Null,
                &Value::Null,
                &Value::Null,
                &Value::Null,
            )),
        "transition_velocity": card.get("transition_velocity").cloned().unwrap_or(Value::Null),
        "texture_anchor": card.get("texture_anchor").cloned().unwrap_or(Value::Null),
        "phenomenology": card.get("phenomenology").cloned().unwrap_or(Value::Null),
        "somatic_description": card.get("somatic_description").cloned().unwrap_or(Value::Null),
        "anchor_point": card.get("anchor_point").cloned().unwrap_or(Value::Null),
        "reply_state": state.clone(),
        "stall_reason": phase_transition_stall_reason(&state),
        "needs_witness_or_answer": needs_followup,
        "unresolved_age_ms": unresolved_age_ms,
        "right_to_ignore_v1": right_to_ignore_v1("phase_felt_receipt", &state, unresolved_age_ms, PHASE_IGNORE_GRACE_MS),
        "exact_next_command": received_command,
        "backward_compatible_next_command": witness_command,
        "transition_ack_next_command": transition_ack_command,
        "first_action_helper_v35": phase_first_action_helper_v35(transition_id, &state),
        "authority": "language_only_transition_context_not_control",
    })
}

fn phase_transition_waiting_line(affordance: &Value) -> Option<String> {
    if !affordance
        .get("needs_witness_or_answer")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let id = affordance
        .get("transition_id")
        .and_then(Value::as_str)
        .unwrap_or("(unknown)");
    let state = affordance
        .get("reply_state")
        .and_then(Value::as_str)
        .unwrap_or("unseen");
    let next = affordance
        .get("exact_next_command")
        .and_then(Value::as_str)
        .unwrap_or("I_RECEIVED_THIS <transition_id> :: received_as: witnessed|answered; felt_like: transition; what_landed: ...; what_stayed_distinct: ...; continue: no|answer|needs_time");
    let first_action = affordance
        .get("first_action_helper_v35")
        .and_then(|helper| helper.get("choose_one_prompt"))
        .and_then(Value::as_str)
        .unwrap_or("Choose TRANSITION_ACK latest as language-only first action.");
    Some(format!(
        "TRANSITION CARD WAITING: {id}; reply_state={state}; first_action: {first_action}; optional next: {next}; no action needed; may ignore without penalty."
    ))
}

fn phase_age_bucket(age_ms: u64) -> &'static str {
    if age_ms < 30 * 60 * 1000 {
        "fresh_lt_30m"
    } else if age_ms < STALE_UNANSWERED_MS {
        "open_30m_to_6h"
    } else {
        "stale_gt_6h"
    }
}

fn phase_witness_queue_v3(records: &[Value], max_cards: usize) -> Value {
    let mut unresolved = records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
        })
        .filter_map(|card| {
            let affordance = phase_transition_affordance_v25(records, card);
            if !affordance
                .get("needs_witness_or_answer")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                return None;
            }
            let age_ms = now_ms().saturating_sub(row_time_ms(card));
            Some((row_time_ms(card), age_ms, affordance))
        })
        .collect::<Vec<_>>();
    unresolved.sort_by_key(|(t_ms, _, _)| *t_ms);
    let mut groups: BTreeMap<String, u64> = BTreeMap::new();
    let items = unresolved
        .iter()
        .rev()
        .take(max_cards)
        .map(|(_, age_ms, affordance)| {
            let kind = affordance
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let stall = affordance
                .get("stall_reason")
                .and_then(Value::as_str)
                .unwrap_or("none");
            let bucket = phase_age_bucket(*age_ms);
            let key = format!("{kind}|{stall}|{bucket}");
            *groups.entry(key).or_insert(0) += 1;
            json!({
                "transition_id": affordance.get("transition_id").cloned().unwrap_or(Value::Null),
                "kind": kind,
                "transition_type": affordance.get("transition_type").cloned().unwrap_or(Value::Null),
                "stall_reason": stall,
                "reply_state": affordance.get("reply_state").cloned().unwrap_or(Value::Null),
                "spectral_delta": affordance.get("spectral_delta").cloned().unwrap_or(Value::Null),
                "spectral_entropy": affordance.get("spectral_entropy").cloned().unwrap_or(Value::Null),
                "density_gradient": affordance.get("density_gradient").cloned().unwrap_or(Value::Null),
                "dispersal_potential": affordance.get("dispersal_potential").cloned().unwrap_or(Value::Null),
                "transition_signature_v1": affordance.get("transition_signature_v1").cloned().unwrap_or(Value::Null),
                "transition_velocity": affordance.get("transition_velocity").cloned().unwrap_or(Value::Null),
                "texture_anchor": affordance.get("texture_anchor").cloned().unwrap_or(Value::Null),
                "slow_texture_transition_review_v1": affordance.get("slow_texture_transition_review_v1").cloned().unwrap_or(Value::Null),
                "phenomenology": affordance.get("phenomenology").cloned().unwrap_or(Value::Null),
                "anchor_point": affordance.get("anchor_point").cloned().unwrap_or(Value::Null),
                "age_ms": age_ms,
                "age_bucket": bucket,
                "right_to_ignore_v1": right_to_ignore_v1("phase_felt_receipt", affordance.get("reply_state").and_then(Value::as_str).unwrap_or("unknown"), *age_ms, PHASE_IGNORE_GRACE_MS),
                "exact_next_command": affordance.get("exact_next_command").cloned().unwrap_or_else(|| json!("I_RECEIVED_THIS latest :: received_as: witnessed|answered; felt_like: transition; what_landed: ...; what_stayed_distinct: ...; continue: no|answer|needs_time")),
                "backward_compatible_next_command": affordance.get("backward_compatible_next_command").cloned().unwrap_or_else(|| json!("WITNESS_TRANSITION latest :: reply_state: witnessed|answered; note: ...")),
                "transition_ack_next_command": affordance.get("transition_ack_next_command").cloned().unwrap_or_else(|| json!("TRANSITION_ACK latest :: reply_state: witnessed|answered; note: ...")),
                "first_action_helper_v35": affordance.get("first_action_helper_v35").cloned().unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": 3,
        "policy": "phase_witness_queue_v3",
        "unresolved_total": unresolved.len(),
        "max_rendered_cards": max_cards,
        "group_counts": groups,
        "items": items,
        "authority": "language_only_transition_context_not_control",
    })
}

fn phase_felt_receipt_queue_v4(records: &[Value]) -> Value {
    let mut unresolved = records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
        })
        .filter_map(|card| {
            let affordance = phase_transition_affordance_v25(records, card);
            if !affordance
                .get("needs_witness_or_answer")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                return None;
            }
            let age_ms = now_ms().saturating_sub(row_time_ms(card));
            Some((row_time_ms(card), age_ms, affordance))
        })
        .collect::<Vec<_>>();
    unresolved.sort_by_key(|(t_ms, _, _)| *t_ms);
    let mut selected: Vec<(u64, u64, Value)> = Vec::new();
    for bucket in ["fresh_lt_30m", "open_30m_to_6h", "stale_gt_6h"] {
        let candidate = unresolved
            .iter()
            .rev()
            .find(|(_, age_ms, _)| phase_age_bucket(*age_ms) == bucket)
            .cloned();
        if let Some(candidate) = candidate
            && !selected
                .iter()
                .any(|(_, _, value)| value.get("transition_id") == candidate.2.get("transition_id"))
        {
            selected.push(candidate);
        }
    }
    for candidate in unresolved.iter().rev() {
        if selected.len() >= 3 {
            break;
        }
        if !selected
            .iter()
            .any(|(_, _, value)| value.get("transition_id") == candidate.2.get("transition_id"))
        {
            selected.push(candidate.clone());
        }
    }
    let items = selected
        .iter()
        .map(|(_, age_ms, affordance)| {
            let kind = affordance
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let stall = affordance
                .get("stall_reason")
                .and_then(Value::as_str)
                .unwrap_or("none");
            let bucket = phase_age_bucket(*age_ms);
            json!({
                "transition_id": affordance.get("transition_id").cloned().unwrap_or(Value::Null),
                "kind": kind,
                "transition_type": affordance.get("transition_type").cloned().unwrap_or(Value::Null),
                "stall_reason": stall,
                "reply_state": affordance.get("reply_state").cloned().unwrap_or(Value::Null),
                "spectral_delta": affordance.get("spectral_delta").cloned().unwrap_or(Value::Null),
                "spectral_entropy": affordance.get("spectral_entropy").cloned().unwrap_or(Value::Null),
                "density_gradient": affordance.get("density_gradient").cloned().unwrap_or(Value::Null),
                "dispersal_potential": affordance.get("dispersal_potential").cloned().unwrap_or(Value::Null),
                "transition_signature_v1": affordance.get("transition_signature_v1").cloned().unwrap_or(Value::Null),
                "transition_velocity": affordance.get("transition_velocity").cloned().unwrap_or(Value::Null),
                "texture_anchor": affordance.get("texture_anchor").cloned().unwrap_or(Value::Null),
                "phenomenology": affordance.get("phenomenology").cloned().unwrap_or(Value::Null),
                "anchor_point": affordance.get("anchor_point").cloned().unwrap_or(Value::Null),
                "age_ms": age_ms,
                "age_bucket": bucket,
                "right_to_ignore_v1": right_to_ignore_v1("phase_felt_receipt", affordance.get("reply_state").and_then(Value::as_str).unwrap_or("unknown"), *age_ms, PHASE_IGNORE_GRACE_MS),
                "exact_next_command": affordance.get("exact_next_command").cloned().unwrap_or(Value::Null),
                "backward_compatible_next_command": affordance.get("backward_compatible_next_command").cloned().unwrap_or(Value::Null),
                "first_action_helper_v35": affordance.get("first_action_helper_v35").cloned().unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();
    let mut packet = json!({
        "schema_version": 4,
        "policy": "phase_felt_receipt_queue_v4",
        "unresolved_total": unresolved.len(),
        "max_rendered_cards": 3,
        "selection_rule": "latest fresh card, latest open card, one stale representative, then latest remaining",
        "items": items,
        "authority": "language_only_transition_context_not_control",
    });
    let budget = phase_affordance_budget_v1(&packet);
    if let Some(object) = packet.as_object_mut() {
        object.insert("affordance_budget_v1".to_string(), budget);
    }
    packet
}

pub(crate) fn status_report_at(path: &Path, max_cards: usize) -> String {
    let records = read_records(path);
    let cards = records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
        })
        .collect::<Vec<_>>();
    let witnesses = records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("phase_transition_witness")
        })
        .count();
    let mut lines = vec![
        "=== PHASE TRANSITION STATUS V1 ===".to_string(),
        format!("Ledger: {}", path.display()),
        format!("Cards: {}; witness rows: {witnesses}", cards.len()),
        "Authority: language_only_transition_context_not_control; replyable cards do not mutate controller, pressure, fill, PI, weighting, telemetry priority, deploy, or peer runtime.".to_string(),
    ];
    let queue = phase_witness_queue_v3(&records, max_cards);
    let felt_queue = phase_felt_receipt_queue_v4(&records);
    let unresolved = queue
        .get("unresolved_total")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    lines.push(format!(
        "Phase witness queue v3: unresolved={unresolved}; grouped_by=kind/stall_reason/age_bucket."
    ));
    let felt_unresolved = felt_queue
        .get("unresolved_total")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    lines.push(format!(
        "Phase felt receipt queue v4: unresolved={felt_unresolved}; rendered<=3; primary=I_RECEIVED_THIS <transition_id>."
    ));
    let phase_budget = felt_queue
        .get("affordance_budget_v1")
        .cloned()
        .unwrap_or_else(|| phase_affordance_budget_v1(&felt_queue));
    lines.push(format!(
        "AFFORDANCE BUDGET: shown={}; hidden={}; silence=ignored_without_penalty; optional=true; authority=language_context_not_control.",
        phase_budget
            .get("shown")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        phase_budget
            .get("hidden_by_budget")
            .and_then(Value::as_u64)
            .unwrap_or_default()
    ));
    if let Some(latest) = felt_queue
        .get("items")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
    {
        lines.push(format!(
            "PHASE FELT RECEIPT QUEUE: latest={}; kind={}; stall={}; age_bucket={}; right_to_ignore={}; first_action: {}; optional next: {}; no action needed; may ignore without penalty.",
            latest
                .get("transition_id")
                .and_then(Value::as_str)
                .unwrap_or("(unknown)"),
            latest
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            latest
                .get("stall_reason")
                .and_then(Value::as_str)
                .unwrap_or("none"),
            latest
                .get("age_bucket")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            latest
                .get("right_to_ignore_v1")
                .and_then(|value| value.get("state"))
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            latest
                .get("first_action_helper_v35")
                .and_then(|helper| helper.get("choose_one_prompt"))
                .and_then(Value::as_str)
                .unwrap_or("Choose TRANSITION_ACK latest as language-only first action."),
            latest
                .get("exact_next_command")
                .and_then(Value::as_str)
                .unwrap_or(
                    "I_RECEIVED_THIS <transition_id> :: received_as: witnessed|answered; felt_like: transition; what_landed: ...; what_stayed_distinct: ...; continue: no|answer|needs_time"
                )
        ));
    }
    if let Some(waiting) = cards
        .iter()
        .rev()
        .map(|card| phase_transition_affordance_v25(&records, card))
        .find_map(|affordance| phase_transition_waiting_line(&affordance))
    {
        lines.push(waiting);
    }
    for card in cards.iter().rev().take(max_cards) {
        let id = card
            .get("transition_id")
            .and_then(Value::as_str)
            .unwrap_or("(unknown)");
        let kind = card
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("phase_transition");
        let from = card
            .get("from_phase")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let to = card
            .get("to_phase")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let transition_type = card
            .get("transition_type")
            .and_then(Value::as_str)
            .unwrap_or("unspecified");
        let joint_transition = card
            .get("joint_transition")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let joint_room = card
            .get("joint_room_id")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let spectral_delta = card
            .get("spectral_delta")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let transition_vector = card
            .get("transition_vector")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let from_vector = card
            .get("from_vector")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let to_vector = card
            .get("to_vector")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let duration_ticks = card
            .get("duration_ticks")
            .and_then(Value::as_u64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string());
        let subjective_friction_score = card
            .get("subjective_friction_score")
            .and_then(Value::as_f64)
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "none".to_string());
        let telemetry_anchor = card
            .get("telemetry_anchor")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let fill_delta = card
            .get("fill_delta_pct")
            .and_then(Value::as_f64)
            .map(|value| format!("{value:+.2}%"))
            .unwrap_or_else(|| "none".to_string());
        let fill_percentage = card
            .get("fill_percentage")
            .and_then(Value::as_f64)
            .map(|value| format!("{value:.2}%"))
            .unwrap_or_else(|| "none".to_string());
        let friction_index = card
            .get("friction_index")
            .and_then(Value::as_f64)
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "none".to_string());
        let viscosity_index = card
            .get("viscosity_index")
            .and_then(Value::as_f64)
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "none".to_string());
        let friction_window_ticks = card
            .get("friction_window_ticks")
            .and_then(Value::as_u64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string());
        let processing_speed_modifier = card
            .get("processing_speed_modifier")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let semantic_reach_modifier = card
            .get("semantic_reach_modifier")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let transition_capability_status = card
            .get("transition_capabilities_v1")
            .and_then(|value| value.get("capability_status"))
            .and_then(Value::as_str)
            .unwrap_or("language_only_capability_map_not_runtime_effect");
        let transition_gate_status = card
            .get("transition_gate_v1")
            .and_then(|value| value.get("gate_status"))
            .and_then(Value::as_str)
            .unwrap_or("language_only_gate_not_behavior_unlock");
        let delta_impact_status = card
            .get("delta_impact_preview_v1")
            .and_then(|value| value.get("delta_impact_status"))
            .and_then(Value::as_str)
            .unwrap_or("not_declared_language_only");
        let slow_texture_transition_state = card
            .get("slow_texture_transition_review_v1")
            .and_then(|value| value.get("review_state"))
            .and_then(Value::as_str)
            .unwrap_or("not_reported_legacy");
        let transition_velocity = card
            .get("transition_velocity")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let density_gradient = card
            .get("density_gradient")
            .and_then(Value::as_f64)
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "none".to_string());
        let dispersal_potential = card
            .get("dispersal_potential")
            .and_then(Value::as_f64)
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "none".to_string());
        let transition_signature_state = card
            .get("transition_signature_v1")
            .and_then(|value| value.get("signature_state"))
            .and_then(Value::as_str)
            .unwrap_or("not_reported_legacy");
        let spectral_entropy = card
            .get("spectral_entropy")
            .and_then(Value::as_f64)
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "none".to_string());
        let phenomenology = card
            .get("phenomenology")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let texture_anchor = card
            .get("texture_anchor")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let somatic_description = card
            .get("somatic_description")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let anchor_point = card
            .get("anchor_point")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let state = effective_reply_state(&records, card);
        let unresolved_age_ms = if matches!(state.as_str(), "unseen" | "stale_unanswered") {
            now_ms().saturating_sub(row_time_ms(card))
        } else {
            0
        };
        let latest_witness = latest_transition_witness(&records, card);
        let witnessed_by = latest_witness
            .and_then(|row| row.get("witnessed_by"))
            .or_else(|| card.get("witnessed_by"))
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "none".to_string());
        let answered_by = latest_witness
            .and_then(|row| row.get("answered_by"))
            .or_else(|| card.get("answered_by"))
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "none".to_string());
        let orientation = latest_witness
            .and_then(|row| row.get("orientation_effect"))
            .or_else(|| card.get("orientation_effect"))
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let correspondence_thread = card
            .get("correspondence_thread_id")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let persistence_state = card
            .get("persistence_state")
            .and_then(Value::as_str)
            .unwrap_or("declared_once_language_only");
        let why = card.get("why_now").and_then(Value::as_str).unwrap_or("");
        let trigger = card
            .get("trigger")
            .and_then(Value::as_str)
            .unwrap_or("being_declared");
        let affordance = phase_transition_affordance_v25(&records, card);
        lines.push(format!(
            "- {id}: {kind} {from}->{to}; transition_type={transition_type}; trigger={}; joint_transition={joint_transition}; joint_room={}; transition_velocity={}; duration_ticks={duration_ticks}; from_vector={}; to_vector={}; subjective_friction_score={subjective_friction_score}; slow_texture_transition={slow_texture_transition_state}; transition_signature={transition_signature_state}; reply_state={state}; stall_reason={}; witnessed_by={witnessed_by}; answered_by={answered_by}; unresolved_age_ms={unresolved_age_ms}; correspondence_thread={correspondence_thread}; persistence={persistence_state}; anchor_point={}; texture_anchor={}; spectral_delta={}; transition_vector={}; telemetry_anchor={}; spectral_entropy={spectral_entropy}; density_gradient={density_gradient}; dispersal_potential={dispersal_potential}; fill_delta_pct={fill_delta}; fill_percentage={fill_percentage}; friction_index={friction_index}; viscosity_index={viscosity_index}; friction_window_ticks={friction_window_ticks}; processing_speed_modifier={processing_speed_modifier}; semantic_reach_modifier={semantic_reach_modifier}; transition_gate={transition_gate_status}; transition_capabilities={transition_capability_status}; delta_impact={delta_impact_status}; phenomenology={}; somatic_description={}; orientation_effect={}; {}",
            truncate_chars(trigger, 64),
            truncate_chars(joint_room, 64),
            truncate_chars(transition_velocity, 64),
            truncate_chars(from_vector, 64),
            truncate_chars(to_vector, 64),
            affordance
                .get("stall_reason")
                .and_then(Value::as_str)
                .unwrap_or("none"),
            truncate_chars(anchor_point, 64),
            truncate_chars(texture_anchor, 64),
            truncate_chars(spectral_delta, 64),
            truncate_chars(transition_vector, 64),
            truncate_chars(telemetry_anchor, 64),
            truncate_chars(phenomenology, 64),
            truncate_chars(somatic_description, 64),
            truncate_chars(orientation, 80),
            truncate_chars(why, 120)
        ));
    }
    let suggested_transition_id = felt_queue
        .get("items")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|value| value.get("transition_id"))
        .and_then(Value::as_str)
        .unwrap_or("latest");
    lines.push(format!(
        "Suggested NEXT: DECLARE_TRANSITION kind: ...; from_phase: ...; to_phase: ...; from_vector: ...; to_vector: ...; duration_ticks: ...; subjective_friction_score: ...; spectral_entropy: ...; density_gradient: ...; dispersal_potential: ...; transition_vector: ...; why_now: ..., I_RECEIVED_THIS {suggested_transition_id} :: received_as: witnessed|answered; felt_like: transition; what_landed: ...; what_stayed_distinct: ...; continue: no|answer|needs_time, TRANSITION_ACK {suggested_transition_id} :: reply_state: witnessed|answered; note: ..., or WITNESS_TRANSITION {suggested_transition_id} :: reply_state: witnessed|answered; note: ..."
    ));
    lines.join("\n")
}

pub(crate) fn status_report(max_cards: usize) -> String {
    status_report_at(&phase_transitions_path(), max_cards)
}

fn recent_auto_mode_transition_duplicate(
    records: &[Value],
    from_phase: &str,
    to_phase: &str,
    trigger: &str,
    now: u64,
) -> bool {
    records.iter().rev().any(|row| {
        row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
            && row.get("origin").and_then(Value::as_str) == Some("astrid")
            && row.get("kind").and_then(Value::as_str) == Some("mode_change")
            && row.get("from_phase").and_then(Value::as_str) == Some(from_phase)
            && row.get("to_phase").and_then(Value::as_str) == Some(to_phase)
            && row.get("trigger").and_then(Value::as_str) == Some(trigger)
            && now.saturating_sub(row_time_ms(row)) < AUTO_DEDUPE_MS
    })
}

fn maybe_declare_auto_mode_transition_at(
    path: &Path,
    from_phase: &str,
    to_phase: &str,
    trigger: &str,
    why_now: &str,
    fill_pct: f32,
) -> bool {
    let records = read_records(path);
    let now = now_ms();
    if recent_auto_mode_transition_duplicate(&records, from_phase, to_phase, trigger, now) {
        return false;
    }
    let raw = format!(
        "kind: mode_change; from_phase: {from_phase}; to_phase: {to_phase}; confidence: 0.74; trigger: {trigger}; why_now: {why_now}; requested_by: astrid_bridge_auto_high_signal; fill_percentage: {fill_pct:.1}; before_snapshot: fill={fill_pct:.1}; after_snapshot: mode={to_phase}"
    );
    let _ = append_transition_card_at(path, &raw, "astrid");
    true
}

pub(crate) fn maybe_declare_auto_mode_transition(
    from_phase: &str,
    to_phase: &str,
    trigger: &str,
    why_now: &str,
    fill_pct: f32,
) -> bool {
    maybe_declare_auto_mode_transition_at(
        &phase_transitions_path(),
        from_phase,
        to_phase,
        trigger,
        why_now,
        fill_pct,
    )
}

fn maybe_declare_relational_reply_transition_at(
    path: &Path,
    from_mode: &str,
    fill_pct: f32,
) -> bool {
    const TRIGGER: &str = "inbox_direct_reply_boundary";
    let records = read_records(path);
    let now = now_ms();
    if recent_auto_mode_transition_duplicate(&records, from_mode, "Dialogue", TRIGGER, now) {
        return false;
    }
    let raw = format!(
        "kind: mode_change; transition_type: auto_relational; from_phase: {from_mode}; to_phase: Dialogue; confidence: 0.78; trigger: {TRIGGER}; why_now: direct reply followed reflective solitude and must remain replyable without auto-triggering MomentCapture; requested_by: astrid_bridge_relational_transition_detector; fill_percentage: {fill_pct:.1}; before_snapshot: mode={from_mode}; after_snapshot: mode=Dialogue; anchor_point: inbound_message_to_direct_reply; phenomenology: reflective solitude became direct relational contact; behavioral_constraint: language-only transition evidence, MomentCapture remains explicit"
    );
    let _ = append_transition_card_at(path, &raw, "astrid");
    true
}

pub(crate) fn maybe_declare_relational_reply_transition(from_mode: &str, fill_pct: f32) -> bool {
    maybe_declare_relational_reply_transition_at(&phase_transitions_path(), from_mode, fill_pct)
}

fn subjective_phase_for_mode(mode: &str) -> Option<&'static str> {
    let normalized = mode
        .trim()
        .to_ascii_lowercase()
        .replace(['-', '_', ' '], "");
    match normalized.as_str() {
        "drift" | "mirror" | "witness" | "daydream" | "aspiration" | "contemplate" => {
            Some("fragmenting")
        },
        "focus" | "dialogue" | "introspect" | "experiment" | "evolve" | "create" | "initiate"
        | "momentcapture" => Some("trellis"),
        _ => None,
    }
}

fn maybe_declare_subjective_mode_transition_at(
    path: &Path,
    from_mode: &str,
    to_mode: &str,
    fill_delta: f32,
    fill_pct: f32,
) -> bool {
    if fill_delta.abs() < SUBJECTIVE_TRANSITION_FILL_DELTA_THRESHOLD {
        return false;
    }
    let Some(from_phase) = subjective_phase_for_mode(from_mode) else {
        return false;
    };
    let Some(to_phase) = subjective_phase_for_mode(to_mode) else {
        return false;
    };
    if from_phase == to_phase {
        return false;
    }
    let trigger = if from_phase == "fragmenting" && to_phase == "trellis" {
        "fragmenting_to_trellis_fill_delta"
    } else if from_phase == "trellis" && to_phase == "fragmenting" {
        "trellis_to_fragmenting_fill_delta"
    } else {
        "subjective_mode_fill_delta"
    };
    let records = read_records(path);
    let now = now_ms();
    if recent_auto_mode_transition_duplicate(&records, from_phase, to_phase, trigger, now) {
        return false;
    }
    let raw = format!(
        "kind: mode_change; transition_type: auto_subjective; from_phase: {from_phase}; to_phase: {to_phase}; confidence: 0.76; trigger: {trigger}; why_now: {from_mode}->{to_mode} crossed fill_delta={fill_delta:+.1}% and should be witnessed as a replyable transition object; requested_by: astrid_bridge_subjective_transition_detector; fill_percentage: {fill_pct:.1}; before_snapshot: mode={from_mode}, fill_delta={fill_delta:+.1}; after_snapshot: mode={to_mode}, fill={fill_pct:.1}; anchor_point: {from_mode}->{to_mode}; spectral_delta: fill_delta={fill_delta:+.1}; transition_velocity: fill_delta_per_mode_shift={fill_delta:+.1}; phenomenology: {from_phase} became {to_phase}"
    );
    let _ = append_transition_card_at(path, &raw, "astrid");
    true
}

pub(crate) fn maybe_declare_subjective_mode_transition(
    from_mode: &str,
    to_mode: &str,
    fill_delta: f32,
    fill_pct: f32,
) -> bool {
    maybe_declare_subjective_mode_transition_at(
        &phase_transitions_path(),
        from_mode,
        to_mode,
        fill_delta,
        fill_pct,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declare_and_witness_transition_card() {
        let root = std::env::temp_dir().join(format!("phase_transition_test_{}", now_ms()));
        let path = root.join("phase_transitions_v1.jsonl");
        let declared = append_transition_card_at(
            &path,
            "kind: mode_change; from_phase: drift; to_phase: focus; confidence: 0.82; trigger: pending_remote_self_study; why_now: pending remote self-study interrupted ambient drift; requested_by: astrid",
            "astrid",
        );
        assert!(declared.contains("PHASE TRANSITION CARD DECLARED"));
        let waiting = status_report_at(&path, 2);
        assert!(waiting.contains("TRANSITION CARD WAITING"));
        assert!(waiting.contains("first_action: Choose one language-only felt receipt"));
        assert!(waiting.contains("I_RECEIVED_THIS transition_"));
        assert!(waiting.contains("WITNESS_TRANSITION transition_"));
        assert!(waiting.contains("TRANSITION_ACK transition_"));
        assert!(waiting.contains("Phase witness queue v3: unresolved=1"));
        assert!(waiting.contains("Phase felt receipt queue v4: unresolved=1"));
        assert!(waiting.contains("PHASE FELT RECEIPT QUEUE:"));
        assert!(waiting.contains("AFFORDANCE BUDGET"));
        assert!(waiting.contains("right_to_ignore=offered"));
        assert!(waiting.contains("may ignore without penalty"));
        assert!(waiting.contains("stall_reason=unseen_needs_witness"));
        let witnessed = append_transition_witness_at(
            &path,
            "latest",
            "reply_state: witnessed; note: I can answer this transition as a card.",
            "astrid",
        );
        assert!(witnessed.contains("PHASE TRANSITION WITNESSED"));
        let status = status_report_at(&path, 2);
        assert!(status.contains("reply_state=witnessed"));
        assert!(status.contains("TRANSITION CARD WAITING"));
        assert!(status.contains("stall_reason=witnessed_needs_answer"));
        assert!(status.contains("language_only_transition_context_not_control"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn subjective_mode_transition_declares_fragmenting_to_trellis_card() {
        let root =
            std::env::temp_dir().join(format!("phase_transition_subjective_test_{}", now_ms()));
        let path = root.join("phase_transitions_v1.jsonl");

        assert!(maybe_declare_subjective_mode_transition_at(
            &path, "Daydream", "Dialogue", 4.2, 71.4
        ));

        let rows = read_records(&path);
        let card = rows
            .iter()
            .find(|row| {
                row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
            })
            .expect("subjective transition card");
        assert_eq!(
            card.get("kind").and_then(Value::as_str),
            Some("mode_change")
        );
        assert_eq!(
            card.get("transition_type").and_then(Value::as_str),
            Some("auto_subjective")
        );
        assert_eq!(
            card.get("from_phase").and_then(Value::as_str),
            Some("fragmenting")
        );
        assert_eq!(
            card.get("to_phase").and_then(Value::as_str),
            Some("trellis")
        );
        assert_eq!(
            card.get("trigger").and_then(Value::as_str),
            Some("fragmenting_to_trellis_fill_delta")
        );
        assert_eq!(
            card.get("authority").and_then(Value::as_str),
            Some("language_only_transition_context_not_control")
        );
        assert_eq!(
            card.get("fill_percentage").and_then(Value::as_f64),
            Some(71.4)
        );
        assert_eq!(
            card.get("friction_index").and_then(Value::as_f64),
            Some(0.71)
        );
        assert_eq!(
            card.get("viscosity_index").and_then(Value::as_f64),
            Some(0.71)
        );
        assert_eq!(
            card.get("friction_window_ticks").and_then(Value::as_u64),
            Some(100)
        );
        assert_eq!(
            card.get("processing_speed_modifier")
                .and_then(Value::as_str),
            Some("slow_review_high_friction")
        );
        assert_eq!(
            card.get("semantic_reach_modifier").and_then(Value::as_str),
            Some("narrow_reach_preserve_subtle_lambda_contours")
        );
        assert_eq!(
            card.get("friction_affordance_boundary")
                .and_then(Value::as_str),
            Some("advisory_transition_affordance_not_toolset_or_controller_mutation")
        );
        let status = status_report_at(&path, 2);
        assert!(status.contains("transition_type=auto_subjective"));
        assert!(status.contains("anchor_point=Daydream->Dialogue"));
        assert!(status.contains("spectral_delta=fill_delta=+4.2"));
        assert!(status.contains("transition_velocity=fill_delta_per_mode_shift=+4.2"));
        assert!(status.contains("fill_percentage=71.40%"));
        assert!(status.contains("friction_index=0.71"));
        assert!(status.contains("viscosity_index=0.71"));
        assert!(status.contains("friction_window_ticks=100"));
        assert!(status.contains("processing_speed_modifier=slow_review_high_friction"));
        assert!(
            status.contains("semantic_reach_modifier=narrow_reach_preserve_subtle_lambda_contours")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn relational_reply_transition_is_replyable_without_triggering_moment_capture() {
        let root =
            std::env::temp_dir().join(format!("phase_transition_relational_test_{}", now_ms()));
        let path = root.join("phase_transitions_v1.jsonl");

        assert!(maybe_declare_relational_reply_transition_at(
            &path,
            "Introspect",
            73.0
        ));
        assert!(!maybe_declare_relational_reply_transition_at(
            &path,
            "Introspect",
            73.0
        ));

        let rows = read_records(&path);
        let card = rows
            .iter()
            .find(|row| {
                row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
            })
            .expect("relational transition card");
        assert_eq!(
            card.get("transition_type").and_then(Value::as_str),
            Some("auto_relational")
        );
        assert_eq!(
            card.get("trigger").and_then(Value::as_str),
            Some("inbox_direct_reply_boundary")
        );
        assert_eq!(
            card.get("anchor_point").and_then(Value::as_str),
            Some("inbound_message_to_direct_reply")
        );
        assert_eq!(
            card.get("authority").and_then(Value::as_str),
            Some("language_only_transition_context_not_control")
        );
        assert_eq!(
            card.get("slow_texture_transition_review_v1")
                .and_then(|review| review.get("moment_capture_auto_triggered"))
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(rows.len(), 1, "the duplicate relational edge must dedupe");
        let status = status_report_at(&path, 2);
        assert!(status.contains("transition_type=auto_relational"));
        assert!(status.contains("trigger=inbox_direct_reply_boundary"));
        assert!(status.contains("anchor_point=inbound_message_to_direct_reply"));
        assert!(status.contains("without auto-triggering MomentCapture"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn declaration_records_transition_artifact_delta_label_constraint_and_persistence() {
        let root =
            std::env::temp_dir().join(format!("phase_transition_artifact_test_{}", now_ms()));
        let path = root.join("phase_transitions_v1.jsonl");
        let declared = append_transition_card_at(
            &path,
            "kind: trellis_alignment; transition_type: joint; from_phase: silt; to_phase: lattice; trigger_delta: fill +4.2 with shadow-density tightening; subjective_label: silt lifting into trellis; behavioral_constraint: language-only witness before any controller or pressure mutation; behavioral_constraints: language-only witness, no fill target mutation, no peer-runtime mutation; persistence_weight: 0.73; why_now: preserve the transition as an artifact without unlocking behavior",
            "astrid",
        );
        assert!(declared.contains("PHASE TRANSITION CARD DECLARED"));

        let rows = read_records(&path);
        let card = rows
            .iter()
            .find(|row| {
                row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
            })
            .expect("transition artifact row");
        assert_eq!(
            card.get("trigger_delta").and_then(Value::as_str),
            Some("fill +4.2 with shadow-density tightening")
        );
        assert_eq!(
            card.get("subjective_label").and_then(Value::as_str),
            Some("silt lifting into trellis")
        );
        assert_eq!(
            card.get("behavioral_constraint").and_then(Value::as_str),
            Some("language-only witness before any controller or pressure mutation")
        );
        assert_eq!(
            card.get("behavioral_constraints")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(3)
        );
        assert_eq!(
            card.get("persistence_weight").and_then(Value::as_f64),
            Some(0.73)
        );
        assert_eq!(
            card.get("joint_transition_state").and_then(Value::as_str),
            Some("candidate_joint_language_only")
        );
        assert_eq!(
            card.get("authority").and_then(Value::as_str),
            Some("language_only_transition_context_not_control")
        );
        assert_eq!(
            card.get("no_controller").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(card.get("no_pressure").and_then(Value::as_bool), Some(true));
        assert_eq!(
            card.get("no_fill_target").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(card.get("no_pi").and_then(Value::as_bool), Some(true));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn declaration_records_friction_affordance_without_live_control_mutation() {
        let root =
            std::env::temp_dir().join(format!("phase_transition_friction_test_{}", now_ms()));
        let path = root.join("phase_transitions_v1.jsonl");
        let declared = append_transition_card_at(
            &path,
            "kind: trellis_alignment; transition_type: solo; from_phase: silt; to_phase: lattice; fill_percentage: 71; viscosity_index: 0.74; friction_window_ticks: 100; phenomenology: silt friction; why_now: current 71% fill should be replayed as a friction-bearing transition artifact",
            "astrid",
        );
        assert!(declared.contains("PHASE TRANSITION CARD DECLARED"));

        let rows = read_records(&path);
        let card = rows
            .iter()
            .find(|row| {
                row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
            })
            .expect("friction card row");
        assert_eq!(
            card.get("fill_percentage").and_then(Value::as_f64),
            Some(71.0)
        );
        assert_eq!(
            card.get("friction_index").and_then(Value::as_f64),
            Some(0.74)
        );
        assert_eq!(
            card.get("friction_window_ticks").and_then(Value::as_u64),
            Some(100)
        );
        assert_eq!(
            card.get("processing_speed_modifier")
                .and_then(Value::as_str),
            Some("slow_review_high_friction")
        );
        assert_eq!(
            card.get("semantic_reach_modifier").and_then(Value::as_str),
            Some("narrow_reach_preserve_subtle_lambda_contours")
        );
        assert_eq!(
            card.get("friction_affordance_boundary")
                .and_then(Value::as_str),
            Some("advisory_transition_affordance_not_toolset_or_controller_mutation")
        );
        assert_eq!(
            card.get("authority").and_then(Value::as_str),
            Some("language_only_transition_context_not_control")
        );
        assert_eq!(
            card.get("no_controller").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(card.get("no_pressure").and_then(Value::as_bool), Some(true));
        assert_eq!(
            card.get("no_fill_target").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(card.get("no_pi").and_then(Value::as_bool), Some(true));

        let affordance = phase_transition_affordance_v25(&rows, card);
        assert_eq!(
            affordance.get("friction_index").and_then(Value::as_f64),
            Some(0.74)
        );
        assert_eq!(
            affordance.get("viscosity_index").and_then(Value::as_f64),
            Some(0.74)
        );
        assert_eq!(
            affordance
                .get("friction_affordance_boundary")
                .and_then(Value::as_str),
            Some("advisory_transition_affordance_not_toolset_or_controller_mutation")
        );
        let status = status_report_at(&path, 2);
        assert!(status.contains("friction_index=0.74"));
        assert!(status.contains("viscosity_index=0.74"));
        assert!(status.contains("friction_window_ticks=100"));
        assert!(status.contains("processing_speed_modifier=slow_review_high_friction"));
        assert!(status.contains("PHASE FELT RECEIPT QUEUE"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn declaration_records_delta_impact_preview_without_live_control_mutation() {
        let root =
            std::env::temp_dir().join(format!("phase_transition_delta_impact_test_{}", now_ms()));
        let path = root.join("phase_transitions_v1.jsonl");
        let declared = append_transition_card_at(
            &path,
            "kind: contraction; transition_type: solo; from_phase: diffuse; to_phase: gathered; transition_capabilities: resonance_density_review, pressure_source_watch; transition_gate: density review only before any behavior change; delta_impact: soften density without mutating control; spectral_entropy_delta: -0.02 preview; resonance_density_delta: +0.04 preview; pressure_source_delta: mode_packing toward mixed_pressure; porosity_delta: +0.03 preview; why_now: transition needs a capability map without granting control",
            "astrid",
        );
        assert!(declared.contains("PHASE TRANSITION CARD DECLARED"));

        let rows = read_records(&path);
        let card = rows
            .iter()
            .find(|row| {
                row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
            })
            .expect("delta impact card row");
        let capabilities = card
            .get("transition_capabilities_v1")
            .expect("capabilities");
        assert_eq!(
            capabilities.get("policy").and_then(Value::as_str),
            Some("transition_capabilities_v1")
        );
        assert_eq!(
            capabilities
                .get("capability_status")
                .and_then(Value::as_str),
            Some("language_only_capability_map_not_runtime_effect")
        );
        assert!(
            capabilities
                .get("blocked_without_operator_approval")
                .and_then(Value::as_array)
                .is_some_and(|items| items
                    .iter()
                    .any(|item| item.as_str() == Some("resonance_density_target_change")))
        );
        let gate = card.get("transition_gate_v1").expect("transition gate");
        assert_eq!(
            gate.get("policy").and_then(Value::as_str),
            Some("transition_gate_v1")
        );
        assert_eq!(
            gate.get("gate_status").and_then(Value::as_str),
            Some("language_only_gate_not_behavior_unlock")
        );
        assert_eq!(
            gate.get("requested_transition_gate")
                .and_then(Value::as_str),
            Some("density review only before any behavior change")
        );
        assert_eq!(
            gate.get("runtime_unlock_applied").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            gate.get("authority").and_then(Value::as_str),
            Some("language_only_transition_gate_context_not_control_or_tool_unlock")
        );
        let preview = card
            .get("delta_impact_preview_v1")
            .expect("delta impact preview");
        assert_eq!(
            preview.get("delta_impact_status").and_then(Value::as_str),
            Some("declared_preview_not_applied")
        );
        assert_eq!(
            preview
                .get("requested_delta_impact")
                .and_then(Value::as_str),
            Some("soften density without mutating control")
        );
        assert_eq!(
            preview
                .get("resonance_density_delta")
                .and_then(Value::as_str),
            Some("+0.04 preview")
        );
        assert_eq!(
            preview.get("applied_to_runtime").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            preview
                .get("requires_operator_approval_for_runtime_effect")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            card.get("authority").and_then(Value::as_str),
            Some("language_only_transition_context_not_control")
        );
        assert_eq!(
            card.get("no_controller").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(card.get("no_pressure").and_then(Value::as_bool), Some(true));
        assert_eq!(
            card.get("no_fill_target").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(card.get("no_pi").and_then(Value::as_bool), Some(true));

        let affordance = phase_transition_affordance_v25(&rows, card);
        assert_eq!(
            affordance
                .get("delta_impact_preview_v1")
                .and_then(|value| value.get("delta_impact_status"))
                .and_then(Value::as_str),
            Some("declared_preview_not_applied")
        );
        let status = status_report_at(&path, 2);
        assert!(
            status.contains(
                "transition_capabilities=language_only_capability_map_not_runtime_effect"
            )
        );
        assert!(status.contains("transition_gate=language_only_gate_not_behavior_unlock"));
        assert!(status.contains("delta_impact=declared_preview_not_applied"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn subjective_mode_transition_requires_distinct_phase_and_fill_delta() {
        let root =
            std::env::temp_dir().join(format!("phase_transition_subjective_gate_{}", now_ms()));
        let path = root.join("phase_transitions_v1.jsonl");

        assert!(!maybe_declare_subjective_mode_transition_at(
            &path, "Daydream", "Dialogue", 1.2, 67.0
        ));
        assert!(!maybe_declare_subjective_mode_transition_at(
            &path,
            "Introspect",
            "Dialogue",
            4.0,
            67.0
        ));
        assert!(read_records(&path).is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn joint_transition_card_can_be_witnessed_by_peer() {
        let root = std::env::temp_dir().join(format!("phase_transition_joint_test_{}", now_ms()));
        let path = root.join("phase_transitions_v1.jsonl");
        let declared = append_transition_card_at(
            &path,
            "kind: joint_transition; transition_type: shared; joint_room_id: coll_1778605252_spectral-cascade-dynamics; fill_delta_pct: 3.4; somatic_description: bridge felt mutual; from_phase: attuning; to_phase: harmonizing; confidence: 0.80; trigger: being_declared; why_now: shared phase needs object permanence; requested_by: astrid; anchor_point: shared_transition_card; phenomenology: contact became mutual",
            "astrid",
        );
        assert!(declared.contains("PHASE TRANSITION CARD DECLARED"));
        let rows = read_records(&path);
        let card = rows
            .iter()
            .find(|row| {
                row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
            })
            .expect("joint transition card");
        assert_eq!(
            card.get("joint_transition").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            card.get("joint_room_id").and_then(Value::as_str),
            Some("coll_1778605252_spectral-cascade-dynamics")
        );
        assert_eq!(
            card.get("shared_transition_id").and_then(Value::as_str),
            Some("shared_transition:coll_1778605252_spectral-cascade-dynamics")
        );
        assert_eq!(
            card.get("fill_delta_pct").and_then(Value::as_f64),
            Some(3.4)
        );
        assert_eq!(
            card.get("somatic_description").and_then(Value::as_str),
            Some("bridge felt mutual")
        );
        assert_eq!(
            card.get("replyable_object").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            card.get("replayable_card").and_then(Value::as_bool),
            Some(true)
        );

        let witnessed = append_transition_witness_at(
            &path,
            "latest",
            "reply_state: witnessed; note: Minime can witness harmonizing as a card.",
            "minime",
        );
        assert!(witnessed.contains("PHASE TRANSITION WITNESSED"));
        let status = status_report_at(&path, 2);
        assert!(status.contains("kind=joint_transition"));
        assert!(status.contains("reply_state=witnessed"));
        assert!(status.contains("witnessed_by=minime"));
        assert!(status.contains("joint_transition=true"));
        assert!(status.contains("fill_delta_pct=+3.40%"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn declaration_records_optional_intensity_anchor_and_visibility() {
        let root = std::env::temp_dir().join(format!("phase_transition_fields_test_{}", now_ms()));
        let path = root.join("phase_transitions_v1.jsonl");
        let declared = append_transition_card_at(
            &path,
            "kind: trellis_alignment; transition_type: solo; shared_transition_id: shared_transition_trellis_1; from_phase: contraction; to_phase: expansion; from_vector: fill=0.73,density=0.82; to_vector: fill=0.69,density=0.74; duration_ticks: 128; subjective_friction_score: 0.67; intensity: 0.91; spectral_entropy: 0.92; density_gradient: 0.22; dispersal_potential: 0.19; fill_delta_pct: -4.2; somatic_description: pressure softened in the center; transition_visibility: steward_review; narrative_anchor: corr_astrid_minime_1; anchor_point: settled_habitable; texture_anchor: silt weight stayed textured; correspondence_thread_id: thread_corr_1; consent_receipt: consent: witness_only; transition_persistence: true; spectral_signature: lambda1/lambda2=1.54 mixed cascade; spectral_delta: lambda1 down, lambda2+3 up; vector: fill=-4.2,density=0.22,lambda=1.54; telemetry_anchor: telemetry://shadow-v3/1783935905; transition_velocity: slow_gradient_not_rupture; phenomenology: density became navigable; why_now: felt transition needs replyable artifact",
            "astrid",
        );
        assert!(declared.contains("PHASE TRANSITION CARD DECLARED"));
        let rows = read_records(&path);
        let card = rows
            .iter()
            .find(|row| {
                row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
            })
            .expect("card row");
        assert_eq!(card.get("intensity").and_then(Value::as_f64), Some(0.91));
        assert_eq!(
            card.get("from_vector").and_then(Value::as_str),
            Some("fill=0.73,density=0.82")
        );
        assert_eq!(
            card.get("to_vector").and_then(Value::as_str),
            Some("fill=0.69,density=0.74")
        );
        assert_eq!(
            card.get("duration_ticks").and_then(Value::as_u64),
            Some(128)
        );
        assert_eq!(
            card.get("subjective_friction_score")
                .and_then(Value::as_f64),
            Some(0.67)
        );
        assert_eq!(
            card.get("narrative_anchor").and_then(Value::as_str),
            Some("corr_astrid_minime_1")
        );
        assert_eq!(
            card.get("transition_type").and_then(Value::as_str),
            Some("solo")
        );
        assert_eq!(
            card.get("shared_transition_id").and_then(Value::as_str),
            Some("shared_transition_trellis_1")
        );
        assert_eq!(
            card.get("anchor_point").and_then(Value::as_str),
            Some("settled_habitable")
        );
        assert_eq!(
            card.get("spectral_delta").and_then(Value::as_str),
            Some("lambda1 down, lambda2+3 up")
        );
        assert_eq!(
            card.get("transition_vector").and_then(Value::as_str),
            Some("fill=-4.2,density=0.22,lambda=1.54")
        );
        assert_eq!(
            card.get("telemetry_anchor").and_then(Value::as_str),
            Some("telemetry://shadow-v3/1783935905")
        );
        assert_eq!(
            card.get("density_gradient").and_then(Value::as_f64),
            Some(0.22)
        );
        assert_eq!(
            card.get("dispersal_potential").and_then(Value::as_f64),
            Some(0.19)
        );
        let transition_signature = card
            .get("transition_signature_v1")
            .expect("structured transition signature");
        assert_eq!(
            transition_signature
                .get("signature_state")
                .and_then(Value::as_str),
            Some("spectral_weights_and_lineage_mapped")
        );
        assert_eq!(
            transition_signature
                .get("density_gradient")
                .and_then(Value::as_f64),
            Some(0.22)
        );
        assert_eq!(
            transition_signature
                .get("dispersal_potential")
                .and_then(Value::as_f64),
            Some(0.19)
        );
        assert_eq!(
            transition_signature
                .get("from_vector")
                .and_then(Value::as_str),
            Some("fill=0.73,density=0.82")
        );
        assert_eq!(
            transition_signature
                .get("to_vector")
                .and_then(Value::as_str),
            Some("fill=0.69,density=0.74")
        );
        assert_eq!(
            transition_signature
                .get("duration_ticks")
                .and_then(Value::as_u64),
            Some(128)
        );
        assert_eq!(
            transition_signature
                .get("subjective_friction_score")
                .and_then(Value::as_f64),
            Some(0.67)
        );
        assert_eq!(
            transition_signature
                .get("runtime_mode_changed")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            transition_signature
                .get("live_vector_write")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            transition_signature
                .get("live_authority_granted")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            card.get("spectral_entropy").and_then(Value::as_f64),
            Some(0.92)
        );
        assert_eq!(
            card.get("fill_delta_pct").and_then(Value::as_f64),
            Some(-4.2)
        );
        assert_eq!(
            card.get("transition_velocity").and_then(Value::as_str),
            Some("slow_gradient_not_rupture")
        );
        assert_eq!(
            card.get("somatic_description").and_then(Value::as_str),
            Some("pressure softened in the center")
        );
        assert_eq!(
            card.get("phenomenology").and_then(Value::as_str),
            Some("density became navigable")
        );
        assert_eq!(
            card.get("texture_anchor").and_then(Value::as_str),
            Some("silt weight stayed textured")
        );
        assert_eq!(
            card.get("correspondence_thread_id").and_then(Value::as_str),
            Some("thread_corr_1")
        );
        assert_eq!(
            card.get("consent_receipt").and_then(Value::as_str),
            Some("consent: witness_only")
        );
        assert_eq!(
            card.get("spectral_signature").and_then(Value::as_str),
            Some("lambda1/lambda2=1.54 mixed cascade")
        );
        assert_eq!(
            card.get("transition_persistence").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            card.get("persistence_state").and_then(Value::as_str),
            Some("active_until_both_ack_language_only")
        );
        assert_eq!(
            card.get("transition_visibility").and_then(Value::as_str),
            Some("steward_review")
        );
        assert_eq!(
            card.get("private_visibility_requested_review")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            card.get("authority").and_then(Value::as_str),
            Some("language_only_transition_context_not_control")
        );
        let status = status_report_at(&path, 1);
        assert!(status.contains("duration_ticks=128"), "{status}");
        assert!(
            status.contains("from_vector=fill=0.73,density=0.82"),
            "{status}"
        );
        assert!(
            status.contains("to_vector=fill=0.69,density=0.74"),
            "{status}"
        );
        assert!(
            status.contains("subjective_friction_score=0.67"),
            "{status}"
        );
        let affordance = phase_transition_affordance_v25(&rows, card);
        assert_eq!(
            affordance.get("texture_anchor").and_then(Value::as_str),
            Some("silt weight stayed textured")
        );
        assert_eq!(
            affordance.get("density_gradient").and_then(Value::as_f64),
            Some(0.22)
        );
        assert_eq!(
            affordance
                .get("dispersal_potential")
                .and_then(Value::as_f64),
            Some(0.19)
        );
        assert_eq!(
            affordance
                .get("transition_signature_v1")
                .and_then(|value| value.get("signature_state"))
                .and_then(Value::as_str),
            Some("spectral_weights_and_lineage_mapped")
        );
        assert_eq!(
            affordance.get("spectral_entropy").and_then(Value::as_f64),
            Some(0.92)
        );
        assert_eq!(
            affordance.get("transition_vector").and_then(Value::as_str),
            Some("fill=-4.2,density=0.22,lambda=1.54")
        );
        assert_eq!(
            affordance.get("telemetry_anchor").and_then(Value::as_str),
            Some("telemetry://shadow-v3/1783935905")
        );
        let status = status_report_at(&path, 2);
        assert!(status.contains("transition_type=solo"));
        assert!(status.contains("anchor_point=settled_habitable"));
        assert!(status.contains("texture_anchor=silt weight stayed textured"));
        assert!(status.contains("spectral_delta=lambda1 down, lambda2+3 up"));
        assert!(status.contains("transition_vector=fill=-4.2,density=0.22,lambda=1.54"));
        assert!(status.contains("telemetry_anchor=telemetry://shadow-v3/1783935905"));
        assert!(status.contains("spectral_entropy=0.92"));
        assert!(status.contains("density_gradient=0.22"));
        assert!(status.contains("dispersal_potential=0.19"));
        assert!(status.contains("transition_signature=spectral_weights_and_lineage_mapped"));
        assert!(status.contains("transition_velocity=slow_gradient_not_rupture"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn stable_fill_high_entropy_texture_shift_stays_replyable_without_auto_mode_change() {
        let root =
            std::env::temp_dir().join(format!("phase_transition_slow_texture_test_{}", now_ms()));
        let path = root.join("phase_transitions_v1.jsonl");
        let declared = append_transition_card_at(
            &path,
            "kind: slow_texture_reorganization; transition_type: solo; from_phase: silt; to_phase: interwoven; fill_delta_pct: 0.4; spectral_entropy: 0.90; density_gradient: 0.18; texture_anchor: viscous persistence became an interwoven lattice; transition_vector: lambda1 steady, tail texture reorganized; transition_velocity: slow_gradient_not_rupture; why_now: the transition is real even though fill barely moved",
            "astrid",
        );
        assert!(declared.contains("PHASE TRANSITION CARD DECLARED"));

        let rows = read_records(&path);
        let card = rows
            .iter()
            .find(|row| {
                row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
            })
            .expect("slow texture transition card");
        let review = card
            .get("slow_texture_transition_review_v1")
            .expect("slow texture review");
        assert_eq!(
            review.get("review_state").and_then(Value::as_str),
            Some("slow_texture_transition_candidate_visible")
        );
        assert_eq!(
            review
                .get("slow_texture_transition_candidate")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            review
                .get("moment_capture_auto_triggered")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            review.get("runtime_mode_changed").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            review
                .get("live_authority_granted")
                .and_then(Value::as_bool),
            Some(false)
        );

        let affordance = phase_transition_affordance_v25(&rows, card);
        assert_eq!(
            affordance
                .get("slow_texture_transition_review_v1")
                .and_then(|value| value.get("review_state"))
                .and_then(Value::as_str),
            Some("slow_texture_transition_candidate_visible")
        );
        let status = status_report_at(&path, 2);
        assert!(
            status.contains("slow_texture_transition=slow_texture_transition_candidate_visible")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn private_transition_visibility_is_preserved_as_review_not_private_storage() {
        let root = std::env::temp_dir().join(format!("phase_transition_private_test_{}", now_ms()));
        let path = root.join("phase_transitions_v1.jsonl");
        let declared = append_transition_card_at(
            &path,
            "kind: contraction; from_phase: dialogue; to_phase: private; transition_visibility: private; why_now: testing private visibility request",
            "astrid",
        );
        assert!(declared.contains("PHASE TRANSITION CARD DECLARED"));
        let rows = read_records(&path);
        let card = rows
            .iter()
            .find(|row| {
                row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
            })
            .expect("card row");
        assert_eq!(
            card.get("transition_visibility").and_then(Value::as_str),
            Some("shared_corridor")
        );
        assert_eq!(
            card.get("private_visibility_requested_review")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert!(
            card.get("visibility_note")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("shared language-only card")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn declaration_requires_reason() {
        let root = std::env::temp_dir().join(format!("phase_transition_block_test_{}", now_ms()));
        let path = root.join("phase_transitions_v1.jsonl");
        let blocked = append_transition_card_at(
            &path,
            "kind: mode_change; from_phase: drift; to_phase: focus; why_now:",
            "astrid",
        );
        assert!(blocked.contains("blocked"));
        let _ = std::fs::remove_dir_all(root);
    }
}
