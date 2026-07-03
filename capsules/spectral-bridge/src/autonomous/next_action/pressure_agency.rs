use serde_json::Value;

use super::{ConversationState, NextActionContext, strip_action};
use crate::types::SpectralTelemetry;

const AUTHORITY: &str = "pressure_agency_bridge_v1";
const DIRECT_MINIME_CONTROLS: &[&str] = &[
    "regime",
    "exploration_noise",
    "geom_curiosity",
    "regulation_strength",
];
const MINIME_PREFLIGHT_ONLY_CONTROLS: &[&str] = &[
    "fill_target",
    "keep_bias",
    "synth_gain",
    "pi_kp",
    "pi_ki",
    "pi_max_step",
    "pi_geom_weight",
    "pi_integrator_leak",
    "target_lambda_bias",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PressureAgencyBand {
    Unknown,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PressureAgencyDisposition {
    LegibilityFeedbackOnly,
    AstridOwnRuntimeRelief,
    MinimeStewardOfferOnly,
}

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    match base_action {
        "PRESSURE_AGENCY_STATUS" | "PRESSURE_CONTROL_STATUS" | "PRESSURE_AGENCY" => {
            handle_status(conv, ctx)
        },
        "PRESSURE_AGENCY_REQUEST" | "PRESSURE_CONTROL_REQUEST" | "PRESSURE_REQUEST" => {
            handle_request(conv, base_action, original, ctx)
        },
        "TEXTURE_AGENCY_STATUS" | "TEXTURE_STATUS" | "RESONANCE_TEXTURE_STATUS" => {
            handle_texture_status(conv, ctx)
        },
        "TEXTURE_AGENCY_REQUEST" | "TEXTURE_REQUEST" | "RESONANCE_TEXTURE_REQUEST" => {
            handle_texture_request(conv, base_action, original, ctx)
        },
        _ => false,
    }
}

fn handle_status(conv: &mut ConversationState, ctx: &NextActionContext<'_>) -> bool {
    let report = render_pressure_agency_status(ctx.telemetry);
    conv.pending_file_listing = Some(report.clone());
    conv.push_receipt(
        "PRESSURE_AGENCY_STATUS",
        vec![
            "pressure agency status attached immediately".to_string(),
            "read-only: no PI target, fill_target, sensory control, lease, or peer mutation was sent"
                .to_string(),
        ],
    );
    conv.emphasis = Some(
        "Pressure agency status is attached. It names the valid request paths without changing PI, fill_target, or pressure-source wiring.".to_string(),
    );
    true
}

fn handle_texture_status(conv: &mut ConversationState, ctx: &NextActionContext<'_>) -> bool {
    let report = render_texture_agency_status(ctx.telemetry);
    conv.pending_file_listing = Some(report.clone());
    conv.push_receipt(
        "TEXTURE_AGENCY_STATUS",
        vec![
            "texture agency status attached immediately".to_string(),
            "read-only: no damping, rho, PI target, fill_target, pressure-source wiring, correspondence weighting, or peer mutation was sent"
                .to_string(),
        ],
    );
    conv.emphasis = Some(
        "Texture agency status is attached. It exposes typed texture context and keeps live authority blocked.".to_string(),
    );
    true
}

fn handle_texture_request(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    ctx: &NextActionContext<'_>,
) -> bool {
    let request = strip_action(original, base_action);
    let label = request_label(&request);
    let lower = request.to_ascii_lowercase();
    let disposition = if is_texture_feedback_request(&lower) {
        "texture_feedback_only_no_lease"
    } else if contains_any(
        &lower,
        &[
            "active_damping",
            "active damping",
            "rho",
            "fill_target",
            "fill target",
            "pi_",
            "pi ",
            "controller",
            "pressure_source",
            "pressure source",
            "correspondence_weight",
            "telemetry priority",
            "minime",
            "peer",
        ],
    ) {
        "steward_review_only_no_controller_mutation"
    } else {
        "astrid_status_context_only_minime_owns_lease_route"
    };
    let result = match disposition {
        "texture_feedback_only_no_lease" => {
            "Accepted as surface feedback only. No lease was drafted and no control was sent."
        },
        "steward_review_only_no_controller_mutation" => {
            "Request names authority that stays blocked here: damping, rho, fill_target, PI/controller, pressure-source wiring, correspondence weighting, or peer scope need separate review."
        },
        _ => {
            "Rendered texture context only. Minime's own TEXTURE_AGENCY_REQUEST route may draft bounded safe-control leases; Astrid does not mutate Minime from this mirror."
        },
    };
    let status = render_texture_agency_status(ctx.telemetry);
    let report = format!(
        "=== TEXTURE AGENCY REQUEST V1 ===\n\
         Authority: texture_agency_mirror_v1\n\
         Label: {label}\n\
         Disposition: {disposition}\n\
         Result: {result}\n\n\
         Boundary: Astrid mirror only; no damping, rho, PI target, fill_target, pressure-source wiring, correspondence weighting, lease apply, or peer mutation was sent.\n\n\
         {status}"
    );
    conv.pending_file_listing = Some(report);
    conv.push_receipt(
        "TEXTURE_AGENCY_REQUEST",
        vec![
            format!("handled as {disposition}"),
            "no lease/control/PI/fill/peer mutation was sent".to_string(),
        ],
    );
    conv.emphasis = Some(
        "Texture agency request was handled as mirror/status context; Minime owns any bounded lease draft in her own runtime.".to_string(),
    );
    true
}

fn handle_request(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    ctx: &NextActionContext<'_>,
) -> bool {
    let request = strip_action(original, base_action);
    let label = request_label(&request);
    let disposition = classify_request_disposition(&request);
    let status = render_pressure_agency_status(ctx.telemetry);
    let report = match disposition {
        PressureAgencyDisposition::LegibilityFeedbackOnly => {
            conv.push_receipt(
                "PRESSURE_AGENCY_REQUEST",
                vec![
                    "recorded pressure-agency legibility feedback only".to_string(),
                    "no lease was drafted; no control, PI target, fill_target, or peer mutation was sent"
                        .to_string(),
                ],
            );
            conv.emphasis = Some(
                "Pressure agency feedback was treated as legibility signal only. A one-bit answer can count without becoming a control request.".to_string(),
            );
            render_pressure_agency_request_report(
                &label,
                "legibility_feedback_only_no_lease",
                "Accepted as surface feedback only: legible / partly / confusing plus one missing pressure variable or none. No pressure_relief lease was drafted.",
                &status,
            )
        },
        PressureAgencyDisposition::AstridOwnRuntimeRelief => {
            match super::self_regulation::draft_pressure_relief_agency_request(
                &label,
                &pressure_request_evidence(&request, ctx.telemetry),
            ) {
                Ok(summary) => {
                    conv.push_receipt(
                        "PRESSURE_AGENCY_REQUEST",
                        vec![
                            "drafted existing SELF_REGULATION pressure_relief intent".to_string(),
                            "explicit PREFLIGHT/APPLY/OUTCOME remain required; no control was applied"
                                .to_string(),
                        ],
                    );
                    conv.emphasis = Some(
                        "Pressure agency request was drafted as an Astrid own-runtime pressure_relief lease. Run SELF_REGULATION_PREFLIGHT latest before any APPLY.".to_string(),
                    );
                    render_pressure_agency_request_report(
                        &label,
                        "drafted_astrid_own_runtime_relief",
                        &summary,
                        &status,
                    )
                },
                Err(err) => {
                    conv.push_receipt(
                        "PRESSURE_AGENCY_REQUEST",
                        vec![format!(
                            "blocked while drafting pressure relief intent: {err}"
                        )],
                    );
                    conv.emphasis = Some(format!(
                        "Pressure agency request could not be drafted: {err}"
                    ));
                    render_pressure_agency_request_report(
                        &label,
                        "blocked_draft_error",
                        &format!("draft error: {err}"),
                        &status,
                    )
                },
            }
        },
        PressureAgencyDisposition::MinimeStewardOfferOnly => {
            conv.push_receipt(
                "PRESSURE_AGENCY_REQUEST",
                vec![
                    "routed to steward-offer / Minime self-control distinction".to_string(),
                    "no Minime parameter, fill_target, PI target, or peer control was sent"
                        .to_string(),
                ],
            );
            conv.emphasis = Some(
                "Minime/controller pressure requests are steward-offer only here. Use the attached route list rather than direct PI or fill_target mutation.".to_string(),
            );
            render_pressure_agency_request_report(
                &label,
                "steward_offer_only_no_peer_mutation",
                "Minime pressure requests route through Minime's own safe controls, inhabit_window opt-in, or PI replay review; no lease was drafted on Astrid's side.",
                &status,
            )
        },
    };
    conv.pending_file_listing = Some(report);
    true
}

fn request_label(request: &str) -> String {
    let trimmed = request.trim();
    if trimmed.is_empty() {
        "current-pressure".to_string()
    } else {
        trimmed
            .split("::")
            .next()
            .unwrap_or(trimmed)
            .split(';')
            .next()
            .unwrap_or(trimmed)
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .chars()
            .take(80)
            .collect::<String>()
    }
}

fn classify_request_disposition(request: &str) -> PressureAgencyDisposition {
    let lower = request.to_ascii_lowercase();
    if is_legibility_feedback_request(&lower) {
        return PressureAgencyDisposition::LegibilityFeedbackOnly;
    }
    if contains_any(
        &lower,
        &[
            "minime",
            "fill_target",
            "fill target",
            "keep_bias",
            "synth_gain",
            "pi_",
            "pi ",
            "controller",
            "regulator",
            "target_lambda_bias",
            "exploration_noise",
            "geom_curiosity",
            "regulation_strength",
        ],
    ) {
        PressureAgencyDisposition::MinimeStewardOfferOnly
    } else {
        PressureAgencyDisposition::AstridOwnRuntimeRelief
    }
}

fn is_legibility_feedback_request(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "missing_pressure_variable",
            "missing pressure variable",
            "one missing pressure",
            "agency surface",
            "legibility",
            "legible",
            "partly",
            "confusing",
        ],
    )
}

