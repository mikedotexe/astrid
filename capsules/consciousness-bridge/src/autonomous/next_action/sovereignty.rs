use serde_json::Value;
#[cfg(not(test))]
use std::process::Command;
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
        _ => false,
    }
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
