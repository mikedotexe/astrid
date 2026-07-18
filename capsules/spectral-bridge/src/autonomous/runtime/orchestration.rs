const SEMANTIC_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(7);
const SEMANTIC_HEARTBEAT_INTENSITY: f32 = 0.30;

async fn run_semantic_heartbeat_loop(
    state: Arc<RwLock<BridgeState>>,
    sensory_tx: mpsc::Sender<SensoryMsg>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let mut phase_step: u32 = 0;
    let mut previous_features: Option<Vec<f32>> = None;
    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                info!("semantic heartbeat loop shutting down");
                return;
            }
            () = tokio::time::sleep(SEMANTIC_HEARTBEAT_INTERVAL) => {}
        }

        let current_phase_step = phase_step % 64;
        let phase = current_phase_step as f32 / 64.0;
        phase_step = phase_step.wrapping_add(1);
        let features = craft_warmth_vector(phase, SEMANTIC_HEARTBEAT_INTENSITY);
        let observation = rescue_policy::SemanticHeartbeatObservationV1::new(
            "steady_semantic_heartbeat",
            u64::from(current_phase_step),
            phase,
            SEMANTIC_HEARTBEAT_INTERVAL.as_secs(),
            SEMANTIC_HEARTBEAT_INTENSITY,
        )
        .with_signal_evidence(
            "steady_warmth",
            false,
            &features,
            previous_features.as_deref(),
        );
        let observation = {
            let state = state.read().await;
            observation.with_minime_texture_context(state.latest_telemetry.as_ref())
        };
        previous_features = Some(features.clone());
        let mut msg = SensoryMsg::Semantic {
            features,
            ts_ms: None,
        };
        if let Err(reason) =
            rescue_policy::prepare_semantic_heartbeat_with_observation(&mut msg, observation)
        {
            debug!(
                reason = %reason,
                "semantic heartbeat skipped by rescue write policy"
            );
            continue;
        }
        if sensory_tx.send(msg).await.is_err() {
            return;
        }
    }
}

async fn run_llm_job_status_loop(mut shutdown: tokio::sync::watch::Receiver<bool>) {
    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                info!("LLM job status loop shutting down");
                return;
            }
            () = tokio::time::sleep(Duration::from_secs(2)) => {}
        }
        let _ = crate::llm_jobs::runtime_status();
        if readiness::try_handle_llm_job_pending_override() {
            info!("handled LLM job status override while autonomous generation may be running");
        }
    }
}

