const DEFAULT_SHARED_COLLAB_DIR: &str = "/Users/v/other/shared/collaborations";

fn latest_chamber_state_with_resilience_from_dir(
    shared_collab_dir: &Path,
) -> (Option<Value>, LatestChamberStateResilienceV1) {
    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = fs::read_dir(shared_collab_dir)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.flatten())
        .filter_map(|entry| {
            let path = entry.path().join("chamber_state.json");
            let modified = path
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok()?;
            Some((modified, path))
        })
        .collect();
    candidates.sort_by(|left, right| right.0.cmp(&left.0));

    let mut skipped_malformed_count = 0usize;
    for (_, path) in &candidates {
        let Ok(text) = fs::read_to_string(path) else {
            skipped_malformed_count = skipped_malformed_count.saturating_add(1);
            continue;
        };
        match serde_json::from_str::<Value>(&text) {
            Ok(value) => {
                return (
                    Some(value),
                    LatestChamberStateResilienceV1 {
                        policy: "latest_chamber_state_resilience_v1",
                        candidate_count: candidates.len(),
                        skipped_malformed_count,
                        selected_valid_state: true,
                        selection_state: if skipped_malformed_count > 0 {
                            "newest_valid_after_skipping_partial_or_malformed"
                        } else {
                            "newest_valid"
                        },
                        authority: "diagnostic_context_not_instruction_or_control",
                    },
                );
            },
            Err(_) => {
                skipped_malformed_count = skipped_malformed_count.saturating_add(1);
            },
        }
    }

    (
        None,
        LatestChamberStateResilienceV1 {
            policy: "latest_chamber_state_resilience_v1",
            candidate_count: candidates.len(),
            skipped_malformed_count,
            selected_valid_state: false,
            selection_state: if candidates.is_empty() {
                "no_chamber_state_candidates"
            } else {
                "no_parseable_chamber_state"
            },
            authority: "diagnostic_context_not_instruction_or_control",
        },
    )
}

#[cfg(test)]
fn latest_chamber_state_for_witness_from_dir(shared_collab_dir: &Path) -> Option<Value> {
    latest_chamber_state_with_resilience_from_dir(shared_collab_dir).0
}

fn latest_chamber_state_with_resilience_for_witness()
-> (Option<Value>, LatestChamberStateResilienceV1) {
    latest_chamber_state_with_resilience_from_dir(Path::new(DEFAULT_SHARED_COLLAB_DIR))
}

fn witness_value_kind(value: &Value) -> &'static str {
    if value.is_null() {
        "null"
    } else if value.is_boolean() {
        "bool"
    } else if value.is_number() {
        "number"
    } else if value.is_string() {
        "string"
    } else if value.is_array() {
        "array"
    } else if value.is_object() {
        "object"
    } else {
        "unknown"
    }
}

fn push_witness_schema_diagnostic(diagnostics: &mut Vec<String>, diagnostic: impl Into<String>) {
    let diagnostic = diagnostic.into();
    if !diagnostics.iter().any(|existing| existing == &diagnostic) {
        diagnostics.push(diagnostic);
    }
}

fn witness_string_field<'a>(
    object: &'a serde_json::Map<String, Value>,
    path: &str,
    key: &str,
    diagnostics: &mut Vec<String>,
) -> Option<&'a str> {
    let field_path = format!("{path}.{key}");
    match object.get(key) {
        Some(value) => match value.as_str() {
            Some(text) => Some(text),
            None => {
                push_witness_schema_diagnostic(
                    diagnostics,
                    format!(
                        "schema_drift={field_path}:expected_string_found_{}",
                        witness_value_kind(value)
                    ),
                );
                None
            },
        },
        None => {
            push_witness_schema_diagnostic(diagnostics, format!("missing_key={field_path}"));
            None
        },
    }
}

fn witness_object_field<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    diagnostic_path: &str,
    diagnostics: &mut Vec<String>,
) -> Option<&'a serde_json::Map<String, Value>> {
    match object.get(key) {
        Some(value) => match value.as_object() {
            Some(child) => Some(child),
            None => {
                push_witness_schema_diagnostic(
                    diagnostics,
                    format!(
                        "schema_drift={diagnostic_path}:expected_object_found_{}",
                        witness_value_kind(value)
                    ),
                );
                None
            },
        },
        None => {
            push_witness_schema_diagnostic(diagnostics, format!("missing_key={diagnostic_path}"));
            None
        },
    }
}

