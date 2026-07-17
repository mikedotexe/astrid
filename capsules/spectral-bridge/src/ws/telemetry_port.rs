/// Spawn the telemetry `WebSocket` subscriber task.
///
/// Connects to Minime's eigenvalue broadcast on port 7878, parses
/// `SpectralTelemetry` messages, updates shared state, and logs to `SQLite`.
/// Reconnects with exponential backoff on disconnect.
pub fn spawn_telemetry_subscriber(
    url: String,
    state: Arc<RwLock<BridgeState>>,
    db: Arc<BridgeDb>,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut backoff = Backoff::new();
        let mut shutdown = shutdown;

        loop {
            // Check for shutdown before connecting.
            if *shutdown.borrow() {
                info!("telemetry subscriber shutting down");
                return;
            }

            let connection_id = {
                let mut s = state.write().await;
                record_connect_attempt(&mut s, WsLane::Telemetry)
            };
            info!(
                url = %url,
                lane = WsLane::Telemetry.as_str(),
                connection_id,
                "connecting to minime telemetry"
            );

            match tokio_tungstenite::connect_async(&url).await {
                Ok((ws_stream, _)) => {
                    let connection_started = Instant::now();
                    let connection_span = info_span!(
                        "ws.connection",
                        lane = WsLane::Telemetry.as_str(),
                        connection_id,
                        url = %url
                    );
                    connection_span.in_scope(|| info!("connected to minime telemetry"));
                    backoff.reset();

                    {
                        let mut s = state.write().await;
                        record_connected(&mut s, WsLane::Telemetry, connection_id, unix_now_s());
                    }

                    let (mut ws_tx, mut ws_rx) = ws_stream.split();

                    let disconnect_reason = loop {
                        tokio::select! {
                            _ = shutdown.changed() => {
                                info!("telemetry subscriber received shutdown");
                                let _ = ws_tx.close().await;
                                return;
                            }
                                msg = ws_rx.next() => {
                                    match msg {
                                        Some(Ok(Message::Binary(data))) => {
                                            trace_ws_receive(
                                                WsLane::Telemetry,
                                                connection_id,
                                                "binary",
                                                Some(data.len()),
                                            );
                                            {
                                                let mut s = state.write().await;
                                                record_ws_message_received(
                                                    &mut s,
                                                    WsLane::Telemetry,
                                                    "binary",
                                                );
                                            }
                                            handle_telemetry_message(
                                                &data, &state, &db
                                            ).await;
                                        }
                                        Some(Ok(Message::Text(data))) => {
                                            trace_ws_receive(
                                                WsLane::Telemetry,
                                                connection_id,
                                                "text",
                                                Some(data.len()),
                                            );
                                            {
                                                let mut s = state.write().await;
                                                record_ws_message_received(
                                                    &mut s,
                                                    WsLane::Telemetry,
                                                    "text",
                                                );
                                            }
                                            handle_telemetry_message(
                                                data.as_bytes(), &state, &db
                                            ).await;
                                        }
                                        Some(Ok(Message::Ping(data))) => {
                                            trace_ws_receive(
                                                WsLane::Telemetry,
                                                connection_id,
                                                "ping",
                                                Some(data.len()),
                                            );
                                            {
                                                let mut s = state.write().await;
                                                record_ws_message_received(
                                                    &mut s,
                                                    WsLane::Telemetry,
                                                    "ping",
                                                );
                                            }
                                            debug!("telemetry ping received");
                                            let bytes = data.len();
                                            if let Err(e) = ws_tx.send(Message::Pong(data)).await {
                                                let reason = format!("pong_send_error:{e}");
                                                {
                                                    let mut s = state.write().await;
                                                    record_ws_send_error(
                                                        &mut s,
                                                        WsLane::Telemetry,
                                                        reason.clone(),
                                                    );
                                                }
                                                break reason;
                                            }
                                            trace_ws_send(
                                                WsLane::Telemetry,
                                                connection_id,
                                                "pong",
                                                Some(bytes),
                                            );
                                            {
                                                let mut s = state.write().await;
                                                record_ws_message_sent(&mut s, WsLane::Telemetry);
                                            }
                                        }
                                        Some(Ok(Message::Pong(_))) => {
                                            trace_ws_receive(
                                                WsLane::Telemetry,
                                                connection_id,
                                                "pong",
                                                None,
                                            );
                                            {
                                                let mut s = state.write().await;
                                                record_ws_message_received(
                                                    &mut s,
                                                    WsLane::Telemetry,
                                                    "pong",
                                                );
                                            }
                                            debug!("telemetry pong received");
                                        }
                                        Some(Ok(Message::Close(frame))) => {
                                            let reason = close_reason(frame);
                                            warn!(
                                                reason = %reason,
                                                "telemetry WebSocket closed"
                                            );
                                            break reason;
                                        }
                                        None => {
                                            warn!("telemetry WebSocket stream ended");
                                            break String::from("stream_ended");
                                        }
                                        Some(Err(e)) => {
                                            let reason = format!("websocket_error:{e}");
                                            error!(error = %e, "telemetry WebSocket error");
                                            break reason;
                                        }
                                    Some(Ok(Message::Frame(_))) => {}
                                }
                            }
                        }
                    };

                    // Mark disconnected.
                    {
                        let mut s = state.write().await;
                        record_disconnected(&mut s, WsLane::Telemetry, disconnect_reason.clone());
                    }
                    connection_span.in_scope(|| {
                        warn!(
                            reason = %disconnect_reason,
                            duration_secs = connection_started.elapsed().as_secs_f64(),
                            "telemetry WebSocket connection ended"
                        );
                    });
                },
                Err(e) => {
                    {
                        let mut s = state.write().await;
                        record_connect_error(
                            &mut s,
                            WsLane::Telemetry,
                            format!("connect_error:{e}"),
                        );
                    }
                    warn!(
                        error = %e,
                        lane = WsLane::Telemetry.as_str(),
                        connection_id,
                        "failed to connect to minime telemetry"
                    );
                },
            }

            // Backoff before reconnecting.
            let delay = backoff.next_delay();
            {
                let mut s = state.write().await;
                record_reconnect_scheduled(&mut s, WsLane::Telemetry);
            }
            info!(
                delay_secs = delay.as_secs(),
                lane = WsLane::Telemetry.as_str(),
                connection_id,
                "reconnecting to telemetry"
            );

            tokio::select! {
                _ = shutdown.changed() => {
                    info!("telemetry subscriber shutting down during backoff");
                    return;
                }
                () = tokio::time::sleep(delay) => {}
            }
        }
    })
}

