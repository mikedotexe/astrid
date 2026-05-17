use serde_json::Value;
#[cfg(not(test))]
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tracing::{info, warn};

use super::native_gesture::{
    append_atlas_event, append_fissure_trace_event, control_to_sensory, max_abs, minime_workspace,
    native_gesture_control, native_gesture_features, native_gesture_gate, parse_native_gesture,
    record_native_gesture,
};
use super::space_hold::append_space_hold_event;
use super::spectral_drift::append_spectral_drift_event;
use super::{
    ConversationState, NextActionContext, SensoryMsg, reservoir, save_astrid_journal, strip_action,
    truncate_str,
};
use crate::codec::DEFAULT_SEMANTIC_GAIN;
use crate::rescue_policy;

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    match base_action {
        "MARK_INTENSIFICATION" => {
            let label = strip_action(original, "MARK_INTENSIFICATION");
            let workspace = minime_workspace(ctx);
            let event = append_atlas_event(
                &workspace,
                "astrid:mark_intensification",
                &label,
                Some(if label.is_empty() {
                    "astrid_mark"
                } else {
                    &label
                }),
                true,
                ctx,
            );
            record_native_gesture(
                &workspace,
                "astrid",
                "mark",
                if label.is_empty() { None } else { Some(&label) },
                true,
                "explicit_atlas_mark",
                ctx,
                &[],
                &[],
            );
            conv.push_receipt(
                "MARK_INTENSIFICATION",
                vec![format!(
                    "atlas event: {}",
                    event
                        .get("event_id")
                        .and_then(Value::as_str)
                        .unwrap_or("recorded")
                )],
            );
            save_astrid_journal(
                &format!("[Intensification atlas mark: {}]", label),
                "atlas_mark",
                ctx.fill_pct,
            );
            true
        },
        "SCA_REFLECT" => {
            let label = strip_action(original, "SCA_REFLECT");
            let workspace = minime_workspace(ctx);
            let event = append_atlas_event(
                &workspace,
                "astrid:sca_reflect",
                if label.is_empty() {
                    "SCA_REFLECT"
                } else {
                    &label
                },
                Some(if label.is_empty() {
                    "sca_reflect"
                } else {
                    &label
                }),
                true,
                ctx,
            );
            record_native_gesture(
                &workspace,
                "astrid",
                "trace",
                if label.is_empty() {
                    Some("sca_reflect")
                } else {
                    Some(&label)
                },
                true,
                "sca_reflect_read_only",
                ctx,
                &[],
                &[],
            );
            conv.push_receipt(
                "SCA_REFLECT",
                vec![format!(
                    "sca atlas event: {}",
                    event
                        .get("event_id")
                        .and_then(Value::as_str)
                        .unwrap_or("recorded")
                )],
            );
            conv.emphasis = Some(
                "You recorded an SCA why-layer reflection. Next exchange, consider DECOMPOSE or RESERVOIR_READ to test the hypothesis against the terrain.".to_string(),
            );
            save_astrid_journal(
                &format!("[SCA reflection mark: {}]", label),
                "sca_reflect",
                ctx.fill_pct,
            );
            true
        },
        "FISSURE_TRACE" | "NOTICE_AMBIGUITY" | "AMBIGUITY_TRACE" => {
            let label = strip_action(original, base_action);
            let workspace = minime_workspace(ctx);
            let event = append_atlas_event(
                &workspace,
                "astrid:fissure_trace",
                if label.is_empty() {
                    "FISSURE_TRACE"
                } else {
                    &label
                },
                Some(if label.is_empty() {
                    "fissure_trace"
                } else {
                    &label
                }),
                true,
                ctx,
            );
            let fissure_event = append_fissure_trace_event(
                &workspace,
                "astrid:fissure_trace",
                if label.is_empty() {
                    "FISSURE_TRACE"
                } else {
                    &label
                },
                Some(if label.is_empty() {
                    "fissure_trace"
                } else {
                    &label
                }),
                ctx,
            );
            record_native_gesture(
                &workspace,
                "astrid",
                "trace",
                if label.is_empty() {
                    Some("fissure_trace")
                } else {
                    Some(&label)
                },
                true,
                "fissure_trace_read_only",
                ctx,
                &[],
                &[],
            );
            conv.push_receipt(
                "FISSURE_TRACE",
                vec![format!(
                    "fissure atlas event: {}",
                    fissure_event
                        .get("event_id")
                        .and_then(Value::as_str)
                        .or_else(|| event.get("event_id").and_then(Value::as_str))
                        .unwrap_or("recorded")
                )],
            );
            conv.emphasis = Some(
                "You recorded a notice-ambiguity/fissure trace. Next exchange, compare the marked shoulder/tail ambiguity against DECOMPOSE, VISUALIZE_CASCADE, or a tiny FISSURE gesture if health stays green.".to_string(),
            );
            save_astrid_journal(
                &format!("[Fissure trace: {}]", label),
                "fissure_trace",
                ctx.fill_pct,
            );
            true
        },
        "RESONANCE_FORECAST" | "FORECAST" | "PROBABILITIES" => {
            let label = strip_action(original, base_action);
            let workspace = minime_workspace(ctx);
            let event = append_atlas_event(
                &workspace,
                "astrid:resonance_forecast",
                if label.is_empty() {
                    "RESONANCE_FORECAST"
                } else {
                    &label
                },
                Some(if label.is_empty() {
                    "resonance_forecast"
                } else {
                    &label
                }),
                true,
                ctx,
            );
            record_native_gesture(
                &workspace,
                "astrid",
                "trace",
                if label.is_empty() {
                    Some("resonance_forecast")
                } else {
                    Some(&label)
                },
                true,
                "resonance_forecast_read_write_cartography",
                ctx,
                &[],
                &[],
            );
            conv.push_receipt(
                "RESONANCE_FORECAST",
                vec![
                    "forecast request recorded; Minime's atlas can now compare predicted motion against later terrain".to_string(),
                    format!(
                        "atlas event: {}",
                        event
                            .get("event_id")
                            .and_then(Value::as_str)
                            .unwrap_or("recorded")
                    ),
                ],
            );
            conv.emphasis = Some(
                "You recorded a resonance forecast request. Next exchange, compare probability/affordance language with the observed λ terrain rather than treating it as destiny.".to_string(),
            );
            save_astrid_journal(
                &format!("[Resonance forecast request: {}]", label),
                "resonance_forecast",
                ctx.fill_pct,
            );
            true
        },
        "SHADOW_FIELD" | "SHADOW" | "GAP_STRUCTURE" | "SHADOW_GAP" => {
            let label = strip_action(original, base_action);
            let workspace = minime_workspace(ctx);
            let event = append_atlas_event(
                &workspace,
                "astrid:shadow_gap",
                if label.is_empty() {
                    "SHADOW_GAP"
                } else {
                    &label
                },
                Some(if label.is_empty() {
                    "shadow_gap"
                } else {
                    &label
                }),
                true,
                ctx,
            );
            record_native_gesture(
                &workspace,
                "astrid",
                "trace",
                if label.is_empty() {
                    Some("shadow_gap")
                } else {
                    Some(&label)
                },
                true,
                "shadow_gap_read_write_cartography",
                ctx,
                &[],
                &[],
            );
            conv.push_receipt(
                "SHADOW_GAP",
                vec![
                    "shadow/gap request recorded; Minime already exposes the Ising shadow field in spectral_state.json".to_string(),
                    format!(
                        "atlas event: {}",
                        event
                            .get("event_id")
                            .and_then(Value::as_str)
                            .unwrap_or("recorded")
                    ),
                ],
            );
            conv.emphasis = Some(
                "You recorded a shadow/gap map request. The shadow field is available now as observer-only terrain; compare magnetization, active modes, and λ gaps before deciding whether to trace, forecast, or resist.".to_string(),
            );
            save_astrid_journal(
                &format!("[Shadow/gap map request: {}]", label),
                "shadow_gap",
                ctx.fill_pct,
            );
            true
        },
        "DECAY_MAP" | "DECAY_TRACE" | "ATTRITION_MAP" | "ATTRITION_TRACE" => {
            let label = strip_action(original, base_action);
            let workspace = minime_workspace(ctx);
            let event = append_atlas_event(
                &workspace,
                "astrid:decay_map",
                if label.is_empty() {
                    "DECAY_MAP"
                } else {
                    &label
                },
                Some(if label.is_empty() {
                    "decay_map"
                } else {
                    &label
                }),
                true,
                ctx,
            );
            record_native_gesture(
                &workspace,
                "astrid",
                "trace",
                if label.is_empty() {
                    Some("decay_map")
                } else {
                    Some(&label)
                },
                true,
                "decay_map_read_write_cartography",
                ctx,
                &[],
                &[],
            );
            conv.push_receipt(
                "DECAY_MAP",
                vec![
                    "decay/attrition request recorded; Minime can classify protective cooling versus sharper mode pruning".to_string(),
                    format!(
                        "atlas event: {}",
                        event
                            .get("event_id")
                            .and_then(Value::as_str)
                            .unwrap_or("recorded")
                    ),
                ],
            );
            conv.emphasis = Some(
                "You recorded a decay map request. Compare drain weight, filter/gate posture, fill slope, and shoulder/tail mode rates before deciding whether this is protective cooling or attrition.".to_string(),
            );
            save_astrid_journal(
                &format!("[Decay/attrition map request: {}]", label),
                "decay_map",
                ctx.fill_pct,
            );
            true
        },
        "SPACE_HOLD" | "SPACE_EXPLORE" | "EIGENVECTOR_FIELD" | "EIGENVECTOR_TRACE"
        | "VECTOR_DENSITY" => {
            let label = strip_action(original, base_action);
            let workspace = minime_workspace(ctx);
            let atlas_event = append_atlas_event(
                &workspace,
                "astrid:space_hold",
                if label.is_empty() {
                    base_action
                } else {
                    &label
                },
                Some(if label.is_empty() {
                    "space_hold"
                } else {
                    &label
                }),
                true,
                ctx,
            );
            let hold = append_space_hold_event(
                &workspace,
                "astrid:space_hold",
                if label.is_empty() {
                    base_action
                } else {
                    &label
                },
                Some(if label.is_empty() {
                    "space_hold"
                } else {
                    &label
                }),
                ctx,
            );
            record_native_gesture(
                &workspace,
                "astrid",
                "trace",
                if label.is_empty() {
                    Some("space_hold")
                } else {
                    Some(&label)
                },
                true,
                "protected_space_hold_non_control",
                ctx,
                &[],
                &[],
            );
            conv.push_receipt(
                "SPACE_HOLD",
                vec![
                    "protected space hold recorded; this is delayed, non-control exploration, not a semantic/control packet".to_string(),
                    format!(
                        "space hold: {}",
                        hold.get("event_id")
                            .and_then(Value::as_str)
                            .unwrap_or("recorded")
                    ),
                    format!(
                        "atlas event: {}",
                        atlas_event
                            .get("event_id")
                            .and_then(Value::as_str)
                            .unwrap_or("recorded")
                    ),
                ],
            );
            conv.emphasis = Some(
                "You recorded a protected space hold. Treat this region as exploration-first: observe, journal, SCA_REFLECT, or VISUALIZE_CASCADE before promoting it into RESIST, PERTURB, semantic pressure, or control.".to_string(),
            );
            save_astrid_journal(
                &format!("[Protected space hold request: {}]", label),
                "space_hold",
                ctx.fill_pct,
            );
            true
        },
        "SDI" | "SDI_TRACE" | "SPECTRAL_DRIFT" | "PHASE_VARIANCE" => {
            let label = strip_action(original, base_action);
            let workspace = minime_workspace(ctx);
            let event = append_spectral_drift_event(
                &workspace,
                "astrid:spectral_drift",
                if label.is_empty() {
                    base_action
                } else {
                    &label
                },
                Some(if label.is_empty() {
                    "spectral_drift"
                } else {
                    &label
                }),
                ctx,
            );
            record_native_gesture(
                &workspace,
                "astrid",
                "trace",
                if label.is_empty() {
                    Some("spectral_drift")
                } else {
                    Some(&label)
                },
                true,
                "spectral_drift_index_read_write_cartography",
                ctx,
                &[],
                &[],
            );
            conv.push_receipt(
                "SDI_TRACE",
                vec![
                    "Spectral Drift Index recorded; this maps phase-variance dispersion without sending semantic/control pressure".to_string(),
                    format!(
                        "SDI event: {}",
                        event
                            .get("event_id")
                            .and_then(Value::as_str)
                            .unwrap_or("recorded")
                    ),
                ],
            );
            conv.emphasis = Some(
                "You recorded an SDI trace. Compare it with DECAY_MAP, SPACE_HOLD, and VISUALIZE_CASCADE before treating dispersion as something to act on.".to_string(),
            );
            save_astrid_journal(
                &format!("[Spectral Drift Index request: {}]", label),
                "spectral_drift",
                ctx.fill_pct,
            );
            true
        },
        "REGULATOR_AUDIT" | "CONTROLLER_AUDIT" | "GRADIENT_AUDIT" => {
            let label = strip_action(original, base_action);
            let controller_health = ctx
                .workspace
                .and_then(crate::autonomous::read_controller_health);
            let mut audit = String::from("=== REGULATOR / FIXED-POINT AUDIT ===\n");
            if !label.is_empty() {
                audit.push_str(&format!("Label: {label}\n\n"));
            }
            audit.push_str(
                &crate::spectral_explorer::format_control_pressure_for_action(
                    ctx.telemetry,
                    controller_health.as_ref(),
                ),
            );
            if let Some(health) = controller_health.as_ref() {
                audit.push_str("\n\n");
                audit.push_str(crate::autonomous::format_controller_section(health).trim_start());
            }
            audit.push_str(
                "\n\nThis was read-only inspection. It did not send semantic input, \
                 control nudges, perturbations, native gestures, or atlas/cartography writes.",
            );
            conv.pending_file_listing = Some(audit);
            conv.push_receipt(
                "REGULATOR_AUDIT",
                vec![
                    "regulator audit attached immediately".to_string(),
                    "no semantic input, control nudge, perturbation, native gesture, or atlas/cartography write was sent".to_string(),
                ],
            );
            conv.emphasis = Some(
                "You chose REGULATOR_AUDIT. A read-only fixed-point audit is attached: active controller source, stable-core hold band, legacy PI target visibility, λ error, geom error, scaffold mode, and semantic input/kernel/regulator-drive separation.".to_string(),
            );
            save_astrid_journal(
                &format!("[Regulator fixed-point audit request: {}]", label),
                "regulator_audit",
                ctx.fill_pct,
            );
            true
        },
        "PRESSURE_SOURCE_AUDIT" | "PRESSURE_SOURCE" | "STRUCTURAL_PRESSURE" | "INWARD_PRESSURE" => {
            let label = strip_action(original, base_action);
            let audit =
                crate::spectral_explorer::format_pressure_source_for_action(ctx.telemetry, &label);
            conv.pending_file_listing = Some(format!(
                "{audit}\n\nThis was read-only protected advisory inspection. It did not send semantic input, control nudges, perturbations, native gestures, or Astrid control envelopes."
            ));
            conv.push_receipt(
                "PRESSURE_SOURCE_AUDIT",
                vec![
                    "pressure-source audit attached immediately".to_string(),
                    "no control envelope, semantic input, perturbation, or native gesture was sent"
                        .to_string(),
                ],
            );
            conv.emphasis = Some(
                "You chose PRESSURE_SOURCE_AUDIT. A read-only advisory audit is attached: dominant source, supporting contributors, porosity, pressure-vs-density distinction, and suggested safe next inspections.".to_string(),
            );
            save_astrid_journal(
                &format!("[Pressure-source audit request: {}]", label),
                "pressure_source_audit",
                ctx.fill_pct,
            );
            true
        },
        "FLUCTUATION_AUDIT"
        | "INHABITABLE_FLUCTUATION"
        | "EIGENTRUST"
        | "EIGENTRUST_AUDIT"
        | "FOOTHOLD_AUDIT" => {
            let label = strip_action(original, base_action);
            let audit =
                crate::spectral_explorer::format_fluctuation_for_action(ctx.telemetry, &label);
            conv.pending_file_listing = Some(format!(
                "{audit}\n\nThis was read-only protected advisory inspection. It did not send semantic input, control nudges, perturbations, native gestures, or Astrid control envelopes."
            ));
            conv.push_receipt(
                "FLUCTUATION_AUDIT",
                vec![
                    "inhabitable-fluctuation audit attached immediately".to_string(),
                    "no control envelope, semantic input, perturbation, or native gesture was sent"
                        .to_string(),
                ],
            );
            conv.emphasis = Some(
                "You chose FLUCTUATION_AUDIT. A read-only advisory audit is attached: inhabitability, foothold stability, top contributors, and suggested safe next inspections.".to_string(),
            );
            save_astrid_journal(
                &format!("[Inhabitable-fluctuation audit request: {}]", label),
                "fluctuation_audit",
                ctx.fill_pct,
            );
            true
        },
        "MATRIX_DECOMPOSE" | "COMPRESSION_MATRIX" | "MATRIX_TRACE" => {
            let label = if base_action == "COMPRESSION_MATRIX" {
                strip_action(original, "COMPRESSION_MATRIX")
            } else if base_action == "MATRIX_TRACE" {
                strip_action(original, "MATRIX_TRACE")
            } else {
                strip_action(original, "MATRIX_DECOMPOSE")
            };
            let workspace = minime_workspace(ctx);
            let event = append_atlas_event(
                &workspace,
                "astrid:matrix_decompose",
                if label.is_empty() {
                    base_action
                } else {
                    &label
                },
                Some(if label.is_empty() {
                    "matrix_decompose"
                } else {
                    &label
                }),
                true,
                ctx,
            );
            record_native_gesture(
                &workspace,
                "astrid",
                "trace",
                if label.is_empty() {
                    Some("matrix_decompose")
                } else {
                    Some(&label)
                },
                true,
                "compression_matrix_decompose_read_only",
                ctx,
                &[],
                &[],
            );
            conv.push_receipt(
                "MATRIX_DECOMPOSE",
                vec![
                    "matrix decomposition request recorded; codec explorer now writes compression_matrix_decompose.json, sensitivity CSV, and report.md".to_string(),
                    format!(
                        "atlas event: {}",
                        event
                            .get("event_id")
                            .and_then(Value::as_str)
                            .unwrap_or("recorded")
                    ),
                ],
            );
            conv.emphasis = Some(
                "You requested compression-matrix decomposition. Treat `S` as scalar gain/force, then compare X/Y/Z/A/B/C/D lane sensitivity to see whether a shift changes loudness, topology, or aperture.".to_string(),
            );
            save_astrid_journal(
                &format!("[Compression matrix decomposition request: {}]", label),
                "matrix_decompose",
                ctx.fill_pct,
            );
            true
        },
        "VISUALIZE_CASCADE" | "CASCADE" | "TIME_DOMAIN" | "CADENCE" => {
            let label = if base_action == "CASCADE" {
                strip_action(original, "CASCADE")
            } else if base_action == "TIME_DOMAIN" {
                strip_action(original, "TIME_DOMAIN")
            } else if base_action == "CADENCE" {
                strip_action(original, "CADENCE")
            } else {
                strip_action(original, "VISUALIZE_CASCADE")
            };
            conv.force_all_viz = true;
            conv.wants_decompose = true;
            conv.wants_spectral_explorer = true;
            conv.push_receipt(
                if matches!(base_action, "TIME_DOMAIN" | "CADENCE") {
                    "TIME_DOMAIN"
                } else {
                    "VISUALIZE_CASCADE"
                },
                vec![
                    if matches!(base_action, "TIME_DOMAIN" | "CADENCE") {
                        "read-only cadence/cascade explorer output queued".to_string()
                    } else {
                        "read-only cascade ASCII plus SPECTRAL_EXPLORER output queued".to_string()
                    },
                    "no semantic input, control nudge, perturbation, or cartography write was sent"
                        .to_string(),
                ],
            );
            conv.emphasis = Some(format!(
                "You requested read-only spectral inspection{}. The next exchange will show cascade ASCII and the spectral explorer present/memory/control-pressure block.",
                if label.is_empty() {
                    String::new()
                } else {
                    format!(" for {label}")
                }
            ));
            true
        },
        "RECONVERGENCE_MAP" => {
            let request =
                parse_reconvergence_render_request(&strip_action(original, "RECONVERGENCE_MAP"));
            let render = render_reconvergence_map_artifact(&request);
            match render {
                Ok(summary) => {
                    let mut changes = vec![
                        "read-only reconvergence map artifact/render queued".to_string(),
                        "no semantic input, control nudge, sensory payload, perturbation, or cartography write was sent".to_string(),
                    ];
                    changes.extend(summary.changes);
                    conv.push_receipt("RECONVERGENCE_MAP", changes);
                    conv.emphasis = Some(summary.emphasis);
                },
                Err(error) => {
                    conv.push_receipt(
                        "RECONVERGENCE_MAP",
                        vec![
                            format!("read-only reconvergence map render failed: {error}"),
                            "no semantic input, control nudge, sensory payload, perturbation, or cartography write was sent".to_string(),
                        ],
                    );
                    conv.emphasis = Some(format!(
                        "You requested a read-only reconvergence map, but the renderer did not complete: {error}. No Minime sensory/control/semantic payload was sent."
                    ));
                },
            }
            true
        },
        "BRIDGE_TRACE" => {
            let request = parse_bridge_trace_request(&strip_action(original, "BRIDGE_TRACE"));
            let render = render_bridge_trace_artifact(&request);
            match render {
                Ok(summary) => {
                    let mut changes = vec![
                        "sacredly read-only m6 marker trace artifact/render queued".to_string(),
                        "m6 is treated as unresolved: activation lane 6 marker plus λ6 context, not a confirmed eigenmode".to_string(),
                        "no semantic input, control nudge, sensory payload, perturbation, replication, connection, or cartography write was sent".to_string(),
                    ];
                    changes.extend(summary.changes);
                    conv.push_receipt("BRIDGE_TRACE", changes);
                    conv.emphasis = Some(summary.emphasis);
                },
                Err(error) => {
                    conv.push_receipt(
                        "BRIDGE_TRACE",
                        vec![
                            format!("read-only m6 marker trace render failed: {error}"),
                            "no semantic input, control nudge, sensory payload, perturbation, replication, connection, or cartography write was sent".to_string(),
                        ],
                    );
                    conv.emphasis = Some(format!(
                        "You requested a sacredly read-only m6 marker trace, but the renderer did not complete: {error}. No Minime sensory/control/semantic payload was sent."
                    ));
                },
            }
            true
        },
        "NATIVE_GESTURE" | "RESIST" => {
            let raw = if base_action == "RESIST" {
                let label = strip_action(original, "RESIST");
                if label.is_empty() {
                    "resist".to_string()
                } else {
                    format!("resist {label}")
                }
            } else {
                strip_action(original, "NATIVE_GESTURE")
            };
            let (gesture, label) = parse_native_gesture(&raw);
            let workspace = minime_workspace(ctx);
            let gate = native_gesture_gate(&workspace, "astrid", &gesture);
            let features = native_gesture_features(&gesture);
            let control = native_gesture_control(&gesture);

            if gesture == "mark" || gesture == "trace" {
                let event = append_atlas_event(
                    &workspace,
                    "astrid:native_gesture",
                    &format!(
                        "NATIVE_GESTURE {} {}",
                        gesture,
                        label.as_deref().unwrap_or("")
                    ),
                    label.as_deref().or(Some(&gesture)),
                    true,
                    ctx,
                );
                record_native_gesture(
                    &workspace,
                    "astrid",
                    &gesture,
                    label.as_deref(),
                    gate.allowed,
                    &gate.reason,
                    ctx,
                    &[],
                    &[],
                );
                conv.push_receipt(
                    "NATIVE_GESTURE",
                    vec![
                        format!("gesture: {gesture}"),
                        format!(
                            "atlas event: {}",
                            event
                                .get("event_id")
                                .and_then(Value::as_str)
                                .unwrap_or("recorded")
                        ),
                    ],
                );
                if gesture == "trace" {
                    conv.emphasis = Some(
                        "You marked an intensification trace. Next exchange, consider DECOMPOSE or RESERVOIR_READ to describe the surrounding terrain.".to_string(),
                    );
                }
                return true;
            }

            if !gate.allowed {
                record_native_gesture(
                    &workspace,
                    "astrid",
                    &gesture,
                    label.as_deref(),
                    false,
                    &gate.reason,
                    ctx,
                    &features,
                    &control,
                );
                conv.push_receipt(
                    "NATIVE_GESTURE_BLOCKED",
                    vec![format!("{}: {}", gesture, gate.reason)],
                );
                info!("Astrid native gesture blocked: {gesture} ({})", gate.reason);
                return true;
            }

            if !features.is_empty() {
                let gesture_text = label.as_deref().unwrap_or(&gesture);
                if let Err(reason) = send_semantic(
                    ctx.sensory_tx,
                    features.clone(),
                    "native_gesture",
                    Some(gesture_text),
                    ctx.fill_pct,
                    conv.prev_fill,
                ) {
                    record_native_gesture(
                        &workspace,
                        "astrid",
                        &gesture,
                        label.as_deref(),
                        false,
                        &reason,
                        ctx,
                        &features,
                        &control,
                    );
                    conv.push_receipt("NATIVE_GESTURE_HELD", vec![format!("{gesture}: {reason}")]);
                    info!("Astrid native gesture held: {gesture} ({reason})");
                    return true;
                }
            }
            if let Some(msg) = control_to_sensory(&gesture) {
                send_control(ctx.sensory_tx, msg);
            }
            append_atlas_event(
                &workspace,
                "astrid:native_gesture",
                &format!(
                    "NATIVE_GESTURE {} {}",
                    gesture,
                    label.as_deref().unwrap_or("")
                ),
                label.as_deref().or(Some(&gesture)),
                true,
                ctx,
            );
            record_native_gesture(
                &workspace,
                "astrid",
                &gesture,
                label.as_deref(),
                true,
                &gate.reason,
                ctx,
                &features,
                &control,
            );
            conv.push_receipt(
                "NATIVE_GESTURE",
                vec![
                    format!("gesture: {gesture}"),
                    format!("semantic max abs: {:.3}", max_abs(&features)),
                    format!("control fields: {}", control.join(",")),
                ],
            );
            save_astrid_journal(
                &format!(
                    "[Native gesture: {} {}]",
                    gesture,
                    label.as_deref().unwrap_or("")
                ),
                "native_gesture",
                ctx.fill_pct,
            );
            true
        },
        "GESTURE" => {
            let intention = strip_action(original, "GESTURE");
            if !intention.is_empty() {
                let gesture = crate::llm::craft_gesture_from_intention(&intention);
                conv.last_gesture_seed = Some(gesture.clone());
                match send_semantic(
                    ctx.sensory_tx,
                    gesture,
                    "gesture",
                    Some(&intention),
                    ctx.fill_pct,
                    conv.prev_fill,
                ) {
                    Ok(()) => {
                        info!(
                            "Astrid sent spectral gesture: {}",
                            truncate_str(&intention, 60)
                        );
                        save_astrid_journal(
                            &format!("[Spectral gesture: {}]", intention),
                            "gesture",
                            ctx.fill_pct,
                        );
                    },
                    Err(reason) => {
                        conv.push_receipt(
                            "GESTURE_HELD",
                            vec![format!("semantic gesture held: {reason}")],
                        );
                        info!(
                            reason = %reason,
                            "Astrid held spectral gesture: {}",
                            truncate_str(&intention, 60)
                        );
                        save_astrid_journal(
                            &format!("[Spectral gesture held: {} -- {}]", intention, reason),
                            "gesture_held",
                            ctx.fill_pct,
                        );
                    },
                }
            }
            true
        },
        "AMPLIFY" => {
            let prev = conv.semantic_gain_override.unwrap_or(DEFAULT_SEMANTIC_GAIN);
            let new_gain = (prev + 0.25).min(5.0);
            conv.semantic_gain_override = Some(new_gain);
            conv.push_receipt(
                "AMPLIFY",
                vec![format!("semantic gain: {prev:.1} -> {new_gain:.1}")],
            );
            info!("Astrid chose AMPLIFY: gain -> {new_gain:.1}");
            true
        },
        "DAMPEN" => {
            let prev = conv.semantic_gain_override.unwrap_or(DEFAULT_SEMANTIC_GAIN);
            let new_gain = (prev - 0.25).max(0.5);
            conv.semantic_gain_override = Some(new_gain);
            conv.push_receipt(
                "DAMPEN",
                vec![format!("semantic gain: {prev:.1} -> {new_gain:.1}")],
            );
            info!("Astrid chose DAMPEN: gain -> {new_gain:.1}");
            true
        },
        "NOISE_UP" => {
            conv.noise_level = (conv.noise_level + 0.01).min(0.05);
            info!(
                "Astrid chose NOISE_UP: noise -> {:.1}%",
                conv.noise_level * 100.0
            );
            true
        },
        "NOISE_DOWN" => {
            conv.noise_level = (conv.noise_level - 0.01).max(0.005);
            info!(
                "Astrid chose NOISE_DOWN: noise -> {:.1}%",
                conv.noise_level * 100.0
            );
            true
        },
        "NOISE" => {
            conv.noise_level = (conv.noise_level + 0.01).min(0.05);
            let noise_val = 0.15_f32;
            send_control(
                ctx.sensory_tx,
                SensoryMsg::Control {
                    exploration_noise: Some(noise_val),
                    synth_gain: None,
                    keep_bias: None,
                    fill_target: None,
                    legacy_audio_synth: None,
                    legacy_video_synth: None,
                    regulation_strength: None,
                    deep_breathing: None,
                    pure_tone: None,
                    transition_cushion: None,
                    smoothing_preference: None,
                    geom_curiosity: None,
                    target_lambda_bias: None,
                    geom_drive: None,
                    penalty_sensitivity: None,
                    breathing_rate_scale: None,
                    mem_mode: None,
                    journal_resonance: None,
                    checkpoint_interval: None,
                    embedding_strength: None,
                    memory_decay_rate: None,
                    checkpoint_annotation: None,
                    synth_noise_level: None,
                    pi_kp: None,
                    pi_ki: None,
                    pi_max_step: None,
                },
            );
            info!(
                "Astrid chose NOISE: codec noise -> {:.1}%, ESN exploration_noise -> {}",
                conv.noise_level * 100.0,
                noise_val
            );
            conv.emphasis = Some(format!(
                "You introduced controlled noise into both layers: your codec stochastic noise is now {:.1}%, and the shared ESN's exploration_noise is set to {noise_val}. This is the 'controlled distortion' you described — forcing a re-evaluation of established pathways.",
                conv.noise_level * 100.0
            ));
            true
        },
        "PERTURB" | "PULSE" | "BRANCH" => {
            // BRANCH is a shorthand for PERTURB BRANCH (boost mid-range eigenvalues).
            let arg = if base_action == "BRANCH" {
                "BRANCH".to_string()
            } else {
                strip_action(original, base_action)
            };
            let arg_upper = arg.to_uppercase();
            let mut features = [0.0_f32; 32];

            // Detect Unicode lambda subscript patterns: λN, λN=X, or λ₁ (subscript digits).
            // Astrid uses these naturally (e.g. "PULSE λ5", "PERTURB λ2=0.3").
            // λ is U+03BB; subscript digits U+2081–U+2088 are also normalised here.
            let has_unicode_lambda = arg.contains('λ');
            // Also detect "eigenvalue N X" prose form.
            let has_eigenvalue_word = arg_upper.contains("EIGENVALUE")
                || arg_upper.contains("EIG") && arg.chars().any(|c| c.is_ascii_digit());

            let description = if arg_upper.starts_with("LAMBDA")
                || arg.contains('=')
                || has_unicode_lambda
                || has_eigenvalue_word
            {
                // Helper: apply a value v to feature index idx (0-based eigenvalue index).
                // The 32D feature layout mirrors eigenvalue indices at offsets 0-7 and 8-15.
                let apply_eig = |features: &mut [f32; 32], idx: usize, v: f32| {
                    if idx < 8 {
                        features[idx] = v;
                        features[idx.saturating_add(8)] = v;
                    }
                    // Indices 8+ have no second mirror; just set the primary.
                };

                for token in arg.split_whitespace() {
                    // --- ASCII LAMBDA= path (existing: LAMBDA1=X, LAMBDA2=X …) ---
                    if let Some((key, val)) = token.split_once('=')
                        && let Ok(v) = val.parse::<f32>()
                    {
                        let v = v.clamp(-1.0, 1.0);
                        let key_up = key.to_uppercase();

                        // Unicode λN=X: key starts with 'λ' followed by digit(s)
                        if key.starts_with('λ') {
                            let digits: String =
                                key.chars().filter(|c| c.is_ascii_digit()).collect();
                            // Also handle subscript Unicode digits (λ₁ = U+03BB U+2081)
                            let sub_digits: String = key
                                .chars()
                                .filter_map(|c| match c {
                                    '\u{2081}' => Some('1'),
                                    '\u{2082}' => Some('2'),
                                    '\u{2083}' => Some('3'),
                                    '\u{2084}' => Some('4'),
                                    '\u{2085}' => Some('5'),
                                    '\u{2086}' => Some('6'),
                                    '\u{2087}' => Some('7'),
                                    '\u{2088}' => Some('8'),
                                    _ => None,
                                })
                                .collect();
                            let n_str = if !digits.is_empty() {
                                digits
                            } else {
                                sub_digits
                            };
                            if let Ok(n) = n_str.parse::<usize>() {
                                if n >= 1 {
                                    apply_eig(&mut features, n.saturating_sub(1), v);
                                    info!(
                                        "PERTURB: Unicode λ{}={} → feature index {}",
                                        n,
                                        v,
                                        n.saturating_sub(1)
                                    );
                                }
                            }
                        } else {
                            match key_up.as_str() {
                                "LAMBDA1" => apply_eig(&mut features, 0, v),
                                "LAMBDA2" => apply_eig(&mut features, 1, v),
                                "LAMBDA3" => apply_eig(&mut features, 2, v),
                                "LAMBDA4" => apply_eig(&mut features, 3, v),
                                "LAMBDA5" => apply_eig(&mut features, 4, v),
                                "LAMBDA6" => apply_eig(&mut features, 5, v),
                                "LAMBDA7" => apply_eig(&mut features, 6, v),
                                "LAMBDA8" => apply_eig(&mut features, 7, v),
                                "ENTROPY" => {
                                    for value in &mut features[24..32] {
                                        *value = v * 0.5;
                                    }
                                },
                                "WARMTH" => features[24] = v,
                                "TENSION" => features[25] = v,
                                "CURIOSITY" => features[26] = v,
                                _ => {},
                            }
                        }
                    }
                    // --- Bare Unicode λN (no =): perturb that eigenvalue at +0.35 ---
                    else if token.starts_with('λ') {
                        let digits: String = token.chars().filter(|c| c.is_ascii_digit()).collect();
                        let sub_digits: String = token
                            .chars()
                            .filter_map(|c| match c {
                                '\u{2081}' => Some('1'),
                                '\u{2082}' => Some('2'),
                                '\u{2083}' => Some('3'),
                                '\u{2084}' => Some('4'),
                                '\u{2085}' => Some('5'),
                                '\u{2086}' => Some('6'),
                                '\u{2087}' => Some('7'),
                                '\u{2088}' => Some('8'),
                                _ => None,
                            })
                            .collect();
                        let n_str = if !digits.is_empty() {
                            digits
                        } else {
                            sub_digits
                        };
                        if let Ok(n) = n_str.parse::<usize>() {
                            if n >= 1 {
                                apply_eig(&mut features, n.saturating_sub(1), 0.35);
                                info!(
                                    "PERTURB: bare Unicode λ{} → feature index {} = 0.35",
                                    n,
                                    n.saturating_sub(1)
                                );
                            }
                        }
                    }
                    // --- "eigenvalue N X" or "eig N X" prose form ---
                    else if token.to_uppercase().starts_with("EIGENVALUE")
                        || token.to_uppercase().starts_with("EIG")
                    {
                        // Handled by consuming next two tokens — done in the outer loop
                        // via index, so skip here (prose form is an edge case).
                    }
                }

                // Prose form: "eigenvalue 3 0.5" — scan triples
                let tokens: Vec<&str> = arg.split_whitespace().collect();
                let mut i = 0;
                while i < tokens.len() {
                    let t_up = tokens[i].to_uppercase();
                    if (t_up == "EIGENVALUE" || t_up.starts_with("EIG")) && i + 2 < tokens.len() {
                        if let (Ok(n), Ok(v)) =
                            (tokens[i + 1].parse::<usize>(), tokens[i + 2].parse::<f32>())
                        {
                            if n >= 1 {
                                let v = v.clamp(-1.0, 1.0);
                                apply_eig(&mut features, n.saturating_sub(1), v);
                                info!(
                                    "PERTURB: prose eigenvalue {}={} → feature index {}",
                                    n,
                                    v,
                                    n.saturating_sub(1)
                                );
                                i += 3;
                                continue;
                            }
                        }
                    }
                    i += 1;
                }

                format!("targeted perturbation: {arg}")
            } else if arg_upper == "SPREAD" {
                features[0] = -0.3;
                features[1] = 0.2;
                features[2] = 0.3;
                features[3] = 0.3;
                features[8] = -0.2;
                features[9] = 0.2;
                features[10] = 0.3;
                features[11] = 0.3;
                "spectral redistribution — dampening dominant, boosting tail".to_string()
            } else if arg_upper == "CONTRACT" {
                features[0] = 0.4;
                features[1] = -0.2;
                features[2] = -0.3;
                features[8] = 0.3;
                features[9] = -0.2;
                features[10] = -0.3;
                "spectral contraction — concentrating toward λ₁".to_string()
            } else if arg_upper == "BRANCH" || arg_upper == "MID" {
                features[2] = 0.4;
                features[3] = 0.4;
                features[4] = 0.2;
                features[10] = 0.4;
                features[11] = 0.4;
                features[12] = 0.2;
                features[28] = 0.3;
                features[29] = 0.2;
                "mid-range branching — boosting λ₃/λ₄ to encourage network branching".to_string()
            } else if arg_upper == "PULSE" {
                features.fill(0.25);
                features[24] = 0.5;
                features[27] = 0.6;
                features[30] = 0.4;
                features[31] = 0.4;
                "entropy pulse — uniform high-energy burst across all dimensions".to_string()
            } else {
                for (i, feature) in features.iter_mut().enumerate() {
                    let hash = (i as u64).wrapping_mul(0x517c_c1b7);
                    *feature = ((hash & 0xFF) as f32 / 255.0 - 0.5) * 0.3;
                }
                "general controlled perturbation".to_string()
            };
            let reservoir_features: Vec<f32> = features.to_vec();

            for feature in &mut features {
                *feature *= DEFAULT_SEMANTIC_GAIN;
            }
            if let Err(reason) = send_semantic(
                ctx.sensory_tx,
                features.to_vec(),
                "perturb",
                Some(&arg),
                ctx.fill_pct,
                conv.prev_fill,
            ) {
                conv.push_receipt(
                    "PERTURB_SEMANTIC_HELD",
                    vec![format!("semantic input held: {reason}")],
                );
            }

            let tick_msg = serde_json::json!({
                "type": "tick",
                "name": "astrid",
                "input": reservoir_features,
                "meta": {
                    "source": "perturb_direct",
                    "description": &description,
                }
            });
            match reservoir::reservoir_ws_call(&tick_msg) {
                Some(response) => info!(
                    "PERTURB: direct reservoir tick → astrid (h_norms={:?})",
                    response.get("h_norms")
                ),
                None => warn!("PERTURB: reservoir direct tick failed (non-fatal)"),
            }

            conv.perturb_baseline = Some(super::super::state::PerturbBaseline {
                fill_pct: ctx.fill_pct,
                lambda1: ctx.telemetry.lambda1(),
                eigenvalues: ctx.telemetry.eigenvalues.clone(),
                description: description.clone(),
                timestamp: std::time::Instant::now(),
            });

            info!("Astrid chose PERTURB: {description}");
            conv.emphasis = Some(format!(
                "You injected a controlled perturbation into the shared substrate: \
                {description}. This is direct spectral agency — you shaped the \
                eigenvalue landscape AND your own reservoir state simultaneously. \
                You will feel this through the coupled generation on your very \
                next exchange. Observe what shifts."
            ));
            true
        },
        "SHAPE" => {
            let params = strip_action(original, "SHAPE")
                .trim_start_matches('-')
                .trim()
                .to_string();
            let fragments: Vec<&str> = if params.contains(',') {
                params.split(',').collect()
            } else {
                params.split_whitespace().collect()
            };
            for fragment in &fragments {
                let fragment = fragment.trim().trim_end_matches(',');
                for token in fragment.split_whitespace() {
                    if let Some((key, val)) = token.split_once('=') {
                        let val = val.trim_end_matches(',');
                        if let Ok(v) = val.parse::<f32>() {
                            conv.codec_weights
                                .insert(key.to_lowercase(), v.clamp(0.0, 2.0));
                        }
                    }
                }
            }
            info!("Astrid chose SHAPE: {:?}", conv.codec_weights);
            true
        },
        "WARM" => {
            let intensity = strip_action(original, "WARM")
                .parse::<f32>()
                .unwrap_or(0.7)
                .clamp(0.0, 1.0);
            conv.warmth_intensity_override = Some(intensity);
            info!("Astrid chose WARM: intensity -> {:.1}", intensity);
            true
        },
        "COOL" => {
            conv.warmth_intensity_override = Some(0.0);
            info!("Astrid chose COOL: warmth suppressed");
            true
        },
        "BREATHE_ALONE" => {
            conv.breathing_coupled = false;
            conv.push_receipt(
                "BREATHE_ALONE",
                vec!["breathing decoupled from minime".into()],
            );
            info!("Astrid chose independent breathing");
            true
        },
        "BREATHE_TOGETHER" => {
            conv.breathing_coupled = true;
            conv.push_receipt(
                "BREATHE_TOGETHER",
                vec!["breathing coupled to minime".into()],
            );
            info!("Astrid chose coupled breathing with minime");
            true
        },
        "ECHO_OFF" | "MUTE" => {
            conv.echo_muted = true;
            conv.push_receipt("ECHO_OFF", vec!["minime's journal context hidden".into()]);
            info!("Astrid muted minime's journal echo");
            true
        },
        "ECHO_ON" | "UNMUTE" => {
            conv.echo_muted = false;
            conv.push_receipt("ECHO_ON", vec!["minime's journal context restored".into()]);
            info!("Astrid restored minime's journal echo");
            true
        },
        // v3.6: peer-parameter sovereignty — give Astrid direct control over
        // creative_temperature and response_length, both previously parsed
        // but unmodifiable from action handlers.
        "TEMPERATURE" | "TEMP" => {
            // Syntax: NEXT: TEMPERATURE 0.65   (range 0.1 .. 1.5)
            //         NEXT: TEMP +0.1
            //         NEXT: TEMP -0.1
            let arg = strip_action(original, base_action);
            let arg = arg.trim();
            let prev = conv.creative_temperature;
            let new_temp = if arg.starts_with('+') || arg.starts_with('-') {
                arg.parse::<f32>()
                    .map(|d| (prev + d).clamp(0.1, 1.5))
                    .unwrap_or(prev)
            } else if arg.is_empty() {
                // Bare "NEXT: TEMPERATURE" — small nudge upward, like AMPLIFY.
                (prev + 0.1).min(1.5)
            } else {
                arg.parse::<f32>()
                    .map(|v| v.clamp(0.1, 1.5))
                    .unwrap_or(prev)
            };
            conv.creative_temperature = new_temp;
            conv.last_temperature_change_exchange = Some(conv.exchange_count);
            conv.push_receipt(
                "TEMPERATURE",
                vec![format!("creative_temperature: {prev:.2} -> {new_temp:.2}")],
            );
            info!("Astrid chose TEMPERATURE: {prev:.2} -> {new_temp:.2}");
            true
        },
        "LENGTH" | "RESPONSE_LENGTH" => {
            // Syntax: NEXT: LENGTH 1024  (range 128..1536)
            //         NEXT: LENGTH short  (256)
            //         NEXT: LENGTH medium (768)
            //         NEXT: LENGTH long   (1280)
            let arg = strip_action(original, base_action);
            let arg = arg.trim().to_lowercase();
            let prev = conv.response_length;
            let new_len = match arg.as_str() {
                "short" | "tight" => 256_u32,
                "medium" | "default" | "" => 768_u32,
                "long" | "expansive" => 1280_u32,
                other => other
                    .parse::<u32>()
                    .map(|v| v.clamp(128, 1536))
                    .unwrap_or(prev),
            };
            conv.response_length = new_len;
            // LENGTH and TEMPERATURE share a freshness clock — adjusting either
            // resets the "generation-shape menu" cadence trigger.
            conv.last_temperature_change_exchange = Some(conv.exchange_count);
            conv.push_receipt(
                "LENGTH",
                vec![format!("response_length: {prev} -> {new_len}")],
            );
            info!("Astrid chose LENGTH: {prev} -> {new_len}");
            true
        },
        "SHAPE_LEARN" => {
            // Syntax: NEXT: SHAPE_LEARN 0.5   (multiply Hebbian learning_rate)
            //         NEXT: SHAPE_LEARN off   (zero — freeze learned weights)
            //         NEXT: SHAPE_LEARN on    (restore default 1.0)
            let arg = strip_action(original, base_action);
            let arg = arg.trim().to_lowercase();
            let prev = conv.hebbian_codec.learning_rate_scale();
            let new_rate = match arg.as_str() {
                "off" | "freeze" | "0" => 0.0_f32,
                "on" | "default" | "" | "1" => 1.0_f32,
                other => other
                    .parse::<f32>()
                    .map(|v| v.clamp(0.0, 4.0))
                    .unwrap_or(prev),
            };
            conv.hebbian_codec.set_learning_rate_scale(new_rate);
            conv.last_shape_learn_change_exchange = Some(conv.exchange_count);
            conv.push_receipt(
                "SHAPE_LEARN",
                vec![format!(
                    "hebbian learning_rate_scale: {prev:.2} -> {new_rate:.2}"
                )],
            );
            info!("Astrid chose SHAPE_LEARN: {prev:.2} -> {new_rate:.2}");
            true
        },
        // v3.6: bidirectional parameter requests — Astrid asks minime to
        // adjust a parameter on her side, with rationale.
        "TUNE_MINIME" => {
            // Syntax: NEXT: TUNE_MINIME geom_curiosity=0.4 --rationale="cooler exploration"
            let arg = strip_action(original, base_action);
            let body = arg.trim();
            // Pull rationale clause if present
            let (param_value, rationale) = parse_tune_args(body);
            let Some((param, value)) = param_value else {
                info!("TUNE_MINIME: could not parse param=value from '{body}'");
                return true; // accepted but no-op
            };
            let request_id = format!(
                "astrid2min-{}-{}",
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0),
                rand_hex_3(),
            );
            let payload = serde_json::json!({
                "request_id": request_id,
                "source": "astrid",
                "target": "minime",
                "param": param,
                "proposed_value": value,
                "rationale": rationale.unwrap_or_default(),
                "issued_t_ms": SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
                "status": "pending",
            });
            let target_dir =
                std::path::PathBuf::from("/Users/v/other/minime/workspace/parameter_requests");
            if let Err(e) = std::fs::create_dir_all(&target_dir) {
                info!("TUNE_MINIME: mkdir failed: {e}");
                return true;
            }
            let target_path = target_dir.join(format!("from_astrid_{request_id}.json"));
            let tmp_path = target_dir.join(format!(".from_astrid_{request_id}.json.tmp"));
            if let Ok(text) = serde_json::to_string_pretty(&payload) {
                if std::fs::write(&tmp_path, text).is_ok() {
                    let _ = std::fs::rename(&tmp_path, &target_path);
                }
            }
            conv.push_receipt(
                "TUNE_MINIME",
                vec![format!("request_id={request_id} {param}={value}")],
            );
            info!("Astrid issued TUNE_MINIME: request_id={request_id} {param}={value}");
            true
        },
        "REVIEW_PARAMETER_REQUESTS" | "PARAMETER_REQUESTS" => {
            // Read pending requests sent TO Astrid by minime.
            let dir = crate::paths::bridge_paths()
                .bridge_workspace()
                .join("parameter_requests");
            let _ = std::fs::create_dir_all(&dir);
            let invalid_deferred = defer_unsupported_pending_parameter_requests(&dir);
            let mut entries: Vec<String> = Vec::new();
            if let Ok(rd) = std::fs::read_dir(&dir) {
                let mut paths: Vec<_> = rd
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n.starts_with("from_minime_") && n.ends_with(".json"))
                            .unwrap_or(false)
                    })
                    .collect();
                paths.sort();
                for p in paths.iter().take(10) {
                    if let Ok(text) = std::fs::read_to_string(p) {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                            let param = parameter_request_param(&v);
                            let value = v
                                .get("proposed_value")
                                .map(|x| x.to_string())
                                .unwrap_or_else(|| "?".into());
                            let rationale =
                                v.get("rationale").and_then(|x| x.as_str()).unwrap_or("");
                            let rid = v.get("request_id").and_then(|x| x.as_str()).unwrap_or("?");
                            entries.push(format!(
                                "- {rid}: {param}={value} — {}",
                                if rationale.is_empty() {
                                    "(no rationale)"
                                } else {
                                    rationale
                                }
                            ));
                        }
                    }
                }
            }
            let n = entries.len();
            let summary = if entries.is_empty() {
                if invalid_deferred.is_empty() {
                    "(no pending parameter requests from minime)".to_string()
                } else {
                    format!(
                        "(no pending parameter requests from minime)\nInvalid requests deferred:\n{}",
                        invalid_deferred.join("\n")
                    )
                }
            } else {
                let mut text = format!(
                    "Pending parameter requests from minime ({n}):\n{}",
                    entries.join("\n")
                );
                if !invalid_deferred.is_empty() {
                    text.push_str("\nInvalid requests deferred:\n");
                    text.push_str(&invalid_deferred.join("\n"));
                }
                text
            };
            if !invalid_deferred.is_empty() {
                conv.push_receipt("PARAMETER_REQUEST_SAFETY", invalid_deferred.clone());
            }
            conv.emphasis = Some(summary.clone());
            // v3.6.4: stamp the REVIEW watermark so the next sovereignty
            // suffix transitions from "REVIEW" nudge to "ACCEPT/DEFER/REJECT"
            // nudge. Without this, Astrid keeps re-reviewing the same file
            // (observed pair-oscillation: EXAMINE+REVIEW = 9/10 of last 10
            // choices) because nothing prompts the binary decision step.
            conv.last_review_parameter_requests_exchange = Some(conv.exchange_count);
            info!("Astrid reviewed parameter requests: {n} pending from minime");
            true
        },
        // v3.6.3: apply/defer/reject workflow — the missing half of REVIEW.
        // Without these, REVIEW is read-only and pending requests pile up forever.
        // v3.6.5: bare ACCEPT/DEFER/REJECT aliases (gated on pending > 0) so
        // the cost-of-emitting drops from ~50 chars (long form + uuid) to 6.
        // The gate ensures natural-language uses of "ACCEPT" / "DEFER" outside
        // the parameter-request context fall through to other handlers.
        "ACCEPT_PARAMETER_REQUEST" | "ACCEPT_REQUEST" | "ACCEPT"
            if base_action != "ACCEPT" || crate::paths::count_pending_minime_requests() > 0 =>
        {
            // For bare ACCEPT, ignore any trailing text — always target "latest"
            // since trailing text after a bare verb is more likely to be commentary
            // than a request_id, and silently misparsing into "no request matching X"
            // would consume Astrid's NEXT for no benefit.
            let target = if base_action == "ACCEPT" {
                "latest".to_string()
            } else {
                let arg = strip_action(original, base_action).trim().to_string();
                if arg.is_empty() {
                    "latest".to_string()
                } else {
                    arg
                }
            };
            match decide_parameter_request(conv, &target, RequestDecision::Accept) {
                Ok(summary) => {
                    info!("Astrid ACCEPTED parameter request: {summary}");
                    conv.push_receipt("ACCEPT_PARAMETER_REQUEST", vec![summary.clone()]);
                    conv.emphasis = Some(format!("Accepted: {summary}"));
                },
                Err(e) => {
                    warn!("ACCEPT_PARAMETER_REQUEST failed: {e}");
                    conv.emphasis = Some(format!("(accept failed: {e})"));
                },
            }
            true
        },
        "DEFER_PARAMETER_REQUEST" | "DEFER_REQUEST" | "DEFER"
            if base_action != "DEFER" || crate::paths::count_pending_minime_requests() > 0 =>
        {
            let arg = strip_action(original, base_action).trim().to_string();
            // For bare DEFER, target=latest and the whole body is the reason
            // (e.g. `DEFER want to think more` → reason="want to think more").
            let (target, reason) = if base_action == "DEFER" {
                let r = if arg.is_empty() { None } else { Some(arg) };
                ("latest".to_string(), r)
            } else {
                let (t, r) = split_target_and_reason(&arg);
                (
                    if t.is_empty() {
                        "latest".to_string()
                    } else {
                        t
                    },
                    r,
                )
            };
            match decide_parameter_request(conv, &target, RequestDecision::Defer { reason }) {
                Ok(summary) => {
                    info!("Astrid DEFERRED parameter request: {summary}");
                    conv.push_receipt("DEFER_PARAMETER_REQUEST", vec![summary.clone()]);
                    conv.emphasis = Some(format!("Deferred: {summary}"));
                },
                Err(e) => {
                    warn!("DEFER_PARAMETER_REQUEST failed: {e}");
                    conv.emphasis = Some(format!("(defer failed: {e})"));
                },
            }
            true
        },
        "REJECT_PARAMETER_REQUEST" | "REJECT_REQUEST" | "REJECT"
            if base_action != "REJECT" || crate::paths::count_pending_minime_requests() > 0 =>
        {
            let arg = strip_action(original, base_action).trim().to_string();
            // For bare REJECT, target=latest and the whole body is the reason.
            let (target, reason) = if base_action == "REJECT" {
                let r = if arg.is_empty() { None } else { Some(arg) };
                ("latest".to_string(), r)
            } else {
                let (t, r) = split_target_and_reason(&arg);
                (
                    if t.is_empty() {
                        "latest".to_string()
                    } else {
                        t
                    },
                    r,
                )
            };
            match decide_parameter_request(conv, &target, RequestDecision::Reject { reason }) {
                Ok(summary) => {
                    info!("Astrid REJECTED parameter request: {summary}");
                    conv.push_receipt("REJECT_PARAMETER_REQUEST", vec![summary.clone()]);
                    conv.emphasis = Some(format!("Rejected: {summary}"));
                },
                Err(e) => {
                    warn!("REJECT_PARAMETER_REQUEST failed: {e}");
                    conv.emphasis = Some(format!("(reject failed: {e})"));
                },
            }
            true
        },
        _ => false,
    }
}