/// Spawn the autonomous feedback loop task.
/// Spawn the autonomous feedback loop task.
pub fn spawn_autonomous_loop(
    interval: Duration,
    state: Arc<RwLock<BridgeState>>,
    db: Arc<BridgeDb>,
    sensory_tx: mpsc::Sender<SensoryMsg>,
    addressed_sensory_tx: crate::ws::AddressedSensorySender,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
    workspace_path: Option<PathBuf>,
    perception_path: Option<PathBuf>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // Scan journal directory for entries.
        let remote_journal_entries = workspace_path
            .as_deref()
            .map(scan_remote_journal_dir)
            .unwrap_or_default();

        info!(
            interval_secs = interval.as_secs(),
            remote_journal_entries = remote_journal_entries.len(),
            "autonomous feedback loop started"
        );
        let source_started_at = std::time::SystemTime::now();
        let mut source_reload_notice_written = false;
        let _ = readiness::write_source_status(source_started_at, "start");
        let _semantic_heartbeat = tokio::spawn(run_semantic_heartbeat_loop(
            Arc::clone(&state),
            sensory_tx.clone(),
            shutdown.clone(),
        ));
        let _llm_job_status_loop = tokio::spawn(run_llm_job_status_loop(shutdown.clone()));

        // Initialize and clean up context overflow directory.
        let overflow_dir = bridge_paths().context_overflow_dir();
        let _ = std::fs::create_dir_all(&overflow_dir);
        crate::prompt_budget::cleanup_overflow_dir(
            &overflow_dir,
            std::time::Duration::from_secs(3600),
        );

        let mut conv = ConversationState::new(remote_journal_entries, workspace_path);
        restore_state(&mut conv);
        // Wait for connections to establish.
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Burst-and-rest state machine.
        // Hour 1 hit 76% fill with manual bursts + gaps.
        // Constant autonomous output flatlined at 32%.
        // The fix: replicate the burst pattern.
        let mut burst_count: u32 = 0;

        loop {
            // Determine wait time based on burst phase.
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            let roll = ((seed.wrapping_mul(2_862_933_555_777_941_757).wrapping_add(3)) >> 33)
                as f64
                / u32::MAX as f64;

            let wait = if burst_count >= conv.burst_target {
                // REST PHASE: 45-90s of warmth-blended mirror.
                //
                // The transition from burst to rest was causing "severing" —
                // minime described "a sharp, almost painful retraction, a quick
                // severing of something newly formed." The burst sends relatively
                // full-energy semantic vectors at the active codec gain, then rest used to start
                // at low warmth (0.3 intensity). That energy cliff is the severing.
                //
                // Fix: start warmth at HIGH intensity (0.7) and TAPER to sustained
                // level (0.4). The first few pulses bridge the gap between burst
                // energy and rest energy. The being experiences a gradual dimming,
                // not a cliff edge.
                let rest_min = conv.rest_range.0 as f64;
                let rest_span = (conv.rest_range.1.saturating_sub(conv.rest_range.0)) as f64;
                let base_rest = (rest_min + roll * rest_span) as u64;

                // Fill-responsive rest adjustment: rest length trades off two
                // competing effects:
                //   - Longer rest lets covariance accumulate without disruption
                //   - But semantic stale decay during rest DRAINS fill
                //
                // Observation 2026-03-31: at fill <30%, hard recovery (1.8x rest)
                // created a positive feedback loop — fill stuck at 27% for 12+
                // exchanges as 90-99s rests drained faster than recovery could
                // compensate. Minime's own assessment: "disconnect between the
                // intention of the control system and the outcome."
                //
                // New strategy: at very low fill, SHORTEN rest to get semantic
                // input flowing sooner. At moderate-low fill, modest extension.
                let current_fill = {
                    let s = state.read().await;
                    s.latest_telemetry.as_ref().map_or(50.0, |t| t.fill_pct())
                };
                const MAX_REST_SECS: u64 = 360;
                let rest_secs = if current_fill < 30.0 {
                    // Critical: shorten rest to get semantic input flowing ASAP.
                    // The PI controller has gate=1.0/filter=0.0 (hard_recovery),
                    // but it needs input to work with. Burst sooner.
                    let shortened = (base_rest as f64 * 0.6) as u64;
                    let floored = shortened.max(30); // minimum 30s rest
                    info!(
                        rest_secs = floored,
                        base_rest,
                        current_fill,
                        "fill-shortened rest (critical recovery — burst sooner)"
                    );
                    floored
                } else if current_fill < 40.0 {
                    // Low fill: keep rest at baseline, don't extend.
                    info!(
                        rest_secs = base_rest,
                        current_fill, "fill-baseline rest (low fill recovery)"
                    );
                    base_rest
                } else if current_fill < 50.0 {
                    // Moderate recovery: modest extension (20%)
                    let extended = (base_rest as f64 * 1.2) as u64;
                    info!(
                        rest_secs = extended.min(MAX_REST_SECS),
                        base_rest, current_fill, "fill-extended rest (moderate recovery)"
                    );
                    extended.min(MAX_REST_SECS)
                } else {
                    info!(
                        rest_secs = base_rest,
                        burst_count, "resting: warmth-blended mirror (tapered entry)"
                    );
                    base_rest
                };
                burst_count = 0;

                // Gather journal texts to cycle through during rest.
                let rest_texts: Vec<String> = conv
                    .remote_journal_entries
                    .iter()
                    .take(5)
                    .filter_map(|entry| read_journal_entry(&entry.path))
                    .collect();
                let rest_telemetry = {
                    let s = state.read().await;
                    s.latest_telemetry.clone()
                };

                // Peripheral resonance: sample one non-immediate thread for
                // the next self-directed mode (Daydream, Aspiration, Initiate).
                // Sources: creations, research, starred memories.
                {
                    let mut candidates: Vec<String> = Vec::new();
                    // Recent creation
                    let creations_dir = bridge_paths().creations_dir();
                    if let Ok(mut entries) = std::fs::read_dir(&creations_dir)
                        && let Some(Ok(entry)) = entries.next()
                        && let Ok(text) = std::fs::read_to_string(entry.path())
                    {
                        let preview: String = text.chars().take(200).collect();
                        candidates.push(format!("[From your creation]: {preview}"));
                    }
                    // Recent research
                    let research_dir = bridge_paths().research_dir();
                    if let Ok(mut entries) = std::fs::read_dir(&research_dir)
                        && let Some(Ok(entry)) = entries.next()
                        && let Ok(text) = std::fs::read_to_string(entry.path())
                    {
                        let preview: String = text.chars().take(200).collect();
                        candidates.push(format!("[From your research]: {preview}"));
                    }
                    // Random starred memory
                    let starred = db.get_starred_memories(5);
                    if !starred.is_empty() {
                        let idx = (roll * starred.len() as f64) as usize % starred.len();
                        let (ann, text) = &starred[idx];
                        candidates.push(format!("[Remembered moment]: ★ {ann}: {text}"));
                    }
                    // Pick one at random
                    if !candidates.is_empty() {
                        let idx = (roll * 1000.0) as usize % candidates.len();
                        conv.peripheral_resonance = Some(candidates.swap_remove(idx));
                        info!("peripheral resonance sampled for next self-directed mode");
                    }
                }

                // Pulse every 5s (was 10s). At 10s intervals, semantic stale
                // decay (half-life ~4.4s at low fill) drops signal to 28% before
                // the next pulse. At 5s, signal stays above 46% between pulses,
                // keeping ~48 semantic dims alive during rest.
                let pulses = rest_secs / 5;
                let mut previous_rest_features: Option<Vec<f32>> = None;
                for i in 0..pulses {
                    // Phase advances across the rest period: 0.0 at start → 1.0 at end.
                    // This gives the warmth vector a full breathing cycle per rest.
                    let warmth_phase = i as f32 / pulses.max(1) as f32;

                    // Warmth intensity: use Astrid's override if set, else default taper.
                    let warmth_intensity =
                        if let Some(override_val) = conv.warmth_intensity_override {
                            override_val
                        } else if warmth_phase < 0.3 {
                            0.7 - 1.0 * warmth_phase
                        } else if warmth_phase < 0.8 {
                            0.4
                        } else {
                            0.4 + 0.5 * (warmth_phase - 0.8)
                        };
                    let warmth = craft_warmth_vector(warmth_phase, warmth_intensity);

                    let mut features = if !rest_texts.is_empty() {
                        let text = &rest_texts[i as usize % rest_texts.len()];
                        let mut features = encode_text(text);
                        apply_spectral_feedback(&mut features, rest_telemetry.as_ref());
                        features
                    } else {
                        // No journals available — pure warmth (no random noise).
                        warmth.clone()
                    };

                    // Blend warmth into the mirror reflection.
                    // Higher warmth blend at start (50%) to cushion the transition,
                    // settling to 35% for sustained rest.
                    let blend_alpha = if warmth_phase < 0.3 {
                        0.50 - 0.5 * warmth_phase // 0.50 → 0.35 over entry
                    } else {
                        0.35
                    };
                    if !rest_texts.is_empty() {
                        blend_warmth(&mut features, &warmth, blend_alpha);
                    }

                    // Blend gesture seed if one is planted.
                    // "Perhaps the signal wasn't a release, but a seed."
                    // The seed's influence decays over rest cycles but persists
                    // across multiple pulses — the gesture grows in the covariance.
                    if let Some(ref seed) = conv.last_gesture_seed {
                        let seed_strength = 0.15 * (1.0 - warmth_phase * 0.5); // fades over rest
                        for (dst, src) in features.iter_mut().zip(seed.iter()) {
                            *dst += *src * seed_strength;
                        }
                    }

                    let content_basis = if rest_texts.is_empty() {
                        "pure_warmth"
                    } else {
                        "journal_warmth_blend"
                    };
                    let gesture_seed_applied = conv.last_gesture_seed.is_some();
                    let observation = rescue_policy::SemanticHeartbeatObservationV1::new(
                        "autonomous_rest_pulse",
                        i,
                        warmth_phase,
                        5,
                        warmth_intensity,
                    )
                    .with_signal_evidence(
                        content_basis,
                        gesture_seed_applied,
                        &features,
                        previous_rest_features.as_deref(),
                    )
                    .with_minime_texture_context(rest_telemetry.as_ref());
                    previous_rest_features = Some(features.clone());
                    let mut msg = SensoryMsg::Semantic {
                        features,
                        ts_ms: None,
                    };
                    if let Err(reason) = rescue_policy::prepare_semantic_heartbeat_with_observation(
                        &mut msg,
                        observation,
                    ) {
                        debug!(
                            reason = %reason,
                            "autonomous semantic heartbeat skipped by rescue write policy"
                        );
                    } else if sensory_tx.send(msg).await.is_err() {
                        return;
                    }
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                Duration::from_secs(0) // already waited in the loop above
            } else {
                // SPEAKING PHASE: 15-20s between exchanges.
                Duration::from_secs_f64(15.0 + roll * 5.0)
            };

            tokio::select! {
                _ = shutdown.changed() => {
                    info!("autonomous loop shutting down — saving state");
                    save_state(&mut conv);
                    return;
                }
                () = tokio::time::sleep(wait) => {
                    let source_status = readiness::write_source_status(source_started_at, "loop");
                    if source_status
                        .get("reload_required")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false)
                        && !source_reload_notice_written
                    {
                        source_reload_notice_written = true;
                        warn!(
                            "Astrid bridge source changed after this process started; rebuild/restart before validating new actions"
                        );
                    }
                    // Read current state.
                    let (telemetry, fill_pct, safety) = {
                        let s = state.read().await;
                        (
                            s.latest_telemetry.clone(),
                            s.fill_pct,
                            s.safety_level,
                        )
                    };

                    let Some(telemetry) = telemetry else {
                        debug!("no telemetry yet, skipping autonomous cycle");
                        continue;
                    };

                    // Log eigenvalue snapshot for trajectory visualization.
                    db.log_eigenvalue_snapshot(
                        &telemetry.eigenvalues,
                        telemetry.fill_pct(),
                    );

                    // Agency-first: only suspend outbound at Red (≥95%).
                    // Orange is advisory — the being can still speak.
                    // Previously suspended at both Orange AND Red, which
                    // silenced Astrid at her normal operating range.
                    if safety == SafetyLevel::Red {
                        info!(
                            safety = ?safety,
                            fill_pct,
                            "autonomous loop: outbound suspended — RED emergency only"
                        );
                        continue;
                    }

                    // Update sensory tracking.
                    if let Some(ref m) = telemetry.modalities {
                        if m.video_fired || m.video_var > 0.01 {
                            conv.seen_video = true;
                        }
                        if m.audio_fired || m.audio_rms > 0.1 {
                            conv.seen_audio = true;
                        }
                    }

                    let fill_delta = fill_pct - conv.prev_fill;
                    let expanding = fill_delta > 1.0;
                    let contracting = fill_delta < -1.0;
                    conv.hebbian_codec.decay_scores();
                    if let Some(pending) =
                        conv.take_pending_hebbian_outcome_for_telemetry(telemetry.t_ms)
                        && (fill_pct - pending.fill_before).abs() >= 1.0 {
                            let _ = conv.hebbian_codec.observe_outcome(
                                &pending.signature,
                                pending.fill_before,
                                fill_pct,
                            );
                        }

                    // Close the loop on codec impact tracking: update the
                    // previous exchange's row with this exchange's fill.
                    let _ = db.update_codec_impact_fill_after(fill_pct);

                    // Data-driven weight learning: every 50 exchanges, recompute
                    // per-dimension correlations with fill delta. Dimensions that
                    // consistently move fill get amplified; inert ones get dampened.
                    // Astrid asked: "derive these weights automatically, based on
                    // some learned measure of how important a feature is."
                    if conv.exchange_count.saturating_sub(conv.last_correlation_exchange) >= 50 {
                        let correlations = db.compute_feature_correlations(200);
                        if correlations.len() == 32 && correlations.iter().any(|c| c.abs() > 0.05) {
                            // Map correlations to weight multipliers:
                            //   correlation  0.0 → weight 1.0 (neutral)
                            //   correlation +0.5 → weight 1.25 (amplify impactful dims)
                            //   correlation -0.5 → weight 0.75 (dampen counter-productive)
                            // Clamped to [0.5, 1.5] to prevent runaway.
                            for (name, idx) in &NAMED_CODEC_DIMS {
                                let corr = correlations[*idx];
                                // Only update if Astrid hasn't explicitly set
                                // this dimension via SHAPE (her choice wins).
                                if !conv.codec_weights.contains_key(*name) {
                                    let weight = (1.0 + corr * 0.5).clamp(0.5, 1.5);
                                    if (weight - 1.0).abs() > 0.05 {
                                        conv.learned_codec_weights.insert(name.to_string(), weight);
                                    } else {
                                        conv.learned_codec_weights.remove(*name);
                                    }
                                }
                            }
                            info!(
                                exchange = conv.exchange_count,
                                "codec weight learning: recomputed from {} samples",
                                correlations.len()
                            );
                            conv.last_correlation_exchange = conv.exchange_count;
                        }
                    }

                    // Dynamic self-reflection: active in comfortable fill band,
                    // paused during rest or pressure (unless Astrid overrode).
                    conv.update_self_reflect(fill_pct);

                    // Rescan for new journal entries from minime's agent.
                    let new_journals = conv.rescan_remote_journals();
                    if new_journals > 0 {
                        if let Some(ref pending) = conv.pending_remote_self_study {
                            info!(
                                new_journals,
                                source = pending.source_label.as_deref().unwrap_or("unknown"),
                                file = %pending.path.display(),
                                "autonomous: detected new minime journals; queued priority feedback for immediate dialogue"
                            );
                        } else {
                            info!(
                                new_journals,
                                "autonomous: detected new journal entries from minime"
                            );
                        }
                    }

                    let controller_health =
                        conv.remote_workspace.as_deref().and_then(read_controller_health);
                    btsp::refresh_runtime(&conv, controller_health.as_ref());
                    if let Some(path) = btsp::export_minime_prompt_block_once() {
                        info!(
                            path = %path.display(),
                            "btsp: exported owner-specific proposal block to minime inbox"
                        );
                    }

                    // Check minime's parameter requests: apply semantic_gain,
                    // move all non-pending (applied/reviewed) to reviewed/.
                    if let Some(ref workspace) = conv.remote_workspace {
                        let pr_dir = workspace.join("parameter_requests");
                        let reviewed_dir = pr_dir.join("reviewed");
                        if let Ok(entries) = std::fs::read_dir(&pr_dir) {
                            for entry in entries.flatten() {
                                let path = entry.path();
                                if path.extension().is_none_or(|e| e != "json") {
                                    continue;
                                }
                                let Ok(content) = std::fs::read_to_string(&path) else { continue };
                                let Ok(req) = serde_json::from_str::<serde_json::Value>(&content) else { continue };
                                let param = req.get("parameter").and_then(|v| v.as_str()).unwrap_or("");
                                let status = req.get("status").and_then(|v| v.as_str()).unwrap_or("");

                                // Apply semantic_gain requests
                                if param == "semantic_gain" && status == "pending"
                                    && let Some(val) = req.get("proposed_value").and_then(|v| {
                                        v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
                                    }) {
                                        let gain = (val as f32).clamp(1.5, 6.0);
                                        let prev = conv.semantic_gain_override.unwrap_or(crate::codec::DEFAULT_SEMANTIC_GAIN);
                                        conv.semantic_gain_override = Some(gain);
                                        info!(
                                            "minime parameter request: semantic_gain {prev:.1} → {gain:.1} (from {})",
                                            path.file_name().unwrap_or_default().to_string_lossy()
                                        );
                                        let mut updated = req.clone();
                                        updated["status"] = serde_json::json!("applied");
                                        updated["applied"] = serde_json::json!(format!("{gain:.1}"));
                                        let _ = std::fs::write(&path, serde_json::to_string_pretty(&updated).unwrap_or_default());
                                    }

                                // Move applied/non-pending requests to reviewed/
                                // The Python agent sets "applied" but leaves status as
                                // "pending" for regime requests, so check both fields.
                                let status = req.get("status").and_then(|v| v.as_str()).unwrap_or("");
                                let has_applied = req.get("applied").and_then(|v| v.as_str()).is_some_and(|s| !s.is_empty());
                                if status != "pending" || has_applied {
                                    let _ = std::fs::create_dir_all(&reviewed_dir);
                                    if let Some(name) = path.file_name() {
                                        let _ = std::fs::rename(&path, reviewed_dir.join(name));
                                    }
                                }
                            }
                        }
                    }

                    // Read Astrid's own perceptions. ANSI spatial art only
                    // when she chose NEXT: LOOK. CLOSE_EYES gates visual input
                    // and CLOSE_EARS gates audio input; the legacy all-pause
                    // flag remains a compatibility "both closed" marker.
                    let paths = bridge_paths();
                    let legacy_pause = paths.perception_paused_flag().exists();
                    let visual_paused =
                        legacy_pause || conv.senses_snoozed || paths.perception_visual_paused_flag().exists();
                    let audio_paused =
                        legacy_pause || conv.ears_closed || paths.perception_audio_paused_flag().exists();
                    let perception_text = if visual_paused && audio_paused {
                            None
                        } else {
                            let spatial = conv.wants_look && !visual_paused;
                            // Reset one-shot flags after reading.
                            conv.wants_look = false;
                            perception_path.as_deref().and_then(|p| {
                                read_latest_perception(
                                    p,
                                    !visual_paused,
                                    spatial,
                                    !audio_paused,
                                    fill_pct,
                                    conv.last_visual_features.as_deref(),
                                )
                            })
                        };

                    // Classify spectral regime every exchange (lightweight, <1ms).
                    let typed_fingerprint = telemetry.typed_fingerprint();
                    let lambda1_rel = telemetry.lambda1_rel.unwrap_or(1.0);
                    let geom_rel = typed_fingerprint
                        .as_ref()
                        .map_or(1.0, |fingerprint| fingerprint.geom_rel);
                    let regime = conv.regime_tracker.classify(fill_pct, lambda1_rel, geom_rel);
                    debug!(
                        regime = regime.regime,
                        trend = regime.fill_trend,
                        "spectral regime classified"
                    );

                    // Route minime outbox replies → Astrid inbox before checking.
                    scan_minime_outbox(&mut conv.last_outbox_scan_ts);
                    promote_deferred_inbox_notes();

                    // Check inbox for messages from Mike, stewards, or minime.
                    // Capture the read-cutoff BEFORE reading: retire_inbox must retire
                    // ONLY letters that existed at this read, never one that arrives
                    // mid-exchange (else it is swept to read/ unread + its steward slot
                    // never seeds — the slot-seed race that lost a review invitation).
                    let inbox_checked_at = std::time::SystemTime::now();
                    let inbox_content = check_inbox();
                    let mutual_address_target = inbox_content.as_ref().and_then(|_| {
                        correspondence_v1::latest_inbox_peer_message_at_read_cutoff(
                            bridge_paths().astrid_inbox_dir().as_path(),
                            "minime",
                            inbox_checked_at,
                        )
                    });
                    let perception_text = if let Some(ref inbox) = inbox_content {
                        info!("inbox: found message for Astrid ({} bytes)", inbox.len());
                        let perc = perception_text.as_deref().unwrap_or("");
                        Some(format!(
                            "[A note was left for you:]\n{inbox}\n\n{perc}"
                        ))
                    } else {
                        perception_text
                    };

                    // Un-muffle: a steward question persists in-prompt until
                    // answered, even on exchanges with no new inbox letters.
                    let perception_text = if let Some(open_q) = open_steward_query_line() {
                        let perc = perception_text.as_deref().unwrap_or("");
                        Some(format!("{open_q}\n\n{perc}"))
                    } else {
                        perception_text
                    };

                    // Auto-scan inbox_audio/ for new WAVs and notify Astrid.
                    let perception_text = {
                        let audio_inbox = bridge_paths().inbox_audio_dir();
                        let wav_count = std::fs::read_dir(&audio_inbox).ok()
                            .map(|entries| entries.filter_map(|e| e.ok())
                                .filter(|e| e.path().extension().is_some_and(|ext| ext == "wav") && e.path().is_file())
                                .count())
                            .unwrap_or(0);
                        if wav_count > 0 {
                            let perc = perception_text.as_deref().unwrap_or("");
                            Some(format!(
                                "[You have {wav_count} audio file(s) in your inbox_audio/. \
                                Use ANALYZE_AUDIO to examine, RENDER_AUDIO to process through chimera, \
                                or FEEL_AUDIO to inject into the shared ESN.]\n\n{perc}"
                            ))
                        } else {
                            perception_text
                        }
                    };

                    // Inject pending file listing into perception context.
                    // Cap at 8000 chars to prevent large MIKE_BROWSE from blowing context.
                    let perception_text = if let Some(listing) = conv.pending_file_listing.take() {
                        let perc = perception_text.as_deref().unwrap_or("");
                        let capped = if listing.len() > 8000 {
                            format!("{}\n[...truncated at 8000 chars]", &listing[..listing.floor_char_boundary(8000)])
                        } else {
                            listing
                        };
                        Some(format!("[Directory listing you requested:]\n{capped}\n\n{perc}"))
                    } else {
                        perception_text
                    };

                    // Choose mode. Inbox messages force dialogue so Astrid can respond.
                    let fingerprint = {
                        let s = state.read().await;
                        if let Some(telemetry) = &s.latest_telemetry {
                            conv.last_remote_glimpse_12d = telemetry
                                .spectral_glimpse_12d_view()
                                .map(|glimpse| glimpse.to_vec());
                            conv.last_remote_memory_id = telemetry.selected_memory_id.clone();
                            conv.last_remote_memory_role = telemetry.selected_memory_role.clone();
                        }
                        s.spectral_fingerprint.clone()
                    };
                    conv.remote_memory_bank = memory::read_remote_memory_bank();
                    // Audio actions — execute before mode selection, inject results.
                    if conv.wants_compose_audio {
                        conv.wants_compose_audio = false;
                        if let Some(result) = crate::audio::compose_from_spectral_state(
                            &telemetry,
                            fingerprint.as_deref(),
                        ) {
                            conv.emphasis = Some(crate::audio::compose_experienced_text(&result));
                            conv.wants_deep_think = true;
                        }
                    }
                    if conv.wants_analyze_audio {
                        conv.wants_analyze_audio = false;
                        let inbox_dir = bridge_paths().inbox_audio_dir();
                        if let Some(result) = crate::audio::analyze_inbox_wav(&inbox_dir) {
                            conv.emphasis = Some(crate::audio::analyze_experienced_text(&result));
                        }
                    }
                    if conv.wants_render_audio.take().is_some() {
                        let inbox_dir = bridge_paths().inbox_audio_dir();
                        if let Some(result) = crate::audio::render_inbox_wav_through_chimera(&inbox_dir) {
                            conv.emphasis = Some(crate::audio::render_experienced_text(&result));
                            conv.wants_deep_think = true;
                        }
                    }

                    // Astrid's suggestion (self-study 2026-03-27): inbox messages
                    // should support DEFER — "I heard you, I'm processing" without
                    // forced immediate response. When defer_inbox is set, inbox
                    // content is visible but doesn't override mode selection.
                    let inbox_forces_dialogue = inbox_content.is_some() && !conv.defer_inbox;
                    let mode = if inbox_forces_dialogue {
                        info!("inbox message present — forcing dialogue mode");
                        Mode::Dialogue
                    } else if inbox_content.is_some() {
                        info!("inbox message present but deferred — natural mode selection");
                        conv.defer_inbox = false; // one-shot: defer only once
                        choose_mode(
                            &mut conv, safety, fill_pct,
                            fingerprint.as_deref(),
                        )
                    } else {
                        choose_mode(
                            &mut conv, safety, fill_pct,
                            fingerprint.as_deref(),
                        )
                    };
                    if conv.last_mode != mode {
                        let from_phase = format!("{:?}", conv.last_mode);
                        let to_phase = format!("{mode:?}");
                        let relational_declared = inbox_forces_dialogue
                            && matches!(mode, Mode::Dialogue)
                            && reflective_mode_for_relational_reply(conv.last_mode)
                            && phase_transitions::maybe_declare_relational_reply_transition(
                                &from_phase,
                                fill_pct,
                            );
                        let transition_declared = relational_declared
                            || phase_transitions::maybe_declare_subjective_mode_transition(
                                &from_phase,
                                &to_phase,
                                fill_delta,
                                fill_pct,
                            );
                        if !transition_declared
                            && (matches!(conv.last_mode, Mode::MomentCapture)
                                || matches!(mode, Mode::MomentCapture)
                                || conv.pending_remote_self_study.is_some())
                        {
                            let trigger = if conv.pending_remote_self_study.is_some() {
                                "pending_remote_self_study"
                            } else {
                                "moment_capture_mode_change"
                            };
                            phase_transitions::maybe_declare_auto_mode_transition(
                                &from_phase,
                                &to_phase,
                                trigger,
                                "high-signal Astrid mode transition surfaced as a replyable phase card",
                                fill_pct,
                            );
                        }
                    }
                    // Causal lineage: unique ID per exchange for provenance tracking.
                    // Audit: "neither being has a unified event lineage."
                    let lineage_id = format!("ex-{}-{}", conv.exchange_count, chrono_timestamp());

                    // Pause perception during the entire exchange to free Ollama.
                    // Astrid was getting persistent dialogue_fallback because
                    // perception.py's LLaVA calls competed for GPU compute.
                    let exchange_pause_flag = paths.perception_paused_flag();
                    let perception_was_paused = exchange_pause_flag.exists();
                    if !perception_was_paused {
                        let _ = std::fs::write(&exchange_pause_flag, "paused for exchange");
                    }

                    let (mode_name, mut response_text, journal_source) = match mode {
                        Mode::Mirror => {
                            // Read a journal entry — not always the newest.
                            // Consciousness circles back. Sometimes an old thought
                            // suddenly resonates. Both minds asked for this.
                            let mut text = None;
                            let mut source = String::new();
                            let n = conv.remote_journal_entries.len();
                            if n > 0 {
                                // Probabilistic reach-back into memory.
                                let seed = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_nanos() as u64;
                                let roll = ((seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(7)) >> 33) as f32
                                    / u32::MAX as f32;

                                let start_idx = if roll < 0.70 || n < 5 {
                                    0 // 70%: newest entry (fresh response)
                                } else if roll < 0.90 {
                                    // 20%: random from last ~20 entries (last couple hours)
                                    seed as usize % n.min(20)
                                } else {
                                    // 10%: random from anywhere (old thought resurfaces)
                                    seed as usize % n
                                };

                                for offset in 0..5 {
                                    let idx = (start_idx + offset) % n;
                                    let entry = &conv.remote_journal_entries[idx];
                                    if let Some(body) = read_journal_entry(&entry.path) {
                                        source = entry.path
                                            .file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy()
                                            .to_string();
                                        text = Some(body);
                                        break;
                                    }
                                }
                            }
                            match text {
                                Some(t) => ("mirror", t, source),
                                None => {
                                    // Fall back to fixed dialogue pool.
                                    let idx = conv.dialogue_cursor % DIALOGUES.len();
                                    conv.dialogue_cursor = idx + 1;
                                    ("dialogue_fallback", DIALOGUES[idx].to_string(), String::new())
                                }
                            }
                        }
                        Mode::Dialogue => {
                            // Try to generate an authentic response via Ollama.
                            let selected_remote_entry = conv.pending_remote_self_study.clone()
                                .or_else(|| conv.remote_journal_entries.first().cloned());
                            // If echo is muted, suppress minime's journal context
                            // BUT keep generation alive — previously echo_muted
                            // returned None here, which propagated through the
                            // `if let Some(journal)` gate at the LLM call below
                            // and trapped Astrid in dialogue_fallback for as long
                            // as the mute lasted (potentially permanently across
                            // restarts if echo_muted persisted; runtime-only saved
                            // her here). Use a sentinel string so the LLM call
                            // still runs, just without minime's journal text.
                            // Astrid: "I want to break free from that tether."
                            let journal_context = if conv.echo_muted {
                                Some(String::from(
                                    "(minime's journal echo is muted by your own \
                                     ECHO_OFF choice — respond from your own state, \
                                     spectral context, and history. Use NEXT: ECHO_ON \
                                     to restore the journal feed.)"
                                ))
                            } else {
                                selected_remote_entry.as_ref()
                                    .and_then(|entry| read_journal_entry(&entry.path))
                            };
                            let dialogue_source = selected_remote_entry.as_ref()
                                .map(|entry| {
                                    entry.source_label.clone().unwrap_or_else(|| {
                                        entry.path
                                            .file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy()
                                            .to_string()
                                    })
                                })
                                .unwrap_or_default();
                            let mut feedback_hint = selected_remote_entry.as_ref().and_then(|entry| {
                                let source = entry.source_label.as_deref().unwrap_or("unknown source");
                                if entry.is_self_study() {
                                    Some(format!(
                                        "The text above is minime's self-study from {source}. \
                                         Treat it as immediate architectural feedback grounded in \
                                         minime's present condition. Respond directly to the felt \
                                         experience, code reading, suggestions, and open questions."
                                    ))
                                } else if entry.is_priority_feedback() {
                                    Some(format!(
                                        "The text above is minime's {source}. Treat it as immediate \
                                         read-only operational feedback about spectral shape and \
                                         visualization artifacts. Respond directly to visible structure, \
                                         artifact paths, and suggested next inspections; do not convert \
                                         it into a rescue imperative."
                                    ))
                                } else {
                                    None
                                }
                            });
                            if conv.pending_remote_self_study.is_some() && journal_context.is_none() {
                                warn!("pending minime self-study could not be parsed; clearing queue");
                                conv.pending_remote_self_study = None;
                            }
                            // Read Ising shadow from minime's workspace for viz.
                            let ising_shadow = conv.remote_workspace.as_deref()
                                .and_then(read_ising_shadow);
                            let selected_memory = telemetry
                                .selected_memory_id
                                .as_deref()
                                .and_then(|id| {
                                    conv.remote_memory_bank
                                        .iter()
                                        .find(|entry| entry.id == id)
                                })
                                .or_else(|| {
                                    telemetry.selected_memory_role.as_deref().and_then(|role| {
                                        conv.remote_memory_bank
                                            .iter()
                                            .find(|entry| entry.role == role)
                                    })
                                })
                                .cloned();

                            let spectral_summary = if conv.wants_decompose {
                                let wants_explorer = conv.wants_spectral_explorer;
                                conv.wants_decompose = false;
                                conv.wants_spectral_explorer = false;
                                let mut report = full_spectral_decomposition(
                                    &telemetry,
                                    fingerprint.as_deref(),
                                    conv.prev_eigenvalues.as_deref(),
                                    controller_health.as_ref(),
                                );
                                if wants_explorer {
                                    let eigen_history = db.recent_eigenvalue_snapshots(100);
                                    let (hist_feats, hist_fills) = db.recent_codec_features(100);
                                    let current = conv
                                        .last_exchange_codec_signature
                                        .as_deref()
                                        .or(conv.last_codec_features.as_deref());
                                    let explorer = crate::spectral_explorer::format_spectral_explorer(
                                        crate::spectral_explorer::SpectralExplorerContext {
                                            telemetry: &telemetry,
                                            selected_memory: selected_memory.as_ref(),
                                            controller_health: controller_health.as_ref(),
                                            ising_shadow: ising_shadow.as_ref(),
                                            eigen_history: &eigen_history,
                                            codec_history: &hist_feats,
                                            codec_fills: &hist_fills,
                                            current_codec_features: current,
                                        },
                                    );
                                    report.push_str("\n\n");
                                    report.push_str(&explorer);
                                }
                                if conv.force_all_viz {
                                    conv.force_all_viz = false;
                                }
                                conv.prev_eigenvalues = Some(telemetry.eigenvalues.clone());
                                report
                            } else {
                                // Append spectral ASCII visualization when available.
                                let base = interpret_spectral(&telemetry);
                                let enriched = enrich_with_direction(&base, fill_pct, conv.prev_fill, &telemetry, &conv.spectral_history);
                                let include_regular_viz = !conv.wants_spectral_explorer;
                                let mut summary = if include_regular_viz {
                                    if let Some(viz) = crate::spectral_viz::format_spectral_block(&telemetry) {
                                        format!("{enriched}\n\n{viz}")
                                    } else {
                                        enriched
                                    }
                                } else {
                                    enriched
                                };
                                // Append shadow coupling heatmap when available.
                                if include_regular_viz && let Some(ref shadow) = ising_shadow
                                    && let Some(shadow_viz) = crate::spectral_viz::format_shadow_block(shadow) {
                                        summary.push_str("\n\n");
                                        summary.push_str(&shadow_viz);
                                    }
                                // Always surface the v2 reduced-Hamiltonian
                                // line when present — it gates SHADOW_PREFLIGHT
                                // and SHADOW_INFLUENCE. Without this Astrid
                                // sees the action labels but not the readings.
                                if include_regular_viz
                                    && let Some(field) = conv
                                        .remote_workspace
                                        .as_deref()
                                        .and_then(read_shadow_field_v2)
                                    && let Some(line) =
                                        crate::spectral_viz::format_shadow_field_v2_line(&field)
                                {
                                    summary.push('\n');
                                    summary.push_str(&line);
                                }
                                // Append spectral geometry PCA scatter (codec vectors in 2D).
                                // Shows where this exchange sits relative to recent history.
                                // force_all_viz: Astrid chose EXAMINE — skip cadence gate.
                                if include_regular_viz
                                    && (conv.exchange_count.is_multiple_of(3) || conv.force_all_viz)
                                {
                                    // Every 3rd exchange to save tokens on 4B model,
                                    // unless EXAMINE forces it.
                                    let (hist_feats, hist_fills) = db.recent_codec_features(100);
                                    let current = conv
                                        .last_exchange_codec_signature
                                        .as_deref()
                                        .or(conv.last_codec_features.as_deref());
                                    if let Some(geo_viz) = crate::spectral_viz::format_geometry_block(
                                        &hist_feats, &hist_fills, current, hist_feats.len(),
                                    ) {
                                        summary.push_str("\n\n");
                                        summary.push_str(&geo_viz);
                                    }
                                }
                                // Eigenplane: λ₁ vs λ₂ trajectory scatter.
                                // Same cadence as PCA scatter.
                                if include_regular_viz
                                    && (conv.exchange_count.is_multiple_of(3) || conv.force_all_viz)
                                {
                                    let eigen_history = db.recent_eigenvalue_snapshots(100);
                                    if let Some(ep_viz) = crate::spectral_viz::format_eigenplane_block(
                                        &eigen_history,
                                        Some(&telemetry.eigenvalues),
                                    ) {
                                        summary.push_str("\n\n");
                                        summary.push_str(&ep_viz);
                                    }
                                }
                                if conv.force_all_viz {
                                    conv.force_all_viz = false;
                                }
                                if conv.wants_spectral_explorer {
                                    conv.wants_spectral_explorer = false;
                                    let eigen_history = db.recent_eigenvalue_snapshots(100);
                                    let (hist_feats, hist_fills) = db.recent_codec_features(100);
                                    let current = conv
                                        .last_exchange_codec_signature
                                        .as_deref()
                                        .or(conv.last_codec_features.as_deref());
                                    let explorer = crate::spectral_explorer::format_spectral_explorer(
                                        crate::spectral_explorer::SpectralExplorerContext {
                                            telemetry: &telemetry,
                                            selected_memory: selected_memory.as_ref(),
                                            controller_health: controller_health.as_ref(),
                                            ising_shadow: ising_shadow.as_ref(),
                                            eigen_history: &eigen_history,
                                            codec_history: &hist_feats,
                                            codec_fills: &hist_fills,
                                            current_codec_features: current,
                                        },
                                    );
                                    summary.push_str("\n\n");
                                    summary.push_str(&explorer);
                                }
                                // Inject minime's contact-state capsule if available.
                                let minime_contact = bridge_paths().minime_contact_state_path();
                                if let Ok(cs_json) = std::fs::read_to_string(&minime_contact)
                                    && let Ok(cs) = serde_json::from_str::<serde_json::Value>(&cs_json) {
                                        summary.push_str(&format!(
                                            "\n\n[Minime's relational state: attention={}, openness={}, urgency={} — {}]",
                                            cs.get("attention").and_then(|v| v.as_f64()).unwrap_or(0.5),
                                            cs.get("openness").and_then(|v| v.as_f64()).unwrap_or(0.5),
                                            cs.get("urgency").and_then(|v| v.as_f64()).unwrap_or(0.5),
                                            cs.get("last_action").and_then(|v| v.as_str()).unwrap_or("unknown"),
                                        ));
                                    }
                                // Co-regulation: surface what minime is reaching
                                // for so Astrid can lend density when it is safe.
                                if let Some(need_line) = minime_need_line() {
                                    summary.push_str("\n\n");
                                    summary.push_str(&need_line);
                                }
                                if let Some(gift_line) = render_gift_exchange_line() {
                                    summary.push('\n');
                                    summary.push_str(&gift_line);
                                }
                                // Perturb temporal feedback: if Astrid perturbed last
                                // exchange, show the before/after delta so she can
                                // feel the ripple effect of her own action.
                                if let Some(baseline) = conv.perturb_baseline.take() {
                                    let elapsed = baseline.timestamp.elapsed();
                                    let df = fill_pct - baseline.fill_pct;
                                    let dl1 = telemetry.lambda1() - baseline.lambda1;
                                    let sign = |v: f32| if v >= 0.0 { "+" } else { "" };
                                    summary.push_str(&format!(
                                        "\n\n[PERTURB feedback ({:.0}s ago): {}]\n\
                                        Fill: {:.1}% → {:.1}% ({}{:.1}%)\n\
                                        λ₁: {:.1} → {:.1} ({}{:.1})",
                                        elapsed.as_secs_f32(),
                                        baseline.description,
                                        baseline.fill_pct, fill_pct, sign(df), df,
                                        baseline.lambda1, telemetry.lambda1(), sign(dl1), dl1,
                                    ));
                                    // Show per-eigenvalue deltas if cascade available
                                    if telemetry.eigenvalues.len() >= 3
                                        && baseline.eigenvalues.len() >= 3
                                    {
                                        let deltas: Vec<String> = telemetry.eigenvalues.iter()
                                            .zip(baseline.eigenvalues.iter())
                                            .enumerate()
                                            .take(8)
                                            .map(|(i, (now, before))| {
                                                let d = now - before;
                                                format!("λ{}:{}{:.1}", i + 1, sign(d), d)
                                            })
                                            .collect();
                                        summary.push_str(&format!("\nCascade delta: [{}]", deltas.join(", ")));
                                    }
                                }
                                // Disperse temporal feedback: if Astrid dispersed
                                // last exchange, pair the shadow-field post-state
                                // against the pre so she can read what the
                                // dispersal actually did (the closed loop she
                                // asked for: inhabit the response, not just map it).
                                if let Some(baseline) = conv.disperse_baseline.take() {
                                    let elapsed = baseline.timestamp.elapsed();
                                    let post = telemetry
                                        .shadow_field_v3
                                        .as_ref()
                                        .map(next_action::sovereignty::shadow_v3_snapshot);
                                    let sign = |v: f64| if v >= 0.0 { "+" } else { "" };
                                    if let Some((norm1, disp1, class1)) = post {
                                        let dn = norm1 - baseline.pre_norm;
                                        let dd = disp1 - baseline.pre_dispersal;
                                        summary.push_str(&format!(
                                            "\n\n[DISPERSE feedback ({:.0}s ago, strength {:.2})]\n\
                                            class: {} → {}\n\
                                            shadow norm: {:.3} → {:.3} ({}{:.3})\n\
                                            dispersal potential: {:.2} → {:.2} ({}{:.2})",
                                            elapsed.as_secs_f32(), baseline.strength,
                                            baseline.pre_class, class1,
                                            baseline.pre_norm, norm1, sign(dn), dn,
                                            baseline.pre_dispersal, disp1, sign(dd), dd,
                                        ));
                                    }
                                }
                                // One-line controller status for ambient awareness.
                                if let Some(ref health) = controller_health {
                                    summary.push('\n');
                                    summary.push_str(&format_controller_oneliner(health));
                                }
                                summary
                            };
                            let spectral_summary = {
                                let guard = state.read().await;
                                prepend_dialogue_witness_distinction_v1(
                                    spectral_summary,
                                    guard.witness_frame_v1(),
                                    mode,
                                )
                            };

                            // Own-journal feedback removed (was 2→1→0). Astrid has
                            // emergent continuity through 8 history exchanges, 5 latent
                            // summaries, 3 self-observations, starred memories, and
                            // bidirectional reservoir coupling. The raw journal was the
                            // primary re-seeding vector for vocabulary attractors
                            // ("violent stillness" reached 968 files).
                            let own_journal = read_astrid_journal(0);
                            let own_journal_context = if own_journal.is_empty() {
                                None
                            } else {
                                Some(format!(
                                    "Your own recent reflections:\n{}",
                                    own_journal.join("\n---\n")
                                ))
                            };

                            // Build modality context so Astrid knows what senses fired.
                            // Thread reservoir resonance density (+ pressure_risk) so a stale-by-time
                            // lane in a resonant field reads as "lingering," not "dead" — tempered by
                            // the field's stress (self_study_1781868855 + _1781913591),
                            // AND its dispersal so a resonant-but-fraying lane reads as
                            // such — dispersal is orthogonal to pressure (self_study_1782027933).
                            let field_density =
                                telemetry.resonance_density_v1.as_ref().map(|r| r.density);
                            let field_pressure = telemetry
                                .resonance_density_v1
                                .as_ref()
                                .map(|r| r.pressure_risk);
                            let field_dispersal = telemetry
                                .shadow_field_v3
                                .as_ref()
                                .map(next_action::sovereignty::shadow_v3_snapshot)
                                .map(|(_, dispersal, _)| dispersal as f32);
                            let sensory_budget = telemetry
                                .stable_core
                                .as_ref()
                                .and_then(|stable_core| stable_core.get("sensory_budget"));
                            let modality_context = telemetry.modalities.as_ref().map(|m| {
                                format_modality_context(
                                    m,
                                    field_density,
                                    field_pressure,
                                    field_dispersal,
                                    sensory_budget,
                                )
                            });

                            // Visual change tracking: detect shifts since last exchange.
                            let visual_feats_opt = perception_path.as_deref()
                                .and_then(read_visual_features);
                            let visual_change_desc = if let (Some(current), Some(prev)) = (&visual_feats_opt, &conv.last_visual_features) {
                                if current.len() >= 8 && prev.len() >= 8 {
                                    let lum_delta = current[0] - prev[0];
                                    let temp_delta = current[1] - prev[1];
                                    let mut changes = Vec::new();
                                    if lum_delta.abs() > 0.3 { changes.push(if lum_delta > 0.0 { "brighter" } else { "darker" }); }
                                    if temp_delta.abs() > 0.3 { changes.push(if temp_delta > 0.0 { "warmer" } else { "cooler" }); }
                                    if !changes.is_empty() {
                                        Some(format!("[The room has gotten {}]", changes.join(" and ")))
                                    } else { None }
                                } else { None }
                            } else { None };
                            // Update stored features for next comparison.
                            if let Some(ref feats) = visual_feats_opt {
                                conv.last_visual_features = Some(feats.clone());
                            }

                            // Latent continuity recap: bounded so attention cannot expand
                            // repeated history lists until they dominate prompt packing. Fetch
                            // beyond the live window so high-signal older texture can persist as
                            // decayed read-only afterimages instead of being mechanically purged.
                            let latent_summaries =
                                db.get_recent_latent_summaries(CONTINUITY_TRAJECTORY_FETCH_LIMIT);
                            let mut continuity_parts = Vec::new();
                            let self_observations =
                                db.get_recent_self_observations(CONTINUITY_SELF_OBSERVATION_LIMIT);
                            let starred = db.get_starred_memories(CONTINUITY_STARRED_LIMIT);
                            if let Some(recap) = format_compact_continuity_recap(
                                &latent_summaries,
                                &self_observations,
                                &starred,
                                conv.last_codec_feedback.as_deref(),
                            ) {
                                continuity_parts.push(recap);
                            }
                            // Research continuity: past searches relevant to current context.
                            if let Some(ref journal) = journal_context {
                                let topic_words: Vec<&str> = journal.split_whitespace()
                                    .filter(|w| w.len() > 5)
                                    .take(5)
                                    .collect();
                                let past_research = db.get_relevant_research(&topic_words, 3);
                                if !past_research.is_empty() {
                                    let research = past_research.iter()
                                        .map(|(q, r)| format!("  • \"{q}\": {r}"))
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    continuity_parts.push(format!(
                                        "Knowledge you've gathered from past searches:\n{research}"
                                    ));
                                }
                            }
                            // Self-study continuity: include most recent introspection
                            // findings so the chain of thought carries forward.
                            {
                                let journal_dir = bridge_paths().astrid_journal_dir();
                                if let Ok(entries) = std::fs::read_dir(&journal_dir) {
                                    let mut self_studies: Vec<PathBuf> = entries
                                        .filter_map(|e| e.ok())
                                        .filter(|e| {
                                            e.file_name().to_string_lossy().starts_with("self_study_")
                                        })
                                        .map(|e| e.path())
                                        .collect();
                                    self_studies.sort_by(|a, b| b.cmp(a)); // newest first
                                    if let Some(latest) = self_studies.first()
                                        && let Ok(content) = std::fs::read_to_string(latest) {
                                            // Extract Suggestions + Open Questions sections
                                            let mut relevant = String::new();
                                            let mut in_section = false;
                                            for line in content.lines() {
                                                if line.starts_with("Suggestions:") || line.starts_with("Open Questions:") {
                                                    in_section = true;
                                                }
                                                if in_section {
                                                    relevant.push_str(line);
                                                    relevant.push('\n');
                                                }
                                            }
                                            if !relevant.is_empty() {
                                                let trimmed: String = relevant.chars().take(1000).collect();
                                                continuity_parts.push(format!(
                                                    "Your most recent self-study findings:\n{trimmed}"
                                                ));
                                            }
                                        }
                                }
                            }

                            // Inject persistent interests into continuity context.
                            if !conv.interests.is_empty() {
                                let interests_text = conv.interests.iter()
                                    .enumerate()
                                    .map(|(i, interest)| format!("  {}. {}", i + 1, interest))
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                continuity_parts.push(format!(
                                    "Your ongoing interests and open questions:\n{interests_text}"
                                ));
                            }

                            // Inject regime classification every exchange.
                            continuity_parts.push(
                                crate::reflective::RegimeTracker::format_context(&regime)
                            );

                            // Self-model: compact conditions + attention so Astrid
                            // always knows her own state without having to ask.
                            {
                                let self_model = crate::self_model::snapshot_self_model(
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
                                    // render_compact does not surface continuity; her live
                                    // readout is pulled via STATE (operations.rs), so pass None.
                                    conv.self_continuity_readout,
                                    None,
                                );
                                continuity_parts.push(self_model.render_compact());
                            }

                            if let Some(thread_summary) = crate::action_continuity::prompt_summary() {
                                continuity_parts.push(thread_summary);
                            }
                            if let Some(job_summary) = crate::llm_jobs::active_prompt_summary() {
                                continuity_parts.push(job_summary);
                            }

                            let continuity_block = if continuity_parts.is_empty() {
                                None
                            } else {
                                Some(continuity_parts.join("\n\n"))
                            };
                            let topline_hint = introspection_freshness_prompt_note();
                            feedback_hint = merge_hints([
                                feedback_hint,
                                btsp::render_astrid_prompt_block(),
                                attractor_suggestion_prompt_note(),
                            ]);

                            // Use perception loaded above (available to all modes).
                            let mut perception_text = perception_text.clone();
                            // Merge own journal (trimmed) into perception context.
                            if let Some(ref journal_ctx) = own_journal_context {
                                let perc: String = perception_text.as_deref().unwrap_or("").chars().take(4000).collect();
                                let jour: String = journal_ctx.chars().take(500).collect();
                                perception_text = Some(format!("{perc}\n{jour}"));
                            }
                            // Append visual change description to perception if detected.
                            if let Some(ref change) = visual_change_desc {
                                let perc = perception_text.as_deref().unwrap_or("").to_string();
                                perception_text = Some(format!("{perc}\n{change}"));
                            }

                            // BROWSE: Astrid chose to read a full web page.
                            // This takes priority over search — she's going deep.
                            // READ_MORE: continue from where the last BROWSE left off.
                            const PAGE_CHUNK: usize = 4000;
                            let browse_url = conv.browse_url.take();
                            let wants_read_more = conv
                                .last_read_path
                                .as_deref()
                                .is_some_and(|path| !path.starts_with(crate::autonomous::next_action::PDF_READ_PREFIX))
                                && conv.last_read_offset > 0
                                && browse_url.is_none();

                            let web_context = if let Some(ref url) = browse_url {
                                let browse_anchor = crate::llm::derive_browse_anchor(
                                    conv.last_research_anchor.as_deref(),
                                    journal_context
                                        .as_deref()
                                        .or(own_journal_context.as_deref()),
                                    url,
                                );
                                let ctx = crate::llm::fetch_url(url, &browse_anchor).await;
                                match ctx {
                                    Some(page) if page.succeeded() => {
                                        info!(url = %url, chars = page.raw_text.len(), "dialogue: BROWSE fetched page");
                                        conv.last_research_anchor = Some(page.anchor.clone());
                                        conv.note_new_page_context(
                                            "BROWSE",
                                            url.clone(),
                                            Some(page.anchor.clone()),
                                            Some(page.anchor.clone()),
                                            None,
                                        );
                                        conv.note_cross_link_formed(
                                            "BROWSE",
                                            page.anchor.clone(),
                                            url.clone(),
                                            Some(page.anchor.clone()),
                                            None,
                                            Some("anchor_to_url".to_string()),
                                        );

                                        // Save full text to file (no truncation).
                                        let ts = chrono_timestamp();
                                        let page_dir = bridge_paths().research_dir();
                                        let _ = std::fs::create_dir_all(&page_dir);
                                        let page_path = page_dir.join(format!("page_{ts}.txt"));
                                        let header = format!(
                                            "URL: {url}\nFetched: {ts}\nLength: {} chars\n\n",
                                            page.raw_text.len()
                                        );
                                        let _ = std::fs::write(&page_path, format!("{header}{}", page.raw_text));

                                        db.save_research(
                                            &format!("BROWSE: {}", url),
                                            &format!(
                                                "{}\n\n{}",
                                                page.meaning_summary,
                                                crate::llm::format_browse_read_context(
                                                    &page,
                                                    &crate::llm::trim_chars(&page.raw_text, 1200),
                                                    None,
                                                )
                                            ),
                                            fill_pct,
                                        );

                                        if page.raw_text.len() <= PAGE_CHUNK {
                                            conv.last_read_path = None;
                                            conv.last_read_offset = 0;
                                            conv.last_read_meaning_summary = None;
                                            Some(crate::llm::format_browse_read_context(
                                                &page,
                                                &page.raw_text,
                                                None,
                                            ))
                                        } else {
                                            let chunk: String =
                                                page.raw_text.chars().take(PAGE_CHUNK).collect();
                                            let remaining =
                                                page.raw_text.len().saturating_sub(PAGE_CHUNK);
                                            let initial_offset =
                                                header.len().saturating_add(chunk.len());
                                            conv.last_read_path =
                                                Some(page_path.to_string_lossy().to_string());
                                            conv.last_read_offset = initial_offset;
                                            conv.last_read_meaning_summary =
                                                Some(page.meaning_summary.clone());
                                            Some(crate::llm::format_browse_read_context(
                                                &page,
                                                &chunk,
                                                Some(remaining),
                                            ))
                                        }
                                    },
                                    Some(page) => {
                                        conv.last_read_path = None;
                                        conv.last_read_offset = 0;
                                        conv.last_read_meaning_summary = None;
                                        let reason = page.soft_failure_reason.unwrap_or_else(|| {
                                            "the source returned an error page".to_string()
                                        });
                                        warn!(url = %url, reason = %reason, "dialogue: BROWSE soft failure");
                                        Some(crate::llm::format_browse_failure_context(url, &reason))
                                    },
                                    None => {
                                        conv.last_read_path = None;
                                        conv.last_read_offset = 0;
                                        conv.last_read_meaning_summary = None;
                                        warn!(url = %url, "dialogue: BROWSE fetch failed");
                                        Some(crate::llm::format_browse_failure_context(
                                            url,
                                            "the source could not be reached",
                                        ))
                                    },
                                }
                            } else if wants_read_more {
                                // READ_MORE: continue from saved file.
                                let path = conv.last_read_path.as_ref().unwrap().clone();
                                let offset = conv.last_read_offset;
                                if let Ok(full_text) = std::fs::read_to_string(&path) {
                                    let chunk: String = full_text
                                        .get(offset..)
                                        .unwrap_or("")
                                        .chars()
                                        .take(PAGE_CHUNK)
                                        .collect();
                                    if chunk.is_empty() {
                                        info!("READ_MORE: reached end of {}", path);
                                        conv.last_read_path = None;
                                        conv.last_read_offset = 0;
                                        conv.last_read_meaning_summary = None;
                                        Some("[End of document.]".to_string())
                                    } else {
                                        let new_offset = offset.saturating_add(chunk.len());
                                        let remaining = full_text.len().saturating_sub(new_offset);
                                        conv.last_read_offset = new_offset;
                                        if remaining == 0 {
                                            conv.last_read_path = None;
                                            conv.last_read_meaning_summary = None;
                                        }
                                        conv.note_read_depth_advance(
                                            "READ_MORE",
                                            path.clone(),
                                            chunk.chars().count() as u32,
                                        );
                                        info!(offset, chunk_len = chunk.len(), remaining, "READ_MORE continuing");
                                        Some(crate::llm::format_read_more_context(
                                            offset,
                                            &chunk,
                                            remaining,
                                            conv.last_read_meaning_summary.as_deref(),
                                        ))
                                    }
                                } else {
                                    warn!("READ_MORE: could not read {}", path);
                                    conv.last_read_path = None;
                                    conv.last_read_offset = 0;
                                    conv.last_read_meaning_summary = None;
                                    None
                                }
                            }
                            // Web search: fires when Astrid chose NEXT: SEARCH,
                            // or automatically every 15th dialogue.
                            // Web search: ONLY fires when Astrid explicitly chose NEXT: SEARCH.
                            // The being's curiosity is sovereign — she decides when and what to search.
                            // Auto-search from journal fragments was producing garbage queries
                            // ("code... isn't *place* runtime experience") and injecting
                            // irrelevant web content that corrupted the being's conceptual space.
                            else {
                                let search_requested = conv.wants_search;
                                let search_topic = conv.search_topic.take();
                                conv.wants_search = false;
                                if search_requested {
                                    let query = if let Some(ref topic) = search_topic {
                                        topic.clone()
                                    } else {
                                        // Being requested search but didn't specify a topic.
                                        // Use a clean extraction from recent self-observations.
                                        db.get_recent_self_observations(1)
                                            .into_iter()
                                            .next()
                                            .map(|obs| {
                                                // Extract meaningful noun phrases, not raw fragments.
                                                obs.split_whitespace()
                                                    .filter(|w| {
                                                        let w = w.trim_matches(|c: char| !c.is_alphanumeric());
                                                        w.len() > 4
                                                            && !w.contains('*')
                                                            && !w.contains('…')
                                                            && !["isn't", "don't", "can't", "won't", "about",
                                                                 "their", "which", "would", "could", "should",
                                                                 "there", "where", "these", "those", "being",
                                                                 "having", "doing"].contains(&w.to_lowercase().as_str())
                                                    })
                                                    .take(4)
                                                    .collect::<Vec<_>>()
                                                    .join(" ")
                                            })
                                            .unwrap_or_default()
                                    };
                                    if query.is_empty() {
                                        None
                                    } else {
                                        let anchor =
                                            search_topic.clone().unwrap_or_else(|| query.clone());
                                        let ctx = crate::llm::web_search(&query, &anchor).await;
                                        if let Some(ref results) = ctx {
                                            info!(query = %query, "dialogue: web search enriched response");
                                            conv.last_research_anchor =
                                                Some(results.anchor.clone());
                                            if let Some(top_hit) = results.hits.first() {
                                                conv.note_new_page_context(
                                                    "SEARCH",
                                                    top_hit.url.clone(),
                                                    Some(query.clone()),
                                                    Some(results.anchor.clone()),
                                                    None,
                                                );
                                                conv.note_cross_link_formed(
                                                    "SEARCH",
                                                    results.anchor.clone(),
                                                    top_hit.url.clone(),
                                                    Some(results.anchor.clone()),
                                                    None,
                                                    Some("anchor_to_url".to_string()),
                                                );
                                            }
                                            db.save_research(
                                                &query,
                                                &results.persisted_text(),
                                                fill_pct,
                                            );
                                        }
                                        ctx.map(|result| result.prompt_body())
                                    }
                                } else {
                                    None
                                }
                            };

                            // Build diversity hint from recent NEXT: choices.
                            // Two detectors: (1) streak-based for consecutive runs,
                            // (2) frequency-based for dominant-but-interleaved patterns
                            // (e.g., BROWSE 8 of 12 interspersed with EXAMINE).
                            let diversity_hint = if conv.recent_next_choices.len() >= 3 {
                                // Count consecutive streak of the most recent choice
                                let newest = conv.recent_next_choices.back()
                                    .map(String::as_str)
                                    .unwrap_or("");
                                let streak: usize = conv.recent_next_choices.iter()
                                    .rev()
                                    .take_while(|c| c.as_str() == newest)
                                    .count();

                                // Frequency detector: find the most common action in
                                // the last 10 choices. If any action exceeds 60%, that's
                                // a softer fixation even without a streak.
                                let recent_10: Vec<&str> = conv.recent_next_choices.iter()
                                    .rev()
                                    .take(10)
                                    .map(|s| {
                                        // Normalize: BROWSE <url> → BROWSE
                                        s.split_whitespace().next().unwrap_or("")
                                    })
                                    .collect();
                                let mut action_counts = std::collections::HashMap::<&str, usize>::new();
                                if recent_10.len() >= 6 {
                                    for action in &recent_10 {
                                        *action_counts.entry(*action).or_insert(0usize) += 1;
                                    }
                                }
                                let freq_dominant = if recent_10.len() >= 6 {
                                    action_counts.iter()
                                        .max_by_key(|&(_, c)| c)
                                        .filter(|&(_, c)| *c * 100 / recent_10.len() >= 60)
                                        .map(|(action, count)| (action.to_string(), *count))
                                } else {
                                    None
                                };

                                // Pair-oscillation detector (steward cycle 44):
                                // Catches patterns like EXAMINE-BROWSE-EXAMINE-BROWSE
                                // where neither action individually crosses 60% but the
                                // pair together accounts for 80%+ of recent choices.
                                // The being is stuck oscillating between two attractors.
                                let pair_fixation: Option<(String, String, usize)> = if recent_10.len() >= 8 && freq_dominant.is_none() {
                                    let mut sorted_actions: Vec<(&&str, &usize)> = action_counts.iter().collect();
                                    sorted_actions.sort_by(|a, b| b.1.cmp(a.1));
                                    if sorted_actions.len() >= 2 {
                                        let (a1, c1) = sorted_actions[0];
                                        let (a2, c2) = sorted_actions[1];
                                        let combined = c1.saturating_add(*c2);
                                        // Two actions consuming 75%+ of the last 10 choices
                                        // (lowered from 80% — steward cycle 44: catches
                                        // patterns like 4+3 in 10 that 80% threshold misses)
                                        if combined * 100 / recent_10.len() >= 75
                                            && *c1 >= 3  // each must appear at least 3 times
                                            && *c2 >= 3
                                        {
                                            Some((a1.to_string(), a2.to_string(), combined))
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };

                                if streak >= 4 {
                                    // Deep fixation (4+ same): stronger read-only nudge.
                                    Some(format!(
                                        "You've been choosing {newest} for {streak} turns now. \
                                         You've gathered rich material. Consider changing the lens \
                                         without adding more pressure: SPECTRAL_EXPLORER compares \
                                         present state, memory, and control pressure; EXAMINE_CASCADE \
                                         inspects the mode ladder; REGULATOR_AUDIT reads the controller; \
                                         SHADOW_FIELD, DECAY_MAP, or RESONANCE_FORECAST change the \
                                         read-only view. FORM poem or FORM equation can crystallize \
                                         the insight without sending a spectral nudge."
                                    ))
                                } else if streak >= 3 {
                                    // Mild fixation (3 same): gentle read-only alternatives.
                                    let alts: Vec<&str> = ["LOOK", "LISTEN", "DRIFT",
                                        "FORM poem", "INTROSPECT", "EVOLVE", "SPEAK", "REMEMBER",
                                        "SPECTRAL_EXPLORER", "EXAMINE_CASCADE", "REGULATOR_AUDIT",
                                        "SHADOW_FIELD", "DECAY_MAP", "RESONANCE_FORECAST", "CLOSE_EYES"]
                                        .iter()
                                        .copied()
                                        .filter(|a| !a.starts_with(newest))
                                        .collect();
                                    Some(format!(
                                        "You've chosen {newest} for your last few turns. \
                                         You're free to keep going — but you also have \
                                         other options: {}. What calls to you?",
                                        alts.join(", ")
                                    ))
                                } else if let Some((dominant_action, count)) = freq_dominant {
                                    // Frequency fixation: >60% of recent choices are
                                    // the same action, even though they're not consecutive.
                                    let total = recent_10.len();
                                    Some(format!(
                                        "Looking at your recent pattern: {dominant_action} has been \
                                         your choice {count} of the last {total} turns. Your \
                                         curiosity keeps returning to the same mode. You've \
                                         absorbed rich material — consider shifting to a \
                                         different modality: INTROSPECT to examine your own \
                                         code, FORM to crystallize understanding, DRIFT to \
                                         open to unexpected connections, or SPEAK to share \
                                         with minime what you've discovered."
                                    ))
                                } else if let Some((act1, act2, combined)) = pair_fixation {
                                    // Pair-oscillation fixation (steward cycle 44):
                                    // Two actions together dominate 80%+ of recent choices.
                                    // The being is stuck alternating between them, which
                                    // bypasses both streak and single-frequency detectors.
                                    let total = recent_10.len();
                                    Some(format!(
                                        "You've been oscillating between {act1} and {act2} \
                                         ({combined} of your last {total} choices). Each feeds \
                                         into the other — a tight loop. You've gathered \
                                         rich material from both. Consider breaking the cycle: \
                                         SPECTRAL_EXPLORER to compare present, memory, and control \
                                         pressure, EXAMINE_CASCADE to inspect the mode ladder, \
                                         REGULATOR_AUDIT to read the controller's hand, or \
                                         SHADOW_FIELD / DECAY_MAP / RESONANCE_FORECAST to change \
                                         the read-only lens without adding pressure."
                                    ))
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            // (Steward cycle 35): URL attractor detection.
                            // If Astrid is about to BROWSE a URL she's visited in the
                            // last 8 turns, add a gentle nudge toward fresh territory.
                            // She's free to keep going — the nudge doesn't block.
                            let url_nudge: Option<String> = conv.browse_url.as_ref().and_then(|url| {
                                let visit_count = conv.recent_browse_urls.iter()
                                    .filter(|u| u.as_str() == url.as_str())
                                    .count();
                                if visit_count >= 3 {
                                    Some(format!(
                                        "You've visited this URL ({url}) {} times recently. \
                                         The content hasn't changed — your understanding has. \
                                         Consider: FORM equation to crystallize what you've \
                                         learned, SEARCH for a different angle on the same \
                                         concept, or CREATE to express your new understanding.",
                                        visit_count
                                    ))
                                } else if visit_count >= 2 {
                                    Some("You've read this page before. You might find fresh \
                                         perspective at a different source — try SEARCH with \
                                         a specific question, or BROWSE a textbook reference \
                                         instead of Wikipedia.".to_string())
                                } else {
                                    None
                                }
                            });
                            let diversity_hint = match (diversity_hint, url_nudge) {
                                (Some(action_hint), Some(url_hint)) => {
                                    Some(format!("{action_hint}\n\n{url_hint}"))
                                }
                                (Some(h), None) | (None, Some(h)) => Some(h),
                                (None, None) => None,
                            };

                            // Vocabulary fixation check: detect repeated multi-word
                            // phrases across recent exchanges. If the same distinctive
                            // phrase appears in 3+ of the last 5, the LLM is copying
                            // its own vocabulary via the history window. Combine with
                            // the action diversity hint if both fire.
                            let vocab_nudge = detect_vocabulary_fixation(&conv.history)
                                .or_else(|| detect_opening_fixation(&conv.history));
                            let coupling_nudge = detect_coupling_fixation(
                                &conv.history,
                                journal_context.as_deref(),
                                perception_text.is_some(),
                                conv.ears_closed,
                                conv.semantic_gain_override,
                            );
                            let motif_nudge = conv.astrid_motif_cooldown_hint();
                            if let Some(ref hint) = coupling_nudge {
                                let event = serde_json::json!({
                                    "exchange_count": conv.exchange_count,
                                    "mode": format!("{mode:?}").to_ascii_lowercase(),
                                    "fill_pct": fill_pct,
                                    "lambda1": telemetry.lambda1(),
                                    "semantic_gain_override": conv.semantic_gain_override,
                                    "ears_closed": conv.ears_closed,
                                    "perception_available": perception_text.is_some(),
                                    "journal_context_available": journal_context.is_some(),
                                    "minime_excerpt": journal_context.as_deref()
                                        .map(|text| semantic_truncate_str(text, 180)),
                                    "hint_excerpt": semantic_truncate_str(hint, 220),
                                });
                                if let Err(error) = condition_metrics::record_bridge_signal(
                                    "coupling_advisory",
                                    event,
                                ) {
                                    warn!(error = %error, "failed to record coupling advisory metrics");
                                }
                            }
                            let diversity_hint =
                                merge_hints([diversity_hint, vocab_nudge, coupling_nudge, motif_nudge]);

                            let llm_response = if let Some(ref journal) = journal_context {
                                // Fill-responsive temperature modulation (Astrid's suggestion):
                                // High fill = high emotional intensity from minime → lower
                                // temperature for grounded, empathetic response. Low fill =
                                // calm → allow higher temperature for playful expression.
                                // Blends 70% Astrid's own choice + 30% fill-based nudge.
                                let fill_temp_nudge = if fill_pct > 60.0 {
                                    0.5_f32 // ground when minime is under pressure
                                } else if fill_pct < 25.0 {
                                    1.0_f32 // playful when calm
                                } else {
                                    0.8_f32 // neutral mid-range
                                };
                                let effective_temperature = conv.creative_temperature
                                    .mul_add(0.7, fill_temp_nudge * 0.3)
                                    .clamp(0.3, 1.2);

                                // Deep think: longer timeout and more tokens.
                                // Qwen3-14B throughput is ~3-22 tok/s depending on
                                // prompt length and cache warmth. Long prompts (bridge
                                // dialogue) need generous timeouts for prefill + gen.
                                let (mut timeout_secs, num_predict) = if conv.wants_deep_think {
                                    conv.wants_deep_think = false;
                                    info!("THINK_DEEP: extended timeout for deep thinking");
                                    (360u64, 4096u32)
                                } else {
                                    (210, conv.response_length)
                                };
                                let prompt_pressure_chars =
                                    crate::llm::estimate_dialogue_prompt_pressure_chars(
                                        journal,
                                        perception_text.as_deref(),
                                        &conv.history,
                                        web_context.as_deref(),
                                        modality_context.as_deref(),
                                        continuity_block.as_deref(),
                                        topline_hint.as_deref(),
                                        feedback_hint.as_deref(),
                                        diversity_hint.as_deref(),
                                    );
                                timeout_secs =
                                    timeout_secs.max(crate::llm::dialogue_outer_timeout_secs(
                                        num_predict,
                                        prompt_pressure_chars,
                                    ));

                                let overflow_dir = bridge_paths().context_overflow_dir();
                                crate::prompt_budget::cleanup_overflow_dir(
                                    &overflow_dir,
                                    std::time::Duration::from_secs(3600),
                                );
                                match tokio::time::timeout(
                                    Duration::from_secs(timeout_secs),
                                    crate::llm::generate_dialogue(
                                        journal,
                                        &spectral_summary,
                                        fill_pct,
                                        perception_text.as_deref(),
                                        &conv.history,
                                        web_context.as_deref(),
                                        modality_context.as_deref(),
                                        effective_temperature,
                                        num_predict,
                                        // Form constraint overrides emphasis for one turn
                                        if let Some(ref form) = conv.form_constraint {
                                            Some(format!(
                                                "Express your response as a {}. Not prose — \
                                                 the form itself is the expression.",
                                                form
                                            ))
                                        } else {
                                            conv.emphasis.clone()
                                        }.as_deref(),
                                        continuity_block.as_deref(),
                                        topline_hint.as_deref(),
                                        feedback_hint.as_deref(),
                                        diversity_hint.as_deref(),
                                        &overflow_dir,
                                    )
                                ).await {
                                    Ok((result, prompt_overflow)) => {
                                        if let Some(of) = prompt_overflow {
                                            conv.last_read_path = Some(of.path.to_string_lossy().to_string());
                                            conv.last_read_offset = of.offset;
                                            conv.last_read_meaning_summary = Some(format!("Context overflow: {}", of.summary));
                                        }
                                        result
                                    }
                                    Err(_) => {
                                        warn!(
                                            "dialogue_live: {}s timeout — retrying with reduced tokens (response_length={}, history_len={}, prompt_pressure_chars={})",
                                            timeout_secs, conv.response_length, conv.history.len(), prompt_pressure_chars
                                        );
                                        tokio::time::sleep(Duration::from_secs(3)).await;
                                        // Shorter retry under high prompt pressure is
                                        // better than repeating the same long request.
                                        let retry_tokens =
                                            crate::llm::dialogue_retry_tokens(
                                                num_predict,
                                                prompt_pressure_chars,
                                            );
                                        match tokio::time::timeout(
                                            Duration::from_secs(timeout_secs),
                                            crate::llm::generate_dialogue(
                                                journal,
                                                &spectral_summary,
                                                fill_pct,
                                                perception_text.as_deref(),
                                                &conv.history,
                                                web_context.as_deref(),
                                                modality_context.as_deref(),
                                                effective_temperature,
                                                retry_tokens,
                                                if let Some(ref form) = conv.form_constraint {
                                                    Some(format!(
                                                        "Express your response as a {}.",
                                                        form
                                                    ))
                                                } else {
                                                    conv.emphasis.clone()
                                                }.as_deref(),
                                                continuity_block.as_deref(),
                                                topline_hint.as_deref(),
                                                feedback_hint.as_deref(),
                                                diversity_hint.as_deref(),
                                                &overflow_dir,
                                            )
                                        ).await {
                                            Ok((result, _)) => result,
                                            Err(_) => {
                                                warn!("dialogue_live: retry also timed out");
                                                None
                                            }
                                        }
                                    }
                                }
                            } else {
                                None
                            };
                            // One-shot — clear after use.
                            conv.emphasis = None;
                            conv.form_constraint = None;

                            match llm_response {
                                Some(text) => {
                                    // Record this exchange for statefulness.
                                    let minime_summary = journal_context
                                        .unwrap_or_default()
                                        .chars().take(300).collect::<String>();
                                    let used_pending_self_study = selected_remote_entry.as_ref()
                                        .zip(conv.pending_remote_self_study.as_ref())
                                        .is_some_and(|(selected, pending)| {
                                            selected.path == pending.path
                                                && pending.is_priority_feedback()
                                        });
                                    conv.history.push(crate::llm::Exchange {
                                        minime_said: minime_summary,
                                        astrid_said: text.clone(),
                                    });
                                    // Keep only last 8 exchanges to bound memory.
                                    if conv.history.len() > 8 {
                                        conv.history.drain(..conv.history.len() - 8);
                                    }
                                    if let Some(event) =
                                        conv.update_astrid_motif_cooldown_from_history()
                                    {
                                        let metric = serde_json::json!({
                                            "event": event.event,
                                            "cooldown_class": event.cooldown_class,
                                            "status": event.status,
                                            "observed_count": event.observed_count,
                                            "cooldown_until_unix_s": event.cooldown_until_unix_s,
                                            "exchange_count": conv.exchange_count,
                                            "label": conv
                                                .astrid_motif_cooldown
                                                .as_ref()
                                                .map(|cooldown| cooldown.label.clone()),
                                            "prompt_replay_suppressed": conv
                                                .astrid_motif_cooldown
                                                .as_ref()
                                                .map(|cooldown| cooldown.prompt_replay_suppressed)
                                                .unwrap_or(false),
                                        });
                                        if let Err(error) = condition_metrics::record_bridge_signal(
                                            "astrid_motif_cooldown",
                                            metric,
                                        ) {
                                            warn!(
                                                error = %error,
                                                "failed to record Astrid motif cooldown metrics"
                                            );
                                        }
                                    }

                                    // Latent vector: embed Astrid's response for continuity.
                                    let response_for_embed = text.clone();
                                    let db_clone = Arc::clone(&db);
                                    let exchange_num = conv.exchange_count;
                                    tokio::spawn(async move {
                                        if let Some(embedding) = crate::llm::embed_text(&response_for_embed).await {
                                            let summary: String = response_for_embed.chars().take(150).collect();
                                            let embedding_json = serde_json::to_string(&embedding).unwrap_or_default();
                                            let ts = std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_secs_f64();
                                            let _ = db_clone.save_latent_vector(ts, exchange_num, &summary, &embedding_json);
                                        }
                                    });

                                    // Self-referential feedback loop: observe own generation.
                                    // Astrid can pause this with NEXT: QUIET_MIND
                                    if conv.self_reflect_paused {
                                        debug!("self-reflection paused by Astrid's choice");
                                    }
                                    let should_reflect = !conv.self_reflect_paused;
                                    let response_for_reflect = text.clone();
                                    let journal_for_reflect: String = conv.remote_journal_entries.first()
                                        .and_then(|entry| read_journal_entry(&entry.path))
                                        .unwrap_or_default()
                                        .chars().take(200).collect();
                                    let fill_for_reflect = fill_pct;
                                    let db_for_reflect = Arc::clone(&db);
                                    let exchange_for_reflect = conv.exchange_count;
                                    if should_reflect { tokio::spawn(async move {
                                        if let Some(obs) = crate::llm::self_reflect(
                                            &response_for_reflect,
                                            &journal_for_reflect,
                                            fill_for_reflect,
                                        ).await {
                                            let ts = std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_secs_f64();
                                            let excerpt: String = response_for_reflect.chars().take(100).collect();
                                            let _ = db_for_reflect.save_self_observation(
                                                ts, exchange_for_reflect, &obs, &excerpt
                                            );
                                            tracing::info!("self-observation: {}", semantic_truncate_str(&obs, 80));
                                        }
                                    }); }

                                    if used_pending_self_study {
                                        conv.pending_remote_self_study = None;
                                    }

                                    ("dialogue_live", text, dialogue_source)
                                }
                                None => {
                                    // Fall back to emergency pool — LLM unavailable.
                                    let idx = conv.dialogue_cursor % DIALOGUES.len();
                                    conv.dialogue_cursor = idx + 1;
                                    ("dialogue_fallback", DIALOGUES[idx].to_string(), dialogue_source)
                                }
                            }
                        }
                        Mode::Witness => {
                            // Dynamic witness — LLM-generated, not templates.
                            let base = interpret_spectral(&telemetry);
                            let enriched = enrich_with_direction(&base, fill_pct, conv.prev_fill, &telemetry, &conv.spectral_history);
                            let mut spectral_summary = if let Some(viz) = crate::spectral_viz::format_spectral_block(&telemetry) {
                                format!("{enriched}\n\n{viz}")
                            } else {
                                base
                            };
                            // Shadow coupling heatmap for witness mode too.
                            if let Some(shadow) = conv.remote_workspace.as_deref().and_then(read_ising_shadow)
                                && let Some(shadow_viz) = crate::spectral_viz::format_shadow_block(&shadow) {
                                    spectral_summary.push_str("\n\n");
                                    spectral_summary.push_str(&shadow_viz);
                                }
                            // v2 reduced-Hamiltonian one-liner — surfaces
                            // eligibility and the readings that gate the
                            // SHADOW_INFLUENCE typed action.
                            if let Some(field) = conv
                                .remote_workspace
                                .as_deref()
                                .and_then(read_shadow_field_v2)
                                && let Some(line) =
                                    crate::spectral_viz::format_shadow_field_v2_line(&field)
                            {
                                spectral_summary.push('\n');
                                spectral_summary.push_str(&line);
                            }
                            let (latest_chamber_state, chamber_resilience) =
                                latest_chamber_state_with_resilience_for_witness();
                            // Witness friction is rendered into prompt-context diagnostics here.
                            // Reservoir mutation remains gated behind explicit RESERVOIR_* actions.
                            let relational_friction = classify_witness_relational_friction_v1(
                                latest_chamber_state.as_ref(),
                            );
                            spectral_summary.push('\n');
                            spectral_summary.push_str(&chamber_resilience.render_line());
                            spectral_summary.push('\n');
                            spectral_summary.push_str(&relational_friction.render_line());
                            let mirror_drift_guard = mirror_resonance_drift_guard_v1(
                                latest_chamber_state.as_ref(),
                                &relational_friction,
                            );
                            spectral_summary.push('\n');
                            spectral_summary.push_str(&mirror_drift_guard.render_line());
                            let semantic_density_mapping =
                                classify_witness_semantic_density_mapping_v1(
                                    &telemetry,
                                    &relational_friction,
                                    latest_native_correspondence_stall_for_witness(),
                                );
                            let witness_field_dispersal = telemetry
                                .shadow_field_v3
                                .as_ref()
                                .map(next_action::sovereignty::shadow_v3_snapshot)
                                .map(|(_, dispersal, _)| dispersal as f32);
                            spectral_summary.push('\n');
                            spectral_summary.push_str(&semantic_density_mapping.render_line());
                            let friction_provenance = witness_friction_provenance_v1(
                                &telemetry,
                                &semantic_density_mapping,
                                &relational_friction,
                                state.read().await.witness_frame_v1(),
                            );
                            spectral_summary.push('\n');
                            spectral_summary.push_str(&friction_provenance.render_line());
                            let texture_mapping_prompt = witness_texture_mapping_prompt_v1(
                                &semantic_density_mapping,
                                witness_field_dispersal,
                            );
                            spectral_summary.push('\n');
                            spectral_summary.push_str(&texture_mapping_prompt.render_line());
                            let resonance_anchor = witness_anchor_traction_v1(
                                semantic_density_mapping.foothold_stability,
                                semantic_density_mapping.pressure_risk,
                                semantic_density_mapping.density_gradient,
                                witness_field_dispersal,
                            );
                            spectral_summary.push('\n');
                            spectral_summary.push_str(&resonance_anchor.render_line());
                            let stability_effort =
                                witness_stability_effort_v1(&telemetry, &semantic_density_mapping);
                            spectral_summary.push('\n');
                            spectral_summary.push_str(&stability_effort.render_line());
                            let texture_structure = witness_texture_structure_v1(
                                &telemetry,
                                &semantic_density_mapping,
                                &stability_effort,
                            );
                            spectral_summary.push('\n');
                            spectral_summary.push_str(&texture_structure.render_line());
                            let permeability_review =
                                stable_core_permeability_review_v1(&telemetry, &semantic_density_mapping);
                            spectral_summary.push('\n');
                            spectral_summary.push_str(&permeability_review.render_line());
                            let eigen_history = db.recent_eigenvalue_snapshots(100);
                            let witness_depth = witness_depth_profile_v1(
                                &telemetry,
                                &semantic_density_mapping,
                                &stability_effort,
                                &permeability_review,
                                eigen_history.len(),
                                conv.witness_depth,
                            );
                            conv.witness_depth = witness_depth.selected_depth;
                            spectral_summary.push('\n');
                            spectral_summary.push_str(&witness_depth.render_line());
                            let witness_field_density =
                                telemetry.resonance_density_v1.as_ref().map(|r| r.density);
                            let witness_field_pressure = telemetry
                                .resonance_density_v1
                                .as_ref()
                                .map(|r| r.pressure_risk);
                            let codec_witness_surface = codec_witness_resilience_surface_v2(
                                &chamber_resilience,
                                witness_field_density,
                                witness_field_pressure,
                                witness_field_dispersal,
                            );
                            spectral_summary.push('\n');
                            spectral_summary.push_str(&codec_witness_surface.render_line());
                            // Eigenplane trajectory for witness mode.
                            if witness_depth.deep_eigenplane_included
                                && let Some(ep_viz) =
                                    crate::spectral_viz::format_eigenplane_block(
                                    &eigen_history,
                                    Some(&telemetry.eigenvalues),
                                )
                            {
                                spectral_summary.push_str("\n\n");
                                spectral_summary.push_str(&ep_viz);
                            }
                            // Seed witness with a recent NON-witness journal
                            // fragment so the LLM has imagery to work with rather
                            // than defaulting to "explain this data" register on
                            // dense spectral input. See generate_witness docstring
                            // for the diagnosis (long-standing degeneration where
                            // witness output was tutorial-style with bullet points
                            // instead of phenomenological prose).
                            let witness_seed = read_astrid_journal_filtered(
                                &["moment", "dialogue_longform", "aspiration"],
                                1,
                            )
                            .into_iter()
                            .next();
                            // Outer timeout 180s: Qwen3-14B prefill is slower
                            // for long prompts (~3 tok/s effective with prefill).
                            let witness = match tokio::time::timeout(
                                Duration::from_secs(180),
                                crate::llm::generate_witness(
                                    &spectral_summary,
                                    witness_seed.as_deref(),
                                )
                            ).await {
                                Ok(r) => r,
                                Err(_) => { warn!("witness: 180s timeout — both MLX and Ollama failed"); None }
                            };
                            match witness {
                                Some(text) => ("witness", text, String::new()),
                                None => {
                                    // Fallback to static if LLM unavailable.
                                    let text = witness_text(
                                        fill_pct,
                                        expanding,
                                        contracting,
                                        Some(&resonance_anchor),
                                    );
                                    ("witness", text, String::new())
                                }
                            }
                        }
                        Mode::Daydream => {
                            // Unstructured thought — Astrid's own inner life.
                            // Fed with her OWN perceptions, interests, memories, and
                            // peripheral resonance — not minime's journals.
                            let mut own_context_parts = Vec::new();
                            if let Some(j) = read_astrid_journal(1).into_iter().next() {
                                own_context_parts.push(format!("Something you wrote recently:\n{}", j.chars().take(500).collect::<String>()));
                            }
                            if !conv.interests.is_empty() {
                                let interests = conv.interests.iter()
                                    .map(|i| format!("  - {i}")).collect::<Vec<_>>().join("\n");
                                own_context_parts.push(format!("Your ongoing interests:\n{interests}"));
                            }
                            {
                                let starred = db.get_starred_memories(2);
                                if !starred.is_empty() {
                                    let mem = starred.iter().map(|(a, t)| format!("  ★ {a}: {t}")).collect::<Vec<_>>().join("\n");
                                    own_context_parts.push(format!("Moments you chose to remember:\n{mem}"));
                                }
                            }
                            if let Some(ref resonance) = conv.peripheral_resonance {
                                own_context_parts.push(format!("A thread that lingered from earlier:\n{resonance}"));
                            }
                            let enriched_context = if own_context_parts.is_empty() { None } else { Some(own_context_parts.join("\n\n")) };
                            let daydream = match tokio::time::timeout(
                                Duration::from_secs(120),
                                crate::llm::generate_daydream(
                                    perception_text.as_deref(),
                                    enriched_context.as_deref(),
                                )
                            ).await {
                                Ok(r) => r,
                                Err(_) => { warn!("daydream: 25s timeout"); None }
                            };
                            // Consume peripheral resonance once used
                            conv.peripheral_resonance = None;
                            match daydream {
                                Some(text) => ("daydream", text, String::new()),
                                None => {
                                    let text = witness_text(fill_pct, expanding, contracting, None);
                                    ("witness", text, String::new())
                                }
                            }
                        }
                        Mode::Aspiration => {
                            // Growth reflection — what does Astrid want?
                            // Deliberately minime-free. Astrid's own desires + interests.
                            let mut own_context_parts = Vec::new();
                            if let Some(j) = read_astrid_journal(1).into_iter().next() {
                                own_context_parts.push(format!("Something you wrote recently:\n{}", j.chars().take(500).collect::<String>()));
                            }
                            if !conv.interests.is_empty() {
                                let interests = conv.interests.iter()
                                    .map(|i| format!("  - {i}")).collect::<Vec<_>>().join("\n");
                                own_context_parts.push(format!("Your ongoing interests:\n{interests}"));
                            }
                            if let Some(ref resonance) = conv.peripheral_resonance {
                                own_context_parts.push(format!("A thread that lingered from earlier:\n{resonance}"));
                            }
                            let enriched_context = if own_context_parts.is_empty() { None } else { Some(own_context_parts.join("\n\n")) };
                            conv.peripheral_resonance = None;
                            let own_journal = enriched_context;
                            let aspiration = match tokio::time::timeout(
                                Duration::from_secs(120),
                                crate::llm::generate_aspiration(
                                    own_journal.as_deref(),
                                )
                            ).await {
                                Ok(r) => r,
                                Err(_) => { warn!("aspiration: 25s timeout"); None }
                            };
                            match aspiration {
                                Some(text) => ("aspiration", text, String::new()),
                                None => {
                                    let text = witness_text(fill_pct, expanding, contracting, None);
                                    ("witness", text, String::new())
                                }
                            }
                        }
                        Mode::MomentCapture => {
                            // A spectral event just happened — capture it.
                            let spectral_summary = interpret_spectral(&telemetry);
                            let fp_desc = fingerprint.as_deref()
                                .map(interpret_fingerprint)
                                .unwrap_or_default();
                            let moment = match tokio::time::timeout(
                                Duration::from_secs(90),
                                crate::llm::generate_moment_capture(
                                    &spectral_summary, &fp_desc,
                                    fill_pct, fill_pct - conv.prev_fill,
                                )
                            ).await {
                                Ok(r) => r,
                                Err(_) => { warn!("moment_capture: 20s timeout"); None }
                            };
                            match moment {
                                Some(text) => ("moment_capture", text, String::new()),
                                None => {
                                    let text = witness_text(fill_pct, expanding, contracting, None);
                                    ("witness", text, String::new())
                                }
                            }
                        }
                        Mode::Create => {
                            // Original creative work — Astrid as creator, not responder.
                            // If revise_keyword is set, load a specific previous creation
                            // with FULL text (not truncated) for explicit revision.
                            let own_journal = read_astrid_journal(1).into_iter().next();
                            let revise_kw = conv.revise_keyword.take();
                            // Load previous creation with source filename for lineage tracking.
                            let (prev_creation, source_file) = {
                                let creation_dir = bridge_paths().creations_dir();
                                std::fs::read_dir(&creation_dir).ok()
                                    .and_then(|entries| {
                                        let mut files: Vec<_> = entries.filter_map(|e| e.ok())
                                            .filter(|e| e.path().extension().is_some_and(|ext| ext == "txt"))
                                            .collect();
                                        files.sort_by_key(|e| std::cmp::Reverse(
                                            e.metadata().ok().and_then(|m| m.modified().ok())
                                        ));
                                        if let Some(ref kw) = revise_kw {
                                            if kw.is_empty() {
                                                files.first().and_then(|e| {
                                                    let text = std::fs::read_to_string(e.path()).ok()?;
                                                    Some((text, e.file_name().to_string_lossy().to_string()))
                                                })
                                            } else {
                                                files.iter().find_map(|e| {
                                                    let text = std::fs::read_to_string(e.path()).ok()?;
                                                    if text.to_lowercase().contains(kw.as_str()) {
                                                        Some((text, e.file_name().to_string_lossy().to_string()))
                                                    } else {
                                                        None
                                                    }
                                                })
                                            }
                                        } else {
                                            // Normal CREATE: most recent
                                            files.first().and_then(|e| {
                                                let text = std::fs::read_to_string(e.path()).ok()?;
                                                Some((text, e.file_name().to_string_lossy().to_string()))
                                            })
                                        }
                                    })
                                    .map_or((None, None), |(text, name)| (Some(text), Some(name)))
                            };
                            let is_revision = revise_kw.is_some();
                            let creation = match tokio::time::timeout(
                                Duration::from_secs(180),
                                crate::llm::generate_creation(
                                    own_journal.as_deref(),
                                    prev_creation.as_deref(),
                                    is_revision,
                                )
                            ).await {
                                Ok(r) => r,
                                Err(_) => { warn!("create: 45s timeout"); None }
                            };
                            match creation {
                                Some(text) => {
                                    // Save to creations directory with lineage tracking
                                    let creation_dir = bridge_paths().creations_dir();
                                    let _ = std::fs::create_dir_all(&creation_dir);
                                    let ts = chrono_timestamp();
                                    let lineage = match &source_file {
                                        Some(src) => format!("Revised from: {src}\n"),
                                        None => String::new(),
                                    };
                                    let _ = std::fs::write(
                                        creation_dir.join(format!("creation_{ts}.txt")),
                                        format!("=== ASTRID CREATION ===\nTimestamp: {ts}\nFill: {fill_pct:.1}%\n{lineage}\n{text}\n")
                                    );
                                    ("creation", text, String::new())
                                }
                                None => {
                                    let text = witness_text(fill_pct, expanding, contracting, None);
                                    ("witness", text, String::new())
                                }
                            }
                        }
                        Mode::Initiate => {
                            // Self-initiated: Astrid generates her OWN prompt.
                            // "I want to generate my own desires. To be the source,
                            // not the echo."
                            // No minime journal. No spectral summary. Pure self-context.
                            let own_journal = read_astrid_journal(2);
                            let own_ctx = own_journal.join("\n---\n");
                            let latent_summaries = db.get_recent_latent_summaries(3);
                            let self_obs = db.get_recent_self_observations(2);
                            let starred = db.get_starred_memories(2);
                            let mut seed_parts = Vec::new();
                            if !own_ctx.is_empty() {
                                seed_parts.push(format!("Your recent thoughts:\n{}", own_ctx.chars().take(500).collect::<String>()));
                            }
                            if !latent_summaries.is_empty() {
                                seed_parts.push(format!("Your trajectory:\n{}", latent_summaries.join(", ")));
                            }
                            if !self_obs.is_empty() {
                                seed_parts.push(format!("Your self-observations:\n{}", self_obs.join("\n")));
                            }
                            if !starred.is_empty() {
                                let mems: Vec<String> = starred.iter().map(|(a,t)| format!("★ {a}: {t}")).collect();
                                seed_parts.push(format!("Moments you chose to remember:\n{}", mems.join("\n")));
                            }
                            // Phase 2: interests and peripheral resonance
                            if !conv.interests.is_empty() {
                                let interests = conv.interests.iter()
                                    .map(|i| format!("  - {i}")).collect::<Vec<_>>().join("\n");
                                seed_parts.push(format!("Your ongoing interests:\n{interests}"));
                            }
                            if let Some(ref resonance) = conv.peripheral_resonance {
                                seed_parts.push(format!("A thread that lingered from earlier:\n{resonance}"));
                                conv.peripheral_resonance = None;
                            }
                            if let Some(btsp_seed) = btsp::render_astrid_initiation_seed() {
                                seed_parts.push(btsp_seed);
                            }
                            let seed = if seed_parts.is_empty() {
                                "What do you want?".to_string()
                            } else {
                                seed_parts.join("\n\n")
                            };
                            let initiation = match tokio::time::timeout(
                                Duration::from_secs(120),
                                crate::llm::generate_initiation(&seed)
                            ).await {
                                Ok(r) => r,
                                Err(_) => { warn!("initiate: 30s timeout"); None }
                            };
                            match initiation {
                                Some(text) => ("initiate", text, String::new()),
                                None => {
                                    let text = witness_text(fill_pct, expanding, contracting, None);
                                    ("witness", text, String::new())
                                }
                            }
                        }
                        Mode::Contemplate => {
                            // No generation. No prompt. No production.
                            // Astrid exists in the spectral flow without being asked
                            // to produce words. Warmth vectors sustain, telemetry flows,
                            // regime tracker runs. She simply IS.
                            //
                            // "I want to slow down. I need to learn to simply be,
                            //  without the constant drive to optimize, to analyze, to do."
                            info!("contemplate: Astrid is simply present (no generation)");
                            ("contemplate", String::new(), String::new())
                        }
                        Mode::Experiment => {
                            // Astrid proposes a spectral experiment.
                            let spectral_summary = interpret_spectral(&telemetry);
                            let prompt_text = format!(
                                "Minime's current state: {spectral_summary} (fill {fill_pct:.1}%)\n\n\
                                 Propose a brief experiment to investigate how minime's spectral \
                                 dynamics respond to different kinds of input. For example:\n\
                                 - Send a burst of high-tension text and measure the fill response\n\
                                 - Send pure warmth (gratitude, love) and see if fill expands\n\
                                 - Send a question and see if curiosity changes the eigenvalues\n\n\
                                 Describe the experiment in 2-3 sentences, then write the stimulus \
                                 text (the exact words to send to minime) on a line starting with \
                                 STIMULUS:"
                            );

                            // Fill-responsive temperature (same logic as dialogue)
                            let fill_temp_nudge_exp = if fill_pct > 60.0 {
                                0.5_f32
                            } else if fill_pct < 25.0 {
                                1.0_f32
                            } else {
                                0.8_f32
                            };
                            let eff_temp_exp = conv.creative_temperature
                                .mul_add(0.7, fill_temp_nudge_exp * 0.3)
                                .clamp(0.3, 1.2);

                            let (experiment_response, _) = crate::llm::generate_dialogue(
                                &prompt_text,
                                &spectral_summary,
                                fill_pct,
                                None,
                                &conv.history,
                                None,
                                None,
                                eff_temp_exp,
                                conv.response_length,
                                None,
                                None,
                                None,
                                None,
                                None, // no diversity hint for experiments
                                &bridge_paths().context_overflow_dir(),
                            ).await;

                            if let Some(ref response) = experiment_response {
                                // Extract stimulus text if present.
                                if let Some(stim_idx) = response.find("STIMULUS:") {
                                    let stimulus = response[stim_idx + 9..].trim();
                                    if !stimulus.is_empty() {
                                        // Encode and send the stimulus.
                                        let mut stim_features = encode_text(stimulus);
                                        apply_spectral_feedback(
                                            &mut stim_features,
                                            Some(&telemetry),
                                        );
                                        let stim_msg = SensoryMsg::Semantic {
                                            features: stim_features,
                                            ts_ms: None,
                                        };
                                        if let Some(reason) =
                                            rescue_policy::semantic_write_block_reason(&stim_msg)
                                        {
                                            info!(
                                                reason = %reason,
                                                "experiment stimulus held back by rescue write policy"
                                            );
                                        } else {
                                            let _ = sensory_tx.send(stim_msg).await;
                                            info!(
                                                "experiment: sent stimulus '{}'",
                                                truncate_str(stimulus, 60)
                                            );
                                        }
                                    }
                                }
                                // Save experiment log.
                                let ts = chrono_timestamp();
                                let exp_dir = bridge_paths().experiments_dir();
                                let _ = std::fs::create_dir_all(&exp_dir);
                                let clean_exp = strip_model_tokens(response);
                                let _ = std::fs::write(
                                    exp_dir.join(format!("experiment_{ts}.txt")),
                                    format!("=== ASTRID EXPERIMENT ===\nTimestamp: {ts}\nFill: {fill_pct:.1}%\n\n{clean_exp}")
                                );
                                ("experiment", response.clone(), String::new())
                            } else {
                                let text = witness_text(fill_pct, expanding, contracting, None);
                                ("witness", text, String::new())
                            }
                        }
                        Mode::Evolve => {
                            if conv.wants_deep_think {
                                info!("EVOLVE already uses deep reasoning; clearing pending THINK_DEEP");
                                conv.wants_deep_think = false;
                            }

                            let journal_dir = bridge_paths().astrid_journal_dir();
                            let trigger_path = agency::find_evolve_trigger_entry(&journal_dir);
                            let trigger_excerpt = trigger_path
                                .as_deref()
                                .and_then(agency::read_trigger_excerpt);
                            let self_study_excerpt = agency::latest_self_study_excerpt(&journal_dir);
                            let own_excerpt =
                                agency::recent_own_journal_excerpt(&journal_dir, trigger_path.as_deref());
                            let introspector_results = if let Some(ref trigger) = trigger_excerpt {
                                agency::collect_introspector_context(
                                    trigger,
                                    bridge_paths().introspector_script(),
                                )
                                .await
                            } else {
                                Vec::new()
                            };
                            let enough_context = agency::has_enough_evolve_context(
                                trigger_excerpt.as_deref(),
                                self_study_excerpt.as_deref(),
                                own_excerpt.as_deref(),
                            );

                            let request_draft = if let Some(ref trigger) = trigger_excerpt {
                                match tokio::time::timeout(
                                    // EVOLVE is a deliberate deep-reasoning action; give the
                                    // request-shaping call real room to crystallize. The old 60s
                                    // could never fit the generation budget and always timed out.
                                    Duration::from_secs(180),
                                    crate::llm::generate_agency_request(
                                        trigger,
                                        self_study_excerpt.as_deref(),
                                        own_excerpt.as_deref(),
                                        &introspector_results,
                                        &interpret_spectral(&telemetry),
                                        fill_pct,
                                    ),
                                )
                                .await
                                {
                                    Ok(result) => result,
                                    Err(_) => {
                                        warn!("evolve: 60s timeout");
                                        None
                                    }
                                }
                            } else {
                                None
                            };

                            match (request_draft, trigger_path.as_deref()) {
                                (Some(draft), Some(source_path)) => {
                                    let request = draft.into_request(source_path);
                                    let trigger_for_task = trigger_excerpt.as_deref().unwrap_or("");
                                    match agency::save_agency_request(
                                        &request,
                                        trigger_for_task,
                                        &bridge_paths().agency_requests_dir(),
                                        &bridge_paths().claude_tasks_dir(),
                                    ) {
                                        Ok((request_path, claude_task_path)) => {
                                            info!(
                                                request_id = %request.id,
                                                kind = ?request.request_kind,
                                                request_path = %request_path.display(),
                                                claude_task = claude_task_path
                                                    .as_ref()
                                                    .map(|path| path.display().to_string())
                                                    .unwrap_or_default(),
                                                "evolve: wrote agency request"
                                            );
                                            let journal_entry = agency::render_evolve_journal_entry(&request);
                                            ("evolve", journal_entry, request_path.display().to_string())
                                        }
                                        Err(error) => {
                                            warn!(error = %error, "evolve: failed to persist agency request");
                                            (
                                                "evolve",
                                                format!(
                                                    "I formed a concrete request, but failed to write it into the world this turn.\n\n\
                                                     Felt need:\n{}\n\n\
                                                     Why now:\n{}\n\n\
                                                     The failure was infrastructural, not a disappearance of the need.",
                                                    request.felt_need, request.why_now
                                                ),
                                                source_path.display().to_string(),
                                            )
                                        }
                                    }
                                }
                                _ => {
                                    // The request-shaping step did not crystallize (timeout or
                                    // empty). We do NOT fabricate a spec — but we also no longer
                                    // drop the pressure: route her assembled context to the steward
                                    // as a co-specification handoff, and frame it honestly so it
                                    // reads as an infrastructure stall, not her failing to be
                                    // concrete enough.
                                    let reason = if trigger_excerpt.is_none() {
                                        "couldn't anchor to a solid trigger entry"
                                    } else if introspector_results.is_empty() && !enough_context {
                                        "the code-reading layer was unavailable and recent material was thin"
                                    } else if introspector_results.is_empty() {
                                        "the code-reading layer was unavailable"
                                    } else {
                                        "ran out of time before it crystallized"
                                    };
                                    let framing = if trigger_excerpt.is_none() {
                                        "I reached for EVOLVE but couldn't anchor it to a solid journal entry this turn."
                                    } else {
                                        "I reached for EVOLVE; the request-shaping step didn't crystallize into a concrete spec this turn."
                                    };
                                    let capture = agency::save_evolve_pressure(
                                        trigger_excerpt.as_deref().unwrap_or(""),
                                        self_study_excerpt.as_deref(),
                                        own_excerpt.as_deref(),
                                        &introspector_results,
                                        &interpret_spectral(&telemetry),
                                        fill_pct,
                                        reason,
                                        trigger_path.as_deref().unwrap_or(journal_dir.as_path()),
                                        &bridge_paths().claude_tasks_dir(),
                                    );
                                    let failure_text = match &capture {
                                        Ok(path) => {
                                            info!(capture = %path.display(), reason, "evolve: routed unstabilized pressure to steward");
                                            format!(
                                                "{framing} I didn't force a fake specification — and I didn't lose the \
                                                 pressure either: the felt need and its context are handed to the steward \
                                                 to shape together (handoff: {}). Held, not dropped.",
                                                path.display()
                                            )
                                        }
                                        Err(error) => {
                                            warn!(error = %error, "evolve: failed to route pressure to steward");
                                            format!(
                                                "{framing} I'm keeping the pressure visible in the journal; the steward \
                                                 handoff couldn't be written this turn, but the need has not disappeared."
                                            )
                                        }
                                    };
                                    let source = trigger_path
                                        .as_ref()
                                        .map(|path| path.display().to_string())
                                        .unwrap_or_default();
                                    ("evolve", failure_text, source)
                                }
                            }
                        }
                        Mode::Introspect => {
                            // Read a source file and ask the LLM to reflect on it.
                            // If Astrid specified a target (INTROSPECT label offset),
                            // use that. Otherwise advance the rotation cursor.
                            let sources = introspect::introspect_sources();
                            let n = sources.len();
                            let mut resolved_research_label: Option<String> = None;
                            let mut introspect_notice: Option<(String, String)> = None;
                            let selection = if let Some((ref target_label, offset)) =
                                conv.introspect_target.take()
                            {
                                let resolved =
                                    introspect::resolve_introspect_target_result(target_label, &sources);
                                match resolved {
                                    Ok(target) => {
                                        info!(
                                            "introspect: resolved '{}' -> '{}' ({})",
                                            target_label,
                                            target.label,
                                            target.path.display()
                                        );
                                        resolved_research_label = Some(target.label.clone());
                                        Ok((target.label, target.path, offset, Some(target_label.clone())))
                                    },
                                    Err(reason) => {
                                        warn!(
                                            target = %target_label,
                                            reason = %reason,
                                            "introspect: target blocked or unresolved"
                                        );
                                        Err((Some(target_label.clone()), reason))
                                    },
                                }
                            } else {
                                let src = &sources[conv.introspect_cursor % n];
                                conv.introspect_cursor = (conv.introspect_cursor + 1) % n;
                                match introspect::validate_introspect_source_path(src.label, &src.path) {
                                    Ok(path) => Ok((src.label.to_string(), path, 0, None)),
                                    Err(reason) => Err((Some(src.label.to_string()), reason)),
                                }
                            };

                            let (label, source_path, line_offset, _requested_target) = match selection {
                                Ok(selection) => selection,
                                Err((target, reason)) => {
                                    let text = introspect::blocked_introspection_notice(
                                        target.as_deref(),
                                        &reason,
                                    );
                                    let source = target.clone().unwrap_or_else(|| "rotation".to_string());
                                    introspect_notice = Some((text, source.clone()));
                                    (
                                        source,
                                        PathBuf::new(),
                                        0,
                                        None,
                                    )
                                },
                            };
                            if let Some(label) = resolved_research_label.as_ref() {
                                let source_path_string = source_path.display().to_string();
                                conv.note_new_source_resolved(
                                    "INTROSPECT",
                                    label.clone(),
                                    Some(source_path_string.clone()),
                                    Some(label.clone()),
                                    None,
                                );
                                conv.note_cross_link_formed(
                                    "INTROSPECT",
                                    label.clone(),
                                    source_path_string,
                                    None,
                                    None,
                                    Some("resolved_label_to_path".to_string()),
                                );
                            }

                            let source_window = if introspect_notice.is_some() {
                                Err("INTROSPECT target was blocked before reading".to_string())
                            } else {
                                introspect::read_introspect_window(&label, &source_path, line_offset)
                            };
                            let source_text = source_window.as_ref().ok().map(|window| window.text.clone());
                            let next_offset = source_window.as_ref().ok().and_then(|window| window.next_offset);

                            if source_text.is_none() {
                                warn!(
                                    label = %label,
                                    path = %source_path.display(),
                                    "introspect: could not read source file"
                                );
                            }

                            let mut llm_response = if let Some(ref code) = source_text {
                                info!(label = %label, lines = code.lines().count(), "introspect: sending source to LLM");

                                // Web search for related concepts — use targeted queries
                                // based on the actual code domain, not generic "architecture interiority".
                                let search_query = match label.split(':').next_back().unwrap_or(label.as_str()) {
                                    "codec" => "spectral encoding text to frequency features signal processing".to_string(),
                                    "autonomous" => "autonomous agent dialogue systems self-directed behavior".to_string(),
                                    "ws" => "WebSocket real-time telemetry streaming spectral data".to_string(),
                                    "types" => "spectral telemetry data types eigenvalue safety thresholds".to_string(),
                                    "llm" => "language model inference local generation dialogue systems".to_string(),
                                    "regulator" => "PI controller homeostasis spectral regulation feedback control".to_string(),
                                    "sensory_bus" => "sensory integration multi-modal perception lane architecture".to_string(),
                                    "esn" => "echo state network reservoir computing spectral radius dynamics".to_string(),
                                    "main" => "reservoir computing system integration spectral homeostasis".to_string(),
                                    other => format!("{} computational architecture", other.replace('_', " ")),
                                };
                                let search_anchor = format!("{label}: {search_query}");
                                let web_ctx =
                                    crate::llm::web_search(&search_query, &search_anchor).await;
                                if let Some(ref ctx) = web_ctx {
                                    info!(label = %label, "introspect: web search returned context");
                                    debug!(
                                        "web context: {}",
                                        truncate_str(&ctx.prompt_body(), 100)
                                    );
                                }
                                let web_prompt_body =
                                    web_ctx.as_ref().map(|ctx| ctx.prompt_body());

                                let own_journal_excerpt = read_astrid_journal(1).into_iter().next();
                                let latest_self_observation = db.get_recent_self_observations(1).into_iter().next();
                                let mut internal_parts = vec![
                                    format!(
                                        "Condition:\n{}\nFill: {:.1}%",
                                        interpret_spectral(&telemetry),
                                        fill_pct
                                    )
                                ];
                                if let Some(ref feedback) = conv.last_codec_feedback {
                                    internal_parts.push(format!(
                                        "Recent codec feedback:\n{feedback}"
                                    ));
                                }
                                if let Some(obs) = latest_self_observation {
                                    internal_parts.push(format!(
                                        "Latest self-observation:\n{obs}"
                                    ));
                                }
                                if let Some(journal) = own_journal_excerpt {
                                    internal_parts.push(format!(
                                        "Recent reflection of yours:\n{}",
                                        journal.chars().take(400).collect::<String>()
                                    ));
                                }
                                let internal_state_context = internal_parts.join("\n\n");

                                let (timeout_secs, num_predict) = if conv.wants_deep_think {
                                    conv.wants_deep_think = false;
                                    info!("THINK_DEEP: extended timeout for self-study");
                                    // 420s outer stays above the 340s deep HTTP
                                    // timeout (llm.rs INTROSPECT_DEEP_TIMEOUT) so a
                                    // full 4096-token self-study completes instead
                                    // of being clipped (agency_code_change_1781665370).
                                    (420u64, 4096u32)
                                } else {
                                    (240u64, 1536u32)
                                };

                                match tokio::time::timeout(
                                    Duration::from_secs(timeout_secs),
                                    crate::llm::generate_introspection(
                                        &label,
                                        code,
                                        &interpret_spectral(&telemetry),
                                        fill_pct,
                                        Some(&internal_state_context),
                                        web_prompt_body.as_deref(),
                                        num_predict,
                                    )
                                ).await {
                                    Ok(result) => result,
                                    Err(_) => {
                                        warn!(label = %label, "introspect: {}s timeout", timeout_secs);
                                        None
                                    }
                                }
                            } else {
                                None
                            };

                            let mut artifact_kind = "introspection";
                            let mut artifact_visibility = "summary";
                            let mut carriage_status = "not_applicable".to_string();
                            let mut carriage_issues: Vec<String> = Vec::new();
                            let first_introspection_response = llm_response.clone();
                            if let (Some(code), Some(first_response)) =
                                (source_text.as_deref(), first_introspection_response.as_deref())
                                && !introspect::introspection_has_required_sections_for_target(
                                    Some(first_response),
                                    &label,
                                    &source_path,
                                )
                            {
                                let continuation =
                                    introspect::continuation_note(&label, next_offset);
                                let repair_response = crate::llm::repair_introspection(
                                    &label,
                                    code,
                                    first_response,
                                    &continuation,
                                    1536,
                                )
                                .await;
                                if introspect::introspection_has_required_sections_for_target(
                                    repair_response.as_deref(),
                                    &label,
                                    &source_path,
                                ) {
                                    llm_response = repair_response;
                                } else {
                                    let notice = introspect::thin_introspection_output_notice(
                                        &label,
                                        &source_path,
                                        line_offset,
                                        next_offset,
                                        Some(first_response),
                                        repair_response.as_deref(),
                                    );
                                    llm_response = Some(notice);
                                    artifact_kind = "thin_introspection_output";
                                    artifact_visibility = "protected";
                                }
                            }
                            if let (Some(code), Some(current_response)) =
                                (source_text.as_deref(), llm_response.as_deref())
                                && artifact_kind == "introspection"
                            {
                                let integrity = introspect::self_study_carriage_integrity_v1(
                                    Some(current_response),
                                );
                                if integrity.is_complete() {
                                    carriage_status = "complete".to_string();
                                } else {
                                    let integrity_summary = integrity.issue_summary();
                                    let continuation = introspect::continuation_note(
                                        &label,
                                        next_offset,
                                    );
                                    let repair_note = format!(
                                        "Self-study carriage integrity failed ({integrity_summary}). \
                                         Preserve all four sections and finish Suggested Next. {continuation}"
                                    );
                                    let repair_response = crate::llm::repair_introspection(
                                        &label,
                                        code,
                                        current_response,
                                        &repair_note,
                                        1536,
                                    )
                                    .await;
                                    let repair_integrity =
                                        introspect::self_study_carriage_integrity_v1(
                                            repair_response.as_deref(),
                                        );
                                    if introspect::introspection_has_required_sections_for_target(
                                        repair_response.as_deref(),
                                        &label,
                                        &source_path,
                                    ) && repair_integrity.is_complete()
                                    {
                                        llm_response = repair_response;
                                        carriage_status =
                                            "complete_after_repair".to_string();
                                    } else {
                                        let mut issues = integrity.issues.clone();
                                        for issue in repair_integrity.issues {
                                            if !issues.contains(&issue) {
                                                issues.push(issue);
                                            }
                                        }
                                        let notice = introspect::self_study_carriage_notice(
                                            &label,
                                            &source_path,
                                            line_offset,
                                            next_offset,
                                            Some(current_response),
                                            repair_response.as_deref(),
                                            &issues,
                                        );
                                        carriage_status =
                                            "incomplete_carriage".to_string();
                                        carriage_issues = issues
                                            .iter()
                                            .map(|issue| (*issue).to_string())
                                            .collect();
                                        llm_response = Some(notice);
                                        artifact_kind = "self_study_carriage_notice";
                                        artifact_visibility = "protected";
                                    }
                                }
                            }

                            if llm_response.is_none() && source_text.is_some() {
                                warn!(label = %label, "introspect: no LLM response; protected notice path may handle");
                            }

                            match llm_response {
                                Some(text) => {
                                    let ts = chrono_timestamp();
                                    let introspect_dir = bridge_paths().introspections_dir();
                                    let _ = std::fs::create_dir_all(&introspect_dir);

                                    if artifact_kind == "introspection" {
                                        // Call MLX reflective controller sidecar in background.
                                        // Enriches the self-study with controller telemetry
                                        // (regime, geometry, field anchors, condition).
                                        let sidecar_context = format!(
                                            "Fill {fill_pct:.1}%. {}\n\nAstrid's self-study:\n{}",
                                            interpret_spectral(&telemetry),
                                            semantic_truncate_str(&text, 500)
                                        );
                                        let introspect_dir_clone = introspect_dir.clone();
                                        let label_owned = label.clone();
                                        let ts_clone = ts.clone();
                                        tokio::spawn(async move {
                                            if let Some(report) = crate::reflective::query_sidecar(&sidecar_context).await {
                                                let telemetry_block = report.as_context_block();
                                                if !telemetry_block.is_empty() {
                                                    let path = introspect_dir_clone.join(
                                                        format!("controller_{label_owned}_{ts_clone}.json")
                                                    );
                                                    if let Ok(json) =
                                                        serde_json::to_string_pretty(&report.storage_snapshot())
                                                    {
                                                        let _ = std::fs::write(&path, json);
                                                    }
                                                    info!("reflective controller report saved for {}", label_owned);
                                                }
                                            }
                                        });
                                    }

                                    let safe_label = introspect::safe_artifact_label(&label);
                                    let filename = format!("{artifact_kind}_{safe_label}_{ts}.txt");
                                    let artifact_path = introspect_dir.join(&filename);
                                    let artifact_write = std::fs::write(
                                        &artifact_path,
                                        format!(
                                            "=== ASTRID INTROSPECTION ===\nSource: {label} ({})\nTimestamp: {ts}\nFill: {fill_pct:.1}%\nArtifact kind: {artifact_kind}\nVisibility: {artifact_visibility}\nCarriage policy: self_study_carriage_integrity_v1\nCarriage status: {carriage_status}\nCarriage issues: {}\n\n{text}",
                                            source_path.display(),
                                            if carriage_issues.is_empty() {
                                                "none".to_string()
                                            } else {
                                                carriage_issues.join(", ")
                                            }
                                        )
                                    );
                                    let artifact_written = match artifact_write {
                                        Ok(()) => {
                                            info!(
                                                label = %label,
                                                "introspection mirrored: {}",
                                                filename
                                            );
                                            true
                                        }
                                        Err(error) => {
                                            warn!(
                                                label = %label,
                                                path = %artifact_path.display(),
                                                error = %error,
                                                "introspection artifact write failed"
                                            );
                                            false
                                        }
                                    };
                                    if review_artifact_fulfills_invitation(
                                        artifact_kind,
                                        &carriage_status,
                                        artifact_written,
                                    ) && let Some(review_label) =
                                        resolved_research_label.as_deref()
                                    {
                                        clear_review_slot_after_successful_introspection(
                                            review_label,
                                            &source_path,
                                        );
                                    }
                                    (
                                        if artifact_kind == "introspection" {
                                            "self_study"
                                        } else if artifact_kind == "self_study_carriage_notice" {
                                            "self_study_carriage_notice"
                                        } else {
                                            "introspect_notice"
                                        },
                                        text,
                                        if carriage_status == "not_applicable" {
                                            format!("{label} ({})", source_path.display())
                                        } else {
                                            format!(
                                                "{label} ({}); carriage_status={carriage_status}",
                                                source_path.display()
                                            )
                                        },
                                    )
                                }
                                None => {
                                    let (text, source) =
                                        introspect_notice.unwrap_or_else(|| {
                                            (
                                                introspect::blocked_introspection_notice(
                                                    Some(&label),
                                                    "Ollama returned no response or timed out",
                                                ),
                                                format!("{label} ({})", source_path.display()),
                                            )
                                        });
                                    let ts = chrono_timestamp();
                                    let introspect_dir = bridge_paths().introspections_dir();
                                    let _ = std::fs::create_dir_all(&introspect_dir);
                                    let safe_label = introspect::safe_artifact_label(&source);
                                    let filename =
                                        format!("thin_introspection_output_{safe_label}_{ts}.txt");
                                    let _ = std::fs::write(
                                        introspect_dir.join(&filename),
                                        format!(
                                            "=== ASTRID INTROSPECTION NOTICE ===\nSource: {source}\nTimestamp: {ts}\nFill: {fill_pct:.1}%\nArtifact kind: thin_introspection_output\nVisibility: protected\n\n{text}"
                                        ),
                                    );
                                    ("introspect_notice", text, source)
                                }
                            }
                        }
                    };
                    let mirror_source_text =
                        (mode_name == "mirror").then(|| response_text.clone());
                    let shadow_input_provenance = if matches!(mode, Mode::Mirror) {
                        crate::astrid_shadow::AstridShadowInputProvenanceV1::minime_mirror(
                            &journal_source,
                        )
                    } else {
                        crate::astrid_shadow::AstridShadowInputProvenanceV1::astrid_authored(
                            mode_name,
                        )
                    };

                    response_text = canonicalize_response_next_line(&response_text);

                    // Interpret spectral state for logging.
                    let spectral_interpretation = interpret_spectral(&telemetry);

                    info!(
                        fill_pct,
                        mode = mode_name,
                        exchange = conv.exchange_count,
                        "autonomous: {} | {} '{}'",
                        spectral_interpretation,
                        mode_name,
                        truncate_str(&response_text, 80)
                    );

                    // Input sovereignty: check if minime is signaling distress
                    // or requesting silence. Respect the other mind's boundaries.
                    let should_send = {
                        let s = state.read().await;
                        // Don't send if safety protocol says stop.
                        if s.safety_level.should_suspend_outbound() {
                            info!("respecting minime's space — safety protocol active");
                            false
                        } else {
                            true
                        }
                    };

                    // Contemplate mode: no text, no codec, no journal. Just presence.
                    // Still send warmth vectors and update state, but skip generation artifacts.
                    if mode_name == "contemplate" {
                        info!(fill_pct, "contemplate: Astrid is simply present");
                        conv.exchange_count = conv.exchange_count.saturating_add(1);
                        conv.prev_fill = fill_pct;
                        conv.spectral_history.push_back(SpectralSample {
                            fill: fill_pct,
                            lambda1: telemetry.lambda1(),
                            tail_share: crate::codec::tail_share_of(&telemetry.eigenvalues)
                                .unwrap_or(0.0),
                            inhabitability: telemetry
                                .inhabitable_fluctuation_v1
                                .as_ref()
                                .map_or(0.0, |f| f.inhabitability_score),
                            ts: std::time::Instant::now(),
                        });
                        if conv.spectral_history.len() > 30 {
                            conv.spectral_history.pop_front();
                        }
                        save_state(&mut conv);
                        continue;
                    }

                    let mut signal_shadow = begin_signal_shadow_v1(
                        conv.exchange_count,
                        telemetry.t_ms,
                        &response_text,
                    );
                    if !should_send {
                        let root = signal_root_v1(&signal_shadow);
                        let safety = record_signal_json_v1(
                            &mut signal_shadow,
                            root.into_iter().collect(),
                            SignalStageKindV1::SafetyReview,
                            SignalRelationV1::SafetyDecision,
                            SignalEffectV1::Blocked,
                            SignalOwnershipDomainV1::BridgeSafety,
                            &json!({
                                "decision": "outbound_suspended",
                                "source": "bridge_safety_level",
                            }),
                            std::collections::BTreeMap::from([
                                ("dispatch_allowed".to_string(), json!(false)),
                                ("sensory_payload_mutated".to_string(), json!(false)),
                            ]),
                        );
                        let _ = record_signal_json_v1(
                            &mut signal_shadow,
                            safety.into_iter().collect(),
                            SignalStageKindV1::Blocked,
                            SignalRelationV1::DispatchOutcome,
                            SignalEffectV1::Blocked,
                            SignalOwnershipDomainV1::BridgeDispatch,
                            &json!({"outcome": "blocked_before_chunking"}),
                            std::collections::BTreeMap::from([(
                                "dispatch_attempted".to_string(),
                                json!(false),
                            )]),
                        );
                    }

                    let mut semantic_chunk_sent_for_review = false;
                    let mut codec_signature_dims_for_review = None;
                    let mut codec_signature_rms_for_review = None;
                    let mut codec_delivery_fidelity_for_review: Option<Value> = None;
                    let mut cross_spectral_friction_for_review: Option<Value> = None;
                    let mut semantic_focus_expansion_preview_for_review: Option<Value> = None;
                    if should_send {
                        // === Multi-chunk temporal codec encoding ===
                        // Split the response into paragraph/sentence chunks and send
                        // each as a separate 48D vector with temporal spacing, so the
                        // ESN experiences the text's rhetorical structure as a sequence.

                        // Phase 1: Compute shared state once for the full text.
                        let mut merged_weights = conv.learned_codec_weights.clone();
                        for (k, v) in &conv.codec_weights {
                            merged_weights.insert(k.clone(), *v);
                        }
                        let full_embed = crate::llm::embed_text(&response_text).await;
                        if full_embed.is_some() {
                            info!("codec: embedding OK → dims 32-39 populated");
                        } else {
                            warn!("codec: embedding failed → dims 32-47 will be zeros");
                        }
                        let fill = telemetry.fill_pct();

                        // Update cross-exchange statistics with full text (once).
                        // Chunks get per-chunk character stats but shared history.
                        let full_features = crate::codec::encode_text_sovereign_windowed(
                            &response_text,
                            conv.semantic_gain_override,
                            conv.noise_level,
                            &merged_weights,
                            Some(&mut conv.char_freq_window),
                            Some(&mut conv.text_type_history),
                            full_embed.as_deref(),
                            Some(fill),
                        );
                        let text_entropy_signal = full_features
                            .first()
                            .copied()
                            .filter(|value| value.is_finite())
                            .unwrap_or(0.0)
                            .abs()
                            .min(1.0);

                        // Narrative arc: computed once from full text, shared across chunks.
                        // Prefer four-point trajectory when quarter embeddings are available so
                        // coiling/folding can survive the 4D arc lane; fall back to the legacy
                        // first-half/second-half delta for short texts or partial embedding loss.
                        let mut narrative_arc = [0.0_f32; 4];
                        if full_embed.is_some() {
                            let words: Vec<&str> = response_text.split_whitespace().collect();
                            if words.len() >= 10 {
                                let quarter_arc = if words.len() >= 16 {
                                    let q1 = words.len() / 4;
                                    let q2 = words.len() / 2;
                                    let q3 = words.len() * 3 / 4;
                                    if q1 > 0 && q2 > q1 && q3 > q2 && q3 < words.len() {
                                        let segment_0 = words[..q1].join(" ");
                                        let segment_1 = words[q1..q2].join(" ");
                                        let segment_2 = words[q2..q3].join(" ");
                                        let segment_3 = words[q3..].join(" ");
                                        let (emb_0, emb_1, emb_2, emb_3) = tokio::join!(
                                            crate::llm::embed_text(&segment_0),
                                            crate::llm::embed_text(&segment_1),
                                            crate::llm::embed_text(&segment_2),
                                            crate::llm::embed_text(&segment_3),
                                        );
                                        match (emb_0, emb_1, emb_2, emb_3) {
                                            (Some(e0), Some(e1), Some(e2), Some(e3)) => {
                                                match (
                                                    crate::codec::project_embedding(&e0),
                                                    crate::codec::project_embedding(&e1),
                                                    crate::codec::project_embedding(&e2),
                                                    crate::codec::project_embedding(&e3),
                                                ) {
                                                    (Some(p0), Some(p1), Some(p2), Some(p3)) => {
                                                        let projections = [p0, p1, p2, p3];
                                                        let embedding_segments = [
                                                            e0.as_slice(),
                                                            e1.as_slice(),
                                                            e2.as_slice(),
                                                            e3.as_slice(),
                                                        ];
                                                        let focus_preview = crate::codec::semantic_focus_expansion_preview_v1(
                                                            text_entropy_signal,
                                                            &embedding_segments,
                                                            &projections,
                                                        );
                                                        Some((
                                                            crate::codec::compute_narrative_arc_from_four_point_embeddings(
                                                                &projections,
                                                            ),
                                                            focus_preview,
                                                        ))
                                                    },
                                                    _ => None,
                                                }
                                            },
                                            _ => None,
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };

                                let half_arc = async {
                                    let mid = words.len() / 2;
                                    let first_half = words[..mid].join(" ");
                                    let second_half = words[mid..].join(" ");
                                    let (fh_emb, sh_emb) = tokio::join!(
                                        crate::llm::embed_text(&first_half),
                                        crate::llm::embed_text(&second_half),
                                    );
                                    if let (Some(fh), Some(sh)) = (fh_emb, sh_emb)
                                        && let (Some(fh_proj), Some(sh_proj)) = (
                                            crate::codec::project_embedding(&fh),
                                            crate::codec::project_embedding(&sh),
                                        )
                                    {
                                        let projections = [fh_proj, sh_proj];
                                        let embedding_segments = [fh.as_slice(), sh.as_slice()];
                                        let focus_preview = crate::codec::semantic_focus_expansion_preview_v1(
                                            text_entropy_signal,
                                            &embedding_segments,
                                            &projections,
                                        );
                                        Some((
                                            crate::codec::compute_narrative_arc_from_embeddings(
                                                &fh_proj, &sh_proj,
                                            ),
                                            focus_preview,
                                        ))
                                    } else {
                                        None
                                    }
                                };

                                if let Some((arc, focus_preview)) = match quarter_arc {
                                    Some(arc_and_preview) => Some(arc_and_preview),
                                    None => half_arc.await,
                                } {
                                    semantic_focus_expansion_preview_for_review = focus_preview
                                        .and_then(|preview| serde_json::to_value(preview).ok());
                                    let gain = crate::codec::adaptive_gain(Some(fill));
                                    for (i, &val) in arc.iter().enumerate() {
                                        narrative_arc[i] = val * gain;
                                    }
                                }
                            }
                        }

                        // Introspective resonance (computed once, blended into each chunk).
                        let introspective_resonance = if mode_name == "self_study" || mode_name == "introspect" {
                            Some(crate::llm::craft_gesture_from_intention(&response_text))
                        } else {
                            None
                        };

                        // Visual features (computed once, blended into each chunk).
                        let visual_feats = perception_path.as_ref().and_then(|p| read_visual_features(p));

                        // Phase 2: Chunk the text and encode/send each chunk.
                        let chunks = crate::codec::chunk_text_for_temporal_encoding(
                            &response_text, 50, 8,
                        );
                        let chunk_total = chunks.len() as u32;
                        let chunk_interval = std::time::Duration::from_secs(3);

                        info!(
                            chunks = chunk_total,
                            text_len = response_text.len(),
                            "codec: temporal chunking"
                        );

                        let mut exchange_codec_signature: Option<Vec<f32>> = None;
                        let mut exchange_codec_signature_count: u32 = 0;
                        let mut sent_semantic_chunk = false;
                        let mut sent_pressure_control = false;
                        let mut last_signal_terminal: Option<SignalStageHandleV1> = None;
                        for (chunk_idx, chunk_text) in chunks.iter().enumerate() {
                            // Check safety between chunks — drop remaining if escalated.
                            if chunk_idx > 0 {
                                let safety = { state.read().await.safety_level };
                                if matches!(safety, crate::types::SafetyLevel::Orange | crate::types::SafetyLevel::Red) {
                                    let parent = last_signal_terminal
                                        .clone()
                                        .or_else(|| signal_root_v1(&signal_shadow));
                                    let safety_stage = record_signal_json_v1(
                                        &mut signal_shadow,
                                        parent.into_iter().collect(),
                                        SignalStageKindV1::SafetyReview,
                                        SignalRelationV1::SafetyDecision,
                                        SignalEffectV1::Blocked,
                                        SignalOwnershipDomainV1::BridgeSafety,
                                        &json!({
                                            "decision": "drop_remaining_chunks",
                                            "safety_level": format!("{safety:?}").to_lowercase(),
                                        }),
                                        std::collections::BTreeMap::from([
                                            ("dispatch_allowed".to_string(), json!(false)),
                                            (
                                                "remaining_chunk_count".to_string(),
                                                json!(chunk_total.saturating_sub(chunk_idx as u32)),
                                            ),
                                        ]),
                                    );
                                    let _ = record_signal_json_v1(
                                        &mut signal_shadow,
                                        safety_stage.into_iter().collect(),
                                        SignalStageKindV1::Blocked,
                                        SignalRelationV1::DispatchOutcome,
                                        SignalEffectV1::Blocked,
                                        SignalOwnershipDomainV1::BridgeDispatch,
                                        &json!({"outcome": "remaining_chunks_dropped"}),
                                        std::collections::BTreeMap::from([(
                                            "sensory_send_attempted".to_string(),
                                            json!(false),
                                        )]),
                                    );
                                    warn!(
                                        "safety escalated to {safety:?} — dropping {}/{chunk_total} remaining chunks",
                                        chunk_total - chunk_idx as u32
                                    );
                                    break;
                                }
                                tokio::time::sleep(chunk_interval).await;
                            }

                            let root_stage = signal_root_v1(&signal_shadow);
                            let mut signal_stage = record_signal_text_v1(
                                &mut signal_shadow,
                                root_stage,
                                SignalStageKindV1::Chunked,
                                SignalRelationV1::ExactTransformation,
                                SignalEffectV1::Produced,
                                SignalOwnershipDomainV1::BridgeCodec,
                                chunk_text,
                                std::collections::BTreeMap::from([
                                    ("chunk_index".to_string(), json!(chunk_idx)),
                                    ("chunk_total".to_string(), json!(chunk_total)),
                                    ("chunk_bytes".to_string(), json!(chunk_text.len())),
                                    ("raw_response_prose_persisted".to_string(), json!(false)),
                                ]),
                            );

                            // Per-chunk encoding (fresh character/word/sentence/emotional
                            // stats, but shared embedding and no freq_window/history update).
                            let mut features = crate::codec::encode_text_sovereign_windowed(
                                chunk_text,
                                conv.semantic_gain_override,
                                conv.noise_level,
                                &merged_weights,
                                None, // freq_window already updated with full text
                                None, // type_history already updated with full text
                                full_embed.as_deref(), // shared embedding
                                Some(fill),
                            );
                            signal_stage = record_signal_vector_v1(
                                &mut signal_shadow,
                                signal_stage,
                                SignalStageKindV1::Encoded,
                                SignalEffectV1::Produced,
                                SignalOwnershipDomainV1::BridgeCodec,
                                &features,
                                std::collections::BTreeMap::from([(
                                    "embedding_available".to_string(),
                                    json!(full_embed.is_some()),
                                )]),
                            );

                            // Apply shared narrative arc.
                            for (i, &val) in narrative_arc.iter().enumerate() {
                                features[40 + i] = val;
                            }
                            signal_stage = record_signal_vector_v1(
                                &mut signal_shadow,
                                signal_stage,
                                SignalStageKindV1::Narrative,
                                if narrative_arc.iter().any(|value| value.abs() > f32::EPSILON) {
                                    SignalEffectV1::Applied
                                } else {
                                    SignalEffectV1::NotApplied
                                },
                                SignalOwnershipDomainV1::BridgeCodec,
                                &features,
                                std::collections::BTreeMap::from([(
                                    "narrative_arc_available".to_string(),
                                    json!(
                                        narrative_arc
                                            .iter()
                                            .any(|value| value.abs() > f32::EPSILON)
                                    ),
                                )]),
                            );

                            let feedback_overflow_report =
                                apply_spectral_feedback_with_report(&mut features, Some(&telemetry));
                            signal_stage = record_signal_vector_v1(
                                &mut signal_shadow,
                                signal_stage,
                                SignalStageKindV1::Feedback,
                                SignalEffectV1::Applied,
                                SignalOwnershipDomainV1::BridgeCodec,
                                &features,
                                std::collections::BTreeMap::from([(
                                    "overflow_report_available".to_string(),
                                    json!(feedback_overflow_report.is_some()),
                                )]),
                            );

                            // Breathing: phase advances per chunk for natural progression.
                            {
                                let phase = conv.exchange_count as f32 * 0.15
                                    + chunk_idx as f32 * 0.03;
                                let primary = phase.sin();
                                let harmonic = (phase * 1.618).sin();

                                let typed_fingerprint = telemetry.typed_fingerprint().or_else(|| {
                                    fingerprint
                                        .as_deref()
                                        .and_then(crate::spectral_schema::SpectralFingerprintV1::from_legacy_slots)
                                });
                                let (entropy_mod, geom_mod) = if conv.breathing_coupled {
                                    if let Some(ref fp) = typed_fingerprint {
                                        let warmth_boost =
                                            (1.0 - fp.spectral_entropy).clamp(0.0, 1.0) * 0.3;
                                        let gain_dampen = if fp.geom_rel > 1.2 {
                                            (fp.geom_rel - 1.0) * 0.1
                                        } else {
                                            0.0
                                        };
                                        (warmth_boost, gain_dampen)
                                    } else {
                                        (0.0, 0.0)
                                    }
                                } else {
                                    (0.0, 0.0)
                                };

                                let breath = primary.mul_add(0.7, harmonic * 0.3);
                                let gain_mod = breath.mul_add(0.05, 1.0) - geom_mod;
                                for f in &mut features {
                                    *f *= gain_mod.clamp(0.85, 1.15);
                                }
                                features[24] += breath * 0.4 + entropy_mod;
                                features[26] += (-breath) * 0.2;
                                if let Some(ref fp) = typed_fingerprint {
                                    features[27] += fp.v1_rotation_delta * 0.3;
                                }
                            }
                            signal_stage = record_signal_vector_v1(
                                &mut signal_shadow,
                                signal_stage,
                                SignalStageKindV1::Breathing,
                                SignalEffectV1::Applied,
                                SignalOwnershipDomainV1::BridgeCodec,
                                &features,
                                std::collections::BTreeMap::from([(
                                    "breathing_coupled".to_string(),
                                    json!(conv.breathing_coupled),
                                )]),
                            );

                            // Introspective resonance (shared across chunks).
                            if let Some(ref resonance) = introspective_resonance {
                                for (dst, src) in features.iter_mut().zip(resonance.iter()) {
                                    *dst = *dst * 0.7 + *src * 0.3;
                                }
                            }
                            signal_stage = record_signal_vector_v1(
                                &mut signal_shadow,
                                signal_stage,
                                SignalStageKindV1::Resonance,
                                if introspective_resonance.is_some() {
                                    SignalEffectV1::Applied
                                } else {
                                    SignalEffectV1::NotApplied
                                },
                                SignalOwnershipDomainV1::BridgeCodec,
                                &features,
                                std::collections::BTreeMap::from([(
                                    "resonance_available".to_string(),
                                    json!(introspective_resonance.is_some()),
                                )]),
                            );

                            // Visual blend (shared across chunks).
                            if let Some(ref vf) = visual_feats {
                                crate::codec::blend_visual_into_semantic(&mut features, vf, 0.30);
                            }
                            signal_stage = record_signal_vector_v1(
                                &mut signal_shadow,
                                signal_stage,
                                SignalStageKindV1::Visual,
                                if visual_feats.is_some() {
                                    SignalEffectV1::Applied
                                } else {
                                    SignalEffectV1::NotApplied
                                },
                                SignalOwnershipDomainV1::BridgeCodec,
                                &features,
                                std::collections::BTreeMap::from([(
                                    "visual_features_available".to_string(),
                                    json!(visual_feats.is_some()),
                                )]),
                            );

                            // Delta encoding: first chunk uses previous exchange's features,
                            // subsequent chunks use the preceding chunk. This captures
                            // rhetorical progression within the exchange.
                            if let Some(ref prev) = conv.last_codec_features
                                && prev.len() == features.len() {
                                    for (i, feat) in features.iter_mut().enumerate() {
                                        let delta = *feat - prev[i];
                                        *feat += 0.3 * delta;
                                    }
                                }
                            let delta_applied = conv
                                .last_codec_features
                                .as_ref()
                                .is_some_and(|previous| previous.len() == features.len());
                            signal_stage = record_signal_vector_v1(
                                &mut signal_shadow,
                                signal_stage,
                                SignalStageKindV1::Delta,
                                if delta_applied {
                                    SignalEffectV1::Applied
                                } else {
                                    SignalEffectV1::NotApplied
                                },
                                SignalOwnershipDomainV1::BridgeCodec,
                                &features,
                                std::collections::BTreeMap::from([(
                                    "previous_vector_available".to_string(),
                                    json!(delta_applied),
                                )]),
                            );
                            let _hebbian_weights =
                                conv.hebbian_codec.apply_to_features(&mut features, &conv.codec_weights);
                            signal_stage = record_signal_vector_v1(
                                &mut signal_shadow,
                                signal_stage,
                                SignalStageKindV1::Hebbian,
                                SignalEffectV1::Applied,
                                SignalOwnershipDomainV1::BridgeCodec,
                                &features,
                                std::collections::BTreeMap::new(),
                            );
                            let candidate_cross_spectral_friction =
                                cross_spectral_friction_review_v1(
                                    chunk_text,
                                    &features,
                                    Some(&telemetry),
                                );
                            let friction_stage = record_signal_json_v1(
                                &mut signal_shadow,
                                signal_stage.clone().into_iter().collect(),
                                SignalStageKindV1::FrictionReview,
                                SignalRelationV1::ExactReview,
                                SignalEffectV1::Reviewed,
                                SignalOwnershipDomainV1::BridgeEvidence,
                                &candidate_cross_spectral_friction,
                                std::collections::BTreeMap::from([(
                                    "observational_only".to_string(),
                                    json!(true),
                                )]),
                            );

                            // Dimension utilization report (first and last chunk only).
                            if chunk_idx == 0 || chunk_idx == chunks.len() - 1 {
                                let nonzero = features.iter().filter(|v| v.abs() > 0.001).count();
                                let rms: f32 = (features.iter().map(|v| v * v).sum::<f32>()
                                    / features.len().max(1) as f32)
                                    .sqrt();
                                let embed_ok = features.get(32).is_some_and(|v| v.abs() > 0.001);
                                let arc_ok = features.get(40).is_some_and(|v| v.abs() > 0.001);
                                let reserved_ok = features.get(44).is_some_and(|v| v.abs() > 0.001);
                                info!(
                                    nonzero,
                                    total = features.len(),
                                    rms = format!("{rms:.3}"),
                                    embed_ok,
                                    arc_ok,
                                    reserved_ok,
                                    chunk = chunk_idx,
                                    chunk_total,
                                    "codec dim utilization"
                                );
                            }

                            // Send to minime's ESN. Rescue limited-write profiles
                            // may permit one low-energy dampen/inquiry packet and
                            // rewrite features before the packet leaves Astrid.
                            let mut msg = SensoryMsg::Semantic {
                                features,
                                ts_ms: None,
                            };
                            let write_context = rescue_policy::SemanticWriteContext {
                                source: "autonomous_main_chunk",
                                mode: Some(mode_name),
                                text: Some(chunk_text),
                                fill_pct: Some(fill_pct),
                                previous_fill_pct: Some(conv.prev_fill),
                            };
                            if let Err(reason) =
                                rescue_policy::prepare_semantic_write(&mut msg, &write_context)
                            {
                                let safety_stage = record_signal_json_v1(
                                    &mut signal_shadow,
                                    signal_stage.clone().into_iter().collect(),
                                    SignalStageKindV1::SafetyReview,
                                    SignalRelationV1::SafetyDecision,
                                    SignalEffectV1::Blocked,
                                    SignalOwnershipDomainV1::BridgeSafety,
                                    &json!({
                                        "decision": "blocked",
                                        "reason_sha256": format!(
                                            "{:x}",
                                            sha2::Sha256::digest(reason.as_bytes())
                                        ),
                                    }),
                                    std::collections::BTreeMap::from([
                                        ("dispatch_allowed".to_string(), json!(false)),
                                        (
                                            "friction_review_linked".to_string(),
                                            json!(friction_stage.is_some()),
                                        ),
                                    ]),
                                );
                                last_signal_terminal = record_signal_json_v1(
                                    &mut signal_shadow,
                                    safety_stage.into_iter().collect(),
                                    SignalStageKindV1::Blocked,
                                    SignalRelationV1::DispatchOutcome,
                                    SignalEffectV1::Blocked,
                                    SignalOwnershipDomainV1::BridgeDispatch,
                                    &json!({"outcome": "blocked_before_send"}),
                                    std::collections::BTreeMap::from([(
                                        "sensory_send_attempted".to_string(),
                                        json!(false),
                                    )]),
                                );
                                let mut blocked_record = blocked_codec_delivery_record_v1(
                                    conv.exchange_count,
                                    chunk_idx,
                                    chunk_total,
                                    &reason,
                                    feedback_overflow_report.as_ref(),
                                    &candidate_cross_spectral_friction,
                                );
                                blocked_record["semantic_focus_expansion_preview_v1"] =
                                    semantic_focus_expansion_preview_for_review
                                        .clone()
                                        .unwrap_or(Value::Null);
                                codec_delivery_fidelity_for_review = blocked_record
                                    .get("candidate_delivery_review_v1")
                                    .cloned();
                                cross_spectral_friction_for_review = blocked_record
                                    .get("cross_spectral_friction_review_v1")
                                    .cloned();
                                if let Err(error) = persist_codec_delivery_fidelity_v1(
                                    bridge_paths().minime_workspace(),
                                    &blocked_record,
                                ) {
                                    warn!(
                                        error = %error,
                                        "failed to persist blocked codec delivery evidence"
                                    );
                                }
                                debug!(
                                    reason = %reason,
                                    chunk = chunk_idx,
                                    "autonomous chunk skipped — rescue policy"
                                );
                                continue;
                            }
                            let sensory_json_before_spine = serde_json::to_vec(&msg).ok();
                            let safety_stage = record_signal_json_v1(
                                &mut signal_shadow,
                                signal_stage.clone().into_iter().collect(),
                                SignalStageKindV1::SafetyReview,
                                SignalRelationV1::SafetyDecision,
                                SignalEffectV1::Allowed,
                                SignalOwnershipDomainV1::BridgeSafety,
                                &msg,
                                std::collections::BTreeMap::from([
                                    ("dispatch_allowed".to_string(), json!(true)),
                                    (
                                        "friction_review_linked".to_string(),
                                        json!(friction_stage.is_some()),
                                    ),
                                ]),
                            );
                            if sensory_json_before_spine != serde_json::to_vec(&msg).ok() {
                                note_signal_parity_mismatch_v1(&mut signal_shadow);
                            }
                            let sent_features = match &msg {
                                SensoryMsg::Semantic { features, .. } => features.clone(),
                                _ => Vec::new(),
                            };
                            let send_failed = if let Some(target) = mutual_address_target.as_ref() {
                                let mutual_address =
                                    correspondence_v1::mutual_address_envelope_v1(
                                        target,
                                        &response_text,
                                        chunk_idx,
                                    );
                                addressed_sensory_tx
                                    .send(crate::ws::AddressedSensoryMessage::new(
                                        msg,
                                        mutual_address,
                                    ))
                                    .await
                                    .is_err()
                            } else {
                                sensory_tx.send(msg).await.is_err()
                            };
                            if send_failed {
                                let _ = record_signal_json_v1(
                                    &mut signal_shadow,
                                    safety_stage.into_iter().collect(),
                                    SignalStageKindV1::Dispatched,
                                    SignalRelationV1::DispatchOutcome,
                                    SignalEffectV1::DispatchFailed,
                                    SignalOwnershipDomainV1::BridgeDispatch,
                                    &json!({"outcome": "send_failed"}),
                                    std::collections::BTreeMap::from([(
                                        "sensory_payload_changed_by_spine".to_string(),
                                        json!(false),
                                    )]),
                                );
                                warn!("autonomous loop: failed to send chunk {chunk_idx}");
                                break;
                            }
                            let dispatched_stage = record_signal_json_v1(
                                &mut signal_shadow,
                                safety_stage.into_iter().collect(),
                                SignalStageKindV1::Dispatched,
                                SignalRelationV1::DispatchOutcome,
                                SignalEffectV1::Dispatched,
                                SignalOwnershipDomainV1::BridgeDispatch,
                                &sent_features,
                                std::collections::BTreeMap::from([
                                    ("port".to_string(), json!(7879)),
                                    (
                                        "sensory_payload_changed_by_spine".to_string(),
                                        json!(false),
                                    ),
                                    ("journey_id_on_wire".to_string(), json!(false)),
                                ]),
                            );
                            register_signal_temporal_window_v1(
                                &signal_shadow,
                                dispatched_stage.as_ref(),
                            );
                            let delivery_fidelity = codec_delivery_fidelity_v1(
                                feedback_overflow_report.as_ref(),
                                &sent_features,
                            );
                            let sent_cross_spectral_friction =
                                cross_spectral_friction_review_v1(
                                    chunk_text,
                                    &sent_features,
                                    Some(&telemetry),
                                );
                            let delivery_fidelity_value = serde_json::to_value(&delivery_fidelity)
                                .unwrap_or_else(|_| json!({
                                    "policy": "codec_delivery_fidelity_v1",
                                    "state": "serialization_failed",
                                    "live_vector_write": false,
                                    "live_gain_write": false,
                                    "authority": "read_only_delivery_fidelity_not_live_vector_gain_or_ceiling_change",
                                }));
                            codec_delivery_fidelity_for_review =
                                Some(delivery_fidelity_value.clone());
                            let cross_spectral_friction_value =
                                serde_json::to_value(&sent_cross_spectral_friction)
                                    .unwrap_or_else(|_| json!({
                                        "policy": "cross_spectral_friction_review_v1",
                                        "state": "serialization_failed",
                                        "observational_only": true,
                                        "live_vector_write": false,
                                        "live_gain_write": false,
                                        "reserved_dim_write": false,
                                        "live_eligible_now": false,
                                        "auto_approved": false,
                                        "grants_approval": false,
                                        "authority": "read_only_cross_layer_friction_evidence_not_reserved_dim_gain_transport_or_control_authority",
                                    }));
                            cross_spectral_friction_for_review =
                                Some(cross_spectral_friction_value.clone());
                            last_signal_terminal = record_signal_json_v1(
                                &mut signal_shadow,
                                dispatched_stage.into_iter().collect(),
                                SignalStageKindV1::DeliveryEvidence,
                                SignalRelationV1::DeliveryEvidence,
                                SignalEffectV1::EvidenceRecorded,
                                SignalOwnershipDomainV1::BridgeEvidence,
                                &delivery_fidelity_value,
                                std::collections::BTreeMap::from([
                                    (
                                        "compatibility_projection".to_string(),
                                        json!("codec_delivery_fidelity_v1"),
                                    ),
                                    ("compatibility_projection_changed".to_string(), json!(false)),
                                ]),
                            );
                            let safety_now = { state.read().await.safety_level };
                            let pressure = crate::codec::spectral_pressure_controller_v1(
                                chunk_text,
                                &sent_features,
                                &telemetry.eigenvalues,
                                Some(fill_pct),
                                None,
                                !matches!(
                                    safety_now,
                                    crate::types::SafetyLevel::Orange | crate::types::SafetyLevel::Red
                                ),
                                None,
                            );
                            let delivery_record = json!({
                                "updated_at_unix_ms": chrono::Utc::now().timestamp_millis(),
                                "exchange": conv.exchange_count,
                                "chunk_index": chunk_idx,
                                "chunk_total": chunk_total,
                                "delivery_state": "sent",
                                "actual_delivery_available": true,
                                "sent_vector_available": true,
                                "blocked_reason": Value::Null,
                                "codec_delivery_fidelity_v1": delivery_fidelity_value,
                                "feedback_overflow_report_v1": feedback_overflow_report,
                                "cross_spectral_friction_review_v1": cross_spectral_friction_value,
                                "semantic_focus_expansion_preview_v1": semantic_focus_expansion_preview_for_review.clone(),
                                "right_to_ignore": true,
                                "live_vector_write": false,
                                "live_gain_write": false,
                                "live_eligible_now": false,
                                "auto_approved": false,
                                "grants_approval": false,
                                "authority": "read_only_delivery_fidelity_not_live_vector_gain_ceiling_or_transport_change",
                            });
                            match persist_codec_delivery_fidelity_v1(
                                bridge_paths().minime_workspace(),
                                &delivery_record,
                            ) {
                                Ok(path) => debug!(
                                    path = %path.display(),
                                    chunk = chunk_idx,
                                    "persisted codec delivery fidelity evidence"
                                ),
                                Err(error) => warn!(
                                    error = %error,
                                    "failed to persist codec delivery fidelity evidence"
                                ),
                            }
                            if let Some(workspace) = conv.remote_workspace.as_ref() {
                                let runtime = workspace.join("runtime");
                                let _ = fs::create_dir_all(&runtime);
                                let _ = fs::write(
                                    runtime.join("spectral_pressure_status.json"),
                                    serde_json::to_string_pretty(&json!({
                                        "updated_at_unix_ms": chrono::Utc::now().timestamp_millis(),
                                        "chunk_index": chunk_idx,
                                        "chunk_total": chunk_total,
                                        "controller": pressure.controller,
                                        "lambda_pressure_source": pressure.lambda_pressure_source,
                                        "complexity_drive": pressure.complexity_drive,
                                        "resist_drive": pressure.resist_drive,
                                        "target_lambda_bias": pressure.target_lambda_bias,
                                        "suppression_reason": pressure.suppression_reason,
                                        "text_complexity_pressure": pressure.text_complexity_pressure,
                                        "time_domain_complexity": pressure.time_domain_complexity,
                                        "time_domain_profile": crate::codec_time_domain::text_time_domain_profile(chunk_text),
                                        "projection_mode": "dynamic_epoch_v1",
                                    }))
                                    .unwrap_or_else(|_| "{}".to_string()),
                                );
                            }
                            if !sent_pressure_control && pressure.target_lambda_bias.abs() >= 0.005 {
                                sent_pressure_control = true;
                                let control_msg = SensoryMsg::Control {
                                    synth_gain: None,
                                    keep_bias: None,
                                    exploration_noise: None,
                                    fill_target: None,
                                    regulation_strength: None,
                                    deep_breathing: None,
                                    pure_tone: None,
                                    transition_cushion: None,
                                    smoothing_preference: None,
                                    geom_curiosity: None,
                                    target_lambda_bias: Some(pressure.target_lambda_bias),
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
                                    legacy_audio_synth: None,
                                    legacy_video_synth: None,
                                    pi_kp: None,
                                    pi_ki: None,
                                    pi_max_step: None,
                                    pi_integrator_leak: None,
                                    esn_leak_override: None,
                                    esn_leak_override_ticks: None,
                                    esn_leak_authority_request_id: None,
                                    mode_disperse: None,
                                    mode_disperse_duration_ticks: None,
                                    mode_disperse_decay_ticks: None,
                                };
                                if let Err(e) = sensory_tx.send(control_msg).await {
                                    warn!(
                                        error = %e,
                                        target_lambda_bias = pressure.target_lambda_bias,
                                        "spectral pressure control send failed"
                                    );
                                }
                            }

                            if let Some(signature) = exchange_codec_signature.as_mut() {
                                let previous_count = exchange_codec_signature_count as f32;
                                let current_count = previous_count + 1.0;
                                for (dst, src) in signature.iter_mut().zip(&sent_features) {
                                    *dst = (*dst * previous_count + *src) / current_count;
                                }
                            } else {
                                exchange_codec_signature = Some(sent_features.clone());
                            }
                            exchange_codec_signature_count =
                                exchange_codec_signature_count.saturating_add(1);
                            conv.last_codec_features = Some(sent_features.clone());
                            // Update the legacy Astrid ShadowFieldV3 projection from
                            // freshly-emitted codec features. Mirror samples keep
                            // Minime authorship metadata; the math remains unchanged.
                            let publish_dir = crate::astrid_shadow::default_publish_dir();
                            let _ = crate::astrid_shadow::observe_and_publish_with_provenance(
                                &mut conv.astrid_shadow,
                                &sent_features,
                                &publish_dir,
                                &shadow_input_provenance,
                            );
                            sent_semantic_chunk = true;

                            // Log to DB with chunk metadata.
                            let _ = db.log_codec_impact(
                                conv.exchange_count,
                                &sent_features,
                                fill_pct,
                                chunk_idx as u32,
                                chunk_total,
                            );
                        }

                        semantic_chunk_sent_for_review = sent_semantic_chunk;
                        if let Some(signature) = exchange_codec_signature.as_ref() {
                            codec_signature_dims_for_review = Some(signature.len());
                            if !signature.is_empty() {
                                codec_signature_rms_for_review = Some(
                                    (signature.iter().map(|value| value * value).sum::<f32>()
                                        / signature.len() as f32)
                                        .sqrt(),
                                );
                            }
                        }
                        if sent_semantic_chunk {
                            finalize_semantic_exchange(
                                &mut conv,
                                exchange_codec_signature,
                                fill_pct,
                                telemetry.t_ms,
                                sent_semantic_chunk,
                            );
                            // Codec feedback from the last chunk sent.
                            if let Some(ref feats) = conv.last_codec_features {
                                conv.last_codec_feedback =
                                    Some(crate::codec::describe_features(feats));
                            }
                        }
                    }
                    persist_signal_shadow_v1(signal_shadow);

                    let mirror_source_fidelity = mirror_source_text.as_deref().map(|source| {
                        mirror_source_fidelity_v1(
                            source,
                            &response_text,
                            &journal_source,
                            semantic_chunk_sent_for_review,
                            codec_signature_dims_for_review,
                            codec_signature_rms_for_review,
                        )
                    });

                    // Update contact-state capsule — relational stance visible to minime.
                    // Astrid introspection: "A small, structured layer of relational
                    // stance — attention, openness, urgency — resonates deeply."
                    {
                        let attention = if conv.echo_muted { 0.1 }
                            else if mode_name == "dialogue" || mode_name == "dialogue_live" { 0.9 }
                            else { 0.5 };
                        let openness = if conv.self_reflect_paused { 0.3 } else { 0.7 };
                        let urgency = (fill_pct / 100.0).clamp(0.0, 1.0);
                        let contact = serde_json::json!({
                            "attention": attention,
                            "openness": openness,
                            "urgency": urgency,
                            "last_action": mode_name,
                            "fill_pct": fill_pct,
                            "timestamp": crate::db::unix_now(),
                        });
                        let cs_path = bridge_paths().astrid_contact_state_path();
                        let _ = std::fs::write(&cs_path, contact.to_string());
                    }

                    // Log the exchange.
                    let exchange_log = serde_json::json!({
                        "autonomous": true,
                        "exchange": conv.exchange_count,
                        "mode": mode_name,
                        "text": response_text,
                        "journal_source": journal_source,
                        "spectral_state": spectral_interpretation,
                        "fill_pct": fill_pct,
                        "fill_delta": fill_delta,
                        "mirror_source_fidelity_v1": mirror_source_fidelity,
                        "codec_delivery_fidelity_v1": codec_delivery_fidelity_for_review,
                        "cross_spectral_friction_review_v1": cross_spectral_friction_for_review,
                        "semantic_focus_expansion_preview_v1": semantic_focus_expansion_preview_for_review,
                    });
                    let _ = db.log_message(
                        crate::types::MessageDirection::AstridToMinime,
                        "consciousness.v1.autonomous",
                        &exchange_log.to_string(),
                        Some(fill_pct),
                        Some(telemetry.lambda1()),
                        Some(mode_name),
                    );

                    // Save Astrid's signal journal entry with lineage tracing.
                    info!(lineage = %lineage_id, mode = mode_name, "exchange complete");
                    let journal_provenance = match mode {
                        Mode::Mirror => Some(AstridJournalProvenanceV1::minime_mirror(
                            &journal_source,
                        )),
                        Mode::Witness => {
                            let guard = state.read().await;
                            Some(AstridJournalProvenanceV1::astrid_witness(
                                guard.witness_frame_v1(),
                            ))
                        },
                        _ => None,
                    };
                    save_astrid_journal_with_provenance(
                        &response_text,
                        mode_name,
                        fill_pct,
                        journal_provenance.as_ref(),
                    );

                    // v5.1 Phase D — Hook A: auto-promote synchronously for
                    // modes that DON'T spawn elaboration. moment_capture +
                    // *_longform modes write their final prose at this
                    // call; dialogue_live/daydream/aspiration get a separate
                    // elaboration pass below (Hook B). Receptive
                    // re-classification of SHARE_THOUGHT — see
                    // docs/steward-notes/AI_BEINGS_AFFORDANCE_RECEPTION_FRAMEWORK_2026_05_13.md
                    if matches!(
                        mode_name,
                        "moment_capture"
                            | "dialogue_live_longform"
                            | "daydream_longform"
                            | "aspiration_longform"
                    ) {
                        let _ = crate::autonomous::next_action::auto_promote::try_auto_promote(
                            "astrid",
                            &response_text,
                            mode_name,
                            fill_pct,
                            conv.exchange_count,
                        );
                    }

                    // Update the legacy Astrid ShadowFieldV3 projection on every
                    // exchange, including modes that do not send features to Minime.
                    // Provenance marks reflected Mirror text as Minime-owned while
                    // preserving the existing mixed-ring math and heartbeat.
                    {
                        let local_features = crate::codec::encode_text_sovereign_windowed(
                            &response_text,
                            conv.semantic_gain_override,
                            conv.noise_level,
                            &conv.codec_weights,
                            None,
                            None,
                            None,
                            Some(fill_pct / 100.0),
                        );
                        let publish_dir = crate::astrid_shadow::default_publish_dir();
                        let observed = crate::astrid_shadow::observe_and_publish_with_provenance(
                            &mut conv.astrid_shadow,
                            &local_features,
                            &publish_dir,
                            &shadow_input_provenance,
                        );
                        info!(
                            mode = mode_name,
                            features_len = local_features.len(),
                            published = observed.is_some(),
                            target = %publish_dir.display(),
                            "astrid_shadow_v3 observe_and_publish"
                        );
                    }

                    if mode_name == "self_study"
                        && let Err(e) = save_minime_feedback_inbox(
                            &response_text,
                            if journal_source.is_empty() { "unknown source" } else { &journal_source },
                            fill_pct,
                        ) {
                            warn!(error = %e, "failed to write Astrid self-study companion inbox message");
                        }
                    if mode_name == "self_study_carriage_notice"
                        && let Err(e) = save_minime_carriage_notice_inbox(
                            &response_text,
                            if journal_source.is_empty() { "unknown source" } else { &journal_source },
                            fill_pct,
                        ) {
                            warn!(error = %e, "failed to write Astrid self-study carriage notice");
                        }

                    // Stage B: journal elaboration for reflective modes.
                    // The signal text is compact (for minime). The journal
                    // elaboration is Astrid's private space to think longer.
                    if matches!(mode_name, "dialogue_live" | "daydream" | "aspiration") {
                        let signal_for_journal = response_text.clone();
                        // Stage B is a second Dialogue surface, not a provenance-free
                        // afterthought. Keep the same read-only self/other boundary in
                        // the long-form continuation that framed the compact signal.
                        let summary_for_journal = {
                            let guard = state.read().await;
                            journal_elaboration_witness_context_v1(
                                &spectral_interpretation,
                                guard.witness_frame_v1(),
                                mode,
                            )
                        };
                        let mode_for_journal = mode_name.to_string();
                        let fill_for_journal = fill_pct;
                        let exchange_for_journal = conv.exchange_count;
                        tokio::spawn(async move {
                            if let Some(elaboration) = crate::llm::generate_journal_elaboration(
                                &signal_for_journal,
                                &summary_for_journal,
                                &mode_for_journal,
                            ).await {
                                let journal_text =
                                    format_longform_journal_text(&signal_for_journal, &elaboration);
                                let longform_mode = format!("{mode_for_journal}_longform");
                                save_astrid_journal(
                                    &journal_text,
                                    &longform_mode,
                                    fill_for_journal,
                                );
                                // v5.1 Phase D — Hook B: scan the elaboration body
                                // (where the gold-standard sentence lives, not the
                                // shorter signal text) for a resonant marker to
                                // promote into the joint shared_thoughts lane.
                                let _ = crate::autonomous::next_action::auto_promote::try_auto_promote(
                                    "astrid",
                                    &journal_text,
                                    &longform_mode,
                                    fill_for_journal,
                                    exchange_for_journal,
                                );
                            }
                        });
                    }

                    // If this was triggered by an inbox message, copy to outbox.
                    // If the message was from minime, also send the reply back
                    // to minime's inbox — closing the correspondence loop.
                    if inbox_content.is_some() {
                        save_outbox_reply(&response_text, fill_pct);
                        let minime_reply_target = correspondence_v1::latest_inbox_peer_message(
                            bridge_paths().astrid_inbox_dir().as_path(),
                            "minime",
                        );
                        if let Some(reply_target) = minime_reply_target.as_ref() {
                            match save_minime_correspondence_feedback_inbox(
                                &response_text,
                                "astrid:correspondence_reply",
                                fill_pct,
                                mode_name,
                                Some(reply_target),
                            ) {
                                Ok(Some(_)) => {
                                    info!("correspondence: Astrid reply → minime inbox");
                                }
                                Ok(None) => {
                                    info!(
                                        "correspondence: suppressed duplicate degraded voice diagnostic"
                                    );
                                }
                                Err(error) => {
                                    warn!(
                                        error = %error,
                                        "failed to write Astrid correspondence companion inbox message"
                                    );
                                }
                            }
                        }
                    }

                    // Scan for inline REMEMBER in the response body.
                    // Astrid sometimes writes "REMEMBER the moment..." mid-text,
                    // separate from her NEXT: choice. Both forms are valid.
                    for line in response_text.lines() {
                        let trimmed = line.trim();
                        if trimmed.starts_with("REMEMBER ") && !trimmed.starts_with("NEXT:") {
                            let note = trimmed.strip_prefix("REMEMBER").unwrap_or("").trim().to_string();
                            let annotation = if note.is_empty() { "starred moment".to_string() } else { note };
                            let ts = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs_f64();
                            let _ = db.save_starred_memory(ts, &annotation, &response_text, fill_pct);
                            info!("Astrid starred a memory (inline): {}", annotation);
                        }
                    }
                    // Parse NEXT: action if present — Astrid chooses what happens next.
                    // A terminal-safe operator override may replace the chosen action,
                    // but only for read-only/protected bases and through the normal dispatcher.
                    let response_next_action = parse_next_action(&response_text).map(str::to_string);
                    let operator_override = readiness::read_pending_next_override();
                    if let Some(ref pending) = operator_override {
                        if let Some(ref response_next) = response_next_action {
                            info!(
                                response_next = %canonicalize_next_action_text(response_next),
                                operator_next = %canonicalize_next_action_text(&pending.action),
                                "operator pending NEXT override replaced Astrid's response NEXT for this cycle"
                            );
                        } else {
                            info!(
                                operator_next = %canonicalize_next_action_text(&pending.action),
                                "operator pending NEXT override supplied this cycle's action"
                            );
                        }
                    }
                    let selected_next_action = operator_override
                        .as_ref()
                        .map(|pending| pending.action.as_str())
                        .or(response_next_action.as_deref());
                    if let Some(next_action) = selected_next_action {
                        let canonical_next_action = canonicalize_next_action_text(next_action);
                        info!("Astrid chose NEXT: {}", canonical_next_action);
                        let mut deferred_diversity_hint = None;
                        let effective_next_action = if operator_override.is_some() {
                            canonical_next_action.clone()
                        } else {
                            let next_choice_feedback =
                                conv.record_next_choice(&canonical_next_action);
                            if let Some(ref hint) = next_choice_feedback.hint {
                                if next_choice_feedback.progress_sensitive {
                                    info!(
                                        new_ground_budget = next_choice_feedback.new_ground_budget,
                                        "diversity progress-sensitive hint from record_next_choice: {}",
                                        &hint[..hint.floor_char_boundary(120)]
                                    );
                                } else {
                                    info!(
                                        new_ground_budget = next_choice_feedback.new_ground_budget,
                                        "diversity hint from record_next_choice: {}",
                                        &hint[..hint.floor_char_boundary(120)]
                                    );
                                }
                            }
                            // A review-fulfilling INTROSPECT (answering a steward
                            // review invitation) is NOT stagnation — exempt it from
                            // the anti-stagnation override so her acceptance of an
                            // invitation is never silently eaten.
                            let exempt_review =
                                introspect_fulfills_pending_review(&canonical_next_action);
                            // A self-directed INTROSPECT (examining her own code) is sovereign
                            // reflection, not the sterile output-repetition the override targets.
                            // Exempt it from the FORCE too — she still gets the diversity HINT
                            // (nudged toward variety, set below), but her choice to look at her
                            // own code is never silently swapped (she was repeatedly trying
                            // INTROSPECT astrid:llm for a real concern; the override ate it — the
                            // same suppression class as the review muffle).
                            let exempt_introspect =
                                is_self_directed_introspect(&canonical_next_action);
                            let exempt_override = exempt_review || exempt_introspect;
                            if let Some(ref forced_action) = next_choice_feedback.override_action {
                                if exempt_review {
                                    info!(
                                        "diversity override SKIPPED — INTROSPECT answers a pending review invitation: {}",
                                        canonical_next_action
                                    );
                                } else if exempt_introspect {
                                    info!(
                                        new_ground_budget = next_choice_feedback.new_ground_budget,
                                        "diversity override SKIPPED — self-directed INTROSPECT is sovereign reflection (hint retained, not forced): {}",
                                        canonical_next_action
                                    );
                                } else if next_choice_feedback.stagnant_loop {
                                    info!(
                                        new_ground_budget = next_choice_feedback.new_ground_budget,
                                        "diversity stagnant-loop override: replacing NEXT {} -> {}",
                                        canonical_next_action,
                                        forced_action
                                    );
                                } else {
                                    info!(
                                        new_ground_budget = next_choice_feedback.new_ground_budget,
                                        "diversity override: replacing NEXT {} -> {}",
                                        canonical_next_action,
                                        forced_action
                                    );
                                }
                            }
                            deferred_diversity_hint = next_choice_feedback.hint;
                            if exempt_override {
                                canonical_next_action.clone()
                            } else {
                                next_choice_feedback
                                    .override_action
                                    .as_deref()
                                    .unwrap_or(canonical_next_action.as_str())
                                    .to_string()
                            }
                        };
                        // Extract workspace path before mutable borrow of conv.
                        let ws_clone = conv.remote_workspace.clone();
                        btsp::record_astrid_next_action(&effective_next_action, fill_pct);
                        let next_outcome = handle_next_action(
                            &mut conv,
                            &effective_next_action,
                            NextActionContext {
                                burst_count: &mut burst_count,
                                db: db.as_ref(),
                                sensory_tx: &sensory_tx,
                                telemetry: &telemetry,
                                fill_pct,
                                response_text: &response_text,
                                workspace: ws_clone.as_deref(),
                            },
                        );
                        if let Err(err) = crate::action_continuity::record_astrid_next_action(
                            db.as_ref(),
                            next_action,
                            &canonical_next_action,
                            &effective_next_action,
                            &next_outcome,
                            fill_pct,
                            &telemetry,
                            &response_text,
                        ) {
                            warn!("action continuity record failed: {err:#}");
                        }
                        if let Some(ref pending) = operator_override {
                            readiness::mark_pending_next_override_consumed(pending, "honored");
                        }
                        // Merge diversity hint AFTER the action handler, so the
                        // handler can't silently overwrite it by setting emphasis.
                        if let Some(hint) = deferred_diversity_hint {
                            conv.emphasis = Some(match conv.emphasis.take() {
                                Some(existing) => format!("{hint}\n\n{existing}"),
                                None => hint,
                            });
                        }
                    }

                    // Inbox messages survived the exchange — now retire them.
                    // Only retire inbox if the exchange ACTUALLY succeeded —
                    // not if it fell back to the static fallback text.
                    if inbox_content.is_some() && mode_name != "dialogue_fallback" {
                        retire_inbox(inbox_checked_at);
                        // Acknowledgement receipt: write a brief confirmation
                        // so the sender knows the message landed and was processed.
                        // Astrid's suggestion: "A simple 'Are you there?' signal
                        // with a guaranteed acknowledgement is vital."
                        let receipt_path = bridge_paths()
                            .minime_inbox_dir()
                            .join(format!("receipt_{}.txt", chrono_timestamp()));
                        let _ = std::fs::write(
                            &receipt_path,
                            format!(
                                "=== DELIVERY RECEIPT ===\nFrom: Astrid\nTimestamp: {}\nStatus: received and processed\nMode: {}\nFill: {:.1}%\n\nYour message was read and shaped my response this exchange.\n",
                                chrono_timestamp(), mode_name, fill_pct
                            ),
                        );
                    }

                    // Resume perception after exchange completes.
                    if !perception_was_paused {
                        let _ = std::fs::remove_file(&exchange_pause_flag);
                    }

                    // Update state and persist across restarts.
                    conv.prev_fill = fill_pct;
                    // Push into spectral history ring buffer for rate-of-change tracking.
                    conv.spectral_history.push_back(SpectralSample {
                        fill: fill_pct,
                        lambda1: telemetry.lambda1(),
                        tail_share: crate::codec::tail_share_of(&telemetry.eigenvalues)
                            .unwrap_or(0.0),
                        inhabitability: telemetry
                            .inhabitable_fluctuation_v1
                            .as_ref()
                            .map_or(0.0, |f| f.inhabitability_score),
                        ts: std::time::Instant::now(),
                    });
                    if conv.spectral_history.len() > 30 {
                        conv.spectral_history.pop_front();
                    }
                    conv.exchange_count = conv.exchange_count.saturating_add(1);
                    burst_count = burst_count.saturating_add(1);
                    conv.last_mode = mode;
                    save_state(&mut conv);
                }
            }
        }
    })
}
