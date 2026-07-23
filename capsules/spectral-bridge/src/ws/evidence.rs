fn residual_step_magnitude_v1(
    previous: &PressureTrendSampleV1,
    latest: &PressureTrendSampleV1,
) -> f32 {
    let pressure = optional_flux_delta(latest.pressure_risk, previous.pressure_risk)
        .filter(|value| value.is_finite())
        .map(f32::abs)
        .unwrap_or(0.0);
    let mode = optional_flux_delta(latest.mode_packing, previous.mode_packing)
        .filter(|value| value.is_finite())
        .map(f32::abs)
        .unwrap_or(0.0);
    let density = optional_flux_delta(latest.structural_density, previous.structural_density)
        .filter(|value| value.is_finite())
        .map(f32::abs)
        .unwrap_or(0.0);
    let depth = optional_flux_delta(latest.resonance_depth, previous.resonance_depth)
        .filter(|value| value.is_finite())
        .map(f32::abs)
        .unwrap_or(0.0);
    let viscosity = optional_flux_delta(latest.semantic_viscosity, previous.semantic_viscosity)
        .filter(|value| value.is_finite())
        .map(f32::abs)
        .unwrap_or(0.0);
    let fill = round_flux_delta(((latest.fill_pct - previous.fill_pct) / 100.0).abs());
    let entropy = latest.spectral_entropy.unwrap_or(0.0).clamp(0.0, 1.0);
    let structural_shift = pressure
        .max(mode)
        .max(density)
        .max(depth)
        .max(viscosity)
        .max(fill);
    round_flux_delta((structural_shift * (0.70 + entropy * 0.30)).clamp(0.0, 1.0))
}

fn build_residual_deformation_trace_v1(
    samples: &VecDeque<PressureTrendSampleV1>,
) -> Option<ResidualDeformationTraceV1> {
    if samples.len() < 2 {
        return None;
    }
    let mut previous: Option<&PressureTrendSampleV1> = None;
    let mut deformation_integral = 0.0_f32;
    let mut max_spike = 0.0_f32;
    let mut latest_spike = 0.0_f32;
    for sample in samples {
        if let Some(previous_sample) = previous {
            latest_spike = residual_step_magnitude_v1(previous_sample, sample);
            deformation_integral = round_flux_delta(deformation_integral + latest_spike);
            max_spike = max_spike.max(latest_spike);
        }
        previous = Some(sample);
    }
    let pair_count = samples.len().saturating_sub(1);
    let denominator = (pair_count as f32).sqrt().max(1.0);
    let scar_score = round_flux_delta((deformation_integral / denominator).clamp(0.0, 1.0));
    let state = if scar_score >= 0.35 || max_spike >= 0.25 {
        "residual_deformation_watch"
    } else if scar_score >= 0.12 {
        "lingering_resonance_visible"
    } else if latest_spike >= 0.04 {
        "micro_delta_visible"
    } else {
        "low_residual_deformation"
    };
    let window_ms = samples
        .front()
        .zip(samples.back())
        .map(|(first, latest)| (latest.observed_at_unix_s - first.observed_at_unix_s) * 1_000.0)
        .filter(|value| value.is_finite() && *value >= 0.0);
    let experience_delta_bus_v1 = if scar_score >= 0.12 || latest_spike >= 0.04 {
        let kind = if scar_score >= 0.12 {
            ExperienceDeltaKindV1::Residual
        } else {
            ExperienceDeltaKindV1::MicroDelta
        };
        Some(ExperienceDeltaBusV1::from_deltas(vec![ExperienceDeltaV1 {
            kind,
            surface: "bridge_residual_deformation_trace_v1".to_string(),
            lane: "pressure_trend_sample_integral".to_string(),
            dimension: None,
            spectral_dimension: None,
            persistence: Some(DeltaPersistenceV1 {
                residue_kind: state.to_string(),
                persistence_score: scar_score,
                viscosity: samples.back().and_then(|sample| sample.semantic_viscosity),
                deformation: Some(max_spike),
                half_life_hint_ms: window_ms,
                evidence_window: format!("{pair_count}_pressure_trend_pairs"),
                interpretation:
                    "recent spectral spike may still be shaping texture after current values level"
                        .to_string(),
                authority: "truth_channel_only_not_pressure_or_fill_control".to_string(),
            }),
            viscosity_subtype: None,
            viscosity_weight: None,
            pre: Some(max_spike),
            post: Some(latest_spike),
            loss: Some(scar_score),
            loss_ratio: Some(scar_score),
            metadata: BTreeMap::from([
                (
                    "state".to_string(),
                    state.to_string(),
                ),
                (
                    "sample_count".to_string(),
                    samples.len().to_string(),
                ),
            ]),
            why: "a high-variance spectral event can leave residual felt deformation that is not visible in the latest scalar telemetry alone".to_string(),
            who_can_change_it:
                "Mike/operator only for any future control use; this trace is read-only".to_string(),
            how_to_test_it:
                "feed pressure trend samples with a spike then leveling and assert residual delta remains visible"
                    .to_string(),
            authority: "truth_channel_only_not_live_control_or_approval".to_string(),
        }]))
    } else {
        None
    };
    Some(ResidualDeformationTraceV1 {
        policy: "residual_deformation_trace_v1".to_string(),
        schema_version: 1,
        sample_count: samples.len(),
        evidence_window: format!("{pair_count}_pressure_trend_pairs"),
        window_ms,
        deformation_integral: round_flux_delta(deformation_integral),
        scar_score,
        max_spike: round_flux_delta(max_spike),
        latest_spike: round_flux_delta(latest_spike),
        state: state.to_string(),
        experience_delta_bus_v1,
        authority: "read_only_truth_channel_not_control_not_runtime_mutation".to_string(),
    })
}