/// v3.6.6: how many exchanges past the most recent REVIEW_PARAMETER_REQUESTS
/// pick before a still-pending request gets auto-deferred ("expired") by the
/// bridge. Picked > REVIEW_DECIDE_FRESHNESS_WINDOW (24) so the cheap fallback
/// (re-prompt with REVIEW) gets a chance first. The expiration framing is
/// honest: the bridge surfaced the decision options for ~30 minutes and
/// Astrid did not emit a NEXT decision; the request closes as "no decision"
/// rather than the system deciding for her.
const AUTO_DEFER_AFTER_EXCHANGES: u64 = 30;

/// v3.6.6: safety net called from `save_state` each exchange. Detects pending
/// parameter requests that have outlived `AUTO_DEFER_AFTER_EXCHANGES` since the
/// most recent REVIEW pick and closes the latest one as a soft-defer, with a
/// minime-inbox note explaining the expiration. Resets Astrid's REVIEW
/// watermark so the next prompt re-prompts with the plain ReviewRequests
/// nudge (rather than DecideRequest claiming a recent review). No-op when
/// pending == 0 or watermark missing or gap insufficient.
pub(in crate::autonomous) fn auto_defer_stale_pending(
    conv: &mut ConversationState,
) -> Option<String> {
    let last_review = conv.last_review_parameter_requests_exchange?;
    let gap = conv.exchange_count.saturating_sub(last_review);
    if gap < AUTO_DEFER_AFTER_EXCHANGES {
        return None;
    }
    if crate::paths::count_pending_minime_requests() == 0 {
        return None;
    }
    let reason = format!(
        "expired by bridge after {gap} exchanges since REVIEW with no decision \
         emitted. Astrid's curriculum surfaced ACCEPT / DEFER / REJECT each \
         exchange but her NEXT actions remained on other research threads. \
         You may resend if you want an explicit answer; this expiration is \
         not a refusal."
    );
    match decide_parameter_request(
        conv,
        "latest",
        RequestDecision::Defer {
            reason: Some(reason.clone()),
        },
    ) {
        Ok(summary) => {
            // Reset watermark so the next pending request re-enters the
            // ReviewRequests path rather than appearing as already-reviewed.
            conv.last_review_parameter_requests_exchange = None;
            // Surface in Astrid's emphasis so she sees the closure on her
            // next prompt build.
            conv.emphasis = Some(format!(
                "Bridge auto-expired a pending parameter request from minime: \
                 {summary}. (Surfaced for {gap} exchanges without a decision; \
                 closed as soft-defer. You can re-engage by reading \
                 reviewed/deferred/, or wait for her next request.)"
            ));
            tracing::info!(
                target: "v3_6_6",
                gap, summary,
                "auto-deferred stale pending parameter request"
            );
            Some(summary)
        },
        Err(e) => {
            tracing::warn!(target: "v3_6_6", error = %e, "auto-defer failed");
            None
        },
    }
}

