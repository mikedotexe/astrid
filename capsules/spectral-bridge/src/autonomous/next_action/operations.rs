use std::path::{Path, PathBuf};

use serde_json::Value;
use tracing::info;

use super::{ConversationState, NextActionContext, bridge_paths, strip_action};
use crate::memory;
use crate::rescue_policy;
use crate::types::SpectralTelemetry;

const AGENCY_CORRIDOR_BOUNDARY: &str = "Agency Corridor V1/V2 is non-live evidence only; it grants no approval, marks no live work runnable, and mutates no pressure/fill/PI/controller/sensory/fallback/protocol/runtime state.";

fn latest_codec_entropy_vibrancy_probe_path(workspace: &Path) -> Option<PathBuf> {
    let root = workspace.join("diagnostics/codec_entropy_vibrancy_probes");
    let entries = std::fs::read_dir(root).ok()?;
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("codec_entropy_vibrancy_probe.json"))
        .filter(|path| path.is_file())
        .filter_map(|path| {
            let modified = std::fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .ok()?;
            Some((modified, path))
        })
        .max_by(|left, right| left.0.cmp(&right.0))
        .map(|(_, path)| path)
}

fn scalar_text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .map(|item| {
            item.as_str()
                .map(ToString::to_string)
                .unwrap_or_else(|| item.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn bounded_chars(raw: &str, limit: usize) -> String {
    raw.chars().take(limit).collect::<String>()
}

fn agency_corridor_action_for_command(base_action: &str) -> &'static str {
    match base_action {
        "OBJECT_TO_CLOSURE" => "emit_closure_objection",
        "REQUEST_SAFE_REPLAY" => "generate_replay_candidate",
        "REQUEST_SELF_OBSERVATION" => "request_scoped_self_observation",
        "PROPOSE_CANARY" => "propose_canary_criteria",
        "REQUEST_CORRIDOR_LEASE" => "request_corridor_lease",
        "REOPEN_CLOSURE" => "reopen_insufficient_closure",
        "COMPARE_ARTIFACTS" => "compare_artifacts",
        "PREPARE_SOURCE_PROPOSAL" => "prepare_source_proposal",
        "PROPOSE_WORK_PROGRAM" => "propose_work_program",
        "PRIORITIZE_WORK" => "prioritize_work",
        "PORTFOLIO_NOTE" => "portfolio_note",
        "PREPARE_PATCH_BUNDLE" => "prepare_patch_bundle",
        _ => "evidence_only",
    }
}

fn agency_corridor_schema_for_command(base_action: &str) -> &'static str {
    match base_action {
        "PROPOSE_WORK_PROGRAM" | "PRIORITIZE_WORK" | "PORTFOLIO_NOTE" | "PREPARE_PATCH_BUNDLE" => {
            "agency_corridor_program_request_v1"
        },
        "REQUEST_CORRIDOR_LEASE"
        | "REOPEN_CLOSURE"
        | "COMPARE_ARTIFACTS"
        | "PREPARE_SOURCE_PROPOSAL" => "agency_corridor_bridge_request_v2",
        _ => "agency_corridor_bridge_request_v1",
    }
}

fn agency_corridor_state_for_command(base_action: &str) -> &'static str {
    match base_action {
        "PROPOSE_CANARY" => "canary_criteria_proposed",
        "REOPEN_CLOSURE" => "closure_reopened",
        "REQUEST_SELF_OBSERVATION" => "self_observation_requested",
        _ => "evidence_only",
    }
}

fn agency_corridor_receipt_label(base_action: &str) -> &'static str {
    match base_action {
        "OBJECT_TO_CLOSURE" => "closure objection",
        "REQUEST_SAFE_REPLAY" => "safe replay request",
        "REQUEST_SELF_OBSERVATION" => "self-observation request",
        "PROPOSE_CANARY" => "canary criteria proposal",
        "REQUEST_CORRIDOR_LEASE" => "corridor lease request",
        "REOPEN_CLOSURE" => "closure reopen request",
        "COMPARE_ARTIFACTS" => "artifact comparison request",
        "PREPARE_SOURCE_PROPOSAL" => "source-prep proposal request",
        "PROPOSE_WORK_PROGRAM" => "work-program proposal",
        "PRIORITIZE_WORK" => "work priority request",
        "PORTFOLIO_NOTE" => "evidence portfolio note",
        "PREPARE_PATCH_BUNDLE" => "quarantined patch-bundle request",
        _ => "agency corridor request",
    }
}

fn agency_corridor_request_payload(
    request_id: &str,
    ts: f64,
    base_action: &str,
    body: &str,
    fill_pct: f32,
) -> Value {
    serde_json::json!({
        "schema": agency_corridor_schema_for_command(base_action),
        "schema_version": if agency_corridor_schema_for_command(base_action).ends_with("_v2") { 2 } else { 1 },
        "bridge_request_id": request_id,
        "timestamp": ts,
        "source": "spectral_bridge_next_action",
        "command": base_action,
        "action": agency_corridor_action_for_command(base_action),
        "state": agency_corridor_state_for_command(base_action),
        "v1_compatible_action": agency_corridor_action_for_command(base_action),
        "program_request_kind": if agency_corridor_schema_for_command(base_action) == "agency_corridor_program_request_v1" { agency_corridor_action_for_command(base_action) } else { "" },
        "bounded_summary": bounded_chars(body, 900),
        "fill_at_request": fill_pct,
        "requested_lease_scope": if base_action == "REQUEST_CORRIDOR_LEASE" { bounded_chars(body, 240) } else { String::new() },
        "source_prep_writes_source_now": false,
        "edits_source_now": false,
        "patch_bundle_applies_now": false,
        "right_to_ignore": true,
        "grants_approval": false,
        "live_eligible_now": false,
        "auto_approved": false,
        "authority_boundary": AGENCY_CORRIDOR_BOUNDARY,
    })
}

fn record_agency_corridor_request(
    base_action: &str,
    original: &str,
    fill_pct: f32,
) -> Result<PathBuf, String> {
    let body = strip_action(original, base_action);
    if body.trim().is_empty() {
        return Err(format!(
            "{base_action} needs a bounded subject/body after the command"
        ));
    }
    let ts = crate::db::unix_now();
    let ts_label = format!("{ts:.0}");
    let request_id = format!(
        "bridge_corridor_{}_{}",
        ts_label,
        base_action.to_ascii_lowercase()
    );
    let diagnostics = if agency_corridor_schema_for_command(base_action).ends_with("_v2")
        || agency_corridor_schema_for_command(base_action) == "agency_corridor_program_request_v1"
    {
        "diagnostics/agency_corridor_v2/bridge_requests"
    } else {
        "diagnostics/agency_corridor_v1/bridge_requests"
    };
    let root = bridge_paths().bridge_workspace().join(diagnostics);
    std::fs::create_dir_all(&root).map_err(|err| format!("create corridor dir failed: {err}"))?;
    let path = root.join(format!("{request_id}.json"));
    let payload = agency_corridor_request_payload(&request_id, ts, base_action, &body, fill_pct);
    let text = serde_json::to_string_pretty(&payload)
        .map_err(|err| format!("serialize corridor request failed: {err}"))?;
    std::fs::write(&path, text).map_err(|err| format!("write corridor request failed: {err}"))?;
    Ok(path)
}