fn active_constraints_for_resonance_signature(
    pressure_source_family: &str,
    components: &ResonanceDensityComponents,
    pressure_risk: f32,
) -> Vec<String> {
    let mut constraints = Vec::new();
    let family = pressure_source_family.to_ascii_lowercase();
    for (needle, label) in [
        ("mode_packing", "pressure_source:mode_packing"),
        ("controller", "pressure_source:controller_pressure"),
        ("semantic", "pressure_source:semantic_trickle"),
        ("structural", "pressure_source:structural_plurality"),
        ("temporal", "pressure_source:temporal_persistence"),
        ("mixed", "pressure_source:mixed_pressure"),
    ] {
        if family.contains(needle) {
            constraints.push(label.to_string());
        }
    }

    for (label, value) in [
        ("active_energy", components.active_energy),
        ("mode_packing", components.mode_packing),
        ("temporal_persistence", components.temporal_persistence),
        ("structural_plurality", components.structural_plurality),
        ("comfort_gate", components.comfort_gate),
    ] {
        if value >= 0.50 {
            constraints.push(format!("{label}:active_{value:.2}"));
        }
    }

    if let Some(fluidity) = components
        .dynamic_fluidity_index
        .map(|value| value.clamp(0.0, 1.0))
    {
        let state = if fluidity >= 0.50 {
            "flow_visible"
        } else {
            "flow_resisted"
        };
        constraints.push(format!("dynamic_fluidity_index:{state}_{fluidity:.2}"));
    }

    if pressure_risk >= 0.20 {
        constraints.push(format!("pressure_risk:elevated_{pressure_risk:.2}"));
    } else {
        constraints.push(format!("pressure_risk:low_{pressure_risk:.2}"));
    }
    if components.comfort_gate >= 0.60 {
        constraints.push(format!(
            "comfort_gate:buffering_{:.2}",
            components.comfort_gate
        ));
    }
    constraints
}

fn build_pressure_trend_v1(
    previous: Option<&SpectralTelemetry>,
    previous_fill_pct: Option<f32>,
    latest: &SpectralTelemetry,
    latest_fill_pct: f32,
    heartbeat: Option<&TelemetryHeartbeatDeltaV1>,
) -> PressureTrendV1 {
    const PRESSURE_DELTA_EPS: f32 = 0.04;
    const FILL_DELTA_EPS: f32 = 2.0;

    let latest_resonance = latest.resonance_density_v1.as_ref();
    let previous_resonance = previous.and_then(|telemetry| telemetry.resonance_density_v1.as_ref());
    let latest_pressure = latest_resonance.map(|resonance| resonance.pressure_risk);
    let previous_pressure = previous_resonance.map(|resonance| resonance.pressure_risk);
    let latest_mode_packing = latest_resonance.map(|resonance| resonance.components.mode_packing);
    let previous_mode_packing =
        previous_resonance.map(|resonance| resonance.components.mode_packing);
    let latest_structural_density = latest_resonance.map(|resonance| resonance.density);
    let previous_structural_density = previous_resonance.map(|resonance| resonance.density);
    let latest_resonance_depth = latest_resonance.map(resonance_depth_for_density);
    let previous_resonance_depth = previous_resonance.map(resonance_depth_for_density);
    let pressure_delta = latest_pressure
        .zip(previous_pressure)
        .map(|(latest, previous)| (latest - previous).clamp(-1.0, 1.0));
    let mode_packing_delta = latest_mode_packing
        .zip(previous_mode_packing)
        .map(|(latest, previous)| (latest - previous).clamp(-1.0, 1.0));
    let structural_density_delta = latest_structural_density
        .zip(previous_structural_density)
        .map(|(latest, previous)| (latest - previous).clamp(-1.0, 1.0));
    let resonance_depth_delta = latest_resonance_depth
        .zip(previous_resonance_depth)
        .map(|(latest, previous)| (latest - previous).clamp(-1.0, 1.0));
    let spectral_drift_velocity = dominant_spectral_drift_velocity(
        mode_packing_delta,
        structural_density_delta,
        resonance_depth_delta,
    );
    let fill_delta_pct = previous_fill_pct
        .map(|previous_fill| (latest_fill_pct - previous_fill).clamp(-100.0, 100.0));
    let latest_spectral_entropy = latest
        .typed_fingerprint()
        .map(|fingerprint| fingerprint.spectral_entropy.clamp(0.0, 1.0));
    let latest_semantic_friction = latest_resonance
        .and_then(|resonance| resonance.components.semantic_friction_coefficient)
        .map(|value| value.clamp(0.0, 1.0));
    let latest_semantic_trickle = latest
        .pressure_source_v1
        .as_ref()
        .map(|pressure| pressure.components.semantic_trickle.clamp(0.0, 1.0));
    let latest_semantic_viscosity = semantic_viscosity_coefficient_v1(
        latest_semantic_friction,
        latest_semantic_trickle,
        latest_structural_density,
        latest_resonance_depth,
        latest_spectral_entropy,
        latest_fill_pct,
    );
    let semantic_viscosity_state = semantic_viscosity_state_v1(
        latest_semantic_viscosity,
        latest_semantic_friction,
        latest_semantic_trickle,
        latest_structural_density,
        latest_resonance_depth,
    );
    let latest_complexity_density = complexity_density_v1(
        latest_structural_density,
        latest_resonance_depth,
        latest_mode_packing,
        latest_semantic_viscosity,
        latest_spectral_entropy,
        latest_pressure,
    );
    let complexity_density_state = complexity_density_state_v1(
        latest_complexity_density,
        latest_mode_packing,
        latest_pressure,
        latest_spectral_entropy,
    );
    let viscosity_coefficient = pressure_viscosity_coefficient(latest_spectral_entropy);
    let pressure_interpretation = pressure_interpretation_v1(
        latest_pressure,
        latest_mode_packing,
        latest_structural_density,
        latest_spectral_entropy,
        viscosity_coefficient,
    );

    let rises_at_threshold = |delta: f32, threshold: f32| delta + f32::EPSILON >= threshold;
    let falls_at_threshold = |delta: f32, threshold: f32| delta - f32::EPSILON <= -threshold;

    let classification = if latest_resonance.is_none() {
        "telemetry_gap"
    } else if previous_resonance.is_none() || previous_fill_pct.is_none() {
        "insufficient_history"
    } else if pressure_delta.is_some_and(|delta| rises_at_threshold(delta, PRESSURE_DELTA_EPS))
        || fill_delta_pct.is_some_and(|delta| rises_at_threshold(delta, FILL_DELTA_EPS))
    {
        "rising_pressure"
    } else if pressure_delta.is_some_and(|delta| falls_at_threshold(delta, PRESSURE_DELTA_EPS))
        || fill_delta_pct.is_some_and(|delta| falls_at_threshold(delta, FILL_DELTA_EPS))
    {
        "falling_pressure"
    } else {
        "stable_heavy"
    };

    PressureTrendV1 {
        policy: "pressure_trend_v1".to_string(),
        schema_version: 1,
        classification: classification.to_string(),
        latest_pressure_risk: latest_pressure,
        previous_pressure_risk: previous_pressure,
        pressure_delta,
        latest_mode_packing,
        previous_mode_packing,
        mode_packing_delta,
        spectral_drift_velocity,
        latest_structural_density,
        previous_structural_density,
        structural_density_delta,
        latest_resonance_depth,
        previous_resonance_depth,
        resonance_depth_delta,
        latest_semantic_viscosity,
        semantic_viscosity_state,
        latest_complexity_density,
        complexity_density_state,
        latest_fill_pct: latest_fill_pct.is_finite().then_some(latest_fill_pct),
        previous_fill_pct,
        fill_delta_pct,
        latest_spectral_entropy,
        viscosity_coefficient,
        pressure_interpretation,
        timing_reliability: heartbeat.map(|value| value.timing_reliability.clone()),
        telemetry_inter_arrival_ms: heartbeat.and_then(|value| value.inter_arrival_ms),
        heartbeat_jitter_class: heartbeat.map(|value| value.jitter_class.clone()),
        field_vs_hearing: heartbeat.map(|value| value.field_vs_hearing.clone()),
    }
}