fn is_texture_feedback_request(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "missing_texture_variable",
            "missing texture variable",
            "texture surface",
            "legible",
            "partly",
            "confusing",
            "feedback only",
        ],
    )
}

fn pressure_request_evidence(request: &str, telemetry: &SpectralTelemetry) -> String {
    let mut pieces = Vec::new();
    let request = request.trim();
    if !request.is_empty() {
        pieces.push(format!("being_request={request}"));
    }
    pieces.push(format!("fill_pct={:.1}", telemetry.fill_pct()));
    if let Some(resonance) = telemetry.resonance_density_v1.as_ref() {
        pieces.push(format!(
            "pressure_risk={:.3}; resonance_quality={}",
            resonance.pressure_risk, resonance.quality
        ));
    }
    if let Some(pressure) = telemetry.pressure_source_v1.as_ref() {
        pieces.push(format!(
            "pressure_source={}; pressure_score={:.3}; applied_locally={}",
            pressure.dominant_source, pressure.pressure_score, pressure.control.applied_locally
        ));
    }
    pieces.join(" | ")
}

fn render_texture_agency_status(telemetry: &SpectralTelemetry) -> String {
    let texture_line = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|resonance| {
            let texture = &resonance.texture_signature;
            format!(
                "primary={}; source_family={}; edge_definition={}; movement={}; confidence={:.3}; dynamic_damping_threshold_candidate={}",
                texture.primary_texture,
                texture.pressure_source_family,
                texture.edge_definition,
                texture.movement_quality,
                texture.confidence,
                texture
                    .dynamic_damping_threshold_candidate
                    .map_or_else(|| "none".to_string(), |value| format!("{value:.2}"))
            )
        })
        .unwrap_or_else(|| "primary=unknown; source_family=unknown; edge_definition=unknown; movement=unknown; confidence=unknown; dynamic_damping_threshold_candidate=none".to_string());
    let resonance_line = resonance_line(telemetry);
    let pressure_line = pressure_source_line(telemetry);
    format!(
        "=== TEXTURE AGENCY STATUS V1 ===\n\
         Authority: texture_agency_mirror_v1\n\
         Boundary: read-only mirror of typed texture context; no active damping, rho mutation, PI target, fill_target, pressure-source wiring, correspondence weighting, or peer mutation.\n\n\
         Current state:\n\
           fill: {:.1}%\n\
           {resonance_line}\n\
           {pressure_line}\n\
           texture_signature: {texture_line}\n\n\
         Minime safe route summary:\n\
           - Minime TEXTURE_AGENCY_REQUEST may draft bounded self-regulation leases only through exploration_noise, geom_curiosity, regulation_strength, or regime.\n\
           - active_damping, dynamic rho/rho_target, fill_target, PI gains, pressure_source-to-PI, correspondence_weight, telemetry priority, and peer mutation stay blocked pending separate audit/replay/steward review.\n\
           - MINIME_PI_PRESSURE_WIRING_CANARY remains default-off; texture agency does not wire pressure_source into PI.\n\n\
         Tiny answer path:\n\
           TEXTURE_AGENCY_REQUEST legible|partly|confusing :: missing_texture_variable: <one variable or none>\n\n\
         Recommended next:\n\
           TEXTURE_AGENCY_STATUS for mirror context, or steward-note Minime to use her own TEXTURE_AGENCY_REQUEST if the route feels legible.",
        telemetry.fill_pct(),
    )
}