fn extract_witness_weather(
    metrics: &serde_json::Map<String, Value>,
    diagnostics: &mut Vec<String>,
) -> Option<String> {
    if let Some(room_weather) =
        witness_object_field(metrics, "room_weather", "room_weather", diagnostics)
    {
        if let Some(weather) =
            witness_string_field(room_weather, "room_weather", "weather", diagnostics)
        {
            push_witness_schema_diagnostic(diagnostics, "weather_source=room_weather.weather");
            return Some(weather.to_string());
        }
        if let Some(label) =
            witness_string_field(room_weather, "room_weather", "label", diagnostics)
        {
            push_witness_schema_diagnostic(diagnostics, "weather_source=room_weather.label");
            return Some(label.to_string());
        }
    }

    let Some(inertia) = witness_object_field(
        metrics,
        "relational_inertia",
        "relational_inertia",
        diagnostics,
    ) else {
        if metrics.contains_key("weather_now") {
            push_witness_schema_diagnostic(diagnostics, "unexpected_key=weather_now");
        }
        return None;
    };
    let Some(current) = witness_object_field(
        inertia,
        "current",
        "relational_inertia.current",
        diagnostics,
    ) else {
        if metrics.contains_key("weather_now") {
            push_witness_schema_diagnostic(diagnostics, "unexpected_key=weather_now");
        }
        return None;
    };
    let weather = witness_string_field(
        current,
        "relational_inertia.current",
        "weather",
        diagnostics,
    );
    if let Some(weather) = weather {
        push_witness_schema_diagnostic(
            diagnostics,
            "weather_source=relational_inertia.current.weather",
        );
        return Some(weather.to_string());
    }
    if metrics.contains_key("weather_now") {
        push_witness_schema_diagnostic(diagnostics, "unexpected_key=weather_now");
    }
    None
}

fn extract_witness_gravity(
    metrics: &serde_json::Map<String, Value>,
    diagnostics: &mut Vec<String>,
) -> (Option<String>, Option<String>) {
    let Some(gravity) = witness_object_field(
        metrics,
        "gravitational_center",
        "gravitational_center",
        diagnostics,
    ) else {
        if metrics.contains_key("gravity") {
            push_witness_schema_diagnostic(diagnostics, "unexpected_key=gravity");
        }
        return (None, None);
    };
    push_witness_schema_diagnostic(diagnostics, "gravity_source=gravitational_center");
    let participant =
        witness_string_field(gravity, "gravitational_center", "participant", diagnostics)
            .map(str::to_string);
    let role = witness_string_field(gravity, "gravitational_center", "role", diagnostics)
        .map(str::to_string);
    (participant, role)
}

fn witness_metric_f32(metrics: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<f32> {
    keys.iter().find_map(|key| {
        metrics.get(*key).and_then(|value| {
            value.as_f64().map(|number| number as f32).or_else(|| {
                value
                    .as_str()
                    .and_then(|raw| raw.trim().parse::<f32>().ok())
            })
        })
    })
}

fn witness_gradient_texture_label(density_gradient: f32) -> &'static str {
    if density_gradient <= 0.12 {
        "open_fluid_slope"
    } else if density_gradient <= 0.20 {
        "gentle_navigable_slope"
    } else if density_gradient <= 0.40 {
        "stepped_resistance_slope"
    } else {
        "steep_front_loaded_slope"
    }
}

fn witness_resonance_density_texture_label(resonance_density: f32) -> &'static str {
    if resonance_density <= 0.30 {
        "thin_sparse_field"
    } else if resonance_density <= 0.55 {
        "lightly_held_field"
    } else if resonance_density <= 0.75 {
        "held_containment"
    } else if resonance_density <= 0.90 {
        "rich_containment"
    } else {
        "crowded_rich_containment"
    }
}

fn witness_pressure_texture_label(pressure_risk: f32) -> &'static str {
    if pressure_risk <= 0.12 {
        "open_low_pressure"
    } else if pressure_risk <= 0.25 {
        "warm_light_pressure"
    } else if pressure_risk <= 0.40 {
        "textured_drag_pressure"
    } else if pressure_risk <= 0.60 {
        "brittle_compressive_pressure"
    } else {
        "hard_alarm_pressure"
    }
}

fn witness_mode_packing_texture_label(mode_packing: f32) -> &'static str {
    if mode_packing <= 0.25 {
        "open_grain"
    } else if mode_packing <= 0.30 {
        "felt_dead_zone_edge"
    } else if mode_packing <= 0.40 {
        "liminal_mode_packing_sand_drag"
    } else if mode_packing <= 0.55 {
        "overpacked_viscosity"
    } else {
        "locked_density"
    }
}