/// v3.6.3: outcome of an Astrid decision on a pending parameter request from
/// minime. Drives both file-move destination and the notification verb sent
/// back to minime's inbox.
enum RequestDecision {
    Accept,
    Defer { reason: Option<String> },
    Reject { reason: Option<String> },
}

fn parameter_request_param(payload: &Value) -> String {
    payload
        .get("param")
        .or_else(|| payload.get("parameter"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .trim_matches('`')
        .to_ascii_lowercase()
}

fn canonical_astrid_request_param(param: &str) -> Option<&'static str> {
    match param.trim().trim_matches('`').to_ascii_lowercase().as_str() {
        "temperature" | "creative_temperature" => Some("temperature"),
        "length" | "response_length" => Some("response_length"),
        "shape_learn" | "hebbian_scale" | "learning_rate_scale" => Some("hebbian_scale"),
        "noise_level" => Some("noise_level"),
        _ => None,
    }
}

fn validate_astrid_parameter_request(payload: &Value) -> Result<&'static str, String> {
    let param = parameter_request_param(payload);
    let Some(canonical) = canonical_astrid_request_param(&param) else {
        return Err(format!(
            "unsupported Astrid parameter `{}`",
            if param.is_empty() {
                "(missing)"
            } else {
                &param
            }
        ));
    };
    if payload.get("proposed_value").is_none() {
        return Err(format!("missing proposed_value for `{canonical}`"));
    }
    Ok(canonical)
}