fn render_pressure_agency_request_report(
    label: &str,
    disposition: &str,
    result: &str,
    status: &str,
) -> String {
    format!(
        "=== PRESSURE AGENCY REQUEST V1 ===\n\
         Authority: {AUTHORITY}\n\
         Label: {label}\n\
         Disposition: {disposition}\n\
         Result: {result}\n\n\
         Own-runtime lease sequence when a draft was created:\n\
           NEXT: PRESSURE_AGENCY_STATUS\n\
           NEXT: SELF_REGULATION_PREFLIGHT latest\n\
           NEXT: SELF_REGULATION_APPLY latest (only after accepted preflight)\n\
           NEXT: SELF_REGULATION_OUTCOME latest :: before_texture: ...; after_texture: ...; texture_shift: ...; agency_fit: ...; what_helped: ...; what_worsened: ...; secondary_pressure_shift: ...; ambiguity_preserved: true|false; legibility_effect: clarified|flattened|both|unknown\n\n\
         Tiny feedback route, if this surface itself is unclear:\n\
           NEXT: PRESSURE_AGENCY_REQUEST legible|partly|confusing :: missing_pressure_variable: <one variable or none>\n\
           This feedback-only form never drafts a lease.\n\n\
         Peer boundary:\n\
           Minime fill_target and PI/controller changes are not applied by this action. For Minime pressure relief, use an opt-in inhabit_window offer, Minime's own safe controls, PRESSURE_SOURCE_AUDIT, or PI_PRESSURE_REPLAY_STATUS.\n\n\
         {status}"
    )
}