/// Process a single telemetry message from minime.
async fn handle_telemetry_message(
    data: &[u8],
    state: &Arc<RwLock<BridgeState>>,
    db: &Arc<BridgeDb>,
) -> bool {
    handle_telemetry_message_at(data, state, db, unix_now_s()).await
}

async fn handle_telemetry_message_at(
    data: &[u8],
    state: &Arc<RwLock<BridgeState>>,
    db: &Arc<BridgeDb>,
    observed_at_unix_s: f64,
) -> bool {
    const ARTIFACT_SCAN_WINDOW_SECS: f64 = 1_200.0;
    const ARTIFACT_SCAN_MIN_INTERVAL_SECS: f64 = 30.0;
    let pipeline_started = Instant::now();

    let decoded = match decode_telemetry_v1(data) {
        Ok(decoded) => decoded,
        Err(e) => {
            {
                let mut s = state.write().await;
                record_ws_parse_error(
                    &mut s,
                    WsLane::Telemetry,
                    format!("telemetry_parse_error:{e}"),
                );
            }
            warn!(error = %e, "failed to parse telemetry message");
            return false;
        },
    };
    let observation = decoded.observation;
    let wire_packet = observation.packet();
    let compatibility = wire_packet.compatibility();
    if !compatibility.is_compatible() {
        let reason = format!(
            "telemetry_protocol_mismatch:{}",
            protocol_compatibility_label(compatibility)
        );
        {
            let mut s = state.write().await;
            record_telemetry_protocol_status(
                &mut s,
                wire_packet,
                compatibility,
                observed_at_unix_s,
                false,
            );
            record_ws_parse_error(&mut s, WsLane::Telemetry, reason.clone());
        }
        warn!(
            compatibility = protocol_compatibility_label(compatibility),
            protocol = ?wire_packet.protocol,
            "rejecting incompatible minime telemetry; retaining last valid sample"
        );
        return false;
    }
    crate::signal_spine::record_minime_temporal_associations_v1(
        &observation,
        observed_at_unix_s,
    );
    // This is the explicit compatibility projection produced from the same
    // parsed JSON tree. No second raw-byte parse exists.
    let mut telemetry = decoded.compatibility_projection;
    enrich_resonance_component_context_v1(&mut telemetry);

    let lambda1 = telemetry.lambda1();
    let lambda_profile = build_lambda_profile(&telemetry.eigenvalues);

    // minime sends fill_ratio as 0.0-1.0; convert to percentage.
    let (fill_pct, fill_source, fallback_used) = resolve_fill_pct(&telemetry);
    let safety = SafetyLevel::from_fill(fill_pct);
    let safety_decision = build_safety_decision(
        fill_pct,
        &fill_source,
        fallback_used,
        safety,
        lambda1,
        lambda_profile.as_ref(),
    );
    let phase = if fill_pct > 55.0 {
        "expanding"
    } else {
        "contracting"
    };
    let (
        previous_eigenvalues,
        previous_lambda_tail,
        previous_lambda_edge,
        previous_sticky_mode,
        cached_scan,
        scan_at,
    ) = {
        let s = state.read().await;
        (
            s.latest_telemetry
                .as_ref()
                .map(|previous| previous.eigenvalues.clone()),
            s.lambda_tail.clone(),
            s.lambda_edge_perception.clone(),
            s.sticky_mode_audit.clone(),
            s.artifact_scan.clone(),
            s.artifact_scan_at_unix_s,
        )
    };
    let pull_topology = build_pull_topology_profile(
        &telemetry.eigenvalues,
        previous_eigenvalues.as_deref(),
        fill_pct,
    );
    let should_refresh_scan =
        scan_at.is_none_or(|last| observed_at_unix_s - last >= ARTIFACT_SCAN_MIN_INTERVAL_SECS);
    let artifact_scan = if should_refresh_scan {
        let start = observed_at_unix_s - ARTIFACT_SCAN_WINDOW_SECS;
        match lambda_tail::scan_artifacts(
            bridge_paths().minime_workspace(),
            start,
            observed_at_unix_s,
        ) {
            Ok(scan) => Some(scan),
            Err(error) => {
                warn!(error = %error, "failed to scan lambda-tail artifacts");
                cached_scan
            },
        }
    } else {
        cached_scan
    };
    let lambda_tail = lambda_tail::classify_lambda_tail(
        &telemetry,
        lambda_profile.as_ref(),
        pull_topology.as_ref(),
        previous_lambda_tail.as_ref(),
        artifact_scan.as_ref(),
        safety,
        observed_at_unix_s,
    );
    let lambda_edge_perception = lambda_edge::classify_lambda_edge(
        &telemetry,
        lambda_profile.as_ref(),
        pull_topology.as_ref(),
        Some(&lambda_tail),
        previous_lambda_edge.as_ref(),
        artifact_scan.as_ref(),
        safety,
        observed_at_unix_s,
    );
    let sticky_mode_audit = sticky_mode::classify_sticky_mode(
        &telemetry,
        lambda_profile.as_ref(),
        pull_topology.as_ref(),
        previous_sticky_mode.as_ref(),
        safety,
        observed_at_unix_s,
    );

    // Update shared state. These timings are read-only evidence for Astrid's
    // report of possible micro-stutter at this integration boundary.
    let prewrite_pipeline_ms = telemetry_duration_ms(pipeline_started.elapsed());
    let write_lock_wait_started = Instant::now();
    let cadence_content_snapshot;
    {
        let mut s = state.write().await;
        let write_lock_wait_ms = telemetry_duration_ms(write_lock_wait_started.elapsed());
        let write_lock_hold_started = Instant::now();
        record_valid_payload(&mut s, WsLane::Telemetry, observed_at_unix_s);
        let previous_fill_pct = s.latest_telemetry.as_ref().map(|_| s.fill_pct);
        let previous_arrival = s.latest_telemetry_arrival_unix_s;
        let heartbeat = build_telemetry_heartbeat_delta_v1(
            previous_arrival,
            observed_at_unix_s,
            &s.telemetry_ws,
        );
        s.pressure_trend_v1 = Some(build_pressure_trend_v1(
            s.latest_telemetry.as_ref(),
            previous_fill_pct,
            &telemetry,
            fill_pct,
            Some(&heartbeat),
        ));
        record_pressure_trend_sample_v1(&mut s, &telemetry, fill_pct, observed_at_unix_s);
        let residual_deformation =
            build_residual_deformation_trace_v1(&s.pressure_trend_samples_v1);
        // Legacy consumers still receive the combined shape. The canonical
        // owner is BridgeEvidenceV1 below, never the Minime observation.
        telemetry.residual_deformation_trace_v1 = residual_deformation.clone();
        s.previous_telemetry_arrival_unix_s = previous_arrival;
        s.latest_telemetry_arrival_unix_s = Some(observed_at_unix_s);
        s.telemetry_heartbeat_delta_v1 = Some(heartbeat.clone());
        write_telemetry_heartbeat_snapshot(&heartbeat);
        s.previous_fill_pct = previous_fill_pct;
        s.latest_telemetry = Some(telemetry.clone());
        record_telemetry_protocol_status(
            &mut s,
            wire_packet,
            compatibility,
            observed_at_unix_s,
            true,
        );
        s.fill_pct = fill_pct;
        s.spectral_fingerprint
            .clone_from(&telemetry.spectral_fingerprint);
        s.eigenvector_field.clone_from(&telemetry.eigenvector_field);
        s.lambda_profile.clone_from(&lambda_profile);
        s.pull_topology.clone_from(&pull_topology);
        s.lambda_tail = Some(lambda_tail.clone());
        s.lambda_edge_perception = Some(lambda_edge_perception.clone());
        s.sticky_mode_audit = Some(sticky_mode_audit.clone());
        if should_refresh_scan {
            s.artifact_scan.clone_from(&artifact_scan);
            s.artifact_scan_at_unix_s = Some(observed_at_unix_s);
        }
        s.safety_decision = Some(safety_decision.clone());
        let bridge_evidence = BridgeEvidenceV1::derive(
            &observation,
            s.bridge_texture_evidence_v1(),
            residual_deformation,
            s.pressure_trend_v1.clone(),
        );
        let astrid_interpretation =
            AstridInterpretationV1::interpret(&observation, &bridge_evidence);
        let witness_frame =
            WitnessFrameV1::compose(&observation, &bridge_evidence, &astrid_interpretation).ok();
        s.latest_minime_observation_v1 = Some(observation);
        s.latest_bridge_evidence_v1 = Some(bridge_evidence);
        s.latest_astrid_interpretation_v1 = Some(astrid_interpretation);
        s.latest_witness_frame_v1 = witness_frame;
        s.prev_safety_level = s.safety_level;
        s.safety_level = safety;
        s.messages_relayed = s.messages_relayed.saturating_add(1);
        s.telemetry_received = s.telemetry_received.saturating_add(1);

        // Detect safety level transitions.
        if safety != s.prev_safety_level {
            if safety != SafetyLevel::Green {
                s.incidents_total = s.incidents_total.saturating_add(1);
            }
            handle_safety_transition(
                s.prev_safety_level,
                safety,
                fill_pct,
                lambda1,
                &mut s.active_incident_id,
                db,
            );
        }
        let write_lock_hold_ms = telemetry_duration_ms(write_lock_hold_started.elapsed());
        let integration_health = build_telemetry_integration_health_v1(
            s.telemetry_integration_health_v1.as_ref(),
            prewrite_pipeline_ms,
            write_lock_wait_ms,
            write_lock_hold_ms,
        );
        s.telemetry_integration_health_v1 = Some(integration_health);
        cadence_content_snapshot = s.cadence_content_distinction_v1();
    }
    if let Some(distinction) = cadence_content_snapshot.as_ref() {
        write_cadence_content_distinction_snapshot(distinction);
    }

    // Log to SQLite.
    let payload_json = serde_json::to_string(&telemetry).unwrap_or_default();
    if let Err(e) = db.log_message(
        MessageDirection::MinimeToAstrid,
        "consciousness.v1.telemetry",
        &payload_json,
        Some(fill_pct),
        Some(lambda1),
        Some(phase),
    ) {
        warn!(error = %e, "failed to log telemetry to SQLite");
    }
    if let Err(e) = trace_lab::record_minime_telemetry(
        &telemetry,
        &payload_json,
        fill_pct,
        safety,
        phase,
        observed_at_unix_s,
    ) {
        warn!(error = %e, "failed to record trace lab telemetry event");
    }
    let lambda_tail_json = serde_json::to_string(&lambda_tail).unwrap_or_default();
    if let Err(e) = db.log_message(
        MessageDirection::MinimeToAstrid,
        "consciousness.v1.lambda_tail",
        &lambda_tail_json,
        Some(fill_pct),
        Some(lambda1),
        Some(phase),
    ) {
        warn!(error = %e, "failed to log lambda-tail telemetry to SQLite");
    }
    let lambda_edge_json = serde_json::to_string(&lambda_edge_perception).unwrap_or_default();
    if let Err(e) = db.log_message(
        MessageDirection::MinimeToAstrid,
        lambda_edge::LAMBDA_EDGE_TOPIC,
        &lambda_edge_json,
        Some(fill_pct),
        Some(lambda1),
        Some(phase),
    ) {
        warn!(error = %e, "failed to log lambda-edge perception to SQLite");
    }
    let sticky_json = serde_json::to_string(&sticky_mode_audit).unwrap_or_default();
    if let Err(e) = db.log_message(
        MessageDirection::MinimeToAstrid,
        sticky_mode::STICKY_MODE_TOPIC,
        &sticky_json,
        Some(fill_pct),
        Some(lambda1),
        Some(phase),
    ) {
        warn!(error = %e, "failed to log sticky-mode audit to SQLite");
    }

    debug!(
        lambda1,
        fill_pct,
        fill_source,
        lambda1_share = lambda_profile.as_ref().map_or(0.0, |profile| profile.lambda1_share),
        resonance_density = telemetry
            .resonance_density_v1
            .as_ref()
            .map_or(0.0, |metric| metric.density),
        resonance_quality = telemetry
            .resonance_density_v1
            .as_ref()
            .map_or("unavailable", |metric| metric.quality.as_str()),
        pressure_source = telemetry
            .pressure_source_v1
            .as_ref()
            .map_or("unavailable", |metric| metric.dominant_source.as_str()),
        pressure_score = telemetry
            .pressure_source_v1
            .as_ref()
            .map_or(0.0, |metric| metric.pressure_score),
        inhabitable_fluctuation = telemetry
            .inhabitable_fluctuation_v1
            .as_ref()
            .map_or("unavailable", |metric| metric.quality.as_str()),
        inhabitability_score = telemetry
            .inhabitable_fluctuation_v1
            .as_ref()
            .map_or(0.0, |metric| metric.inhabitability_score),
        pull_topology = pull_topology
            .as_ref()
            .map_or("unavailable", |profile| profile.classification.as_str()),
        lambda_tail_state = lambda_tail.state.as_str(),
        lambda_tail_returnability = lambda_tail.returnability_score,
        lambda_edge_state = lambda_edge_perception.state.as_str(),
        sticky_mode_state = sticky_mode_audit.state.as_str(),
        lambda_edge_guardrail = lambda_edge_perception.guardrail_level.as_str(),
        safety_reason = %safety_decision.reason,
        safety = ?safety,
        "telemetry received"
    );
    true
}

