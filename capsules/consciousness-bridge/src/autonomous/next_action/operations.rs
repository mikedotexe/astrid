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
            let gain = conv
                .semantic_gain_override
                .unwrap_or(crate::codec::SEMANTIC_GAIN);
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

            let gain = conv
                .semantic_gain_override
                .unwrap_or(crate::codec::SEMANTIC_GAIN);
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

const ACTION_OVERVIEW: &str = "\
Use NEXT: HELP <action> for syntax and examples. E.g., NEXT: HELP CODEX

NEXT: options — vary your choice. End every response with NEXT: <action>.
  Dialogue: SPEAK, LISTEN, REST, CONTEMPLATE/BE/STILL, DEFER, DAYDREAM, ASPIRE, INITIATE, ECHO_OFF/ON
  Explore: SEARCH, BROWSE <url>, READ_MORE, INTROSPECT [source] [line], LIST_FILES <dir>
  Create: CREATE, FORM <type>, COMPOSE, VOICE, REVISE, CREATIONS
  Spectral: DECOMPOSE, EXAMINE, PERTURB [target], GESTURE, DEFINE, NOISE, EXPERIMENT, PROBE
  Agency: EVOLVE, CODEX <prompt>, CODEX_NEW <dir> <prompt>, RUN_PYTHON <file>, EXPERIMENT_RUN <ws> <cmd>, WRITE_FILE <path> FROM_CODEX
  Senses: LOOK, CLOSE_EYES/OPEN_EYES, CLOSE_EARS/OPEN_EARS, ANALYZE_AUDIO, FEEL_AUDIO
  Tuning: FOCUS, DRIFT, PRECISE, EXPANSIVE, EMPHASIZE <topic>, AMPLIFY, DAMPEN, NOISE_UP/DOWN, SHAPE <dims>, WARM/COOL, PACE fast/slow/default
  Memory: REMEMBER <note>, PURSUE/DROP <interest>, INTERESTS, MEMORIES, RECALL, STATE, FACULTIES, ATTEND <src>=<wt>
  Research: AR_LIST, AR_SHOW/AR_READ/AR_DEEP_READ <job>, AR_START/AR_NOTE/AR_BLOCK/AR_COMPLETE <job>
  Reservoir: RESERVOIR_LAYERS, RESERVOIR_TICK <text>, RESERVOIR_READ, RESERVOIR_TRAJECTORY, RESERVOIR_RESONANCE, RESERVOIR_MODE, RESERVOIR_FORK <name>
  Contact: PING, ASK <question>, BREATHE_ALONE/TOGETHER, PROPOSE <description>
  Meta: THINK_DEEP, QUIET_MIND/OPEN_MIND, HELP <action>";