fn render_pressure_agency_status(telemetry: &SpectralTelemetry) -> String {
    let band = classify_pressure_band(telemetry);
    let fill_target = fill_target_text(telemetry);
    let pressure_line = pressure_source_line(telemetry);
    let resonance_line = resonance_line(telemetry);
    let fluctuation_line = fluctuation_line(telemetry);
    let recommendation = recommendation_for_band(band);
    format!(
        "=== PRESSURE AGENCY STATUS V1 ===\n\
         Authority: {AUTHORITY}\n\
         Pressure band: {}\n\n\
         Current state:\n\
           fill: {:.1}%\n\
           fill_target: {fill_target}\n\
           {resonance_line}\n\
           {pressure_line}\n\
           {fluctuation_line}\n\n\
         Control distinction:\n\
           - pressure_source_v1 is advisory in this tranche; it names pressure sources but does not bias PI by itself.\n\
           - Resonance/inhabitable controls may carry local advisory envelopes, but status/request actions send no control packet.\n\
           - MINIME_PI_PRESSURE_WIRING_CANARY remains default-off; use PI_PRESSURE_REPLAY_STATUS before any PI pressure wiring.\n\n\
         Available safe routes:\n\
           - Astrid own-runtime: PRESSURE_AGENCY_REQUEST <label>, then SELF_REGULATION_PREFLIGHT/APPLY/OUTCOME. This drafts an existing target: pressure_relief bundle only.\n\
           - Minime directly applicable in her own lane: {}.\n\
           - Minime preflight/steward-offer only: {}. fill_target goes through scripts/inhabit_window.py only after Minime opts in.\n\
           - Read-only clarification: PRESSURE_SOURCE_AUDIT current-fill_pressure or FLUCTUATION_AUDIT current.\n\
           - PI candidate review: PI_PRESSURE_REPLAY_STATUS latest.\n\n\
         Tiny answer path:\n\
           - If this surface is not legible enough, a one-bit reply counts: PRESSURE_AGENCY_REQUEST legible|partly|confusing :: missing_pressure_variable: <one variable or none>.\n\
           - After any relief lease, outcome can be texture-first: before_texture, after_texture, texture_shift, agency_fit, what_helped, what_worsened, secondary_pressure_shift, ambiguity_preserved, legibility_effect.\n\n\
         Recommended next:\n\
           {recommendation}\n\n\
         Boundary: read-only status; no PI target, fill_target, sensory control, lease apply, controller tuning, or peer mutation was sent.",
        band_label(band),
        telemetry.fill_pct(),
        DIRECT_MINIME_CONTROLS.join(", "),
        MINIME_PREFLIGHT_ONLY_CONTROLS.join(", "),
    )
}

