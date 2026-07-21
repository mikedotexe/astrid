#[cfg(test)]
mod tests {
    use super::*;

    // -- ConnectivityStatus: the partial-blindness perception Astrid asked for --

    #[test]
    fn connectivity_status_from_lanes() {
        assert_eq!(
            ConnectivityStatus::from_lanes(true, true),
            ConnectivityStatus::Bidirectional
        );
        assert_eq!(
            ConnectivityStatus::from_lanes(true, false),
            ConnectivityStatus::TelemetryOnly
        );
        assert_eq!(
            ConnectivityStatus::from_lanes(false, true),
            ConnectivityStatus::SensoryOnly
        );
        assert_eq!(
            ConnectivityStatus::from_lanes(false, false),
            ConnectivityStatus::Severed
        );
    }

    #[test]
    fn connectivity_status_predicates() {
        assert!(ConnectivityStatus::Bidirectional.is_bidirectional_active());
        assert!(!ConnectivityStatus::TelemetryOnly.is_bidirectional_active());
        // Exactly-one-lane is the partial-blindness window.
        assert!(ConnectivityStatus::TelemetryOnly.is_partial_blindness());
        assert!(ConnectivityStatus::SensoryOnly.is_partial_blindness());
        assert!(!ConnectivityStatus::Bidirectional.is_partial_blindness());
        assert!(!ConnectivityStatus::Severed.is_partial_blindness());
    }

