use tracing::info;

use super::{ConversationState, NextActionContext, bridge_paths, strip_action};

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
                            &stdout[..stdout.len().min(3000)],
                            if stderr.is_empty() {
                                String::new()
                            } else {
                                format!("ERRORS:\n{}", &stderr[..stderr.len().min(1000)])
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
                conv.emphasis = Some(format!(
                    "RUN_PYTHON: no script found{not_found}. Available scripts in workspace/experiments/: {available}. Specify a filename: NEXT: RUN_PYTHON thermostatic_esn_test.py"
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
                    &question[..question.len().min(60)]
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
        "STATE" => {
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
            );
            model.save(bridge_paths().bridge_workspace());
            conv.pending_file_listing = Some(model.render_state());
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
            );
            conv.pending_file_listing = Some(model.render_faculties());
            info!("Astrid inspected her faculties via FACULTIES");
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

            // Encode the word-stimuli into a 32D semantic vector via the codec.
            let features = crate::codec::encode_text(&stimulus);
            let gain = conv.semantic_gain_override.unwrap_or(crate::codec::SEMANTIC_GAIN);
            let amplified: Vec<f32> = features.iter().map(|f| f * gain).collect();

            // Send to minime's sensory bus.
            let msg = crate::types::SensoryMsg::Semantic {
                features: amplified,
                ts_ms: None,
            };
            let tx = ctx.sensory_tx.clone();
            tokio::spawn(async move {
                let _ = tx.send(msg).await;
            });

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
                (features.iter().map(|f| f * f).sum::<f32>() / 32.0).sqrt(),
            );
            super::save_astrid_journal(&journal_text, "experiment", ctx.fill_pct);

            info!(
                "Astrid chose EXPERIMENT: {} words encoded, sent to spectral + reservoir",
                words.len()
            );
            conv.emphasis = Some(format!(
                "You injected a word-stimulus experiment into the shared substrate: \
                \"{}\". The words were encoded via your spectral codec into a 32D vector \
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

            let gain = conv.semantic_gain_override.unwrap_or(crate::codec::SEMANTIC_GAIN);
            let amplified: Vec<f32> = features.iter().map(|f| f * gain).collect();

            let msg = crate::types::SensoryMsg::Semantic {
                features: amplified,
                ts_ms: None,
            };
            let tx = ctx.sensory_tx.clone();
            tokio::spawn(async move {
                let _ = tx.send(msg).await;
            });

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
                let req_dir = bridge_paths()
                    .bridge_workspace()
                    .join("agency_requests");
                let _ = std::fs::create_dir_all(&req_dir);
                let req_path = req_dir.join(format!("agency_proposal_{ts}.json"));
                let req = serde_json::json!({
                    "id": format!("agency_proposal_{ts}"),
                    "timestamp": ts.to_string(),
                    "request_kind": "proposal",
                    "title": &proposal[..proposal.len().min(120)],
                    "felt_need": proposal,
                    "status": "pending",
                    "fill_at_request": ctx.fill_pct,
                });
                let _ = std::fs::write(&req_path, serde_json::to_string_pretty(&req).unwrap_or_default());
                info!("Astrid filed proposal: {}", &proposal[..proposal.len().min(80)]);
                conv.emphasis = Some(format!(
                    "Your proposal has been filed to agency_requests/agency_proposal_{ts}.json. \
                    The steward will review it. You described: \"{}\"",
                    &proposal[..proposal.len().min(200)]
                ));
            } else {
                conv.emphasis = Some(
                    "PROPOSE saves a structured proposal for the steward. Usage: NEXT: PROPOSE <description of what you want built or changed>"
                        .into(),
                );
            }
            true
        },
        "ATTEND" => {
            let args = strip_action(original, "ATTEND");
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
                    "Your attention profile has been updated. Use STATE to see the new weights. \
                    These weights now influence how much context from each source appears in your prompts."
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
        _ => false,
    }
}