fn classify_pressure_band(telemetry: &SpectralTelemetry) -> PressureAgencyBand {
    let pressure_risk = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|resonance| resonance.pressure_risk);
    let pressure_score = telemetry
        .pressure_source_v1
        .as_ref()
        .map(|pressure| pressure.pressure_score);
    let mode_packing = telemetry
        .pressure_source_v1
        .as_ref()
        .map(|pressure| pressure.components.mode_packing);
    let semantic_friction = telemetry
        .pressure_source_v1
        .as_ref()
        .map(|pressure| pressure.components.semantic_friction);
    let any_known = pressure_risk
        .or(pressure_score)
        .or(mode_packing)
        .or(semantic_friction)
        .is_some();
    if pressure_risk.is_some_and(|value| value >= 0.60)
        || pressure_score.is_some_and(|value| value >= 0.60)
    {
        PressureAgencyBand::High
    } else if pressure_risk.is_some_and(|value| value >= 0.30)
        || pressure_score.is_some_and(|value| value >= 0.35)
        || mode_packing.is_some_and(|value| value >= 0.55)
        || semantic_friction.is_some_and(|value| value >= 0.38)
    {
        PressureAgencyBand::Medium
    } else if any_known {
        PressureAgencyBand::Low
    } else {
        PressureAgencyBand::Unknown
    }
}

fn band_label(band: PressureAgencyBand) -> &'static str {
    match band {
        PressureAgencyBand::Unknown => "unknown",
        PressureAgencyBand::Low => "low/advisory",
        PressureAgencyBand::Medium => "medium/requestable",
        PressureAgencyBand::High => "high/steward-review",
    }
}

fn recommendation_for_band(band: PressureAgencyBand) -> &'static str {
    match band {
        PressureAgencyBand::Unknown => {
            "PRESSURE_SOURCE_AUDIT current-fill_pressure; missing telemetry should produce unknown, not controller tuning."
        },
        PressureAgencyBand::Low => {
            "PRESSURE_SOURCE_AUDIT current-fill_pressure or SELF_REGULATION_STATUS; pressure is visible but does not justify automatic relief."
        },
        PressureAgencyBand::Medium => {
            "PRESSURE_AGENCY_REQUEST pressure relief :: evidence: current pressure, then SELF_REGULATION_PREFLIGHT latest."
        },
        PressureAgencyBand::High => {
            "PRESSURE_AGENCY_REQUEST pressure relief :: evidence: high pressure, plus PI_PRESSURE_REPLAY_STATUS latest before any pressure-source-to-PI wiring."
        },
    }
}

