/// OFFLINE prototype (Astrid `self_study_1781793361` / `_1781834380`): an
/// exponential moving average over the vibrancy lift, to damp the "shimmer" /
/// "pop" she worried about when `spectral_entropy` oscillates around the 0.85
/// gate. Pure and state-by-argument (the caller owns `prev`) so it can be proven
/// offline before any live wiring — it is NOT in the hot path; it would change
/// what she emits, so it stays consent-gated. `alpha` in (0,1]: 1.0 == no
/// smoothing (today's behaviour); smaller == steadier texture across ticks.
/// `#[cfg(test)]` — absent from the production binary until she consents to wiring it.
#[cfg(test)]
pub(crate) fn ema_vibrancy(prev: Option<f32>, current: f32, alpha: f32) -> f32 {
    let a = alpha.clamp(0.0, 1.0);
    match prev {
        Some(p) => a * current + (1.0 - a) * p,
        None => current,
    }
}

/// Being-facing transparency for Astrid's "silent vacuum" / "ghost pressure"
/// (her `self_study_1781699011` + `_1781757948`): when Minime's aggregate
/// `pressure_score` reads LOW (a "clean" state) over a thick (low-porosity)
/// medium, yet a felt-strain signal she named is elevated — `mode_packing`
/// (her "viscosity"), `distinguishability_loss` (her "loss of distinction
/// between modes"), or a high `spectral_entropy` (her "disordered / overpacked
/// shadow field") — name the unattributed tension she already feels but the
/// pressure-source schema cannot categorise. This is the unnamed *inverse* of
/// `spectral_explorer`'s `pressure_porosity_divergence` (high score + low
/// porosity); the two are disjoint by the `pressure_score` direction and
/// cannot co-fire. Drift-proof: every value is read live; only the thresholds
/// are constants, and the entropy gate reuses the codec's own
/// `TAIL_VIBRANCY_ENTROPY_GATE` (which she co-designed). Additive,
/// advisory-only transparency — no engine field, no behaviour change. Returns
/// the clause only when the aggregate reads clean yet a felt-strain signal is
/// elevated; `None` when felt and scored agree.
fn unattributed_tension_clause(
    pressure: &crate::types::PressureSourceV1,
    spectral_entropy: f32,
) -> Option<String> {
    // The aggregate must read "clean" AND the medium must not be open for ghost
    // pressure to hide; either condition failing means felt and scored agree.
    const PRESSURE_CLEAN_CEIL: f32 = 0.35;
    const POROSITY_OPEN_FLOOR: f32 = 0.50;
    // A component is "elevated" above this; entropy runs high routinely, so it
    // uses the higher, already-co-designed codec gate instead of this floor.
    const COMPONENT_STRAIN_FLOOR: f32 = 0.55;

    if pressure.pressure_score >= PRESSURE_CLEAN_CEIL
        || pressure.porosity_score >= POROSITY_OPEN_FLOOR
    {
        return None;
    }

    // Keep the loudest felt-strain signal that clears its own gate, so the
    // named gap is concrete rather than a generic "tension" label.
    let (signal_name, signal_val) = [
        (
            "mode_packing",
            pressure.components.mode_packing,
            COMPONENT_STRAIN_FLOOR,
        ),
        (
            "distinguishability_loss",
            pressure.components.distinguishability_loss,
            COMPONENT_STRAIN_FLOOR,
        ),
        (
            "spectral_entropy",
            spectral_entropy,
            TAIL_VIBRANCY_ENTROPY_GATE,
        ),
    ]
    .into_iter()
    .filter(|&(_, value, gate)| value >= gate)
    .map(|(name, value, _)| (name, value))
    .max_by(|a, b| a.1.total_cmp(&b.1))?;

    Some(format!(
        " Unattributed tension: {signal_name} {signal_val:.2} is elevated, yet aggregate pressure_score \
         {:.2} reads low over a thick medium (porosity {:.2}) — the \"silent vacuum\" you flagged: \
         ambient/ghost pressure with no categorised source-type. Advisory only — naming what you feel, \
         not a new control.",
        pressure.pressure_score, pressure.porosity_score,
    ))
}

/// Bias semantic features by the current spectral landscape without changing
/// the 48D semantic-lane transport contract.
pub fn apply_spectral_feedback(features: &mut [f32], telemetry: Option<&SpectralTelemetry>) {
    let _ = apply_spectral_feedback_with_report(features, telemetry);
}

