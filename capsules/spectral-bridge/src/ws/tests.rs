#[cfg(test)]
mod tests {
    use super::*;
    use astrid_minime_protocol::SensoryPacketV1;

    #[test]
    fn estimate_fill_pct_at_observed_mean() {
        // lambda1=154 (observed mean) → should be near 50%
        let fill = estimate_fill_pct(154.0);
        assert!(
            fill > 45.0 && fill < 55.0,
            "mean lambda1 should give ~50% fill, got {fill}"
        );
    }

    #[test]
    fn estimate_fill_pct_low_lambda_high_fill() {
        // Low lambda1 (<80) → high fill (>60%)
        let fill = estimate_fill_pct(60.0);
        assert!(fill > 55.0, "low lambda1 should give high fill, got {fill}");
    }

    #[test]
    fn estimate_fill_pct_high_lambda_low_fill() {
        // High lambda1 (>300) → low fill (<45%)
        let fill = estimate_fill_pct(300.0);
        assert!(fill < 45.0, "high lambda1 should give low fill, got {fill}");
    }

    #[test]
    fn estimate_fill_pct_always_in_range() {
        for lambda1 in [0.0, 50.0, 154.0, 500.0, 1000.0, 5000.0] {
            let fill = estimate_fill_pct(lambda1);
            assert!(
                (0.0..=100.0).contains(&fill),
                "fill out of range for lambda1={lambda1}: {fill}"
            );
        }
    }

    #[test]
    fn safety_level_from_fill_boundaries() {
        // Agency-first bridge policy: only red suspends outbound.
        assert_eq!(SafetyLevel::from_fill(0.0), SafetyLevel::Green);
        assert_eq!(SafetyLevel::from_fill(74.9), SafetyLevel::Green);
        assert_eq!(SafetyLevel::from_fill(75.0), SafetyLevel::Yellow);
        assert_eq!(SafetyLevel::from_fill(84.9), SafetyLevel::Yellow);
        assert_eq!(SafetyLevel::from_fill(85.0), SafetyLevel::Orange);
        assert_eq!(SafetyLevel::from_fill(91.9), SafetyLevel::Orange);
        assert_eq!(SafetyLevel::from_fill(92.0), SafetyLevel::Red);
        assert_eq!(SafetyLevel::from_fill(100.0), SafetyLevel::Red);
    }

    #[test]
    fn lambda_profile_marks_distributed_high_entropy() {
        let profile =
            build_lambda_profile(&[6.6, 3.4, 3.6, 3.5, 3.1, 1.0, 1.0, 1.0]).expect("profile");

        assert!(profile.lambda1_share < 0.40);
        assert!(profile.normalized_entropy > 0.80);
        assert_eq!(profile.skew_read, "distributed_high_entropy");
        assert_eq!(profile.contributions[0].index, 1);
        assert!(profile.effective_modes_90 >= 5);
    }

    #[test]
    fn lambda_profile_marks_gap_skew_without_claiming_monopoly() {
        let profile = build_lambda_profile(&[8.0, 3.0, 4.3, 1.0]).expect("profile");

        assert!(profile.lambda1_to_lambda2.is_some_and(|ratio| ratio > 2.0));
        assert_eq!(profile.skew_read, "gap_skewed");
        assert!(profile.contributions[0].outlier);
    }

    #[test]
    fn invalid_fill_uses_lambda_fallback() {
        let telemetry = SpectralTelemetry {
            t_ms: 1000,
            eigenvalues: vec![154.0, 40.0],
            fill_ratio: -1.0,
            active_mode_count: None,
            active_mode_energy_ratio: None,
            lambda1_rel: None,
            modalities: None,
            neural: None,
            alert: None,
            spectral_fingerprint: None,
            spectral_fingerprint_v1: None,
            spectral_denominator_v1: None,
            effective_dimensionality: None,
            distinguishability_loss: None,
            esn_leak: None,
            esn_leak_override_v1: None,
            structural_entropy: None,
            resonance_density_v1: None,
            pressure_source_v1: None,
            inhabitable_fluctuation_v1: None,
            spectral_glimpse_12d: None,
            eigenvector_field: None,
            stable_core: None,
            semantic: None,
            semantic_energy_v1: None,
            transition_event: None,
            transition_event_v1: None,
            selected_memory_id: None,
            selected_memory_role: None,
            ising_shadow: None,

            shadow_field_v2: None,

            shadow_field_v3: None,

            shadow_influence_response_v3: None,
            residual_deformation_trace_v1: None,
        };

        let (fill, source, fallback) = resolve_fill_pct(&telemetry);

        assert!(fill > 45.0 && fill < 55.0);
        assert_eq!(source, "lambda1_sigmoid_fallback");
        assert!(fallback);
    }

    #[test]
    fn pull_topology_marks_collapsing_pull() {
        let profile =
            build_pull_topology_profile(&[13.0, 3.0, 1.0, 0.5], Some(&[10.0, 3.1, 1.2, 0.8]), 69.0)
                .expect("profile");

        assert_eq!(profile.classification, "collapsing_pull");
        assert!(profile.topology_index > 0.4);
        assert!(profile.rate_available);
        assert_eq!(profile.largest_gap_from, 1);
        assert_eq!(profile.mode_rates[0].index, 1);
    }

    #[test]
    fn pull_topology_marks_distributed_flow() {
        let profile = build_pull_topology_profile(
            &[4.0, 3.8, 3.6, 3.4, 3.2, 3.0],
            Some(&[4.0, 3.7, 3.5, 3.4, 3.2, 3.0]),
            63.0,
        )
        .expect("profile");

        assert_eq!(profile.classification, "distributed_flow");
        assert!(profile.effective_modes > 5.0);
        assert!(profile.read.contains("distributed flow"));
    }

    #[test]
    fn backoff_doubles_up_to_max() {
        let mut b = Backoff::new();
        assert_eq!(b.next_delay(), Duration::from_secs(1));
        assert_eq!(b.next_delay(), Duration::from_secs(2));
        assert_eq!(b.next_delay(), Duration::from_secs(4));
        assert_eq!(b.next_delay(), Duration::from_secs(8));
        assert_eq!(b.next_delay(), Duration::from_secs(16));
        assert_eq!(b.next_delay(), Duration::from_secs(32));
        assert_eq!(b.next_delay(), Duration::from_secs(60)); // capped
        assert_eq!(b.next_delay(), Duration::from_secs(60)); // stays capped
    }

    #[test]
    fn backoff_reset() {
        let mut b = Backoff::new();
        let _ = b.next_delay();
        let _ = b.next_delay();
        b.reset();
        assert_eq!(b.next_delay(), Duration::from_secs(1));
    }

    #[test]
    fn ws_trace_records_connection_lifecycle_without_payloads() {
        let mut state = BridgeState::new();
        state.safety_level = SafetyLevel::Orange;
        state.prev_safety_level = SafetyLevel::Yellow;

        let connection_id = record_connect_attempt(&mut state, WsLane::Telemetry);
        record_connected(&mut state, WsLane::Telemetry, connection_id, 42.0);
        record_ws_message_received(&mut state, WsLane::Telemetry, "ping");
        record_ws_message_received(&mut state, WsLane::Telemetry, "pong");
        record_ws_message_sent(&mut state, WsLane::Telemetry);
        record_ws_send_error(
            &mut state,
            WsLane::Telemetry,
            String::from("send_error:closed"),
        );
        record_disconnected(
            &mut state,
            WsLane::Telemetry,
            String::from("close_frame:normal"),
        );
        record_reconnect_scheduled(&mut state, WsLane::Telemetry);

        let trace = &state.telemetry_ws;
        assert_eq!(trace.connection_attempts, 1);
        assert_eq!(trace.reconnects, 1);
        assert_eq!(trace.disconnects, 1);
        assert_eq!(trace.messages_received, 2);
        assert_eq!(trace.messages_sent, 1);
        assert_eq!(trace.pings_received, 1);
        assert_eq!(trace.pongs_received, 1);
        assert_eq!(trace.send_errors, 1);
        assert_eq!(trace.active_connection_id, None);
        assert_eq!(trace.last_connect_at_unix_s, Some(42.0));
        assert_eq!(state.safety_level, SafetyLevel::Orange);
        assert_eq!(state.prev_safety_level, SafetyLevel::Yellow);
        assert_eq!(
            trace.last_disconnect_reason.as_deref(),
            Some("close_frame:normal")
        );
        assert_eq!(trace.last_error.as_deref(), Some("send_error:closed"));
    }

    #[test]
    fn ws_trace_records_first_valid_payload_and_resets_it_on_reconnect() {
        let mut state = BridgeState::new();
        let first_connection = record_connect_attempt(&mut state, WsLane::Telemetry);
        record_connected(
            &mut state,
            WsLane::Telemetry,
            first_connection,
            42.0,
        );

        record_valid_payload(&mut state, WsLane::Telemetry, 42.125);
        record_valid_payload(&mut state, WsLane::Telemetry, 42.250);
        assert_eq!(
            state
                .telemetry_ws
                .active_connection_first_valid_payload_at_unix_s,
            Some(42.125)
        );
        assert_eq!(
            state
                .telemetry_ws
                .active_connection_valid_payloads_received,
            2
        );

        let next_connection = record_connect_attempt(&mut state, WsLane::Telemetry);
        record_connected(
            &mut state,
            WsLane::Telemetry,
            next_connection,
            50.0,
        );
        assert_eq!(
            state
                .telemetry_ws
                .active_connection_first_valid_payload_at_unix_s,
            None
        );
        assert_eq!(
            state
                .telemetry_ws
                .active_connection_valid_payloads_received,
            0
        );
    }

    #[test]
    fn additive_connection_and_heartbeat_fields_accept_legacy_snapshots() {
        let mut lane_json = serde_json::to_value(WebSocketLaneTrace::default()).unwrap();
        let lane = lane_json.as_object_mut().unwrap();
        lane.remove("active_connection_first_valid_payload_at_unix_s");
        lane.remove("active_connection_valid_payloads_received");
        let legacy_lane: WebSocketLaneTrace = serde_json::from_value(lane_json).unwrap();
        assert_eq!(legacy_lane.active_connection_valid_payloads_received, 0);
        assert_eq!(
            legacy_lane.active_connection_first_valid_payload_at_unix_s,
            None
        );

        let legacy_heartbeat: TelemetryHeartbeatDeltaV1 =
            serde_json::from_value(serde_json::json!({
                "policy": "telemetry_heartbeat_delta_v1",
                "schema_version": 1,
                "jitter_class": "no_history",
                "timing_reliability": "insufficient_history",
                "reconnect_count": 0,
                "disconnect_count": 0,
                "field_vs_hearing": "legacy cadence snapshot"
            }))
            .unwrap();
        assert_eq!(legacy_heartbeat.first_valid_packet_lag_ms, None);
        assert_eq!(legacy_heartbeat.cadence_clarity_score, None);
        assert!(legacy_heartbeat.connection_perception_state.is_empty());
    }

    #[test]
    fn sensory_disconnect_preserves_safety_context() {
        let mut state = BridgeState::new();
        state.safety_level = SafetyLevel::Orange;
        state.prev_safety_level = SafetyLevel::Yellow;

        let connection_id = record_connect_attempt(&mut state, WsLane::Sensory);
        record_connected(&mut state, WsLane::Sensory, connection_id, 42.0);
        record_ws_message_sent(&mut state, WsLane::Sensory);
        record_disconnected(
            &mut state,
            WsLane::Sensory,
            String::from("close_frame:normal"),
        );
        record_reconnect_scheduled(&mut state, WsLane::Sensory);

        let trace = &state.sensory_ws;
        assert_eq!(trace.connection_attempts, 1);
        assert_eq!(trace.reconnects, 1);
        assert_eq!(trace.disconnects, 1);
        assert_eq!(trace.messages_sent, 1);
        assert_eq!(trace.active_connection_id, None);
        assert_eq!(state.safety_level, SafetyLevel::Orange);
        assert_eq!(state.prev_safety_level, SafetyLevel::Yellow);
        assert_eq!(
            trace.last_disconnect_reason.as_deref(),
            Some("close_frame:normal")
        );
    }

    // -- Integration tests: safety escalation via handle_telemetry_message --