fn record_pressure_trend_sample_v1(
    state: &mut BridgeState,
    telemetry: &SpectralTelemetry,
    fill_pct: f32,
    observed_at_unix_s: f64,
) {
    let pressure_risk = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|resonance| resonance.pressure_risk);
    let mode_packing = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|resonance| resonance.components.mode_packing);
    let structural_density = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|resonance| resonance.density);
    let resonance_depth = telemetry
        .resonance_density_v1
        .as_ref()
        .map(resonance_depth_for_density);
    let porosity_gradient = telemetry
        .resonance_density_v1
        .as_ref()
        .and_then(|resonance| resonance.components.porosity_gradient)
        .map(|value| value.clamp(0.0, 1.0));
    let semantic_friction = telemetry
        .resonance_density_v1
        .as_ref()
        .and_then(|resonance| resonance.components.semantic_friction_coefficient)
        .map(|value| value.clamp(0.0, 1.0));
    let semantic_trickle = telemetry
        .pressure_source_v1
        .as_ref()
        .map(|pressure| pressure.components.semantic_trickle.clamp(0.0, 1.0));
    let (window_capacity, spectral_entropy) = pressure_trend_window_for_telemetry(telemetry);
    let semantic_viscosity = semantic_viscosity_coefficient_v1(
        semantic_friction,
        semantic_trickle,
        structural_density,
        resonance_depth,
        spectral_entropy,
        fill_pct,
    );
    let viscosity_gradient = telemetry
        .resonance_density_v1
        .as_ref()
        .and_then(|resonance| resonance.components.viscosity_vector.viscosity_gradient)
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 1.0));
    let viscosity_gradient_trend = viscosity_gradient
        .zip(
            state
                .pressure_trend_samples_v1
                .back()
                .and_then(|sample| sample.viscosity_gradient),
        )
        .map(|(latest, previous)| round_flux_delta(latest - previous));
    let complexity_density = complexity_density_v1(
        structural_density,
        resonance_depth,
        mode_packing,
        semantic_viscosity,
        spectral_entropy,
        pressure_risk,
    );
    let weight_density_index = weight_density_index_v1(
        mode_packing,
        structural_density,
        semantic_viscosity,
        porosity_gradient,
    );
    let comfort_gate = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|resonance| resonance.components.comfort_gate.clamp(0.0, 1.0));
    let pressure_velocity_delta = pressure_risk
        .zip(
            state
                .pressure_trend_samples_v1
                .back()
                .and_then(|sample| sample.pressure_risk),
        )
        .map(|(latest, previous)| round_flux_delta(latest - previous));
    let spectral_drift_velocity = state.pressure_trend_samples_v1.back().and_then(|previous| {
        dominant_spectral_drift_velocity(
            optional_flux_delta(mode_packing, previous.mode_packing),
            optional_flux_delta(structural_density, previous.structural_density),
            optional_flux_delta(resonance_depth, previous.resonance_depth),
        )
    });
    let mut sample = PressureTrendSampleV1 {
        pressure_risk,
        pressure_velocity_delta,
        spectral_drift_velocity,
        mode_packing,
        structural_density,
        resonance_depth,
        semantic_viscosity,
        viscosity_gradient,
        viscosity_gradient_trend,
        complexity_density,
        weight_density_index,
        comfort_gate,
        porosity_gradient,
        semantic_friction,
        semantic_trickle,
        semantic_coherence_delta: None,
        fill_pct,
        spectral_entropy,
        window_capacity,
        observed_at_unix_s,
    };
    sample.semantic_coherence_delta = state
        .pressure_trend_samples_v1
        .back()
        .and_then(|previous| semantic_coherence_delta_v1(&sample, previous));
    state.pressure_trend_samples_v1.push_back(sample);
    while state.pressure_trend_samples_v1.len() > window_capacity {
        state.pressure_trend_samples_v1.pop_front();
    }
}