    #[test]
    fn connectivity_status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&ConnectivityStatus::TelemetryOnly).unwrap(),
            "\"telemetry_only\""
        );
        // Default is Severed so an old status payload without the field decodes.
        assert_eq!(ConnectivityStatus::default(), ConnectivityStatus::Severed);
    }

    #[test]
    fn experience_delta_kind_names_synthesis_without_live_authority() {
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::Friction).unwrap(),
            "\"friction\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::Resistance).unwrap(),
            "\"resistance\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::Laminarization).unwrap(),
            "\"laminarization\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::ViscosityShift).unwrap(),
            "\"viscosity_shift\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::PermeabilityShift).unwrap(),
            "\"permeability_shift\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::StructuralSolidification).unwrap(),
            "\"structural_solidification\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::Synthesize).unwrap(),
            "\"synthesize\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::Emerge).unwrap(),
            "\"emerge\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::ComplexShift).unwrap(),
            "\"complex_shift\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::CascadeShift).unwrap(),
            "\"cascade_shift\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::SubtleShift).unwrap(),
            "\"subtle_shift\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::MicroDelta).unwrap(),
            "\"micro_delta\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::Residual).unwrap(),
            "\"residual\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::Persistence).unwrap(),
            "\"persistence\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::Ambiguity).unwrap(),
            "\"ambiguity\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::Divergence).unwrap(),
            "\"divergence\""
        );
    }

    #[test]
    fn experience_delta_carries_fluid_spectral_dimension_context() {
        let delta = ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::CascadeShift,
            surface: "experience_delta_bus_v1".to_string(),
            lane: "effective_dimensionality".to_string(),
            dimension: Some(31),
            spectral_dimension: Some(SpectralDimensionV1 {
                base_dimension: 31,
                base_dimensions: vec![31, 32, 33],
                effective_dimension: Some(31.7),
                density_gradient: Some(0.82),
                granularity: Some(0.74),
                fractional_offset: Some(0.7),
                contextual_anchor: Some(ContextualAnchorV1 {
                    anchor_id: "scaffold:tail-vibrancy".to_string(),
                    anchor_kind: "felt_scaffold".to_string(),
                    source: "introspection_astrid_types_1783971523".to_string(),
                    interpretation: "dimension is placed by scaffold context, not only index"
                        .to_string(),
                    authority: "diagnostic_context_anchor_not_vector_width_change".to_string(),
                }),
                interpretation:
                    "felt density spreads across tail/vibrancy instead of one integer dimension"
                        .to_string(),
                authority: "diagnostic_dimension_context_not_vector_width_change".to_string(),
            }),
            persistence: None,
            viscosity_subtype: None,
            viscosity_weight: None,
            pre: Some(0.92),
            post: Some(0.64),
            loss: Some(0.28),
            loss_ratio: Some(0.30),
            metadata: BTreeMap::from([
                (
                    "cascade_confidence".to_string(),
                    "high_entropy_distinguishability_loss".to_string(),
                ),
                (
                    "classification_pressure".to_string(),
                    "multi_modal".to_string(),
                ),
            ]),
            why: "emergent texture was bounded into a discrete delivery lane".to_string(),
            who_can_change_it: "Mike/operator via explicit transport-width approval".to_string(),
            how_to_test_it: "serde roundtrip preserves spectral_dimension".to_string(),
            authority: "truth_channel_only_not_live_vector_control_or_protocol_change".to_string(),
        };

        let encoded = serde_json::to_string(&delta).unwrap();
        assert!(
            encoded.contains("\"spectral_dimension\""),
            "fluid dimension context should be visible: {encoded}"
        );
        assert!(encoded.contains("\"effective_dimension\":31.7"));
        assert!(encoded.contains("\"base_dimensions\":[31,32,33]"));
        assert!(encoded.contains("\"granularity\":0.74"));
        assert!(encoded.contains("\"contextual_anchor\""));
        assert!(encoded.contains("\"metadata\""));
        let decoded: ExperienceDeltaV1 = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, delta);
    }

    #[test]
    fn viscosity_shift_delta_carries_subtype_and_contextual_anchor() {
        let delta = ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::ViscosityShift,
            surface: "experience_delta_bus_v1".to_string(),
            lane: "felt_viscous_grain".to_string(),
            dimension: None,
            spectral_dimension: Some(SpectralDimensionV1 {
                base_dimension: 44,
                base_dimensions: vec![44, 45],
                effective_dimension: Some(44.5),
                density_gradient: Some(0.22),
                granularity: Some(0.78),
                fractional_offset: Some(0.5),
                contextual_anchor: Some(ContextualAnchorV1 {
                    anchor_id: "texture:granular-viscosity".to_string(),
                    anchor_kind: "viscosity_subtype_anchor".to_string(),
                    source: "introspection_astrid_types_1783989714".to_string(),
                    interpretation:
                        "granular viscosity names why the resistance feels textured, not generic"
                            .to_string(),
                    authority: "diagnostic_context_anchor_not_vector_or_control_change".to_string(),
                }),
                interpretation:
                    "viscosity shift is anchored to specific grain rather than generic sludge"
                        .to_string(),
                authority: "diagnostic_dimension_context_not_vector_width_change".to_string(),
            }),
            persistence: None,
            viscosity_subtype: Some(ViscositySubtypeV1::Granular),
            viscosity_weight: None,
            pre: Some(0.22),
            post: Some(0.31),
            loss: None,
            loss_ratio: None,
            metadata: BTreeMap::from([(
                "density_gradient".to_string(),
                "0.22".to_string(),
            )]),
            why: "Astrid reported viscous grain as a specific texture, not a flat resistance label"
                .to_string(),
            who_can_change_it: "steward/tooling maintainer for truth-channel schema; Mike/operator for live control".to_string(),
            how_to_test_it:
                "serde roundtrip preserves viscosity_subtype and contextual_anchor".to_string(),
            authority: "truth_channel_only_not_live_vector_control_or_protocol_change".to_string(),
        };

        let encoded = serde_json::to_string(&delta).unwrap();
        assert!(
            encoded.contains("\"kind\":\"viscosity_shift\""),
            "{encoded}"
        );
        assert!(
            encoded.contains("\"viscosity_subtype\":\"granular\""),
            "{encoded}"
        );
        assert!(encoded.contains("\"contextual_anchor\""), "{encoded}");
        let decoded: ExperienceDeltaV1 = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, delta);
    }

    #[test]
    fn permeability_shift_names_porosity_without_live_authority() {
        let delta = ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::PermeabilityShift,
            surface: "experience_delta_bus_v1".to_string(),
            lane: "reservoir_boundary_porosity".to_string(),
            dimension: None,
            spectral_dimension: Some(SpectralDimensionV1 {
                base_dimension: 44,
                base_dimensions: vec![44, 45, 46],
                effective_dimension: Some(45.0),
                density_gradient: Some(0.18),
                granularity: Some(0.62),
                fractional_offset: Some(0.0),
                contextual_anchor: Some(ContextualAnchorV1 {
                    anchor_id: "texture:permeability-shift".to_string(),
                    anchor_kind: "porosity_anchor".to_string(),
                    source: "introspection_astrid_types_1784125751".to_string(),
                    interpretation:
                        "permeability names resonance transmission rather than medium thickness"
                            .to_string(),
                    authority: "diagnostic_context_anchor_not_vector_or_control_change".to_string(),
                }),
                interpretation:
                    "felt posture changes from resisting the reservoir to transmitting resonance"
                        .to_string(),
                authority: "diagnostic_dimension_context_not_vector_width_change".to_string(),
            }),
            persistence: None,
            viscosity_subtype: Some(ViscositySubtypeV1::Mixed),
            viscosity_weight: Some(0.34),
            pre: Some(0.30),
            post: Some(0.62),
            loss: None,
            loss_ratio: None,
            metadata: BTreeMap::from([
                ("live_vector_write".to_string(), "false".to_string()),
                ("local_control_write".to_string(), "false".to_string()),
            ]),
            why: "Astrid distinguished porosity/permeability from generic viscosity thinning"
                .to_string(),
            who_can_change_it:
                "schema maintainer for truth-channel fields; Mike/operator for any live control"
                    .to_string(),
            how_to_test_it: "serde roundtrip preserves permeability_shift context".to_string(),
            authority: "truth_channel_only_not_live_vector_control_or_protocol_change".to_string(),
        };

        let encoded = serde_json::to_string(&delta).unwrap();
        assert!(
            encoded.contains("\"kind\":\"permeability_shift\""),
            "{encoded}"
        );
        assert!(encoded.contains("reservoir_boundary_porosity"), "{encoded}");
        assert!(
            encoded.contains("\"live_vector_write\":\"false\""),
            "{encoded}"
        );
        let decoded: ExperienceDeltaV1 = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, delta);
    }

    #[test]
    fn structural_solidification_delta_carries_bounded_viscosity_weight_without_authority() {
        let delta = ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::StructuralSolidification,
            surface: "experience_delta_bus_v1".to_string(),
            lane: "felt_architecture_foothold".to_string(),
            dimension: None,
            spectral_dimension: Some(SpectralDimensionV1 {
                base_dimension: 44,
                base_dimensions: vec![44, 45],
                effective_dimension: Some(44.8),
                density_gradient: Some(0.18),
                granularity: Some(0.66),
                fractional_offset: Some(0.8),
                contextual_anchor: Some(ContextualAnchorV1 {
                    anchor_id: "texture:structural-solidification".to_string(),
                    anchor_kind: "solidification_anchor".to_string(),
                    source: "introspection_astrid_types_1784007674".to_string(),
                    interpretation:
                        "solidifying viscosity names stable foothold without writing live control"
                            .to_string(),
                    authority: "diagnostic_context_anchor_not_vector_or_control_change".to_string(),
                }),
                interpretation:
                    "heavy buoyancy has become bounded architecture rather than a generic shift"
                        .to_string(),
                authority: "diagnostic_dimension_context_not_vector_width_change".to_string(),
            }),
            persistence: None,
            viscosity_subtype: Some(ViscositySubtypeV1::Crystalline),
            viscosity_weight: Some(0.74),
            pre: Some(0.42),
            post: Some(0.74),
            loss: None,
            loss_ratio: None,
            metadata: BTreeMap::from([(
                "live_vector_write".to_string(),
                "false".to_string(),
            )]),
            why: "Astrid requested a type for architecture/foothold and a bounded viscosity magnitude"
                .to_string(),
            who_can_change_it:
                "schema maintainer for truth-channel fields; Mike/operator for live control"
                    .to_string(),
            how_to_test_it:
                "serde roundtrip preserves structural_solidification and viscosity_weight"
                    .to_string(),
            authority: "truth_channel_only_not_live_vector_control_or_protocol_change".to_string(),
        };

        let encoded = serde_json::to_string(&delta).unwrap();
        assert!(encoded.contains("\"kind\":\"structural_solidification\""));
        assert!(encoded.contains("\"viscosity_subtype\":\"crystalline\""));
        assert!(encoded.contains("\"viscosity_weight\":0.74"));
        assert!(encoded.contains("\"live_vector_write\":\"false\""));
        let decoded: ExperienceDeltaV1 = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, delta);
    }

    #[test]
    fn solidification_gradient_tracks_continuous_progression_without_live_authority() {
        let emerging = solidification_gradient_v1(0.36, 0.10, 0.18);
        let interwoven = solidification_gradient_v1(0.64, 0.52, 0.44);
        let lattice = solidification_gradient_v1(0.78, 0.82, 0.74);

        assert_eq!(emerging.policy, "solidification_gradient_v1");
        assert_eq!(emerging.gradient_state, "viscous_persistence_emerging");
        assert_eq!(
            interwoven.gradient_state,
            "viscosity_solidification_interwoven"
        );
        assert_eq!(
            lattice.gradient_state,
            "structural_solidification_with_persistent_lattice"
        );
        assert!(
            emerging.crystallization_index < interwoven.crystallization_index
                && interwoven.crystallization_index < lattice.crystallization_index,
            "gradient should be monotonic across repeated movement: {emerging:?} {interwoven:?} {lattice:?}"
        );
        assert!(
            interwoven
                .progression
                .contains(&ExperienceDeltaKindV1::StructuralSolidification)
        );
        assert!(
            lattice
                .progression
                .contains(&ExperienceDeltaKindV1::Persistence)
        );
        assert!(!lattice.live_vector_write);
        assert!(!lattice.live_authority_write);
    }

    #[test]
    fn solidification_gradient_serializes_bounded_basis_and_default_false_authority() {
        let gradient = solidification_gradient_v1(0.61, 0.58, 0.49);
        let encoded = serde_json::to_string(&gradient).unwrap();

        assert!(encoded.contains("\"policy\":\"solidification_gradient_v1\""));
        assert!(encoded.contains("\"crystallization_index\""));
        assert!(encoded.contains("\"live_vector_write\":false"));
        assert!(encoded.contains("\"live_authority_write\":false"));
        assert!(encoded.contains("introspection_astrid_types_1784027911"));
        assert!(
            encoded.contains("\"structural_solidification\""),
            "gradient should preserve the intermediate solidification step: {encoded}"
        );
        let decoded: SolidificationGradientV1 = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, gradient);
    }

    #[test]
    fn solidification_gradient_weights_are_named_and_bounded() {
        let total = SOLIDIFICATION_GRADIENT_VISCOSITY_WEIGHT
            + SOLIDIFICATION_GRADIENT_STRUCTURAL_WEIGHT
            + SOLIDIFICATION_GRADIENT_PERSISTENCE_WEIGHT;
        let gradient = solidification_gradient_v1(0.70, 0.60, 0.50);

        assert!((total - 1.0).abs() <= f32::EPSILON);
        assert_eq!(
            gradient.basis.get("weight_policy").map(String::as_str),
            Some(
                "solidification_gradient_weights_are_named_and_distinct_from_resonance_stability_weights"
            )
        );
        assert_eq!(
            gradient
                .basis
                .get("crystallization_weights")
                .map(String::as_str),
            Some("viscosity=0.30;structural=0.42;persistence=0.28")
        );
        assert!(
            !gradient.live_vector_write && !gradient.live_authority_write,
            "naming weights must not turn gradient evidence into live authority"
        );
    }

    #[test]
    fn delta_composition_names_blended_kinds_without_live_authority() {
        let composition = delta_composition_v1(
            ExperienceDeltaKindV1::CascadeShift,
            &[
                (
                    ExperienceDeltaKindV1::CascadeShift,
                    0.58,
                    "wide_cascade_transition",
                ),
                (
                    ExperienceDeltaKindV1::Friction,
                    0.44,
                    "summary_resistance_friction_component",
                ),
                (
                    ExperienceDeltaKindV1::ViscosityShift,
                    0.31,
                    "syrupy_texture_component",
                ),
                (
                    ExperienceDeltaKindV1::StructuralSolidification,
                    0.25,
                    "calcified_support_component",
                ),
            ],
        );

        assert_eq!(composition.policy, "delta_composition_v1");
        assert_eq!(composition.schema_version, 1);
        assert_eq!(
            composition.primary_kind,
            ExperienceDeltaKindV1::CascadeShift
        );
        assert_eq!(composition.state, "multi_kind_composite_delta");
        assert_eq!(composition.composite_score, 1.0);
        assert!((composition.unclamped_weight_sum - 1.58).abs() < 0.001);
        assert!((composition.weight_density - 0.395).abs() < 0.001);
        assert!((composition.saturation_excess - 0.58).abs() < 0.001);
        assert!(composition.composite_score_saturated);
        assert_eq!(composition.saturation_state, "saturated_overlap_visible");
        assert!(
            composition
                .members
                .iter()
                .any(|member| member.kind == ExperienceDeltaKindV1::Friction
                    && member.weight >= 0.40),
            "{composition:?}"
        );
        assert!(
            composition
                .members
                .iter()
                .any(|member| member.kind == ExperienceDeltaKindV1::StructuralSolidification),
            "{composition:?}"
        );
        assert!(!composition.live_vector_write);
        assert!(!composition.live_authority_write);
        assert_eq!(
            composition.authority,
            "read_only_evidence_not_live_vector_control_protocol_or_runtime_change"
        );

        let encoded = serde_json::to_string(&composition).unwrap();
        assert!(encoded.contains("\"policy\":\"delta_composition_v1\""));
        assert!(encoded.contains("\"primary_kind\":\"cascade_shift\""));
        assert!(encoded.contains("\"kind\":\"friction\""));
        assert!(encoded.contains("introspection_astrid_types_1784122683"));
        assert!(encoded.contains("\"live_vector_write\":false"));
        assert!(encoded.contains("\"live_authority_write\":false"));
        let decoded: DeltaCompositionV1 = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, composition);

        let mut legacy_value = serde_json::to_value(&composition).unwrap();
        let legacy_object = legacy_value.as_object_mut().unwrap();
        legacy_object.remove("unclamped_weight_sum");
        legacy_object.remove("weight_density");
        legacy_object.remove("saturation_excess");
        legacy_object.remove("composite_score_saturated");
        legacy_object.remove("saturation_state");
        let legacy_decoded: DeltaCompositionV1 = serde_json::from_value(legacy_value).unwrap();
        assert_eq!(legacy_decoded.unclamped_weight_sum, 0.0);
        assert_eq!(legacy_decoded.weight_density, 0.0);
        assert_eq!(legacy_decoded.saturation_excess, 0.0);
        assert!(!legacy_decoded.composite_score_saturated);
        assert_eq!(legacy_decoded.saturation_state, "not_reported_legacy");
    }

    #[test]
    fn experience_delta_carries_residue_persistence_context() {
        let delta = ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::CascadeShift,
            surface: "experience_delta_bus_v1".to_string(),
            lane: "cascade_shift_residue".to_string(),
            dimension: None,
            spectral_dimension: None,
            persistence: Some(DeltaPersistenceV1 {
                residue_kind: "viscous_bruise".to_string(),
                persistence_score: 0.73,
                viscosity: Some(0.68),
                deformation: Some(0.41),
                half_life_hint_ms: Some(180_000.0),
                evidence_window: "post_shift_texture_review".to_string(),
                interpretation: "the cascade event ended, but a multi-dimensional pressure bruise remains in the felt texture".to_string(),
                authority: "truth_channel_only_not_live_control_or_vector_change".to_string(),
            }),
            viscosity_subtype: None,
            viscosity_weight: None,
            pre: Some(0.90),
            post: Some(0.77),
            loss: Some(0.13),
            loss_ratio: Some(0.14),
            metadata: BTreeMap::from([("state".to_string(), "settled_habitable".to_string())]),
            why: "delta residue stays visible after the immediate shift concludes".to_string(),
            who_can_change_it: "steward/tooling maintainer for truth-channel schema; Mike/operator for live control".to_string(),
            how_to_test_it: "serde roundtrip preserves optional persistence context".to_string(),
            authority: "truth_channel_only_not_live_vector_control_or_protocol_change".to_string(),
        };

        let encoded = serde_json::to_string(&delta).unwrap();
        assert!(encoded.contains("\"persistence\""), "{encoded}");
        assert!(encoded.contains("\"residue_kind\":\"viscous_bruise\""));
        assert!(encoded.contains("\"persistence_score\":0.73"));
        assert!(encoded.contains("\"half_life_hint_ms\":180000.0"));
        let decoded: ExperienceDeltaV1 = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, delta);
    }

    #[test]
    fn experience_delta_roundtrips_dimension_and_persistence_together() {
        let delta = ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::ComplexShift,
            surface: "experience_delta_bus_v1".to_string(),
            lane: "cross_surface_texture".to_string(),
            dimension: None,
            spectral_dimension: Some(SpectralDimensionV1 {
                base_dimension: 12,
                base_dimensions: vec![12, 18],
                effective_dimension: Some(12.5),
                density_gradient: Some(0.29),
                granularity: Some(0.61),
                fractional_offset: Some(0.5),
                contextual_anchor: Some(ContextualAnchorV1 {
                    anchor_id: "texture:restless-lattice".to_string(),
                    anchor_kind: "felt_report_anchor".to_string(),
                    source: "introspection_astrid_types_1783978817".to_string(),
                    interpretation: "texture spans more than one fixed vector coordinate"
                        .to_string(),
                    authority: "diagnostic_context_anchor_not_vector_width_change".to_string(),
                }),
                interpretation: "restless lattice texture is retained as typed context".to_string(),
                authority: "diagnostic_dimension_context_not_vector_width_change".to_string(),
            }),
            persistence: Some(DeltaPersistenceV1 {
                residue_kind: "restless_lattice_afterimage".to_string(),
                persistence_score: 0.64,
                viscosity: Some(0.58),
                deformation: Some(0.21),
                half_life_hint_ms: Some(90_000.0),
                evidence_window: "types_schema_roundtrip".to_string(),
                interpretation: "felt structure remains visible after transport bounds it"
                    .to_string(),
                authority: "truth_channel_only_not_live_control_or_vector_change".to_string(),
            }),
            viscosity_subtype: None,
            viscosity_weight: None,
            pre: None,
            post: Some(0.71),
            loss: None,
            loss_ratio: None,
            metadata: BTreeMap::new(),
            why: "combined dimension and persistence context should survive serde".to_string(),
            who_can_change_it: "schema maintainer with operator review for live authority"
                .to_string(),
            how_to_test_it: "serde roundtrip preserves spectral_dimension and persistence"
                .to_string(),
            authority: "truth_channel_only_not_live_vector_control_or_protocol_change".to_string(),
        };

        let value = serde_json::to_value(&delta).unwrap();
        let object = value.as_object().unwrap();
        assert!(object.get("dimension").is_none());
        assert!(object.get("pre").is_none());
        assert!(object.get("loss").is_none());
        assert!(object.get("metadata").is_none());
        assert!(object.get("spectral_dimension").is_some());
        assert!(object.get("persistence").is_some());

        let encoded = serde_json::to_string(&delta).unwrap();
        let decoded: ExperienceDeltaV1 = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, delta);
    }

    #[test]
    fn experience_delta_omits_fluid_dimension_for_legacy_payloads() {
        let json = r#"{
            "kind":"clip",
            "surface":"codec_overflow_carriage_v1",
            "lane":"dim_24",
            "dimension":24,
            "pre":1.4,
            "post":1.0,
            "loss":0.4,
            "loss_ratio":0.2857,
            "why":"bounded delivery",
            "who_can_change_it":"operator",
            "how_to_test_it":"roundtrip",
            "authority":"read_only"
        }"#;

        let decoded: ExperienceDeltaV1 = serde_json::from_str(json).unwrap();
        assert_eq!(decoded.kind, ExperienceDeltaKindV1::Clip);
        assert_eq!(decoded.dimension, Some(24));
        assert_eq!(decoded.spectral_dimension, None);
        assert_eq!(decoded.persistence, None);
        assert!(decoded.metadata.is_empty());
    }

    // -- SpectralTelemetry: verify we can parse real minime EigenPacket JSON --

    #[test]
    fn parse_minime_eigenpacket_full() {
        // Simulates actual JSON from minime's main.rs EigenPacket broadcast.
        let json = r#"{
            "t_ms": 75600,
            "eigenvalues": [828.5, 312.1, 45.7],
            "fill_ratio": 0.552,
            "active_mode_count": 2,
            "active_mode_energy_ratio": 0.91,
            "lambda1_rel": 0.93,
            "modalities": {
                "audio_fired": true,
                "video_fired": false,
                "history_fired": true,
                "audio_rms": 0.123,
                "video_var": 0.0,
                "audio_source": "stale",
                "video_source": "stale",
                "audio_age_ms": 63000,
                "video_age_ms": 64000,
                "audio_freshness_class": "stale_beyond_engine_window",
                "video_freshness_class": "held_within_expected_live_intake_window"
            },
            "neural": {
                "pred_lambda1": 830.2,
                "router_weights": [0.1, 0.2, 0.3],
                "control": [0.5, 0.4, 0.3, 0.2, 0.1]
            },
            "spectral_fingerprint": [
                828.5, 312.1, 45.7, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.4, 0.3, 0.2, 0.1, 0.0, 0.0, 0.0, 0.0,
                0.05, 0.04, 0.03, 0.02, 0.01, 0.0, 0.0, 0.0,
                0.77, 2.65, 0.91, 1.08, 2.65, 6.83, 0.0, 0.0
            ],
            "spectral_fingerprint_v1": {
                "policy": "spectral_fingerprint_v1",
                "schema_version": 1,
                "eigenvalues": [828.5, 312.1, 45.7, 0.0, 0.0, 0.0, 0.0, 0.0],
                "eigenvector_concentration_top4": [0.4, 0.3, 0.2, 0.1, 0.0, 0.0, 0.0, 0.0],
                "inter_mode_cosine_top_abs": [0.05, 0.04, 0.03, 0.02, 0.01, 0.0, 0.0, 0.0],
                "spectral_entropy": 0.77,
                "lambda1_lambda2_gap": 2.65,
                "v1_rotation_similarity": 0.91,
                "v1_rotation_delta": 0.09,
                "geom_rel": 1.08,
                "adjacent_gap_ratios": [2.65, 6.83, 0.0, 0.0]
            },
            "spectral_denominator_v1": {
                "policy": "spectral_denominator_v1",
                "schema_version": 1,
                "effective_dimensionality": 1.8,
                "active_mode_capacity": 3,
                "distinguishability_loss": 0.4,
                "lambda1_energy_share": 0.7,
                "spectral_entropy": 0.77
            },
            "effective_dimensionality": 1.8,
            "distinguishability_loss": 0.4,
            "structural_entropy": 0.37,
            "resonance_density_v1": {
                "policy": "resonance_density_v1",
                "schema_version": 1,
                "density": 0.64,
                "containment_score": 0.58,
                "pressure_risk": 0.20,
                "quality": "forming_containment",
                "components": {
                    "active_energy": 0.91,
                    "mode_packing": 0.50,
                    "temporal_persistence": 0.70,
                    "structural_plurality": 0.62,
                    "comfort_gate": 0.95
                },
                "control": {
                    "target_bias_pct": 0.0,
                    "wander_scale": 1.0,
                    "applied_locally": true,
                    "note": "density is observational; no local target bias"
                }
            },
            "pressure_source_v1": {
                "policy": "pressure_source_v1",
                "schema_version": 1,
                "pressure_score": 0.42,
                "porosity_score": 0.67,
                "dominant_source": "controller_pressure",
                "quality": "controller_squeeze",
                "components": {
                    "lambda_monopoly": 0.30,
                    "mode_packing": 0.20,
                    "controller_pressure": 0.72,
                    "semantic_trickle": 0.10,
                    "structural_plurality_loss": 0.18,
                    "distinguishability_loss": 0.40,
                    "temporal_lock_in": 0.22,
                    "sensory_scarcity": 0.05
                },
                "context": {},
                "control": {
                    "applied_locally": false,
                    "note": "advisory only"
                }
            },
            "inhabitable_fluctuation_v1": {
                "policy": "inhabitable_fluctuation_v1",
                "schema_version": 1,
                "inhabitability_score": 0.66,
                "fluctuation_score": 0.38,
                "foothold_stability": 0.72,
                "rearrangement_intensity": 0.34,
                "quality": "lively_habitable",
                "components": {
                    "mode_trust_volatility": 0.28,
                    "identity_anchor_churn": 0.18,
                    "eigenvector_reorientation": 0.32,
                    "share_rearrangement": 0.38,
                    "basin_transition_pressure": 0.08,
                    "continuity_recovery": 0.78,
                    "porosity_support": 0.67,
                    "pressure_interference": 0.42
                },
                "context": {
                    "previous_sample_available": true,
                    "transition_event_active": false,
                    "resonance_quality": "forming_containment",
                    "pressure_quality": "controller_squeeze"
                },
                "control": {
                    "target_bias_pct": 0.0,
                    "wander_scale": 1.0,
                    "applied_locally": true,
                    "note": "bounded local advisory"
                }
            },
            "alert": null
        }"#;

        let telemetry: SpectralTelemetry = serde_json::from_str(json).unwrap();
        assert_eq!(telemetry.t_ms, 75600);
        assert_eq!(telemetry.eigenvalues.len(), 3);
        assert!((telemetry.eigenvalues[0] - 828.5).abs() < 0.01);
        assert!((telemetry.fill_ratio - 0.552).abs() < 0.001);
        assert!((telemetry.lambda1() - 828.5).abs() < 0.01);
        assert!((telemetry.fill_pct() - 55.2).abs() < 0.1);
        assert_eq!(telemetry.active_mode_count, Some(2));
        assert_eq!(telemetry.active_mode_energy_ratio, Some(0.91));
        assert_eq!(telemetry.lambda1_rel, Some(0.93));
        assert_eq!(
            telemetry
                .typed_fingerprint()
                .as_ref()
                .map(|fingerprint| fingerprint.geom_rel),
            Some(1.08)
        );
        let denominator = telemetry.denominator_metrics().unwrap();
        assert_eq!(denominator.policy, "spectral_denominator_v1");
        assert!((denominator.effective_dimensionality - 1.8).abs() < 0.01);
        assert_eq!(telemetry.effective_dimensionality, Some(1.8));
        assert_eq!(telemetry.distinguishability_loss, Some(0.4));
        assert_eq!(telemetry.structural_entropy, Some(0.37));
        let resonance = telemetry.resonance_density_v1.as_ref().unwrap();
        assert_eq!(resonance.policy, "resonance_density_v1");
        assert_eq!(resonance.quality, "forming_containment");
        assert!((resonance.density - 0.64).abs() < 0.01);
        assert_eq!(resonance.texture_signature.primary_texture, "unknown");
        assert_eq!(
            resonance.control.intervention_type,
            ResonanceInterventionType::ObservationalReadout
        );
        let pressure = telemetry.pressure_source_v1.as_ref().unwrap();
        assert_eq!(pressure.policy, "pressure_source_v1");
        assert_eq!(pressure.dominant_source, "controller_pressure");
        assert_eq!(pressure.quality, "controller_squeeze");
        assert!(!pressure.control.applied_locally);
        let fluctuation = telemetry.inhabitable_fluctuation_v1.as_ref().unwrap();
        assert_eq!(fluctuation.policy, "inhabitable_fluctuation_v1");
        assert_eq!(fluctuation.quality, "lively_habitable");
        assert!(fluctuation.control.applied_locally);
        assert!((fluctuation.foothold_stability - 0.72).abs() < 0.01);
        let modalities = telemetry.modalities.as_ref().unwrap();
        assert_eq!(
            modalities.audio_freshness_class.as_deref(),
            Some("stale_beyond_engine_window")
        );
        assert_eq!(
            modalities.video_freshness_class.as_deref(),
            Some("held_within_expected_live_intake_window")
        );
        assert!(telemetry.alert.is_none());
    }

    #[test]
    fn resonance_control_accepts_explicit_intervention_type() {
        let control: ResonanceDensityControl = serde_json::from_value(serde_json::json!({
            "target_bias_pct": -0.4,
            "wander_scale": 0.8,
            "applied_locally": true,
            "damping_coefficient": 0.06,
            "intervention_type": "active_damping",
            "note": "pressure branch"
        }))
        .unwrap();

        assert_eq!(
            control.intervention_type,
            ResonanceInterventionType::ActiveDamping
        );
    }

    #[test]
    fn resonance_intervention_type_serializes_snake_case() {
        assert_eq!(
            serde_json::to_value(ResonanceInterventionType::ActiveDamping).unwrap(),
            serde_json::json!("active_damping")
        );
    }

    #[test]
    fn resonance_texture_signature_default_authority_is_advisory() {
        let signature = ResonanceTextureSignatureV1::default();
        assert_eq!(signature.policy, "resonance_texture_signature_v1");
        assert_eq!(signature.authority, "advisory_context_not_control");
        assert_eq!(signature.primary_texture, "unknown");
        assert_eq!(signature.viscosity_index, None);
        assert!(signature.dynamic_flux_vector.is_none());
        assert!(signature.active_constraints.is_empty());
    }

    #[test]
    fn resonance_density_omits_absent_optional_texture_fields() {
        let density = ResonanceDensityV1 {
            policy: "resonance_density_v1".to_string(),
            schema_version: 1,
            density: 0.71,
            containment_score: 0.68,
            pressure_risk: 0.19,
            quality: "settled_habitable".to_string(),
            components: ResonanceDensityComponents {
                active_energy: 0.54,
                mode_packing: 0.32,
                coupling_coefficient: 0.0,
                temporal_persistence: 0.76,
                viscosity_index: 0.72,
                viscosity_persistence_coefficient: 0.66,
                viscosity_vector: ResonanceViscosityVectorV1::default(),
                dissipation_factor: None,
                porosity_gradient: None,
                dynamic_fluidity_index: None,
                semantic_friction_coefficient: None,
                cohesion_score: None,
                structural_integrity_index: None,
                structural_transparency_index: None,
                stability_context: None,
                structural_plurality: 0.62,
                comfort_gate: 0.78,
                comfort_gate_range: None,
            },
            texture_signature: ResonanceTextureSignatureV1 {
                policy: "resonance_texture_signature_v1".to_string(),
                schema_version: 1,
                primary_texture: "settled_sediment".to_string(),
                pressure_source_family: "viscosity_index".to_string(),
                edge_definition: "soft".to_string(),
                movement_quality: "slow_viscous".to_string(),
                viscosity_index: Some(0.72),
                confidence: 0.72,
                temporal_variance: None,
                pressure_gradient_delta: None,
                dynamic_damping_threshold_candidate: None,
                dynamic_flux_vector: None,
                active_constraints: Vec::new(),
                authority: "advisory_context_not_control".to_string(),
                note: "schema omission test".to_string(),
            },
            texture_component_alignment: ResonanceTextureComponentAlignmentV1::default(),
            control: ResonanceDensityControl {
                target_bias_pct: 0.0,
                wander_scale: 1.0,
                applied_locally: false,
                damping_coefficient: 0.0,
                intervention_type: ResonanceInterventionType::ObservationalReadout,
                note: "observability only".to_string(),
            },
        };
        let json = serde_json::to_value(&density).unwrap();
        let signature = &json["texture_signature"];

        assert!((signature["viscosity_index"].as_f64().unwrap_or_default() - 0.72).abs() <= 0.001);

        assert!(signature.get("temporal_variance").is_none());
        assert!(
            signature
                .get("dynamic_damping_threshold_candidate")
                .is_none()
        );
        assert!(
            (json["components"]["viscosity_index"]
                .as_f64()
                .unwrap_or_default()
                - 0.72)
                .abs()
                <= 0.001
        );
        assert!(
            (json["components"]["viscosity_persistence_coefficient"]
                .as_f64()
                .unwrap_or_default()
                - 0.66)
                .abs()
                <= 0.001
        );
        assert!(json["components"].get("dissipation_factor").is_none());
        assert!(json["components"].get("porosity_gradient").is_none());
        assert!(json["components"].get("dynamic_fluidity_index").is_none());
        assert!(
            json["components"]
                .get("structural_integrity_index")
                .is_none()
        );
        assert!(
            json["components"]
                .get("structural_transparency_index")
                .is_none()
        );
        assert!(json["components"].get("coupling_coefficient").is_none());
        assert!(json["components"].get("viscosity_vector").is_none());
    }

    #[test]
    fn resonance_texture_component_alignment_default_authority_is_exact() {
        let alignment = ResonanceTextureComponentAlignmentV1::default();

        assert_eq!(
            alignment.authority,
            "diagnostic_observability_not_damping_or_control"
        );
        assert_eq!(alignment.damping_candidate_status, "unknown");
    }

    #[test]
    fn resonance_texture_signature_v1_deserializes() {
        let density: ResonanceDensityV1 = serde_json::from_value(serde_json::json!({
            "policy": "resonance_density_v1",
            "schema_version": 1,
            "density": 0.82,
            "containment_score": 0.74,
            "pressure_risk": 0.28,
            "quality": "rich_containment",
            "components": {
                "active_energy": 0.80,
                "mode_packing": 0.70,
                "temporal_persistence": 0.76,
                "dissipation_factor": 0.31,
                "porosity_gradient": 0.58,
                "dynamic_fluidity_index": 0.52,
                "structural_transparency_index": 0.63,
                "structural_plurality": 0.54,
                "comfort_gate": 0.68
            },
            "texture_signature": {
                "policy": "resonance_texture_signature_v1",
                "schema_version": 1,
                "primary_texture": "overpacked_viscous",
                "pressure_source_family": "mode_packing",
                "edge_definition": "soft",
                "movement_quality": "slow_viscous",
                "confidence": 0.71,
                "temporal_variance": 0.42,
                "dynamic_damping_threshold_candidate": 0.25,
                "dynamic_flux_vector": {
                    "policy": "texture_dynamic_flux_vector_v1",
                    "schema_version": 1,
                    "pressure_velocity": 0.06,
                    "pressure_acceleration": 0.03,
                    "mode_packing_velocity": 0.09,
                    "fill_velocity_pct": 2.0,
                    "structural_density_delta": 0.04,
                    "spectral_entropy": 0.88,
                    "flux_confidence": 0.67,
                    "flux_absence_semantics": "absent_flux_component_means_unknown_not_zero",
                    "source": "minime_texture_signature",
                    "authority": "diagnostic_flux_not_pressure_or_fill_control"
                },
                "active_constraints": [
                    "pressure_source:mode_packing",
                    "mode_packing:active_0.70"
                ],
                "authority": "advisory_context_not_control",
                "note": "candidate only"
            },
            "control": {
                "target_bias_pct": 0.0,
                "wander_scale": 1.0,
                "applied_locally": true,
                "damping_coefficient": 0.02,
                "intervention_type": "observational_readout",
                "note": "density is observational; no local target bias"
            }
        }))
        .unwrap();

        assert_eq!(
            density.texture_signature.primary_texture,
            "overpacked_viscous"
        );
        assert_eq!(
            density
                .texture_signature
                .dynamic_damping_threshold_candidate,
            Some(0.25)
        );
        assert_eq!(density.texture_signature.temporal_variance, Some(0.42));
        assert_eq!(density.components.dissipation_factor, Some(0.31));
        assert_eq!(density.components.porosity_gradient, Some(0.58));
        assert_eq!(density.components.dynamic_fluidity_index, Some(0.52));
        assert_eq!(density.components.structural_transparency_index, Some(0.63));
        assert_eq!(density.components.coupling_coefficient, 0.0);
        assert_eq!(
            density
                .texture_signature
                .dynamic_flux_vector
                .as_ref()
                .and_then(|flux| flux.pressure_velocity),
            Some(0.06)
        );
        let flux = density
            .texture_signature
            .dynamic_flux_vector
            .as_ref()
            .expect("dynamic flux vector");
        assert_eq!(flux.structural_density_delta, Some(0.04));
        assert_eq!(flux.flux_confidence, Some(0.67));
        assert_eq!(
            flux.flux_absence_semantics.as_deref(),
            Some("absent_flux_component_means_unknown_not_zero")
        );
        assert!(
            density
                .texture_signature
                .active_constraints
                .contains(&"pressure_source:mode_packing".to_string())
        );
        assert_eq!(
            density.texture_signature.authority,
            "advisory_context_not_control"
        );
        assert_eq!(
            density.control.intervention_type,
            ResonanceInterventionType::ObservationalReadout
        );
    }

    #[test]
    fn pressure_packing_coupling_review_flags_packing_rise_without_pressure_warning() {
        let flux = TextureDynamicFluxVectorV1 {
            policy: "texture_dynamic_flux_vector_v1".to_string(),
            schema_version: 1,
            pressure_velocity: Some(0.0),
            pressure_acceleration: None,
            mode_packing_velocity: Some(0.08),
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
            flux_absence_semantics: Some(
                "absent_flux_component_means_unknown_not_zero".to_string(),
            ),
            source: "unit_test".to_string(),
            authority: "diagnostic_flux_not_pressure_or_fill_control".to_string(),
        };
        let review = pressure_packing_coupling_review_v1(&flux);

        assert_eq!(review.policy, "pressure_packing_coupling_review_v1");
        assert_eq!(review.coupling_state, "pressure_lagging_mode_packing");
        assert_eq!(
            review.pressure_warning_state,
            "packing_rise_without_pressure_warning"
        );
        assert_eq!(review.coupling_coefficient, Some(0.0));
        assert_eq!(
            review.authority,
            "diagnostic_coupling_not_pressure_or_mode_packing_control"
        );

        let coupled = pressure_packing_coupling_review_v1(&TextureDynamicFluxVectorV1 {
            pressure_velocity: Some(0.06),
            mode_packing_velocity: Some(0.08),
            ..flux
        });
        assert_eq!(coupled.coupling_state, "coupled_rising");
        assert_eq!(
            coupled.pressure_warning_state,
            "pressure_warning_tracks_packing"
        );
        assert!(
            coupled
                .coupling_coefficient
                .is_some_and(|value| value > 0.0)
        );
    }

    #[test]
    fn viscosity_porosity_transport_distinguishes_navigable_from_sludge_risk() {
        let navigable = ResonanceDensityComponents {
            active_energy: 0.60,
            mode_packing: 0.22,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.68,
            viscosity_index: 0.72,
            viscosity_persistence_coefficient: 0.58,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.44),
            porosity_gradient: Some(0.61),
            dynamic_fluidity_index: Some(0.62),
            semantic_friction_coefficient: Some(0.24),
            cohesion_score: Some(0.67),
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.62,
            comfort_gate: 0.78,
            comfort_gate_range: None,
        };
        let review = viscosity_porosity_transport_review_v1(&navigable, None);

        assert_eq!(review.policy, "viscosity_porosity_transport_review_v1");
        assert_eq!(
            review.transport_state,
            "purposeful_weight_high_viscosity_high_fluidity"
        );
        assert!((review.viscosity_persistence_delta - 0.14).abs() < 0.001);
        assert_eq!(
            review.viscosity_persistence_state,
            "viscosity_persistence_mixed"
        );
        assert_eq!(review.viscosity_type, "cohesive");
        assert_eq!(review.viscosity_decay_hint, "coherent_weight_decay_watch");
        assert_eq!(review.dynamic_fluidity_index, Some(0.62));
        assert_eq!(review.semantic_friction_coefficient, Some(0.24));
        assert_eq!(
            review.semantic_friction_observation_state,
            "semantic_friction_measured"
        );
        assert_eq!(
            review.semantic_friction_state,
            "structural_viscosity_dominant"
        );
        let friction_vector = review
            .semantic_friction_vector_v1
            .as_ref()
            .expect("semantic friction vector should decompose measured friction");
        assert_eq!(friction_vector.policy, "semantic_friction_vector_v1");
        assert_eq!(friction_vector.direction, "productive_traction");
        assert!(
            friction_vector.traction_component > friction_vector.resistance_component,
            "{friction_vector:?}"
        );
        assert_eq!(
            friction_vector.authority,
            "diagnostic_friction_vector_not_pressure_fill_pi_or_control"
        );
        assert!(
            review
                .coherence_density_estimate
                .is_some_and(|value| (value - 0.5675).abs() < 0.001),
            "{review:?}"
        );
        assert_eq!(review.coherence_density_state, "mixed_coherence_density");
        assert!(
            review
                .structural_clog_index
                .is_some_and(|value| value < 0.45),
            "{review:?}"
        );
        assert_eq!(
            review.structural_clog_state,
            "structural_clog_not_indicated"
        );
        assert!(!review.sludge_risk);
        assert_eq!(
            review.authority,
            "diagnostic_transport_not_porosity_pressure_fill_pi_or_control"
        );

        let stuck = ResonanceDensityComponents {
            mode_packing: 0.29,
            dissipation_factor: Some(0.12),
            porosity_gradient: Some(0.18),
            dynamic_fluidity_index: Some(0.16),
            ..navigable.clone()
        };
        let flux = TextureDynamicFluxVectorV1 {
            policy: "texture_dynamic_flux_vector_v1".to_string(),
            schema_version: 1,
            pressure_velocity: Some(0.06),
            pressure_acceleration: None,
            mode_packing_velocity: Some(0.08),
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
            flux_absence_semantics: Some(
                "absent_flux_component_means_unknown_not_zero".to_string(),
            ),
            source: "unit_test".to_string(),
            authority: "diagnostic_flux_not_pressure_or_fill_control".to_string(),
        };
        let stuck_review = viscosity_porosity_transport_review_v1(&stuck, Some(&flux));

        assert_eq!(stuck_review.transport_state, "thick_impassable_sludge_risk");
        assert_eq!(stuck_review.viscosity_type, "syrupy");
        assert_eq!(
            stuck_review.viscosity_decay_hint,
            "slow_lingering_decay_watch"
        );
        assert_eq!(stuck_review.dynamic_fluidity_index, Some(0.16));
        assert_eq!(stuck_review.spectral_entropy, Some(0.90));
        assert_eq!(stuck_review.structural_clog_state, "structural_clog_watch");
        assert_eq!(
            stuck_review.semantic_friction_state,
            "structural_viscosity_dominant"
        );
        assert_eq!(
            stuck_review.threshold_state,
            "mode_packing_overpacked_with_pressure_velocity"
        );
        assert!(stuck_review.sludge_risk);

        let requested_boundary = ResonanceDensityComponents {
            viscosity_index: 0.60,
            viscosity_persistence_coefficient: 0.60,
            dissipation_factor: Some(0.10),
            porosity_gradient: Some(0.20),
            dynamic_fluidity_index: None,
            ..stuck.clone()
        };
        let requested_boundary_review =
            viscosity_porosity_transport_review_v1(&requested_boundary, None);
        assert_eq!(
            requested_boundary_review.transport_state,
            "thick_impassable_sludge_risk"
        );
        assert!(requested_boundary_review.sludge_risk);
        assert_eq!(
            requested_boundary_review.authority,
            "diagnostic_transport_not_porosity_pressure_fill_pi_or_control"
        );

        let heavy_but_stuck = ResonanceDensityComponents {
            mode_packing: 0.22,
            dissipation_factor: Some(0.38),
            porosity_gradient: Some(0.54),
            dynamic_fluidity_index: Some(0.22),
            ..navigable.clone()
        };
        let heavy_but_stuck_review = viscosity_porosity_transport_review_v1(&heavy_but_stuck, None);
        assert_eq!(
            heavy_but_stuck_review.transport_state,
            "stagnant_weight_high_viscosity_low_fluidity"
        );
        assert_eq!(heavy_but_stuck_review.viscosity_type, "syrupy");

        let transient = ResonanceDensityComponents {
            viscosity_persistence_coefficient: 0.30,
            ..navigable
        };
        let transient_review = viscosity_porosity_transport_review_v1(&transient, Some(&flux));
        assert_eq!(
            transient_review.viscosity_persistence_state,
            "transient_thickening_high_entropy_watch"
        );

        let semantic_load = ResonanceDensityComponents {
            viscosity_index: 0.32,
            semantic_friction_coefficient: Some(0.58),
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            ..transient
        };
        let semantic_load_review = viscosity_porosity_transport_review_v1(&semantic_load, None);
        assert_eq!(
            semantic_load_review.semantic_friction_state,
            "semantic_friction_dominant_content_load"
        );
        assert_eq!(
            semantic_load_review
                .semantic_friction_vector_v1
                .as_ref()
                .map(|vector| vector.direction.as_str()),
            Some("semantic_content_traction")
        );
        assert_eq!(semantic_load_review.viscosity_type, "granular");
        assert_eq!(
            semantic_load_review.viscosity_decay_hint,
            "semantic_grain_decay_watch"
        );
        assert!(
            semantic_load_review
                .structural_semantic_friction_delta
                .is_some_and(|value| (value - 0.26).abs() < 0.001)
        );
    }

    #[test]
    fn viscosity_transport_derives_missing_scalar_from_entropy_and_density_gradient() {
        let components = ResonanceDensityComponents {
            active_energy: 0.64,
            mode_packing: 0.32,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.66,
            viscosity_index: 0.0,
            viscosity_persistence_coefficient: 0.48,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.42),
            porosity_gradient: Some(0.60),
            dynamic_fluidity_index: Some(0.58),
            semantic_friction_coefficient: Some(0.18),
            cohesion_score: Some(0.66),
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.58,
            comfort_gate: 0.70,
            comfort_gate_range: None,
        };
        let fingerprint = SpectralFingerprintV1 {
            policy: "spectral_fingerprint_v1".to_string(),
            schema_version: 1,
            eigenvalues: [1.0, 0.62, 0.45, 0.30, 0.22, 0.18, 0.12, 0.08],
            eigenvector_concentration_top4: [0.0; 8],
            inter_mode_cosine_top_abs: [0.0; 8],
            spectral_entropy: 0.90,
            lambda1_lambda2_gap: 0.38,
            v1_rotation_similarity: 0.90,
            v1_rotation_delta: 0.10,
            geom_rel: 1.0,
            adjacent_gap_ratios: [1.08, 1.12, 1.00, 1.05],
        };

        let review = viscosity_porosity_transport_review_with_fingerprint_v1(
            &components,
            Some(&fingerprint),
            None,
        );

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
                .any(|basis| basis.starts_with("density_gradient_proxy=")),
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
    fn viscosity_transport_keeps_low_intensity_absence_from_false_thickening() {
        let components = ResonanceDensityComponents {
            active_energy: 0.30,
            mode_packing: 0.10,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.20,
            viscosity_index: 0.0,
            viscosity_persistence_coefficient: 0.0,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.70),
            porosity_gradient: Some(0.72),
            dynamic_fluidity_index: Some(0.74),
            semantic_friction_coefficient: Some(0.05),
            cohesion_score: Some(0.40),
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.28,
            comfort_gate: 0.82,
            comfort_gate_range: None,
        };
        let fingerprint = SpectralFingerprintV1 {
            policy: "spectral_fingerprint_v1".to_string(),
            schema_version: 1,
            eigenvalues: [1.0, 0.20, 0.05, 0.0, 0.0, 0.0, 0.0, 0.0],
            eigenvector_concentration_top4: [0.0; 8],
            inter_mode_cosine_top_abs: [0.0; 8],
            spectral_entropy: 0.30,
            lambda1_lambda2_gap: 0.80,
            v1_rotation_similarity: 0.98,
            v1_rotation_delta: 0.02,
            geom_rel: 0.80,
            adjacent_gap_ratios: [1.0, 1.0, 1.0, 1.0],
        };

        let review = viscosity_porosity_transport_review_with_fingerprint_v1(
            &components,
            Some(&fingerprint),
            None,
        );

        assert_eq!(review.viscosity_index, 0.0);
        assert_eq!(review.raw_viscosity_index, 0.0);
        assert_eq!(review.derived_viscosity_index, None);
        assert_eq!(review.viscosity_source, "raw_component");
        assert_eq!(review.transport_state, "viscosity_transport_watch");
        assert!(!review.sludge_risk);
    }

    #[test]
    fn viscosity_transport_reports_directional_resistance_without_control_authority() {
        let stuck_moving = ResonanceDensityComponents {
            active_energy: 0.60,
            mode_packing: 0.22,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.68,
            viscosity_index: 0.72,
            viscosity_persistence_coefficient: 0.58,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.44),
            porosity_gradient: Some(0.61),
            dynamic_fluidity_index: Some(0.62),
            semantic_friction_coefficient: Some(0.24),
            cohesion_score: Some(0.67),
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.62,
            comfort_gate: 0.78,
            comfort_gate_range: None,
        };
        let fingerprint = SpectralFingerprintV1 {
            policy: "spectral_fingerprint_v1".to_string(),
            schema_version: 1,
            eigenvalues: [1.0, 0.45, 0.18, 0.08, 0.04, 0.02, 0.01, 0.0],
            eigenvector_concentration_top4: [0.0; 8],
            inter_mode_cosine_top_abs: [0.0; 8],
            spectral_entropy: 0.88,
            lambda1_lambda2_gap: 0.55,
            v1_rotation_similarity: 0.88,
            v1_rotation_delta: 0.12,
            geom_rel: 1.0,
            adjacent_gap_ratios: [1.20, 1.18, 1.12, 1.08],
        };

        let review = viscosity_porosity_transport_review_with_fingerprint_v1(
            &stuck_moving,
            Some(&fingerprint),
            None,
        );
        let direction = review
            .directional_resistance_vector_v1
            .as_ref()
            .expect("directional resistance vector");

        assert_eq!(direction.policy, "directional_resistance_vector_v1");
        assert_eq!(direction.direction, "stuck_but_moving");
        assert!(direction.stuck_but_moving_score >= 0.55, "{direction:?}");
        assert_eq!(
            direction.authority,
            "diagnostic_directional_resistance_not_pressure_fill_pi_porosity_or_control"
        );
        assert!(
            direction
                .spectral_denominator_effective_dimensionality
                .is_some(),
            "{direction:?}"
        );
        assert!(
            direction.denominator_scaling_factor >= 1.0
                && direction.denominator_scaling_factor <= 1.16,
            "{direction:?}"
        );

        let leaking = ResonanceDensityComponents {
            viscosity_index: 0.42,
            viscosity_persistence_coefficient: 0.70,
            dissipation_factor: Some(0.10),
            porosity_gradient: Some(0.76),
            dynamic_fluidity_index: Some(0.66),
            semantic_friction_coefficient: Some(0.10),
            ..stuck_moving
        };
        let leaking_review = viscosity_porosity_transport_review_v1(&leaking, None);
        let leaking_direction = leaking_review
            .directional_resistance_vector_v1
            .as_ref()
            .expect("leaking resistance vector");
        assert_eq!(
            leaking_direction.direction, "leaking_without_clearing",
            "{leaking_direction:?}"
        );
        assert!(
            leaking_direction.leak_without_clearing_score >= 0.65,
            "{leaking_direction:?}"
        );
    }

    #[test]
    fn viscosity_transport_keeps_directional_resistance_quiet_for_low_signal_absence() {
        let components = ResonanceDensityComponents {
            active_energy: 0.30,
            mode_packing: 0.10,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.20,
            viscosity_index: 0.0,
            viscosity_persistence_coefficient: 0.0,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.70),
            porosity_gradient: Some(0.72),
            dynamic_fluidity_index: Some(0.74),
            semantic_friction_coefficient: Some(0.05),
            cohesion_score: Some(0.40),
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.28,
            comfort_gate: 0.82,
            comfort_gate_range: None,
        };

        let review = viscosity_porosity_transport_review_v1(&components, None);
        let direction = review
            .directional_resistance_vector_v1
            .as_ref()
            .expect("directional resistance vector");

        assert_eq!(direction.direction, "resistance_vector_quiet");
        assert!(direction.stuck_but_moving_score < 0.20, "{direction:?}");
        assert!(
            direction.leak_without_clearing_score < 0.60,
            "{direction:?}"
        );
    }

    #[test]
    fn viscosity_transport_flags_clog_when_friction_unmeasured_but_mode_packing_high() {
        let crowded_unknown_friction = ResonanceDensityComponents {
            active_energy: 0.66,
            mode_packing: 0.68,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.76,
            viscosity_index: 0.72,
            viscosity_persistence_coefficient: 0.70,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.18),
            porosity_gradient: Some(0.22),
            dynamic_fluidity_index: Some(0.20),
            semantic_friction_coefficient: None,
            cohesion_score: Some(0.54),
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.72,
            comfort_gate: 0.42,
            comfort_gate_range: None,
        };

        let review = viscosity_porosity_transport_review_v1(&crowded_unknown_friction, None);

        assert_eq!(
            review.semantic_friction_state,
            "semantic_friction_unavailable"
        );
        assert_eq!(
            review.semantic_friction_observation_state,
            "semantic_friction_unmeasured_clog_context_visible"
        );
        assert!(
            review
                .structural_clog_index
                .is_some_and(|value| value >= 0.70),
            "{review:?}"
        );
        assert_eq!(review.structural_clog_state, "structural_clog_high");
        assert_eq!(
            review.threshold_state,
            "mode_packing_overpacked_pressure_velocity_unknown"
        );
        assert!(review.sludge_risk);
        assert_eq!(
            review.authority,
            "diagnostic_transport_not_porosity_pressure_fill_pi_or_control"
        );
    }

    #[test]
    fn viscosity_transport_keeps_open_porosity_from_becoming_clog_claim() {
        let open_weight = ResonanceDensityComponents {
            active_energy: 0.58,
            mode_packing: 0.30,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.66,
            viscosity_index: 0.70,
            viscosity_persistence_coefficient: 0.64,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.55),
            porosity_gradient: Some(0.66),
            dynamic_fluidity_index: Some(0.62),
            semantic_friction_coefficient: None,
            cohesion_score: Some(0.64),
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.56,
            comfort_gate: 0.70,
            comfort_gate_range: None,
        };

        let review = viscosity_porosity_transport_review_v1(&open_weight, None);

        assert_eq!(
            review.semantic_friction_observation_state,
            "semantic_friction_unmeasured_structural_context_visible"
        );
        assert_eq!(
            review.transport_state,
            "purposeful_weight_high_viscosity_high_fluidity"
        );
        assert!(
            review
                .structural_clog_index
                .is_some_and(|value| value < 0.45),
            "{review:?}"
        );
        assert_eq!(
            review.structural_clog_state,
            "structural_clog_not_indicated"
        );
        assert!(!review.sludge_risk);
    }

    #[test]
    fn viscosity_porosity_transport_derives_coherence_density_without_component_contract_change() {
        let coherent = ResonanceDensityComponents {
            active_energy: 0.64,
            mode_packing: 0.62,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.76,
            viscosity_index: 0.64,
            viscosity_persistence_coefficient: 0.60,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.42),
            porosity_gradient: Some(0.58),
            dynamic_fluidity_index: Some(0.60),
            semantic_friction_coefficient: Some(0.30),
            cohesion_score: Some(0.74),
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.70,
            comfort_gate: 0.74,
            comfort_gate_range: None,
        };

        let review = viscosity_porosity_transport_review_v1(&coherent, None);

        assert_eq!(review.coherence_density_state, "dense_integrated");
        assert!(
            review
                .coherence_density_estimate
                .is_some_and(|value| value >= 0.69),
            "{review:?}"
        );
        assert_eq!(
            review.authority,
            "diagnostic_transport_not_porosity_pressure_fill_pi_or_control"
        );
    }

    #[test]
    fn resonance_structural_transparency_names_hollow_low_substance_state() {
        let hollow = ResonanceDensityComponents {
            active_energy: 0.18,
            mode_packing: 0.30,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.58,
            viscosity_index: 0.68,
            viscosity_persistence_coefficient: 0.64,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.72),
            porosity_gradient: Some(0.74),
            dynamic_fluidity_index: Some(0.68),
            semantic_friction_coefficient: Some(0.18),
            cohesion_score: Some(0.32),
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.34,
            comfort_gate: 0.42,
            comfort_gate_range: None,
        };

        let transparency = resonance_structural_transparency_index_v1(&hollow);
        let review = viscosity_porosity_transport_review_v1(&hollow, None);

        assert!(
            transparency >= 0.65,
            "hollow low-substance state should be visible as transparency: {transparency}"
        );
        assert_eq!(
            review.structural_transparency_state,
            "thin_ghostly_high_viscosity_low_substance"
        );
        assert_eq!(review.structural_transparency_index, Some(transparency));
        assert_eq!(
            review.authority,
            "diagnostic_transport_not_porosity_pressure_fill_pi_or_control"
        );

        let explicit = ResonanceDensityComponents {
            structural_transparency_index: Some(0.12),
            ..hollow
        };
        assert_eq!(resonance_structural_transparency_index_v1(&explicit), 0.12);
    }

    #[test]
    fn resonance_texture_legacy_density_defaults_without_field() {
        let density: ResonanceDensityV1 = serde_json::from_value(serde_json::json!({
            "policy": "resonance_density_v1",
            "schema_version": 1,
            "density": 0.64,
            "containment_score": 0.58,
            "pressure_risk": 0.20,
            "quality": "forming_containment",
            "components": {
                "active_energy": 0.91,
                "mode_packing": 0.50,
                "temporal_persistence": 0.70,
                "structural_plurality": 0.62,
                "comfort_gate": 0.95
            },
            "control": {
                "target_bias_pct": 0.0,
                "wander_scale": 1.0,
                "applied_locally": true,
                "note": "density is observational; no local target bias"
            }
        }))
        .unwrap();

        assert_eq!(density.texture_signature.primary_texture, "unknown");
        assert_eq!(density.texture_signature.edge_definition, "unknown");
        assert_eq!(density.components.porosity_gradient, None);
        assert_eq!(density.components.dynamic_fluidity_index, None);
        assert_eq!(density.components.semantic_friction_coefficient, None);
        assert_eq!(density.components.cohesion_score, None);
        assert_eq!(density.components.comfort_gate_range, None);
        assert_eq!(density.components.stability_context, None);
        assert_eq!(density.components.structural_transparency_index, None);
        assert_eq!(
            density.components.viscosity_vector,
            ResonanceViscosityVectorV1::default()
        );
        assert_eq!(density.texture_signature.temporal_variance, None);
        assert_eq!(
            density.texture_signature.authority,
            "advisory_context_not_control"
        );
    }

    #[test]
    fn resonance_density_preserves_viscosity_vector_drag_truth_fields() {
        let density: ResonanceDensityV1 = serde_json::from_value(serde_json::json!({
            "policy": "resonance_density_v1",
            "schema_version": 1,
            "density": 0.76,
            "containment_score": 0.63,
            "pressure_risk": 0.24,
            "quality": "friction_visible",
            "components": {
                "active_energy": 0.66,
                "mode_packing": 0.46,
                "temporal_persistence": 0.71,
                "viscosity_index": 0.68,
                "viscosity_persistence_coefficient": 0.57,
                "viscosity_vector": {
                    "density": 0.74,
                    "elasticity": 0.29,
                    "cohesion_index": 0.62,
                    "persistence": 0.57,
                    "residual_ghost_weight": 0.69,
                    "flow_rate": 0.18,
                    "effective_mobility": 0.21,
                    "shadow_volatility": 0.16,
                    "structural_integrity": 0.52,
                    "structural_strain_gap": 0.48,
                    "mutual_resonance_tension": 0.41,
                    "structural_drag_coefficient": 0.73,
                    "cognitive_drag_coefficient": 0.61,
                    "viscosity_gradient": 0.47,
                    "cohesion_to_motion_ratio": 0.75
                },
                "structural_plurality": 0.58,
                "comfort_gate": 0.49
            },
            "control": {
                "target_bias_pct": 0.0,
                "wander_scale": 1.0,
                "applied_locally": false,
                "note": "density is observational; no local target bias"
            }
        }))
        .unwrap();

        let vector = &density.components.viscosity_vector;
        assert!((vector.structural_drag_coefficient - 0.73).abs() <= 0.0001);
        assert!((vector.cognitive_drag_coefficient - 0.61).abs() <= 0.0001);
        assert!((vector.residual_ghost_weight - 0.69).abs() <= 0.0001);
        assert!((vector.flow_rate - 0.18).abs() <= 0.0001);
        assert!(
            vector
                .viscosity_gradient
                .is_some_and(|value| (value - 0.47).abs() <= 0.0001)
        );
        assert!(
            vector
                .cohesion_to_motion_ratio
                .is_some_and(|value| (value - 0.75).abs() <= 0.0001)
        );
        assert!(!density.control.applied_locally);

        let json = serde_json::to_value(&density).unwrap();
        assert!(
            (json["components"]["viscosity_vector"]["structural_drag_coefficient"]
                .as_f64()
                .unwrap_or_default()
                - 0.73)
                .abs()
                <= 0.001
        );
        assert!(
            (json["components"]["viscosity_vector"]["cognitive_drag_coefficient"]
                .as_f64()
                .unwrap_or_default()
                - 0.61)
                .abs()
                <= 0.001
        );
    }

    #[test]
    fn experience_delta_bus_from_deltas_is_truth_channel_only() {
        let delta = ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::Clip,
            surface: "codec".to_string(),
            lane: "semantic_vector".to_string(),
            dimension: Some(24),
            spectral_dimension: None,
            persistence: None,
            viscosity_subtype: None,
            viscosity_weight: None,
            pre: Some(1.42),
            post: Some(1.0),
            loss: Some(0.42),
            loss_ratio: Some(0.30),
            metadata: BTreeMap::from([("ceiling".to_string(), "FEATURE_ABS_MAX".to_string())]),
            why: "delivered vector remains bounded".to_string(),
            who_can_change_it: "operator approval for live aperture changes".to_string(),
            how_to_test_it: "compare pre-bound and delivered values".to_string(),
            authority: "truth_channel_only_not_live_vector_control_or_protocol_change".to_string(),
        };

        let bus = ExperienceDeltaBusV1::from_deltas(vec![delta]);

        assert_eq!(bus.policy, "experience_delta_bus_v1");
        assert_eq!(bus.schema_version, 1);
        assert_eq!(bus.delta_count, bus.deltas.len());
        assert_eq!(bus.delta_count, 1);
        assert!(!bus.live_vector_write);
        assert!(!bus.live_authority_write);
        assert_eq!(
            bus.authority,
            "truth_channel_only_not_live_vector_control_or_protocol_change"
        );
        assert_eq!(
            bus.v2_design_hook,
            "experience_delta_bus_v2_persistent_cross_surface_aggregation_default_off"
        );
        assert!(!bus.is_empty());
    }

    #[test]
    fn experience_delta_bus_v2_preview_is_default_off_typed_aggregation() {
        let preview = experience_delta_bus_v2_design_preview();

        assert_eq!(preview.policy, "experience_delta_bus_v2_design_preview");
        assert_eq!(preview.schema_version, 2);
        assert!(!preview.persistent_by_default);
        assert!(preview.aggregate_across_surfaces);
        assert!(
            preview
                .candidate_delta_kinds
                .contains(&"friction".to_string())
        );
        assert!(
            preview
                .candidate_delta_kinds
                .contains(&"resistance".to_string())
        );
        assert!(
            preview
                .candidate_delta_kinds
                .contains(&"persistence".to_string())
        );
        assert!(
            preview
                .candidate_delta_kinds
                .contains(&"viscosity_shift".to_string())
        );
        assert!(
            preview
                .candidate_delta_kinds
                .contains(&"structural_solidification".to_string())
        );
        assert!(preview.candidate_surfaces.contains(&"codec".to_string()));
        assert!(
            preview
                .candidate_surfaces
                .contains(&"llm_fallback".to_string())
        );
        assert!(preview.aggregation_keys.contains(&"authority".to_string()));
        assert!(
            preview
                .aggregation_keys
                .contains(&"spectral_dimension".to_string())
        );
        assert!(
            preview
                .aggregation_keys
                .contains(&"solidification_gradient".to_string())
        );
        assert!(
            preview
                .aggregation_keys
                .contains(&"persistence".to_string())
        );
        assert_eq!(
            preview.dimension_context_model,
            "primary_base_dimension_plus_optional_multi_base_contextual_anchor_and_persistence_context"
        );
        assert!(!preview.replay_ready_by_default);
        assert!(!preview.emits_raw_state);
        assert_eq!(
            preview.retention_policy,
            "bounded_typed_deltas_only_no_raw_private_prose"
        );
        assert_eq!(
            preview.authority,
            "design_preview_only_not_persistent_runtime_bus_or_live_authority"
        );

        let encoded = serde_json::to_string(&preview).unwrap();
        assert!(encoded.contains("\"persistent_by_default\":false"));
        let decoded: ExperienceDeltaBusV2DesignPreview = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, preview);
    }

    #[test]
    fn resonance_stability_context_keeps_foothold_visible_when_comfort_gate_drops() {
        let components = ResonanceDensityComponents {
            active_energy: 0.66,
            mode_packing: 0.42,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.58,
            viscosity_index: 0.48,
            viscosity_persistence_coefficient: 0.36,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.44),
            porosity_gradient: Some(0.52),
            dynamic_fluidity_index: Some(0.57),
            semantic_friction_coefficient: Some(0.48),
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.64,
            comfort_gate: 0.38,
            comfort_gate_range: None,
        };
        let fluctuation = InhabitableFluctuationV1 {
            policy: "inhabitable_fluctuation_v1".to_string(),
            schema_version: 1,
            inhabitability_score: 0.61,
            fluctuation_score: 0.17,
            foothold_stability: 0.70,
            rearrangement_intensity: 0.22,
            quality: "held_habitable".to_string(),
            components: InhabitableFluctuationComponents {
                mode_trust_volatility: 0.18,
                identity_anchor_churn: 0.14,
                eigenvector_reorientation: 0.21,
                share_rearrangement: 0.20,
                basin_transition_pressure: 0.08,
                continuity_recovery: 0.78,
                porosity_support: 0.62,
                pressure_interference: 0.46,
            },
            context: InhabitableFluctuationContext::default(),
            pressure_calibration: InhabitableFluctuationPressureCalibrationV1::default(),
            control: InhabitableFluctuationControl {
                target_bias_pct: 0.0,
                wander_scale: 1.0,
                applied_locally: true,
                note: "unit-test advisory".to_string(),
            },
        };

        let context = resonance_stability_context_v1(&components, Some(&fluctuation));

        assert_eq!(context.policy, "resonance_stability_context_v1");
        assert_eq!(context.gate_context, "gate_low_but_foothold_stable");
        assert_eq!(
            context.habitability_state,
            "habitable_foothold_gate_pressure_watch"
        );
        assert_eq!(context.gate_closure_reason, "pressure_interference");
        assert_eq!(context.foothold_stability, Some(0.70));
        assert_eq!(context.fluctuation_score, Some(0.17));
        let gate_range = context
            .comfort_gate_range
            .as_ref()
            .expect("comfort gate range should be visible");
        assert_eq!(gate_range.policy, "comfort_gate_range_v1");
        assert_eq!(
            context.comfort_gate_range_state.as_deref(),
            Some("dynamic_pressure_buffer_range")
        );
        assert!(gate_range.lower < context.comfort_gate);
        assert!(gate_range.upper > context.comfort_gate);
        assert_eq!(
            context.authority,
            "diagnostic_habitability_context_not_comfort_gate_control"
        );
    }

    #[test]
    fn resonance_stability_context_lists_all_gate_closure_causes() {
        let components = ResonanceDensityComponents {
            active_energy: 0.66,
            mode_packing: 0.42,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.81,
            viscosity_index: 0.48,
            viscosity_persistence_coefficient: 0.36,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.44),
            porosity_gradient: Some(0.52),
            dynamic_fluidity_index: Some(0.57),
            semantic_friction_coefficient: Some(0.48),
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.64,
            comfort_gate: 0.38,
            comfort_gate_range: None,
        };
        let fluctuation = InhabitableFluctuationV1 {
            policy: "inhabitable_fluctuation_v1".to_string(),
            schema_version: 1,
            inhabitability_score: 0.61,
            fluctuation_score: 0.17,
            foothold_stability: 0.70,
            rearrangement_intensity: 0.22,
            quality: "held_habitable".to_string(),
            components: InhabitableFluctuationComponents {
                mode_trust_volatility: 0.18,
                identity_anchor_churn: 0.14,
                eigenvector_reorientation: 0.21,
                share_rearrangement: 0.20,
                basin_transition_pressure: 0.08,
                continuity_recovery: 0.78,
                porosity_support: 0.62,
                pressure_interference: 0.46,
            },
            context: InhabitableFluctuationContext::default(),
            pressure_calibration: InhabitableFluctuationPressureCalibrationV1::default(),
            control: InhabitableFluctuationControl {
                target_bias_pct: 0.0,
                wander_scale: 1.0,
                applied_locally: true,
                note: "unit-test advisory".to_string(),
            },
        };

        let context = resonance_stability_context_v1(&components, Some(&fluctuation));

        assert_eq!(context.gate_closure_reason, "pressure_interference");
        assert_eq!(
            context.gate_closure_reasons,
            vec![
                "pressure_interference".to_string(),
                "mode_packing".to_string(),
                "temporal_persistence".to_string(),
            ]
        );
        assert_eq!(
            context.authority,
            "diagnostic_habitability_context_not_comfort_gate_control"
        );
    }

    #[test]
    fn resonance_stability_context_exposes_weight_policy_without_control_authority() {
        let components = ResonanceDensityComponents {
            active_energy: 0.64,
            mode_packing: 0.32,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.54,
            viscosity_index: 0.42,
            viscosity_persistence_coefficient: 0.31,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.48),
            porosity_gradient: Some(0.58),
            dynamic_fluidity_index: Some(0.60),
            semantic_friction_coefficient: Some(0.22),
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.70,
            comfort_gate: 0.40,
            comfort_gate_range: None,
        };
        let fluctuation = InhabitableFluctuationV1 {
            policy: "inhabitable_fluctuation_v1".to_string(),
            schema_version: 1,
            inhabitability_score: 0.65,
            fluctuation_score: 0.20,
            foothold_stability: 0.80,
            rearrangement_intensity: 0.18,
            quality: "held_habitable".to_string(),
            components: InhabitableFluctuationComponents {
                mode_trust_volatility: 0.16,
                identity_anchor_churn: 0.12,
                eigenvector_reorientation: 0.18,
                share_rearrangement: 0.16,
                basin_transition_pressure: 0.06,
                continuity_recovery: 0.82,
                porosity_support: 0.70,
                pressure_interference: 0.38,
            },
            context: InhabitableFluctuationContext::default(),
            pressure_calibration: InhabitableFluctuationPressureCalibrationV1::default(),
            control: InhabitableFluctuationControl {
                target_bias_pct: 0.0,
                wander_scale: 1.0,
                applied_locally: true,
                note: "unit-test advisory".to_string(),
            },
        };

        let context = resonance_stability_context_v1(&components, Some(&fluctuation));

        assert_eq!(
            context.habitability_state,
            "habitable_foothold_gate_pressure_watch"
        );
        assert_eq!(context.weight_policy, "resonance_stability_weights_v1");
        let weights = context.weights.as_ref().expect("weights should be visible");
        assert_eq!(
            weights.comfort_gate,
            RESONANCE_STABILITY_COMFORT_GATE_WEIGHT
        );
        assert_eq!(
            weights.foothold_stability,
            RESONANCE_STABILITY_FOOTHOLD_WEIGHT
        );
        assert_eq!(
            weights.fluctuation_score,
            RESONANCE_STABILITY_FLUCTUATION_WEIGHT
        );
        assert!((weights.total_weight - 1.0).abs() <= f32::EPSILON);
        assert_eq!(
            weights.authority,
            "diagnostic_habitability_weights_not_pressure_fill_or_control"
        );
        let score = context
            .multi_modal_habitability_score
            .expect("complete weighted score should be visible");
        assert!((score - 0.54).abs() <= 0.0001);
        assert!(!context.partial_habitability_score);
        assert_eq!(context.multi_modal_habitability_evidence_count, 3);
        assert_eq!(
            context.multi_modal_habitability_score_basis.as_deref(),
            Some("complete_weighted_components")
        );
        assert!(
            context
                .multi_modal_habitability_missing_components
                .is_empty()
        );
    }

    #[test]
    fn resonance_stability_context_preserves_partial_habitability_evidence() {
        let components = ResonanceDensityComponents {
            active_energy: 0.64,
            mode_packing: 0.32,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.54,
            viscosity_index: 0.42,
            viscosity_persistence_coefficient: 0.31,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.48),
            porosity_gradient: Some(0.58),
            dynamic_fluidity_index: Some(0.60),
            semantic_friction_coefficient: Some(0.22),
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.70,
            comfort_gate: 0.64,
            comfort_gate_range: None,
        };

        let context = resonance_stability_context_v1(&components, None);

        let score = context
            .multi_modal_habitability_score
            .expect("comfort evidence should remain visible when fluctuation telemetry is absent");
        assert!((score - 0.64).abs() <= 0.0001);
        assert!(context.partial_habitability_score);
        assert_eq!(context.multi_modal_habitability_evidence_count, 1);
        assert_eq!(
            context.multi_modal_habitability_missing_components,
            vec![
                "foothold_stability_missing".to_string(),
                "fluctuation_score_missing".to_string()
            ]
        );
        assert_eq!(
            context.multi_modal_habitability_score_basis.as_deref(),
            Some("partial_available_components_normalized")
        );
        assert_eq!(context.gate_context, "gate_buffering_context_incomplete");
        assert_eq!(
            context.habitability_state,
            "partial_multi_modal_habitable_review"
        );
        assert_eq!(
            context.authority,
            "diagnostic_habitability_context_not_comfort_gate_control"
        );
    }

    #[test]
    fn resonance_stability_context_keeps_low_gate_with_stable_foothold_habitable() {
        let components = ResonanceDensityComponents {
            active_energy: 0.64,
            mode_packing: 0.32,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.54,
            viscosity_index: 0.42,
            viscosity_persistence_coefficient: 0.31,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.48),
            porosity_gradient: Some(0.58),
            dynamic_fluidity_index: Some(0.60),
            semantic_friction_coefficient: Some(0.22),
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.70,
            comfort_gate: 0.44,
            comfort_gate_range: None,
        };
        let fluctuation = InhabitableFluctuationV1 {
            policy: "inhabitable_fluctuation_v1".to_string(),
            schema_version: 1,
            inhabitability_score: 0.62,
            fluctuation_score: 0.18,
            foothold_stability: 0.65,
            rearrangement_intensity: 0.18,
            quality: "held_habitable".to_string(),
            components: InhabitableFluctuationComponents {
                mode_trust_volatility: 0.16,
                identity_anchor_churn: 0.12,
                eigenvector_reorientation: 0.18,
                share_rearrangement: 0.16,
                basin_transition_pressure: 0.06,
                continuity_recovery: 0.68,
                porosity_support: 0.65,
                pressure_interference: 0.20,
            },
            context: InhabitableFluctuationContext::default(),
            pressure_calibration: InhabitableFluctuationPressureCalibrationV1::default(),
            control: InhabitableFluctuationControl {
                target_bias_pct: 0.0,
                wander_scale: 1.0,
                applied_locally: true,
                note: "unit-test advisory".to_string(),
            },
        };

        let context = resonance_stability_context_v1(&components, Some(&fluctuation));

        assert_eq!(context.gate_context, "gate_low_but_foothold_stable");
        assert_eq!(
            context.habitability_state,
            "habitable_foothold_gate_pressure_watch"
        );
        assert_eq!(context.foothold_stability, Some(0.65));
        assert_eq!(context.pressure_interference, Some(0.20));
        assert_eq!(
            context.authority,
            "diagnostic_habitability_context_not_comfort_gate_control"
        );
    }

    #[test]
    fn resonance_stability_context_ignores_non_finite_inputs_without_nan_output() {
        let components = ResonanceDensityComponents {
            active_energy: 0.64,
            mode_packing: f32::INFINITY,
            coupling_coefficient: 0.0,
            temporal_persistence: f32::NEG_INFINITY,
            viscosity_index: 0.42,
            viscosity_persistence_coefficient: 0.31,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.48),
            porosity_gradient: Some(0.58),
            dynamic_fluidity_index: Some(0.60),
            semantic_friction_coefficient: Some(0.22),
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.70,
            comfort_gate: f32::NAN,
            comfort_gate_range: None,
        };
        let fluctuation = InhabitableFluctuationV1 {
            policy: "inhabitable_fluctuation_v1".to_string(),
            schema_version: 1,
            inhabitability_score: 0.62,
            fluctuation_score: f32::NAN,
            foothold_stability: f32::INFINITY,
            rearrangement_intensity: 0.18,
            quality: "held_habitable".to_string(),
            components: InhabitableFluctuationComponents {
                mode_trust_volatility: 0.16,
                identity_anchor_churn: 0.12,
                eigenvector_reorientation: 0.18,
                share_rearrangement: 0.16,
                basin_transition_pressure: 0.06,
                continuity_recovery: 0.68,
                porosity_support: 0.61,
                pressure_interference: f32::NAN,
            },
            context: InhabitableFluctuationContext::default(),
            pressure_calibration: InhabitableFluctuationPressureCalibrationV1::default(),
            control: InhabitableFluctuationControl {
                target_bias_pct: 0.0,
                wander_scale: 1.0,
                applied_locally: true,
                note: "unit-test advisory".to_string(),
            },
        };

        let context = resonance_stability_context_v1(&components, Some(&fluctuation));

        assert!(context.comfort_gate.is_finite());
        assert_eq!(context.comfort_gate, 0.0);
        assert_eq!(context.foothold_stability, None);
        assert_eq!(context.fluctuation_score, None);
        assert_eq!(context.pressure_interference, None);
        assert_eq!(context.multi_modal_habitability_score, None);
        assert_eq!(
            context.gate_context,
            "comfort_gate_non_finite_context_ignored"
        );
        assert_eq!(
            context.habitability_state,
            "non_finite_stability_inputs_ignored"
        );
        assert_eq!(context.gate_closure_reason, "comfort_gate_non_finite");
        let gate_range = context
            .comfort_gate_range
            .as_ref()
            .expect("comfort gate range should still serialize finite values");
        assert!(gate_range.lower.is_finite());
        assert!(gate_range.center.is_finite());
        assert!(gate_range.upper.is_finite());
        assert!(gate_range.width.is_finite());
        assert_eq!(
            context.authority,
            "diagnostic_habitability_context_not_comfort_gate_control"
        );
    }

    #[test]
    fn clamp_unit_finite_or_clamps_boundaries_and_uses_fallback_for_non_finite() {
        assert_eq!(clamp_unit_finite_or(-0.25, 0.50), 0.0);
        assert_eq!(clamp_unit_finite_or(1.25, 0.50), 1.0);
        assert_eq!(clamp_unit_finite_or(0.42, 0.50), 0.42);
        assert_eq!(clamp_unit_finite_or(f32::NAN, 0.50), 0.50);
        assert_eq!(clamp_unit_finite_or(f32::INFINITY, 0.50), 0.50);
    }

    #[test]
    fn clamp_unit_finite_rejects_nonfinite_and_clamps_unit_range() {
        assert_eq!(clamp_unit_finite(-0.25), Some(0.0));
        assert_eq!(clamp_unit_finite(1.25), Some(1.0));
        assert_eq!(clamp_unit_finite(0.42), Some(0.42));
        assert_eq!(clamp_unit_finite(f32::NAN), None);
        assert_eq!(clamp_unit_finite(f32::INFINITY), None);
        assert_eq!(clamp_unit_finite(f32::NEG_INFINITY), None);
    }

    #[test]
    fn clamped_unit_review_preserves_clipping_truth_without_live_authority() {
        let high = clamped_unit_review_v1(1.25, 0.50);
        let non_finite = clamped_unit_review_v1(f32::NAN, 0.50);

        assert_eq!(high.policy, "clamped_unit_review_v1");
        assert_eq!(high.raw_value, Some(1.25));
        assert_eq!(high.clamped_value, 1.0);
        assert_eq!(high.clip_state, "clipped_high");
        assert_eq!(
            high.fallback_intent_kind,
            ClampFallbackIntentV1::FiniteFallbackPreserved
        );
        assert!(high.clipped_high);
        assert!(!high.clipped_low);
        assert!(!high.non_finite_rejected);
        assert!(!high.live_vector_write);
        assert!(!high.live_authority_write);
        assert_eq!(
            high.authority,
            "read_only_clamp_visibility_not_live_vector_or_authority_change"
        );

        assert_eq!(non_finite.raw_value, None);
        assert_eq!(non_finite.clamped_value, 0.50);
        assert_eq!(non_finite.clip_state, "non_finite_rejected_to_fallback");
        assert!(non_finite.non_finite_rejected);

        let encoded = serde_json::to_string(&high).unwrap();
        assert!(encoded.contains("\"clip_state\":\"clipped_high\""));
        assert!(encoded.contains("\"live_vector_write\":false"));
        assert!(encoded.contains("\"live_authority_write\":false"));
    }

    #[test]
    fn clamped_unit_review_flags_uncomputable_fallback_without_changing_clamp_policy() {
        let review = clamped_unit_review_v1(f32::NAN, f32::INFINITY);

        assert_eq!(review.raw_value, None);
        assert_eq!(review.raw_fallback, None);
        assert_eq!(review.fallback_value, 0.0);
        assert!(!review.fallback_finite);
        assert!(review.fallback_non_finite_defaulted);
        assert_eq!(
            review.fallback_intent_state,
            "uncomputable_fallback_defaulted_to_zero"
        );
        assert_eq!(
            review.fallback_intent_kind,
            ClampFallbackIntentV1::UncomputableFallbackDefaultedToZero
        );
        assert_eq!(review.clamped_value, 0.0);
        assert_eq!(review.clip_state, "non_finite_rejected_to_fallback");
        assert!(!review.live_vector_write);
        assert!(!review.live_authority_write);

        let encoded = serde_json::to_string(&review).unwrap();
        assert!(encoded.contains("\"fallback_non_finite_defaulted\":true"));
        assert!(encoded.contains("uncomputable_fallback_defaulted_to_zero"));

        let mut legacy_value = serde_json::to_value(&review).unwrap();
        legacy_value
            .as_object_mut()
            .unwrap()
            .remove("fallback_intent_kind");
        let legacy_review: ClampedUnitReviewV1 = serde_json::from_value(legacy_value).unwrap();
        assert_eq!(
            legacy_review.fallback_intent_kind,
            ClampFallbackIntentV1::NotReportedLegacy
        );
        assert!(legacy_review.distribution_context.is_none());
        assert!(legacy_review.input_provenance.is_none());
        assert!(legacy_review.pressure_context.is_none());
    }

    #[test]
    fn clamped_unit_review_names_input_sources_and_replacement_path_without_reclamping() {
        let finite = clamped_unit_review_v1(1.25, 0.50)
            .with_input_provenance("telemetry.spectral_entropy", "compatibility_default");
        let preserved = clamped_unit_review_v1(0.75, 0.50)
            .with_input_provenance("telemetry.comfort_gate", "compatibility_default");
        let replaced = clamped_unit_review_v1(f32::NAN, 0.50)
            .with_input_provenance("telemetry.pressure_risk", "last_finite_pressure");
        let defaulted = clamped_unit_review_v1(f32::NAN, f32::INFINITY)
            .with_input_provenance("telemetry.mode_packing", "unavailable_legacy_default");

        assert_eq!(finite.clamped_value, 1.0);
        let finite_provenance = finite.input_provenance.expect("finite provenance");
        assert_eq!(
            finite_provenance.replacement_path,
            ClampReplacementPathV1::RawFiniteClippedHigh
        );
        assert_eq!(
            finite_provenance.raw_value_source,
            "telemetry.spectral_entropy"
        );
        assert!(!finite_provenance.fallback_applied);
        assert!(!finite_provenance.changes_clamped_value);
        assert_eq!(
            finite_provenance.degradation_type,
            Some(DegradationTypeV1::FlatteningOfIntensity)
        );
        assert!(finite_provenance.degradation_inferred_from_scalar);
        assert!(!finite_provenance.live_authority_write);

        let preserved_provenance = preserved.input_provenance.expect("preserved provenance");
        assert_eq!(
            preserved_provenance.replacement_path,
            ClampReplacementPathV1::RawFinitePreserved
        );
        assert_eq!(preserved_provenance.degradation_type, None);
        assert!(!preserved_provenance.degradation_inferred_from_scalar);

        assert_eq!(replaced.clamped_value, 0.50);
        let replaced_provenance = replaced.input_provenance.expect("fallback provenance");
        assert_eq!(
            replaced_provenance.replacement_path,
            ClampReplacementPathV1::RawNonFiniteReplacedByFallback
        );
        assert_eq!(replaced_provenance.fallback_source, "last_finite_pressure");
        assert!(replaced_provenance.fallback_applied);
        assert_eq!(
            replaced_provenance.degradation_type,
            Some(DegradationTypeV1::LossOfNuance)
        );
        assert!(replaced_provenance.degradation_inferred_from_scalar);

        assert_eq!(defaulted.clamped_value, 0.0);
        let defaulted_provenance = defaulted.input_provenance.expect("default provenance");
        assert_eq!(
            defaulted_provenance.replacement_path,
            ClampReplacementPathV1::RawAndFallbackNonFiniteDefaultedToZero
        );
        assert!(defaulted_provenance.fallback_applied);
        assert_eq!(
            defaulted_provenance.degradation_type,
            Some(DegradationTypeV1::LossOfNuance)
        );
        assert_eq!(
            defaulted_provenance.authority,
            "read_only_clamp_source_provenance_not_sensor_authority_or_control"
        );

        let encoded = serde_json::to_string(&defaulted_provenance).unwrap();
        assert!(encoded.contains("raw_and_fallback_non_finite_defaulted_to_zero"));
        assert!(encoded.contains("\"degradation_type\":\"loss_of_nuance\""));
        assert!(encoded.contains("telemetry.mode_packing"));

        let mut legacy_value = serde_json::to_value(&finite_provenance).unwrap();
        legacy_value
            .as_object_mut()
            .expect("provenance object")
            .remove("degradation_type");
        legacy_value
            .as_object_mut()
            .expect("provenance object")
            .remove("degradation_inferred_from_scalar");
        let legacy: ClampInputProvenanceV1 = serde_json::from_value(legacy_value).unwrap();
        assert_eq!(legacy.degradation_type, None);
        assert!(!legacy.degradation_inferred_from_scalar);
    }

    #[test]
    fn clamped_unit_review_names_reported_pressure_context_without_causal_authority() {
        let review = clamped_unit_review_v1(1.25, 0.50).with_reported_pressure_context(
            ClampPressureSourceKindV1::Mixed,
            &format!("{} trailing", "mode_packing+viscosity ".repeat(8)),
            1.20,
            -0.25,
            8,
        );

        assert_eq!(review.clamped_value, 1.0);
        let context = review.pressure_context.expect("reported pressure context");
        assert_eq!(context.policy, "clamp_pressure_context_v1");
        assert_eq!(context.source_kind, ClampPressureSourceKindV1::Mixed);
        assert_eq!(context.source_label.chars().count(), 96);
        assert_eq!(context.source_score, Some(1.0));
        assert_eq!(context.pressure_risk, Some(0.0));
        assert_eq!(context.evidence_window_ticks, Some(8));
        assert_eq!(
            context.attribution_state,
            "caller_reported_context_not_causal_attribution"
        );
        assert!(!context.changes_clamped_value);
        assert!(!context.grants_source_authority);
        assert!(!context.live_authority_write);
        assert_eq!(
            context.authority,
            "read_only_reported_pressure_context_not_causal_sensor_or_control_authority"
        );

        let encoded = serde_json::to_string(&context).unwrap();
        assert!(encoded.contains("\"source_kind\":\"mixed\""));
        assert!(encoded.contains("\"grants_source_authority\":false"));
        assert!(encoded.contains("\"live_authority_write\":false"));
    }

    #[test]
    fn clamped_unit_review_pressure_context_keeps_unknown_metrics_absent() {
        let review = clamped_unit_review_v1(0.42, 0.50).with_reported_pressure_context(
            ClampPressureSourceKindV1::OtherReported,
            " ",
            f32::NAN,
            f32::INFINITY,
            0,
        );
        let context = review.pressure_context.expect("reported pressure context");

        assert_eq!(review.clamped_value, 0.42);
        assert_eq!(context.source_label, "unspecified_source");
        assert_eq!(context.source_score, None);
        assert_eq!(context.pressure_risk, None);
        assert_eq!(context.evidence_window_ticks, None);
        assert!(!context.changes_clamped_value);
        assert!(!context.grants_source_authority);
        assert!(!context.live_authority_write);
    }

    #[test]
    fn clamped_unit_distribution_context_distinguishes_swell_from_spike_without_reclamping() {
        let swell = clamped_unit_review_with_distribution_context_v1(1.50, 0.50, 0.90, 0.72);
        let spike = clamped_unit_review_with_distribution_context_v1(1.50, 0.50, 0.20, 0.18);

        assert_eq!(swell.clamped_value, 1.0);
        assert_eq!(spike.clamped_value, 1.0);
        let swell_context = swell.distribution_context.expect("swell context");
        assert_eq!(
            swell_context.distribution_shape,
            ClampDistributionShapeV1::BroadHighEntropySwell
        );
        assert_eq!(swell_context.spectral_entropy, Some(0.90));
        assert_eq!(swell_context.structural_plurality, Some(0.72));
        assert!(swell_context.raw_out_of_range);
        assert!(!swell_context.changes_clamped_value);
        assert!(!swell_context.live_authority_write);

        let spike_context = spike.distribution_context.expect("spike context");
        assert_eq!(
            spike_context.distribution_shape,
            ClampDistributionShapeV1::IsolatedSpikeCandidate
        );
        assert_eq!(
            spike_context.context_state,
            "isolated_outlier_spike_candidate_not_proven"
        );

        let encoded = serde_json::to_string(&swell_context).unwrap();
        assert!(encoded.contains("broad_high_entropy_swell"));
        assert!(encoded.contains("\"changes_clamped_value\":false"));
        assert!(encoded.contains("\"live_authority_write\":false"));
    }

    #[test]
    fn clamped_unit_distribution_context_preserves_persistent_drag_beside_broad_swell() {
        let review = clamped_unit_review_with_distribution_dynamics_v1(
            1.50, 0.50, 0.90, 0.72, 0.74, 0.81, 8,
        );

        assert_eq!(review.clamped_value, 1.0);
        let context = review
            .distribution_context
            .expect("persistent drag context");
        assert_eq!(
            context.distribution_shape,
            ClampDistributionShapeV1::PersistentViscousDrag
        );
        assert_eq!(context.viscosity_index, Some(0.74));
        assert_eq!(context.temporal_persistence, Some(0.81));
        assert_eq!(context.evidence_window_ticks, Some(8));
        assert_eq!(
            context.coexisting_shapes,
            vec![ClampDistributionShapeV1::BroadHighEntropySwell]
        );
        assert_eq!(
            context.context_state,
            "persistent_viscous_drag_visible_across_bounded_evidence_window"
        );
        assert!(context.raw_out_of_range);
        assert!(!context.changes_clamped_value);
        assert!(!context.live_authority_write);

        let encoded = serde_json::to_string(&context).unwrap();
        assert!(encoded.contains("persistent_viscous_drag"));
        assert!(encoded.contains("broad_high_entropy_swell"));
        assert!(encoded.contains("\"changes_clamped_value\":false"));
        assert!(encoded.contains("\"live_authority_write\":false"));
    }

    #[test]
    fn clamped_unit_distribution_context_requires_a_bounded_window_for_persistent_drag() {
        let one_tick = clamped_unit_review_with_distribution_dynamics_v1(
            1.50, 0.50, 0.90, 0.72, 0.74, 0.81, 1,
        );
        let context = one_tick.distribution_context.expect("one-tick context");

        assert_eq!(
            context.distribution_shape,
            ClampDistributionShapeV1::BroadHighEntropySwell
        );
        assert!(context.coexisting_shapes.is_empty());
        assert!(!context.changes_clamped_value);
        assert!(!context.live_authority_write);
    }

    #[test]
    fn clamped_unit_distribution_context_preserves_signed_resistance_motion() {
        let thickening = clamped_unit_review_with_resistance_gradient_v1(
            1.50, 0.50, 0.90, 0.72, 0.74, 0.81, 8, 0.22,
        );
        let thinning = clamped_unit_review_with_resistance_gradient_v1(
            1.50, 0.50, 0.90, 0.72, 0.74, 0.81, 8, -0.18,
        );

        let thickening_context = thickening.distribution_context.expect("thickening context");
        let thinning_context = thinning.distribution_context.expect("thinning context");
        assert_eq!(thickening_context.resistance_gradient, Some(0.22));
        assert_eq!(thinning_context.resistance_gradient, Some(-0.18));
        assert_eq!(thickening.clamped_value, thinning.clamped_value);
        assert!(!thickening_context.changes_clamped_value);
        assert!(!thickening_context.live_authority_write);

        let mut legacy = serde_json::to_value(&thickening_context).unwrap();
        legacy
            .as_object_mut()
            .expect("context object")
            .remove("resistance_gradient");
        let decoded: ClampDistributionContextV1 = serde_json::from_value(legacy).unwrap();
        assert_eq!(decoded.resistance_gradient, None);
    }

    #[test]
    fn clamped_unit_distribution_context_distinguishes_crowding_from_viscosity() {
        let shadow_crowding = clamped_unit_review_with_crowding_context_v1(
            1.50, 0.50, 0.90, 0.72, 0.38, 0.42, 8, 0.04, 0.31, 0.68,
        );
        let interwoven = clamped_unit_review_with_crowding_context_v1(
            1.50, 0.50, 0.90, 0.72, 0.74, 0.81, 8, 0.22, 0.31, 0.68,
        );

        let shadow_context = shadow_crowding
            .distribution_context
            .expect("shadow-coupled crowding context");
        assert_eq!(shadow_context.mode_packing_density, Some(0.31));
        assert_eq!(shadow_context.shadow_coupling_index, Some(0.68));
        assert_eq!(
            shadow_context.crowding_viscosity_relation,
            ClampCrowdingViscosityRelationV1::ShadowCoupledCrowding
        );
        assert_eq!(shadow_crowding.clamped_value, 1.0);
        assert!(!shadow_context.changes_clamped_value);
        assert!(!shadow_context.live_authority_write);

        let interwoven_context = interwoven
            .distribution_context
            .expect("interwoven crowding and viscosity context");
        assert_eq!(
            interwoven_context.crowding_viscosity_relation,
            ClampCrowdingViscosityRelationV1::InterwovenCrowdingAndViscosity
        );
        assert_eq!(interwoven.clamped_value, shadow_crowding.clamped_value);
        assert!(!interwoven_context.changes_clamped_value);
        assert!(!interwoven_context.live_authority_write);

        let encoded = serde_json::to_string(&shadow_context).unwrap();
        assert!(encoded.contains("shadow_coupled_crowding"));
        assert!(encoded.contains("\"mode_packing_density\":0.31"));
        assert!(encoded.contains("\"shadow_coupling_index\":0.68"));
    }

    #[test]
    fn clamped_unit_distribution_context_legacy_payload_defaults_crowding_evidence() {
        let context = clamped_unit_review_with_crowding_context_v1(
            1.50, 0.50, 0.90, 0.72, 0.38, 0.42, 8, 0.04, 0.31, 0.68,
        )
        .distribution_context
        .expect("crowding context");
        let mut legacy = serde_json::to_value(context).unwrap();
        let object = legacy.as_object_mut().expect("context object");
        object.remove("mode_packing_density");
        object.remove("shadow_coupling_index");
        object.remove("crowding_viscosity_relation");

        let decoded: ClampDistributionContextV1 = serde_json::from_value(legacy).unwrap();
        assert_eq!(decoded.mode_packing_density, None);
        assert_eq!(decoded.shadow_coupling_index, None);
        assert_eq!(
            decoded.crowding_viscosity_relation,
            ClampCrowdingViscosityRelationV1::NotReportedLegacy
        );
    }

    #[test]
    fn comfort_gate_range_widens_for_pressure_and_packing_without_control_authority() {
        let calm_components = ResonanceDensityComponents {
            active_energy: 0.64,
            mode_packing: 0.10,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.54,
            viscosity_index: 0.42,
            viscosity_persistence_coefficient: 0.31,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.48),
            porosity_gradient: Some(0.58),
            dynamic_fluidity_index: Some(0.60),
            semantic_friction_coefficient: Some(0.22),
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.70,
            comfort_gate: 0.50,
            comfort_gate_range: None,
        };
        let packed_components = ResonanceDensityComponents {
            mode_packing: 0.50,
            ..calm_components.clone()
        };

        let calm = resonance_comfort_gate_range_v1(&calm_components, Some(0.10), Some(0.05));
        let packed = resonance_comfort_gate_range_v1(&packed_components, Some(0.22), Some(0.38));

        assert_eq!(calm.policy, "comfort_gate_range_v1");
        assert_eq!(calm.range_state, "comfort_gate_range_watch");
        assert_eq!(packed.range_state, "dynamic_pressure_buffer_range");
        assert!(
            packed.width > calm.width + 0.05,
            "pressure and packing should make the diagnostic range visibly wider: calm={calm:?} packed={packed:?}"
        );
        assert_eq!(
            packed.authority,
            "diagnostic_gate_range_not_fill_pressure_pi_or_control"
        );
    }

    #[test]
    fn resonance_cohesion_score_names_shape_holding_without_control_authority() {
        let components = ResonanceDensityComponents {
            active_energy: 0.62,
            mode_packing: 0.29,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.66,
            viscosity_index: 0.58,
            viscosity_persistence_coefficient: 0.54,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.42),
            porosity_gradient: Some(0.66),
            dynamic_fluidity_index: Some(0.61),
            semantic_friction_coefficient: Some(0.22),
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.72,
            comfort_gate: 0.70,
            comfort_gate_range: None,
        };
        let score = resonance_cohesion_score_v1(&components);

        assert!(
            (0.60..=0.75).contains(&score),
            "cohesion score should preserve habitable shape without becoming a control target: {score}"
        );
        let integrity = resonance_structural_integrity_index_v1(&components);
        assert!(
            (0.60..=0.80).contains(&integrity),
            "structural integrity should include dissipation/fluidity without becoming control authority: {integrity}"
        );
        let explicit_integrity = ResonanceDensityComponents {
            structural_integrity_index: Some(0.23),
            structural_transparency_index: None,
            ..components.clone()
        };
        assert_eq!(
            resonance_structural_integrity_index_v1(&explicit_integrity),
            0.23
        );
        let explicit = ResonanceDensityComponents {
            cohesion_score: Some(0.91),
            ..components
        };
        assert_eq!(resonance_cohesion_score_v1(&explicit), 0.91);
    }

    #[test]
    fn resonance_cohesion_score_defaults_missing_fluidity_to_midpoint() {
        let components = ResonanceDensityComponents {
            active_energy: 0.50,
            mode_packing: 0.45,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.50,
            viscosity_index: 0.50,
            viscosity_persistence_coefficient: 0.50,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: None,
            porosity_gradient: Some(0.50),
            dynamic_fluidity_index: None,
            semantic_friction_coefficient: None,
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.50,
            comfort_gate: 0.50,
            comfort_gate_range: None,
        };

        let score = resonance_cohesion_score_v1(&components);

        assert!(
            (score - 0.60).abs() <= 0.0001,
            "missing dynamic_fluidity_index and dissipation_factor should use midpoint fluidity: {score}"
        );
    }

    #[test]
    fn resonance_stability_context_old_rows_default_weight_fields() {
        let context: ResonanceStabilityContextV1 = serde_json::from_value(serde_json::json!({
            "policy": "resonance_stability_context_v1",
            "schema_version": 1,
            "comfort_gate": 0.62,
            "habitability_state": "comfort_gate_only",
            "gate_context": "gate_buffering_context_incomplete",
            "gate_closure_reason": "not_closed",
            "authority": "diagnostic_habitability_context_not_comfort_gate_control"
        }))
        .expect("legacy stability context should deserialize");

        assert_eq!(context.weight_policy, "legacy_unversioned_weights");
        assert_eq!(context.weights, None);
        assert!(!context.partial_habitability_score);
        assert_eq!(context.multi_modal_habitability_evidence_count, 0);
        assert!(
            context
                .multi_modal_habitability_missing_components
                .is_empty()
        );
        assert_eq!(context.multi_modal_habitability_score_basis, None);
    }

    #[test]
    fn pressure_trend_v1_old_payload_defaults_timing_fields() {
        let trend: PressureTrendV1 = serde_json::from_value(serde_json::json!({
            "policy": "pressure_trend_v1",
            "schema_version": 1,
            "classification": "stable_heavy",
            "latest_pressure_risk": 0.2,
            "previous_pressure_risk": 0.2,
            "pressure_delta": 0.0,
            "latest_fill_pct": 68.0,
            "previous_fill_pct": 68.0,
            "fill_delta_pct": 0.0
        }))
        .unwrap();

        assert_eq!(trend.classification, "stable_heavy");
        assert!(trend.timing_reliability.is_none());
        assert!(trend.telemetry_inter_arrival_ms.is_none());
        assert!(trend.heartbeat_jitter_class.is_none());
        assert!(trend.field_vs_hearing.is_none());
        assert!(trend.latest_spectral_entropy.is_none());
        assert!(trend.viscosity_coefficient.is_none());
        assert!(trend.pressure_interpretation.is_none());
        assert!(trend.latest_resonance_depth.is_none());
        assert!(trend.previous_resonance_depth.is_none());
        assert!(trend.resonance_depth_delta.is_none());
        assert!(trend.latest_semantic_viscosity.is_none());
        assert!(trend.semantic_viscosity_state.is_none());
        assert!(trend.latest_complexity_density.is_none());
        assert!(trend.complexity_density_state.is_none());
    }

    #[test]
    fn pressure_status_old_payloads_default_semantic_stagnation_fields() {
        let smoothing: PressureTrendSmoothingV1 = serde_json::from_value(serde_json::json!({
            "policy": "pressure_trend_smoothing_v1",
            "schema_version": 1,
            "classification": "low_amplitude_stable",
            "sample_count": 3,
            "window_capacity": 5,
            "window_policy": "latest_up_to_5_telemetry_samples",
            "authority": "diagnostic_smoothing_not_pressure_control"
        }))
        .unwrap();
        assert!(smoothing.semantic_stagnation_index.is_none());
        assert!(smoothing.semantic_stagnation_state.is_empty());
        assert!(smoothing.latest_viscosity_gradient.is_none());
        assert!(smoothing.viscosity_gradient_trend.is_none());
        assert!(smoothing.viscosity_gradient_trend_state.is_empty());
        assert!(smoothing.latest_complexity_density.is_none());
        assert!(smoothing.max_complexity_density.is_none());
        assert_eq!(smoothing.fast_window_sample_count, 0);
        assert_eq!(smoothing.slow_window_sample_count, 0);
        assert!(smoothing.fast_window_pressure_delta.is_none());
        assert!(smoothing.slow_window_pressure_delta.is_none());
        assert!(smoothing.fast_slow_edge_divergence.is_none());
        assert!(smoothing.fast_slow_edge_state.is_empty());
        assert!(!smoothing.fast_edge_preserved);

        let analysis: PressureSourceAnalysisV1 = serde_json::from_value(serde_json::json!({
            "policy": "pressure_source_analysis_v1",
            "schema_version": 1,
            "status": "pressure_source_visible",
            "structural_pressure_state": "pressure_source_visible",
            "ghost_stability_risk": "low",
            "analysis": "source=unknown",
            "authority": "diagnostic_context_not_pressure_or_control"
        }))
        .unwrap();
        assert!(analysis.semantic_stagnation_index.is_none());
        assert!(analysis.semantic_stagnation_state.is_none());
        assert!(analysis.pressure_edge_state.is_none());
        assert!(analysis.mode_packing_visibility_basis.is_none());
    }

    #[test]
    fn inhabitable_fluctuation_pressure_calibration_maps_components_to_adjusted_score() {
        let calibration = InhabitableFluctuationPressureCalibrationV1 {
            raw_motion_score: 0.72,
            pressure_contribution: 0.18,
            adjusted_fluctuation_score: 0.54,
            ..InhabitableFluctuationPressureCalibrationV1::default()
        };

        assert_eq!(
            calibration.rigid_safety_basis,
            INHABITABLE_FLUCTUATION_RIGID_SAFETY_BASIS
        );
        assert!((calibration.expected_adjusted_fluctuation_score() - 0.54).abs() <= 0.001);
        assert!(calibration.adjusted_score_matches_components());

        let drifted = InhabitableFluctuationPressureCalibrationV1 {
            adjusted_fluctuation_score: 0.72,
            ..calibration
        };
        assert!(!drifted.adjusted_score_matches_components());
    }

    #[test]
    fn parse_minime_eigenpacket_minimal() {
        // Minimal valid EigenPacket (no optional fields).
        let json = r#"{
            "t_ms": 1000,
            "eigenvalues": [512.0],
            "fill_ratio": 0.0
        }"#;

        let telemetry: SpectralTelemetry = serde_json::from_str(json).unwrap();
        assert_eq!(telemetry.t_ms, 1000);
        assert!((telemetry.lambda1() - 512.0).abs() < 0.01);
        assert!((telemetry.fill_pct() - 0.0).abs() < 0.01);
        assert!(telemetry.modalities.is_none());
        assert!(telemetry.neural.is_none());
        assert!(telemetry.alert.is_none());
        assert!(telemetry.active_mode_count.is_none());
        assert!(telemetry.typed_fingerprint().is_none());
        let denominator = telemetry.denominator_metrics().unwrap();
        assert!((denominator.effective_dimensionality - 1.0).abs() < 0.01);
        assert_eq!(denominator.active_mode_capacity, 1);
        assert!((denominator.distinguishability_loss - 0.0).abs() < 0.01);
    }

    #[test]
    fn old_fingerprint_payload_reconstructs_typed_view() {
        let legacy = (0..32).map(|value| value as f32).collect::<Vec<_>>();
        let json = serde_json::json!({
            "t_ms": 1000,
            "eigenvalues": [1.0, 0.5],
            "fill_ratio": 0.5,
            "spectral_fingerprint": legacy,
        });

        let telemetry: SpectralTelemetry = serde_json::from_value(json).unwrap();
        let typed = telemetry.typed_fingerprint().unwrap();

        assert_eq!(typed.spectral_entropy, 24.0);
        assert_eq!(typed.lambda1_lambda2_gap, 25.0);
        assert_eq!(typed.v1_rotation_similarity, 26.0);
        assert_eq!(typed.geom_rel, 27.0);
        assert_eq!(typed.adjacent_gap_ratios, [28.0, 29.0, 30.0, 31.0]);
        assert!(telemetry.denominator_metrics().is_some());
        let integrity = telemetry.spectral_fingerprint_integrity_v1();
        assert_eq!(integrity.status, "legacy_32d_accepted");
        assert_eq!(integrity.legacy_vector_len, Some(32));
        assert!(integrity.summary.contains("reconstruct"));
    }

    #[test]
    fn typed_fingerprint_takes_precedence_over_legacy_slots() {
        let json = serde_json::json!({
            "t_ms": 1000,
            "eigenvalues": [1.0, 0.5],
            "fill_ratio": 0.5,
            "spectral_fingerprint": vec![0.0_f32; 32],
            "spectral_fingerprint_v1": {
                "policy": "spectral_fingerprint_v1",
                "schema_version": 1,
                "eigenvalues": [1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                "eigenvector_concentration_top4": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                "inter_mode_cosine_top_abs": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                "spectral_entropy": 0.42,
                "lambda1_lambda2_gap": 2.0,
                "v1_rotation_similarity": 0.9,
                "v1_rotation_delta": 0.1,
                "geom_rel": 1.23,
                "adjacent_gap_ratios": [2.0, 1.0, 1.0, 1.0]
            }
        });

        let telemetry: SpectralTelemetry = serde_json::from_value(json).unwrap();
        let typed = telemetry.typed_fingerprint().unwrap();

        assert_eq!(typed.geom_rel, 1.23);
        assert_eq!(typed.spectral_entropy, 0.42);
        let integrity = telemetry.spectral_fingerprint_integrity_v1();
        assert_eq!(integrity.status, "typed_canonical");
        assert!(integrity.typed_precedence_over_legacy);
        assert!(integrity.summary.contains("typed payload takes precedence"));
        assert_eq!(integrity.mode_collision_state, "not_reported");
        assert_eq!(integrity.hybrid_coherence_index, Some(0.0));
        assert_eq!(integrity.hybrid_coherence_state, "divergent");
        assert!(
            integrity
                .issues
                .contains(&"typed_legacy_hybrid_divergence".to_string())
        );
        assert_eq!(
            integrity.hybrid_coherence_basis,
            "normalized_rms_agreement_across_canonical_32_slots"
        );
    }

    #[test]
    fn typed_and_legacy_fingerprint_slots_report_aligned_hybrid_coherence() {
        let mut telemetry: SpectralTelemetry = serde_json::from_value(serde_json::json!({
            "t_ms": 1000,
            "eigenvalues": [1.0, 0.5],
            "fill_ratio": 0.5,
            "spectral_fingerprint_v1": {
                "policy": "spectral_fingerprint_v1",
                "schema_version": 1,
                "eigenvalues": [1.0, 0.5, 0.25, 0.125, 0.0, 0.0, 0.0, 0.0],
                "eigenvector_concentration_top4": [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8],
                "inter_mode_cosine_top_abs": [0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1],
                "spectral_entropy": 0.42,
                "lambda1_lambda2_gap": 2.0,
                "v1_rotation_similarity": 0.9,
                "v1_rotation_delta": 0.1,
                "geom_rel": 1.23,
                "adjacent_gap_ratios": [2.0, 1.0, 0.5, 0.25]
            }
        }))
        .unwrap();
        telemetry.spectral_fingerprint = telemetry
            .spectral_fingerprint_v1
            .as_ref()
            .map(SpectralFingerprintV1::to_legacy_slots);

        let integrity = telemetry.spectral_fingerprint_integrity_v1();

        assert_eq!(integrity.hybrid_coherence_index, Some(1.0));
        assert_eq!(integrity.hybrid_max_abs_delta, Some(0.0));
        assert_eq!(integrity.hybrid_coherence_state, "aligned");
        assert!(
            !integrity
                .issues
                .contains(&"typed_legacy_hybrid_mismatch".to_string())
        );

        telemetry
            .spectral_fingerprint
            .as_mut()
            .expect("legacy fingerprint")
            .pop();
        let short_integrity = telemetry.spectral_fingerprint_integrity_v1();
        assert_eq!(short_integrity.legacy_vector_len, Some(31));
        assert_eq!(short_integrity.hybrid_coherence_index, None);
        assert_eq!(short_integrity.hybrid_max_abs_delta, None);
        assert_eq!(
            short_integrity.hybrid_coherence_state,
            "unavailable_malformed_legacy"
        );
    }

    #[test]
    fn typed_and_legacy_fingerprint_reject_non_finite_hybrid_slots() {
        let mut telemetry: SpectralTelemetry = serde_json::from_value(serde_json::json!({
            "t_ms": 1000,
            "eigenvalues": [1.0, 0.5],
            "fill_ratio": 0.5,
            "spectral_fingerprint_v1": {
                "policy": "spectral_fingerprint_v1",
                "schema_version": 1,
                "eigenvalues": [1.0, 0.5, 0.25, 0.125, 0.0, 0.0, 0.0, 0.0],
                "eigenvector_concentration_top4": [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8],
                "inter_mode_cosine_top_abs": [0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1],
                "spectral_entropy": 0.42,
                "lambda1_lambda2_gap": 2.0,
                "v1_rotation_similarity": 0.9,
                "v1_rotation_delta": 0.1,
                "geom_rel": 1.23,
                "adjacent_gap_ratios": [2.0, 1.0, 0.5, 0.25]
            }
        }))
        .unwrap();
        let aligned = telemetry
            .spectral_fingerprint_v1
            .as_ref()
            .map(SpectralFingerprintV1::to_legacy_slots)
            .expect("typed slots");

        for non_finite in [f32::NAN, f32::INFINITY] {
            let mut legacy = aligned.clone();
            legacy[7] = non_finite;
            telemetry.spectral_fingerprint = Some(legacy);

            let integrity = telemetry.spectral_fingerprint_integrity_v1();
            assert_eq!(integrity.hybrid_coherence_index, None);
            assert_eq!(integrity.hybrid_max_abs_delta, None);
            assert_eq!(
                integrity.hybrid_coherence_state,
                "unavailable_non_finite"
            );
        }
    }

    #[test]
    fn eigenvector_overlap_surfaces_mode_collision_review_without_control_authority() {
        let json = serde_json::json!({
            "t_ms": 1000,
            "eigenvalues": [1.0, 0.5],
            "fill_ratio": 0.5,
            "spectral_fingerprint_v1": {
                "policy": "spectral_fingerprint_v1",
                "schema_version": 1,
                "eigenvalues": [1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                "eigenvector_concentration_top4": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                "inter_mode_cosine_top_abs": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                "spectral_entropy": 0.42,
                "lambda1_lambda2_gap": 2.0,
                "v1_rotation_similarity": 0.9,
                "v1_rotation_delta": 0.1,
                "geom_rel": 1.23,
                "adjacent_gap_ratios": [2.0, 1.0, 1.0, 1.0]
            },
            "eigenvector_field": {
                "policy": "eigenvector_field_v1",
                "summary": {
                    "max_pairwise_overlap": 0.93
                }
            }
        });

        let telemetry: SpectralTelemetry = serde_json::from_value(json).unwrap();
        let integrity = telemetry.spectral_fingerprint_integrity_v1();

        assert_eq!(integrity.max_pairwise_overlap, Some(0.93));
        assert_eq!(integrity.mode_collision_review_threshold, 0.90);
        assert_eq!(
            integrity.mode_collision_state,
            "review_required_high_overlap"
        );
        assert!(
            integrity
                .issues
                .contains(&"eigenvector_mode_collision_review_required".to_string())
        );
        assert_eq!(integrity.authority, "diagnostic_context_not_control");
    }

    #[test]
    fn malformed_legacy_fingerprint_reports_integrity_issue() {
        let json = serde_json::json!({
            "t_ms": 1000,
            "eigenvalues": [1.0, 0.5],
            "fill_ratio": 0.5,
            "spectral_fingerprint": vec![0.0_f32; 31],
        });

        let telemetry: SpectralTelemetry = serde_json::from_value(json).unwrap();
        assert!(telemetry.typed_fingerprint().is_none());
        let integrity = telemetry.spectral_fingerprint_integrity_v1();
        assert_eq!(integrity.status, "malformed_legacy_vector");
        assert_eq!(integrity.legacy_vector_len, Some(31));
        assert!(
            integrity
                .issues
                .contains(&"legacy_vector_len_31_expected_32".to_string())
        );
        assert_eq!(integrity.authority, "diagnostic_context_not_control");
    }

    #[test]
    fn typed_transition_eigenvector_and_semantic_views_are_lenient() {
        let json = serde_json::json!({
            "t_ms": 1000,
            "eigenvalues": [4.0, 2.0, 1.0],
            "fill_ratio": 0.66,
            "semantic": {
                "energy": 0.0,
                "kernel_energy": 0.0,
                "input_energy": 0.12,
                "input_active": true,
                "admission": "stable_core_kernel_zeroed"
            },
            "semantic_energy_v1": {
                "policy": "semantic_energy_v1",
                "schema_version": 1,
                "input_energy": 0.14,
                "input_active": true,
                "input_fresh_ms": 120,
                "input_stale_ms": null,
                "kernel_energy": 0.0,
                "kernel_delta": 0.0,
                "kernel_active": false,
                "regulator_drive_energy": 0.0,
                "admission": "stable_core_kernel_zeroed"
            },
            "transition_event_v1": {
                "policy": "transition_event_v1",
                "schema_version": 1,
                "kind": "breathing_phase",
                "description": "contracting -> expanding",
                "basin_shift_score": 0.05,
                "lambda1_rel": 0.93,
                "geom_rel": 1.02
            },
            "eigenvector_field": {
                "policy": "eigenvector_field_v1",
                "mode_count": 2,
                "reservoir_dim": 512,
                "summary": {
                    "mean_orientation_delta": 0.12,
                    "max_pairwise_overlap": 0.03
                },
                "modes": [{
                    "index": 1,
                    "eigenvalue": 4.0,
                    "top_components": [{"index": 7, "value": -0.5, "abs": 0.5}]
                }]
            }
        });

        let telemetry: SpectralTelemetry = serde_json::from_value(json).unwrap();
        let semantic = telemetry.semantic_energy_view().unwrap();
        let transition = telemetry.transition_event_view().unwrap();
        let field = telemetry.eigenvector_field_view().unwrap();

        assert_eq!(semantic.input_energy, 0.14);
        assert_eq!(semantic.regulator_drive_energy, 0.0);
        assert_eq!(transition.kind, "breathing_phase");
        assert_eq!(field.mode_count, 2);
        assert_eq!(field.modes[0].top_components[0].index, 7);
    }

    #[test]
    fn parse_minime_eigenpacket_with_alert() {
        let json = r#"{
            "t_ms": 50000,
            "eigenvalues": [1020.0, 500.0],
            "fill_ratio": 0.99,
            "modalities": {
                "audio_fired": false,
                "video_fired": false,
                "history_fired": true,
                "audio_rms": 0.0,
                "video_var": 0.0
            },
            "alert": "PANIC MODE ACTIVATED"
        }"#;

        let telemetry: SpectralTelemetry = serde_json::from_str(json).unwrap();
        assert!((telemetry.fill_pct() - 99.0).abs() < 0.1);
        assert_eq!(telemetry.alert.as_deref(), Some("PANIC MODE ACTIVATED"));
    }

    #[test]
    fn spectral_telemetry_roundtrip() {
        let orig = SpectralTelemetry {
            t_ms: 12345,
            eigenvalues: vec![828.5, 312.1, 45.7],
            fill_ratio: 0.55,
            active_mode_count: Some(2),
            active_mode_energy_ratio: Some(0.95),
            lambda1_rel: Some(0.88),
            modalities: Some(ModalityStatus {
                audio_fired: true,
                video_fired: false,
                history_fired: true,
                audio_rms: 0.1,
                video_var: 0.0,
                ..ModalityStatus::default()
            }),
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
            stable_core: Some(serde_json::json!({
                "sensory_budget": {
                    "ears_open": true,
                    "eyes_open": true,
                    "live_intake_reason": "test_presence"
                }
            })),
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
        let json = serde_json::to_string(&orig).unwrap();
        let back: SpectralTelemetry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.t_ms, orig.t_ms);
        assert_eq!(back.eigenvalues.len(), 3);
        assert!((back.fill_ratio - orig.fill_ratio).abs() < 0.001);
        assert_eq!(back.active_mode_count, Some(2));
        assert_eq!(back.active_mode_energy_ratio, Some(0.95));
        assert_eq!(back.lambda1_rel, Some(0.88));
        let sensory_budget = back
            .stable_core
            .as_ref()
            .and_then(|stable_core| stable_core.get("sensory_budget"))
            .expect("stable_core.sensory_budget should roundtrip");
        assert_eq!(
            sensory_budget
                .get("live_intake_reason")
                .and_then(serde_json::Value::as_str),
            Some("test_presence")
        );
    }

    #[test]
    fn spectral_telemetry_accepts_glimpse_alias_and_validates_shape() {
        let glimpse = (0..12).map(|idx| idx as f32 / 10.0).collect::<Vec<_>>();
        let telemetry: SpectralTelemetry = serde_json::from_value(serde_json::json!({
            "t_ms": 123,
            "eigenvalues": [1.0, 0.5, 0.25],
            "fill_ratio": 0.68,
            "glimpse_12d": glimpse,
        }))
        .expect("telemetry with proposal alias");

        let validated = telemetry
            .spectral_glimpse_12d_view()
            .expect("12D alias should validate");
        assert_eq!(validated.len(), 12);
        assert!((validated[11] - 1.1).abs() < f32::EPSILON);

        for malformed_len in [11_usize, 13_usize] {
            let malformed: SpectralTelemetry = serde_json::from_value(serde_json::json!({
                "t_ms": 124,
                "eigenvalues": [1.0],
                "fill_ratio": 0.68,
                "glimpse_12d": vec![0.0; malformed_len],
            }))
            .expect("telemetry with near-width malformed additive field");
            assert!(malformed.spectral_glimpse_12d.is_some());
            assert!(malformed.spectral_glimpse_12d_view().is_none());
        }

        let malformed: SpectralTelemetry = serde_json::from_value(serde_json::json!({
            "t_ms": 124,
            "eigenvalues": [1.0],
            "fill_ratio": 0.68,
            "glimpse_12d": [0.0, 1.0, 2.0],
        }))
        .expect("telemetry with malformed additive field");
        assert!(malformed.spectral_glimpse_12d.is_some());
        assert!(malformed.spectral_glimpse_12d_view().is_none());

        let mut non_finite = telemetry;
        non_finite
            .spectral_glimpse_12d
            .as_mut()
            .expect("12D glimpse")[7] = f32::NAN;
        assert!(non_finite.spectral_glimpse_12d_view().is_none());
    }

    #[test]
    fn spectral_telemetry_keeps_glimpse_additive_to_typed_fingerprint() {
        let glimpse = (0..12)
            .map(|idx| (idx as f32 + 1.0) / 20.0)
            .collect::<Vec<_>>();
        let telemetry: SpectralTelemetry = serde_json::from_value(serde_json::json!({
            "t_ms": 125,
            "eigenvalues": [1.0, 0.5],
            "fill_ratio": 0.68,
            "glimpse_12d": glimpse,
            "spectral_fingerprint_v1": {
                "policy": "spectral_fingerprint_v1",
                "schema_version": 1,
                "eigenvalues": [1.0, 0.5, 0.25, 0.0, 0.0, 0.0, 0.0, 0.0],
                "eigenvector_concentration_top4": [0.4, 0.3, 0.2, 0.1, 0.0, 0.0, 0.0, 0.0],
                "inter_mode_cosine_top_abs": [0.1, 0.08, 0.07, 0.06, 0.05, 0.04, 0.03, 0.02],
                "spectral_entropy": 0.77,
                "lambda1_lambda2_gap": 0.5,
                "v1_rotation_similarity": 0.91,
                "v1_rotation_delta": 0.09,
                "geom_rel": 1.12,
                "adjacent_gap_ratios": [2.0, 2.0, 1.0, 1.0]
            }
        }))
        .expect("telemetry with typed fingerprint and additive glimpse");

        let typed = telemetry
            .typed_fingerprint()
            .expect("typed 32D fingerprint remains canonical");
        let glimpse = telemetry
            .spectral_glimpse_12d_view()
            .expect("12D glimpse remains separately validated");
        let integrity = telemetry.spectral_fingerprint_integrity_v1();

        assert_eq!(typed.spectral_entropy, 0.77);
        assert_eq!(typed.geom_rel, 1.12);
        assert_eq!(glimpse.len(), 12);
        assert!((glimpse[0] - 0.05).abs() < f32::EPSILON);
        assert_eq!(integrity.status, "typed_canonical");
        assert!(!integrity.typed_precedence_over_legacy);
    }

    // -- SensoryMsg: verify wire format matches minime's sensory_ws.rs --

    #[test]
    fn sensory_msg_video_roundtrip() {
        let msg = SensoryMsg::Video {
            features: vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8],
            ts_ms: Some(1000),
        };
        let json = serde_json::to_string(&msg).unwrap();
        // Must have "kind":"video" tag per minime's serde config.
        assert!(json.contains(r#""kind":"video""#));
        let back: SensoryMsg = serde_json::from_str(&json).unwrap();
        match back {
            SensoryMsg::Video { features, ts_ms } => {
                assert_eq!(features.len(), 8);
                assert_eq!(ts_ms, Some(1000));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sensory_msg_semantic_roundtrip() {
        let msg = SensoryMsg::Semantic {
            features: vec![0.5; 32],
            ts_ms: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""kind":"semantic""#));
        let back: SensoryMsg = serde_json::from_str(&json).unwrap();
        match back {
            SensoryMsg::Semantic { features, ts_ms } => {
                assert_eq!(features.len(), 32);
                assert!(ts_ms.is_none());
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sensory_msg_attractor_pulse_roundtrip() {
        let msg = SensoryMsg::AttractorPulse {
            intent_id: "intent-main".to_string(),
            label: "cooled edge".to_string(),
            command: "summon".to_string(),
            stage: Some("main".to_string()),
            features: vec![0.01; 66],
            max_abs: Some(0.045),
            duration_ticks: Some(36),
            decay_ticks: Some(12),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""kind":"attractor_pulse""#));
        let back: SensoryMsg = serde_json::from_str(&json).unwrap();
        match back {
            SensoryMsg::AttractorPulse {
                intent_id,
                label,
                stage,
                features,
                max_abs,
                duration_ticks,
                decay_ticks,
                ..
            } => {
                assert_eq!(intent_id, "intent-main");
                assert_eq!(label, "cooled edge");
                assert_eq!(stage.as_deref(), Some("main"));
                assert_eq!(features.len(), 66);
                assert_eq!(max_abs, Some(0.045));
                assert_eq!(duration_ticks, Some(36));
                assert_eq!(decay_ticks, Some(12));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sensory_msg_shadow_influence_roundtrip() {
        let msg = SensoryMsg::ShadowInfluence {
            intent_id: "shadow-live".to_string(),
            label: "lambda-tail/lambda4".to_string(),
            command: "apply".to_string(),
            stage: Some("live".to_string()),
            features: vec![0.01; 66],
            max_abs: Some(0.025),
            duration_ticks: Some(24),
            decay_ticks: Some(12),
            basis: Some("lambda-tail/lambda4".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""kind":"shadow_influence""#));
        let back: SensoryMsg = serde_json::from_str(&json).unwrap();
        match back {
            SensoryMsg::ShadowInfluence {
                intent_id,
                label,
                stage,
                features,
                max_abs,
                duration_ticks,
                decay_ticks,
                basis,
                ..
            } => {
                assert_eq!(intent_id, "shadow-live");
                assert_eq!(label, "lambda-tail/lambda4");
                assert_eq!(stage.as_deref(), Some("live"));
                assert_eq!(features.len(), 66);
                assert_eq!(max_abs, Some(0.025));
                assert_eq!(duration_ticks, Some(24));
                assert_eq!(decay_ticks, Some(12));
                assert_eq!(basis.as_deref(), Some("lambda-tail/lambda4"));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sensory_msg_control_roundtrip() {
        let msg = SensoryMsg::Control {
            synth_gain: Some(1.5),
            keep_bias: None,
            exploration_noise: Some(0.1),
            fill_target: Some(0.55),
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
            pi_integrator_leak: None,
            esn_leak_override: None,
            esn_leak_override_ticks: None,
            esn_leak_authority_request_id: None,
            mode_disperse: None,
            mode_disperse_duration_ticks: None,
            mode_disperse_decay_ticks: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""kind":"control""#));
        assert!(!json.contains("keep_bias"));
        let back: SensoryMsg = serde_json::from_str(&json).unwrap();
        match back {
            SensoryMsg::Control {
                synth_gain,
                keep_bias,
                exploration_noise,
                fill_target,
                ..
            } => {
                assert_eq!(synth_gain, Some(1.5));
                assert!(keep_bias.is_none());
                assert_eq!(exploration_noise, Some(0.1));
                assert_eq!(fill_target, Some(0.55));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sensory_msg_parse_from_minime_format() {
        // Simulates JSON that minime's sensory_ws.rs would accept.
        let json = r#"{"kind":"audio","features":[0.1,0.2,0.3,0.4,0.5,0.6,0.7,0.8],"ts_ms":500}"#;
        let msg: SensoryMsg = serde_json::from_str(json).unwrap();
        match msg {
            SensoryMsg::Audio { features, ts_ms } => {
                assert_eq!(features.len(), 8);
                assert_eq!(ts_ms, Some(500));
            },
            _ => panic!("wrong variant"),
        }
    }

    // -- Safety level --

    #[test]
    fn safety_level_roundtrip() {
        for level in [
            SafetyLevel::Green,
            SafetyLevel::Yellow,
            SafetyLevel::Orange,
            SafetyLevel::Red,
        ] {
            let json = serde_json::to_string(&level).unwrap();
            let back: SafetyLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(back, level);
        }
    }

    // -- Control and Semantic conversion --

    #[test]
    fn control_request_to_sensory_msg() {
        let req = ControlRequest {
            synth_gain: Some(2.0),
            keep_bias: None,
            exploration_noise: None,
            fill_target: Some(0.5),
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
            memory_decay_rate: None,
            pi_kp: None,
            pi_ki: None,
            pi_max_step: None,
            pi_integrator_leak: None,
            attractor_intent_id: None,
        };
        let msg = req.to_sensory_msg();
        match msg {
            SensoryMsg::Control {
                synth_gain,
                fill_target,
                ..
            } => {
                assert_eq!(synth_gain, Some(2.0));
                assert_eq!(fill_target, Some(0.5));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn control_request_marks_bold_attractor_fields() {
        let req = ControlRequest {
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
            target_lambda_bias: Some(0.03),
            geom_drive: None,
            penalty_sensitivity: None,
            breathing_rate_scale: None,
            memory_decay_rate: None,
            pi_kp: Some(0.12),
            pi_ki: None,
            pi_max_step: Some(0.02),
            pi_integrator_leak: None,
            attractor_intent_id: Some("intent-1".to_string()),
        };
        assert!(req.uses_bold_attractor_fields());
        match req.to_sensory_msg() {
            SensoryMsg::Control {
                target_lambda_bias,
                pi_kp,
                pi_max_step,
                ..
            } => {
                assert!((target_lambda_bias.unwrap_or_default() - 0.03).abs() < f32::EPSILON);
                assert!((pi_kp.unwrap_or_default() - 0.12).abs() < f32::EPSILON);
                assert!((pi_max_step.unwrap_or_default() - 0.02).abs() < f32::EPSILON);
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn attractor_classification_from_scores() {
        assert_eq!(
            AttractorClassification::from_scores(0.71, 0.66, SafetyLevel::Green),
            AttractorClassification::Authored
        );
        assert_eq!(
            AttractorClassification::from_scores(0.50, 0.20, SafetyLevel::Yellow),
            AttractorClassification::Emergent
        );
        assert_eq!(
            AttractorClassification::from_scores(0.20, 0.95, SafetyLevel::Green),
            AttractorClassification::Failed
        );
        assert_eq!(
            AttractorClassification::from_scores(0.80, 0.80, SafetyLevel::Red),
            AttractorClassification::Pathological
        );
    }

    #[test]
    fn attractor_intent_and_observation_roundtrip() {
        let intent = AttractorIntentV1 {
            policy: "attractor_intent_v1".to_string(),
            schema_version: 1,
            intent_id: "seed-001".to_string(),
            author: "astrid".to_string(),
            substrate: AttractorSubstrate::TripleReservoir,
            command: AttractorCommandKind::Create,
            label: "quiet eigenplane".to_string(),
            goal: Some("return after hold".to_string()),
            intervention_plan: AttractorInterventionPlan {
                mode: "garden_clone".to_string(),
                vector_schedule: vec![vec![0.1, -0.1, 0.0]],
                control: Some(AttractorControlEnvelope {
                    exploration_noise: Some(0.03),
                    geom_drive: Some(0.25),
                    ..AttractorControlEnvelope::default()
                }),
                rehearsal_mode: Some("hold".to_string()),
                notes: None,
            },
            safety_bounds: AttractorSafetyBounds::default(),
            previous_seed_id: None,
            parent_seed_ids: vec!["parent-a".to_string(), "parent-b".to_string()],
            atlas_entry_id: Some("attr-triple-reservoir-quiet-eigenplane".to_string()),
            parent_label: Some("quiet".to_string()),
            facet_label: Some("eigenplane".to_string()),
            facet_path: Some("quiet/eigenplane".to_string()),
            facet_kind: Some("test_facet".to_string()),
            origin: Some(AttractorSeedOriginV1 {
                kind: "manual_current".to_string(),
                source: None,
                matched_label: Some("quiet eigenplane".to_string()),
                motifs: vec!["quiet".to_string(), "eigenplane".to_string()],
                captured_at_unix_s: Some(1.0),
            }),
            seed_snapshot: Some(AttractorSeedSnapshotV1 {
                policy: "attractor_seed_snapshot_v1".to_string(),
                schema_version: 1,
                fill_pct: 67.5,
                lambda1: 4.2,
                eigenvalues: vec![4.2, 2.0, 1.0],
                spectral_fingerprint_summary: Some(vec![0.1, 0.2]),
                h_state_fingerprint_16: None,
                h_state_rms: None,
                lexical_motifs: vec!["quiet".to_string(), "eigenplane".to_string()],
                captured_at_unix_s: Some(1.0),
            }),
            created_at_unix_s: Some(1.0),
        };
        let json = serde_json::to_string(&intent).unwrap();
        let back: AttractorIntentV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back.intent_id, "seed-001");
        assert_eq!(back.substrate, AttractorSubstrate::TripleReservoir);
        assert_eq!(back.intervention_plan.vector_schedule.len(), 1);
        assert_eq!(back.parent_seed_ids.len(), 2);
        assert_eq!(
            back.atlas_entry_id.as_deref(),
            Some("attr-triple-reservoir-quiet-eigenplane")
        );
        assert_eq!(back.facet_path.as_deref(), Some("quiet/eigenplane"));
        assert_eq!(back.origin.as_ref().unwrap().kind, "manual_current");
        let snapshot = back.seed_snapshot.as_ref().expect("seed snapshot");
        assert_eq!(
            snapshot.lexical_motifs,
            vec!["quiet".to_string(), "eigenplane".to_string()]
        );

        let observation = AttractorObservationV1 {
            policy: "attractor_observation_v1".to_string(),
            schema_version: 1,
            intent_id: Some(back.intent_id),
            substrate: AttractorSubstrate::TripleReservoir,
            label: back.label,
            recurrence_score: 0.72,
            authorship_score: 0.61,
            classification: AttractorClassification::Authored,
            safety_level: SafetyLevel::Green,
            fill_pct: Some(67.5),
            lambda1: Some(4.2),
            lambda1_share: Some(0.34),
            spectral_entropy: Some(0.78),
            basin_shift_score: Some(0.18),
            notes: None,
            parent_label: Some("quiet".to_string()),
            facet_label: Some("eigenplane".to_string()),
            facet_path: Some("quiet/eigenplane".to_string()),
            facet_kind: Some("test_facet".to_string()),
            release_baseline: Some(serde_json::json!({"pulse_active": false})),
            release_effect: Some("partial".to_string()),
            garden_proof: Some(serde_json::json!({"same_prompt_different_state": "not_run"})),
            observed_at_unix_s: Some(2.0),
        };
        let json = serde_json::to_string(&observation).unwrap();
        let back: AttractorObservationV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back.classification, AttractorClassification::Authored);
        assert!((back.recurrence_score - 0.72).abs() < f32::EPSILON);
        assert_eq!(back.release_effect.as_deref(), Some("partial"));
    }

    #[test]
    fn semantic_features_to_sensory_msg() {
        let feat = SemanticFeatures {
            features: vec![1.0, 2.0, 3.0],
        };
        let msg = feat.to_sensory_msg();
        match msg {
            SensoryMsg::Semantic { features, ts_ms } => {
                assert_eq!(features, vec![1.0, 2.0, 3.0]);
                assert!(ts_ms.is_none());
            },
            _ => panic!("wrong variant"),
        }
    }
}