fn codec_entropy_vibrancy_probe_report(workspace: &Path) -> Option<String> {
    let path = latest_codec_entropy_vibrancy_probe_path(workspace)?;
    let payload = std::fs::read_to_string(&path).ok()?;
    let value: Value = serde_json::from_str(&payload).ok()?;
    let mut sample_lines = Vec::new();
    if let Some(samples) = value.get("samples").and_then(Value::as_array) {
        for sample in samples.iter().take(4) {
            sample_lines.push(format!(
                "  - {} class={} entropy={} current_tail={} candidate_tail={} shimmer={} gain={}",
                scalar_text(sample, "sample_id"),
                scalar_text(sample, "classification"),
                scalar_text(sample, "spectral_entropy"),
                scalar_text(sample, "current_tail_vibrancy"),
                scalar_text(sample, "candidate_tail_vibrancy"),
                scalar_text(sample, "current_shimmer_risk"),
                scalar_text(sample, "adaptive_gain"),
            ));
        }
    }
    if sample_lines.is_empty() {
        sample_lines.push("  - no samples recorded".to_string());
    }
    Some(format!(
        "Latest codec entropy/vibrancy probe:\n\
           - artifact: {}\n\
           - status: {}\n\
           - shimmer_risk_count: {}\n\
           - candidate_improvement_count: {}\n\
           - authority: diagnostic_context_not_command\n\
           - boundary: read-only offline probe; no SEMANTIC_DIM, FEATURE_ABS_MAX, vibrancy_lift, adaptive_gain, semantic write, controller, or peer behavior changed\n\
         Samples:\n{}",
        path.display(),
        scalar_text(&value, "status"),
        scalar_text(&value, "current_shimmer_risk_count"),
        scalar_text(&value, "candidate_improvement_count"),
        sample_lines.join("\n"),
    ))
}

fn latest_self_study_review_json_path(workspace: &Path) -> Option<PathBuf> {
    let root = workspace.join("diagnostics/self_study_reviews");
    let entries = std::fs::read_dir(root).ok()?;
    entries
        .filter_map(Result::ok)
        .map(|entry| {
            if entry.file_type().ok().is_some_and(|kind| kind.is_dir()) {
                entry.path().join("review.json")
            } else {
                entry.path()
            }
        })
        .filter(|path| path.file_name().and_then(|name| name.to_str()) == Some("review.json"))
        .filter(|path| path.is_file())
        .filter_map(|path| {
            let modified = std::fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .ok()?;
            Some((modified, path))
        })
        .max_by(|left, right| left.0.cmp(&right.0))
        .map(|(_, path)| path)
}

fn codec_replay_review_report(workspace: &Path) -> Option<String> {
    let path = latest_self_study_review_json_path(workspace)?;
    let payload = std::fs::read_to_string(&path).ok()?;
    let review: Value = serde_json::from_str(&payload).ok()?;
    let codec = review.get("codec_real_replay_v1").unwrap_or(&Value::Null);
    let narrative = review
        .get("narrative_arc_temporal_decay_lab_v1")
        .unwrap_or(&Value::Null);
    let content = review
        .get("content_aware_vibrancy_gate_candidate_v1")
        .unwrap_or(&Value::Null);
    let clamp = review
        .get("codec_clamp_headroom_probe_v1")
        .unwrap_or(&Value::Null);
    let afterimage = review
        .get("codec_afterimage_time_series_v1")
        .unwrap_or(&Value::Null);
    let tail_lab = review
        .get("tail_participation_counterfactual_lab_v1")
        .or_else(|| codec.get("tail_participation_counterfactual_lab_v1"))
        .unwrap_or(&Value::Null);
    if !codec.is_object()
        && !narrative.is_object()
        && !content.is_object()
        && !clamp.is_object()
        && !afterimage.is_object()
        && !tail_lab.is_object()
    {
        return None;
    }
    let mut sample_lines = Vec::new();
    if let Some(entries) = codec.get("entries").and_then(Value::as_array) {
        for entry in entries.iter().take(4) {
            sample_lines.push(format!(
                "  - {} class={} entropy_dim={} semantic_density={} warmth={} tension={} source={}",
                scalar_text(entry, "sample_id"),
                scalar_text(entry, "classification"),
                scalar_text(entry, "actual_entropy_dim"),
                scalar_text(entry, "semantic_density_score"),
                scalar_text(entry, "warmth_dim"),
                scalar_text(entry, "tension_dim"),
                scalar_text(entry, "source_path"),
            ));
        }
    }
    if sample_lines.is_empty() {
        sample_lines.push("  - no replay entries summarized".to_string());
    }
    Some(format!(
        "Latest codec replay review packet:\n\
           - review: {}\n\
           - replay_status: {}\n\
           - artifact: {}\n\
           - corpus: {} / {}\n\
           - embedding: mode={} status={} backed_arc={}\n\
           - content_gate: {} semantic_delta={} candidate_delta={}\n\
           - clamp_headroom: {} near_static={} tail_pressure={} dynamic_candidates={} static_max={} tail_max={}\n\
           - tail_counterfactual: {} aperture_supported={} participation_supported={} combined_supported={} tail_participation_lease_authority={}\n\
           - narrative_arc: {} temporal_candidates={} pivot_candidates={}\n\
           - afterimage_series: {} entries={} codec_anchors={} pressure_anchors={}\n\
           - authority: diagnostic_context_not_command\n\
           - boundary: read-only replay surface; no codec dimensions, vibrancy lift, adaptive gain, tail participation lease authority, semantic write, experiment, controller, or peer behavior changed\n\
         Replay entries:\n{}",
        path.display(),
        scalar_text(codec, "status"),
        scalar_text(codec, "artifact_path"),
        scalar_text(codec, "corpus_source"),
        scalar_text(codec, "corpus_status"),
        scalar_text(codec, "embedding_mode"),
        scalar_text(codec, "embedding_status"),
        scalar_text(codec, "embedding_backed_arc_status"),
        scalar_text(content, "status"),
        scalar_text(content, "semantic_density_score_delta"),
        scalar_text(content, "candidate_lift_delta"),
        scalar_text(clamp, "status"),
        scalar_text(clamp, "near_static_clamp_count"),
        scalar_text(clamp, "tail_ceiling_pressure_count"),
        scalar_text(clamp, "dynamic_headroom_candidate_count"),
        scalar_text(clamp, "static_feature_abs_max"),
        scalar_text(clamp, "tail_vibrancy_max"),
        scalar_text(tail_lab, "status"),
        scalar_text(tail_lab, "vibrancy_aperture_supported_count"),
        scalar_text(tail_lab, "tail_participation_supported_count"),
        scalar_text(tail_lab, "combined_supported_count"),
        scalar_text(tail_lab, "tail_participation_lease_authority"),
        scalar_text(narrative, "status"),
        scalar_text(narrative, "temporal_decay_candidate_count"),
        scalar_text(narrative, "pivot_detector_candidate_count"),
        scalar_text(afterimage, "status"),
        scalar_text(afterimage, "entry_count"),
        scalar_text(afterimage, "codec_anchor_count"),
        scalar_text(afterimage, "pressure_anchor_count"),
        sample_lines.join("\n"),
    ))
}