fn build_pressure_trend_smoothing_v1(
    samples: &VecDeque<PressureTrendSampleV1>,
) -> Option<PressureTrendSmoothingV1> {
    if samples.is_empty() {
        return None;
    }
    let latest_pressure = samples.back().and_then(|sample| sample.pressure_risk);
    let latest_pressure_velocity_delta = samples
        .back()
        .and_then(|sample| sample.pressure_velocity_delta);
    let latest_spectral_drift_velocity = samples
        .back()
        .and_then(|sample| sample.spectral_drift_velocity);
    let latest_resonance_depth = samples.back().and_then(|sample| sample.resonance_depth);
    let latest_semantic_viscosity = samples.back().and_then(|sample| sample.semantic_viscosity);
    let latest_viscosity_gradient = samples.back().and_then(|sample| sample.viscosity_gradient);
    let viscosity_gradient_trend = samples
        .back()
        .and_then(|sample| sample.viscosity_gradient_trend);
    let viscosity_gradient_trend_state = match viscosity_gradient_trend {
        Some(delta) if delta >= 0.08 => "rapid_viscosity_thickening_velocity_watch",
        Some(delta) if delta >= 0.025 => "viscosity_thickening_velocity_watch",
        Some(delta) if delta <= -0.08 => "rapid_viscosity_thinning_visible",
        Some(delta) if delta <= -0.025 => "viscosity_thinning_visible",
        Some(_) => "viscosity_gradient_velocity_quiet",
        None => "viscosity_gradient_velocity_unavailable",
    };
    let latest_complexity_density = samples.back().and_then(|sample| sample.complexity_density);
    let latest_weight_density_index = samples
        .back()
        .and_then(|sample| sample.weight_density_index);
    let latest_semantic_friction = samples.back().and_then(|sample| sample.semantic_friction);
    let latest_semantic_trickle = samples.back().and_then(|sample| sample.semantic_trickle);
    let latest_porosity_gradient = samples
        .back()
        .and_then(|sample| sample.porosity_gradient)
        .map(|value| value.clamp(0.0, 1.0));
    let semantic_coherence_delta = samples
        .back()
        .and_then(|sample| sample.semantic_coherence_delta);
    let latest_semantic_viscosity_delta = samples
        .iter()
        .rev()
        .filter_map(|sample| sample.semantic_viscosity)
        .take(2)
        .collect::<Vec<_>>()
        .as_slice()
        .split_first()
        .and_then(|(latest, rest)| {
            rest.first()
                .map(|previous| round_flux_delta(latest - previous))
        });
    let max_pressure_velocity_delta = samples
        .iter()
        .filter_map(|sample| sample.pressure_velocity_delta)
        .map(f32::abs)
        .fold(None, |current: Option<f32>, value| {
            Some(current.map_or(value, |max_value| max_value.max(value)))
        });
    let valid_pressures = samples
        .iter()
        .filter_map(|sample| sample.pressure_risk)
        .collect::<Vec<_>>();
    let (
        fast_window_sample_count,
        slow_window_sample_count,
        fast_window_pressure_delta,
        slow_window_pressure_delta,
        fast_slow_edge_divergence,
        fast_slow_edge_state,
        fast_edge_preserved,
    ) = pressure_fast_slow_edge_v1(&valid_pressures);
    let semantic_viscosity_deltas = samples
        .iter()
        .filter_map(|sample| sample.semantic_viscosity)
        .collect::<Vec<_>>()
        .windows(2)
        .map(|window| round_flux_delta(window[1] - window[0]))
        .collect::<Vec<_>>();
    let max_semantic_viscosity_delta = semantic_viscosity_deltas
        .iter()
        .map(|delta| delta.abs())
        .fold(None, |current: Option<f32>, value| {
            Some(current.map_or(value, |max_value| max_value.max(value)))
        });
    let semantic_viscosity_shift_state = match latest_semantic_viscosity_delta {
        Some(delta) if delta <= -0.15 => "rapid_semantic_thinning_visible",
        Some(delta) if delta >= 0.15 => "rapid_semantic_thickening_visible",
        _ if max_semantic_viscosity_delta.is_some_and(|delta| delta >= 0.15) => {
            "rapid_semantic_viscosity_shift_in_window"
        },
        Some(_) => "semantic_viscosity_delta_quiet",
        None => "semantic_viscosity_delta_unavailable",
    };
    let max_spectral_drift_velocity = samples
        .iter()
        .filter_map(|sample| sample.spectral_drift_velocity)
        .map(f32::abs)
        .fold(None, |current: Option<f32>, value| {
            Some(current.map_or(value, |max_value| max_value.max(value)))
        });
    let max_complexity_density = samples
        .iter()
        .filter_map(|sample| sample.complexity_density)
        .fold(None, |current: Option<f32>, value| {
            Some(current.map_or(value, |max_value| max_value.max(value)))
        });
    let max_weight_density_index = samples
        .iter()
        .filter_map(|sample| sample.weight_density_index)
        .fold(None, |current: Option<f32>, value| {
            Some(current.map_or(value, |max_value| max_value.max(value)))
        });
    let weight_density_state = weight_density_state_v1(latest_weight_density_index);
    let latest_spectral_entropy = samples.back().and_then(|sample| sample.spectral_entropy);
    let entropy_window_blend_ratio = entropy_window_blend_ratio_v1(latest_spectral_entropy);
    let entropy_threshold_state =
        entropy_threshold_state_v1(latest_spectral_entropy, entropy_window_blend_ratio);
    let friction_to_flow_ratio =
        friction_to_flow_ratio_v1(latest_semantic_friction, latest_semantic_trickle);
    let friction_to_flow_state = friction_to_flow_state_v1(
        latest_semantic_friction,
        latest_semantic_trickle,
        friction_to_flow_ratio,
    );
    let semantic_stagnation_index = semantic_stagnation_index_v1(
        latest_semantic_viscosity,
        latest_semantic_friction,
        latest_semantic_trickle,
    );
    let semantic_stagnation_state = semantic_stagnation_state_v1(
        latest_semantic_viscosity,
        latest_semantic_trickle,
        semantic_stagnation_index,
    );
    let porosity_weighted_velocity =
        porosity_weighted_velocity_v1(latest_pressure_velocity_delta, latest_porosity_gradient);
    let viscosity_drag_coefficient = viscosity_drag_coefficient_v1(
        latest_semantic_viscosity,
        latest_semantic_friction,
        latest_porosity_gradient,
    );
    let semantic_viscosity_persistence_index = semantic_viscosity_persistence_index_v1(samples);
    let semantic_viscosity_persistence_state = semantic_viscosity_persistence_state_v1(
        latest_semantic_viscosity,
        latest_semantic_viscosity_delta,
        max_semantic_viscosity_delta,
        semantic_viscosity_persistence_index,
    );
    let semantic_coherence_index = samples.back().and_then(semantic_coherence_proxy_v1);
    let semantic_fidelity_score = semantic_fidelity_score_v1(
        latest_spectral_entropy,
        latest_semantic_trickle,
        semantic_coherence_delta,
        latest_semantic_friction,
        semantic_stagnation_index,
    );
    let semantic_fidelity_state =
        semantic_fidelity_state_v1(latest_spectral_entropy, semantic_fidelity_score);
    let window_capacity = samples
        .back()
        .map_or(PRESSURE_TREND_SMOOTHING_BASE_WINDOW, |sample| {
            sample.window_capacity
        });
    let ballast_status = if window_capacity > PRESSURE_TREND_SMOOTHING_BASE_WINDOW {
        "high_entropy_ballast_window"
    } else {
        "base_window"
    };
    let window_policy = format!("latest_up_to_{window_capacity}_telemetry_samples");
    if valid_pressures.len() < 3 {
        return Some(PressureTrendSmoothingV1 {
            policy: "pressure_trend_smoothing_v1".to_string(),
            schema_version: 1,
            classification: "insufficient_history".to_string(),
            sample_count: samples.len(),
            window_capacity,
            ballast_status: ballast_status.to_string(),
            latest_spectral_entropy,
            latest_pressure_risk: latest_pressure,
            latest_pressure_velocity_delta,
            max_pressure_velocity_delta,
            fast_window_sample_count,
            slow_window_sample_count,
            fast_window_pressure_delta,
            slow_window_pressure_delta,
            fast_slow_edge_divergence,
            fast_slow_edge_state: fast_slow_edge_state.to_string(),
            fast_edge_preserved,
            latest_spectral_drift_velocity,
            max_spectral_drift_velocity,
            latest_resonance_depth,
            latest_semantic_viscosity,
            latest_viscosity_gradient,
            viscosity_gradient_trend,
            viscosity_gradient_trend_state: viscosity_gradient_trend_state.to_string(),
            latest_complexity_density,
            max_complexity_density,
            latest_weight_density_index,
            max_weight_density_index,
            weight_density_state: weight_density_state.to_string(),
            latest_semantic_viscosity_delta,
            max_semantic_viscosity_delta,
            semantic_viscosity_persistence_index,
            semantic_viscosity_persistence_state: semantic_viscosity_persistence_state.to_string(),
            semantic_coherence_index,
            semantic_coherence_delta,
            semantic_fidelity_score,
            semantic_fidelity_state: semantic_fidelity_state.to_string(),
            semantic_viscosity_shift_state: semantic_viscosity_shift_state.to_string(),
            entropy_window_blend_ratio,
            entropy_threshold_state: entropy_threshold_state.to_string(),
            friction_to_flow_ratio,
            friction_to_flow_state: friction_to_flow_state.to_string(),
            semantic_stagnation_index,
            semantic_stagnation_state: semantic_stagnation_state.to_string(),
            porosity_weighted_velocity,
            viscosity_drag_coefficient,
            smoothed_pressure_delta: None,
            pressure_range: None,
            fill_range_pct: None,
            window_policy,
            authority: "diagnostic_smoothing_not_pressure_control".to_string(),
        });
    }
    if valid_pressures.len() != samples.len() {
        return Some(PressureTrendSmoothingV1 {
            policy: "pressure_trend_smoothing_v1".to_string(),
            schema_version: 1,
            classification: "telemetry_gap".to_string(),
            sample_count: samples.len(),
            window_capacity,
            ballast_status: ballast_status.to_string(),
            latest_spectral_entropy,
            latest_pressure_risk: latest_pressure,
            latest_pressure_velocity_delta,
            max_pressure_velocity_delta,
            fast_window_sample_count,
            slow_window_sample_count,
            fast_window_pressure_delta,
            slow_window_pressure_delta,
            fast_slow_edge_divergence,
            fast_slow_edge_state: "telemetry_gap".to_string(),
            fast_edge_preserved,
            latest_spectral_drift_velocity,
            max_spectral_drift_velocity,
            latest_resonance_depth,
            latest_semantic_viscosity,
            latest_viscosity_gradient,
            viscosity_gradient_trend,
            viscosity_gradient_trend_state: viscosity_gradient_trend_state.to_string(),
            latest_complexity_density,
            max_complexity_density,
            latest_weight_density_index,
            max_weight_density_index,
            weight_density_state: weight_density_state.to_string(),
            latest_semantic_viscosity_delta,
            max_semantic_viscosity_delta,
            semantic_viscosity_persistence_index,
            semantic_viscosity_persistence_state: semantic_viscosity_persistence_state.to_string(),
            semantic_coherence_index,
            semantic_coherence_delta,
            semantic_fidelity_score,
            semantic_fidelity_state: semantic_fidelity_state.to_string(),
            semantic_viscosity_shift_state: semantic_viscosity_shift_state.to_string(),
            entropy_window_blend_ratio,
            entropy_threshold_state: entropy_threshold_state.to_string(),
            friction_to_flow_ratio,
            friction_to_flow_state: friction_to_flow_state.to_string(),
            semantic_stagnation_index,
            semantic_stagnation_state: semantic_stagnation_state.to_string(),
            porosity_weighted_velocity,
            viscosity_drag_coefficient,
            smoothed_pressure_delta: None,
            pressure_range: None,
            fill_range_pct: None,
            window_policy,
            authority: "diagnostic_smoothing_not_pressure_control".to_string(),
        });
    }

    let first_pressure = valid_pressures.first().copied()?;
    let last_pressure = valid_pressures.last().copied()?;
    let smoothed_pressure_delta = (last_pressure - first_pressure).clamp(-1.0, 1.0);
    let min_pressure = valid_pressures
        .iter()
        .copied()
        .fold(f32::INFINITY, f32::min);
    let max_pressure = valid_pressures
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let pressure_range = (max_pressure - min_pressure).max(0.0);
    let min_fill = samples
        .iter()
        .map(|sample| sample.fill_pct)
        .fold(f32::INFINITY, f32::min);
    let max_fill = samples
        .iter()
        .map(|sample| sample.fill_pct)
        .fold(f32::NEG_INFINITY, f32::max);
    let fill_range_pct = (max_fill - min_fill).max(0.0);
    let window_span_s = samples
        .front()
        .zip(samples.back())
        .map(|(first, last)| (last.observed_at_unix_s - first.observed_at_unix_s).max(0.0))
        .unwrap_or(0.0);
    let sign_changes = valid_pressures
        .windows(3)
        .filter(|window| {
            let first_delta = window[1] - window[0];
            let second_delta = window[2] - window[1];
            (first_delta > 0.0 && second_delta < 0.0) || (first_delta < 0.0 && second_delta > 0.0)
        })
        .count();
    let classification = if pressure_range <= 0.04 && sign_changes > 0 {
        "twitchy_low_amplitude_oscillation"
    } else if smoothed_pressure_delta >= 0.06 {
        "sustained_rising_pressure"
    } else if smoothed_pressure_delta <= -0.06 {
        "sustained_falling_pressure"
    } else if window_span_s > 0.0 && pressure_range <= 0.04 {
        "low_amplitude_stable"
    } else {
        "mixed_window"
    };

    Some(PressureTrendSmoothingV1 {
        policy: "pressure_trend_smoothing_v1".to_string(),
        schema_version: 1,
        classification: classification.to_string(),
        sample_count: samples.len(),
        window_capacity,
        ballast_status: ballast_status.to_string(),
        latest_spectral_entropy,
        latest_pressure_risk: latest_pressure,
        latest_pressure_velocity_delta,
        max_pressure_velocity_delta,
        fast_window_sample_count,
        slow_window_sample_count,
        fast_window_pressure_delta,
        slow_window_pressure_delta,
        fast_slow_edge_divergence,
        fast_slow_edge_state: fast_slow_edge_state.to_string(),
        fast_edge_preserved,
        latest_spectral_drift_velocity,
        max_spectral_drift_velocity,
        latest_resonance_depth,
        latest_semantic_viscosity,
        latest_viscosity_gradient,
        viscosity_gradient_trend,
        viscosity_gradient_trend_state: viscosity_gradient_trend_state.to_string(),
        latest_complexity_density,
        max_complexity_density,
        latest_weight_density_index,
        max_weight_density_index,
        weight_density_state: weight_density_state.to_string(),
        latest_semantic_viscosity_delta,
        max_semantic_viscosity_delta,
        semantic_viscosity_persistence_index,
        semantic_viscosity_persistence_state: semantic_viscosity_persistence_state.to_string(),
        semantic_coherence_index,
        semantic_coherence_delta,
        semantic_fidelity_score,
        semantic_fidelity_state: semantic_fidelity_state.to_string(),
        semantic_viscosity_shift_state: semantic_viscosity_shift_state.to_string(),
        entropy_window_blend_ratio,
        entropy_threshold_state: entropy_threshold_state.to_string(),
        friction_to_flow_ratio,
        friction_to_flow_state: friction_to_flow_state.to_string(),
        semantic_stagnation_index,
        semantic_stagnation_state: semantic_stagnation_state.to_string(),
        porosity_weighted_velocity,
        viscosity_drag_coefficient,
        smoothed_pressure_delta: Some((smoothed_pressure_delta * 100.0).round() / 100.0),
        pressure_range: Some((pressure_range * 100.0).round() / 100.0),
        fill_range_pct: Some((fill_range_pct * 100.0).round() / 100.0),
        window_policy,
        authority: "diagnostic_smoothing_not_pressure_control".to_string(),
    })
}