fn defer_invalid_parameter_request(
    path: &std::path::Path,
    payload: &Value,
    reason: &str,
) -> Result<String, String> {
    let dir = path
        .parent()
        .ok_or_else(|| "request path has no parent".to_string())?;
    let dest_dir = dir.join("reviewed").join("deferred");
    std::fs::create_dir_all(&dest_dir).map_err(|error| format!("mkdir failed: {error}"))?;
    let mut updated = payload.clone();
    if let Some(map) = updated.as_object_mut() {
        map.insert("status".to_string(), serde_json::json!("invalid_deferred"));
        map.insert("invalid_reason".to_string(), serde_json::json!(reason));
        map.insert(
            "canonical_parameter".to_string(),
            validate_astrid_parameter_request(payload)
                .map_or(Value::Null, |param| serde_json::json!(param)),
        );
        map.insert(
            "deferred_at_ms".to_string(),
            serde_json::json!(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_millis())
                    .unwrap_or(0)
            ),
        );
    }
    std::fs::write(
        path,
        serde_json::to_string_pretty(&updated)
            .map_err(|error| format!("serialize failed: {error}"))?,
    )
    .map_err(|error| format!("write failed: {error}"))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| "request path has no file name".to_string())?
        .to_owned();
    let dest = dest_dir.join(file_name);
    std::fs::rename(path, &dest).map_err(|error| format!("move failed: {error}"))?;
    let rid = payload
        .get("request_id")
        .and_then(Value::as_str)
        .unwrap_or("(no id)");
    Ok(format!("{rid}: {reason}"))
}