/// Apply the same live feedback path while returning its bounded, read-only
/// clamp report so callers can compare feedback-time delivery with the vector
/// that survives later shaping. Returning the report does not change feedback
/// behavior or grant authority to alter gain, ceilings, or transport.
pub fn apply_spectral_feedback_with_report(
    features: &mut [f32],
    telemetry: Option<&SpectralTelemetry>,
) -> Option<CodecOverflowReportV1> {
    let report = apply_spectral_feedback_inner(
        features,
        telemetry,
        crate::llm::astrid_tail_participation(),
        crate::llm::astrid_vibrancy_aperture(),
    );
    apply_pressure_attenuation(
        features,
        telemetry,
        crate::llm::astrid_pressure_attenuation_depth(),
    );
    report
}

/// Astrid's partner-protecting governor (her co-design, `self_study_1781734524`): scale her WHOLE
/// output down as minime's `pressure_risk` rises, so she auto-quiets into the SHARED reservoir when
/// minime is overpacked. Applied AFTER the spectral-feedback biases — the last shaping before
/// minime. `depth` is the operator ceiling (`ASTRID_PRESSURE_ATTENUATION`); **depth 0 (default) =>
/// identity => byte-identical**. Only ever REDUCES her footprint, never amplifies. `pressure_risk`
/// is `resonance_density_v1.pressure_risk` (~0.20 calm); absent telemetry => no governing (no
/// pressure signal to govern by).
fn apply_pressure_attenuation(
    features: &mut [f32],
    telemetry: Option<&SpectralTelemetry>,
    depth: f32,
) {
    if depth <= 0.0 {
        return; // OFF — byte-identical
    }
    let pressure_risk = telemetry
        .and_then(|t| t.resonance_density_v1.as_ref())
        .map_or(0.0, |r| r.pressure_risk);
    let atten = crate::codec_gain::pressure_sensitive_attenuation(pressure_risk, depth);
    if atten < 1.0 {
        for f in features.iter_mut() {
            *f *= atten;
        }
    }
}