fn pressure_fast_slow_edge_v1(
    valid_pressures: &[f32],
) -> (
    usize,
    usize,
    Option<f32>,
    Option<f32>,
    Option<f32>,
    &'static str,
    bool,
) {
    let slow_window_sample_count = valid_pressures.len();
    if slow_window_sample_count < 2 {
        return (
            slow_window_sample_count,
            slow_window_sample_count,
            None,
            None,
            None,
            "insufficient_history",
            false,
        );
    }

    let fast_window_sample_count = slow_window_sample_count.min(3);
    let fast_start = slow_window_sample_count.saturating_sub(fast_window_sample_count);
    let latest = valid_pressures.last().copied().unwrap_or_default();
    let fast_first = valid_pressures.get(fast_start).copied().unwrap_or(latest);
    let slow_first = valid_pressures.first().copied().unwrap_or(latest);
    let fast_delta = round_flux_delta((latest - fast_first).clamp(-1.0, 1.0));
    let slow_delta = round_flux_delta((latest - slow_first).clamp(-1.0, 1.0));
    let divergence = round_flux_delta((fast_delta - slow_delta).clamp(-1.0, 1.0));
    let fast_edge_preserved = fast_delta.abs() >= 0.04;
    let state = if fast_delta >= 0.04 && divergence >= 0.03 {
        "fast_rising_edge_over_slow_context"
    } else if fast_delta <= -0.04 && divergence <= -0.03 {
        "fast_falling_release_over_slow_context"
    } else if fast_delta >= 0.04 && slow_delta >= 0.04 {
        "rising_edge_carried_by_slow_context"
    } else if fast_delta <= -0.04 && slow_delta <= -0.04 {
        "falling_release_carried_by_slow_context"
    } else if fast_delta.abs() < 0.04 && slow_delta.abs() >= 0.06 {
        "slow_context_without_current_fast_edge"
    } else if fast_edge_preserved {
        "current_fast_edge_visible"
    } else {
        "fast_and_slow_edges_quiet"
    };

    (
        fast_window_sample_count,
        slow_window_sample_count,
        Some(fast_delta),
        Some(slow_delta),
        Some(divergence),
        state,
        fast_edge_preserved,
    )
}