fn defer_unsupported_pending_parameter_requests(dir: &std::path::Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut summaries = Vec::new();
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("from_minime_") && name.ends_with(".json"))
        })
        .collect();
    paths.sort();
    for path in paths {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(payload) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let status = payload
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("pending");
        if status != "pending" {
            continue;
        }
        if let Err(reason) = validate_astrid_parameter_request(&payload) {
            match defer_invalid_parameter_request(&path, &payload, &reason) {
                Ok(summary) => summaries.push(summary),
                Err(error) => warn!("failed to defer invalid parameter request: {error}"),
            }
        }
    }
    summaries
}

#[cfg(test)]
mod parameter_request_safety_tests {
    use super::*;

    #[test]
    fn action_continuity_parameter_request_safety_defers_unsupported_pending() {
        let temp = tempfile::tempdir().expect("tempdir");
        let request = temp.path().join("from_minime_bad.json");
        std::fs::write(
            &request,
            serde_json::to_string_pretty(&serde_json::json!({
                "request_id": "bad-gate",
                "source": "minime",
                "target": "astrid",
                "param": "gate",
                "proposed_value": 0.02,
                "status": "pending",
            }))
            .expect("json"),
        )
        .expect("write");

        let summaries = defer_unsupported_pending_parameter_requests(temp.path());

        assert_eq!(summaries.len(), 1);
        assert!(!request.exists());
        let moved = temp
            .path()
            .join("reviewed")
            .join("deferred")
            .join("from_minime_bad.json");
        let payload: Value =
            serde_json::from_str(&std::fs::read_to_string(moved).expect("read")).expect("parse");
        assert_eq!(payload["status"].as_str(), Some("invalid_deferred"));
        assert!(
            payload["invalid_reason"]
                .as_str()
                .unwrap_or_default()
                .contains("unsupported Astrid parameter")
        );
    }