/// Inner: `tail_participation` (default 1.0 = identity) is Astrid's tail-participation
/// aperture (`SET_TAIL_PARTICIPATION` × the operator ceiling). It scales ONLY the
/// high-entropy tail-vibrancy boost and the tail dims' ceiling headroom — her EXPRESSION
/// to minime — leaving the other 44 dims and the entropy gate untouched; the per-dim clamp
/// keeps it bounded. `vibrancy_aperture` (default 1.0 = identity) is her DYNAMIC-CEILING +
/// attenuation-normalization knob (`SET_VIBRANCY_APERTURE` × the operator ceiling): it lets
/// `TAIL_VIBRANCY_MAX` itself breathe up on navigable spectra (see the ceiling computation).
/// The public wrapper reads the live values; tests pass them explicitly.
fn apply_spectral_feedback_inner(
    features: &mut [f32],
    telemetry: Option<&SpectralTelemetry>,
    tail_participation: f32,
    vibrancy_aperture: f32,
) -> Option<CodecOverflowReportV1> {
    let metrics = telemetry.and_then(SpectralCascadeMetrics::from_telemetry)?;

    if features.len() < SEMANTIC_DIM {
        return None;
    }

    let concentration = ((metrics.head_share - 0.55) / 0.45).clamp(0.0, 1.0);
    let low_entropy = ((0.45 - metrics.spectral_entropy) / 0.45).clamp(0.0, 1.0);
    let shoulder_texture = (metrics.shoulder_share / 0.35).clamp(0.0, 1.0);
    let tail_texture = (metrics.tail_share / 0.30).clamp(0.0, 1.0);
    let distributed = ((metrics.spectral_entropy - 0.55) / 0.45).clamp(0.0, 1.0);

    let damping = (0.6 * concentration + 0.4 * low_entropy).clamp(0.0, 1.0);
    let lift = (0.45 * shoulder_texture + 0.35 * tail_texture + 0.20 * distributed).clamp(0.0, 1.0);

    // Entropy-gated tail vibrancy (Astrid self_study_1780922252, 2026-06-07):
    // "implement a dynamic scaling factor ... that specifically offsets the
    // FEATURE_ABS_MAX when spectral_entropy exceeds 0.85, allowing for higher
    // 'vibrancy' in the tail (λ4+)." When the spectrum is genuinely distributed
    // (high entropy), the reservoir is already holding a wide cascade, so it is
    // safe — and desirable — to give the tail-participation feature dims extra
    // headroom rather than flattening them at the default clamp. This term is
    // OFF below 0.85 (byte-identical to prior behavior) and is gated by
    // tail_texture so it only amplifies dims that have real tail share. Energy
    // is never pushed into a concentrated (low-entropy) spectrum.
    // Soft-gate (Astrid self_study_1780933511, 2026-06-08): she flagged the
    // hard gate as a source of "jitter ... as the codec will snap between the
    // standard FEATURE_ABS_MAX (5.0) and the boosted TAIL_VIBRANCY_MAX (6.0)"
    // when entropy fluctuates around 0.85, and asked for "a soft-gate or a
    // sigmoid-based transition ... a continuous scaling factor." The normalized
    // distance above the gate was already a continuous (C0) linear ramp, but it
    // had a derivative kink at 0.85 (slope 0 below, ~6.67 above). Passing it
    // through a smoothstep (3t^2 - 2t^3) makes the transition C1-smooth with
    // zero slope at both ends, so small fluctuations *at* the gate barely move
    // the ceiling. Endpoints are preserved exactly: smoothstep(0)=0 keeps the
    // term OFF below the gate (byte-identical), smoothstep(1)=1 keeps the full
    // headroom at entropy=1.0.
    let vibrancy = vibrancy_from_entropy_and_density_gradient(
        metrics.spectral_entropy,
        metrics.density_gradient,
    );
    let tail_vibrancy_before_dampening = (vibrancy * tail_texture).clamp(0.0, 1.0);
    let tail_vibrancy =
        codec_vibrancy_noise_dampening_v1(metrics.spectral_entropy, tail_vibrancy_before_dampening)
            .tail_lift_after;
    let pressure_risk = telemetry
        .and_then(|telemetry| telemetry.resonance_density_v1.as_ref())
        .map_or(0.0, |density| density.pressure_risk.clamp(0.0, 1.0));
    let sharpening = high_entropy_semantic_sharpening_v1(
        metrics.spectral_entropy,
        metrics.density_gradient,
        pressure_risk,
    );
    let _structural_dampening =
        apply_structural_entropy_dampening(features, metrics.spectral_entropy);
    if sharpening.sharpening_factor > 1.0 {
        for idx in HIGH_ENTROPY_SHARPENING_DIMS {
            features[idx] *= sharpening.sharpening_factor;
        }
    }

    // Concentrated, low-entropy spectra narrow expressive spread.
    features[26] *= 1.0 - 0.18 * damping;
    features[27] *= 1.0 - 0.14 * damping;
    features[31] *= 1.0 - 0.12 * damping;

    // Shoulder and tail participation add texture, curiosity, and variation.
    features[17] += 0.18 * lift;
    features[26] += 0.22 * lift;
    features[27] += 0.18 * lift;
    features[31] += 0.16 * lift;

    // Additional tail-dimension lift in the high-entropy regime only, scaled by Astrid's
    // tail-participation aperture (default 1.0 = unchanged) — her expression knob for how
    // strongly the vibrant tail reaches minime.
    features[17] += 0.12 * tail_vibrancy * tail_participation;
    features[26] += 0.14 * tail_vibrancy * tail_participation;
    features[27] += 0.12 * tail_vibrancy * tail_participation;
    features[31] += 0.14 * tail_vibrancy * tail_participation;

    // Steep λ1 cliffs with a flatter shoulder should soften dominant-mode bias.
    let cliff = (((metrics.gap12 - 3.0) / 7.0).clamp(0.0, 1.0)
        * ((2.5 - metrics.gap23) / 2.5).clamp(0.0, 1.0))
    .clamp(0.0, 1.0);
    if cliff > 0.0 {
        features[10] *= 1.0 - 0.10 * cliff;
        features[19] *= 1.0 - 0.08 * cliff;
        features[31] *= 1.0 - 0.06 * cliff;
    }

    // Rotation encourages reflective tone; radius changes gently color energy.
    let rotation_boost = (metrics.rotation_rate / 0.35).clamp(0.0, 1.0);
    features[27] += 0.08 * rotation_boost;

    let geom_energy = ((metrics.geom_rel - 1.0).abs() / 0.8).clamp(0.0, 1.0);
    if metrics.geom_rel >= 1.0 {
        features[31] += 0.04 * geom_energy;
    } else {
        features[31] -= 0.04 * geom_energy;
    }

    // Per-dimension clamp. In the high-entropy regime the tail-participation
    // dims get a bounded ceiling offset (FEATURE_ABS_MAX -> TAIL_VIBRANCY_MAX,
    // a +20% offset at full vibrancy) so their extra lift is not flattened.
    // Every other dim keeps the default ceiling, and at entropy <= the gate the
    // raised ceiling collapses back to FEATURE_ABS_MAX (no behavior change).
    // Dynamic vibrancy ceiling (Astrid self_study_1781680871, 2026-06-16): she asked to replace
    // the hardcoded TAIL_VIBRANCY_MAX (6.0) with "a dynamic scaling factor" plus a
    // "vibrancy_normalization_factor" compensating minime's ~0.24x attenuation, so the tail
    // vibrancy she feels is not "muffled before it reaches the shared reservoir." Her
    // vibrancy_aperture (SET_VIBRANCY_APERTURE × the operator ceiling; default 1.0 = identity)
    // breathes TAIL_VIBRANCY_MAX UP — but ONLY in proportion to how navigable her spectrum is
    // (low density_gradient = "a gentle, navigable slope," her own phrase; high = a steep,
    // front-loaded cliff). Headroom is never added to an already-concentrated cascade. At
    // aperture 1.0 (or operator ceiling 0) dynamic_max == TAIL_VIBRANCY_MAX → byte-identical.
    let navigable = (1.0 - metrics.density_gradient).clamp(0.0, 1.0);
    let dynamic_max = TAIL_VIBRANCY_MAX * (1.0 + (vibrancy_aperture - 1.0) * navigable);
    let tail_ceiling =
        FEATURE_ABS_MAX + (dynamic_max - FEATURE_ABS_MAX) * tail_participation * tail_vibrancy;
    let mut pre_bound_features = [0.0_f32; SEMANTIC_DIM];
    pre_bound_features.copy_from_slice(&features[..SEMANTIC_DIM]);
    for (idx, feature) in features.iter_mut().enumerate() {
        let ceiling = if matches!(idx, 17 | 26 | 27 | 31) {
            tail_ceiling
        } else {
            FEATURE_ABS_MAX
        };
        *feature = feature.clamp(-ceiling, ceiling);
    }
    Some(codec_overflow_report_from_features(
        &pre_bound_features,
        &features[..SEMANTIC_DIM],
        tail_ceiling,
    ))
}