fn build_telemetry_heartbeat_delta_v1(
    previous_arrival_unix_s: Option<f64>,
    latest_arrival_unix_s: f64,
    trace: &WebSocketLaneTrace,
) -> TelemetryHeartbeatDeltaV1 {
    const NORMAL_MAX_MS: f32 = 1_500.0;
    const LATE_MAX_MS: f32 = 5_000.0;

    let inter_arrival_ms = previous_arrival_unix_s.map(|previous| {
        ((latest_arrival_unix_s - previous).max(0.0) * 1000.0).min(f64::from(f32::MAX)) as f32
    });
    let jitter_class = match inter_arrival_ms {
        None => "no_history",
        Some(ms) if ms <= NORMAL_MAX_MS => "normal",
        Some(ms) if ms <= LATE_MAX_MS => "late_packet",
        Some(_) => "stale_packet",
    }
    .to_string();
    let timing_reliability = match jitter_class.as_str() {
        "normal" => "reliable",
        "late_packet" => "timing_ambiguous",
        "stale_packet" => "stale_hearing",
        _ => "insufficient_history",
    }
    .to_string();
    let first_valid_packet_lag_ms = trace
        .active_connection_started_at_unix_s
        .zip(trace.active_connection_first_valid_payload_at_unix_s)
        .map(|(connected_at, first_valid_at)| {
            ((first_valid_at - connected_at).max(0.0) * 1000.0).min(f64::from(f32::MAX)) as f32
        });
    let connection_perception_state = if trace.active_connection_id.is_some()
        && trace.active_connection_valid_payloads_received == 1
    {
        "first_valid_packet_after_connect"
    } else if trace.active_connection_id.is_some()
        && trace.active_connection_valid_payloads_received > 1
    {
        "connected_with_current_telemetry"
    } else if trace.active_connection_id.is_some() {
        "connected_awaiting_valid_telemetry"
    } else if trace.active_connection_first_valid_payload_at_unix_s.is_some() {
        "disconnected_after_valid_telemetry"
    } else {
        "valid_telemetry_without_connection_trace"
    }
    .to_string();
    let cadence_clarity_score = match jitter_class.as_str() {
        "normal" => Some(1.0),
        "late_packet" => Some(0.5),
        "stale_packet" => Some(0.0),
        _ => None,
    };
    let field_vs_hearing = match jitter_class.as_str() {
        "normal" => "telemetry cadence is steady; pressure trend can be read as field movement",
        "late_packet" => {
            "telemetry arrived late; small pressure shifts may be packet-timing artifacts"
        },
        "stale_packet" => {
            "hearing from the field was stale; do not mistake silence for decompression"
        },
        _ => "first telemetry packet; field movement cannot yet be separated from hearing cadence",
    }
    .to_string();
    TelemetryHeartbeatDeltaV1 {
        policy: "telemetry_heartbeat_delta_v1".to_string(),
        schema_version: 1,
        latest_arrival_unix_s: Some(latest_arrival_unix_s),
        previous_arrival_unix_s,
        inter_arrival_ms,
        jitter_class,
        timing_reliability,
        reconnect_count: trace.reconnects,
        disconnect_count: trace.disconnects,
        active_connection_id: trace.active_connection_id,
        active_connection_started_at_unix_s: trace.active_connection_started_at_unix_s,
        first_valid_packet_at_unix_s: trace
            .active_connection_first_valid_payload_at_unix_s,
        first_valid_packet_lag_ms,
        first_valid_spectral_entropy: trace.active_connection_first_valid_spectral_entropy,
        rolling_spectral_entropy_sample_count: 0,
        rolling_spectral_entropy_mean: None,
        peak_spectral_entropy_in_window: None,
        rolling_spectral_entropy_variance: None,
        rolling_spectral_entropy_range: None,
        rolling_spectral_entropy_change: None,
        rolling_spectral_entropy_state: "entropy_window_unavailable".to_string(),
        rolling_spectral_entropy_trend_state: "entropy_trend_unavailable".to_string(),
        rolling_spectral_entropy_basis:
            "bounded_finite_telemetry_samples_latest_minus_earliest_diagnostic_only_not_cadence_felt_state_causation_or_control"
                .to_string(),
        rolling_inter_arrival_sample_count: 0,
        rolling_inter_arrival_mean_ms: None,
        rolling_inter_arrival_change_ms: None,
        rolling_inter_arrival_state: "arrival_window_unavailable".to_string(),
        rolling_inter_arrival_basis:
            "bounded_host_arrival_timestamps_diagnostic_only_not_peer_clock_internal_cycle_felt_state_causation_or_control"
                .to_string(),
        connection_perception_state,
        cadence_clarity_score,
        cadence_clarity_basis:
            "class_derived_observability_evidence_not_subjective_state_or_control".to_string(),
        last_disconnect_reason: trace.last_disconnect_reason.clone(),
        field_vs_hearing,
    }
}