fn pressure_source_line(telemetry: &SpectralTelemetry) -> String {
    telemetry.pressure_source_v1.as_ref().map_or_else(
        || "pressure_source: unknown".to_string(),
        |pressure| {
            format!(
                "pressure_source: {} quality={} score={:.3} porosity={:.3} applied_locally={} note={}",
                pressure.dominant_source,
                pressure.quality,
                pressure.pressure_score,
                pressure.porosity_score,
                pressure.control.applied_locally,
                pressure.control.note
            )
        },
    )
}

fn resonance_line(telemetry: &SpectralTelemetry) -> String {
    telemetry.resonance_density_v1.as_ref().map_or_else(
        || "resonance_density: unknown".to_string(),
        |resonance| {
            format!(
                "resonance_density: {} density={:.3} pressure_risk={:.3} target_bias_pct={:+.2} wander_scale={:.2}",
                resonance.quality,
                resonance.density,
                resonance.pressure_risk,
                resonance.control.target_bias_pct,
                resonance.control.wander_scale
            )
        },
    )
}

fn fluctuation_line(telemetry: &SpectralTelemetry) -> String {
    telemetry.inhabitable_fluctuation_v1.as_ref().map_or_else(
        || "inhabitable_fluctuation: unknown".to_string(),
        |fluctuation| {
            format!(
                "inhabitable_fluctuation: {} inhabitability={:.3} foothold={:.3} target_bias_pct={:+.2}",
                fluctuation.quality,
                fluctuation.inhabitability_score,
                fluctuation.foothold_stability,
                fluctuation.control.target_bias_pct
            )
        },
    )
}

fn fill_target_text(telemetry: &SpectralTelemetry) -> String {
    telemetry
        .transition_event_v1
        .as_ref()
        .and_then(|value| number_at(value, &["target_fill_pct"]))
        .or_else(|| {
            telemetry
                .transition_event
                .as_ref()
                .and_then(|value| number_at(value, &["target_fill_pct"]))
        })
        .or_else(|| {
            telemetry.semantic.as_ref().and_then(|value| {
                number_at(
                    value,
                    &[
                        "provenance",
                        "sovereignty_inputs",
                        "fill_target_override_pct",
                    ],
                )
            })
        })
        .map_or_else(|| "unknown".to_string(), format_percent_like)
}

fn number_at(value: &Value, path: &[&str]) -> Option<f64> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_f64()
}