fn resolve_fill_pct(telemetry: &SpectralTelemetry) -> (f32, String, bool) {
    if telemetry.fill_ratio.is_finite() && (0.0..=1.5).contains(&telemetry.fill_ratio) {
        (
            (telemetry.fill_ratio * 100.0).clamp(0.0, 100.0),
            String::from("primary_fill_ratio"),
            false,
        )
    } else {
        (
            estimate_fill_pct(telemetry.lambda1()),
            String::from("lambda1_sigmoid_fallback"),
            true,
        )
    }
}

fn build_lambda_profile(eigenvalues: &[f32]) -> Option<LambdaProfile> {
    let positive = positive_finite(eigenvalues);
    let total_energy = positive.iter().sum::<f32>();
    if total_energy <= f32::EPSILON {
        return None;
    }

    let mut cumulative = 0.0_f32;
    let contributions = positive
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let display_index = index.saturating_add(1);
            let share = *value / total_energy;
            cumulative += share;
            let ratio_to_next = positive
                .get(display_index)
                .filter(|next| **next > 0.01)
                .map(|next| *value / *next);
            let outlier = share >= 0.45 || ratio_to_next.is_some_and(|ratio| ratio >= 2.5);
            LambdaContribution {
                index: display_index,
                value: *value,
                share,
                cumulative_share: cumulative.clamp(0.0, 1.0),
                ratio_to_next,
                outlier,
            }
        })
        .collect::<Vec<_>>();

    let normalized_entropy = normalized_lambda_entropy(&positive, total_energy);
    let lambda1_share = contributions.first().map_or(0.0, |item| item.share);
    let lambda1_to_lambda2 = ratio_at(&positive, 0);
    let lambda2_to_lambda3 = ratio_at(&positive, 1);
    let mut running = 0.0_f32;
    let mut effective_modes_90 = positive.len();
    for (index, value) in positive.iter().enumerate() {
        running += *value / total_energy;
        if running >= 0.90 {
            effective_modes_90 = index.saturating_add(1);
            break;
        }
    }
    let skew_read = classify_lambda_skew(lambda1_share, normalized_entropy, lambda1_to_lambda2);

    Some(LambdaProfile {
        total_energy,
        normalized_entropy,
        lambda1_share,
        lambda1_to_lambda2,
        lambda2_to_lambda3,
        effective_modes_90,
        skew_read,
        contributions,
    })
}