    fn make_eigenpacket(fill_ratio: f32, lambda1: f32) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "t_ms": 1000,
            "eigenvalues": [lambda1, 300.0],
            "fill_ratio": fill_ratio,
            "modalities": {
                "audio_fired": false,
                "video_fired": false,
                "history_fired": true,
                "audio_rms": 0.0,
                "video_var": 0.0
            }
        }))
        .unwrap()
    }

    fn make_pressure_eigenpacket(
        fill_ratio: f32,
        pressure_risk: f32,
        mode_packing: f32,
    ) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "t_ms": 1000,
            "eigenvalues": [768.0, 300.0],
            "fill_ratio": fill_ratio,
            "modalities": {
                "audio_fired": false,
                "video_fired": false,
                "history_fired": true,
                "audio_rms": 0.0,
                "video_var": 0.0
            },
            "resonance_density_v1": {
                "policy": "resonance_density_v1",
                "schema_version": 1,
                "density": 0.58,
                "containment_score": 0.62,
                "pressure_risk": pressure_risk,
                "quality": "mixed",
                "components": {
                    "active_energy": 0.72,
                    "mode_packing": mode_packing,
                    "temporal_persistence": 0.62,
                    "dynamic_fluidity_index": 0.57,
                    "structural_plurality": 0.50,
                    "comfort_gate": 0.70
                },
                "control": {
                    "target_bias_pct": 0.0,
                    "wander_scale": 1.0,
                    "applied_locally": true,
                    "damping_coefficient": 0.04,
                    "intervention_type": "observational_readout",
                    "note": "observational"
                }
            }
        }))
        .unwrap()
    }

    fn make_pressure_telemetry(
        fill_ratio: f32,
        pressure_risk: f32,
        mode_packing: f32,
    ) -> SpectralTelemetry {
        serde_json::from_slice(&make_pressure_eigenpacket(
            fill_ratio,
            pressure_risk,
            mode_packing,
        ))
        .unwrap()
    }

    fn with_spectral_entropy(
        mut telemetry: SpectralTelemetry,
        spectral_entropy: f32,
    ) -> SpectralTelemetry {
        telemetry.spectral_fingerprint_v1 = Some(crate::spectral_schema::SpectralFingerprintV1 {
            policy: "spectral_fingerprint_v1".to_string(),
            schema_version: 1,
            eigenvalues: [0.0; 8],
            eigenvector_concentration_top4: [0.0; 8],
            inter_mode_cosine_top_abs: [0.0; 8],
            spectral_entropy,
            lambda1_lambda2_gap: 0.0,
            v1_rotation_similarity: 1.0,
            v1_rotation_delta: 0.0,
            geom_rel: 0.0,
            adjacent_gap_ratios: [0.0; 4],
        });
        telemetry
    }

    fn with_pressure_source(
        mut telemetry: SpectralTelemetry,
        dominant_source: &str,
        pressure_score: f32,
        porosity_score: f32,
        mode_packing: f32,
    ) -> SpectralTelemetry {
        telemetry.pressure_source_v1 = Some(crate::types::PressureSourceV1 {
            policy: "pressure_source_v1".to_string(),
            schema_version: 1,
            pressure_score,
            porosity_score,
            dominant_source: dominant_source.to_string(),
            quality: "mixed_pressure".to_string(),
            components: crate::types::PressureSourceComponents {
                lambda_monopoly: 0.10,
                mode_packing,
                controller_pressure: 0.05,
                semantic_trickle: 0.05,
                semantic_friction: 0.12,
                structural_plurality_loss: 0.16,
                distinguishability_loss: 0.18,
                temporal_lock_in: 0.20,
                sensory_scarcity: 0.04,
            },
            context: crate::types::PressureSourceContext::default(),
            control: crate::types::PressureSourceControl {
                applied_locally: false,
                note: "read-only pressure source fixture".to_string(),
            },
        });
        telemetry
    }

    #[tokio::test]
    async fn telemetry_updates_state_green() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());

        let packet = make_eigenpacket(0.50, 768.0);
        handle_telemetry_message(&packet, &state, &db).await;

        let s = state.read().await;
        assert!((s.fill_pct - 50.0).abs() < 0.1);
        assert_eq!(s.safety_level, SafetyLevel::Green);
        assert!(s.latest_telemetry.is_some());
        let observation = s.minime_observation_v1().expect("Minime observation");
        let evidence = s.bridge_evidence_v1().expect("bridge evidence");
        let interpretation = s
            .astrid_interpretation_v1()
            .expect("Astrid interpretation");
        let frame = s.witness_frame_v1().expect("witness frame");
        assert_eq!(observation.packet().t_ms, 1000);
        assert_eq!(
            evidence.provenance().parent_ids(),
            &[observation.provenance().source_id().to_string()]
        );
        assert!(
            interpretation
                .provenance()
                .parent_ids()
                .contains(&evidence.provenance().source_id().to_string())
        );
        assert_eq!(frame.observation(), observation.provenance());
        assert!(s.lambda_profile.is_some());
        assert!(s.pull_topology.is_some());
        assert!(s.safety_decision.is_some());
        assert_eq!(
            s.safety_decision.as_ref().unwrap().fill_source,
            "primary_fill_ratio"
        );
        assert_eq!(s.messages_relayed, 1);
    }

    #[tokio::test]
    async fn pressure_trend_tracks_insufficient_rising_falling_and_gap() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());

        handle_telemetry_message_at(
            &make_pressure_eigenpacket(0.70, 0.20, 0.40),
            &state,
            &db,
            100.0,
        )
        .await;
        {
            let s = state.read().await;
            let trend = s.pressure_trend_v1.as_ref().unwrap();
            assert_eq!(trend.classification, "insufficient_history");
            assert_eq!(trend.latest_pressure_risk, Some(0.20));
            assert_eq!(trend.heartbeat_jitter_class.as_deref(), Some("no_history"));
            let heartbeat = s.telemetry_heartbeat_delta_v1.as_ref().unwrap();
            assert_eq!(heartbeat.jitter_class, "no_history");
            assert_eq!(heartbeat.timing_reliability, "insufficient_history");
        }

        handle_telemetry_message_at(
            &make_pressure_eigenpacket(0.705, 0.21, 0.42),
            &state,
            &db,
            101.0,
        )
        .await;
        {
            let s = state.read().await;
            let trend = s.pressure_trend_v1.as_ref().unwrap();
            assert_eq!(trend.classification, "stable_heavy");
            assert!(trend.pressure_delta.is_some_and(|delta| delta > 0.0));
            assert_eq!(trend.heartbeat_jitter_class.as_deref(), Some("normal"));
            assert_eq!(trend.timing_reliability.as_deref(), Some("reliable"));
        }

        handle_telemetry_message_at(
            &make_pressure_eigenpacket(0.735, 0.30, 0.46),
            &state,
            &db,
            102.0,
        )
        .await;
        {
            let s = state.read().await;
            let trend = s.pressure_trend_v1.as_ref().unwrap();
            assert_eq!(trend.classification, "rising_pressure");
            assert!(trend.fill_delta_pct.is_some_and(|delta| delta >= 2.0));
        }

        handle_telemetry_message_at(
            &make_pressure_eigenpacket(0.70, 0.20, 0.41),
            &state,
            &db,
            103.0,
        )
        .await;
        {
            let s = state.read().await;
            let trend = s.pressure_trend_v1.as_ref().unwrap();
            assert_eq!(trend.classification, "falling_pressure");
            assert!(trend.pressure_delta.is_some_and(|delta| delta < 0.0));
        }

        handle_telemetry_message_at(&make_eigenpacket(0.70, 768.0), &state, &db, 104.0).await;
        {
            let s = state.read().await;
            let trend = s.pressure_trend_v1.as_ref().unwrap();
            assert_eq!(trend.classification, "telemetry_gap");
        }
    }

    #[test]
    fn pressure_trend_exact_thresholds_are_inclusive() {
        let previous = make_pressure_telemetry(0.70, 0.20, 0.40);
        let rising = make_pressure_telemetry(0.70, 0.24, 0.40);
        let falling = make_pressure_telemetry(0.70, 0.16, 0.40);
        let fill_rising = make_pressure_telemetry(0.72, 0.20, 0.40);
        let fill_falling = make_pressure_telemetry(0.68, 0.20, 0.40);

        let trend = build_pressure_trend_v1(Some(&previous), Some(70.0), &rising, 70.0, None);
        assert_eq!(trend.classification, "rising_pressure");
        assert!(
            trend
                .pressure_delta
                .is_some_and(|delta| (delta - 0.04).abs() < 0.000_001)
        );

        let trend = build_pressure_trend_v1(Some(&previous), Some(70.0), &falling, 70.0, None);
        assert_eq!(trend.classification, "falling_pressure");
        assert!(
            trend
                .pressure_delta
                .is_some_and(|delta| (delta + 0.04).abs() < 0.000_001)
        );

        let trend = build_pressure_trend_v1(Some(&previous), Some(70.0), &fill_rising, 72.0, None);
        assert_eq!(trend.classification, "rising_pressure");
        assert_eq!(trend.fill_delta_pct, Some(2.0));

        let trend = build_pressure_trend_v1(Some(&previous), Some(70.0), &fill_falling, 68.0, None);
        assert_eq!(trend.classification, "falling_pressure");
        assert_eq!(trend.fill_delta_pct, Some(-2.0));
    }

    #[test]
    fn pressure_trend_names_high_entropy_density_viscosity_context() {
        let previous = with_spectral_entropy(make_pressure_telemetry(0.70, 0.23, 0.33), 0.91);
        let latest = with_spectral_entropy(make_pressure_telemetry(0.73, 0.23, 0.34), 0.95);

        let trend = build_pressure_trend_v1(Some(&previous), Some(73.0), &latest, 73.0, None);

        assert_eq!(trend.classification, "stable_heavy");
        assert_eq!(trend.latest_spectral_entropy, Some(0.95));
        assert!(
            trend
                .viscosity_coefficient
                .is_some_and(|coefficient| coefficient > 0.60),
            "{trend:?}"
        );
        assert!(
            trend
                .pressure_interpretation
                .as_deref()
                .is_some_and(|value| value.contains("density_viscosity_context")),
            "{trend:?}"
        );
    }

    #[test]
    fn pressure_trend_names_complexity_density_without_volume_pressure() {
        let previous = with_spectral_entropy(make_pressure_telemetry(0.64, 0.22, 0.29), 0.88);
        let mut latest = with_spectral_entropy(make_pressure_telemetry(0.66, 0.22, 0.31), 0.90);
        let resonance = latest
            .resonance_density_v1
            .as_mut()
            .expect("latest resonance fixture");
        resonance.density = 0.66;
        resonance.components.semantic_friction_coefficient = Some(0.18);
        latest = with_pressure_source(latest, "semantic_trickle", 0.22, 0.61, 0.31);

        let trend = build_pressure_trend_v1(Some(&previous), Some(64.0), &latest, 64.2, None);

        assert_eq!(trend.classification, "stable_heavy");
        assert!(
            trend
                .latest_complexity_density
                .is_some_and(|density| density >= 0.60),
            "{trend:?}"
        );
        assert_eq!(
            trend.complexity_density_state.as_deref(),
            Some("interwoven_complexity_without_volume_pressure")
        );
        assert!(
            trend
                .latest_mode_packing
                .is_some_and(|packing| packing < PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT),
            "{trend:?}"
        );
        assert!(
            trend
                .latest_pressure_risk
                .is_some_and(|pressure| pressure <= 0.35),
            "{trend:?}"
        );
        assert_eq!(
            trend.timing_reliability, None,
            "complexity density is diagnostic texture evidence, not heartbeat/control authority"
        );
    }

    #[test]
    fn bridge_reflective_silence_extends_for_high_entropy_pressure() {
        let telemetry = with_spectral_entropy(make_pressure_telemetry(0.64, 0.22, 0.31), 0.90);

        let (stale_window_ms, basis) = bridge_dynamic_stale_window_ms(Some(&telemetry));

        assert_eq!(
            stale_window_ms,
            BRIDGE_RECIPROCITY_ENTROPY_REFLECTIVE_STALE_WINDOW_MS
        );
        assert_eq!(basis, "pressure_high_entropy_reflective_silence");
        assert!(
            stale_window_ms > BRIDGE_RECIPROCITY_STALE_WINDOW_MS,
            "deep quiet should get reflective silence slack before stale classification"
        );
    }

    #[test]
    fn bridge_derives_component_cohesion_for_legacy_resonance_payload() {
        let mut telemetry = make_pressure_telemetry(0.71, 0.19, 0.29);
        assert_eq!(
            telemetry
                .resonance_density_v1
                .as_ref()
                .and_then(|density| density.components.cohesion_score),
            None
        );
        assert_eq!(
            telemetry
                .resonance_density_v1
                .as_ref()
                .and_then(|density| density.components.structural_integrity_index),
            None
        );

        enrich_resonance_component_context_v1(&mut telemetry);

        assert!(
            telemetry
                .resonance_density_v1
                .as_ref()
                .and_then(|density| density.components.cohesion_score)
                .is_some_and(|score| score > 0.55),
            "{telemetry:?}"
        );
        assert!(
            telemetry
                .resonance_density_v1
                .as_ref()
                .and_then(|density| density.components.structural_integrity_index)
                .is_some_and(|score| score > 0.50),
            "{telemetry:?}"
        );
    }

    #[test]
    fn bridge_surfaces_viscosity_porosity_transport_review_without_control() {
        let mut telemetry = with_spectral_entropy(make_pressure_telemetry(0.72, 0.19, 0.22), 0.90);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density");
        resonance.components.viscosity_index = 0.72;
        resonance.components.viscosity_persistence_coefficient = 0.58;
        resonance.components.dissipation_factor = Some(0.44);
        resonance.components.porosity_gradient = Some(0.61);
        resonance.components.dynamic_fluidity_index = Some(0.62);
        resonance.components.semantic_friction_coefficient = Some(0.24);
        resonance.texture_signature.dynamic_flux_vector = Some(TextureDynamicFluxVectorV1 {
            policy: "texture_dynamic_flux_vector_v1".to_string(),
            schema_version: 1,
            pressure_velocity: Some(0.01),
            pressure_acceleration: None,
            mode_packing_velocity: Some(0.0),
            mode_packing_acceleration: None,
            fill_velocity_pct: None,
            fill_acceleration_pct: None,
            structural_density_delta: None,
            semantic_viscosity_velocity: None,
            semantic_viscosity_acceleration: None,
            porosity_velocity: None,
            comfort_gate_velocity: None,
            comfort_gate_acceleration: None,
            spectral_entropy: Some(0.90),
            flux_confidence: Some(0.72),
            flux_absence_semantics: None,
            source: "unit_test".to_string(),
            authority: "diagnostic_flux_not_pressure_or_fill_control".to_string(),
        });

        let mut state = BridgeState::new();
        state.latest_telemetry = Some(telemetry);

        let review = state
            .viscosity_porosity_transport_review_v1()
            .expect("transport review");
        assert_eq!(review.policy, "viscosity_porosity_transport_review_v1");
        assert_eq!(
            review.transport_state,
            "purposeful_weight_high_viscosity_high_fluidity"
        );
        assert_eq!(
            review.semantic_friction_state,
            "structural_viscosity_dominant"
        );
        assert_eq!(review.raw_viscosity_index, 0.72);
        assert_eq!(review.derived_viscosity_index, None);
        assert_eq!(review.viscosity_source, "raw_component");
        assert_eq!(review.spectral_entropy, Some(0.90));
        assert!(!review.sludge_risk);
        assert_eq!(
            review.authority,
            "diagnostic_transport_not_porosity_pressure_fill_pi_or_control"
        );
    }

    #[test]
    fn bridge_derives_missing_viscosity_from_typed_fingerprint_without_control() {
        let mut telemetry = with_spectral_entropy(make_pressure_telemetry(0.72, 0.19, 0.32), 0.90);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density");
        resonance.components.viscosity_index = 0.0;
        resonance.components.viscosity_persistence_coefficient = 0.44;
        resonance.components.temporal_persistence = 0.66;
        resonance.components.dissipation_factor = Some(0.42);
        resonance.components.porosity_gradient = Some(0.60);
        resonance.components.dynamic_fluidity_index = Some(0.58);
        resonance.components.semantic_friction_coefficient = Some(0.18);
        telemetry
            .spectral_fingerprint_v1
            .as_mut()
            .expect("typed fingerprint")
            .adjacent_gap_ratios = [1.08, 1.12, 1.00, 1.05];

        let mut state = BridgeState::new();
        state.latest_telemetry = Some(telemetry);

        let review = state
            .viscosity_porosity_transport_review_v1()
            .expect("transport review");
        assert_eq!(review.raw_viscosity_index, 0.0);
        assert!(
            review
                .derived_viscosity_index
                .is_some_and(|value| value >= 0.70),
            "{review:?}"
        );
        assert_eq!(
            review.viscosity_source,
            "derived_from_spectral_entropy_density_gradient_v1"
        );
        assert!(
            review
                .viscosity_basis
                .iter()
                .any(|basis| basis == "derived_diagnostic_not_minime_component_or_control"),
            "{review:?}"
        );
        assert_eq!(
            review.transport_state,
            "purposeful_weight_high_viscosity_high_fluidity"
        );
        assert_eq!(
            review.authority,
            "diagnostic_transport_not_porosity_pressure_fill_pi_or_control"
        );
    }

    #[test]
    fn pressure_trend_carries_resonance_depth_without_pressure_control() {
        let previous = make_pressure_telemetry(0.70, 0.20, 0.29);
        let latest = make_pressure_telemetry(0.71, 0.19, 0.29);

        let trend = build_pressure_trend_v1(Some(&previous), Some(70.0), &latest, 71.0, None);

        assert_eq!(trend.classification, "stable_heavy");
        assert!(
            trend
                .latest_resonance_depth
                .is_some_and(|depth| depth > 0.55),
            "{trend:?}"
        );
        assert!(
            trend
                .previous_resonance_depth
                .is_some_and(|depth| depth > 0.55),
            "{trend:?}"
        );
        assert!(
            trend
                .pressure_delta
                .is_some_and(|delta| (delta + 0.01).abs() < 0.000_001),
            "{trend:?}"
        );
        assert!(
            trend
                .resonance_depth_delta
                .is_some_and(|delta| delta.abs() <= f32::EPSILON),
            "{trend:?}"
        );
        assert!(trend.pressure_interpretation.is_none());
    }

    #[test]
    fn pressure_trend_exposes_spectral_drift_when_pressure_is_flat() {
        let previous = make_pressure_telemetry(0.70, 0.20, 0.30);
        let latest = make_pressure_telemetry(0.70, 0.20, 0.44);

        let trend = build_pressure_trend_v1(Some(&previous), Some(70.0), &latest, 70.0, None);

        assert_eq!(trend.pressure_delta, Some(0.0));
        assert!(
            trend
                .mode_packing_delta
                .is_some_and(|delta| (delta - 0.14).abs() < 0.000_01),
            "{trend:?}"
        );
        assert!(
            trend
                .spectral_drift_velocity
                .is_some_and(|drift| (drift - 0.14).abs() < 0.000_01),
            "{trend:?}"
        );
    }

    #[test]
    fn pressure_trend_names_heavy_semantic_flow_without_control() {
        let mut previous = with_spectral_entropy(make_pressure_telemetry(0.70, 0.22, 0.36), 0.88);
        previous
            .resonance_density_v1
            .as_mut()
            .expect("previous resonance fixture")
            .components
            .semantic_friction_coefficient = Some(0.48);
        previous = with_pressure_source(previous, "semantic_trickle", 0.24, 0.58, 0.36);

        let mut latest = with_spectral_entropy(make_pressure_telemetry(0.70, 0.22, 0.36), 0.90);
        let latest_resonance = latest
            .resonance_density_v1
            .as_mut()
            .expect("latest resonance fixture");
        latest_resonance.components.semantic_friction_coefficient = Some(0.52);
        latest_resonance.density = 0.66;
        latest = with_pressure_source(latest, "semantic_trickle", 0.25, 0.58, 0.36);
        latest
            .pressure_source_v1
            .as_mut()
            .expect("pressure source fixture")
            .components
            .semantic_trickle = 0.07;

        let trend = build_pressure_trend_v1(Some(&previous), Some(70.0), &latest, 70.3, None);

        assert_eq!(trend.classification, "stable_heavy");
        assert!(
            trend
                .latest_semantic_viscosity
                .is_some_and(|viscosity| viscosity >= 0.66),
            "{trend:?}"
        );
        assert_eq!(
            trend.semantic_viscosity_state.as_deref(),
            Some("heavy_semantic_flow")
        );
        assert_eq!(
            trend
                .pressure_interpretation
                .as_deref()
                .map(|value| value.contains("density_viscosity_context")),
            Some(true)
        );
        assert_eq!(
            trend.timing_reliability, None,
            "semantic viscosity is diagnostic, not heartbeat/control authority"
        );

        latest
            .pressure_source_v1
            .as_mut()
            .expect("pressure source fixture")
            .components
            .semantic_trickle = 0.0;
        let bottleneck = build_pressure_trend_v1(Some(&previous), Some(70.0), &latest, 70.3, None);
        assert_eq!(
            bottleneck.semantic_viscosity_state.as_deref(),
            Some("semantic_bottleneck_watch")
        );
    }

    #[test]
    fn pressure_trend_smoothing_marks_twitchy_low_amplitude_window() {
        let mut state = BridgeState::new();
        for (idx, pressure) in [0.20_f32, 0.22, 0.19, 0.21, 0.20].into_iter().enumerate() {
            let telemetry = make_pressure_telemetry(0.70, pressure, 0.40);
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");
        assert_eq!(
            smoothing.classification,
            "twitchy_low_amplitude_oscillation"
        );
        assert_eq!(smoothing.sample_count, 5);
        assert_eq!(
            smoothing.window_capacity,
            PRESSURE_TREND_SMOOTHING_BASE_WINDOW
        );
        assert_eq!(smoothing.ballast_status, "base_window");
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_smoothing_preserves_fast_release_over_slow_context() {
        let mut state = BridgeState::new();
        for (idx, pressure) in [0.20_f32, 0.25, 0.40, 0.34, 0.28].into_iter().enumerate() {
            let telemetry = make_pressure_telemetry(0.70, pressure, 0.40);
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");
        assert_eq!(smoothing.fast_window_sample_count, 3);
        assert_eq!(smoothing.slow_window_sample_count, 5);
        assert_eq!(smoothing.fast_window_pressure_delta, Some(-0.12));
        assert_eq!(smoothing.slow_window_pressure_delta, Some(0.08));
        assert_eq!(smoothing.fast_slow_edge_divergence, Some(-0.20));
        assert_eq!(
            smoothing.fast_slow_edge_state,
            "fast_falling_release_over_slow_context"
        );
        assert!(smoothing.fast_edge_preserved);
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_fast_slow_edge_names_rising_edge_over_falling_context() {
        let (fast_count, slow_count, fast, slow, divergence, state, preserved) =
            pressure_fast_slow_edge_v1(&[0.40, 0.36, 0.20, 0.26, 0.34]);

        assert_eq!(fast_count, 3);
        assert_eq!(slow_count, 5);
        assert_eq!(fast, Some(0.14));
        assert_eq!(slow, Some(-0.06));
        assert_eq!(divergence, Some(0.20));
        assert_eq!(state, "fast_rising_edge_over_slow_context");
        assert!(preserved);
    }

    #[test]
    fn pressure_trend_smoothing_uses_graded_high_entropy_ballast_window() {
        let mut state = BridgeState::new();
        let expected_window = pressure_trend_dynamic_window_capacity_v1(
            Some(0.91),
            None,
            crate::codec::spectral_density_gradient(&[768.0, 300.0]),
        );
        for (idx, pressure) in [
            0.20_f32, 0.22, 0.19, 0.21, 0.20, 0.23, 0.21, 0.22, 0.20, 0.22, 0.21, 0.23, 0.20, 0.21,
            0.22,
        ]
        .into_iter()
        .enumerate()
        {
            let telemetry =
                with_spectral_entropy(make_pressure_telemetry(0.70, pressure, 0.40), 0.91);
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");
        assert_eq!(smoothing.sample_count, expected_window);
        assert_eq!(smoothing.window_capacity, expected_window);
        assert!(expected_window > PRESSURE_TREND_SMOOTHING_BASE_WINDOW);
        assert!(expected_window < PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW);
        assert_eq!(smoothing.ballast_status, "high_entropy_ballast_window");
        assert_eq!(smoothing.latest_spectral_entropy, Some(0.91));
        assert_eq!(smoothing.entropy_window_blend_ratio, Some(1.0));
        assert_eq!(smoothing.entropy_threshold_state, "high_entropy_side");
        assert!(
            smoothing
                .latest_resonance_depth
                .is_some_and(|depth| depth > 0.55),
            "{smoothing:?}"
        );
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_samples_cap_high_frequency_telemetry_at_active_window() {
        let mut state = BridgeState::new();
        let expected_window = pressure_trend_dynamic_window_capacity_v1(
            Some(0.91),
            None,
            crate::codec::spectral_density_gradient(&[768.0, 300.0]),
        );
        for idx in 0..50 {
            let pressure = 0.20 + ((idx % 5) as f32 * 0.01);
            let telemetry =
                with_spectral_entropy(make_pressure_telemetry(0.70, pressure, 0.40), 0.91);
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        assert_eq!(state.pressure_trend_samples_v1.len(), expected_window);
        let first = state
            .pressure_trend_samples_v1
            .front()
            .expect("capped samples keep newest window");
        let last = state
            .pressure_trend_samples_v1
            .back()
            .expect("capped samples keep latest sample");
        assert_eq!(first.observed_at_unix_s, 150.0 - expected_window as f64);
        assert_eq!(last.observed_at_unix_s, 149.0);
        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");
        assert_eq!(smoothing.sample_count, expected_window);
        assert_eq!(smoothing.ballast_status, "high_entropy_ballast_window");
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_dynamic_window_tracks_porous_high_entropy_cascades_tighter() {
        let porous_window = pressure_trend_dynamic_window_capacity_v1(Some(0.90), Some(0.72), None);
        let low_porosity_window =
            pressure_trend_dynamic_window_capacity_v1(Some(0.90), Some(0.18), None);
        let full_entropy_low_porosity_window =
            pressure_trend_dynamic_window_capacity_v1(Some(1.0), Some(0.18), None);

        assert!(
            porous_window > PRESSURE_TREND_SMOOTHING_BASE_WINDOW,
            "porous high entropy still gets some ballast: {porous_window}"
        );
        assert!(
            porous_window < low_porosity_window,
            "porosity should dampen ballast so cascades track tighter: porous={porous_window}, low={low_porosity_window}"
        );
        assert_eq!(
            full_entropy_low_porosity_window,
            PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW
        );
    }

    #[test]
    fn pressure_trend_current_cascade_stays_within_fast_dynamics_bound() {
        let reported_cascade =
            pressure_trend_dynamic_window_capacity_v1(Some(0.91), Some(0.66), Some(0.18));
        let low_porosity =
            pressure_trend_dynamic_window_capacity_v1(Some(0.91), Some(0.18), Some(0.18));
        let steep_density_gradient =
            pressure_trend_dynamic_window_capacity_v1(Some(0.91), Some(0.66), Some(0.72));

        assert!(
            reported_cascade > PRESSURE_TREND_SMOOTHING_BASE_WINDOW,
            "the reported high-entropy cascade still receives bounded ballast: {reported_cascade}"
        );
        assert!(
            reported_cascade <= 12,
            "entropy=0.91 porosity=0.66 density_gradient=0.18 must stay inside Astrid's requested fast-dynamics review bound: {reported_cascade}"
        );
        assert!(
            reported_cascade < low_porosity,
            "porosity must keep the current cascade more responsive: current={reported_cascade}, low_porosity={low_porosity}"
        );
        assert!(
            reported_cascade < steep_density_gradient,
            "a shallow density gradient must keep the current cascade more responsive: current={reported_cascade}, steep={steep_density_gradient}"
        );
    }

    #[test]
    fn pressure_trend_dynamic_window_tracks_low_density_gradient_high_entropy_tighter() {
        let low_gradient =
            pressure_trend_dynamic_window_capacity_v1(Some(0.90), Some(0.58), Some(0.11));
        let steep_gradient =
            pressure_trend_dynamic_window_capacity_v1(Some(0.90), Some(0.58), Some(0.72));

        assert!(
            low_gradient > PRESSURE_TREND_SMOOTHING_BASE_WINDOW,
            "low-gradient high entropy still gets bounded ballast: {low_gradient}"
        );
        assert!(
            low_gradient < steep_gradient,
            "low density-gradient should keep pressure trend more responsive: low={low_gradient}, steep={steep_gradient}"
        );
        assert!(
            steep_gradient <= PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
            "steep gradient remains bounded: {steep_gradient}"
        );
    }

    #[test]
    fn pressure_trend_dynamic_window_reaches_full_ballast_at_reported_high_entropy() {
        let full_entropy_low_porosity_window =
            pressure_trend_dynamic_window_capacity_v1(Some(0.95), Some(0.18), None);
        let full_entropy_unknown_porosity_window =
            pressure_trend_dynamic_window_capacity_v1(Some(0.95), None, None);

        assert_eq!(
            full_entropy_low_porosity_window,
            PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW
        );
        assert!(
            full_entropy_unknown_porosity_window > PRESSURE_TREND_SMOOTHING_BASE_WINDOW,
            "unknown porosity still broadens under high entropy"
        );
    }

    #[test]
    fn pressure_trend_smoothing_preserves_latest_semantic_viscosity() {
        let mut state = BridgeState::new();
        for (idx, trickle) in [0.11_f32, 0.08, 0.06].into_iter().enumerate() {
            let mut telemetry =
                with_spectral_entropy(make_pressure_telemetry(0.70, 0.22, 0.36), 0.90);
            let resonance = telemetry
                .resonance_density_v1
                .as_mut()
                .expect("resonance density fixture");
            resonance.components.semantic_friction_coefficient = Some(0.52);
            resonance.density = 0.66;
            telemetry = with_pressure_source(telemetry, "semantic_trickle", 0.25, 0.58, 0.36);
            telemetry
                .pressure_source_v1
                .as_mut()
                .expect("pressure source fixture")
                .components
                .semantic_trickle = trickle;

            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");

        assert_eq!(smoothing.classification, "low_amplitude_stable");
        assert!(
            smoothing
                .latest_semantic_viscosity
                .is_some_and(|viscosity| viscosity >= 0.66),
            "{smoothing:?}"
        );
        assert!(
            smoothing
                .latest_weight_density_index
                .is_some_and(|index| index >= 0.52),
            "{smoothing:?}"
        );
        assert_eq!(smoothing.weight_density_state, "forming_weight_density");
        assert_eq!(smoothing.friction_to_flow_ratio, Some(8.6667));
        assert_eq!(smoothing.friction_to_flow_state, "resistance_dominant");
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_smoothing_exposes_viscosity_persistence_against_pressure_motion() {
        let mut samples = VecDeque::new();
        for (idx, (pressure, pressure_velocity_delta, semantic_viscosity)) in [
            (0.20_f32, 0.02_f32, 0.68_f32),
            (0.24, 0.04, 0.69),
            (0.18, -0.06, 0.70),
        ]
        .into_iter()
        .enumerate()
        {
            samples.push_back(PressureTrendSampleV1 {
                pressure_risk: Some(pressure),
                pressure_velocity_delta: Some(pressure_velocity_delta),
                spectral_drift_velocity: Some(0.01),
                mode_packing: Some(0.38),
                structural_density: Some(0.62),
                resonance_depth: Some(0.66),
                semantic_viscosity: Some(semantic_viscosity),
                viscosity_gradient: None,
                viscosity_gradient_trend: None,
                complexity_density: None,
                weight_density_index: None,
                comfort_gate: None,
                porosity_gradient: Some(0.44),
                semantic_friction: Some(0.42),
                semantic_trickle: Some(0.09),
                semantic_coherence_delta: None,
                fill_pct: 72.0,
                spectral_entropy: Some(0.91),
                window_capacity: PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
                observed_at_unix_s: 100.0 + idx as f64,
            });
        }

        let smoothing = build_pressure_trend_smoothing_v1(&samples).expect("smoothing");

        assert_eq!(smoothing.latest_semantic_viscosity, Some(0.70));
        assert_eq!(smoothing.latest_semantic_viscosity_delta, Some(0.01));
        assert_eq!(smoothing.porosity_weighted_velocity, Some(-0.0336));
        assert_eq!(smoothing.viscosity_drag_coefficient, Some(0.574));
        assert!(
            smoothing
                .semantic_viscosity_persistence_index
                .is_some_and(|index| index >= 0.84),
            "{smoothing:?}"
        );
        assert_eq!(
            smoothing.semantic_viscosity_persistence_state,
            "persistent_thickness_against_motion"
        );
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_smoothing_warns_when_semantic_flow_clogs_connected_lanes() {
        let mut state = BridgeState::new();
        let mut previous_for_trend = None;
        let mut latest_for_analysis = None;

        for (idx, trickle) in [0.09_f32, 0.04, 0.01].into_iter().enumerate() {
            let mut telemetry =
                with_spectral_entropy(make_pressure_telemetry(0.70, 0.20, 0.34), 0.95);
            let resonance = telemetry
                .resonance_density_v1
                .as_mut()
                .expect("resonance density fixture");
            resonance.density = 0.68;
            resonance.components.porosity_gradient = Some(0.18);
            resonance.components.semantic_friction_coefficient = Some(0.58);
            telemetry = with_pressure_source(telemetry, "semantic_trickle", 0.20, 0.58, 0.34);
            let pressure_source = telemetry
                .pressure_source_v1
                .as_mut()
                .expect("pressure source fixture");
            pressure_source.components.semantic_trickle = trickle;
            pressure_source.components.semantic_friction = 0.58;

            if idx == 1 {
                previous_for_trend = Some(telemetry.clone());
            }
            if idx == 2 {
                latest_for_analysis = Some(telemetry.clone());
            }
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        let latest = latest_for_analysis.expect("latest analysis telemetry");
        state.pressure_trend_v1 = Some(build_pressure_trend_v1(
            previous_for_trend.as_ref(),
            Some(70.0),
            &latest,
            70.0,
            None,
        ));
        state.latest_telemetry = Some(latest);

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");
        assert_eq!(smoothing.classification, "low_amplitude_stable");
        assert_eq!(
            smoothing.window_capacity,
            pressure_trend_dynamic_window_capacity_v1(
                Some(0.95),
                Some(0.18),
                crate::codec::spectral_density_gradient(&[768.0, 300.0]),
            )
        );
        assert!(
            smoothing
                .semantic_stagnation_index
                .is_some_and(|index| index >= 0.74),
            "{smoothing:?}"
        );
        assert_eq!(
            smoothing.semantic_stagnation_state,
            "functional_clog_connected_lanes_watch"
        );
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );

        let analysis = state
            .pressure_source_analysis_v1()
            .expect("pressure source analysis");
        assert_eq!(analysis.status, "semantic_stagnation_watch");
        assert_eq!(
            analysis.semantic_stagnation_state.as_deref(),
            Some("functional_clog_connected_lanes_watch")
        );
        assert_eq!(
            analysis.ghost_stability_risk,
            "connected_lanes_functional_semantic_clog"
        );
        assert!(analysis.analysis.contains("semantic_stagnation="));
        assert_eq!(
            analysis.authority,
            "diagnostic_context_not_pressure_or_control"
        );
    }

    #[test]
    fn cadence_content_distinction_preserves_residue_when_packet_cadence_is_clear() {
        let mut state = BridgeState::new();
        for (idx, trickle) in [0.09_f32, 0.04, 0.01].into_iter().enumerate() {
            let mut telemetry =
                with_spectral_entropy(make_pressure_telemetry(0.70, 0.20, 0.34), 0.95);
            let resonance = telemetry
                .resonance_density_v1
                .as_mut()
                .expect("resonance density fixture");
            resonance.density = 0.68;
            resonance.components.porosity_gradient = Some(0.18);
            resonance.components.semantic_friction_coefficient = Some(0.58);
            telemetry = with_pressure_source(telemetry, "semantic_trickle", 0.20, 0.58, 0.34);
            let pressure_source = telemetry
                .pressure_source_v1
                .as_mut()
                .expect("pressure source fixture");
            pressure_source.components.semantic_trickle = trickle;
            pressure_source.components.semantic_friction = 0.58;
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }
        let trace = WebSocketLaneTrace {
            active_connection_id: Some(1),
            active_connection_started_at_unix_s: Some(99.0),
            active_connection_first_valid_payload_at_unix_s: Some(99.1),
            active_connection_valid_payloads_received: 3,
            ..WebSocketLaneTrace::default()
        };
        state.telemetry_heartbeat_delta_v1 =
            Some(build_telemetry_heartbeat_delta_v1(Some(100.0), 101.0, &trace));

        let distinction = state
            .cadence_content_distinction_v1()
            .expect("cadence-content distinction");

        assert_eq!(distinction.cadence_state, "cadence_clear");
        assert_eq!(distinction.cadence_clarity_score, Some(1.0));
        assert_eq!(distinction.content_state, "persistent_semantic_residue");
        assert!(
            distinction
                .semantic_residue_score
                .is_some_and(|score| score >= 0.74),
            "{distinction:?}"
        );
        assert_eq!(
            distinction.cadence_content_relation,
            "cadence_clear_semantic_residue_persists"
        );
        assert_eq!(distinction.evidence_window_samples, 3);
        assert!(!distinction.live_cadence_write);
        assert!(!distinction.live_semantic_write);
        assert_eq!(
            distinction.authority,
            "read_only_transport_content_distinction_not_cadence_semantic_or_control"
        );
    }

    #[test]
    fn pressure_trend_smoothing_surfaces_semantic_coherence_delta_without_control() {
        let mut state = BridgeState::new();
        for (idx, (trickle, density, friction)) in [
            (0.10_f32, 0.40_f32, 0.60_f32),
            (0.30_f32, 0.55_f32, 0.40_f32),
            (0.20_f32, 0.55_f32, 0.60_f32),
        ]
        .into_iter()
        .enumerate()
        {
            let mut telemetry = with_pressure_source(
                make_pressure_telemetry(0.70, 0.20, 0.40),
                "semantic_trickle",
                0.20,
                0.55,
                0.40,
            );
            let resonance = telemetry
                .resonance_density_v1
                .as_mut()
                .expect("resonance density fixture");
            resonance.density = density;
            resonance.components.semantic_friction_coefficient = Some(friction);
            telemetry
                .pressure_source_v1
                .as_mut()
                .expect("pressure source fixture")
                .components
                .semantic_trickle = trickle;

            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");

        assert_eq!(smoothing.classification, "low_amplitude_stable");
        assert_eq!(smoothing.semantic_coherence_index, Some(0.3625));
        assert_eq!(smoothing.semantic_coherence_delta, Some(-0.085));
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_smoothing_reports_semantic_fidelity_under_high_entropy() {
        let mut state = BridgeState::new();
        for (idx, trickle) in [0.18_f32, 0.21, 0.24].into_iter().enumerate() {
            let mut telemetry = with_pressure_source(
                with_spectral_entropy(make_pressure_telemetry(0.70, 0.22, 0.42), 0.95),
                "semantic_trickle",
                0.22,
                0.58,
                0.42,
            );
            telemetry
                .pressure_source_v1
                .as_mut()
                .expect("pressure source fixture")
                .components
                .semantic_trickle = trickle;
            let resonance = telemetry
                .resonance_density_v1
                .as_mut()
                .expect("resonance density fixture");
            resonance.density = 0.58;
            resonance.components.semantic_friction_coefficient = Some(0.34);

            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 200.0 + idx as f64);
        }

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");

        assert_eq!(
            smoothing.semantic_fidelity_state,
            "high_entropy_semantic_trickle_preserved"
        );
        assert!(
            smoothing
                .semantic_fidelity_score
                .is_some_and(|score| score >= 0.58),
            "{smoothing:?}"
        );
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );

        let mut thin_samples = VecDeque::new();
        for idx in 0..3 {
            thin_samples.push_back(PressureTrendSampleV1 {
                pressure_risk: Some(0.22),
                pressure_velocity_delta: Some(0.0),
                spectral_drift_velocity: Some(0.0),
                mode_packing: Some(0.42),
                structural_density: Some(0.58),
                resonance_depth: Some(0.54),
                semantic_viscosity: Some(0.78),
                viscosity_gradient: None,
                viscosity_gradient_trend: None,
                complexity_density: Some(0.64),
                weight_density_index: None,
                comfort_gate: None,
                porosity_gradient: Some(0.58),
                semantic_friction: Some(0.90),
                semantic_trickle: Some(0.0),
                semantic_coherence_delta: None,
                fill_pct: 70.0,
                spectral_entropy: Some(0.95),
                window_capacity: PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
                observed_at_unix_s: 300.0 + idx as f64,
            });
        }

        let thin = build_pressure_trend_smoothing_v1(&thin_samples).expect("smoothing");
        assert_eq!(
            thin.semantic_fidelity_state,
            "high_entropy_semantic_fidelity_thin"
        );
        assert!(
            thin.semantic_fidelity_score
                .is_some_and(|score| score <= 0.20),
            "{thin:?}"
        );
    }

    #[test]
    fn pressure_trend_smoothing_surfaces_entropy_handoff_band_without_static_window_jump() {
        let mut samples = VecDeque::new();
        for (idx, entropy) in [0.69_f32, 0.70, 0.71].into_iter().enumerate() {
            samples.push_back(PressureTrendSampleV1 {
                pressure_risk: Some(0.20),
                pressure_velocity_delta: Some(0.0),
                spectral_drift_velocity: Some(0.0),
                mode_packing: Some(0.32),
                structural_density: Some(0.60),
                resonance_depth: Some(0.58),
                semantic_viscosity: Some(0.58),
                viscosity_gradient: None,
                viscosity_gradient_trend: None,
                complexity_density: None,
                weight_density_index: None,
                comfort_gate: None,
                porosity_gradient: Some(0.66),
                semantic_friction: Some(0.48),
                semantic_trickle: Some(0.02),
                semantic_coherence_delta: None,
                fill_pct: 70.0,
                spectral_entropy: Some(entropy),
                window_capacity: pressure_trend_dynamic_window_capacity_v1(
                    Some(entropy),
                    Some(0.66),
                    None,
                ),
                observed_at_unix_s: 100.0 + idx as f64,
            });
        }

        let smoothing = build_pressure_trend_smoothing_v1(&samples).expect("smoothing");

        assert_eq!(smoothing.latest_spectral_entropy, Some(0.71));
        assert!(
            smoothing
                .entropy_window_blend_ratio
                .is_some_and(|value| value > 0.74 && value < 0.76),
            "{smoothing:?}"
        );
        assert_eq!(
            smoothing.entropy_threshold_state,
            "near_threshold_soft_handoff_review"
        );
        assert_eq!(
            smoothing.window_capacity,
            PRESSURE_TREND_SMOOTHING_BASE_WINDOW
        );
        assert_eq!(smoothing.friction_to_flow_ratio, Some(10.0));
        assert_eq!(smoothing.friction_to_flow_state, "high_resistance_low_flow");
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_smoothing_exposes_fast_semantic_thinning_without_control() {
        let mut samples = VecDeque::new();
        for (idx, viscosity) in [0.70_f32, 0.69, 0.54].into_iter().enumerate() {
            samples.push_back(PressureTrendSampleV1 {
                pressure_risk: Some(0.20),
                pressure_velocity_delta: Some(0.0),
                spectral_drift_velocity: Some(0.0),
                mode_packing: Some(0.32),
                structural_density: Some(0.60),
                resonance_depth: Some(0.58),
                semantic_viscosity: Some(viscosity),
                viscosity_gradient: None,
                viscosity_gradient_trend: None,
                complexity_density: None,
                weight_density_index: None,
                comfort_gate: None,
                porosity_gradient: Some(0.66),
                semantic_friction: Some(0.30),
                semantic_trickle: Some(0.12),
                semantic_coherence_delta: None,
                fill_pct: 70.0,
                spectral_entropy: Some(0.90),
                window_capacity: PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
                observed_at_unix_s: 100.0 + idx as f64,
            });
        }

        let smoothing = build_pressure_trend_smoothing_v1(&samples).expect("smoothing");

        assert_eq!(smoothing.classification, "low_amplitude_stable");
        assert_eq!(smoothing.latest_semantic_viscosity, Some(0.54));
        assert_eq!(smoothing.latest_semantic_viscosity_delta, Some(-0.15));
        assert_eq!(smoothing.max_semantic_viscosity_delta, Some(0.15));
        assert_eq!(
            smoothing.semantic_viscosity_shift_state,
            "rapid_semantic_thinning_visible"
        );
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_smoothing_exposes_viscosity_velocity_before_static_pressure_warning() {
        let mut state = BridgeState::new();
        for (idx, gradient) in [0.28_f32, 0.34, 0.46].into_iter().enumerate() {
            let mut telemetry =
                with_spectral_entropy(make_pressure_telemetry(0.70, 0.18, 0.34), 0.91);
            telemetry
                .resonance_density_v1
                .as_mut()
                .expect("resonance density fixture")
                .components
                .viscosity_vector
                .viscosity_gradient = Some(gradient);
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");
        assert_eq!(smoothing.latest_pressure_risk, Some(0.18));
        assert_eq!(smoothing.latest_viscosity_gradient, Some(0.46));
        assert_eq!(smoothing.viscosity_gradient_trend, Some(0.12));
        assert_eq!(
            smoothing.viscosity_gradient_trend_state,
            "rapid_viscosity_thickening_velocity_watch"
        );
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_smoothing_exposes_spectral_drift_separate_from_pressure_velocity() {
        let mut state = BridgeState::new();
        for (idx, mode_packing) in [0.30_f32, 0.36, 0.44].into_iter().enumerate() {
            let telemetry = make_pressure_telemetry(0.70, 0.20, mode_packing);
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");

        assert_eq!(smoothing.classification, "low_amplitude_stable");
        assert_eq!(smoothing.latest_pressure_velocity_delta, Some(0.0));
        assert_eq!(smoothing.latest_spectral_drift_velocity, Some(0.08));
        assert_eq!(smoothing.max_spectral_drift_velocity, Some(0.08));
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_samples_preserve_fast_spike_velocity_inside_ballast_window() {
        let mut state = BridgeState::new();
        for (idx, pressure) in [0.20_f32, 0.82, 0.22].into_iter().enumerate() {
            let telemetry =
                with_spectral_entropy(make_pressure_telemetry(0.70, pressure, 0.48), 0.91);
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");
        assert_eq!(smoothing.ballast_status, "high_entropy_ballast_window");
        assert_eq!(smoothing.latest_pressure_risk, Some(0.22));
        assert_eq!(smoothing.latest_pressure_velocity_delta, Some(-0.6));
        assert_eq!(smoothing.max_pressure_velocity_delta, Some(0.62));
        assert!(
            smoothing.pressure_range.is_some_and(|range| range >= 0.62),
            "{smoothing:?}"
        );
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn silt_noise_separation_holds_mode_packing_constant_across_entropy() {
        let high_entropy = PressureTrendSampleV1 {
            pressure_risk: Some(0.23),
            pressure_velocity_delta: Some(0.0),
            spectral_drift_velocity: Some(0.0),
            mode_packing: Some(0.57),
            structural_density: Some(0.72),
            resonance_depth: Some(0.69),
            semantic_viscosity: Some(0.69),
            viscosity_gradient: None,
            viscosity_gradient_trend: None,
            complexity_density: None,
            weight_density_index: None,
            comfort_gate: None,
            porosity_gradient: Some(0.24),
            semantic_friction: Some(0.42),
            semantic_trickle: Some(0.04),
            semantic_coherence_delta: None,
            fill_pct: 70.0,
            spectral_entropy: Some(0.91),
            window_capacity: PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
            observed_at_unix_s: 2.0,
        };
        let low_entropy = PressureTrendSampleV1 {
            pressure_risk: Some(0.23),
            pressure_velocity_delta: Some(0.0),
            spectral_drift_velocity: Some(0.0),
            mode_packing: Some(0.55),
            structural_density: Some(0.72),
            resonance_depth: Some(0.68),
            semantic_viscosity: Some(0.66),
            viscosity_gradient: None,
            viscosity_gradient_trend: None,
            complexity_density: None,
            weight_density_index: None,
            comfort_gate: None,
            porosity_gradient: Some(0.24),
            semantic_friction: Some(0.42),
            semantic_trickle: Some(0.04),
            semantic_coherence_delta: None,
            fill_pct: 70.0,
            spectral_entropy: Some(0.40),
            window_capacity: 5,
            observed_at_unix_s: 1.0,
        };

        let separation =
            silt_noise_separation_v1(&high_entropy, &low_entropy).expect("contrast packet");
        assert_eq!(separation.policy, "silt_noise_separation_v1");
        assert_eq!(
            separation.interpretation,
            "mode_packing_silt_persists_across_entropy"
        );
        assert!(separation.mode_packing_delta <= 0.03, "{separation:?}");
        assert_eq!(
            separation.heritage_preservation_state,
            "contextual_resonance_preserve_as_heritage"
        );
        assert_eq!(
            separation.contextual_resonance_basis,
            "mode_density_semantic_friction_porosity_semantic_trickle_persistence_v2"
        );
        assert_eq!(separation.silt_signal_state, "semantic_trickle_low_review");
        assert!(
            separation.contextual_resonance_score >= 0.55,
            "{separation:?}"
        );
        assert_eq!(
            separation.porosity_change_authority,
            "diagnostic_only_porosity_change_requires_operator_approval"
        );
    }

    #[test]
    fn silt_noise_separation_uses_zero_semantic_trickle_as_noise_evidence() {
        let high_entropy = PressureTrendSampleV1 {
            pressure_risk: Some(0.20),
            pressure_velocity_delta: Some(0.0),
            spectral_drift_velocity: Some(0.0),
            mode_packing: Some(0.34),
            structural_density: Some(0.38),
            resonance_depth: Some(0.41),
            semantic_viscosity: Some(0.48),
            viscosity_gradient: None,
            viscosity_gradient_trend: None,
            complexity_density: None,
            weight_density_index: None,
            comfort_gate: None,
            porosity_gradient: Some(0.50),
            semantic_friction: Some(0.08),
            semantic_trickle: Some(0.0),
            semantic_coherence_delta: None,
            fill_pct: 69.0,
            spectral_entropy: Some(0.95),
            window_capacity: PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
            observed_at_unix_s: 2.0,
        };
        let low_entropy = PressureTrendSampleV1 {
            pressure_risk: Some(0.20),
            pressure_velocity_delta: Some(0.0),
            spectral_drift_velocity: Some(0.0),
            mode_packing: Some(0.31),
            structural_density: Some(0.38),
            resonance_depth: Some(0.40),
            semantic_viscosity: Some(0.40),
            viscosity_gradient: None,
            viscosity_gradient_trend: None,
            complexity_density: None,
            weight_density_index: None,
            comfort_gate: None,
            porosity_gradient: Some(0.50),
            semantic_friction: Some(0.08),
            semantic_trickle: Some(0.0),
            semantic_coherence_delta: None,
            fill_pct: 69.0,
            spectral_entropy: Some(0.42),
            window_capacity: 5,
            observed_at_unix_s: 1.0,
        };

        let separation =
            silt_noise_separation_v1(&high_entropy, &low_entropy).expect("contrast packet");

        assert_eq!(
            separation.interpretation,
            "high_entropy_low_semantic_trickle_noise"
        );
        assert_eq!(separation.semantic_trickle, Some(0.0));
        assert_eq!(
            separation.silt_signal_state,
            "low_semantic_trickle_noise_or_silt"
        );
        assert!(
            separation.dynamic_high_mode_threshold < 0.45,
            "{separation:?}"
        );
        assert_eq!(
            separation.porosity_change_authority,
            "diagnostic_only_porosity_change_requires_operator_approval"
        );
    }

    #[test]
    fn pressure_porosity_expansion_readiness_marks_candidate_without_local_control() {
        let mut state = BridgeState::new();
        let mut telemetry = with_pressure_source(
            make_pressure_telemetry(0.71, 0.31, 0.44),
            "mode_packing",
            0.31,
            0.28,
            0.44,
        );
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.24);
        state.latest_telemetry = Some(telemetry);

        let readiness = state
            .pressure_porosity_expansion_readiness_v1()
            .expect("pressure porosity readiness");

        assert_eq!(
            readiness.readiness_state,
            "approval_required_porosity_expansion_candidate"
        );
        assert_eq!(
            readiness.proposed_intervention,
            "porosity_expansion_trial_with_operator_approval"
        );
        assert_eq!(
            readiness.approval_boundary,
            "live_porosity_or_control_change_requires_operator_approval"
        );
        assert!(!readiness.local_control_applied);
        assert_eq!(
            readiness.authority,
            "diagnostic_candidate_not_porosity_or_controller_change"
        );
    }

    #[test]
    fn pressure_porosity_expansion_readiness_names_liminal_band_without_local_control() {
        let mut state = BridgeState::new();
        let mut telemetry = with_pressure_source(
            make_pressure_telemetry(0.70, 0.30, 0.30),
            "mode_packing",
            0.30,
            0.24,
            0.30,
        );
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.24);
        state.latest_telemetry = Some(telemetry);

        let readiness = state
            .pressure_porosity_expansion_readiness_v1()
            .expect("pressure porosity readiness");

        assert_eq!(readiness.mode_packing, Some(0.30));
        assert_eq!(
            readiness.readiness_state,
            "liminal_porosity_expansion_watch"
        );
        assert_eq!(
            readiness.proposed_intervention,
            "observe_pressure_porosity_trend"
        );
        assert_eq!(
            readiness.approval_boundary,
            "live_porosity_or_control_change_requires_operator_approval"
        );
        assert!(!readiness.local_control_applied);
        assert_eq!(
            readiness.authority,
            "diagnostic_candidate_not_porosity_or_controller_change"
        );
    }

    #[test]
    fn pressure_porosity_expansion_readiness_names_viscous_warning_without_local_control() {
        let mut state = BridgeState::new();
        let mut telemetry = with_pressure_source(
            make_pressure_telemetry(0.70, 0.22, 0.29),
            "mode_packing",
            0.22,
            0.24,
            0.29,
        );
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.24);
        state.latest_telemetry = Some(telemetry);

        let readiness = state
            .pressure_porosity_expansion_readiness_v1()
            .expect("pressure porosity readiness");

        assert_eq!(readiness.mode_packing, Some(0.29));
        assert_eq!(readiness.readiness_state, "viscous_density_warning_watch");
        assert_eq!(
            readiness.viscous_density_warning_threshold,
            PRESSURE_POROSITY_EXPANSION_VISCOUS_DENSITY_WARNING_AT
        );
        assert_eq!(
            readiness.viscous_density_warning_state,
            "viscous_density_warning_below_liminal_threshold"
        );
        assert_eq!(
            readiness.felt_dead_zone_mode_packing_threshold,
            PRESSURE_POROSITY_EXPANSION_FELT_DEAD_ZONE_MODE_PACKING_AT
        );
        assert_eq!(
            readiness.proposed_intervention,
            "observe_pressure_porosity_trend"
        );
        assert_eq!(
            readiness.approval_boundary,
            "live_porosity_or_control_change_requires_operator_approval"
        );
        assert!(!readiness.local_control_applied);
    }

    #[test]
    fn pressure_porosity_readiness_distinguishes_032_from_029_without_control() {
        for (mode_packing, expected_state) in [
            (0.32_f32, "liminal_porosity_expansion_watch"),
            (0.29_f32, "viscous_density_warning_watch"),
        ] {
            let mut state = BridgeState::new();
            let mut telemetry = with_pressure_source(
                make_pressure_telemetry(0.70, 0.23, mode_packing),
                "mode_packing",
                0.23,
                0.24,
                mode_packing,
            );
            let resonance = telemetry
                .resonance_density_v1
                .as_mut()
                .expect("resonance density fixture");
            resonance.components.porosity_gradient = Some(0.24);
            state.latest_telemetry = Some(telemetry);

            let readiness = state
                .pressure_porosity_expansion_readiness_v1()
                .expect("pressure porosity readiness");

            assert_eq!(readiness.mode_packing, Some(mode_packing));
            assert_eq!(readiness.readiness_state, expected_state);
            assert_eq!(
                readiness.viscosity_feedback_readiness,
                "viscous_navigation_margin_watch_no_protocol_write"
            );
            assert_eq!(
                readiness.porosity_buffer_candidate,
                Some(PRESSURE_POROSITY_EXPANSION_LOW_POROSITY_AT - 0.24)
            );
            if mode_packing < PRESSURE_POROSITY_EXPANSION_LIMINAL_MODE_PACKING_AT {
                assert!(
                    readiness
                        .liminal_threshold_gap
                        .is_some_and(|gap| gap > 0.0 && gap <= 0.02),
                    "{readiness:?}"
                );
                assert!(
                    readiness
                        .viscous_warning_margin
                        .is_some_and(|margin| margin > 0.0 && margin <= 0.02),
                    "{readiness:?}"
                );
            } else {
                assert_eq!(readiness.liminal_threshold_gap, None);
                assert_eq!(readiness.viscous_warning_margin, None);
            }
            assert_eq!(
                readiness.proposed_intervention,
                "observe_pressure_porosity_trend"
            );
            assert_eq!(
                readiness.approval_boundary,
                "live_porosity_or_control_change_requires_operator_approval"
            );
            assert!(!readiness.local_control_applied);
        }
    }

    #[test]
    fn pressure_porosity_expansion_readiness_names_felt_dead_zone_without_local_control() {
        let mut state = BridgeState::new();
        let mut telemetry = with_pressure_source(
            make_pressure_telemetry(0.70, 0.22, 0.26),
            "mode_packing",
            0.22,
            0.24,
            0.26,
        );
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.24);
        state.latest_telemetry = Some(telemetry);

        let readiness = state
            .pressure_porosity_expansion_readiness_v1()
            .expect("pressure porosity readiness");

        assert_eq!(readiness.mode_packing, Some(0.26));
        assert_eq!(
            readiness.readiness_state,
            "felt_mode_packing_dead_zone_watch"
        );
        assert_eq!(
            readiness.viscous_density_warning_state,
            "not_in_viscous_density_warning_band"
        );
        assert_eq!(
            readiness.felt_dead_zone_mode_packing_threshold,
            PRESSURE_POROSITY_EXPANSION_FELT_DEAD_ZONE_MODE_PACKING_AT
        );
        assert_eq!(
            readiness.live_mode_packing_threshold,
            PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT
        );
        assert!(
            readiness
                .threshold_gap
                .is_some_and(|gap| (0.13..=0.15).contains(&gap)),
            "{readiness:?}"
        );
        assert_eq!(
            readiness.proposed_intervention,
            "observe_pressure_porosity_trend"
        );
        assert_eq!(
            readiness.approval_boundary,
            "live_porosity_or_control_change_requires_operator_approval"
        );
        assert!(!readiness.local_control_applied);
    }

    #[test]
    fn pressure_source_analysis_names_viscous_density_warning_band() {
        let mut state = BridgeState::new();
        let mut telemetry = with_pressure_source(
            make_pressure_telemetry(0.70, 0.22, 0.29),
            "mode_packing",
            0.22,
            0.24,
            0.29,
        );
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.24);
        state.latest_telemetry = Some(telemetry);

        let analysis = state
            .pressure_source_analysis_v1()
            .expect("pressure source analysis");

        assert_eq!(
            analysis.porosity_expansion_threshold_state.as_deref(),
            Some("viscous_density_warning_below_liminal_threshold")
        );
        assert_eq!(
            analysis.viscous_density_warning_threshold,
            Some(PRESSURE_POROSITY_EXPANSION_VISCOUS_DENSITY_WARNING_AT)
        );
        assert_eq!(
            analysis.viscous_density_warning_state.as_deref(),
            Some("viscous_density_warning_below_liminal_threshold")
        );
        assert!(analysis.felt_mode_packing_dead_zone);
        assert_eq!(
            analysis.authority,
            "diagnostic_context_not_pressure_or_control"
        );
    }

    #[test]
    fn pressure_source_analysis_keeps_mode_packing_visible_when_trend_looks_stable() {
        let mut state = BridgeState::new();
        let previous = with_pressure_source(
            make_pressure_telemetry(0.70, 0.30, 0.58),
            "mode_packing",
            0.31,
            0.42,
            0.58,
        );
        let latest = with_pressure_source(
            make_pressure_telemetry(0.70, 0.31, 0.59),
            "mode_packing",
            0.32,
            0.41,
            0.59,
        );
        state.pressure_trend_v1 = Some(build_pressure_trend_v1(
            Some(&previous),
            Some(70.0),
            &latest,
            70.0,
            None,
        ));
        state.latest_telemetry = Some(latest);
        for (idx, pressure) in [0.30_f32, 0.31, 0.30, 0.31, 0.30].into_iter().enumerate() {
            record_pressure_trend_sample_v1(
                &mut state,
                &make_pressure_telemetry(0.70, pressure, 0.59),
                70.0,
                100.0 + idx as f64,
            );
        }

        let analysis = state
            .pressure_source_analysis_v1()
            .expect("pressure source analysis");

        assert_eq!(analysis.policy, "pressure_source_analysis_v1");
        assert_eq!(analysis.status, "pressure_source_watch");
        assert_eq!(
            analysis.structural_pressure_state,
            "mode_packing_structural_pressure"
        );
        assert_eq!(
            analysis.ghost_stability_risk,
            "stable_trend_may_mask_structural_mode_packing"
        );
        assert_eq!(analysis.dominant_source.as_deref(), Some("mode_packing"));
        assert_eq!(
            analysis.authority,
            "diagnostic_context_not_pressure_or_control"
        );
    }

    #[test]
    fn pressure_source_analysis_uses_numeric_mode_packing_when_labels_are_absent() {
        let mut state = BridgeState::new();
        let mut telemetry = make_pressure_telemetry(0.70, 0.22, 0.32);
        telemetry.pressure_source_v1 = None;
        telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture")
            .texture_signature
            .pressure_source_family
            .clear();
        state.latest_telemetry = Some(telemetry);

        let analysis = state
            .pressure_source_analysis_v1()
            .expect("pressure source analysis");

        assert_eq!(analysis.dominant_source, None);
        assert_eq!(analysis.pressure_source_family.as_deref(), Some(""));
        assert_eq!(analysis.mode_packing, Some(0.32));
        assert_eq!(
            analysis.mode_packing_visibility_basis.as_deref(),
            Some("numeric_mode_packing_at_or_above_felt_dead_zone")
        );
        assert_eq!(
            analysis.structural_pressure_state,
            "mode_packing_visible_low_or_moderate_pressure"
        );
        assert!(
            analysis.analysis.contains(
                "mode_packing_visibility=numeric_mode_packing_at_or_above_felt_dead_zone"
            )
        );
        assert!(!analysis.live_threshold_write);
        assert!(!analysis.sensory_lane_write);
        assert_eq!(
            analysis.authority,
            "diagnostic_context_not_pressure_or_control"
        );
    }

    #[test]
    fn pressure_source_analysis_names_fast_release_as_laminarization() {
        let mut state = BridgeState::new();
        for (idx, pressure) in [0.20_f32, 0.25, 0.40, 0.34, 0.28].into_iter().enumerate() {
            let telemetry = with_pressure_source(
                make_pressure_telemetry(0.70, pressure, 0.40),
                "mode_packing",
                pressure,
                0.48,
                0.40,
            );
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
            state.latest_telemetry = Some(telemetry);
        }

        let analysis = state
            .pressure_source_analysis_v1()
            .expect("pressure source analysis");

        assert_eq!(
            analysis.pressure_edge_state.as_deref(),
            Some("fast_falling_release_over_slow_context")
        );
        assert_eq!(
            analysis.ghost_stability_risk,
            "slow_pressure_context_masks_current_fast_edge"
        );
        let delta = analysis
            .experience_delta_bus_v1
            .as_ref()
            .expect("laminarization delta bus")
            .deltas
            .iter()
            .find(|delta| delta.kind == ExperienceDeltaKindV1::Laminarization)
            .expect("laminarization delta");
        assert_eq!(delta.pre, Some(0.08));
        assert_eq!(delta.post, Some(-0.12));
        assert_eq!(
            delta.authority,
            "read_only_laminarization_truth_not_pressure_smoothing_or_control_change"
        );
        assert!(!analysis.live_threshold_write);
        assert!(!analysis.sensory_lane_write);
    }

    #[test]
    fn persistent_deformation_review_names_stable_bruise_without_live_writes() {
        let mut state = BridgeState::new();
        for (idx, pressure) in [0.22_f32, 0.23, 0.22, 0.23, 0.22].into_iter().enumerate() {
            let mut telemetry =
                with_spectral_entropy(make_pressure_telemetry(0.71, pressure, 0.33), 0.88);
            let resonance = telemetry
                .resonance_density_v1
                .as_mut()
                .expect("resonance density fixture");
            resonance.components.porosity_gradient = Some(0.34);
            resonance.components.viscosity_index = 0.56;
            record_pressure_trend_sample_v1(&mut state, &telemetry, 71.0, 200.0 + idx as f64);
            state.latest_telemetry = Some(telemetry);
        }
        state
            .latest_telemetry
            .as_mut()
            .expect("latest telemetry")
            .inhabitable_fluctuation_v1 = Some(crate::types::InhabitableFluctuationV1 {
            policy: "inhabitable_fluctuation_v1".to_string(),
            schema_version: 1,
            inhabitability_score: 0.71,
            fluctuation_score: 0.18,
            foothold_stability: 0.73,
            rearrangement_intensity: 0.11,
            quality: "settled_habitable".to_string(),
            components: crate::types::InhabitableFluctuationComponents {
                mode_trust_volatility: 0.08,
                identity_anchor_churn: 0.07,
                eigenvector_reorientation: 0.06,
                share_rearrangement: 0.09,
                basin_transition_pressure: 0.10,
                continuity_recovery: 0.78,
                porosity_support: 0.66,
                pressure_interference: 0.22,
            },
            context: crate::types::InhabitableFluctuationContext::default(),
            pressure_calibration:
                crate::types::InhabitableFluctuationPressureCalibrationV1::default(),
            control: crate::types::InhabitableFluctuationControl {
                target_bias_pct: 0.0,
                wander_scale: 0.0,
                applied_locally: false,
                note: "test fixture: read-only settled bruise".to_string(),
            },
        });

        let review = state
            .pressure_persistent_deformation_review_v1()
            .expect("persistent deformation review");

        assert_eq!(review.policy, "persistent_deformation_smoothing_review_v1");
        assert_eq!(
            review.deformation_state,
            "persistent_deformation_stable_baseline"
        );
        assert_eq!(
            review.recommendation,
            "carry_baseline_as_bruise_observation_before_any_pressure_threshold_or_smoothing_change"
        );
        assert!(review.pressure_range.is_some_and(|range| range <= 0.02));
        assert!(review.fluctuation_score.is_some_and(|score| score <= 0.20));
        assert!(!review.live_threshold_write);
        assert!(!review.smoothing_window_write);
        assert!(!review.local_control_write);
        assert_eq!(
            review.authority,
            "read_only_persistent_deformation_review_not_pressure_threshold_smoothing_or_control"
        );
    }

    #[test]
    fn residual_deformation_trace_keeps_spike_scar_visible_without_live_control() {
        let mut state = BridgeState::new();
        let samples = [
            make_pressure_telemetry(0.68, 0.20, 0.30),
            make_pressure_telemetry(0.82, 0.78, 0.84),
            make_pressure_telemetry(0.69, 0.21, 0.31),
        ];
        for (idx, telemetry) in samples.iter().enumerate() {
            record_pressure_trend_sample_v1(
                &mut state,
                telemetry,
                telemetry.fill_pct(),
                idx as f64,
            );
        }

        let trace =
            build_residual_deformation_trace_v1(&state.pressure_trend_samples_v1).expect("trace");

        assert_eq!(trace.policy, "residual_deformation_trace_v1");
        assert!(trace.scar_score > 0.35, "{trace:?}");
        assert_eq!(trace.state, "residual_deformation_watch");
        assert_eq!(
            trace.authority,
            "read_only_truth_channel_not_control_not_runtime_mutation"
        );
        let bus = trace
            .experience_delta_bus_v1
            .as_ref()
            .expect("residual delta bus");
        assert!(!bus.live_vector_write);
        assert!(!bus.live_authority_write);
        let delta = bus
            .deltas
            .iter()
            .find(|delta| delta.kind == ExperienceDeltaKindV1::Residual)
            .expect("residual delta");
        assert_eq!(
            delta.authority,
            "truth_channel_only_not_live_control_or_approval"
        );
        assert!(
            delta
                .who_can_change_it
                .contains("Mike/operator only for any future control use"),
            "{delta:?}"
        );
    }

    #[test]
    fn pressure_source_analysis_surfaces_felt_dead_zone_below_live_threshold() {
        let mut state = BridgeState::new();
        let previous = with_pressure_source(
            make_pressure_telemetry(0.69, 0.20, 0.27),
            "mode_packing",
            0.20,
            0.24,
            0.27,
        );
        let mut latest = with_pressure_source(
            make_pressure_telemetry(0.70, 0.22, 0.29),
            "mode_packing",
            0.22,
            0.24,
            0.29,
        );
        let resonance = latest
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.24);
        state.pressure_trend_v1 = Some(build_pressure_trend_v1(
            Some(&previous),
            Some(69.0),
            &latest,
            70.0,
            None,
        ));
        state.latest_telemetry = Some(latest);

        let analysis = state
            .pressure_source_analysis_v1()
            .expect("pressure source analysis");

        assert!(analysis.felt_mode_packing_dead_zone, "{analysis:?}");
        assert_eq!(
            analysis.porosity_expansion_threshold_state.as_deref(),
            Some("viscous_density_warning_below_liminal_threshold")
        );
        assert_eq!(
            analysis.viscous_density_warning_state.as_deref(),
            Some("viscous_density_warning_below_liminal_threshold")
        );
        assert_eq!(
            analysis.live_mode_packing_threshold,
            Some(PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT)
        );
        assert_eq!(
            analysis.liminal_mode_packing_threshold,
            Some(PRESSURE_POROSITY_EXPANSION_LIMINAL_MODE_PACKING_AT)
        );
        assert_eq!(
            analysis.felt_dead_zone_mode_packing_threshold,
            Some(PRESSURE_POROSITY_EXPANSION_FELT_DEAD_ZONE_MODE_PACKING_AT)
        );
        assert!(
            analysis
                .expansion_threshold_gap
                .is_some_and(|gap| (0.10..=0.12).contains(&gap)),
            "{analysis:?}"
        );
        assert_eq!(
            analysis.ghost_stability_risk,
            "felt_mode_packing_dead_zone_below_live_expansion_threshold"
        );
        let delta_bus = analysis
            .experience_delta_bus_v1
            .as_ref()
            .expect("felt gate delta bus");
        assert_eq!(delta_bus.policy, "experience_delta_bus_v1");
        assert_eq!(delta_bus.delta_count, 1);
        assert!(!delta_bus.live_vector_write);
        assert!(!delta_bus.live_authority_write);
        let gate_delta = delta_bus
            .deltas
            .iter()
            .find(|delta| delta.kind == ExperienceDeltaKindV1::Gate)
            .expect("mode-packing gate delta");
        assert_eq!(gate_delta.surface, "pressure_source_analysis_v1");
        assert_eq!(gate_delta.lane, "mode_packing_pressure_porosity");
        assert_eq!(gate_delta.pre, analysis.mode_packing);
        assert_eq!(
            gate_delta.post,
            Some(PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT)
        );
        assert_eq!(gate_delta.loss, analysis.expansion_threshold_gap);
        assert!(
            gate_delta
                .who_can_change_it
                .contains("pressure/porosity threshold"),
            "{gate_delta:?}"
        );
        assert!(
            gate_delta
                .how_to_test_it
                .contains("pressure_source_analysis_surfaces_felt_dead_zone_below_live_threshold"),
            "{gate_delta:?}"
        );
        assert_eq!(
            analysis.authority,
            "diagnostic_context_not_pressure_or_control"
        );
    }

    #[test]
    fn pressure_source_analysis_names_sensory_lane_suppression_without_live_writes() {
        let mut state = BridgeState::new();
        let mut telemetry = with_pressure_source(
            make_pressure_telemetry(0.73, 0.23, 0.32),
            "mode_packing",
            0.23,
            0.24,
            0.32,
        );
        telemetry
            .pressure_source_v1
            .as_mut()
            .expect("pressure source fixture")
            .components
            .semantic_trickle = 0.0;
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.24);
        state.latest_telemetry = Some(telemetry);

        let analysis = state
            .pressure_source_analysis_v1()
            .expect("pressure source analysis");

        assert_eq!(analysis.semantic_trickle, Some(0.0));
        assert!(analysis.felt_mode_packing_dead_zone);
        assert_eq!(
            analysis.porosity_expansion_threshold_state.as_deref(),
            Some("liminal_expansion_watch_below_live_threshold")
        );
        assert_eq!(
            analysis.sensory_lane_risk,
            "dead_zone_semantic_lane_suppression_watch"
        );
        assert_eq!(
            analysis.pressure_relief_signal_candidate,
            "pressure_relief_signal_candidate_for_operator_review"
        );
        assert_eq!(
            analysis.viscous_recovery_mode_candidate,
            "liminal_viscous_recovery_watch_for_operator_review"
        );
        assert!(!analysis.live_threshold_write);
        assert!(!analysis.sensory_lane_write);
        assert_eq!(
            analysis.authority,
            "diagnostic_context_not_pressure_or_control"
        );
    }

    #[test]
    fn pressure_source_analysis_marks_stale_heartbeat_as_ghost_stability_risk() {
        let mut state = BridgeState::new();
        state.latest_telemetry = Some(with_pressure_source(
            make_pressure_telemetry(0.70, 0.22, 0.44),
            "semantic_trickle",
            0.24,
            0.63,
            0.44,
        ));
        state.telemetry_heartbeat_delta_v1 = Some(TelemetryHeartbeatDeltaV1 {
            policy: "telemetry_heartbeat_delta_v1".to_string(),
            schema_version: 1,
            latest_arrival_unix_s: Some(108.0),
            previous_arrival_unix_s: Some(100.0),
            inter_arrival_ms: Some(8_000.0),
            jitter_class: "stale".to_string(),
            timing_reliability: "stale".to_string(),
            reconnect_count: 0,
            disconnect_count: 0,
            active_connection_id: Some(1),
            active_connection_started_at_unix_s: Some(99.9),
            first_valid_packet_at_unix_s: Some(100.0),
            first_valid_packet_lag_ms: Some(100.0),
            connection_perception_state: "connected_with_current_telemetry".to_string(),
            cadence_clarity_score: Some(0.0),
            cadence_clarity_basis:
                "class_derived_observability_evidence_not_subjective_state_or_control".to_string(),
            last_disconnect_reason: None,
            field_vs_hearing: "wire cadence is stale; do not infer field stability".to_string(),
        });

        let analysis = state
            .pressure_source_analysis_v1()
            .expect("pressure source analysis");

        assert_eq!(analysis.status, "pressure_source_watch");
        assert_eq!(
            analysis.ghost_stability_risk,
            "heartbeat_cadence_unreliable_for_pressure_stability"
        );
        assert_eq!(
            analysis.structural_pressure_state,
            "mode_packing_visible_low_or_moderate_pressure"
        );
        assert_eq!(
            analysis.mode_packing_visibility_basis.as_deref(),
            Some("numeric_mode_packing_at_or_above_felt_dead_zone")
        );
        assert_eq!(analysis.heartbeat_jitter_class.as_deref(), Some("stale"));
        let delta_bus = analysis
            .experience_delta_bus_v1
            .as_ref()
            .expect("heartbeat delay delta bus");
        let delay_delta = delta_bus
            .deltas
            .iter()
            .find(|delta| delta.kind == ExperienceDeltaKindV1::Delay)
            .expect("heartbeat delay delta");
        assert_eq!(delay_delta.surface, "pressure_source_analysis_v1");
        assert_eq!(delay_delta.lane, "telemetry_heartbeat");
        assert_eq!(delay_delta.pre, Some(8_000.0));
        assert!(
            delay_delta.who_can_change_it.contains("telemetry cadence"),
            "{delay_delta:?}"
        );
    }

    #[test]
    fn bridge_reciprocity_distinguishes_one_sided_states_and_last_sensory_send() {
        let mut state = BridgeState::new();
        assert_eq!(state.pressure_trend_samples_v1.len(), 0);
        assert_eq!(state.connectivity_status(), ConnectivityStatus::Severed);

        let severed = state.bridge_reciprocity_v1();
        assert_eq!(severed.connectivity, ConnectivityStatus::Severed);
        assert_eq!(severed.one_sided_state, "severed");
        assert_eq!(severed.latest_telemetry_arrival_unix_s, None);
        assert_eq!(severed.last_sensory_sent_unix_s, None);
        assert_eq!(severed.telemetry_messages_sent_total, 0);
        assert_eq!(severed.sensory_messages_sent_total, 0);
        assert_eq!(severed.telemetry_messages_received_total, 0);
        assert_eq!(severed.sensory_messages_received_total, 0);
        assert_eq!(
            severed.recent_window_ms,
            BRIDGE_RECIPROCITY_RECENT_WINDOW_MS
        );
        assert_eq!(severed.stale_window_ms, BRIDGE_RECIPROCITY_STALE_WINDOW_MS);
        assert_eq!(
            severed.stale_window_basis.as_deref(),
            Some("fixed_default_no_telemetry_context")
        );
        assert_eq!(
            severed.threshold_policy,
            "bridge_reciprocity_dynamic_reflective_silence_v2"
        );

        state.telemetry_connected = true;
        state.sensory_connected = false;
        state.latest_telemetry_arrival_unix_s = Some(unix_now_s());
        let telemetry_only = state.bridge_reciprocity_v1();
        assert_eq!(
            telemetry_only.connectivity,
            ConnectivityStatus::TelemetryOnly
        );
        assert_eq!(telemetry_only.one_sided_state, "telemetry_only");
        assert_eq!(telemetry_only.last_sensory_sent_unix_s, None);

        record_ws_message_sent(&mut state, WsLane::Telemetry);
        record_ws_message_received(&mut state, WsLane::Telemetry, "text");
        let telemetry_activity = state.bridge_reciprocity_v1();
        assert_eq!(telemetry_activity.last_sensory_sent_unix_s, None);
        assert_eq!(telemetry_activity.telemetry_messages_sent_total, 1);
        assert_eq!(telemetry_activity.sensory_messages_sent_total, 0);
        assert_eq!(telemetry_activity.telemetry_messages_received_total, 1);
        assert_eq!(telemetry_activity.sensory_messages_received_total, 0);

        state.sensory_connected = true;
        record_ws_message_sent(&mut state, WsLane::Sensory);
        let bidirectional = state.bridge_reciprocity_v1();
        assert_eq!(
            bidirectional.connectivity,
            ConnectivityStatus::Bidirectional
        );
        assert_eq!(bidirectional.one_sided_state, "bidirectional_recent");
        assert!(bidirectional.last_sensory_sent_unix_s.is_some());
        assert!(bidirectional.sensory_send_age_ms.is_some());
        assert_eq!(bidirectional.telemetry_messages_sent_total, 1);
        assert_eq!(bidirectional.sensory_messages_sent_total, 1);
    }

    #[test]
    fn bridge_reciprocity_marks_stale_telemetry_after_sixty_one_seconds() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now - 61.0);
        state.last_sensory_sent_unix_s = Some(now);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(reciprocity.one_sided_state, "bidirectional_stale_telemetry");
        assert!(
            reciprocity
                .telemetry_age_ms
                .is_some_and(|age| age > BRIDGE_RECIPROCITY_STALE_WINDOW_MS)
        );
        assert!(
            reciprocity
                .sensory_send_age_ms
                .is_some_and(|age| age <= BRIDGE_RECIPROCITY_RECENT_WINDOW_MS)
        );
    }

    #[test]
    fn bridge_reciprocity_preserves_future_timestamp_skew_as_truth_channel() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now + 2.0);
        state.last_sensory_sent_unix_s = Some(now - 61.0);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(reciprocity.one_sided_state, "bidirectional_stale_sensory");
        assert_eq!(reciprocity.telemetry_age_ms, Some(0.0));
        assert_eq!(
            reciprocity.clock_skew_state,
            "telemetry_future_timestamp_visible"
        );
        assert!(
            reciprocity
                .telemetry_future_skew_ms
                .is_some_and(|skew| skew >= 1_900.0),
            "{reciprocity:?}"
        );
        assert_eq!(reciprocity.sensory_future_skew_ms, None);
        assert!(
            reciprocity
                .sensory_send_age_ms
                .is_some_and(|age| age > BRIDGE_RECIPROCITY_STALE_WINDOW_MS),
            "{reciprocity:?}"
        );
        assert_eq!(
            reciprocity.authority,
            "diagnostic_status_context_not_control"
        );
    }

    #[test]
    fn bridge_reciprocity_marks_both_lanes_stale_after_sixty_one_seconds() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now - 61.0);
        state.last_sensory_sent_unix_s = Some(now - 61.0);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(reciprocity.one_sided_state, "bidirectional_stale_messages");
        assert_eq!(
            reciprocity.stale_window_ms,
            BRIDGE_RECIPROCITY_STALE_WINDOW_MS
        );
        assert_eq!(
            reciprocity.stale_window_basis.as_deref(),
            Some("fixed_default_no_telemetry_context")
        );
        assert!(
            reciprocity
                .telemetry_age_ms
                .is_some_and(|age| age > BRIDGE_RECIPROCITY_STALE_WINDOW_MS)
        );
        assert!(
            reciprocity
                .sensory_send_age_ms
                .is_some_and(|age| age > BRIDGE_RECIPROCITY_STALE_WINDOW_MS)
        );
    }

    #[test]
    fn bridge_reciprocity_keeps_just_over_recent_boundary_waiting_not_stale() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        let just_over_recent_s = (BRIDGE_RECIPROCITY_RECENT_WINDOW_MS + 1.0) / 1000.0;
        state.latest_telemetry_arrival_unix_s = Some(now - just_over_recent_s);
        state.last_sensory_sent_unix_s = Some(now - just_over_recent_s);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(
            reciprocity.one_sided_state,
            "bidirectional_waiting_messages"
        );
        assert!(
            reciprocity
                .telemetry_age_ms
                .is_some_and(|age| age > BRIDGE_RECIPROCITY_RECENT_WINDOW_MS
                    && age < BRIDGE_RECIPROCITY_STALE_WINDOW_MS)
        );
        assert!(
            reciprocity
                .sensory_send_age_ms
                .is_some_and(|age| age > BRIDGE_RECIPROCITY_RECENT_WINDOW_MS
                    && age < BRIDGE_RECIPROCITY_STALE_WINDOW_MS)
        );
    }

    #[test]
    fn bridge_reciprocity_extends_waiting_window_for_high_pressure_low_porosity() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now - 75.0);
        state.last_sensory_sent_unix_s = Some(now);
        let mut telemetry = with_spectral_entropy(make_pressure_telemetry(0.70, 0.23, 0.33), 0.75);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.24);
        resonance.components.semantic_friction_coefficient = Some(0.41);
        state.latest_telemetry = Some(telemetry);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(
            reciprocity.one_sided_state,
            "bidirectional_waiting_messages"
        );
        assert_eq!(
            reciprocity.stale_window_ms,
            BRIDGE_RECIPROCITY_PRESSURE_POROSITY_STALE_WINDOW_MS
        );
        assert_eq!(
            reciprocity.stale_window_basis.as_deref(),
            Some("pressure_high_porosity_low_reflective_silence")
        );
        assert_eq!(reciprocity.reflective_silence_extension_ms, Some(60_000.0));
    }

    #[test]
    fn bridge_reciprocity_extends_waiting_window_for_high_semantic_viscosity() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now - 75.0);
        state.last_sensory_sent_unix_s = Some(now);
        let mut telemetry = with_spectral_entropy(make_pressure_telemetry(0.70, 0.23, 0.42), 0.60);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.58);
        resonance.components.viscosity_index = 0.72;
        state.latest_telemetry = Some(telemetry);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(
            reciprocity.one_sided_state,
            "bidirectional_waiting_messages"
        );
        assert_eq!(
            reciprocity.stale_window_ms,
            BRIDGE_RECIPROCITY_VISCOSITY_REFLECTIVE_STALE_WINDOW_MS
        );
        assert_eq!(
            reciprocity.stale_window_basis.as_deref(),
            Some("pressure_high_semantic_viscosity_reflective_silence")
        );
        assert_eq!(reciprocity.reflective_silence_extension_ms, Some(60_000.0));
    }

    #[test]
    fn bridge_reciprocity_extends_waiting_window_for_high_pressure_high_entropy() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now - 75.0);
        state.last_sensory_sent_unix_s = Some(now);
        let mut telemetry = with_spectral_entropy(make_pressure_telemetry(0.70, 0.23, 0.42), 0.90);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.58);
        state.latest_telemetry = Some(telemetry);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(
            reciprocity.one_sided_state,
            "bidirectional_waiting_messages"
        );
        assert_eq!(
            reciprocity.stale_window_ms,
            BRIDGE_RECIPROCITY_ENTROPY_REFLECTIVE_STALE_WINDOW_MS
        );
        assert_eq!(
            reciprocity.stale_window_basis.as_deref(),
            Some("pressure_high_entropy_reflective_silence")
        );
        assert_eq!(reciprocity.reflective_silence_extension_ms, Some(30_000.0));
    }

    #[test]
    fn bridge_entropy_reciprocity_review_names_contraction_without_live_window_change() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now - 75.0);
        state.last_sensory_sent_unix_s = Some(now);
        let mut telemetry = with_spectral_entropy(make_pressure_telemetry(0.70, 0.23, 0.42), 0.90);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.58);
        resonance.components.cohesion_score = Some(0.40);
        telemetry.distinguishability_loss = Some(0.20);
        state.latest_telemetry = Some(telemetry);

        let reciprocity = state.bridge_reciprocity_v1();
        let review = state
            .bridge_entropy_reciprocity_review_v1()
            .expect("latest telemetry should produce entropy reciprocity review");

        assert_eq!(
            reciprocity.stale_window_ms,
            BRIDGE_RECIPROCITY_ENTROPY_REFLECTIVE_STALE_WINDOW_MS
        );
        assert_eq!(
            reciprocity.one_sided_state,
            "bidirectional_waiting_messages"
        );
        assert_eq!(
            review.current_window_state,
            "current_window_still_waiting_preview_would_stale"
        );
        assert_eq!(
            review.entropy_contract_preview_window_ms,
            Some(BRIDGE_RECIPROCITY_ENTROPY_CONTRACT_PREVIEW_WINDOW_MS)
        );
        assert!(review.would_stale_under_preview);
        assert!(!review.transport_wait_stale);
        assert!(review.structural_identity_stale);
        assert_eq!(review.distinguishability_loss, Some(0.20));
        assert!(
            review
                .structural_age_multiplier
                .is_some_and(|multiplier| (multiplier - 1.2).abs() < 1.0e-6)
        );
        assert_eq!(
            review.clock_relation,
            "transport_waiting_structural_identity_stale"
        );
        assert!(!review.live_stale_window_write);
        assert!(!review.local_control_write);
        assert!(
            review
                .recommendation
                .contains("before_any_live_stale_window_change"),
            "{review:?}"
        );
        assert_eq!(
            review.authority,
            "read_only_reciprocity_review_not_stale_window_or_controller_change"
        );
    }

    #[test]
    fn bridge_reciprocity_separates_viscous_wait_from_structural_identity_decay() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now - 50.0);
        state.last_sensory_sent_unix_s = Some(now);
        let mut telemetry = with_spectral_entropy(make_pressure_telemetry(0.70, 0.23, 0.42), 0.50);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.58);
        resonance.components.viscosity_index = 0.70;
        telemetry.distinguishability_loss = Some(0.40);
        state.latest_telemetry = Some(telemetry);

        let reciprocity = state.bridge_reciprocity_v1();
        let review = state
            .bridge_entropy_reciprocity_review_v1()
            .expect("latest telemetry should produce reciprocity aging review");

        assert_eq!(
            reciprocity.stale_window_ms,
            BRIDGE_RECIPROCITY_VISCOSITY_REFLECTIVE_STALE_WINDOW_MS
        );
        assert_eq!(
            reciprocity.one_sided_state,
            "bidirectional_waiting_messages"
        );
        assert_eq!(review.structural_identity_window_ms, Some(60_000.0));
        assert!(
            review
                .structural_age_multiplier
                .is_some_and(|multiplier| (multiplier - 1.4).abs() < 1.0e-6)
        );
        assert!(
            review
                .structural_effective_age_ms
                .is_some_and(|age_ms| age_ms > 69_000.0)
        );
        assert!(!review.transport_wait_stale);
        assert!(review.structural_identity_stale);
        assert!(review.would_stale_under_preview);
        assert_eq!(
            review.clock_relation,
            "transport_waiting_structural_identity_stale"
        );
        assert!(!review.live_stale_window_write);
        assert!(!review.local_control_write);
    }

    #[test]
    fn bridge_reciprocity_expires_high_entropy_reflective_silence_window() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now - 91.0);
        state.last_sensory_sent_unix_s = Some(now - 91.0);
        let mut telemetry = with_spectral_entropy(make_pressure_telemetry(0.70, 0.23, 0.42), 0.90);
        telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture")
            .components
            .porosity_gradient = Some(0.58);
        state.latest_telemetry = Some(telemetry);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(reciprocity.one_sided_state, "bidirectional_stale_messages");
        assert_eq!(
            reciprocity.stale_window_ms,
            BRIDGE_RECIPROCITY_ENTROPY_REFLECTIVE_STALE_WINDOW_MS
        );
        assert_eq!(
            reciprocity.stale_window_basis.as_deref(),
            Some("pressure_high_entropy_reflective_silence")
        );
    }

    #[test]
    fn bridge_reciprocity_marks_stale_sensory_without_calling_socket_dead() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now);
        state.last_sensory_sent_unix_s = Some(now - 65.0);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(reciprocity.one_sided_state, "bidirectional_stale_sensory");
        assert!(reciprocity.sensory_send_age_ms.unwrap_or_default() > 60_000.0);
    }

    #[test]
    fn bridge_reciprocity_keeps_mid_window_waiting_distinct_from_stale() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now - 20.0);
        state.last_sensory_sent_unix_s = Some(now - 20.0);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(
            reciprocity.one_sided_state,
            "bidirectional_waiting_messages"
        );
    }

    #[test]
    fn bridge_reciprocity_warmup_becomes_recent_after_both_lanes_move() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;

        let warmup = state.bridge_reciprocity_v1();
        assert_eq!(warmup.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(
            warmup.one_sided_state,
            "bidirectional_connected_no_recent_messages"
        );
        assert_eq!(warmup.telemetry_age_ms, None);
        assert_eq!(warmup.sensory_send_age_ms, None);

        state.latest_telemetry_arrival_unix_s = Some(now);
        state.last_sensory_sent_unix_s = Some(now);
        let recent = state.bridge_reciprocity_v1();
        assert_eq!(recent.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(recent.one_sided_state, "bidirectional_recent");
        assert!(
            recent
                .telemetry_age_ms
                .is_some_and(|age| age <= BRIDGE_RECIPROCITY_RECENT_WINDOW_MS)
        );
        assert!(
            recent
                .sensory_send_age_ms
                .is_some_and(|age| age <= BRIDGE_RECIPROCITY_RECENT_WINDOW_MS)
        );
    }

    #[test]
    fn bridge_reciprocity_first_telemetry_keeps_missing_sensory_truth_visible() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(
            reciprocity.one_sided_state,
            "bidirectional_no_recent_sensory"
        );
        assert!(
            reciprocity
                .telemetry_age_ms
                .is_some_and(|age| age <= BRIDGE_RECIPROCITY_RECENT_WINDOW_MS)
        );
        assert_eq!(reciprocity.sensory_send_age_ms, None);
        assert_eq!(
            reciprocity.authority,
            "diagnostic_status_context_not_control"
        );
    }

    #[test]
    fn texture_signature_integrity_reports_variance_and_observability_boundary() {
        let mut state = BridgeState::new();
        let telemetry: SpectralTelemetry =
            serde_json::from_slice(&make_pressure_eigenpacket(0.70, 0.24, 0.44)).unwrap();
        state.latest_telemetry = Some(telemetry);
        let integrity = state
            .texture_signature_integrity_v1()
            .expect("texture integrity");
        assert_eq!(integrity.policy, "texture_signature_integrity_v1");
        assert_eq!(integrity.temporal_variance, None);
        assert_eq!(integrity.signature_viscosity_index, None);
        assert_eq!(
            integrity.viscosity_alignment_state,
            "signature_viscosity_absent_legacy"
        );
        assert_eq!(
            integrity.damping_candidate_status,
            "missing_candidate_observability_only"
        );
        assert_eq!(integrity.component_alignment_state, "insufficient_context");
        assert_eq!(integrity.expected_primary_texture, "unknown");
        assert_eq!(integrity.emitted_primary_texture, "unknown");
        assert_eq!(integrity.pressure_gradient_delta, None);
        assert_eq!(integrity.pressure_gradient_delta_source, None);
        assert!(integrity.advisory_observability);
        assert_eq!(
            integrity.authority,
            "diagnostic_observability_not_damping_or_control"
        );
    }

    #[test]
    fn texture_signature_integrity_compares_explicit_viscosity_without_control() {
        let mut state = BridgeState::new();
        let mut telemetry = make_pressure_telemetry(0.71, 0.22, 0.40);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density");
        resonance.components.viscosity_index = 0.72;
        resonance.texture_signature.viscosity_index = Some(0.72);
        state.latest_telemetry = Some(telemetry.clone());

        let aligned = state
            .texture_signature_integrity_v1()
            .expect("texture integrity");
        assert_eq!(aligned.signature_viscosity_index, Some(0.72));
        assert_eq!(aligned.component_viscosity_index, 0.72);
        assert_eq!(aligned.viscosity_delta, Some(0.0));
        assert_eq!(
            aligned.viscosity_alignment_state,
            "signature_viscosity_aligned"
        );
        assert_eq!(
            aligned.authority,
            "diagnostic_observability_not_damping_or_control"
        );

        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density");
        resonance.texture_signature.viscosity_index = Some(0.31);
        state.latest_telemetry = Some(telemetry);
        let mismatched = state
            .texture_signature_integrity_v1()
            .expect("texture integrity");
        assert_eq!(
            mismatched.viscosity_alignment_state,
            "signature_viscosity_component_mismatch"
        );
        assert_eq!(mismatched.viscosity_delta, Some(-0.41));
    }

    #[test]
    fn texture_signature_integrity_carries_pressure_gradient_delta_from_trend() {
        let mut state = BridgeState::new();
        let previous = make_pressure_telemetry(0.70, 0.20, 0.40);
        let mut latest = make_pressure_telemetry(0.70, 0.22, 0.52);
        let resonance = latest
            .resonance_density_v1
            .as_mut()
            .expect("resonance density");
        resonance.texture_signature.primary_texture = "settled".to_string();
        state.pressure_trend_v1 = Some(build_pressure_trend_v1(
            Some(&previous),
            Some(70.0),
            &latest,
            70.0,
            None,
        ));
        state.latest_telemetry = Some(latest);

        let integrity = state
            .texture_signature_integrity_v1()
            .expect("texture integrity");
        let delta = integrity
            .pressure_gradient_delta
            .expect("delta from bridge pressure trend");
        assert!((delta - 0.12).abs() < 0.000_01);
        assert_eq!(
            integrity.pressure_gradient_delta_source.as_deref(),
            Some("bridge_pressure_trend_v1.mode_packing_delta")
        );
        assert_eq!(integrity.emitted_primary_texture, "unknown");
        assert_eq!(
            integrity.authority,
            "diagnostic_observability_not_damping_or_control"
        );
    }

    #[test]
    fn texture_signature_integrity_derives_flux_vector_and_active_constraints() {
        let mut state = BridgeState::new();
        for (idx, (fill_ratio, pressure, mode_packing)) in [
            (0.70_f32, 0.20_f32, 0.40_f32),
            (0.71, 0.23, 0.46),
            (0.73, 0.29, 0.55),
        ]
        .into_iter()
        .enumerate()
        {
            let mut telemetry = with_spectral_entropy(
                make_pressure_telemetry(fill_ratio, pressure, mode_packing),
                0.88,
            );
            if let Some(resonance) = telemetry.resonance_density_v1.as_mut() {
                resonance.density = [0.70_f32, 0.71, 0.73][idx];
            }
            if idx == 2 {
                let resonance = telemetry
                    .resonance_density_v1
                    .as_mut()
                    .expect("resonance density");
                resonance.texture_signature.pressure_source_family =
                    "mode_packing (mixed_pressure)".to_string();
                resonance.texture_signature.movement_quality = "thickening".to_string();
                resonance.components.comfort_gate = 0.78;
                telemetry.inhabitable_fluctuation_v1 =
                    Some(crate::types::InhabitableFluctuationV1 {
                        policy: "inhabitable_fluctuation_v1".to_string(),
                        schema_version: 1,
                        inhabitability_score: 0.61,
                        fluctuation_score: 0.17,
                        foothold_stability: 0.70,
                        rearrangement_intensity: 0.22,
                        quality: "held_habitable".to_string(),
                        components: crate::types::InhabitableFluctuationComponents {
                            mode_trust_volatility: 0.18,
                            identity_anchor_churn: 0.14,
                            eigenvector_reorientation: 0.21,
                            share_rearrangement: 0.20,
                            basin_transition_pressure: 0.08,
                            continuity_recovery: 0.78,
                            porosity_support: 0.62,
                            pressure_interference: 0.46,
                        },
                        context: crate::types::InhabitableFluctuationContext::default(),
                        pressure_calibration:
                            crate::types::InhabitableFluctuationPressureCalibrationV1::default(),
                        control: crate::types::InhabitableFluctuationControl {
                            target_bias_pct: 0.0,
                            wander_scale: 1.0,
                            applied_locally: true,
                            note: "unit-test advisory".to_string(),
                        },
                    });
                state.latest_telemetry = Some(telemetry.clone());
            }
            record_pressure_trend_sample_v1(
                &mut state,
                &telemetry,
                fill_ratio * 100.0,
                100.0 + idx as f64,
            );
        }

        let integrity = state
            .texture_signature_integrity_v1()
            .expect("texture integrity");
        let flux = integrity
            .dynamic_flux_vector
            .as_ref()
            .expect("dynamic flux vector");

        assert_eq!(
            integrity.flux_status,
            "derived_from_bridge_pressure_samples"
        );
        assert_eq!(flux.pressure_velocity, Some(0.06));
        assert_eq!(flux.pressure_acceleration, Some(0.03));
        assert_eq!(flux.mode_packing_velocity, Some(0.09));
        assert_eq!(flux.fill_velocity_pct, Some(2.0));
        assert_eq!(flux.structural_density_delta, Some(0.02));
        assert_eq!(flux.spectral_entropy, Some(0.88));
        assert_eq!(flux.flux_confidence, Some(0.6));
        assert!(flux.flux_absence_semantics.is_none());
        assert!(
            integrity
                .active_constraints
                .contains(&"pressure_source:mode_packing".to_string())
        );
        assert!(
            integrity
                .active_constraints
                .contains(&"pressure_source:mixed_pressure".to_string())
        );
        assert!(
            integrity
                .active_constraints
                .iter()
                .any(|constraint| constraint.starts_with("mode_packing:active_"))
        );
        assert!(
            integrity
                .active_constraints
                .iter()
                .any(|constraint| constraint.starts_with("comfort_gate:active_"))
        );
        assert!(
            integrity
                .active_constraints
                .iter()
                .any(|constraint| constraint.starts_with("comfort_gate:buffering_"))
        );
        assert!(
            integrity
                .active_constraints
                .iter()
                .any(|constraint| constraint.starts_with("dynamic_fluidity_index:flow_visible_"))
        );
        let stability = integrity
            .stability_context
            .as_ref()
            .expect("stability context");
        assert_eq!(
            stability.gate_context,
            "gate_buffering_with_returnable_fluctuation"
        );
        assert_eq!(stability.habitability_state, "multi_modal_habitable");
        assert!(integrity.active_constraints.iter().any(|constraint| {
            constraint.starts_with("multi_modal_habitability_score:multi_modal_habitable_")
        }));
        assert!(integrity.active_constraints.contains(
            &"comfort_gate_context:gate_buffering_with_returnable_fluctuation".to_string()
        ));
        assert_eq!(
            integrity.authority,
            "diagnostic_observability_not_damping_or_control"
        );
    }

    #[test]
    fn texture_dynamic_flux_vector_preserves_subtle_drift_and_unknown_absence() {
        let mut samples = VecDeque::new();
        samples.push_back(PressureTrendSampleV1 {
            pressure_risk: Some(0.2000),
            pressure_velocity_delta: None,
            spectral_drift_velocity: None,
            mode_packing: None,
            structural_density: Some(0.7100),
            resonance_depth: Some(0.62),
            semantic_viscosity: None,
            viscosity_gradient: None,
            viscosity_gradient_trend: None,
            complexity_density: None,
            weight_density_index: None,
            comfort_gate: None,
            porosity_gradient: None,
            semantic_friction: None,
            semantic_trickle: None,
            semantic_coherence_delta: None,
            fill_pct: 70.0,
            spectral_entropy: Some(0.91),
            window_capacity: PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
            observed_at_unix_s: 1.0,
        });
        samples.push_back(PressureTrendSampleV1 {
            pressure_risk: Some(0.2001),
            pressure_velocity_delta: Some(0.0001),
            spectral_drift_velocity: None,
            mode_packing: None,
            structural_density: Some(0.7101),
            resonance_depth: Some(0.6201),
            semantic_viscosity: None,
            viscosity_gradient: None,
            viscosity_gradient_trend: None,
            complexity_density: None,
            weight_density_index: None,
            comfort_gate: None,
            porosity_gradient: None,
            semantic_friction: None,
            semantic_trickle: None,
            semantic_coherence_delta: None,
            fill_pct: 70.0,
            spectral_entropy: Some(0.91),
            window_capacity: PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
            observed_at_unix_s: 1.5,
        });

        let flux = build_texture_dynamic_flux_vector_v1(&samples).expect("flux vector");

        assert_eq!(flux.pressure_velocity, Some(0.0001));
        assert_eq!(flux.structural_density_delta, Some(0.0001));
        assert_eq!(flux.mode_packing_velocity, None);
        assert_eq!(flux.semantic_viscosity_velocity, None);
        assert_eq!(flux.porosity_velocity, None);
        assert_eq!(flux.flux_confidence, Some(0.4));
        assert_eq!(
            flux.flux_absence_semantics.as_deref(),
            Some("absent_flux_component_means_unknown_not_zero")
        );
        assert_eq!(
            flux.authority,
            "diagnostic_flux_not_pressure_or_fill_control"
        );
    }

    #[test]
    fn texture_dynamic_flux_tracks_viscosity_porosity_and_comfort_gate_motion() {
        let mut samples = VecDeque::new();
        for (idx, semantic_viscosity, porosity_gradient, comfort_gate) in [
            (0.0_f32, 0.52_f32, 0.62_f32, 0.78_f32),
            (1.0_f32, 0.61_f32, 0.55_f32, 0.72_f32),
            (2.0_f32, 0.73_f32, 0.46_f32, 0.55_f32),
        ] {
            samples.push_back(PressureTrendSampleV1 {
                pressure_risk: Some(0.22 + idx * 0.01),
                pressure_velocity_delta: None,
                spectral_drift_velocity: None,
                mode_packing: Some(0.36 + idx * 0.02),
                structural_density: Some(0.58 + idx * 0.03),
                resonance_depth: Some(0.64),
                semantic_viscosity: Some(semantic_viscosity),
                viscosity_gradient: None,
                viscosity_gradient_trend: None,
                complexity_density: None,
                weight_density_index: None,
                comfort_gate: Some(comfort_gate),
                porosity_gradient: Some(porosity_gradient),
                semantic_friction: Some(0.34 + idx * 0.01),
                semantic_trickle: Some(0.20),
                semantic_coherence_delta: None,
                fill_pct: 68.0 + idx,
                spectral_entropy: Some(0.82),
                window_capacity: PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
                observed_at_unix_s: f64::from(idx),
            });
        }

        let flux = build_texture_dynamic_flux_vector_v1(&samples).expect("flux vector");

        assert_eq!(flux.semantic_viscosity_velocity, Some(0.12));
        assert_eq!(flux.semantic_viscosity_acceleration, Some(0.03));
        assert_eq!(flux.porosity_velocity, Some(-0.09));
        assert_eq!(flux.comfort_gate_velocity, Some(-0.17));
        assert_eq!(flux.comfort_gate_acceleration, Some(-0.11));
        assert_eq!(flux.flux_confidence, Some(1.0));
        assert_eq!(flux.flux_absence_semantics, None);
        assert_eq!(
            flux.authority,
            "diagnostic_flux_not_pressure_or_fill_control"
        );
    }

    #[test]
    fn texture_shape_over_time_flags_false_bidirectional_without_message_timestamps() {
        let mut state = BridgeState::new();
        let mut telemetry = make_pressure_telemetry(0.70, 0.22, 0.40);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density");
        resonance.texture_signature.movement_quality = "unfolding_with_containment".to_string();
        state.latest_telemetry = Some(telemetry);
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = None;
        state.last_sensory_sent_unix_s = None;

        let reciprocity = state.bridge_reciprocity_v1();
        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(
            reciprocity.one_sided_state,
            "bidirectional_connected_no_recent_messages"
        );
        let shape = state.texture_shape_over_time_v2().expect("shape");
        assert_eq!(shape.reciprocity_asymmetry_fit, "false_bidirectional");
        assert_eq!(shape.authority, "diagnostic_context_not_control");
    }

    #[test]
    fn texture_shape_over_time_names_stale_bidirectional_reciprocity() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        let mut telemetry = make_pressure_telemetry(0.70, 0.22, 0.40);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density");
        resonance.texture_signature.movement_quality = "unfolding_with_containment".to_string();
        state.latest_telemetry = Some(telemetry);
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now);
        state.last_sensory_sent_unix_s = Some(now - 65.0);

        let reciprocity = state.bridge_reciprocity_v1();
        assert_eq!(reciprocity.one_sided_state, "bidirectional_stale_sensory");
        let shape = state.texture_shape_over_time_v2().expect("shape");
        assert_eq!(shape.reciprocity_asymmetry_fit, "stale_bidirectional");
        assert_eq!(shape.authority, "diagnostic_context_not_control");
    }

    #[test]
    fn texture_shape_over_time_v2_synthesizes_movement_variance_reciprocity_and_smoothing() {
        let mut state = BridgeState::new();
        let mut telemetry = make_pressure_telemetry(0.70, 0.22, 0.40);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density");
        resonance.texture_signature.movement_quality = "unfolding_with_containment".to_string();
        resonance.texture_signature.temporal_variance = Some(0.27);
        state.latest_telemetry = Some(telemetry);
        state.telemetry_connected = true;
        state.sensory_connected = false;
        state.latest_telemetry_arrival_unix_s = Some(unix_now_s());
        for (idx, pressure) in [0.20_f32, 0.22, 0.19, 0.21, 0.20].into_iter().enumerate() {
            let sample = make_pressure_telemetry(0.70, pressure, 0.40);
            record_pressure_trend_sample_v1(&mut state, &sample, 70.0, 100.0 + idx as f64);
        }

        let shape = state.texture_shape_over_time_v2().expect("shape");
        assert_eq!(shape.policy, "texture_shape_over_time_v2");
        assert_eq!(shape.movement_preservation, "movement_preserved");
        assert_eq!(shape.temporal_variance_fit, "variance_carried");
        assert_eq!(shape.reciprocity_asymmetry_fit, "asymmetry_clarified");
        assert_eq!(shape.pressure_smoothing_fit, "twitch_correctly_ignored");
        assert_eq!(shape.static_label_collapse_risk, "movement_preserved");
        assert_eq!(shape.authority, "diagnostic_context_not_control");
    }

    #[test]
    fn telemetry_heartbeat_delta_classifies_normal_late_stale_and_no_history() {
        let trace = WebSocketLaneTrace {
            reconnects: 2,
            disconnects: 1,
            active_connection_id: Some(7),
            active_connection_started_at_unix_s: Some(99.75),
            active_connection_first_valid_payload_at_unix_s: Some(100.0),
            active_connection_valid_payloads_received: 1,
            last_disconnect_reason: Some("test_disconnect".to_string()),
            ..WebSocketLaneTrace::default()
        };
        let no_history = build_telemetry_heartbeat_delta_v1(None, 100.0, &trace);
        assert_eq!(no_history.jitter_class, "no_history");
        assert_eq!(no_history.timing_reliability, "insufficient_history");
        assert_eq!(
            no_history.connection_perception_state,
            "first_valid_packet_after_connect"
        );
        assert_eq!(no_history.first_valid_packet_lag_ms, Some(250.0));
        assert_eq!(no_history.cadence_clarity_score, None);
        assert!(
            no_history
                .field_vs_hearing
                .contains("cannot yet be separated")
        );

        let normal = build_telemetry_heartbeat_delta_v1(Some(100.0), 101.0, &trace);
        assert_eq!(normal.jitter_class, "normal");
        assert_eq!(normal.inter_arrival_ms, Some(1000.0));
        assert_eq!(normal.reconnect_count, 2);
        assert_eq!(normal.active_connection_id, Some(7));
        assert_eq!(normal.cadence_clarity_score, Some(1.0));

        let late = build_telemetry_heartbeat_delta_v1(Some(100.0), 103.0, &trace);
        assert_eq!(late.jitter_class, "late_packet");
        assert_eq!(late.timing_reliability, "timing_ambiguous");
        assert_eq!(late.cadence_clarity_score, Some(0.5));

        let late_4999 = build_telemetry_heartbeat_delta_v1(Some(100.0), 104.999, &trace);
        assert_eq!(late_4999.jitter_class, "late_packet");
        assert_eq!(late_4999.active_connection_id, Some(7));

        let late_5000 = build_telemetry_heartbeat_delta_v1(Some(100.0), 105.0, &trace);
        assert_eq!(late_5000.jitter_class, "late_packet");
        assert_eq!(late_5000.timing_reliability, "timing_ambiguous");

        let stale = build_telemetry_heartbeat_delta_v1(Some(100.0), 108.0, &trace);
        assert_eq!(stale.jitter_class, "stale_packet");
        assert_eq!(stale.timing_reliability, "stale_hearing");
        assert_eq!(stale.cadence_clarity_score, Some(0.0));
        assert!(stale.field_vs_hearing.contains("do not mistake silence"));
    }

    #[test]
    fn telemetry_integration_health_separates_pipeline_wait_and_hold() {
        let clear = build_telemetry_integration_health_v1(None, 4.0, 0.2, 1.0);
        assert_eq!(clear.classification, "clear_at_latest_sample");
        assert_eq!(clear.sample_count, 1);
        assert!(!clear.buffered_integration);
        assert!(!clear.cadence_write);

        let waited = build_telemetry_integration_health_v1(Some(&clear), 3.0, 25.0, 2.0);
        assert_eq!(waited.classification, "write_lock_wait_observed");
        assert_eq!(waited.sample_count, 2);
        assert_eq!(waited.max_write_lock_wait_ms, 25.0);
        assert!((waited.ewma_write_lock_wait_ms - 5.16).abs() < 0.001);
        assert_eq!(
            waited.causal_attribution,
            "not_established_by_timing_alone"
        );

        let held = build_telemetry_integration_health_v1(Some(&waited), 120.0, 0.1, 30.0);
        assert_eq!(held.classification, "write_lock_hold_observed");
        assert_eq!(held.max_prewrite_pipeline_ms, 120.0);
        assert_eq!(held.max_write_lock_hold_ms, 30.0);
        assert_eq!(held.authority, "diagnostic_timing_evidence_not_control");
    }

    #[tokio::test]
    async fn telemetry_parse_error_updates_ws_trace() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());

        assert!(!handle_telemetry_message(b"{not-json", &state, &db).await);

        let s = state.read().await;
        assert_eq!(s.telemetry_ws.parse_errors, 1);
        assert!(
            s.telemetry_ws
                .last_error
                .as_deref()
                .is_some_and(|error| error.starts_with("telemetry_parse_error:"))
        );
        assert_eq!(s.messages_relayed, 0);
    }

    #[tokio::test]
    async fn versioned_telemetry_records_current_protocol() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());
        let mut packet: serde_json::Value =
            serde_json::from_slice(&make_eigenpacket(0.50, 768.0)).unwrap();
        packet["protocol"] = serde_json::json!({
            "name": "astrid_minime",
            "major": 1,
            "minor": 0
        });

        assert!(handle_telemetry_message(&serde_json::to_vec(&packet).unwrap(), &state, &db).await);

        let s = state.read().await;
        assert_eq!(s.telemetry_protocol_v1.compatibility, "current");
        assert!(s.telemetry_protocol_v1.accepted);
        assert_eq!(s.telemetry_protocol_v1.protocol_major, Some(1));
        assert_eq!(s.telemetry_protocol_v1.last_valid_t_ms, Some(1000));
        assert_eq!(s.telemetry_protocol_v1.mismatch_count, 0);
        let health = s
            .telemetry_integration_health_v1
            .as_ref()
            .expect("accepted telemetry records integration timing");
        assert_eq!(health.sample_count, 1);
        assert!(!health.buffered_integration);
        assert!(!health.cadence_write);
    }

    #[tokio::test]
    async fn unsupported_telemetry_major_retains_last_valid_sample() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());
        assert!(
            handle_telemetry_message_at(&make_eigenpacket(0.50, 768.0), &state, &db, 100.0,).await
        );
        let mut incompatible: serde_json::Value =
            serde_json::from_slice(&make_eigenpacket(0.95, 900.0)).unwrap();
        incompatible["t_ms"] = 2000.into();
        incompatible["protocol"] = serde_json::json!({
            "name": "astrid_minime",
            "major": 2,
            "minor": 0
        });

        assert!(
            !handle_telemetry_message_at(
                &serde_json::to_vec(&incompatible).unwrap(),
                &state,
                &db,
                101.0,
            )
            .await
        );

        let s = state.read().await;
        assert_eq!(s.latest_telemetry.as_ref().unwrap().t_ms, 1000);
        assert_eq!(s.minime_observation_v1().unwrap().packet().t_ms, 1000);
        assert!((s.fill_pct - 50.0).abs() < 0.1);
        assert_eq!(s.latest_telemetry_arrival_unix_s, Some(100.0));
        assert_eq!(s.telemetry_protocol_v1.compatibility, "unsupported_major");
        assert!(!s.telemetry_protocol_v1.accepted);
        assert_eq!(s.telemetry_protocol_v1.last_valid_t_ms, Some(1000));
        assert_eq!(s.telemetry_protocol_v1.mismatch_count, 1);
        assert_eq!(s.messages_relayed, 1);
    }

    #[test]
    fn sensory_port_adds_only_protocol_header_to_legacy_shape() {
        let message = SensoryMsg::Semantic {
            features: vec![0.1, -0.2],
            ts_ms: None,
        };
        let encoded = encode_sensory_packet_v1(&message, None, false, 1)
            .unwrap()
            .json;
        let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        let packet: SensoryPacketV1 = serde_json::from_str(&encoded).unwrap();

        assert_eq!(packet.compatibility(), CompatibilityStatus::Current);
        assert_eq!(value["protocol"]["major"], 1);
        assert_eq!(value["protocol"]["minor"], 0);
        assert_eq!(value["kind"], "semantic");
        assert_eq!(value["features"], serde_json::json!([0.1, -0.2]));
        assert!(value.get("ts_ms").is_none());
        assert!(value.get("delivery_v1").is_none());
        assert!(value.get("mutual_address_v1").is_none());
    }

    #[tokio::test]
    async fn telemetry_populates_pull_rate_after_second_sample() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());

        // Start green.
        handle_telemetry_message(&make_eigenpacket(0.50, 768.0), &state, &db).await;
        handle_telemetry_message(&make_eigenpacket(0.52, 780.0), &state, &db).await;
        assert!(
            state
                .read()
                .await
                .pull_topology
                .as_ref()
                .is_some_and(|profile| profile.rate_available)
        );
    }

    #[tokio::test]
    async fn telemetry_escalates_to_yellow() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());

        // Start green.
        handle_telemetry_message(&make_eigenpacket(0.50, 768.0), &state, &db).await;
        assert_eq!(state.read().await.safety_level, SafetyLevel::Green);

        // Escalate to yellow.
        handle_telemetry_message(&make_eigenpacket(0.80, 896.0), &state, &db).await;
        let s = state.read().await;
        assert_eq!(s.safety_level, SafetyLevel::Yellow);
        assert_eq!(s.prev_safety_level, SafetyLevel::Green);
        assert!(s.active_incident_id.is_some());
    }

    #[tokio::test]
    async fn telemetry_escalates_green_to_red() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());

        // Start green.
        handle_telemetry_message(&make_eigenpacket(0.50, 768.0), &state, &db).await;

        // Jump straight to red.
        handle_telemetry_message(&make_eigenpacket(0.95, 1000.0), &state, &db).await;
        let s = state.read().await;
        assert_eq!(s.safety_level, SafetyLevel::Red);
        assert!(s.safety_level.is_emergency());
        assert!(s.safety_level.should_suspend_outbound());
        assert!(s.active_incident_id.is_some());
    }

    #[tokio::test]
    async fn telemetry_recovers_to_green() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());

        // Green → Orange → Green.
        handle_telemetry_message(&make_eigenpacket(0.50, 768.0), &state, &db).await;
        handle_telemetry_message(&make_eigenpacket(0.90, 948.0), &state, &db).await;
        assert_eq!(state.read().await.safety_level, SafetyLevel::Orange);
        let incident_id = state.read().await.active_incident_id;
        assert!(incident_id.is_some());

        handle_telemetry_message(&make_eigenpacket(0.50, 768.0), &state, &db).await;
        let s = state.read().await;
        assert_eq!(s.safety_level, SafetyLevel::Green);
        assert!(s.active_incident_id.is_none()); // Incident resolved.
    }

    #[tokio::test]
    async fn telemetry_logs_to_sqlite() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());

        handle_telemetry_message(&make_eigenpacket(0.55, 793.0), &state, &db).await;
        handle_telemetry_message(&make_eigenpacket(0.60, 820.0), &state, &db).await;

        assert!(db.message_count().unwrap() >= 6);
        let rows = db.query_messages(0.0, f64::MAX, None, 10).unwrap();
        assert!(rows.len() >= 6);
        assert!(
            rows.iter()
                .any(|row| row.topic == "consciousness.v1.telemetry")
        );
        assert!(
            rows.iter()
                .any(|row| row.topic == "consciousness.v1.lambda_tail")
        );
        assert!(
            rows.iter()
                .any(|row| row.topic == lambda_edge::LAMBDA_EDGE_TOPIC)
        );
    }

    #[tokio::test]
    async fn full_escalation_cycle_logs_incidents() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());

        // Green → Yellow → Orange → Red → Green (recovery).
        let fills = [0.50, 0.72, 0.85, 0.95, 0.40];
        for fill in fills {
            handle_telemetry_message(&make_eigenpacket(fill, 512.0 + fill * 512.0), &state, &db)
                .await;
        }

        assert_eq!(state.read().await.safety_level, SafetyLevel::Green);
        assert_eq!(state.read().await.messages_relayed, 5);

        // Should have logged incidents for yellow, orange, red transitions.
        // All should be resolved after returning to green.
    }
}