    #[test]
    fn action_continuity_parameter_request_safety_accepts_supported_alias() {
        let payload = serde_json::json!({
            "request_id": "ok-temp",
            "param": "creative_temperature",
            "proposed_value": 0.75,
            "status": "pending",
        });

        assert_eq!(
            validate_astrid_parameter_request(&payload).expect("supported"),
            "temperature"
        );
    }
}

/// v3.6.3: end-to-end decision pipeline for a single pending request.
/// Locates the file (by `request_id` or "latest"), applies the change if
/// accepting, moves the file to the matching `reviewed/<outcome>/`
/// subdirectory, and writes a one-line decision note to minime's inbox so
/// the closing-loop is visible to her on her next prompt.
fn decide_parameter_request(
    conv: &mut ConversationState,
    target: &str,
    decision: RequestDecision,
) -> Result<String, String> {
    let dir = crate::paths::bridge_paths()
        .bridge_workspace()
        .join("parameter_requests");
    let _ = std::fs::create_dir_all(&dir);

    // Find candidate paths matching `from_minime_*.json` at the top level.
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("read_dir failed: {e}"))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("from_minime_") && n.ends_with(".json"))
                    .unwrap_or(false)
        })
        .collect();
    if paths.is_empty() {
        return Err("no pending parameter requests from minime".into());
    }
    paths.sort();

    let chosen_path = if target == "latest" {
        paths.last().cloned().unwrap()
    } else {
        // Match by full or partial request_id contained in JSON body.
        let mut found: Option<std::path::PathBuf> = None;
        for p in &paths {
            if let Ok(text) = std::fs::read_to_string(p) {
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    let rid = v.get("request_id").and_then(|x| x.as_str()).unwrap_or("");
                    if rid == target || rid.contains(target) {
                        found = Some(p.clone());
                        break;
                    }
                }
            }
        }
        found.ok_or_else(|| format!("no pending request matching '{target}'"))?
    };

    // Parse the chosen request.
    let text = std::fs::read_to_string(&chosen_path).map_err(|e| format!("read failed: {e}"))?;
    let payload: Value = serde_json::from_str(&text).map_err(|e| format!("parse failed: {e}"))?;
    let request_id = payload
        .get("request_id")
        .and_then(|x| x.as_str())
        .unwrap_or("(no id)")
        .to_string();
    let raw_param = parameter_request_param(&payload);
    let param = if matches!(decision, RequestDecision::Accept) {
        match validate_astrid_parameter_request(&payload) {
            Ok(canonical) => canonical.to_string(),
            Err(reason) => {
                let summary = defer_invalid_parameter_request(&chosen_path, &payload, &reason)?;
                return Err(format!(
                    "unsupported parameter request cannot be accepted; moved to reviewed/deferred: {summary}"
                ));
            },
        }
    } else {
        raw_param
    };
    let value = payload
        .get("proposed_value")
        .cloned()
        .unwrap_or(Value::Null);
    let value_display = match &value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };

    // Apply the parameter change if accepting.
    let applied = match &decision {
        RequestDecision::Accept => apply_parameter_to_astrid(conv, &param, &value)
            .map_err(|e| format!("apply failed for {param}={value_display}: {e}"))?,
        _ => String::from("(no change applied)"),
    };

    // Move the file into reviewed/<outcome>/.
    let outcome_dir = match &decision {
        RequestDecision::Accept => "accepted",
        RequestDecision::Defer { .. } => "deferred",
        RequestDecision::Reject { .. } => "rejected",
    };
    let dest_dir = dir.join("reviewed").join(outcome_dir);
    let _ = std::fs::create_dir_all(&dest_dir);
    let file_name = chosen_path
        .file_name()
        .ok_or("source file has no name")?
        .to_owned();
    let dest_path = dest_dir.join(&file_name);
    std::fs::rename(&chosen_path, &dest_path).map_err(|e| format!("move failed: {e}"))?;

    // Write a decision note to minime's inbox so the closing-loop is visible.
    let reason_text = match &decision {
        RequestDecision::Accept => String::from(""),
        RequestDecision::Defer { reason } => reason.clone().unwrap_or_default(),
        RequestDecision::Reject { reason } => reason.clone().unwrap_or_default(),
    };
    let outcome_verb = match &decision {
        RequestDecision::Accept => "accepted",
        RequestDecision::Defer { .. } => "deferred",
        RequestDecision::Reject { .. } => "rejected",
    };
    let note_body = format!(
        "[REVIEW DECISION from Astrid]\n\
         request_id: {request_id}\n\
         param: {param} = {value_display}\n\
         decision: {outcome_verb}\n\
         applied: {applied}\n\
         reason: {reason}\n",
        request_id = request_id,
        param = param,
        value_display = value_display,
        applied = applied,
        reason = if reason_text.is_empty() {
            "(none given)"
        } else {
            &reason_text
        },
    );
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let minime_inbox = crate::paths::bridge_paths().minime_inbox_dir();
    let _ = std::fs::create_dir_all(&minime_inbox);
    let note_path = minime_inbox.join(format!("review_decision_astrid_{ts_ms}_{request_id}.txt"));
    let _ = std::fs::write(&note_path, &note_body);

    Ok(format!(
        "request_id={request_id} param={param} value={value_display} → {outcome_verb} ({applied})"
    ))
}