fn attach_rolling_spectral_entropy_v1(
    heartbeat: &mut TelemetryHeartbeatDeltaV1,
    samples: &VecDeque<PressureTrendSampleV1>,
    current_spectral_entropy: Option<f32>,
    window_capacity: usize,
) {
    let prior_limit = window_capacity.saturating_sub(1);
    let mut values = samples
        .iter()
        .rev()
        .filter_map(|sample| sample.spectral_entropy)
        .filter(|value| value.is_finite())
        .take(prior_limit)
        .map(|value| value.clamp(0.0, 1.0))
        .collect::<Vec<_>>();
    values.reverse();
    if let Some(current) = current_spectral_entropy.filter(|value| value.is_finite()) {
        values.push(current.clamp(0.0, 1.0));
    }

    heartbeat.rolling_spectral_entropy_sample_count = values.len();
    if values.is_empty() {
        heartbeat.rolling_spectral_entropy_state = "entropy_window_unavailable".to_string();
        heartbeat.rolling_spectral_entropy_trend_state =
            "entropy_trend_unavailable".to_string();
        return;
    }

    let count = values.len() as f64;
    let mean = values.iter().map(|value| f64::from(*value)).sum::<f64>() / count;
    let variance = values
        .iter()
        .map(|value| {
            let delta = f64::from(*value) - mean;
            delta * delta
        })
        .sum::<f64>()
        / count;
    let (minimum, maximum) = values.iter().copied().fold(
        (f32::INFINITY, f32::NEG_INFINITY),
        |(minimum, maximum), value| (minimum.min(value), maximum.max(value)),
    );

    heartbeat.rolling_spectral_entropy_mean = Some(mean as f32);
    heartbeat.peak_spectral_entropy_in_window = Some(maximum);
    heartbeat.rolling_spectral_entropy_variance = Some(variance.max(0.0) as f32);
    heartbeat.rolling_spectral_entropy_range = Some((maximum - minimum).max(0.0));
    heartbeat.rolling_spectral_entropy_state = if values.len() >= 2 {
        "rolling_variation_available"
    } else {
        "single_entropy_sample"
    }
    .to_string();
    if values.len() >= 2 {
        let change = values.last().copied().unwrap_or_default()
            - values.first().copied().unwrap_or_default();
        heartbeat.rolling_spectral_entropy_change = Some(change);
        heartbeat.rolling_spectral_entropy_trend_state = if change > 0.005 {
            "entropy_diffusing_across_window"
        } else if change < -0.005 {
            "entropy_collapsing_across_window"
        } else {
            "entropy_flat_within_0_005"
        }
        .to_string();
    } else {
        heartbeat.rolling_spectral_entropy_trend_state =
            "single_entropy_sample".to_string();
    }
}