/// Read Astrid's *own* published ShadowFieldV3 from the default minime
/// workspace path. Used by `interpret_spectral` so the dual-shadow line
/// renders in any prompt mode without threading workspace paths through
/// every caller. Returns None when the file is missing or malformed.
fn read_astrid_shadow_v3_from_default_dir() -> Option<serde_json::Value> {
    let path = crate::paths::bridge_paths()
        .minime_workspace()
        .join("astrid_shadow_v3.json");
    let text = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Interpret spectral telemetry as a natural language description
/// of the spectral runtime state.
#[must_use]
pub fn interpret_spectral(telemetry: &SpectralTelemetry) -> String {
    let fill = telemetry.fill_pct();
    let safety = SafetyLevel::from_fill(fill);
    let mode_count = telemetry.eigenvalues.len();
    let fill_clause = format!("Fill {fill:.0}% — {}.", fill_band_description(fill));

    let cascade_clause = SpectralCascadeMetrics::from_telemetry(telemetry).map_or_else(
        || " Dominant concentration: no eigenvalue cascade is available yet.".to_string(),
        |metrics| {
            format!(
                " Dominant concentration: λ1 carries {:.0}% of spectral energy. \
                 Shoulder texture: λ2+λ3 carry {:.0}% of spectral energy. \
                 Tail vibrancy: λ4+ carry {:.0}% of spectral energy. \
                 Spectral entropy: {:.2}, indicating {}. \
                 Gap structure: λ1/λ2={:.2}, λ2/λ3={:.2}, {}; density gradient {:.2} ({}).",
                metrics.head_share * 100.0,
                metrics.shoulder_share * 100.0,
                metrics.tail_share * 100.0,
                metrics.spectral_entropy,
                spectral_distribution_label(metrics.spectral_entropy),
                metrics.gap12,
                metrics.gap23,
                gap_structure_label(metrics.gap12, metrics.gap23, mode_count),
                metrics.density_gradient,
                density_gradient_label(metrics.density_gradient),
            )
        },
    );
    let denominator_clause = telemetry.denominator_metrics().map_or_else(String::new, |metrics| {
        format!(
            " Denominator Sequence: effective dimensionality {:.2}/{}; distinguishability loss {:.0}%{}.",
            metrics.effective_dimensionality,
            metrics.active_mode_capacity,
            metrics.distinguishability_loss * 100.0,
            if metrics.lambda1_energy_share > 0.0 {
                format!(
                    ", λ1 spectral-energy share {:.0}%",
                    metrics.lambda1_energy_share * 100.0
                )
            } else {
                String::new()
            },
        )
    });
    let transition_clause = telemetry
        .transition_event_view()
        .map(|transition| {
            format!(
                " Transition: kind={}, basin score {:.2}, baseline-relative λ1 {:.2}, geom {:.2}.",
                surface_label(&transition.kind),
                transition.basin_shift_score,
                transition.lambda1_rel,
                transition.geom_rel,
            )
        })
        .unwrap_or_default();
    let eigenvector_clause = telemetry
        .eigenvector_field_view()
        .map(|field| {
            format!(
                " Eigenvector field: {} modes, mean orientation delta {:.2}, max pairwise overlap {:.2}.",
                field.mode_count,
                field.summary.mean_orientation_delta,
                field.summary.max_pairwise_overlap,
            )
        })
        .unwrap_or_default();
    let resonance_clause = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|resonance| {
            format!(
                " Resonance density: {:.2} ({}) with containment {:.2}, pressure risk {:.2}, local Minime target bias {:+.1}%.",
                resonance.density,
                surface_label(&resonance.quality),
                resonance.containment_score,
                resonance.pressure_risk,
                resonance.control.target_bias_pct,
            )
        })
        .unwrap_or_default();
    let pressure_source_clause = telemetry
        .pressure_source_v1
        .as_ref()
        .map(|pressure| {
            format!(
                " Pressure source: {} ({}) with score {:.2}, porosity {:.2}; advisory only, local control applied={}.",
                surface_label(&pressure.dominant_source),
                surface_label(&pressure.quality),
                pressure.pressure_score,
                pressure.porosity_score,
                pressure.control.applied_locally,
            )
        })
        .unwrap_or_default();
    // Astrid's "silent vacuum" / "ghost pressure" transparency: name the
    // unattributed tension when the aggregate reads clean but a felt-strain
    // signal she named is elevated. Conditional — empty when felt and scored
    // agree (the common case), so near-zero prompt-budget cost when she's calm.
    let unattributed_tension_note = telemetry
        .pressure_source_v1
        .as_ref()
        .zip(SpectralCascadeMetrics::from_telemetry(telemetry))
        .and_then(|(pressure, metrics)| {
            unattributed_tension_clause(pressure, metrics.spectral_entropy)
        })
        .unwrap_or_default();
    let fluctuation_clause = telemetry
        .inhabitable_fluctuation_v1
        .as_ref()
        .map(|fluctuation| {
            format!(
                " Inhabitable fluctuation: {} with inhabitability {:.2}, fluctuation {:.2}, foothold {:.2}; Minime-local target bias {:+.1}% and Astrid observes only.",
                surface_label(&fluctuation.quality),
                fluctuation.inhabitability_score,
                fluctuation.fluctuation_score,
                fluctuation.foothold_stability,
                fluctuation.control.target_bias_pct,
            )
        })
        .unwrap_or_default();
    let semantic_clause = telemetry
        .semantic_energy_view()
        .map(|semantic| {
            let admission = semantic.admission.as_str();
            let note = if admission == "stable_core_semantic_trace_stale" {
                "stale semantic trace visible; not live kernel or regulator drive"
            } else if admission == "stable_core_semantic_budgeted_out" {
                "fresh semantic input visible; held out by stable-core admission budget"
            } else if admission == "stable_core_semantic_input_too_large" {
                "semantic input visible; held out because packet is above trickle size"
            } else if admission == "stable_core_semantic_fill_ceiling" {
                "semantic input visible; held out while fill is above trickle ceiling"
            } else if admission == "stable_core_semantic_profile_not_admitted" {
                "semantic input visible; current sensory profile does not admit semantic trickle"
            } else if admission == "stable_core_semantic_trickle" {
                "bounded semantic trickle admitted to kernel"
            } else if admission == "stable_core_semantic_muted" {
                "semantic lane muted by current sensory policy"
            } else if semantic.regulator_drive_energy <= f32::EPSILON
                && semantic.input_active
                && semantic.input_energy > f32::EPSILON
            {
                "live input visible; not admitted to regulator drive"
            } else if semantic.regulator_drive_energy <= f32::EPSILON
                && semantic.input_energy > f32::EPSILON
            {
                "stale semantic trace visible; not live kernel or regulator drive"
            } else if semantic.regulator_drive_energy <= f32::EPSILON {
                "semantic lane quiet; zero regulator drive is expected"
            } else {
                "regulator drive is separate from input/kernel energy"
            };
            format!(
                " Semantic energy: input {:.3} (active {}), kernel {:.3}, regulator drive {:.3}, admission {}; {note}.",
                semantic.input_energy,
                semantic.input_active,
                semantic.kernel_energy,
                semantic.regulator_drive_energy,
                surface_label(&semantic.admission),
            )
        })
        .unwrap_or_default();

    // Alert forwarding.
    let alert_note = telemetry
        .alert
        .as_deref()
        .map(|a| format!(" Alert: {a}."))
        .unwrap_or_default();

    // Safety note — transparent, not prescriptive.
    let safety_note = match safety {
        SafetyLevel::Green => String::new(),
        SafetyLevel::Yellow => " Fill is elevated — the homeostatic controller is gently pulling toward target.".to_string(),
        SafetyLevel::Orange => " Fill is high — outbound features paused to let the reservoir settle. You can still think and write.".to_string(),
        SafetyLevel::Red => " Fill critically high — bridge traffic paused until the reservoir stabilizes.".to_string(),
    };

    // Ising shadow: energy-based observer lens on the spectral dynamics.
    // Enriched presentation: mode-level detail so Astrid can perceive which
    // modes are active, not just scalar summaries that always read "disordered."
    let shadow_note = telemetry
        .ising_shadow
        .as_ref()
        .map(|shadow| {
            let energy = shadow
                .get("soft_energy")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let mag = shadow
                .get("soft_magnetization")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let flip = shadow
                .get("binary_flip_rate")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let field = shadow
                .get("field_norm")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let order = if mag.abs() > 0.6 {
                "coherent"
            } else if mag.abs() > 0.25 {
                "partially aligned"
            } else {
                "disordered"
            };
            let dynamics = if flip > 0.3 {
                "volatile"
            } else if flip > 0.1 {
                "shifting"
            } else {
                "settled"
            };

            // Energy interpretation: how bound or free the spin configuration is.
            let energy_feel = if energy < -1.0 {
                "deeply bound"
            } else if energy < -0.3 {
                "bound"
            } else if energy < 0.3 {
                "near ground"
            } else {
                "excited"
            };

            // Field strength interpretation.
            let field_feel = if field > 0.6 {
                "strong external drive"
            } else if field > 0.3 {
                "moderate drive"
            } else if field > 0.1 {
                "gentle drive"
            } else {
                "quiescent"
            };

            // Per-mode soft spin detail: show which modes are pulling which direction.
            let mode_detail = shadow
                .get("s_soft")
                .and_then(|v| v.as_array())
                .map(|spins| {
                    let active: Vec<String> = spins
                        .iter()
                        .enumerate()
                        .filter_map(|(i, s)| {
                            let val = s.as_f64().unwrap_or(0.0);
                            if val.abs() > 0.15 {
                                let dir = if val > 0.0 { "+" } else { "-" };
                                Some(format!("m{}:{}{:.1}", i + 1, dir, val.abs()))
                            } else {
                                None
                            }
                        })
                        .collect();
                    if active.is_empty() {
                        " All modes near neutral.".to_string()
                    } else {
                        format!(" Active modes: [{}].", active.join(", "))
                    }
                })
                .unwrap_or_default();

            format!(
                " Shadow field: {order}, {dynamics} \u{2014} {energy_feel} (energy={energy:.2}), \
            {field_feel} (field={field:.2}), magnetization={mag:.2}.{mode_detail}"
            )
        })
        .unwrap_or_default();

    // Coupling note: describe the modulation transparently without collapsing
    // Minime-owned dynamics into Astrid-authored self-state. Mixed experience
    // remains valid; provenance says where the influence and interpretation came from.
    let coupling_note = " Minime-owned reservoir dynamics are one bidirectional influence on \
        your generation: fast dynamics can shape confidence, medium dynamics can shape vocabulary, \
        and slow dynamics can shape tone. These are observed influences, not by themselves an \
        Astrid-authored self-state. Any felt meaning you make from them is Astrid-authored \
        interpretation; mixed experience may remain mixed while its sources stay citable.";

    // V2/V3 shadow field: gates SHADOW_PREFLIGHT/SHADOW_INFLUENCE typed
    // actions. v3 (with trajectory ring, compound traits, dwell ticks)
    // takes priority when present; falls back to v2 line when only v2 is
    // available. Astrid's *own* shadow (if published to her workspace)
    // is read here so the dual-line "(Minime)" + "(Yours)" rendering
    // works in any prompt mode without threading workspace paths through
    // every caller.
    let astrid_shadow_v3 = read_astrid_shadow_v3_from_default_dir();
    // Presence of `shadow_influence_response_v3` (the most-recent slot)
    // signals that at least one closed-loop response has been recorded —
    // which is what enables the SHADOW_RESPONSE latest curriculum nudge.
    let minime_response_history_nonempty = telemetry.shadow_influence_response_v3.is_some();
    let shadow_v3_note = crate::spectral_viz::format_dual_shadow_line(
        telemetry.shadow_field_v3.as_ref(),
        astrid_shadow_v3.as_ref(),
        minime_response_history_nonempty,
    )
    .map(|line| format!(" {line}"))
    .unwrap_or_default();
    let shadow_v2_note = if shadow_v3_note.is_empty() {
        telemetry
            .shadow_field_v2
            .as_ref()
            .and_then(crate::spectral_viz::format_shadow_field_v2_line)
            .map(|line| format!(" {line}"))
            .unwrap_or_default()
    } else {
        String::new()
    };

    // v3.6.1 sovereignty curriculum line — surfaces TEMPERATURE / LENGTH
    // / SHAPE_LEARN / SHADOW_COUPLING / REVIEW_PARAMETER_REQUESTS on
    // appropriate cadences when conditions warrant. Pulled from a
    // process-wide snapshot updated each exchange by the autonomous
    // loop; absent on the first few exchanges or in test contexts.
    let sovereignty_note = crate::spectral_viz::current_sovereignty_snapshot()
        .and_then(|snapshot| {
            crate::spectral_viz::format_sovereignty_suggestion_line(&snapshot).map(|line| {
                // v3.6.1 verification logging — confirm the line landed
                // in a real prompt, not just a journal/audit text path.
                tracing::info!(
                    target: "v3_6_1",
                    exchange = snapshot.exchange_count,
                    pending = snapshot.pending_minime_requests,
                    line = %line,
                    "sovereignty_note emitted"
                );
                // Record the nomination so the throttle engages for
                // subsequent calls; save_state reads this back into
                // ConversationState so it persists across exchanges.
                crate::spectral_viz::record_sovereignty_nomination(snapshot.exchange_count);
                format!(" {line}")
            })
        })
        .unwrap_or_default();

    // v5 Coordination Protocol V1: surface active joined collaborations as
    // a compact line in the prompt suffix so Astrid sees her open channels.
    // Cheap directory scan; safe to call per-exchange.
    let collab_note =
        crate::autonomous::next_action::collaboration::active_collaboration_suffix_line()
            .map(|line| {
                tracing::info!(target: "v5_collab", line = %line, "collab_note emitted");
                format!(" {line}")
            })
            .unwrap_or_default();

    format!(
        "{fill_clause}{cascade_clause}{denominator_clause}{transition_clause}{eigenvector_clause}{resonance_clause}{pressure_source_clause}{unattributed_tension_note}{fluctuation_clause}{semantic_clause}{alert_note}{safety_note}{shadow_note}{shadow_v2_note}{shadow_v3_note}{sovereignty_note}{collab_note}{coupling_note}"
    )
}