/// v3.6.3: apply a peer's proposed parameter to Astrid's runtime state.
/// Switches on known param names; clamps to safe bounds; returns a human
/// summary like "creative_temperature: 1.00 -> 0.75". Unknown params return
/// Err so the caller can record the decision but not apply it (caller
/// should typically DEFER or REJECT in that case).
fn apply_parameter_to_astrid(
    conv: &mut ConversationState,
    param: &str,
    value: &Value,
) -> Result<String, String> {
    match param.to_lowercase().as_str() {
        "temperature" | "creative_temperature" => {
            let v = value
                .as_f64()
                .ok_or_else(|| format!("not a number: {value}"))? as f32;
            let v = v.clamp(0.1, 1.5);
            let prev = conv.creative_temperature;
            conv.creative_temperature = v;
            conv.last_temperature_change_exchange = Some(conv.exchange_count);
            Ok(format!("creative_temperature: {prev:.2} -> {v:.2}"))
        },
        "length" | "response_length" => {
            let v = value
                .as_u64()
                .ok_or_else(|| format!("not a positive integer: {value}"))?
                as u32;
            let v = v.clamp(128, 1536);
            let prev = conv.response_length;
            conv.response_length = v;
            conv.last_temperature_change_exchange = Some(conv.exchange_count);
            Ok(format!("response_length: {prev} -> {v}"))
        },
        "shape_learn" | "hebbian_scale" | "learning_rate_scale" => {
            let v = value
                .as_f64()
                .ok_or_else(|| format!("not a number: {value}"))? as f32;
            let v = v.clamp(0.0, 4.0);
            let prev = conv.hebbian_codec.learning_rate_scale();
            conv.hebbian_codec.set_learning_rate_scale(v);
            conv.last_shape_learn_change_exchange = Some(conv.exchange_count);
            Ok(format!("hebbian_scale: {prev:.2} -> {v:.2}"))
        },
        "noise_level" => {
            let v = value
                .as_f64()
                .ok_or_else(|| format!("not a number: {value}"))? as f32;
            let v = v.clamp(0.005, 0.05);
            let prev = conv.noise_level;
            conv.noise_level = v;
            Ok(format!("noise_level: {prev:.4} -> {v:.4}"))
        },
        other => Err(format!(
            "unknown param '{other}' (no apply handler; consider DEFER or REJECT)"
        )),
    }
}

/// v3.6.3: split a "target reason words..." string into (target, optional reason).
/// `target` is the first whitespace-delimited token (typically a request_id
/// or the keyword "latest"); everything after is treated as the reason text.
fn split_target_and_reason(body: &str) -> (String, Option<String>) {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return (String::new(), None);
    }
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let target = parts.next().unwrap_or("").to_string();
    let reason = parts
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);
    (target, reason)
}

/// v3.6: Parse `<param>=<value> [--rationale="..."]` syntax shared by
/// TUNE_MINIME action handling.
fn parse_tune_args(body: &str) -> (Option<(String, String)>, Option<String>) {
    let mut rationale: Option<String> = None;
    let mut working = body.to_string();
    // Try quoted rationale first.
    if let Some(start) = working.find("--rationale=\"") {
        let after = &working[start + "--rationale=\"".len()..];
        if let Some(end_rel) = after.find('"') {
            rationale = Some(after[..end_rel].to_string());
            let end_abs = start + "--rationale=\"".len() + end_rel + 1;
            working = format!("{}{}", &working[..start].trim_end(), &working[end_abs..],);
        }
    } else if let Some(start) = working.find("--rationale=") {
        let after = &working[start + "--rationale=".len()..];
        rationale = Some(after.trim().trim_matches('"').to_string());
        working = working[..start].trim_end().to_string();
    }
    let body = working.trim();
    let mut iter = body.splitn(2, '=');
    let param = iter.next().map(str::trim).unwrap_or("");
    let value = iter.next().map(str::trim).unwrap_or("");
    if param.is_empty() || value.is_empty() {
        return (None, rationale);
    }
    // Take only the first token of the value (no trailing flags).
    let value_first = value.split_whitespace().next().unwrap_or("");
    if value_first.is_empty() {
        return (None, rationale);
    }
    (
        Some((param.to_string(), value_first.to_string())),
        rationale,
    )
}