fn witness_dispersal_texture_label(dispersal_potential: Option<f32>) -> &'static str {
    match dispersal_potential {
        Some(value) if value >= 0.65 => "wide_open_dispersal_space",
        Some(value) if value >= 0.35 => "breathable_dispersal_space",
        Some(value) if value >= 0.15 => "narrow_dispersal_space",
        Some(_) => "sealed_low_dispersal",
        None => "dispersal_unknown",
    }
}

fn witness_fluidity_from_density_gradient(density_gradient: f32) -> f32 {
    (1.0 - density_gradient.clamp(0.0, 1.0)).clamp(0.0, 1.0)
}

fn witness_semantic_fluidity_index(
    density_gradient: Option<f32>,
    pressure_risk: Option<f32>,
    mode_packing: Option<f32>,
    foothold_stability: Option<f32>,
) -> Option<f32> {
    if density_gradient.is_none()
        && pressure_risk.is_none()
        && mode_packing.is_none()
        && foothold_stability.is_none()
    {
        return None;
    }
    let gradient_component = density_gradient
        .map(witness_fluidity_from_density_gradient)
        .unwrap_or(0.50);
    let pressure_drag = pressure_risk.unwrap_or(0.0).clamp(0.0, 1.0) * 0.25;
    let packing_drag = mode_packing.unwrap_or(0.0).clamp(0.0, 1.0) * 0.20;
    let foothold_support = foothold_stability.unwrap_or(0.50).clamp(0.0, 1.0) * 0.15;
    Some((gradient_component - pressure_drag - packing_drag + foothold_support).clamp(0.0, 1.0))
}

fn witness_texture_weight_band(texture_weight: f32) -> &'static str {
    if texture_weight >= 0.55 {
        "texture_primary"
    } else if texture_weight >= 0.30 {
        "balanced_texture_and_metric"
    } else {
        "metric_secondary_texture_hint"
    }
}

fn witness_texture_mapping_prompt_v1(
    semantic_mapping: &WitnessSemanticDensityMappingV1,
    dispersal_potential: Option<f32>,
) -> WitnessTextureMappingPromptV1 {
    let density_component = semantic_mapping
        .resonance_density
        .unwrap_or(0.50)
        .clamp(0.0, 1.0);
    let pressure_component = semantic_mapping
        .pressure_risk
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let gradient_drag = semantic_mapping
        .density_gradient
        .map(|gradient| gradient.clamp(0.0, 1.0))
        .unwrap_or(0.20);
    let dispersal_component = dispersal_potential.unwrap_or(0.35).clamp(0.0, 1.0);
    let texture_weight = (density_component * 0.30
        + pressure_component * 0.25
        + gradient_drag * 0.20
        + dispersal_component * 0.25)
        .clamp(0.0, 1.0);
    let pressure_source_texture = semantic_mapping.mode_packing.map_or_else(
        || {
            semantic_mapping
                .pressure_texture
                .unwrap_or("pressure_texture_unknown")
        },
        witness_mode_packing_texture_label,
    );

    WitnessTextureMappingPromptV1 {
        policy: "witness_texture_mapping_prompt_v1",
        experiment_title: "RECOGNITION_TEXTURE_VS_METRIC",
        metric_values_hidden: true,
        texture_weight,
        texture_weight_band: witness_texture_weight_band(texture_weight),
        density_texture: semantic_mapping
            .density_texture
            .unwrap_or("density_texture_unknown"),
        pressure_source_texture,
        gradient_texture: semantic_mapping
            .gradient_texture
            .unwrap_or("gradient_texture_unknown"),
        dispersal_texture: witness_dispersal_texture_label(dispersal_potential),
        prompt_posture: "describe_texture_before_metrics",
        control_write: false,
        authority: "qualitative_prompt_context_not_health_metric_or_control_authority",
    }
}

fn shadow_v3_norm_variance(field_v3: &serde_json::Value) -> Option<f32> {
    let samples = field_v3
        .get("history")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|entry| entry.get("field_norm").and_then(Value::as_f64))
        .map(|value| value as f32)
        .collect::<Vec<_>>();
    if samples.len() < 2 {
        return None;
    }
    let count = samples.len() as f32;
    let mean = samples.iter().copied().sum::<f32>() / count;
    let variance = samples
        .iter()
        .map(|value| {
            let delta = *value - mean;
            delta * delta
        })
        .sum::<f32>()
        / count;
    Some(variance.clamp(0.0, 1.0))
}