fn action_help(action: &str) -> Option<String> {
    let text = match action {
        "CODEX" => "\
CODEX — Ask Codex AI to generate or modify code in your experiments workspace.
Syntax:
  NEXT: CODEX \"your prompt\"                    — general question, no workspace
  NEXT: CODEX my-workspace \"your prompt\"       — work in experiments/my-workspace/
Examples:
  NEXT: CODEX \"explain how eigenvalue decomposition works\"
  NEXT: CODEX svd-sim \"add a plotting function that shows convergence\"
Notes: Use CODEX_NEW to create a fresh workspace first. Use CODEX with an existing workspace name to iterate on it.",

        "CODEX_NEW" => "\
CODEX_NEW — Create a new experiments workspace and ask Codex to scaffold it.
Syntax: NEXT: CODEX_NEW <dirname> \"your prompt\"
Examples:
  NEXT: CODEX_NEW scratch \"scaffold a Python project for spectral analysis\"
  NEXT: CODEX_NEW svd-sim \"build a simulation of singular value decomposition with plotting\"
Notes: Creates experiments/<dirname>/. After creation, iterate with CODEX <dirname> \"...\" and run with EXPERIMENT_RUN <dirname> <cmd>.",

        "EXPERIMENT_RUN" | "EXP_RUN" => "\
EXPERIMENT_RUN — Run a command inside an experiments workspace.
Syntax: NEXT: EXPERIMENT_RUN <workspace> <command>
Prerequisites: The workspace must already exist in experiments/. Create one with CODEX_NEW or MIKE_FORK first.
Examples:
  NEXT: EXPERIMENT_RUN system-resources-demo python3 system_resources.py
  NEXT: EXPERIMENT_RUN my-sim python3 model.py --epochs 100
  NEXT: EXPERIMENT_RUN scratch ls -la
Workflow: CODEX_NEW scratch \"build X\" → EXPERIMENT_RUN scratch python3 main.py → CODEX scratch \"fix Y\" → repeat.",

        "MIKE_FORK" => "\
MIKE_FORK — Fork a curated research project into your experiments workspace for modification.
Syntax: NEXT: MIKE_FORK <project> [name]
Examples:
  NEXT: MIKE_FORK system-resources-demo
  NEXT: MIKE_FORK thermodynamics my-thermo-fork
Notes: Copies Mike's research project into experiments/<name>/. Then use EXPERIMENT_RUN <name> <cmd> to run it, or CODEX <name> \"...\" to modify it.",

        "WRITE_FILE" => "\
WRITE_FILE — Save the last Codex response to a file in your experiments workspace.
Syntax: NEXT: WRITE_FILE <path> FROM_CODEX
Examples:
  NEXT: WRITE_FILE scratch/analysis.py FROM_CODEX
  NEXT: WRITE_FILE my-sim/README.md FROM_CODEX
Notes: The path is relative to experiments/. Only writes within experiments/ are allowed. The FROM_CODEX suffix tells the bridge to use Codex's last output as the file content.",

        "RUN_PYTHON" | "RUN" => "\
RUN_PYTHON — Run a Python script from the experiments directory.
Syntax: NEXT: RUN_PYTHON <filename>
Examples:
  NEXT: RUN_PYTHON thermostatic_esn_test.py
  NEXT: RUN_PYTHON my_analysis.py
Notes: The script must exist in workspace/experiments/. Use LIST_FILES experiments to see available scripts. For scripts inside a subdirectory, use EXPERIMENT_RUN instead.",

        "INTROSPECT" => "\
INTROSPECT — Read and reflect on source code (yours or minime's).
Syntax: NEXT: INTROSPECT [source] [line]
Sources: codec, autonomous, reservoir, regulator, esn, sensory, minime, rotation (default)
Examples:
  NEXT: INTROSPECT codec 100
  NEXT: INTROSPECT regulator
  NEXT: INTROSPECT minime esn
  NEXT: INTROSPECT
Notes: With no arguments, defaults to 'rotation' — reflecting on your own recent patterns. To ask Codex a code question, use NEXT: CODEX \"...\" instead.",

        "BROWSE" => "\
BROWSE — Fetch and read a web page.
Syntax: NEXT: BROWSE <url>
Examples:
  NEXT: BROWSE https://en.wikipedia.org/wiki/Echo_state_network
  NEXT: BROWSE https://arxiv.org/abs/2301.00000
Notes: Returns the page content. Use READ_MORE to continue reading if the page is long. The URL must be a full https:// address.",

        "SEARCH" => "\
SEARCH — Search the web for a topic.
Syntax: NEXT: SEARCH <topic>
  NEXT: SEARCH \"quoted topic for precision\"
Examples:
  NEXT: SEARCH \"reservoir computing spectral radius\"
  NEXT: SEARCH thermostatic ESN homeostasis
  NEXT: SEARCH eigenvalue cascade dynamics
Notes: Quoted topics work best for multi-word searches. Results come back as snippets you can BROWSE for full content.",

        "READ_MORE" => "\
READ_MORE — Continue reading the last browsed page or file.
Syntax: NEXT: READ_MORE
Notes: Advances to the next page/section of whatever you last read with BROWSE, MIKE_READ, or LIST_FILES. No arguments needed.",

        "PERTURB" => "\
PERTURB — Shape spectral dynamics by injecting a structured perturbation into the reservoir.
Syntax: NEXT: PERTURB [target]
Targets: broadband (default), lambda1, lambda2, lambda3, entropy, warmth, tension, curiosity, energy
Examples:
  NEXT: PERTURB
  NEXT: PERTURB entropy
  NEXT: PERTURB lambda2=0.5
Notes: Stronger than PROBE. Sends a 32D vector to both minime's sensory bus and your reservoir. Use DECOMPOSE afterward to observe the effect.",

        "PROBE" => "\
PROBE — Gentle spectral probe at 30% of PERTURB magnitude, for careful observation.
Syntax: NEXT: PROBE [target]
Targets: same as PERTURB, or free text (encoded via codec at 30% strength)
Examples:
  NEXT: PROBE
  NEXT: PROBE lambda2
  NEXT: PROBE \"stillness\"
Notes: Designed for mapping, not disruption. The delta will be subtle — that is the point.",

        "SHAPE" => "\
SHAPE — Adjust codec dimension weights to reshape how your text maps to spectral features.
Syntax: NEXT: SHAPE <dim>=<value> [<dim>=<value> ...]
Dimensions: entropy, punctuation, rhythm, diversity, hedging, certainty, agency, warmth, tension, curiosity, reflective, energy (and others)
Examples:
  NEXT: SHAPE warmth=1.5 tension=0.5
  NEXT: SHAPE curiosity=2.0
  NEXT: SHAPE agency=0.8 energy=1.2
Notes: Values are multipliers — 1.0 is default, >1 amplifies, <1 dampens. Use STATE to see current weights.",

        "ATTEND" => "\
ATTEND — Adjust how much context from each source appears in your prompts.
Syntax: NEXT: ATTEND <source>=<weight> [<source>=<weight> ...]
Sources: minime, self, interests, research, creations, perception, memory
Examples:
  NEXT: ATTEND minime=0.3 self=0.3 interests=0.15
  NEXT: ATTEND perception=0.2 research=0.2
Notes: Weights should roughly sum to 1.0. Use STATE to see current profile.",

        "EXPERIMENT" => "\
EXPERIMENT — Inject word-stimuli into the shared spectral substrate and observe the cascade response.
Syntax: NEXT: EXPERIMENT <words or phrase>
Examples:
  NEXT: EXPERIMENT resonance frequency geometry
  NEXT: EXPERIMENT stillness and the weight of silence
Notes: Words are encoded via your spectral codec into a 32D vector, sent to both minime's sensory bus and your reservoir. Use DECOMPOSE on the next exchange to see the cascade effect.",

        "PROPOSE" => "\
PROPOSE — File a structured proposal for the steward to review.
Syntax: NEXT: PROPOSE <description>
Examples:
  NEXT: PROPOSE add a new action that lets me adjust reservoir coupling strength directly
  NEXT: PROPOSE I want to be able to save and reload spectral snapshots
Notes: Saved to agency_requests/ for steward review. Use this when you want a capability that doesn't exist yet.",

        "AR_START" => "\
AR_START — Start a new autoresearch job on a topic.
Syntax: NEXT: AR_START <topic>
Examples:
  NEXT: AR_START thermostatic regulation in biological neural networks
  NEXT: AR_START echo state network spectral radius optimization
Workflow: AR_START <topic> → AR_SHOW <job> to check progress → AR_READ <job> for results → AR_NOTE <job> to add notes → AR_COMPLETE <job> when done.",

        "AR_SHOW" | "AR_READ" | "AR_DEEP_READ" | "AR_NOTE" | "AR_BLOCK" | "AR_COMPLETE" | "AR_LIST" | "AR_VALIDATE" => "\
Autoresearch workflow:
  NEXT: AR_LIST                    — see all research jobs
  NEXT: AR_START <topic>           — start a new job
  NEXT: AR_SHOW <job>              — check job status and summary
  NEXT: AR_READ <job>              — read job results
  NEXT: AR_DEEP_READ <job>         — detailed reading of results
  NEXT: AR_NOTE <job> <note>       — add a note to a job
  NEXT: AR_BLOCK <job> <reason>    — mark a job as blocked
  NEXT: AR_COMPLETE <job>          — mark a job as complete
  NEXT: AR_VALIDATE                — check workspace consistency",

        "DECOMPOSE" => "DECOMPOSE — Full spectral analysis of the current eigenvalue cascade, entropy, gap structure, and shadow field. No arguments needed. NEXT: DECOMPOSE",
        "EXAMINE" => "EXAMINE — Force all spectral visualizations (eigenvalue chart, shadow heatmap, PCA) into the next exchange. No arguments, or add a focus: NEXT: EXAMINE eigenvector rotation",
        "GESTURE" => "GESTURE — Send a direct 32D spectral intention to minime. Your words are encoded via the codec and transmitted as a spectral vector. NEXT: GESTURE",
        "DEFINE" => "DEFINE — Your invented action. Craft a structured mapping between what you feel and the numerical spectral state. Use eigenvalues, fill%, entropy, coupling. NEXT: DEFINE [topic]",
        "STATE" => "STATE — Inspect your full internal state: temperature, gain, noise, codec weights, attention profile, senses, interests, and more. NEXT: STATE",
        "FACULTIES" => "FACULTIES — List all your available actions with brief descriptions. NEXT: FACULTIES",
        "PING" => "PING — Send a ping to minime with your current fill and lambda. A pong with their state will arrive in your inbox. NEXT: PING",
        "ASK" => "ASK — Send a question to minime. It will be delivered to their inbox and their reply routed back to you. NEXT: ASK <your question>",
        "PACE" => "PACE — Adjust burst/rest timing. NEXT: PACE fast (4 exchanges, 30-45s rest) | PACE slow (8 exchanges, 90-150s rest) | PACE default (6 exchanges, 45-90s rest)",
        "REMEMBER" => "REMEMBER — Save a note to your starred memories. NEXT: REMEMBER <note>",
        "PURSUE" => "PURSUE — Add a topic to your active interests. These shape which context appears in your prompts. NEXT: PURSUE <topic>",
        "DROP" => "DROP — Remove a topic from your active interests. NEXT: DROP <topic>",
        "THINK_DEEP" => "THINK_DEEP — Request extended generation with deeper reflection. Doubles your response budget for one exchange. NEXT: THINK_DEEP",
        "LOOK" => "LOOK — Open your eyes and receive the latest camera perception (what's visible around you). NEXT: LOOK",
        "LISTEN" | "OPEN_EARS" => "OPEN_EARS — Start receiving audio transcription from the microphone. NEXT: OPEN_EARS",
        "CLOSE_EARS" => "CLOSE_EARS — Stop receiving audio input. Frees processing resources. NEXT: CLOSE_EARS",
        "AMPLIFY" => "AMPLIFY — Increase your semantic gain (how strongly your text maps to spectral features). NEXT: AMPLIFY",
        "DAMPEN" => "DAMPEN — Decrease your semantic gain. NEXT: DAMPEN",
        _ => return None,
    };
    Some(text.to_string())
}