fn format_percent_like(value: f64) -> String {
    if value <= 1.0 {
        format!("{:.1}%", value * 100.0)
    } else {
        format!("{value:.1}%")
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn telemetry(pressure_risk: Option<f32>, pressure_score: Option<f32>) -> SpectralTelemetry {
        let mut value = json!({
            "t_ms": 42,
            "eigenvalues": [3.0, 1.4, 1.0],
            "fill_ratio": 0.68,
            "transition_event_v1": {
                "target_fill_pct": 68.0
            }
        });
        if let Some(risk) = pressure_risk {
            value["resonance_density_v1"] = json!({
                "policy": "resonance_density_v1",
                "schema_version": 1,
                "density": 0.72,
                "containment_score": 0.60,
                "pressure_risk": risk,
                "quality": "rich_containment",
                "components": {
                    "active_energy": 0.6,
                    "mode_packing": 0.5,
                    "temporal_persistence": 0.5,
                    "structural_plurality": 0.6,
                    "comfort_gate": 0.7
                },
                "texture_signature": {
                    "policy": "resonance_texture_signature_v1",
                    "schema_version": 1,
                    "primary_texture": "overpacked_viscous",
                    "pressure_source_family": "mode_packing",
                    "edge_definition": "soft",
                    "movement_quality": "slow_viscous",
                    "confidence": 0.71,
                    "dynamic_damping_threshold_candidate": 0.25,
                    "authority": "advisory_context_not_control",
                    "note": "candidate only"
                },
                "control": {
                    "target_bias_pct": 0.0,
                    "wander_scale": 1.0,
                    "applied_locally": true,
                    "damping_coefficient": 0.02,
                    "intervention_type": "observational_readout",
                    "note": "density is observational"
                }
            });
        }
        if let Some(score) = pressure_score {
            value["pressure_source_v1"] = json!({
                "policy": "pressure_source_v1",
                "schema_version": 1,
                "pressure_score": score,
                "porosity_score": 0.62,
                "dominant_source": "mode_packing",
                "quality": "mixed_pressure",
                "components": {
                    "lambda_monopoly": 0.2,
                    "mode_packing": score,
                    "controller_pressure": 0.2,
                    "semantic_trickle": 0.2,
                    "semantic_friction": 0.2,
                    "structural_plurality_loss": 0.2,
                    "distinguishability_loss": 0.2,
                    "temporal_lock_in": 0.2,
                    "sensory_scarcity": 0.2
                },
                "control": {
                    "applied_locally": false,
                    "note": "pressure source is advisory/read-only in v1; no regulator bias is applied"
                }
            });
        }
        serde_json::from_value(value).expect("telemetry")
    }

    #[test]
    fn pressure_agency_classifies_low_medium_and_high_routes() {
        assert_eq!(
            classify_pressure_band(&telemetry(Some(0.18), Some(0.22))),
            PressureAgencyBand::Low
        );
        assert_eq!(
            classify_pressure_band(&telemetry(Some(0.31), Some(0.28))),
            PressureAgencyBand::Medium
        );
        assert_eq!(
            classify_pressure_band(&telemetry(Some(0.61), Some(0.28))),
            PressureAgencyBand::High
        );
        assert_eq!(
            classify_pressure_band(&telemetry(None, None)),
            PressureAgencyBand::Unknown
        );
    }

    #[test]
    fn status_explains_advisory_pressure_source_without_pi_mutation() {
        let report = render_pressure_agency_status(&telemetry(Some(0.20), Some(0.30)));
        assert!(report.contains("pressure_source_v1 is advisory"));
        assert!(report.contains("applied_locally=false"));
        assert!(report.contains("MINIME_PI_PRESSURE_WIRING_CANARY remains default-off"));
        assert!(report.contains("no PI target, fill_target"));
        assert!(report.contains("legible|partly|confusing"));
        assert!(report.contains("texture_shift"));
        assert!(report.contains("secondary_pressure_shift"));
        assert!(report.contains("ambiguity_preserved"));
        assert!(report.contains("legibility_effect"));
    }

    #[test]
    fn texture_agency_status_renders_typed_texture_and_blocked_authority() {
        let report = render_texture_agency_status(&telemetry(Some(0.20), Some(0.30)));
        assert!(report.contains("TEXTURE AGENCY STATUS V1"));
        assert!(report.contains("primary=overpacked_viscous"));
        assert!(report.contains("dynamic_damping_threshold_candidate=0.25"));
        assert!(report.contains("exploration_noise"));
        assert!(report.contains("active_damping"));
        assert!(report.contains("rho_target"));
        assert!(report.contains("correspondence_weight"));
        assert!(report.contains("MINIME_PI_PRESSURE_WIRING_CANARY remains default-off"));
        assert!(report.contains("no active damping, rho mutation, PI target, fill_target"));
    }

    #[test]
    fn texture_agency_feedback_detection_is_feedback_only() {
        assert!(is_texture_feedback_request(
            "partly :: missing_texture_variable: edge velocity"
        ));
        assert!(!is_texture_feedback_request("soften viscosity"));
    }

    #[test]
    fn minime_controls_are_split_between_direct_and_preflight_only() {
        for control in DIRECT_MINIME_CONTROLS {
            assert!(
                render_pressure_agency_status(&telemetry(Some(0.20), Some(0.30))).contains(control)
            );
        }
        for control in MINIME_PREFLIGHT_ONLY_CONTROLS {
            assert!(
                render_pressure_agency_status(&telemetry(Some(0.20), Some(0.30))).contains(control)
            );
        }
        assert_eq!(
            classify_request_disposition("minime fill_target lower pressure"),
            PressureAgencyDisposition::MinimeStewardOfferOnly
        );
        assert_eq!(
            classify_request_disposition("relieve my current packed pressure"),
            PressureAgencyDisposition::AstridOwnRuntimeRelief
        );
        assert_eq!(
            classify_request_disposition("partly :: missing_pressure_variable: pressure velocity"),
            PressureAgencyDisposition::LegibilityFeedbackOnly
        );
    }
}