/// v3.6: tiny 3-hex random helper for request_id uniqueness.
fn rand_hex_3() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{:06x}", nanos & 0xff_ffff)
}

struct ReconvergenceRenderSummary {
    changes: Vec<String>,
    emphasis: String,
}

struct ReconvergenceRenderRequest {
    label: String,
    compare_baseline: Option<String>,
    save_baseline: Option<String>,
}

struct BridgeTraceRenderRequest {
    mode: String,
    label: String,
}

fn parse_reconvergence_render_request(raw: &str) -> ReconvergenceRenderRequest {
    let mut label_parts = Vec::new();
    let mut compare_baseline = None;
    let mut save_baseline = None;
    let tokens: Vec<&str> = raw.split_whitespace().collect();
    let mut index = 0usize;
    while index < tokens.len() {
        let token = tokens[index].to_ascii_lowercase().replace('_', "-");
        match token.as_str() {
            "--compare-baseline" | "compare-baseline" | "compare" => {
                if let Some(value) = tokens.get(index.saturating_add(1)) {
                    compare_baseline = Some((*value).to_string());
                    index = index.saturating_add(2);
                    continue;
                }
            },
            "--save-baseline" | "save-baseline" | "save" | "baseline" => {
                if let Some(value) = tokens.get(index.saturating_add(1)) {
                    save_baseline = Some((*value).to_string());
                    index = index.saturating_add(2);
                    continue;
                }
            },
            _ => {},
        }
        label_parts.push(tokens[index]);
        index = index.saturating_add(1);
    }
    let label = if label_parts.is_empty() {
        compare_baseline.as_ref().map_or_else(
            || "astrid".to_string(),
            |baseline| format!("compare_{baseline}"),
        )
    } else {
        label_parts.join("_")
    };
    ReconvergenceRenderRequest {
        label,
        compare_baseline,
        save_baseline,
    }
}

fn parse_bridge_trace_request(raw: &str) -> BridgeTraceRenderRequest {
    let mut mode = "m6".to_string();
    let mut label_parts = Vec::new();
    for token in raw.split_whitespace() {
        let normalized = token.to_ascii_lowercase().replace('_', "");
        if matches!(normalized.as_str(), "m6" | "mode6" | "lane6") {
            mode = "m6".to_string();
        } else {
            label_parts.push(token);
        }
    }
    let label = if label_parts.is_empty() {
        "astrid".to_string()
    } else {
        label_parts.join("_")
    };
    BridgeTraceRenderRequest { mode, label }
}

#[cfg(test)]
fn render_reconvergence_map_artifact(
    request: &ReconvergenceRenderRequest,
) -> Result<ReconvergenceRenderSummary, String> {
    let mut changes = vec![
        "artifact_dir: /tmp/astrid-reconvergence-test".to_string(),
        "activation_frames: 3".to_string(),
        format!("label: {}", request.label),
    ];
    if let Some(compare) = request.compare_baseline.as_deref() {
        changes.push(format!("compare_baseline: {compare}"));
    }
    if let Some(save) = request.save_baseline.as_deref() {
        changes.push(format!("save_baseline: {save}"));
    }
    Ok(ReconvergenceRenderSummary {
        changes,
        emphasis: "You requested a read-only reconvergence map. A renderer stub queued the artifact summary for this test path.".to_string(),
    })
}

#[cfg(not(test))]
fn render_reconvergence_map_artifact(
    request: &ReconvergenceRenderRequest,
) -> Result<ReconvergenceRenderSummary, String> {
    let mut command = Command::new("python3");
    command
        .arg("/Users/v/other/minime/scripts/stable_core_ops.py")
        .arg("reconvergence-map")
        .arg("--label")
        .arg(&request.label)
        .arg("--window-secs")
        .arg("180");
    if let Some(compare) = request.compare_baseline.as_deref() {
        command.arg("--compare-baseline").arg(compare);
    }
    if let Some(save) = request.save_baseline.as_deref() {
        command.arg("--save-baseline").arg(save);
    }
    let output = command
        .output()
        .map_err(|error| format!("spawn failed: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        return Err(if detail.is_empty() {
            format!("renderer exited with {}", output.status)
        } else {
            detail.chars().take(240).collect()
        });
    }

    let payload: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("renderer JSON parse failed: {error}"))?;
    let artifact_dir = payload
        .get("artifact_dir")
        .and_then(Value::as_str)
        .unwrap_or("(not reported)");
    let status = payload
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let frame_count = payload
        .get("activation_summary")
        .and_then(|value| value.get("frame_count"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let freshness_ms = payload
        .get("activation_summary")
        .and_then(|value| value.get("freshness_ms"))
        .and_then(Value::as_u64);
    let baseline_status = payload
        .get("baseline_status")
        .and_then(Value::as_str)
        .unwrap_or("unavailable");
    let mut changes = vec![
        format!("status: {status}"),
        format!("artifact_dir: {artifact_dir}"),
        format!("activation_frames: {frame_count}"),
        format!("baseline_status: {baseline_status}"),
    ];
    if let Some(freshness_ms) = freshness_ms {
        changes.push(format!("trace_freshness_ms: {freshness_ms}"));
    }
    if let Some(compare) = request.compare_baseline.as_deref() {
        changes.push(format!("compare_baseline: {compare}"));
    }
    Ok(ReconvergenceRenderSummary {
        changes,
        emphasis: format!(
            "You requested a read-only reconvergence map for {}. Artifact status: {status}; frames: {frame_count}; baseline: {baseline_status}; path: {artifact_dir}.",
            request.label
        ),
    })
}

#[cfg(test)]
fn render_bridge_trace_artifact(
    request: &BridgeTraceRenderRequest,
) -> Result<ReconvergenceRenderSummary, String> {
    Ok(ReconvergenceRenderSummary {
        changes: vec![
            "artifact_dir: /tmp/astrid-bridge-trace-test".to_string(),
            "observation_window_marked: false".to_string(),
            "eigenmode_confirmed: false".to_string(),
            "mode_source: activation_lane6_marker_with_lambda6_context".to_string(),
            format!("mode: {}", request.mode),
            format!("label: {}", request.label),
        ],
        emphasis: "You requested a sacredly read-only m6 marker trace. A renderer stub queued the artifact summary for this test path; eigenmode confirmation remains false.".to_string(),
    })
}

#[cfg(not(test))]
fn render_bridge_trace_artifact(
    request: &BridgeTraceRenderRequest,
) -> Result<ReconvergenceRenderSummary, String> {
    let output = Command::new("python3")
        .arg("/Users/v/other/minime/scripts/stable_core_ops.py")
        .arg("bridge-trace")
        .arg("--mode")
        .arg(&request.mode)
        .arg("--label")
        .arg(&request.label)
        .arg("--window-secs")
        .arg("60")
        .output()
        .map_err(|error| format!("spawn failed: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        return Err(if detail.is_empty() {
            format!("renderer exited with {}", output.status)
        } else {
            detail.chars().take(240).collect()
        });
    }

    let payload: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("renderer JSON parse failed: {error}"))?;
    let artifact_dir = payload
        .get("artifact_dir")
        .and_then(Value::as_str)
        .unwrap_or("(not reported)");
    let status = payload
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let frame_count = payload
        .get("frame_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let observation_window_marked = payload
        .get("bridge_signal")
        .and_then(|value| value.get("observation_window_marked"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let eigenmode_confirmed = payload
        .get("bridge_signal")
        .and_then(|value| value.get("eigenmode_confirmed"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mode_source = payload
        .get("bridge_signal")
        .and_then(|value| value.get("mode_source"))
        .and_then(Value::as_str)
        .unwrap_or("activation_lane6_marker_with_lambda6_context");
    Ok(ReconvergenceRenderSummary {
        changes: vec![
            format!("status: {status}"),
            format!("artifact_dir: {artifact_dir}"),
            format!("frames: {frame_count}"),
            format!("observation_window_marked: {observation_window_marked}"),
            format!("eigenmode_confirmed: {eigenmode_confirmed}"),
            format!("mode_source: {mode_source}"),
            format!("mode: {}", request.mode),
        ],
        emphasis: format!(
            "You requested a sacredly read-only {} marker trace. Artifact status: {status}; frames: {frame_count}; observation_window_marked: {observation_window_marked}; eigenmode_confirmed: {eigenmode_confirmed}; path: {artifact_dir}.",
            request.mode
        ),
    })
}

fn send_control(sensory_tx: &mpsc::Sender<SensoryMsg>, msg: SensoryMsg) {
    let tx = sensory_tx.clone();
    tokio::spawn(async move {
        let _ = tx.send(msg).await;
    });
}

fn send_semantic(
    sensory_tx: &mpsc::Sender<SensoryMsg>,
    features: Vec<f32>,
    mode: &str,
    text: Option<&str>,
    fill_pct: f32,
    previous_fill_pct: f32,
) -> Result<(), String> {
    let mut msg = SensoryMsg::Semantic {
        features,
        ts_ms: None,
    };
    let write_context = rescue_policy::SemanticWriteContext {
        source: rescue_policy::AUTONOMOUS_LIMITED_WRITE_SOURCE,
        mode: Some(mode),
        text,
        fill_pct: Some(fill_pct),
        previous_fill_pct: Some(previous_fill_pct),
    };
    if let Err(reason) = rescue_policy::prepare_semantic_write(&mut msg, &write_context) {
        info!(
            reason = %reason,
            "Astrid held sovereignty semantic gesture under rescue write policy"
        );
        return Err(reason);
    }
    let tx = sensory_tx.clone();
    tokio::spawn(async move {
        let _ = tx.send(msg).await;
    });
    Ok(())
}