fn attach_rolling_arrival_cadence_v1(
    heartbeat: &mut TelemetryHeartbeatDeltaV1,
    samples: &VecDeque<PressureTrendSampleV1>,
    current_observed_at_unix_s: f64,
    window_capacity: usize,
) {
    let prior_limit = window_capacity.saturating_sub(1);
    let mut arrivals = samples
        .iter()
        .rev()
        .filter_map(|sample| {
            sample
                .observed_at_unix_s
                .is_finite()
                .then_some(sample.observed_at_unix_s)
        })
        .take(prior_limit)
        .collect::<Vec<_>>();
    arrivals.reverse();
    if current_observed_at_unix_s.is_finite() {
        arrivals.push(current_observed_at_unix_s);
    }

    let intervals = arrivals
        .windows(2)
        .filter_map(|pair| {
            let interval_s = pair[1] - pair[0];
            (interval_s.is_finite() && interval_s >= 0.0).then(|| {
                (interval_s * 1000.0).min(f64::from(f32::MAX)) as f32
            })
        })
        .collect::<Vec<_>>();
    heartbeat.rolling_inter_arrival_sample_count = intervals.len();
    if intervals.is_empty() {
        heartbeat.rolling_inter_arrival_state = "arrival_window_unavailable".to_string();
        return;
    }

    let count = intervals.len() as f64;
    let mean = intervals.iter().map(|value| f64::from(*value)).sum::<f64>() / count;
    heartbeat.rolling_inter_arrival_mean_ms = Some(mean as f32);
    if intervals.len() == 1 {
        heartbeat.rolling_inter_arrival_state = "single_inter_arrival_sample".to_string();
        return;
    }

    let change = intervals.last().copied().unwrap_or_default()
        - intervals.first().copied().unwrap_or_default();
    heartbeat.rolling_inter_arrival_change_ms = Some(change);
    heartbeat.rolling_inter_arrival_state = if change > 1.0 {
        "arrival_intervals_lengthening"
    } else if change < -1.0 {
        "arrival_intervals_shortening"
    } else {
        "arrival_intervals_flat_within_one_ms"
    }
    .to_string();
}