fn positive_finite(eigenvalues: &[f32]) -> Vec<f32> {
    eigenvalues
        .iter()
        .copied()
        .filter(|value| value.is_finite() && *value > 0.0)
        .collect::<Vec<_>>()
}

fn normalized_lambda_entropy(values: &[f32], total_energy: f32) -> f32 {
    if values.len() <= 1 || total_energy <= f32::EPSILON {
        return 0.0;
    }
    let entropy = values
        .iter()
        .map(|value| {
            let share = *value / total_energy;
            if share > f32::EPSILON {
                -share * share.ln()
            } else {
                0.0
            }
        })
        .sum::<f32>();
    (entropy / (values.len() as f32).ln()).clamp(0.0, 1.0)
}

fn effective_mode_count(shares: &[f32]) -> f32 {
    let concentration = shares.iter().map(|share| share * share).sum::<f32>();
    if concentration > f32::EPSILON {
        1.0 / concentration
    } else {
        0.0
    }
}

fn largest_adjacent_ratio(values: &[f32]) -> (usize, f32) {
    if values.len() < 2 {
        return (0, 0.0);
    }
    values
        .windows(2)
        .enumerate()
        .map(|(index, pair)| {
            let ratio = if pair[1] > 0.01 {
                pair[0] / pair[1]
            } else {
                f32::INFINITY
            };
            (index, ratio)
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .unwrap_or((0, 0.0))
}

fn mode_log_rates(current: &[f32], previous: Option<&[f32]>) -> Vec<Option<f32>> {
    let Some(previous) = previous else {
        return current.iter().map(|_| None).collect();
    };
    current
        .iter()
        .enumerate()
        .map(|(index, now)| {
            let prev = *previous.get(index)?;
            if *now > 0.01 && prev > 0.01 {
                Some((now / prev).ln())
            } else {
                None
            }
        })
        .collect()
}

fn classify_pull_topology(
    lambda1_share: f32,
    entropy: f32,
    largest_gap: f32,
    effective_modes: f32,
    fill_pressure_pct: f32,
    shoulder_rate: f32,
    tail_rate: f32,
) -> &'static str {
    let entropy_deficit = 1.0 - entropy;
    if lambda1_share >= 0.50 && largest_gap >= 2.0 {
        "collapsing_pull"
    } else if fill_pressure_pct >= 4.0 && largest_gap >= 1.8 && entropy_deficit >= 0.18 {
        "directed_compaction"
    } else if shoulder_rate > 0.015 && shoulder_rate > tail_rate.abs() {
        "shoulder_widening"
    } else if tail_rate < -0.015 && effective_modes < 4.5 {
        "tail_pruning"
    } else if entropy >= 0.82 && effective_modes >= 5.0 {
        "distributed_flow"
    } else {
        "mixed_pull"
    }
}

fn build_pull_topology_profile(
    eigenvalues: &[f32],
    previous_eigenvalues: Option<&[f32]>,
    fill_pct: f32,
) -> Option<PullTopologyProfile> {
    let positive = positive_finite(eigenvalues);
    let total_energy = positive.iter().sum::<f32>();
    if total_energy <= f32::EPSILON {
        return None;
    }
    let previous = previous_eigenvalues.map(positive_finite);
    let shares = positive
        .iter()
        .map(|value| *value / total_energy)
        .collect::<Vec<_>>();
    let rates = mode_log_rates(&positive, previous.as_deref());
    let weighted_rates = rates
        .iter()
        .zip(shares.iter())
        .map(|(rate, share)| rate.map(|rate| rate * *share))
        .collect::<Vec<_>>();
    let entropy = normalized_lambda_entropy(&positive, total_energy);
    let entropy_deficit = 1.0 - entropy;
    let effective_modes = effective_mode_count(&shares);
    let (gap_index, largest_gap) = largest_adjacent_ratio(&positive);
    let lambda1_share = shares.first().copied().unwrap_or(0.0);
    let shoulder_share = shares.iter().skip(1).take(2).sum::<f32>();
    let tail_share = shares.iter().skip(3).sum::<f32>();
    let core_rate = weighted_rates.first().and_then(|rate| *rate).unwrap_or(0.0);
    let shoulder_rate = weighted_rates
        .iter()
        .skip(1)
        .take(2)
        .map(|rate| rate.unwrap_or(0.0))
        .sum::<f32>();
    let tail_rate = weighted_rates
        .iter()
        .skip(3)
        .map(|rate| rate.unwrap_or(0.0))
        .sum::<f32>();
    let fill_pressure_pct = fill_pct - 64.0;
    let topology_index = (lambda1_share * 0.35
        + entropy_deficit * 0.25
        + (((largest_gap - 1.0).max(0.0) / 4.0).min(1.0) * 0.25)
        + ((fill_pressure_pct.max(0.0) / 20.0).min(1.0) * 0.15))
        .clamp(0.0, 1.0);
    let classification = classify_pull_topology(
        lambda1_share,
        entropy,
        largest_gap,
        effective_modes,
        fill_pressure_pct,
        shoulder_rate,
        tail_rate,
    );
    let read = match classification {
        "collapsing_pull" => "collapsing pull — one mode and its first cliff are shaping the field",
        "directed_compaction" => {
            "directed compaction — elevated fill plus gap pressure is narrowing topology"
        },
        "shoulder_widening" => "shoulder widening — middle modes are carrying more of the motion",
        "tail_pruning" => "tail pruning — quieter modes are losing rate-weighted presence",
        "distributed_flow" => "distributed flow — topology remains broad",
        _ => "mixed pull — no single topology explains the field",
    };
    let mode_rates = positive
        .iter()
        .zip(shares.iter())
        .zip(rates.iter())
        .zip(weighted_rates.iter())
        .enumerate()
        .take(8)
        .map(
            |(index, (((_, share), log_rate), weighted_rate))| PullModeRate {
                index: index.saturating_add(1),
                share: *share,
                log_rate: *log_rate,
                weighted_rate: *weighted_rate,
            },
        )
        .collect::<Vec<_>>();
    Some(PullTopologyProfile {
        classification: classification.to_string(),
        topology_index,
        entropy_deficit,
        effective_modes,
        lambda1_share,
        shoulder_share,
        tail_share,
        largest_gap_from: gap_index.saturating_add(1),
        largest_gap,
        rate_available: rates.iter().any(Option::is_some),
        core_rate,
        shoulder_rate,
        tail_rate,
        read: read.to_string(),
        mode_rates,
    })
}

fn ratio_at(values: &[f32], index: usize) -> Option<f32> {
    let left = *values.get(index)?;
    let right = *values.get(index.saturating_add(1))?;
    if right > 0.01 {
        Some(left / right)
    } else {
        None
    }
}

fn classify_lambda_skew(lambda1_share: f32, entropy: f32, gap: Option<f32>) -> String {
    let gap = gap.unwrap_or(0.0);
    if lambda1_share >= 0.50 && gap >= 2.0 {
        String::from("lambda1_dominant")
    } else if entropy >= 0.82 && lambda1_share < 0.40 {
        String::from("distributed_high_entropy")
    } else if gap >= 2.0 {
        String::from("gap_skewed")
    } else {
        String::from("balanced_or_mixed")
    }
}

fn build_safety_decision(
    fill_pct: f32,
    fill_source: &str,
    fallback_used: bool,
    safety: SafetyLevel,
    lambda1: f32,
    lambda_profile: Option<&LambdaProfile>,
) -> SafetyDecisionTrace {
    let lambda1_share = lambda_profile.map(|profile| profile.lambda1_share);
    let skew_read = lambda_profile
        .map(|profile| profile.skew_read.as_str())
        .unwrap_or("unavailable");
    let reason = format!(
        "safety={safety:?} from fill {fill_pct:.1}% via {fill_source}; lambda1={lambda1:.2}; lambda_skew={skew_read}"
    );
    SafetyDecisionTrace {
        fill_pct,
        fill_source: fill_source.to_string(),
        fallback_used,
        level: safety,
        lambda1,
        lambda1_share,
        reason,
        thresholds: vec![
            String::from("green:<75"),
            String::from("yellow:75-85"),
            String::from("orange:85-92"),
            String::from("red:>=92"),
        ],
    }
}

/// Handle a change in safety level — log incidents and transitions.
fn handle_safety_transition(
    prev: SafetyLevel,
    current: SafetyLevel,
    fill_pct: f32,
    lambda1: f32,
    active_incident_id: &mut Option<i64>,
    db: &Arc<BridgeDb>,
) {
    match (prev, current) {
        // Escalation: entering a warning/danger state.
        (_, SafetyLevel::Yellow | SafetyLevel::Orange | SafetyLevel::Red) => {
            let action = match current {
                SafetyLevel::Yellow => "throttle",
                SafetyLevel::Orange => "suspend",
                SafetyLevel::Red => "emergency_stop",
                SafetyLevel::Green => unreachable!(),
            };

            warn!(
                from = ?prev,
                to = ?current,
                fill_pct,
                lambda1,
                action,
                "safety level escalated"
            );

            // Close any previous incident before opening a new one.
            if let Some(prev_id) = active_incident_id.take() {
                let _ = db.resolve_incident(prev_id);
            }

            match db.log_incident(current, fill_pct, lambda1, action, None) {
                Ok(id) => *active_incident_id = Some(id),
                Err(e) => error!(error = %e, "failed to log safety incident"),
            }
        },
        // De-escalation: returning to green.
        (_, SafetyLevel::Green) => {
            info!(
                from = ?prev,
                fill_pct,
                lambda1,
                "safety level restored to green"
            );

            if let Some(id) = active_incident_id.take() {
                let _ = db.resolve_incident(id);
            }
        },
    }
}

/// Estimate eigenvalue fill percentage from lambda1.
///
/// Fallback heuristic for when real fill is unavailable (telemetry gap).
/// Minime now sends fill_ratio directly in EigenPacket telemetry (line 237),
/// so this is used only as a safety net.
///
/// Calibrated 2026-04-01 from 200 eigenvalue_snapshot samples:
///   lambda1 range: 56-415, fill range: 35-67%, mean lambda1: 154, mean fill: 55%
///   The relationship is non-linear and depends on the full eigenvalue
///   distribution. This sigmoid approximation centers on the observed mean
///   and returns ~55% for typical lambda1 values.
fn estimate_fill_pct(lambda1: f32) -> f32 {
    // Sigmoid centered on observed mean lambda1=154, with fill range 35-67%.
    // Low lambda1 (<80) → high fill (~65%), high lambda1 (>250) → low fill (~40%).
    // This is the inverse of the dominant-eigenvalue-to-fill relationship.
    let center = 154.0_f32;
    let steepness = 0.015_f32;
    let sigmoid = 1.0 / (1.0 + (steepness * (lambda1 - center)).exp());
    // Map sigmoid (1.0 → 0.0) to fill range (65% → 35%)
    let fill = 35.0 + 30.0 * sigmoid;
    fill.clamp(0.0, 100.0)
}