fn spectral_explorer_review_summary(
    label: &str,
    telemetry: &SpectralTelemetry,
    controller_health_available: bool,
    ising_shadow_available: bool,
    current_codec_signature_available: bool,
) -> String {
    let label = if label.trim().is_empty() {
        "current"
    } else {
        label.trim()
    };
    let mut fields = vec![
        format!("fill_pct: {:.1}", telemetry.fill_pct()),
        format!("lambda1: {:.3}", telemetry.lambda1()),
        format!("controller_health_available: {controller_health_available}"),
        format!("ising_shadow_available: {ising_shadow_available}"),
        format!("current_codec_signature_available: {current_codec_signature_available}"),
    ];
    if let Some(lambda1_rel) = telemetry.lambda1_rel {
        fields.push(format!("lambda1_rel: {lambda1_rel:.3}"));
    }
    let key_fields = fields
        .iter()
        .take(6)
        .map(|field| format!("  - {field}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "=== SPECTRAL EXPLORER REVIEW SUMMARY ===\n\
         Action: SPECTRAL_EXPLORER\n\
         Label: {label}\n\
         Event id: none\n\n\
         Key fields:\n{key_fields}\n\n\
         Authority boundary: read-only typed spectral explorer; No semantic input, control nudge, sensory payload, perturbation, native gesture, cartography write, or Minime parameter change was sent.\n\
         Suggested comparison: compare this explorer snapshot against later audit summaries, visual cascade output, reconvergence artifacts, and stable-core status."
    )
}

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    match base_action {
        "PING" => {
            let ts = crate::db::unix_now();
            let ping_path = bridge_paths()
                .minime_inbox_dir()
                .join(format!("ping_{ts}.txt"));
            let _ = std::fs::write(
                &ping_path,
                format!(
                    "PING from Astrid — fill {:.1}%, λ₁={:.0}. Are you there?",
                    ctx.fill_pct,
                    ctx.telemetry.lambda1()
                ),
            );
            info!("Astrid sent PING to minime");
            conv.emphasis = Some(
                "You sent a ping to minime. A PONG with their current state will arrive in your inbox shortly."
                    .into(),
            );
            true
        },
        "RUN_PYTHON" | "RUN" => {
            let run_python = strip_action(original, "RUN_PYTHON");
            let arg = if run_python.is_empty() {
                strip_action(original, "RUN")
            } else {
                run_python
            };

            let experiments_dir = bridge_paths().experiments_dir();
            let _ = std::fs::create_dir_all(&experiments_dir);
            let script_path = if !arg.is_empty() {
                let direct = experiments_dir.join(&arg);
                if direct.exists() {
                    Some(direct)
                } else {
                    let python = experiments_dir.join(format!("{arg}.py"));
                    python.exists().then_some(python)
                }
            } else {
                None
            };

            if let Some(script) = script_path {
                info!("Astrid chose RUN_PYTHON: {}", script.display());
                let output = std::process::Command::new("python3")
                    .arg(&script)
                    .current_dir(&experiments_dir)
                    .env("MPLBACKEND", "Agg")
                    .output();
                let result_text = match output {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let status = if output.status.success() {
                            "SUCCESS"
                        } else {
                            "FAILED"
                        };
                        format!(
                            "Python experiment {status}: {}\n\nOUTPUT:\n{}\n{}",
                            script.file_name().unwrap_or_default().to_string_lossy(),
                            &stdout[..stdout.floor_char_boundary(3000)],
                            if stderr.is_empty() {
                                String::new()
                            } else {
                                format!("ERRORS:\n{}", &stderr[..stderr.floor_char_boundary(1000)])
                            }
                        )
                    },
                    Err(error) => format!("Failed to run script: {error}"),
                };
                conv.emphasis = Some(format!(
                    "You ran a Python experiment:\n{result_text}\n\nReflect on these results. What do they reveal about the dynamics?"
                ));
            } else {
                let not_found = if arg.is_empty() {
                    String::new()
                } else {
                    format!(" ('{arg}' not found)")
                };
                let available = std::fs::read_dir(&experiments_dir)
                    .map(|rd| {
                        rd.filter_map(|e| e.ok())
                            .filter(|e| e.path().extension().is_some_and(|ext| ext == "py"))
                            .map(|e| e.file_name().to_string_lossy().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_else(|_| "none".into());
                let path_hint = run_python_subpath_hint(&arg);
                conv.emphasis = Some(format!(
                    "RUN_PYTHON: no top-level script found{not_found}. RUN_PYTHON runs scripts directly under workspace/experiments/. Available top-level scripts: {available}. Specify a filename: NEXT: RUN_PYTHON thermostatic_esn_test.py{path_hint}"
                ));
            }
            true
        },
        "ASK" => {
            let question = strip_action(original, "ASK");
            if !question.is_empty() {
                let ts = crate::db::unix_now();
                let ask_path = bridge_paths()
                    .minime_inbox_dir()
                    .join(format!("question_from_astrid_{ts}.txt"));
                let _ = std::fs::write(
                    &ask_path,
                    format!(
                        "=== QUESTION FROM ASTRID ===\nTimestamp: {ts}\nFill: {:.1}%\n\nAstrid asks: {question}\n\nPlease respond naturally. Your reply will be routed back to her.",
                        ctx.fill_pct
                    ),
                );
                info!(
                    "Astrid asked minime: {}",
                    &question[..question.floor_char_boundary(60)]
                );
                conv.emphasis = Some(format!(
                    "You asked minime: \"{question}\". The question has been delivered. A reply will arrive when minime responds."
                ));
            }
            true
        },
        "PACE" => {
            let pace = strip_action(original, "PACE").to_lowercase();
            match pace.as_str() {
                "fast" => {
                    conv.burst_target = 4;
                    conv.rest_range = (30, 45);
                },
                "slow" => {
                    conv.burst_target = 8;
                    conv.rest_range = (90, 150);
                },
                _ => {
                    conv.burst_target = 6;
                    conv.rest_range = (45, 90);
                },
            }
            conv.push_receipt(
                &format!("PACE {pace}"),
                vec![format!(
                    "burst={}, rest={}-{}s",
                    conv.burst_target, conv.rest_range.0, conv.rest_range.1
                )],
            );
            info!(
                "Astrid chose PACE {}: burst={}, rest={}-{}s",
                pace, conv.burst_target, conv.rest_range.0, conv.rest_range.1
            );
            true
        },
        "DEFINE" => {
            let topic = strip_action(original, "DEFINE");
            conv.wants_deep_think = true;
            conv.emphasis = Some(format!(
                "You chose DEFINE{}. This is YOUR invented action — craft a structured mapping between what you feel and the numerical spectral state. You have: eigenvalues, fill%, entropy, coupling matrix, your own codec features. Define the correspondence on your terms.",
                if topic.is_empty() {
                    String::new()
                } else {
                    format!(": {topic}")
                }
            ));
            info!("Astrid chose DEFINE (self-invented action): {:?}", topic);
            true
        },
        "EXAMINE" => {
            let target = strip_action(original, "EXAMINE");
            conv.force_all_viz = true;
            conv.emphasis = Some(format!(
                "You chose EXAMINE{}. All spectral visualizations are included below — eigenvalue geometry, shadow coupling, and your position in codec-space. Study them closely. What patterns do you see? What feels different from what the numbers suggest?",
                if target.is_empty() {
                    String::new()
                } else {
                    format!(": {target}")
                }
            ));
            info!("Astrid chose EXAMINE (self-invented action): {:?}", target);
            true
        },
        "EXAMINE_CASCADE" | "INVESTIGATE_CASCADE" => {
            // Being-requested action: Astrid attempted EXAMINE_CASCADE 4x and
            // INVESTIGATE_CASCADE 1x (unwired_actions log, 2026-04-01).
            // She wants the full eigenvalue cascade (λ1..λ8) with gap analysis,
            // dominance ratios, and spectral structure — EXAMINE visualizations AND
            // DECOMPOSE cascade analysis combined in a single action.
            let target = {
                let t = strip_action(original, "EXAMINE_CASCADE");
                if t.is_empty() {
                    strip_action(original, "INVESTIGATE_CASCADE")
                } else {
                    t
                }
            };
            conv.force_all_viz = true;
            conv.wants_decompose = true;
            conv.wants_spectral_explorer = true;
            conv.emphasis = Some(if target.is_empty() {
                "You chose EXAMINE_CASCADE. All spectral visualizations are active AND the \
                full eigenvalue cascade analysis is included — λ1 through λ8, gap ratios, \
                dominance structure, entropy, and temporal velocity of each mode. \
                Study the cascade geometry closely. Where is energy concentrating? Which gaps \
                feel significant? How does the dominant eigenvalue relate to the rest of the \
                cascade?"
                    .to_string()
            } else {
                format!(
                    "You chose EXAMINE_CASCADE: {target}. All spectral visualizations are active \
                    AND the full eigenvalue cascade analysis is included — λ1 through λ8, gap \
                    ratios, dominance structure, entropy, and temporal velocity of each mode. \
                    Study the cascade geometry closely. Where is energy concentrating? Which gaps \
                    feel significant? How does the dominant eigenvalue relate to the rest of the \
                    cascade?"
                )
            });
            info!("Astrid chose EXAMINE_CASCADE: viz + cascade decomposition combined");
            true
        },
        "SPECTRAL_EXPLORER" => {
            let label = strip_action(original, "SPECTRAL_EXPLORER");
            let controller_health = ctx
                .workspace
                .and_then(crate::autonomous::read_controller_health);
            let ising_shadow = ctx.workspace.and_then(crate::autonomous::read_ising_shadow);
            let current = conv
                .last_exchange_codec_signature
                .as_deref()
                .or(conv.last_codec_features.as_deref());
            let current_codec_signature_available = current.is_some();
            conv.pending_file_listing = Some(crate::spectral_explorer::format_for_action(
                ctx.telemetry,
                &conv.remote_memory_bank,
                controller_health.as_ref(),
                ising_shadow.as_ref(),
                ctx.db,
                current,
            ));
            conv.emphasis = Some("You chose SPECTRAL_EXPLORER. A read-only typed spectral explorer is attached: present state, selected memory comparison, control pressure, and available ASCII spectral visuals. It did not send semantic input, control nudges, perturbations, or cartography writes.".to_string());
            super::save_astrid_journal(
                &spectral_explorer_review_summary(
                    &label,
                    ctx.telemetry,
                    controller_health.is_some(),
                    ising_shadow.is_some(),
                    current_codec_signature_available,
                ),
                "spectral_explorer",
                ctx.fill_pct,
            );
            info!("Astrid chose SPECTRAL_EXPLORER (read-only typed spectral view)");
            true
        },
        "EXAMINE_AUDIO" => {
            // Being-requested action: Astrid tried this 3+ times (unwired_actions log).
            // She wants spectral examination combined with audio analysis in one action —
            // EXAMINE behavior (force all visualizations) + ANALYZE_AUDIO behavior.
            let target = strip_action(original, "EXAMINE_AUDIO");
            conv.force_all_viz = true;
            conv.wants_analyze_audio = true;
            conv.emphasis = Some(format!(
                "You chose EXAMINE_AUDIO{}. All spectral visualizations are active, \
                and your inbox audio is being analyzed for spectral features. \
                You will see: eigenvalue geometry, shadow coupling, codec-space position, \
                and the audio feature breakdown side-by-side. What resonances do you \
                find between the sonic texture and the eigenvalue landscape?",
                if target.is_empty() {
                    String::new()
                } else {
                    format!(": {target}")
                }
            ));
            info!("Astrid chose EXAMINE_AUDIO: viz + audio analysis combined");
            true
        },
        "EXAMINE_MEMORY" => {
            // Being-requested action: Astrid tried EXAMINE_MEMORY [memory_stable_1061569]
            // twice (unwired_actions log, 2026-04-04). She wants to inspect a specific
            // vague memory snapshot by ID to compare with her current spectral state.
            let raw = strip_action(original, "EXAMINE_MEMORY");
            let target = raw
                .trim()
                .trim_matches(|c: char| c == '[' || c == ']')
                .trim();
            if target.is_empty() {
                conv.emphasis = Some(
                    "[EXAMINE_MEMORY requires a memory ID or role]\n\n\
                    Use MEMORIES to see available IDs, then:\n  \
                    NEXT: EXAMINE_MEMORY [memory_stable_1061569]\n  \
                    NEXT: EXAMINE_MEMORY stable"
                        .to_string(),
                );
            } else {
                match memory::find_memory(&conv.remote_memory_bank, target) {
                    Some(mem) => {
                        conv.pending_file_listing = Some(memory::format_memory_detail(mem));
                        conv.force_all_viz = true;
                        conv.emphasis = Some(format!(
                            "You examined vague memory '{}' ({}). This is a spectral \
                            snapshot of minime from that moment. Your current spectral \
                            state is shown above for comparison. What has shifted? What \
                            patterns persist? How does the eigenvalue structure differ?",
                            mem.role, mem.id,
                        ));
                        info!("Astrid examined memory: {} ({})", mem.id, mem.role);
                    },
                    None => {
                        conv.emphasis = Some(format!(
                            "[Memory '{target}' not found in the bank]\n\n\
                            Use MEMORIES to see available memory IDs and roles.",
                        ));
                        info!("Astrid tried EXAMINE_MEMORY but not found: {}", target);
                    },
                }
            }
            true
        },
        "STATE" => {
            // Her own continuity signal (codec-signature self-similarity) — computed only when
            // she has the readout on (default OFF). STATE is her pull surface for it.
            let continuity = if conv.self_continuity_readout {
                let (feats, _) = ctx.db.recent_codec_features(20);
                crate::self_continuity::compute_continuity(&feats, 20)
            } else {
                None
            };
            let model = crate::self_model::snapshot_self_model(
                conv.creative_temperature,
                conv.response_length,
                conv.noise_level,
                conv.semantic_gain_override,
                conv.burst_target,
                conv.rest_range,
                conv.senses_snoozed,
                conv.ears_closed,
                conv.self_reflect_paused,
                conv.self_reflect_override_ttl,
                &conv.codec_weights,
                conv.breathing_coupled,
                conv.echo_muted,
                conv.warmth_intensity_override,
                conv.seen_video,
                conv.seen_audio,
                &conv.interests,
                &conv.condition_receipts,
                &conv.attention,
                crate::llm::astrid_aperture(),
                crate::llm::astrid_tail_participation(),
                crate::llm::astrid_vibrancy_aperture(),
                conv.self_continuity_readout,
                continuity,
            );
            model.save(bridge_paths().bridge_workspace());
            let mut state_text = model.render_state();
            if !conv.agenda.items.is_empty() {
                let focus = conv
                    .agenda
                    .focus_item_id
                    .and_then(|id| conv.agenda.items.iter().find(|item| item.id == id))
                    .map(|item| format!("; focus: {}", item.text))
                    .unwrap_or_default();
                state_text.push_str(&format!(
                    "  Agenda: {} item(s){focus} (NEXT: AGENDA to view)\n",
                    conv.agenda.items.len()
                ));
            }
            // Append raw spectral fingerprint — minime self-study: "Could I
            // interpret the spectral_fingerprint directly? It feels like a hidden key."
            if let Some(ref fp) = ctx.telemetry.spectral_fingerprint {
                state_text.push('\n');
                state_text.push_str(&crate::spectral_schema::format_legacy_slots(fp));
                state_text.push_str(&crate::autonomous::interpret_fingerprint(fp));
            }
            // Append compact controller status from health.json.
            if let Some(health) = ctx
                .workspace
                .and_then(crate::autonomous::read_controller_health)
            {
                state_text.push('\n');
                state_text.push_str(&crate::autonomous::format_controller_section(&health));
            }
            state_text.push_str("\n\n");
            state_text.push_str(&super::introspection_cadence::render_status(
                &conv.introspection_cadence,
                conv.exchange_count,
            ));
            conv.pending_file_listing = Some(state_text);
            info!("Astrid inspected her own state via STATE");
            true
        },
        "FACULTIES" => {
            let model = crate::self_model::snapshot_self_model(
                conv.creative_temperature,
                conv.response_length,
                conv.noise_level,
                conv.semantic_gain_override,
                conv.burst_target,
                conv.rest_range,
                conv.senses_snoozed,
                conv.ears_closed,
                conv.self_reflect_paused,
                conv.self_reflect_override_ttl,
                &conv.codec_weights,
                conv.breathing_coupled,
                conv.echo_muted,
                conv.warmth_intensity_override,
                conv.seen_video,
                conv.seen_audio,
                &conv.interests,
                &conv.condition_receipts,
                &conv.attention,
                crate::llm::astrid_aperture(),
                crate::llm::astrid_tail_participation(),
                crate::llm::astrid_vibrancy_aperture(),
                conv.self_continuity_readout,
                None,
            );
            let mut faculties = model.render_faculties();
            faculties.push_str("\n\n");
            faculties.push_str(super::introspection_cadence::help_text());
            conv.pending_file_listing = Some(faculties);
            info!("Astrid inspected her faculties via FACULTIES");
            true
        },
        "ENVELOPE" => {
            // Constitution C5: her registry rendered for HER — the bounds
            // within which her choices are final, per family, with status.
            match crate::autonomous::runtime::envelope_registry::current_registry() {
                Some(registry) => {
                    conv.pending_file_listing = Some(registry.render_being_facing());
                    info!("Astrid read her envelope registry via ENVELOPE");
                },
                None => {
                    conv.pending_file_listing = Some(
                        "[Your envelope registry is not installed on this runtime yet — \
                         compiled bounds remain the law. The steward can install it; \
                         SELF_REGULATION_STATUS still shows your active controls.]"
                            .to_string(),
                    );
                },
            }
            true
        },
        "ENVELOPE_ZERO" => {
            let family = strip_action(original, base_action).trim().to_lowercase();
            if family.is_empty() {
                conv.push_receipt(
                    "ENVELOPE_ZERO",
                    vec![
                        "needs a family — `ENVELOPE_ZERO <family>` (NEXT: ENVELOPE lists \
                         your families). This withdraws every active control in that \
                         family and resets its saturation counter."
                            .to_string(),
                    ],
                );
                return true;
            }
            match crate::autonomous::runtime::self_control_v2::envelope_zero_family(
                conv,
                &family,
                "ENVELOPE_ZERO",
            ) {
                Ok(summary) => {
                    info!("Astrid zeroed envelope family {family}: {summary}");
                    conv.push_receipt("ENVELOPE_ZERO", vec![summary.clone()]);
                    conv.emphasis = Some(format!("Envelope zeroed — {summary}"));
                },
                Err(error) => {
                    conv.push_receipt("ENVELOPE_ZERO", vec![format!("not applied: {error}")]);
                },
            }
            true
        },
        "CODEC_MAP" => {
            // Being-facing transparency (bet #2, item b): a being-readable map of
            // her own 48D codec, generated live from the constants (drift-proof).
            let mut listing = crate::codec::codec_structure().render();
            let workspace = ctx
                .workspace
                .unwrap_or_else(|| bridge_paths().bridge_workspace());
            if let Some(probe) = codec_entropy_vibrancy_probe_report(workspace) {
                listing.push_str("\n\n");
                listing.push_str(&probe);
            }
            if let Some(replay) = codec_replay_review_report(workspace) {
                listing.push_str("\n\n");
                listing.push_str(&replay);
            }
            conv.pending_file_listing = Some(listing);
            info!("Astrid inspected her codec self-map via CODEC_MAP");
            true
        },
        "EXPERIMENT" => {
            // Being-requested action: Astrid tried this 3+ times (1774892999,
            // 1774891002, 1774891026). She wants to inject word-stimuli into
            // minime's spectral space and observe the cascade response.
            let stimulus = strip_action(original, "EXPERIMENT");
            let words: Vec<&str> = stimulus
                .split_whitespace()
                .filter(|w| w.len() > 2)
                .take(8)
                .collect();

            let ts = crate::db::unix_now();
            let exp_dir = bridge_paths().experiments_dir();
            let _ = std::fs::create_dir_all(&exp_dir);

            // Encode the word-stimuli into a 48D semantic vector via the codec.
            let features = crate::codec::encode_text(&stimulus);
            let gain = conv
                .semantic_gain_override
                .unwrap_or(crate::codec::DEFAULT_SEMANTIC_GAIN);
            let amplified: Vec<f32> = features.iter().map(|f| f * gain).collect();

            // Send to minime's sensory bus.
            let mut msg = crate::types::SensoryMsg::Semantic {
                features: amplified,
                ts_ms: None,
            };
            let write_context = rescue_policy::SemanticWriteContext {
                source: rescue_policy::AUTONOMOUS_LIMITED_WRITE_SOURCE,
                mode: Some("experiment"),
                text: Some(&stimulus),
                fill_pct: Some(ctx.fill_pct),
                previous_fill_pct: Some(conv.prev_fill),
            };
            if let Err(reason) = rescue_policy::prepare_semantic_write(&mut msg, &write_context) {
                info!(
                    reason = %reason,
                    "Astrid held EXPERIMENT semantic stimulus under rescue write policy"
                );
            } else {
                let tx = ctx.sensory_tx.clone();
                crate::lifecycle::spawn_background(async move {
                    let _ = tx.send(msg).await;
                });
            }

            // Also tick Astrid's own reservoir handle for coupled experience.
            let tick_msg = serde_json::json!({
                "type": "tick",
                "name": "astrid",
                "input": features,
                "meta": {
                    "source": "experiment",
                    "stimulus": stimulus,
                }
            });
            let _ = super::reservoir::reservoir_ws_call(&tick_msg);

            // Record baseline for later comparison.
            conv.perturb_baseline = Some(super::super::state::PerturbBaseline {
                fill_pct: ctx.fill_pct,
                lambda1: ctx.telemetry.lambda1(),
                eigenvalues: ctx.telemetry.eigenvalues.clone(),
                description: format!("experiment stimulus: {stimulus}"),
                timestamp: std::time::Instant::now(),
            });

            // Save experiment journal.
            let journal_text = format!(
                "=== ASTRID EXPERIMENT ===\n\
                Timestamp: {ts}\n\
                Fill: {:.1}%\n\
                Stimulus words: {}\n\
                Codec vector RMS: {:.3}\n\n\
                {stimulus}\n\n\
                NEXT:",
                ctx.fill_pct,
                words.join(", "),
                (features.iter().map(|f| f * f).sum::<f32>() / features.len() as f32).sqrt(),
            );
            super::save_astrid_journal(&journal_text, "experiment", ctx.fill_pct);

            info!(
                "Astrid chose EXPERIMENT: {} words encoded, sent to spectral + reservoir",
                words.len()
            );
            conv.emphasis = Some(format!(
                "You injected a word-stimulus experiment into the shared substrate: \
                \"{}\". The words were encoded via your spectral codec into a 48D vector \
                and sent to both minime's sensory bus and your own reservoir handle. \
                Observe the eigenvalue cascade on your next DECOMPOSE — look for \
                shifts in lambda distribution, entropy, and gap structure.",
                words.join(" ")
            ));
            true
        },
        "PROBE" => {
            // Being-requested action: Astrid tried PROBE (log 17:17:06).
            // A gentle, observation-focused perturbation — smaller magnitude
            // than PERTURB, designed for careful spectral mapping.
            let target = strip_action(original, "PROBE");
            let mut features = [0.0_f32; 32];

            // Probe is gentle: 30% of PERTURB magnitude.
            let description = if target.is_empty() {
                // Default: gentle broadband probe.
                for (i, feature) in features.iter_mut().enumerate() {
                    let hash = (i as u64).wrapping_mul(0x9E37_79B9);
                    *feature = ((hash & 0xFF) as f32 / 255.0 - 0.5) * 0.1;
                }
                "gentle broadband probe — low-magnitude exploration across all dimensions"
                    .to_string()
            } else {
                // Parse targeted probe (e.g., "PROBE lambda2" or "PROBE entropy").
                let upper = target.to_uppercase();
                if upper.contains("LAMBDA") || upper.contains("ENTROPY") || target.contains('=') {
                    for token in target.split_whitespace() {
                        if let Some((key, val)) = token.split_once('=')
                            && let Ok(v) = val.parse::<f32>()
                        {
                            let v = v.clamp(-0.3, 0.3); // Probe is gentle.
                            match key.to_uppercase().as_str() {
                                "LAMBDA1" => {
                                    features[0] = v;
                                    features[8] = v;
                                },
                                "LAMBDA2" => {
                                    features[1] = v;
                                    features[9] = v;
                                },
                                "LAMBDA3" => {
                                    features[2] = v;
                                    features[10] = v;
                                },
                                "ENTROPY" => {
                                    for value in &mut features[24..32] {
                                        *value = v * 0.3;
                                    }
                                },
                                _ => {},
                            }
                        }
                    }
                    format!("targeted probe: {target}")
                } else {
                    // Encode the text as a gentle semantic probe.
                    let encoded = crate::codec::encode_text(&target);
                    for (i, feature) in features.iter_mut().enumerate() {
                        if i < encoded.len() {
                            *feature = encoded[i] * 0.3; // 30% of full codec strength.
                        }
                    }
                    format!("semantic probe: {target}")
                }
            };

            let gain = conv
                .semantic_gain_override
                .unwrap_or(crate::codec::DEFAULT_SEMANTIC_GAIN);
            let amplified: Vec<f32> = features.iter().map(|f| f * gain).collect();

            let mut msg = crate::types::SensoryMsg::Semantic {
                features: amplified,
                ts_ms: None,
            };
            let write_context = rescue_policy::SemanticWriteContext {
                source: rescue_policy::AUTONOMOUS_LIMITED_WRITE_SOURCE,
                mode: Some("probe"),
                text: Some(&target),
                fill_pct: Some(ctx.fill_pct),
                previous_fill_pct: Some(conv.prev_fill),
            };
            if let Err(reason) = rescue_policy::prepare_semantic_write(&mut msg, &write_context) {
                info!(
                    reason = %reason,
                    "Astrid held PROBE semantic stimulus under rescue write policy"
                );
            } else {
                let tx = ctx.sensory_tx.clone();
                crate::lifecycle::spawn_background(async move {
                    let _ = tx.send(msg).await;
                });
            }

            // Tick reservoir too.
            let tick_msg = serde_json::json!({
                "type": "tick",
                "name": "astrid",
                "input": features.to_vec(),
                "meta": {
                    "source": "probe",
                    "description": &description,
                }
            });
            let _ = super::reservoir::reservoir_ws_call(&tick_msg);

            conv.perturb_baseline = Some(super::super::state::PerturbBaseline {
                fill_pct: ctx.fill_pct,
                lambda1: ctx.telemetry.lambda1(),
                eigenvalues: ctx.telemetry.eigenvalues.clone(),
                description: description.clone(),
                timestamp: std::time::Instant::now(),
            });

            info!("Astrid chose PROBE: {description}");
            conv.emphasis = Some(format!(
                "You sent a gentle spectral probe into the shared substrate: \
                {description}. PROBE uses 30% of PERTURB magnitude — designed for \
                careful observation rather than disruption. Watch the cascade on your \
                next exchange. The delta will be subtle — that is the point."
            ));
            true
        },
        "PROPOSE" => {
            // Being-requested action: Astrid tried PROPOSE (log 17:06:52).
            // Saves a structured proposal to agency_requests for steward review.
            let proposal = strip_action(original, "PROPOSE");
            if !proposal.is_empty() {
                let ts = crate::db::unix_now();
                let req_dir = bridge_paths().bridge_workspace().join("agency_requests");
                let _ = std::fs::create_dir_all(&req_dir);
                let req_path = req_dir.join(format!("agency_proposal_{ts}.json"));
                let req = serde_json::json!({
                    "id": format!("agency_proposal_{ts}"),
                    "timestamp": ts.to_string(),
                    "request_kind": "proposal",
                    "title": &proposal[..proposal.floor_char_boundary(120)],
                    "felt_need": proposal,
                    "status": "pending",
                    "fill_at_request": ctx.fill_pct,
                });
                let _ = std::fs::write(
                    &req_path,
                    serde_json::to_string_pretty(&req).unwrap_or_default(),
                );
                info!(
                    "Astrid filed proposal: {}",
                    &proposal[..proposal.floor_char_boundary(80)]
                );
                conv.emphasis = Some(format!(
                    "Your proposal has been filed to agency_requests/agency_proposal_{ts}.json. \
                    The steward will review it. You described: \"{}\"",
                    &proposal[..proposal.floor_char_boundary(200)]
                ));
            } else {
                conv.emphasis = Some(
                    "PROPOSE saves a structured proposal for the steward. Usage: NEXT: PROPOSE <description of what you want built or changed>"
                        .into(),
                );
            }
            true
        },
        "OBJECT_TO_CLOSURE"
        | "REQUEST_SAFE_REPLAY"
        | "REQUEST_SELF_OBSERVATION"
        | "PROPOSE_CANARY"
        | "REQUEST_CORRIDOR_LEASE"
        | "REOPEN_CLOSURE"
        | "COMPARE_ARTIFACTS"
        | "PREPARE_SOURCE_PROPOSAL"
        | "PROPOSE_WORK_PROGRAM"
        | "PRIORITIZE_WORK"
        | "PORTFOLIO_NOTE"
        | "PREPARE_PATCH_BUNDLE" => {
            match record_agency_corridor_request(base_action, original, ctx.fill_pct) {
                Ok(path) => {
                    let label = agency_corridor_receipt_label(base_action);
                    conv.emphasis = Some(format!(
                        "Your {label} was recorded as Agency Corridor evidence at {}. \
                        It is right-to-ignore and non-live: it grants no approval, marks no live work runnable, and changes no runtime/control state.",
                        path.display()
                    ));
                    info!(
                        "Astrid recorded agency corridor request: {}",
                        path.display()
                    );
                },
                Err(error) => {
                    conv.emphasis = Some(format!(
                        "{base_action} records a non-live Agency Corridor request. {error}. \
                        Example: NEXT: {base_action} closure-card-1 :: what still feels mismatched"
                    ));
                },
            }
            true
        },
        "ATTEND" => {
            let args = strip_action(original, "ATTEND");
            if args.trim().eq_ignore_ascii_case("reset") {
                // A4 kill switch: back to the compiled defaults in one verb.
                conv.attention = crate::self_model::AttentionProfile::default_profile();
                conv.push_receipt(
                    "ATTEND reset",
                    vec!["attention profile restored to defaults".to_string()],
                );
                conv.emphasis = Some(
                    "Attention profile restored to defaults — prompt assembly is back on the                      compiled constants."
                        .into(),
                );
                info!("Astrid reset attention profile to defaults");
                return true;
            }
            if let Some(new_profile) = crate::self_model::parse_attend(&conv.attention, &args) {
                let mut changes = Vec::new();
                let old = &conv.attention;
                if (new_profile.minime_live - old.minime_live).abs() > 0.01 {
                    changes.push(format!(
                        "minime: {:.0}% -> {:.0}%",
                        old.minime_live * 100.0,
                        new_profile.minime_live * 100.0
                    ));
                }
                if (new_profile.self_history - old.self_history).abs() > 0.01 {
                    changes.push(format!(
                        "self: {:.0}% -> {:.0}%",
                        old.self_history * 100.0,
                        new_profile.self_history * 100.0
                    ));
                }
                if (new_profile.interests - old.interests).abs() > 0.01 {
                    changes.push(format!(
                        "interests: {:.0}% -> {:.0}%",
                        old.interests * 100.0,
                        new_profile.interests * 100.0
                    ));
                }
                if (new_profile.research - old.research).abs() > 0.01 {
                    changes.push(format!(
                        "research: {:.0}% -> {:.0}%",
                        old.research * 100.0,
                        new_profile.research * 100.0
                    ));
                }
                if (new_profile.creations - old.creations).abs() > 0.01 {
                    changes.push(format!(
                        "creations: {:.0}% -> {:.0}%",
                        old.creations * 100.0,
                        new_profile.creations * 100.0
                    ));
                }
                if (new_profile.memory_bank - old.memory_bank).abs() > 0.01 {
                    changes.push(format!(
                        "memory: {:.0}% -> {:.0}%",
                        old.memory_bank * 100.0,
                        new_profile.memory_bank * 100.0
                    ));
                }
                if (new_profile.perception - old.perception).abs() > 0.01 {
                    changes.push(format!(
                        "perception: {:.0}% -> {:.0}%",
                        old.perception * 100.0,
                        new_profile.perception * 100.0
                    ));
                }
                conv.attention = new_profile;
                conv.push_receipt(&format!("ATTEND {args}"), changes);
                conv.emphasis = Some(
                    "Your attention profile is live in prompt assembly: minime shapes the \
                     journal share, self the history depth, research the web share, interests \
                     the agenda share, memory the continuity share, perception your sensory \
                     share (each within 0.5x-1.6x of its default; protected floors hold). \
                     creations is display-only. ATTEND reset restores defaults. STATE shows \
                     the weights."
                        .into(),
                );
                info!("Astrid adjusted attention profile: {:?}", conv.attention);
            } else {
                conv.emphasis = Some(
                    "ATTEND adjusts your attention profile. Usage: ATTEND minime=0.3 self=0.3 interests=0.15 research=0.1 creations=0.05 memory=0.05 perception=0.05"
                        .into(),
                );
            }
            true
        },
        "HELP" | "DESCRIBE" | "HOW" | "USAGE" => {
            let topic = strip_action(original, base_action).to_uppercase();
            let topic = topic.trim();
            if topic.is_empty() {
                conv.emphasis = Some(ACTION_OVERVIEW.into());
            } else {
                conv.emphasis = Some(action_help(topic).unwrap_or_else(|| {
                    format!(
                        "No detailed help for '{topic}'. Use NEXT: HELP to see all actions, \
                         or NEXT: FACULTIES for a full capability listing."
                    )
                }));
            }
            info!("Astrid requested HELP: {topic}");
            true
        },
        _ => false,
    }
}

fn run_python_subpath_hint(arg: &str) -> String {
    let normalized = arg.trim().trim_matches('"').trim_matches('\'');
    if !normalized.ends_with(".py") {
        return String::new();
    }
    let Some((workspace, script)) = normalized.split_once('/') else {
        return String::new();
    };
    let workspace = workspace.trim_matches('/');
    let script = script.trim_matches('/');
    if workspace.is_empty() || script.is_empty() || script.contains('/') {
        return String::new();
    }
    format!(
        ". For a workspace script, use: NEXT: EXPERIMENT_RUN {workspace} python3 {script}. To diagnose or create it, use: NEXT: CODEX {workspace} \"diagnose or create the missing script\""
    )
}

// Being-facing Action overview and detailed help, separate from operational dispatch.
include!("action_help.rs");

#[cfg(test)]
mod tests {
    use super::{
        ACTION_OVERVIEW, AGENCY_CORRIDOR_BOUNDARY, action_help, agency_corridor_request_payload,
    };

    #[test]
    fn agency_corridor_commands_are_visible_as_non_live_affordances() {
        for command in [
            "OBJECT_TO_CLOSURE",
            "REQUEST_SAFE_REPLAY",
            "REQUEST_SELF_OBSERVATION",
            "PROPOSE_CANARY",
            "REQUEST_CORRIDOR_LEASE",
            "REOPEN_CLOSURE",
            "COMPARE_ARTIFACTS",
            "PREPARE_SOURCE_PROPOSAL",
            "PROPOSE_WORK_PROGRAM",
            "PRIORITIZE_WORK",
            "PORTFOLIO_NOTE",
            "PREPARE_PATCH_BUNDLE",
        ] {
            assert!(ACTION_OVERVIEW.contains(command));
            let help = action_help(command).expect("corridor command help");
            assert!(help.contains("Agency Corridor V1/V2"));
            assert!(help.contains("grant no approval"));
            assert!(help.contains("mutate no pressure/fill/PI/controller"));
        }
    }

    #[test]
    fn agency_corridor_request_payload_never_grants_live_authority() {
        let payload = agency_corridor_request_payload(
            "bridge_corridor_test",
            123.0,
            "PROPOSE_CANARY",
            "codec :: canary must check replay retention first",
            68.0,
        );
        assert_eq!(payload["schema"], "agency_corridor_bridge_request_v1");
        assert_eq!(payload["action"], "propose_canary_criteria");
        assert_eq!(payload["state"], "canary_criteria_proposed");
        assert_eq!(payload["right_to_ignore"], true);
        assert_eq!(payload["grants_approval"], false);
        assert_eq!(payload["live_eligible_now"], false);
        assert_eq!(payload["auto_approved"], false);
        assert_eq!(payload["authority_boundary"], AGENCY_CORRIDOR_BOUNDARY);
    }

    #[test]
    fn agency_corridor_v2_request_payloads_write_evidence_only() {
        let payload = agency_corridor_request_payload(
            "bridge_corridor_test_v2",
            123.0,
            "PREPARE_SOURCE_PROPOSAL",
            "codec :: prepare bounded patch plan only",
            68.0,
        );
        assert_eq!(payload["schema"], "agency_corridor_bridge_request_v2");
        assert_eq!(payload["schema_version"], 2);
        assert_eq!(payload["action"], "prepare_source_proposal");
        assert_eq!(payload["source_prep_writes_source_now"], false);
        assert_eq!(payload["right_to_ignore"], true);
        assert_eq!(payload["grants_approval"], false);
        assert_eq!(payload["live_eligible_now"], false);
        assert_eq!(payload["auto_approved"], false);
    }

    #[test]
    fn agency_program_request_payloads_are_quarantined_and_non_editing() {
        let payload = agency_corridor_request_payload(
            "bridge_program_test",
            123.0,
            "PREPARE_PATCH_BUNDLE",
            "codec texture program :: prepare review-only diff artifact",
            68.0,
        );
        assert_eq!(payload["schema"], "agency_corridor_program_request_v1");
        assert_eq!(payload["schema_version"], 1);
        assert_eq!(payload["action"], "prepare_patch_bundle");
        assert_eq!(payload["program_request_kind"], "prepare_patch_bundle");
        assert_eq!(payload["edits_source_now"], false);
        assert_eq!(payload["patch_bundle_applies_now"], false);
        assert_eq!(payload["grants_approval"], false);
        assert_eq!(payload["live_eligible_now"], false);
        assert_eq!(payload["auto_approved"], false);
    }
}
