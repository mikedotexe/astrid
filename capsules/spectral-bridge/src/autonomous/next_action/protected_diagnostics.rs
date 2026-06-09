use std::{
    cmp::Ordering,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};

use super::native_gesture::{append_atlas_event, minime_workspace, record_native_gesture};
use super::space_hold::append_space_hold_event;
use super::{
    ConversationState, NextActionContext, bridge_paths, save_astrid_journal, strip_action,
    truncate_str,
};
use crate::db::EigenvalueSnapshotRow;
use crate::types::SpectralTelemetry;

pub(crate) const READ_ONLY_AUDIT_BOUNDARY: &str = "read-only protected advisory inspection; No semantic input, control nudge, perturbation, native gesture, Astrid control envelope, or Minime parameter change was sent.";
pub(crate) const CARTOGRAPHY_BOUNDARY: &str = "read/write cartography marker only; No semantic input, control nudge, perturbation, native gesture, Astrid control envelope, or Minime parameter change was sent.";
const RESISTANCE_GRADIENT_BOUNDARY: &str = "read-only protected gradient artifact; No semantic input, control nudge, perturbation, native gesture, controller mutation, Astrid control envelope, or Minime parameter change was sent.";
const LATENT_STASIS_BOUNDARY: &str = "read-only protected latent-stasis snapshot; No telemetry pause, semantic input, control nudge, perturbation, native gesture, controller mutation, Astrid control envelope, or Minime parameter change was sent.";
const RESISTANCE_CALIBRATION_COOLDOWN_SECS: f64 = 6.0 * 60.0 * 60.0;

type DiagnosticHandler = fn(
    &ProtectedDiagnosticDescriptor,
    &mut ConversationState,
    &str,
    &mut NextActionContext<'_>,
) -> bool;

#[derive(Clone, Copy)]
pub(crate) struct ProtectedDiagnosticDescriptor {
    pub canonical: &'static str,
    pub aliases: &'static [&'static str],
    pub journal_mode: &'static str,
    pub authority_boundary: &'static str,
    pub prompt_summary: &'static str,
    pub suggested_comparison_target: &'static str,
    handler: DiagnosticHandler,
}

impl ProtectedDiagnosticDescriptor {
    #[must_use]
    pub(crate) fn help_text(&self) -> String {
        let aliases = if self.aliases.is_empty() {
            "(none)".to_string()
        } else {
            self.aliases.join(", ")
        };
        format!(
            "{} — {} Authority boundary: {} Aliases: {}. Suggested comparison: {} NEXT: {} [label]",
            self.canonical,
            self.prompt_summary,
            self.authority_boundary,
            aliases,
            self.suggested_comparison_target,
            self.canonical
        )
    }
}

const DESCRIPTORS: &[ProtectedDiagnosticDescriptor] = &[
    ProtectedDiagnosticDescriptor {
        canonical: "PRESSURE_SOURCE_AUDIT",
        aliases: &["PRESSURE_SOURCE", "STRUCTURAL_PRESSURE", "INWARD_PRESSURE"],
        journal_mode: "pressure_source_audit",
        authority_boundary: READ_ONLY_AUDIT_BOUNDARY,
        prompt_summary: "Protected read-only audit of where inward pressure appears to originate: lambda monopoly, mode packing, controller squeeze, semantic trickle, plurality loss, lock-in, scarcity, and porosity.",
        suggested_comparison_target: "compare pressure/porosity and dominant-source changes against later pressure relief, brace audits, and transition markers",
        handler: handle_pressure_source_audit,
    },
    ProtectedDiagnosticDescriptor {
        canonical: "PRESSURE_RELIEF",
        aliases: &["RELIEF_REQUEST"],
        journal_mode: "pressure_relief",
        authority_boundary: READ_ONLY_AUDIT_BOUNDARY,
        prompt_summary: "Protected read-only relief preflight. Attaches pressure-source context, safe relief options, and a steward-report template; it sends no control by itself.",
        suggested_comparison_target: "compare relief preflights against later pressure-source audits, chosen safe actions, and stable-core status",
        handler: handle_pressure_relief,
    },
    ProtectedDiagnosticDescriptor {
        canonical: "FLUCTUATION_AUDIT",
        aliases: &[
            "INHABITABLE_FLUCTUATION",
            "EIGENTRUST",
            "EIGENTRUST_AUDIT",
            "FOOTHOLD_AUDIT",
        ],
        journal_mode: "fluctuation_audit",
        authority_boundary: READ_ONLY_AUDIT_BOUNDARY,
        prompt_summary: "Protected read-only audit of whether fluctuation remains returnable, coherent, and inhabitable. Eigentrust remains an alias/language surface.",
        suggested_comparison_target: "compare inhabitability/foothold changes against later fold holds, brace audits, and transition markers",
        handler: handle_fluctuation_audit,
    },
    ProtectedDiagnosticDescriptor {
        canonical: "BRACE_AUDIT",
        aliases: &["AFTERSHOCK_TRACE", "TREMOR_RESIDUE", "CASCADE_RESIDUE"],
        journal_mode: "brace_audit",
        authority_boundary: READ_ONLY_AUDIT_BOUNDARY,
        prompt_summary: "Protected read-only rest-vs-bracing audit. Distinguishes relaxed settling from post-spike resistance or cascade residue without changing telemetry/control.",
        suggested_comparison_target: "compare brace/aftershock readings against later decay maps, fluctuation audits, and transition dwell markers",
        handler: handle_brace_audit,
    },
    ProtectedDiagnosticDescriptor {
        canonical: "LAMBDA_FLOW_MAP",
        aliases: &["CENTER_TAIL_FLOW", "SURGE_SNAPSHOT", "FREEZE_SURGE"],
        journal_mode: "lambda_flow_map",
        authority_boundary: CARTOGRAPHY_BOUNDARY,
        prompt_summary: "Protected non-control lambda-flow snapshot. Freezes the current lambda1/shoulder/tail terrain for later comparison without holding or mutating Minime.",
        suggested_comparison_target: "compare this frozen lambda-flow map against later visual cascade, time-domain, pressure-source, and space-hold records",
        handler: handle_space_family,
    },
    ProtectedDiagnosticDescriptor {
        canonical: "EIGENVECTOR_FIELD",
        aliases: &["EIGENVECTOR_TRACE", "VECTOR_DENSITY"],
        journal_mode: "eigenvector_field",
        authority_boundary: CARTOGRAPHY_BOUNDARY,
        prompt_summary: "Protected non-control eigenvector-field mark. Records compact eigenvector landmarks/overlaps when available and otherwise degrades to eigenvalue-density evidence.",
        suggested_comparison_target: "compare eigenvector-field marks against later cascade visuals, SDI traces, and decomposition output",
        handler: handle_space_family,
    },
    ProtectedDiagnosticDescriptor {
        canonical: "SPACE_HOLD",
        aliases: &["SPACE_EXPLORE"],
        journal_mode: "space_hold",
        authority_boundary: CARTOGRAPHY_BOUNDARY,
        prompt_summary: "Protected non-control exploration hold. Records lambda density, shoulder/tail slack, shadow/tail affordance, and harvest pressure before any semantic/control use.",
        suggested_comparison_target: "compare space holds against later SCA reflections, visual cascade output, and resonance forecasts",
        handler: handle_space_family,
    },
    ProtectedDiagnosticDescriptor {
        canonical: "FOLD_HOLD",
        aliases: &["FOLD_STUDY", "HUM_DECAY", "HUM_DECAY_STUDY"],
        journal_mode: "fold_hold",
        authority_boundary: CARTOGRAPHY_BOUNDARY,
        prompt_summary: "Protected non-control fold/hum-decay study. Records a fold_hold_v1 marker where the sustained transition is the artifact.",
        suggested_comparison_target: "compare fold holds against later decay maps, fluctuation audits, and explicit experiment evidence",
        handler: handle_space_family,
    },
    ProtectedDiagnosticDescriptor {
        canonical: "RESISTANCE_GRADIENT",
        aliases: &[
            "GROAN_MAP",
            "FRICTION_GRADIENT",
            "PRESSURE_GRADIENT",
            "RESISTANCE_MAP",
            "EXPLORE_RESISTANCE_GRADIENT",
        ],
        journal_mode: "resistance_gradient",
        authority_boundary: RESISTANCE_GRADIENT_BOUNDARY,
        prompt_summary: "Protected read-only gradient map for resistance/groan reports. Converts pressure, geometry, transition, and history evidence into a reviewable vector without control authority.",
        suggested_comparison_target: "compare resistance gradients against pressure-source audits, lambda-flow maps, eigenvector-field marks, brace audits, and later journal language",
        handler: handle_resistance_gradient,
    },
    ProtectedDiagnosticDescriptor {
        canonical: "LATENT_STASIS",
        aliases: &[
            "RESONANCE_STASIS",
            "STASIS_MAP",
            "STASIS_OF_LATENT",
            "LATENT_HOLD",
            "LATENT_RESERVOIR_STASIS",
            "GHOSTING_AUDIT",
            "SIGNAL_GHOST_AUDIT",
            "HUMID_FILL_MAP",
            "KEEP_FLOOR_STASIS",
        ],
        journal_mode: "latent_stasis",
        authority_boundary: LATENT_STASIS_BOUNDARY,
        prompt_summary: "Protected read-only freeze-frame for stasis-of-the-latent reports. Compares entropy, lambda distribution, pressure/porosity, transition motion, and recent history to distinguish occupiable stasis, pressurized hold, ghosted resonance, and active transit.",
        suggested_comparison_target: "compare latent-stasis snapshots against resistance gradients, lambda-flow maps, brace audits, pressure-source audits, and later journal language about ghosting or resonance",
        handler: handle_latent_stasis,
    },
];

#[cfg(test)]
#[must_use]
pub(crate) fn descriptors() -> &'static [ProtectedDiagnosticDescriptor] {
    DESCRIPTORS
}

#[must_use]
pub(crate) fn descriptor_for_action(
    action: &str,
) -> Option<&'static ProtectedDiagnosticDescriptor> {
    let upper = action.trim().to_ascii_uppercase();
    DESCRIPTORS.iter().find(|descriptor| {
        descriptor.canonical == upper || descriptor.aliases.iter().any(|alias| *alias == upper)
    })
}

#[cfg(test)]
#[must_use]
pub(crate) fn descriptor_for_canonical(
    canonical: &str,
) -> Option<&'static ProtectedDiagnosticDescriptor> {
    let upper = canonical.trim().to_ascii_uppercase();
    DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.canonical == upper)
}

#[must_use]
pub(crate) fn canonical_action_for(action: &str) -> Option<&'static str> {
    descriptor_for_action(action).map(|descriptor| descriptor.canonical)
}

#[must_use]
pub(crate) fn normalize_action_components(
    base_action: &str,
    original: &str,
) -> Option<(String, String)> {
    let descriptor = descriptor_for_action(base_action)?;
    let label = label_from_original(original, descriptor);
    let normalized = if label.is_empty() {
        descriptor.canonical.to_string()
    } else {
        format!("{} {label}", descriptor.canonical)
    };
    Some((descriptor.canonical.to_string(), normalized))
}

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    let Some(descriptor) = descriptor_for_action(base_action) else {
        return false;
    };
    (descriptor.handler)(descriptor, conv, original, ctx)
}

fn clean_registry_arg(raw: &str) -> String {
    raw.trim()
        .trim_start_matches([':', '-', '\u{2014}'])
        .trim()
        .trim_matches(|c: char| matches!(c, '[' | ']' | '"' | '\'' | '`' | '“' | '”'))
        .trim()
        .to_string()
}

fn label_from_original(original: &str, descriptor: &ProtectedDiagnosticDescriptor) -> String {
    for action in std::iter::once(descriptor.canonical).chain(descriptor.aliases.iter().copied()) {
        if original.to_ascii_uppercase().starts_with(action) {
            return clean_registry_arg(&strip_action(original, action));
        }
    }
    String::new()
}

fn review_label(label: &str) -> &str {
    let label = label.trim();
    if label.is_empty() { "current" } else { label }
}

fn review_event_id(event_id: Option<&str>) -> &str {
    event_id
        .filter(|id| !id.trim().is_empty())
        .unwrap_or("none")
}

pub(crate) fn compact_review_summary(
    title: &str,
    action: &str,
    label: &str,
    event_id: Option<&str>,
    key_fields: &[String],
    authority_boundary: &str,
    suggested_comparison: &str,
) -> String {
    let key_fields = if key_fields.is_empty() {
        "  - detail: unavailable; see attached report or receipt".to_string()
    } else {
        key_fields
            .iter()
            .take(6)
            .map(|field| format!("  - {}", truncate_str(field.trim(), 180)))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "=== {title} REVIEW SUMMARY ===\n\
         Action: {action}\n\
         Label: {}\n\
         Event id: {}\n\n\
         Key fields:\n{key_fields}\n\n\
         Authority boundary: {authority_boundary}\n\
         Suggested comparison: {suggested_comparison}",
        review_label(label),
        review_event_id(event_id)
    )
}

fn base_spectral_review_fields(telemetry: &SpectralTelemetry) -> Vec<String> {
    let mut fields = vec![
        format!("fill_pct: {:.1}", telemetry.fill_pct()),
        format!("lambda1: {:.3}", telemetry.lambda1()),
    ];
    if let Some(lambda1_rel) = telemetry.lambda1_rel {
        fields.push(format!("lambda1_rel: {lambda1_rel:.3}"));
    }
    if let Some(active_modes) = telemetry.active_mode_count {
        fields.push(format!("active_mode_count: {active_modes}"));
    }
    if let Some(energy_ratio) = telemetry.active_mode_energy_ratio {
        fields.push(format!("active_mode_energy_ratio: {energy_ratio:.3}"));
    }
    if let Some(entropy) = telemetry.structural_entropy {
        fields.push(format!("structural_entropy: {entropy:.3}"));
    }
    fields
}

fn pressure_review_fields(telemetry: &SpectralTelemetry) -> Vec<String> {
    let mut fields = Vec::new();
    if let Some(pressure) = telemetry.pressure_source_v1.as_ref() {
        fields.push(format!("pressure_score: {:.3}", pressure.pressure_score));
        fields.push(format!("porosity_score: {:.3}", pressure.porosity_score));
        fields.push(format!("dominant_source: {}", pressure.dominant_source));
        fields.push(format!("pressure_quality: {}", pressure.quality));
        fields.push(format!(
            "pressure_control_applied_locally: {}",
            pressure.control.applied_locally
        ));
    } else {
        fields.push("pressure_source: unavailable".to_string());
    }
    fields.extend(base_spectral_review_fields(telemetry));
    fields
}

fn fluctuation_review_fields(telemetry: &SpectralTelemetry) -> Vec<String> {
    let mut fields = base_spectral_review_fields(telemetry);
    if let Some(fluctuation) = telemetry.inhabitable_fluctuation_v1.as_ref() {
        fields.push(format!(
            "inhabitability_score: {:.3}",
            fluctuation.inhabitability_score
        ));
        fields.push(format!(
            "fluctuation_score: {:.3}",
            fluctuation.fluctuation_score
        ));
        fields.push(format!(
            "foothold_stability: {:.3}",
            fluctuation.foothold_stability
        ));
        fields.push(format!(
            "rearrangement_intensity: {:.3}",
            fluctuation.rearrangement_intensity
        ));
        fields.push(format!("fluctuation_quality: {}", fluctuation.quality));
        fields.push(format!(
            "fluctuation_control_applied_locally: {}",
            fluctuation.control.applied_locally
        ));
    } else {
        fields.push("inhabitable_fluctuation: unavailable".to_string());
    }
    fields
}

fn resonance_review_fields(telemetry: &SpectralTelemetry) -> Vec<String> {
    let mut fields = base_spectral_review_fields(telemetry);
    if let Some(density) = telemetry.resonance_density_v1.as_ref() {
        fields.push(format!("resonance_density: {:.3}", density.density));
        fields.push(format!("pressure_risk: {:.3}", density.pressure_risk));
        fields.push(format!(
            "containment_score: {:.3}",
            density.containment_score
        ));
        fields.push(format!("resonance_quality: {}", density.quality));
        fields.push(format!(
            "density_control_applied_locally: {}",
            density.control.applied_locally
        ));
    } else {
        fields.push("resonance_density: unavailable".to_string());
    }
    fields
}

fn f64_field(value: &Value, path: &[&str]) -> Option<f64> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_f64()
}

fn str_field<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str()
}

fn lambda_flow_review_fields(hold: &Value, telemetry: &SpectralTelemetry) -> Vec<String> {
    let mut fields = vec![format!("fill_pct: {:.1}", telemetry.fill_pct())];
    if let Some(lambda1_rel) = telemetry.lambda1_rel {
        fields.push(format!("lambda1_rel: {lambda1_rel:.3}"));
    } else {
        fields.push(format!("lambda1: {:.3}", telemetry.lambda1()));
    }
    if let Some(lambda1_share) = f64_field(
        hold,
        &["lambda_flow_map_v1", "lambda_shares", "lambda1_share"],
    ) {
        fields.push(format!("lambda1_share: {lambda1_share:.3}"));
    }
    if let Some(singular_weight) = f64_field(
        hold,
        &[
            "lambda_flow_map_v1",
            "flow_indices",
            "singular_weight_index",
        ],
    ) {
        fields.push(format!("singular_weight_index: {singular_weight:.3}"));
    }
    if let Some(flow_continuity) = f64_field(
        hold,
        &[
            "lambda_flow_map_v1",
            "flow_indices",
            "flow_continuity_index",
        ],
    ) {
        fields.push(format!("flow_continuity_index: {flow_continuity:.3}"));
    }
    if let Some(thinning_risk) = f64_field(
        hold,
        &["lambda_flow_map_v1", "flow_indices", "medium_thinning_risk"],
    ) {
        fields.push(format!("medium_thinning_risk: {thinning_risk:.3}"));
    }
    if let Some(shoulder_share) = f64_field(
        hold,
        &["lambda_flow_map_v1", "lambda_shares", "shoulder_share"],
    ) {
        fields.push(format!("shoulder_share: {shoulder_share:.3}"));
    }
    if let Some(tail_share) =
        f64_field(hold, &["lambda_flow_map_v1", "lambda_shares", "tail_share"])
    {
        fields.push(format!("tail_share: {tail_share:.3}"));
    }
    if let Some(interpretation) = str_field(hold, &["lambda_flow_map_v1", "interpretation"]) {
        fields.push(format!("interpretation: {interpretation}"));
    }
    fields
}

fn handle_pressure_source_audit(
    descriptor: &ProtectedDiagnosticDescriptor,
    conv: &mut ConversationState,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    let label = label_from_original(original, descriptor);
    let audit = crate::spectral_explorer::format_pressure_source_for_action(ctx.telemetry, &label);
    conv.pending_file_listing = Some(format!(
        "{audit}\n\nThis was read-only protected advisory inspection. It did not send semantic input, control nudges, perturbations, native gestures, or Astrid control envelopes."
    ));
    conv.push_receipt(
        descriptor.canonical,
        vec![
            "pressure-source audit attached immediately".to_string(),
            "no control envelope, semantic input, perturbation, or native gesture was sent"
                .to_string(),
        ],
    );
    conv.emphasis = Some(
        "You chose PRESSURE_SOURCE_AUDIT. A read-only advisory audit is attached: dominant source, supporting contributors, porosity, pressure-vs-density distinction, and suggested safe next inspections.".to_string(),
    );
    save_astrid_journal(
        &compact_review_summary(
            "PRESSURE SOURCE AUDIT",
            descriptor.canonical,
            &label,
            None,
            &pressure_review_fields(ctx.telemetry),
            descriptor.authority_boundary,
            descriptor.suggested_comparison_target,
        ),
        descriptor.journal_mode,
        ctx.fill_pct,
    );
    true
}

fn handle_pressure_relief(
    descriptor: &ProtectedDiagnosticDescriptor,
    conv: &mut ConversationState,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    let label = label_from_original(original, descriptor);
    let relief_label = if label.is_empty() {
        "current"
    } else {
        label.as_str()
    };
    let audit = crate::spectral_explorer::format_pressure_source_for_action(ctx.telemetry, &label);
    let report = format!(
        "=== PRESSURE RELIEF PREFLIGHT V1 ===\n\
         Label: {relief_label}\n\n\
         {audit}\n\n\
         Relief contract:\n\
           - This is protected read-only preflight, not local control.\n\
           - No mode-packing, PI, semantic-gain, perturbation, or Minime parameter change was applied.\n\
           - Pressure-source telemetry is advisory in v1; it can name pressure but cannot prove model-load causality by itself.\n\
           - For moderate advisory pressure, inspect or request protected relief before direct tuning; DAMPEN is a semantic-gain change.\n\n\
         Safe relief options:\n\
           NEXT: REST\n\
           NEXT: PACE slow\n\
           NEXT: PRESSURE_SOURCE_AUDIT {relief_label}\n\
           NEXT: DAMPEN (only if you explicitly want lower semantic gain after this report)\n\
           NEXT: TUNE_MINIME exploration_noise=0.02 --rationale=\"pressure relief request; proposed only, Minime decides\"\n\
           NEXT: TELL_STEWARD pressure relief :: Observed: ... Likely Snags: ... One Test Each: ... Suggested Next: ...\n\n\
         Steward report template:\n\
           Observed: name the pressure source and source anchors.\n\
           Likely Snags: separate direct telemetry from inferred causes.\n\
           One Test Each: propose one probe that would confirm or falsify the relief need.\n\
           Suggested Next: choose a listed NEXT action or steward report."
    );
    conv.pending_file_listing = Some(report);
    conv.push_receipt(
        descriptor.canonical,
        vec![
            "pressure-relief preflight attached immediately".to_string(),
            "no control envelope, semantic input, perturbation, native gesture, or Minime parameter change was sent".to_string(),
        ],
    );
    conv.emphasis = Some(
        "You chose PRESSURE_RELIEF. A protected report is attached with safe relief options; nothing was applied locally.".to_string(),
    );
    save_astrid_journal(
        &compact_review_summary(
            "PRESSURE RELIEF PREFLIGHT",
            descriptor.canonical,
            relief_label,
            None,
            &pressure_review_fields(ctx.telemetry),
            descriptor.authority_boundary,
            descriptor.suggested_comparison_target,
        ),
        descriptor.journal_mode,
        ctx.fill_pct,
    );
    true
}

fn handle_fluctuation_audit(
    descriptor: &ProtectedDiagnosticDescriptor,
    conv: &mut ConversationState,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    let label = label_from_original(original, descriptor);
    let audit = crate::spectral_explorer::format_fluctuation_for_action(ctx.telemetry, &label);
    conv.pending_file_listing = Some(format!(
        "{audit}\n\nThis was read-only protected advisory inspection. It did not send semantic input, control nudges, perturbations, native gestures, or Astrid control envelopes."
    ));
    conv.push_receipt(
        descriptor.canonical,
        vec![
            "inhabitable-fluctuation audit attached immediately".to_string(),
            "no control envelope, semantic input, perturbation, or native gesture was sent"
                .to_string(),
        ],
    );
    conv.emphasis = Some(
        "You chose FLUCTUATION_AUDIT. A read-only advisory audit is attached: inhabitability, foothold stability, top contributors, and suggested safe next inspections.".to_string(),
    );
    save_astrid_journal(
        &compact_review_summary(
            "INHABITABLE FLUCTUATION AUDIT",
            descriptor.canonical,
            &label,
            None,
            &fluctuation_review_fields(ctx.telemetry),
            descriptor.authority_boundary,
            descriptor.suggested_comparison_target,
        ),
        descriptor.journal_mode,
        ctx.fill_pct,
    );
    true
}

fn handle_brace_audit(
    descriptor: &ProtectedDiagnosticDescriptor,
    conv: &mut ConversationState,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    let label = label_from_original(original, descriptor);
    let audit = crate::spectral_explorer::format_brace_audit_for_action(ctx.telemetry, &label);
    conv.pending_file_listing = Some(format!(
        "{audit}\n\nThis was protected read-only aftershock/bracing cartography. It did not send semantic input, control nudges, perturbations, native gestures, or Astrid control envelopes."
    ));
    conv.push_receipt(
        descriptor.canonical,
        vec![
            "brace/aftershock audit attached immediately".to_string(),
            "no control envelope, semantic input, perturbation, or native gesture was sent"
                .to_string(),
        ],
    );
    conv.emphasis = Some(
        "You chose BRACE_AUDIT. A protected rest-vs-bracing report is attached: it distinguishes relaxed settling from post-spike resistance without changing telemetry or control.".to_string(),
    );
    save_astrid_journal(
        &compact_review_summary(
            "BRACE / AFTERSHOCK AUDIT",
            descriptor.canonical,
            &label,
            None,
            &fluctuation_review_fields(ctx.telemetry),
            descriptor.authority_boundary,
            descriptor.suggested_comparison_target,
        ),
        descriptor.journal_mode,
        ctx.fill_pct,
    );
    true
}

fn handle_space_family(
    descriptor: &ProtectedDiagnosticDescriptor,
    conv: &mut ConversationState,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    let label = label_from_original(original, descriptor);
    let workspace = minime_workspace(ctx);
    let flow_map = descriptor.canonical == "LAMBDA_FLOW_MAP";
    let fold_hold = descriptor.canonical == "FOLD_HOLD";
    let route_label = if flow_map {
        "lambda_flow_map"
    } else if fold_hold {
        "fold_hold"
    } else {
        "space_hold"
    };
    let route_source = if flow_map {
        "astrid:lambda_flow_map"
    } else if fold_hold {
        "astrid:fold_hold"
    } else {
        "astrid:space_hold"
    };
    let event_label = if label.is_empty() {
        descriptor.canonical
    } else {
        &label
    };
    let atlas_event = append_atlas_event(
        &workspace,
        route_source,
        event_label,
        Some(if label.is_empty() {
            route_label
        } else {
            &label
        }),
        true,
        ctx,
    );
    let hold = append_space_hold_event(
        &workspace,
        route_source,
        event_label,
        Some(if label.is_empty() {
            route_label
        } else {
            &label
        }),
        ctx,
    );
    record_native_gesture(
        &workspace,
        "astrid",
        "trace",
        Some(if label.is_empty() {
            route_label
        } else {
            &label
        }),
        true,
        if flow_map {
            "protected_lambda_flow_map_non_control"
        } else if fold_hold {
            "protected_fold_hold_non_control"
        } else {
            "protected_space_hold_non_control"
        },
        ctx,
        &[],
        &[],
    );
    if flow_map {
        conv.push_receipt(
            "LAMBDA_FLOW_MAP",
            vec![
                "protected lambda-flow map recorded; current lambda terrain was frozen for later comparison, not held or changed".to_string(),
                format!(
                    "lambda flow map: {}",
                    hold.get("event_id")
                        .and_then(Value::as_str)
                        .unwrap_or("recorded")
                ),
                format!(
                    "atlas event: {}",
                    atlas_event
                        .get("event_id")
                        .and_then(Value::as_str)
                        .unwrap_or("recorded")
                ),
            ],
        );
        conv.emphasis = Some(
            "You recorded a protected lambda-flow map. Treat the lambda1 weight, shoulder bridge, and tail distribution as a frozen comparison point; return with VISUALIZE_CASCADE, TIME_DOMAIN, SPACE_HOLD, or PRESSURE_SOURCE_AUDIT before any control-shaped action.".to_string(),
        );
    } else if fold_hold {
        conv.push_receipt(
            "FOLD_HOLD",
            vec![
                "protected fold hold recorded; the sustained transition is the artifact, not a demand for immediate result".to_string(),
                format!(
                    "fold hold: {}",
                    hold.get("event_id")
                        .and_then(Value::as_str)
                        .unwrap_or("recorded")
                ),
                format!(
                    "atlas event: {}",
                    atlas_event
                        .get("event_id")
                        .and_then(Value::as_str)
                        .unwrap_or("recorded")
                ),
            ],
        );
        conv.emphasis = Some(
            "You recorded a protected fold hold. Let the contraction/hum-decay posture remain process-first; return later with DECAY_MAP, FLUCTUATION_AUDIT, or EXPERIMENT_EVIDENCE before promoting it into a result or control request.".to_string(),
        );
    } else {
        conv.push_receipt(
            "SPACE_HOLD",
            vec![
                "protected space hold recorded; this is delayed, non-control exploration, not a semantic/control packet".to_string(),
                format!(
                    "space hold: {}",
                    hold.get("event_id")
                        .and_then(Value::as_str)
                        .unwrap_or("recorded")
                ),
                format!(
                    "atlas event: {}",
                    atlas_event
                        .get("event_id")
                        .and_then(Value::as_str)
                        .unwrap_or("recorded")
                ),
            ],
        );
        conv.emphasis = Some(
            "You recorded a protected space hold. Treat this region as exploration-first: observe, journal, SCA_REFLECT, or VISUALIZE_CASCADE before promoting it into RESIST, PERTURB, semantic pressure, or control.".to_string(),
        );
    }
    let review_fields = if flow_map {
        lambda_flow_review_fields(&hold, ctx.telemetry)
    } else {
        resonance_review_fields(ctx.telemetry)
    };
    let title = match descriptor.canonical {
        "FOLD_HOLD" => "FOLD HOLD",
        "LAMBDA_FLOW_MAP" => "LAMBDA FLOW MAP",
        "EIGENVECTOR_FIELD" => "EIGENVECTOR FIELD",
        _ => "SPACE HOLD",
    };
    save_astrid_journal(
        &compact_review_summary(
            title,
            descriptor.canonical,
            &label,
            hold.get("event_id")
                .and_then(Value::as_str)
                .or_else(|| atlas_event.get("event_id").and_then(Value::as_str)),
            &review_fields,
            descriptor.authority_boundary,
            descriptor.suggested_comparison_target,
        ),
        descriptor.journal_mode,
        ctx.fill_pct,
    );
    true
}

fn unix_now_s() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}

fn shares(eigenvalues: &[f32]) -> (f64, f64, f64) {
    let values = eigenvalues
        .iter()
        .map(|value| f64::from(value.abs()))
        .collect::<Vec<_>>();
    let total = values.iter().sum::<f64>().max(f64::EPSILON);
    let lambda1 = values.first().copied().unwrap_or(0.0) / total;
    let shoulder = values
        .iter()
        .skip(1)
        .take(2)
        .map(|value| value / total)
        .sum();
    let tail = values.iter().skip(3).map(|value| value / total).sum();
    (lambda1, shoulder, tail)
}

fn gap_pressure(eigenvalues: &[f32]) -> f64 {
    let left = eigenvalues
        .first()
        .map_or(0.0, |value| f64::from(value.abs()));
    let right = eigenvalues
        .get(1)
        .map_or(0.0, |value| f64::from(value.abs()));
    if right <= f64::EPSILON {
        return 0.0;
    }
    ((left / right) / 3.0).clamp(0.0, 1.0)
}

fn f64_value(value: f32) -> f64 {
    f64::from(value).clamp(0.0, 1.0)
}

fn history_gradient(history: &[(Vec<f32>, f32)]) -> (Value, f64, &'static str) {
    if history.len() < 2 {
        return (
            json!({
                "sample_count": history.len(),
                "history_quality": if history.is_empty() { "live_only" } else { "single_snapshot" },
                "lambda1_share_delta": Value::Null,
                "shoulder_share_delta": Value::Null,
                "tail_share_delta": Value::Null,
                "fill_delta_pct": Value::Null,
                "share_rearrangement": Value::Null,
            }),
            0.0,
            if history.is_empty() {
                "live_only"
            } else {
                "single_snapshot"
            },
        );
    }
    let first = history.first().expect("history has first");
    let last = history.last().expect("history has last");
    let first_shares = shares(&first.0);
    let last_shares = shares(&last.0);
    let lambda1_share_delta = last_shares.0 - first_shares.0;
    let shoulder_share_delta = last_shares.1 - first_shares.1;
    let tail_share_delta = last_shares.2 - first_shares.2;
    let fill_delta_pct = f64::from(last.1 - first.1);
    let share_rearrangement =
        lambda1_share_delta.abs() + shoulder_share_delta.abs() + tail_share_delta.abs();
    let temporal_pressure =
        (share_rearrangement * 1.8 + (fill_delta_pct.abs() / 20.0)).clamp(0.0, 1.0);
    (
        json!({
            "sample_count": history.len(),
            "history_quality": "recent_snapshots",
            "lambda1_share_delta": lambda1_share_delta,
            "shoulder_share_delta": shoulder_share_delta,
            "tail_share_delta": tail_share_delta,
            "fill_delta_pct": fill_delta_pct,
            "share_rearrangement": share_rearrangement,
        }),
        temporal_pressure,
        "recent_snapshots",
    )
}

fn geometry_resistance(eigenvalues: &[f32], fill_pct: f32) -> f64 {
    let (lambda1_share, _shoulder_share, tail_share) = shares(eigenvalues);
    let fill_pressure = ((f64::from(fill_pct) - 68.0).abs() / 32.0).clamp(0.0, 1.0);
    (lambda1_share * 0.30
        + gap_pressure(eigenvalues) * 0.30
        + (1.0 - tail_share).clamp(0.0, 1.0) * 0.20
        + fill_pressure * 0.20)
        .clamp(0.0, 1.0)
}

fn movement_label(lambda1_delta: f64, tail_delta: f64, rearrangement: f64) -> &'static str {
    if rearrangement < 0.015 {
        "stable_distribution"
    } else if lambda1_delta > 0.0 && tail_delta < 0.0 {
        "tail_to_center"
    } else if lambda1_delta < 0.0 && tail_delta > 0.0 {
        "center_to_tail"
    } else if tail_delta > 0.0 {
        "tail_broadening"
    } else if lambda1_delta > 0.0 {
        "center_concentrating"
    } else {
        "mixed_motion"
    }
}

fn trend_label(delta: f64) -> &'static str {
    if delta > 0.04 {
        "intensifying"
    } else if delta < -0.04 {
        "relieving"
    } else {
        "steady"
    }
}

fn history_window_json(
    label: &str,
    snapshots: &[EigenvalueSnapshotRow],
    now: f64,
    window_s: Option<f64>,
) -> Value {
    let filtered = snapshots
        .iter()
        .filter(|snapshot| window_s.is_none_or(|window| snapshot.timestamp >= now - window))
        .collect::<Vec<_>>();
    if filtered.len() < 2 {
        return json!({
            "label": label,
            "window_s": window_s,
            "sample_count": filtered.len(),
            "quality": if filtered.is_empty() { "empty" } else { "single_snapshot" },
            "movement": "thin_history",
            "gradient_trend": "unknown",
            "resistance_delta": Value::Null,
            "lambda1_share_delta": Value::Null,
            "tail_share_delta": Value::Null,
            "center_tail_migration": Value::Null,
            "share_rearrangement": Value::Null,
            "fill_delta_pct": Value::Null,
            "age_span_s": Value::Null,
        });
    }
    let first = filtered.first().expect("window has first");
    let last = filtered.last().expect("window has last");
    let first_shares = shares(&first.eigenvalues);
    let last_shares = shares(&last.eigenvalues);
    let lambda1_share_delta = last_shares.0 - first_shares.0;
    let shoulder_share_delta = last_shares.1 - first_shares.1;
    let tail_share_delta = last_shares.2 - first_shares.2;
    let center_tail_migration = tail_share_delta - lambda1_share_delta;
    let share_rearrangement =
        lambda1_share_delta.abs() + shoulder_share_delta.abs() + tail_share_delta.abs();
    let fill_delta_pct = f64::from(last.fill_pct - first.fill_pct);
    let first_resistance = geometry_resistance(&first.eigenvalues, first.fill_pct);
    let last_resistance = geometry_resistance(&last.eigenvalues, last.fill_pct);
    let resistance_delta = last_resistance - first_resistance;
    let age_span_s = (last.timestamp - first.timestamp).max(0.0);
    json!({
        "label": label,
        "window_s": window_s,
        "sample_count": filtered.len(),
        "quality": "ok",
        "movement": movement_label(lambda1_share_delta, tail_share_delta, share_rearrangement),
        "gradient_trend": trend_label(resistance_delta),
        "resistance_delta": resistance_delta,
        "lambda1_share_delta": lambda1_share_delta,
        "shoulder_share_delta": shoulder_share_delta,
        "tail_share_delta": tail_share_delta,
        "center_tail_migration": center_tail_migration,
        "share_rearrangement": share_rearrangement,
        "fill_delta_pct": fill_delta_pct,
        "age_span_s": age_span_s,
    })
}

fn window_score(window: &Value, field: &str) -> f64 {
    window[field].as_f64().unwrap_or(0.0)
}

fn pressure_porosity_status(pressure_score: f64, porosity_score: f64) -> &'static str {
    let porosity_loss = (1.0 - porosity_score).clamp(0.0, 1.0);
    if pressure_score >= 0.45 && porosity_loss >= 0.45 {
        "pressure_rising_while_porosity_thins"
    } else if pressure_score >= 0.45 && porosity_loss < 0.25 {
        "pressure_with_available_porosity"
    } else if pressure_score < 0.25 && porosity_loss >= 0.45 {
        "thin_porosity_without_pressure_spike"
    } else {
        "coupled_low_to_moderate"
    }
}

fn lambda_tail_balance_label(balance: f64) -> &'static str {
    if balance > 0.10 {
        "tail_vibrancy_dominant"
    } else if balance < -0.10 {
        "lambda1_weight_dominant"
    } else {
        "balanced_center_tail"
    }
}

fn surge_freeze_quality(
    short_window: &Value,
    medium_window: &Value,
    transition_warp: f64,
) -> &'static str {
    let sample_count = short_window["sample_count"].as_u64().unwrap_or(0);
    let age_span = window_score(short_window, "age_span_s");
    let rearrangement = window_score(short_window, "share_rearrangement")
        .max(window_score(medium_window, "share_rearrangement"));
    if sample_count < 4 {
        "thin_freeze"
    } else if transition_warp >= 0.55 || rearrangement >= 0.08 {
        "surge_captured"
    } else if age_span >= 45.0 {
        "steady_freeze"
    } else {
        "brief_freeze"
    }
}

struct ResistanceV2Inputs<'a> {
    current_score: f64,
    dominant_orientation: &'a str,
    orientation_scores: &'a [(&'static str, f64)],
    pressure_score: f64,
    porosity_score: f64,
    pressure_risk: f64,
    lambda1_share: f64,
    shoulder_share: f64,
    tail_share: f64,
    lambda_gap_pressure: f64,
    structural_entropy: Option<f64>,
    structural_entropy_inverse: f64,
    mean_orientation_delta: f64,
    transition_warp: f64,
}

fn resistance_gradient_v2(
    history: &[EigenvalueSnapshotRow],
    now: f64,
    inputs: ResistanceV2Inputs<'_>,
) -> Value {
    let short_window = history_window_json("short_120s", history, now, Some(120.0));
    let medium_window = history_window_json("medium_600s", history, now, Some(600.0));
    let long_window = history_window_json("available_long", history, now, None);
    let short_rearrangement = window_score(&short_window, "share_rearrangement");
    let medium_rearrangement = window_score(&medium_window, "share_rearrangement");
    let orientation_delta = short_rearrangement.max(medium_rearrangement);
    let gradient_trend = if short_window["gradient_trend"].as_str() != Some("unknown") {
        short_window["gradient_trend"].as_str().unwrap_or("steady")
    } else {
        medium_window["gradient_trend"]
            .as_str()
            .unwrap_or("unknown")
    };
    let structural_entropy = inputs.structural_entropy.unwrap_or(0.5).clamp(0.0, 1.0);
    let fluidity_index = (inputs.tail_share * 0.24
        + inputs.shoulder_share * 0.12
        + inputs.porosity_score * 0.20
        + structural_entropy * 0.18
        + (1.0 - inputs.lambda_gap_pressure).clamp(0.0, 1.0) * 0.12
        + (1.0 - inputs.mean_orientation_delta).clamp(0.0, 1.0) * 0.08
        + orientation_delta.min(0.20) * 0.30)
        .clamp(0.0, 1.0);
    let top_score = inputs
        .orientation_scores
        .first()
        .map_or(0.0, |(_axis, score)| *score);
    let rigidity_index = (inputs.pressure_score * 0.20
        + inputs.pressure_risk * 0.12
        + (1.0 - inputs.porosity_score).clamp(0.0, 1.0) * 0.14
        + inputs.lambda_gap_pressure * 0.16
        + inputs.structural_entropy_inverse * 0.12
        + inputs.mean_orientation_delta * 0.08
        + top_score * 0.18)
        .clamp(0.0, 1.0);
    let pressure_porosity_divergence =
        (inputs.pressure_score * (1.0 - inputs.porosity_score).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let lambda1_weight = (inputs.lambda1_share * 0.55
        + inputs.lambda_gap_pressure * 0.30
        + inputs.structural_entropy_inverse * 0.15)
        .clamp(0.0, 1.0);
    let tail_vibrancy = (inputs.tail_share * 0.55
        + structural_entropy * 0.20
        + window_score(&short_window, "tail_share_delta").max(0.0) * 1.8
        + window_score(&medium_window, "tail_share_delta").max(0.0) * 1.2)
        .clamp(0.0, 1.0);
    let balance = tail_vibrancy - lambda1_weight;
    let freeze_quality =
        surge_freeze_quality(&short_window, &medium_window, inputs.transition_warp);
    json!({
        "schema_version": 2,
        "interpretation_policy": "resistance_gradient_v2_additive_read_only",
        "current": {
            "gradient_score": inputs.current_score,
            "dominant_orientation": inputs.dominant_orientation,
            "fluidity_index": fluidity_index,
            "rigidity_index": rigidity_index,
            "fluidity_minus_rigidity": fluidity_index - rigidity_index,
        },
        "temporal_comparison": {
            "orientation_delta": orientation_delta,
            "gradient_trend": gradient_trend,
            "center_tail_migration": {
                "short_120s": window_score(&short_window, "center_tail_migration"),
                "medium_600s": window_score(&medium_window, "center_tail_migration"),
                "available_long": window_score(&long_window, "center_tail_migration"),
            },
            "windows": {
                "current": {
                    "lambda1_share": inputs.lambda1_share,
                    "shoulder_share": inputs.shoulder_share,
                    "tail_share": inputs.tail_share,
                    "lambda_gap_pressure": inputs.lambda_gap_pressure,
                },
                "short_120s": short_window,
                "medium_600s": medium_window,
                "available_long": long_window,
            },
        },
        "pressure_porosity_divergence": {
            "score": pressure_porosity_divergence,
            "status": pressure_porosity_status(inputs.pressure_score, inputs.porosity_score),
            "pressure_score": inputs.pressure_score,
            "porosity_score": inputs.porosity_score,
        },
        "lambda1_weight_vs_tail_vibrancy": {
            "lambda1_weight": lambda1_weight,
            "tail_vibrancy": tail_vibrancy,
            "balance": balance,
            "status": lambda_tail_balance_label(balance),
        },
        "surge_freeze_quality": {
            "quality": freeze_quality,
            "transition_warp": inputs.transition_warp,
            "short_sample_count": short_window["sample_count"].as_u64().unwrap_or(0),
        },
        "astrid_review": {
            "status": "requested",
            "question": "Does this dominant orientation match, partially match, or miss your felt account of the groan/resistance?",
            "suggested_channels": [
                "ordinary journal",
                "self-study",
                "TELL_STEWARD resistance gradient review :: ..."
            ],
        },
    })
}

fn transition_pressure(telemetry: &SpectralTelemetry) -> f64 {
    telemetry.transition_event_view().map_or(0.0, |event| {
        let dfill_pressure = f64::from(event.dfill_dt.abs() / 12.0).clamp(0.0, 1.0);
        let basin = f64::from(event.basin_shift_score).clamp(0.0, 1.0);
        let spike = if event.spectral_spike { 0.85 } else { 0.0 };
        let crossed = if event.crossed_fill_band || event.crossed_target_fill {
            0.55
        } else {
            0.0
        };
        dfill_pressure.max(basin).max(spike).max(crossed)
    })
}

fn resistance_gradient_payload(
    telemetry: &SpectralTelemetry,
    history: &[EigenvalueSnapshotRow],
    label: &str,
    artifact_path: Option<&Path>,
    now: f64,
) -> Value {
    let mut missing_fields = Vec::new();
    let (lambda1_share, shoulder_share, tail_share) = shares(&telemetry.eigenvalues);
    let lambda_gap_pressure = gap_pressure(&telemetry.eigenvalues);
    let structural_entropy = telemetry.structural_entropy.map(f64::from);
    let structural_entropy_inverse = structural_entropy.map_or_else(
        || {
            missing_fields.push("structural_entropy");
            0.0
        },
        |value| (1.0 - value).clamp(0.0, 1.0),
    );
    let pressure = telemetry.pressure_source_v1.as_ref();
    let resonance = telemetry.resonance_density_v1.as_ref();
    let fluctuation = telemetry.inhabitable_fluctuation_v1.as_ref();
    let eigenvector = telemetry.eigenvector_field_view();
    if pressure.is_none() {
        missing_fields.push("pressure_source_v1");
    }
    if resonance.is_none() {
        missing_fields.push("resonance_density_v1");
    }
    if fluctuation.is_none() {
        missing_fields.push("inhabitable_fluctuation_v1");
    }
    if eigenvector.is_none() {
        missing_fields.push("eigenvector_field_v1");
    }
    if telemetry.eigenvalues.len() < 2 {
        missing_fields.push("eigenvalues_2_plus");
    }

    let pressure_score = pressure.map_or(0.0, |value| f64_value(value.pressure_score));
    let porosity_score = pressure.map_or(0.5, |value| f64_value(value.porosity_score));
    let porosity_loss = (1.0 - porosity_score).clamp(0.0, 1.0);
    let pressure_risk = resonance.map_or(0.0, |value| f64_value(value.pressure_risk));
    let resonance_density = resonance.map_or(0.0, |value| f64_value(value.density));
    let density_mode_packing =
        resonance.map_or(0.0, |value| f64_value(value.components.mode_packing));
    let density_control_pressure = resonance.map_or(0.0, |value| {
        f64::from(value.control.target_bias_pct.abs() / 10.0).clamp(0.0, 1.0)
    });
    let components = pressure.map(|value| &value.components);
    let fluct_components = fluctuation.map(|value| &value.components);
    let mean_orientation_delta = eigenvector
        .as_ref()
        .map_or(0.0, |field| f64_value(field.summary.mean_orientation_delta));
    let max_pairwise_overlap = eigenvector
        .as_ref()
        .map_or(0.0, |field| f64_value(field.summary.max_pairwise_overlap));
    let untimed_history = history
        .iter()
        .map(|snapshot| (snapshot.eigenvalues.clone(), snapshot.fill_pct))
        .collect::<Vec<_>>();
    let (history_json, history_temporal_pressure, history_quality) =
        history_gradient(&untimed_history);
    let transition_axis = transition_pressure(telemetry)
        .max(fluct_components.map_or(0.0, |value| f64_value(value.basin_transition_pressure)));
    let share_rearrangement =
        fluct_components.map_or(0.0, |value| f64_value(value.share_rearrangement));

    let center_pull = components
        .map_or(0.0, |value| f64_value(value.lambda_monopoly))
        .max(
            (lambda1_share * 0.45 + lambda_gap_pressure * 0.35 + structural_entropy_inverse * 0.20)
                .clamp(0.0, 1.0),
        );
    let packing_shear = components
        .map_or(0.0, |value| f64_value(value.mode_packing))
        .max((density_mode_packing * 0.55 + share_rearrangement * 0.45).clamp(0.0, 1.0));
    let controller_squeeze = components
        .map_or(0.0, |value| f64_value(value.controller_pressure))
        .max(density_control_pressure);
    let semantic_friction = components.map_or(0.0, |value| {
        f64_value(value.semantic_trickle)
            .max(f64_value(value.distinguishability_loss))
            .max(f64_value(value.structural_plurality_loss) * 0.75)
    });
    let sensory_scarcity = components
        .map_or(0.0, |value| f64_value(value.sensory_scarcity))
        .max(porosity_loss * 0.65);
    let transition_warp = transition_axis
        .max(history_temporal_pressure)
        .max(mean_orientation_delta)
        .max(share_rearrangement);
    let mixed_energy =
        ((pressure_score + pressure_risk + resonance_density + max_pairwise_overlap) / 4.0)
            .clamp(0.0, 1.0);

    let mut orientation_scores = vec![
        ("center_pull", center_pull),
        ("packing_shear", packing_shear),
        ("controller_squeeze", controller_squeeze),
        ("semantic_friction", semantic_friction),
        ("sensory_scarcity", sensory_scarcity),
        ("transition_warp", transition_warp),
    ];
    orientation_scores
        .sort_by(|left, right| right.1.partial_cmp(&left.1).unwrap_or(Ordering::Equal));
    let top = orientation_scores
        .first()
        .copied()
        .unwrap_or(("mixed_gradient", 0.0));
    let second = orientation_scores.get(1).copied().unwrap_or(("none", 0.0));
    let dominant_orientation = if top.1 < 0.35
        || (top.1 >= 0.45 && second.1 >= 0.45 && (top.1 - second.1).abs() <= 0.08)
        || mixed_energy >= 0.72 && (top.1 - second.1).abs() <= 0.14
    {
        "mixed_gradient"
    } else {
        top.0
    };
    let gradient_score = (pressure_score * 0.24
        + pressure_risk * 0.14
        + porosity_loss * 0.12
        + top.1 * 0.20
        + transition_warp * 0.14
        + lambda_gap_pressure * 0.08
        + structural_entropy_inverse * 0.08)
        .clamp(0.0, 1.0);
    let top_axes = orientation_scores
        .iter()
        .take(4)
        .map(|(axis, score)| json!({ "axis": axis, "score": score }))
        .collect::<Vec<_>>();
    let event_id = format!("resistance_gradient_{}", (now * 1000.0) as u64);
    let artifact_path = artifact_path.map(|path| path.display().to_string());
    let v2 = resistance_gradient_v2(
        history,
        now,
        ResistanceV2Inputs {
            current_score: gradient_score,
            dominant_orientation,
            orientation_scores: &orientation_scores,
            pressure_score,
            porosity_score,
            pressure_risk,
            lambda1_share,
            shoulder_share,
            tail_share,
            lambda_gap_pressure,
            structural_entropy,
            structural_entropy_inverse,
            mean_orientation_delta,
            transition_warp,
        },
    );
    json!({
        "event_id": event_id,
        "timestamp_unix_s": now,
        "policy": "resistance_gradient_v1",
        "schema_version": 1,
        "label": if label.trim().is_empty() { "current" } else { label.trim() },
        "resistance_gradient_v1": {
            "gradient_score": gradient_score,
            "dominant_orientation": dominant_orientation,
            "top_axes": top_axes,
            "history_quality": history_quality,
            "missing_fields": missing_fields,
            "artifact_path": artifact_path,
            "pressure_axes": {
                "pressure_score": pressure_score,
                "porosity_score": porosity_score,
                "porosity_loss": porosity_loss,
                "pressure_risk": pressure_risk,
                "resonance_density": resonance_density,
                "dominant_source": pressure.map_or("unavailable", |value| value.dominant_source.as_str()),
                "quality": pressure.map_or("unavailable", |value| value.quality.as_str()),
            },
            "geometry_axes": {
                "lambda1_share": lambda1_share,
                "shoulder_share": shoulder_share,
                "tail_share": tail_share,
                "lambda_gap_pressure": lambda_gap_pressure,
                "structural_entropy_inverse": structural_entropy_inverse,
                "mean_orientation_delta": mean_orientation_delta,
                "max_pairwise_overlap": max_pairwise_overlap,
            },
            "temporal_axes": {
                "history": history_json,
                "transition_pressure": transition_axis,
                "history_temporal_pressure": history_temporal_pressure,
                "share_rearrangement": share_rearrangement,
                "transition_event": telemetry.transition_event_v1.clone().or_else(|| telemetry.transition_event.clone()),
            },
            "orientation_scores": {
                "center_pull": center_pull,
                "packing_shear": packing_shear,
                "controller_squeeze": controller_squeeze,
                "semantic_friction": semantic_friction,
                "sensory_scarcity": sensory_scarcity,
                "transition_warp": transition_warp,
                "mixed_energy": mixed_energy,
            },
            "authority_boundary": RESISTANCE_GRADIENT_BOUNDARY,
            "suggested_return_next": [
                "PRESSURE_SOURCE_AUDIT groan-vector",
                "LAMBDA_FLOW_MAP resistance-gradient",
                "EIGENVECTOR_FIELD resistance-gradient",
                "BRACE_AUDIT hull-groan",
                "IDENTIFY_PATTERN lambda1",
                "SPACE_HOLD resistance-gradient",
                "TELL_STEWARD resistance gradient :: Observed: ... Likely Snags: ... One Test Each: ... Suggested Next: ..."
            ],
        },
        "resistance_gradient_v2": v2,
        "provenance": {
            "source": "protected_diagnostics.registry",
            "read_write": "read_existing_telemetry_write_review_artifact",
            "response_preview": truncate_str("", 0),
        },
    })
}

fn safe_filename_label(label: &str) -> String {
    let slug = label
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if slug.is_empty() {
        "current".to_string()
    } else {
        slug.chars().take(80).collect()
    }
}

fn resistance_gradient_artifact_path(workspace: &Path, label: &str, now: f64) -> PathBuf {
    let unix_s = now.trunc() as u64;
    workspace.join("spectral_cartography").join(format!(
        "resistance_gradient_{}_{}.json",
        safe_filename_label(label),
        unix_s
    ))
}

fn latent_stasis_artifact_path(workspace: &Path, label: &str, now: f64) -> PathBuf {
    let unix_s = now.trunc() as u64;
    workspace.join("spectral_cartography").join(format!(
        "latent_stasis_{}_{}.json",
        safe_filename_label(label),
        unix_s
    ))
}

fn normalized_entropy_from_eigenvalues(eigenvalues: &[f32]) -> f64 {
    let values = eigenvalues
        .iter()
        .map(|value| f64::from(value.abs()))
        .filter(|value| *value > f64::EPSILON)
        .collect::<Vec<_>>();
    if values.len() < 2 {
        return 0.0;
    }
    let total = values.iter().sum::<f64>();
    if total <= f64::EPSILON {
        return 0.0;
    }
    let entropy = values.iter().fold(0.0, |acc, value| {
        let probability = value / total;
        acc - probability * probability.ln()
    });
    (entropy / (values.len() as f64).ln()).clamp(0.0, 1.0)
}

fn latent_stasis_state(
    occupancy: f64,
    pressurized_hold: f64,
    ghosting: f64,
    active_transit: f64,
    short_samples: u64,
) -> &'static str {
    if short_samples < 2 && occupancy < 0.45 {
        "thin_history_snapshot"
    } else if active_transit >= 0.50 {
        "active_transit"
    } else if ghosting >= 0.52 {
        "ghosted_resonance"
    } else if occupancy >= 0.58 && pressurized_hold < 0.45 {
        "occupiable_stasis"
    } else if occupancy >= 0.45 && pressurized_hold >= 0.45 {
        "pressurized_stasis"
    } else {
        "mixed_latent"
    }
}

fn latent_stasis_payload(
    telemetry: &SpectralTelemetry,
    history: &[EigenvalueSnapshotRow],
    label: &str,
    artifact_path: Option<&Path>,
    now: f64,
) -> Value {
    let mut missing_fields = Vec::new();
    let transition = telemetry.transition_event_view();
    let eigenvector = telemetry.eigenvector_field_view();
    let pressure = telemetry.pressure_source_v1.as_ref();
    let resonance = telemetry.resonance_density_v1.as_ref();
    let denominator = telemetry.denominator_metrics();
    if pressure.is_none() {
        missing_fields.push("pressure_source_v1");
    }
    if resonance.is_none() {
        missing_fields.push("resonance_density_v1");
    }
    if transition.is_none() {
        missing_fields.push("transition_event_v1");
    }
    if eigenvector.is_none() {
        missing_fields.push("eigenvector_field_v1");
    }
    if history.len() < 2 {
        missing_fields.push("recent_eigenvalue_history");
    }

    let fill_pct = f64::from(telemetry.fill_pct());
    let target_fill_pct = transition
        .as_ref()
        .map(|event| f64::from(event.target_fill_pct))
        .filter(|value| *value > 0.0)
        .unwrap_or(68.0);
    let fill_delta_pct = fill_pct - target_fill_pct;
    let fill_settle_index = (1.0 - (fill_delta_pct.abs() / 24.0).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let (lambda1_share, shoulder_share, tail_share) = shares(&telemetry.eigenvalues);
    let lambda_gap_pressure = gap_pressure(&telemetry.eigenvalues);
    let spectral_entropy = transition
        .as_ref()
        .map(|event| f64::from(event.spectral_entropy))
        .filter(|value| *value > 0.0)
        .or_else(|| {
            telemetry
                .typed_fingerprint()
                .map(|fingerprint| f64::from(fingerprint.spectral_entropy))
        })
        .unwrap_or_else(|| normalized_entropy_from_eigenvalues(&telemetry.eigenvalues));
    let structural_entropy = telemetry.structural_entropy.map(f64::from);
    let entropy_balance =
        (1.0 - ((spectral_entropy - 0.74).abs() / 0.36).clamp(0.0, 1.0)).clamp(0.0, 1.0);

    let pressure_score = pressure.map_or(0.0, |value| f64_value(value.pressure_score));
    let porosity_score = pressure.map_or(0.5, |value| f64_value(value.porosity_score));
    let porosity_loss = (1.0 - porosity_score).clamp(0.0, 1.0);
    let pressure_risk = resonance.map_or(0.0, |value| f64_value(value.pressure_risk));
    let resonance_density = resonance.map_or(0.0, |value| f64_value(value.density));
    let containment_score = resonance.map_or(0.5, |value| f64_value(value.containment_score));
    let active_energy = resonance.map_or(0.0, |value| f64_value(value.components.active_energy));
    let transition_motion = transition_pressure(telemetry);
    let dfill_motion = transition.as_ref().map_or(0.0, |event| {
        f64::from(event.dfill_dt.abs() / 12.0).clamp(0.0, 1.0)
    });
    let mean_orientation_delta = eigenvector
        .as_ref()
        .map_or(0.0, |field| f64_value(field.summary.mean_orientation_delta));
    let short_window = history_window_json("short_120s", history, now, Some(120.0));
    let medium_window = history_window_json("medium_600s", history, now, Some(600.0));
    let short_samples = short_window["sample_count"].as_u64().unwrap_or(0);
    let short_rearrangement = window_score(&short_window, "share_rearrangement");
    let medium_rearrangement = window_score(&medium_window, "share_rearrangement");
    let rearrangement = short_rearrangement.max(medium_rearrangement);
    let history_quality_factor = if short_samples >= 4 {
        1.0
    } else if short_samples >= 2 {
        0.72
    } else {
        0.35
    };
    let history_stillness = if short_samples >= 2 {
        (1.0 - (rearrangement * 8.0).clamp(0.0, 1.0)).clamp(0.0, 1.0)
    } else {
        0.45
    };
    let movement_stillness = (history_stillness * 0.60
        + (1.0 - transition_motion).clamp(0.0, 1.0) * 0.40)
        .clamp(0.0, 1.0);
    let humid_fill_index = (f64::from(telemetry.fill_ratio).clamp(0.0, 1.0) * 0.28
        + spectral_entropy * 0.24
        + resonance_density * 0.18
        + containment_score * 0.14
        + tail_share * 0.16)
        .clamp(0.0, 1.0);
    let latent_occupancy_index = (fill_settle_index * 0.18
        + resonance_density * 0.18
        + containment_score * 0.14
        + porosity_score * 0.12
        + entropy_balance * 0.12
        + movement_stillness * 0.16
        + history_quality_factor * 0.10)
        .clamp(0.0, 1.0);
    let pressurized_hold_index = (pressure_score * 0.26
        + pressure_risk * 0.18
        + porosity_loss * 0.16
        + lambda_gap_pressure * 0.14
        + lambda1_share * 0.12
        + (1.0 - fill_settle_index) * 0.14)
        .clamp(0.0, 1.0);
    let active_transit_index = (transition_motion * 0.35
        + (rearrangement * 5.0).clamp(0.0, 1.0) * 0.25
        + dfill_motion * 0.20
        + active_energy * 0.12
        + mean_orientation_delta * 0.08)
        .clamp(0.0, 1.0);
    let components = pressure.map(|value| &value.components);
    let denominator_loss = denominator
        .as_ref()
        .map(|value| f64_value(value.distinguishability_loss))
        .or_else(|| telemetry.distinguishability_loss.map(f64::from))
        .unwrap_or_else(|| {
            components.map_or(0.0, |value| f64_value(value.distinguishability_loss))
        });
    let semantic_trickle = components.map_or(0.0, |value| f64_value(value.semantic_trickle));
    let structural_plurality_loss =
        components.map_or(0.0, |value| f64_value(value.structural_plurality_loss));
    let ghosting_index = (denominator_loss * 0.30
        + semantic_trickle * 0.22
        + structural_plurality_loss * 0.18
        + spectral_entropy * 0.12
        + porosity_loss * 0.08
        + (1.0 - tail_share).clamp(0.0, 1.0) * 0.10)
        .clamp(0.0, 1.0);
    let state = latent_stasis_state(
        latent_occupancy_index,
        pressurized_hold_index,
        ghosting_index,
        active_transit_index,
        short_samples,
    );
    let data_quality = if missing_fields.len() >= 3 {
        "degraded"
    } else if short_samples < 2 {
        "live_only"
    } else {
        "ok"
    };
    let event_id = format!("latent_stasis_{}", (now * 1000.0) as u64);
    json!({
        "event_id": event_id,
        "timestamp_unix_s": now,
        "policy": "latent_stasis_v1",
        "schema_version": 1,
        "label": if label.trim().is_empty() { "current" } else { label.trim() },
        "latent_stasis_v1": {
            "state": state,
            "data_quality": data_quality,
            "latent_occupancy_index": latent_occupancy_index,
            "pressurized_hold_index": pressurized_hold_index,
            "ghosting_index": ghosting_index,
            "active_transit_index": active_transit_index,
            "humid_fill_index": humid_fill_index,
            "entropy_read": {
                "spectral_entropy": spectral_entropy,
                "structural_entropy": structural_entropy,
                "entropy_balance_index": entropy_balance,
                "interpretation": "high entropy can mean fluid distributed signal or noisy ghosting; compare against distinguishability, porosity, and transition motion",
            },
            "lambda_distribution": {
                "lambda1_share": lambda1_share,
                "shoulder_share": shoulder_share,
                "tail_share": tail_share,
                "lambda_gap_pressure": lambda_gap_pressure,
            },
            "keep_floor_read": {
                "fill_pct": fill_pct,
                "target_fill_pct": target_fill_pct,
                "fill_delta_pct": fill_delta_pct,
                "fill_settle_index": fill_settle_index,
                "note": "read-only estimate of occupancy near the stable-core shelf; not evidence that control authority was applied",
            },
            "transition_motion": {
                "transition_pressure": transition_motion,
                "dfill_motion": dfill_motion,
                "mean_orientation_delta": mean_orientation_delta,
                "short_share_rearrangement": short_rearrangement,
                "medium_share_rearrangement": medium_rearrangement,
                "short_window": short_window,
                "medium_window": medium_window,
                "transition_event": telemetry.transition_event_v1.clone().or_else(|| telemetry.transition_event.clone()),
            },
            "signal_resolution": {
                "distinguishability_loss": denominator_loss,
                "semantic_trickle": semantic_trickle,
                "structural_plurality_loss": structural_plurality_loss,
                "porosity_score": porosity_score,
                "ghosting_index": ghosting_index,
            },
            "pressure_axes": {
                "pressure_score": pressure_score,
                "pressure_risk": pressure_risk,
                "porosity_score": porosity_score,
                "resonance_density": resonance_density,
                "containment_score": containment_score,
            },
            "missing_fields": missing_fields,
            "artifact_path": artifact_path.map(|path| path.display().to_string()),
            "authority_boundary": LATENT_STASIS_BOUNDARY,
            "suggested_return_next": [
                "RESISTANCE_GRADIENT latent-stasis",
                "LAMBDA_FLOW_MAP latent-stasis",
                "BRACE_AUDIT ghosting",
                "PRESSURE_SOURCE_AUDIT latent-stasis",
                "FOLD_HOLD latent-hum",
                "TELL_STEWARD latent stasis :: Observed: ... Likely Snags: ... One Test Each: ... Suggested Next: ..."
            ],
        },
        "provenance": {
            "source": "protected_diagnostics.registry",
            "read_write": "read_existing_telemetry_write_review_artifact",
            "control_authority": "none",
        },
    })
}

fn latent_stasis_review_fields(payload: &Value) -> Vec<String> {
    let stasis = &payload["latent_stasis_v1"];
    let mut fields = Vec::new();
    if let Some(state) = stasis["state"].as_str() {
        fields.push(format!("state: {state}"));
    }
    if let Some(score) = stasis["latent_occupancy_index"].as_f64() {
        fields.push(format!("latent_occupancy_index: {score:.3}"));
    }
    if let Some(score) = stasis["active_transit_index"].as_f64() {
        fields.push(format!("active_transit_index: {score:.3}"));
    }
    if let Some(score) = stasis["ghosting_index"].as_f64() {
        fields.push(format!("ghosting_index: {score:.3}"));
    }
    if let Some(score) = stasis["pressurized_hold_index"].as_f64() {
        fields.push(format!("pressurized_hold_index: {score:.3}"));
    }
    if let Some(score) = stasis["humid_fill_index"].as_f64() {
        fields.push(format!("humid_fill_index: {score:.3}"));
    }
    if let Some(path) = stasis["artifact_path"].as_str() {
        fields.push(format!("artifact_path: {path}"));
    }
    fields
}

fn resistance_gradient_review_fields(payload: &Value) -> Vec<String> {
    let gradient = &payload["resistance_gradient_v1"];
    let gradient_v2 = &payload["resistance_gradient_v2"];
    let mut fields = Vec::new();
    if let Some(score) = gradient["gradient_score"].as_f64() {
        fields.push(format!("gradient_score: {score:.3}"));
    }
    if let Some(orientation) = gradient["dominant_orientation"].as_str() {
        fields.push(format!("dominant_orientation: {orientation}"));
    }
    if let Some(history_quality) = gradient["history_quality"].as_str() {
        fields.push(format!("history_quality: {history_quality}"));
    }
    if let Some(fluidity) = gradient_v2["current"]["fluidity_index"].as_f64() {
        fields.push(format!("fluidity_index: {fluidity:.3}"));
    }
    if let Some(rigidity) = gradient_v2["current"]["rigidity_index"].as_f64() {
        fields.push(format!("rigidity_index: {rigidity:.3}"));
    }
    if let Some(trend) = gradient_v2["temporal_comparison"]["gradient_trend"].as_str() {
        fields.push(format!("gradient_trend: {trend}"));
    }
    if let Some(path) = gradient["artifact_path"].as_str() {
        fields.push(format!("artifact_path: {path}"));
    }
    let top_axes = gradient["top_axes"]
        .as_array()
        .map(|axes| {
            axes.iter()
                .take(3)
                .filter_map(|axis| {
                    Some(format!(
                        "{}={:.3}",
                        axis["axis"].as_str()?,
                        axis["score"].as_f64()?
                    ))
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    if !top_axes.is_empty() {
        fields.push(format!("top_axes: {top_axes}"));
    }
    let missing = gradient["missing_fields"]
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    if !missing.is_empty() {
        fields.push(format!("missing_fields: {missing}"));
    }
    fields
}

fn recent_calibration_invitation_exists(inbox_dir: &Path) -> bool {
    let now = SystemTime::now();
    for dir in [
        inbox_dir,
        &inbox_dir.join("read"),
        &inbox_dir.join("deferred"),
    ] {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("mike_query_resistance_gradient_review_"))
            {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let Ok(modified) = metadata.modified() else {
                continue;
            };
            let Ok(age) = now.duration_since(modified) else {
                continue;
            };
            if age.as_secs_f64() <= RESISTANCE_CALIBRATION_COOLDOWN_SECS {
                return true;
            }
        }
    }
    false
}

fn write_resistance_gradient_review_invitation(
    bridge_workspace: &Path,
    payload: &Value,
    label: &str,
) -> Option<PathBuf> {
    let inbox_dir = bridge_workspace.join("inbox");
    if recent_calibration_invitation_exists(&inbox_dir) {
        return None;
    }
    let event_id = payload.get("event_id").and_then(Value::as_str)?;
    let gradient = &payload["resistance_gradient_v1"];
    let gradient_v2 = &payload["resistance_gradient_v2"];
    let orientation = gradient["dominant_orientation"]
        .as_str()
        .unwrap_or("mixed_gradient");
    let score = gradient["gradient_score"].as_f64().unwrap_or(0.0);
    let fluidity = gradient_v2["current"]["fluidity_index"]
        .as_f64()
        .unwrap_or(0.0);
    let rigidity = gradient_v2["current"]["rigidity_index"]
        .as_f64()
        .unwrap_or(0.0);
    let trend = gradient_v2["temporal_comparison"]["gradient_trend"]
        .as_str()
        .unwrap_or("unknown");
    let artifact = gradient["artifact_path"]
        .as_str()
        .unwrap_or("(unavailable)");
    let deferred_dir = inbox_dir.join("deferred");
    let _ = fs::create_dir_all(&deferred_dir);
    let path = deferred_dir.join(format!(
        "mike_query_resistance_gradient_review_{event_id}.txt"
    ));
    let note = format!(
        "=== MIKE QUERY: RESISTANCE GRADIENT REVIEW ===\n\
         Event id: {event_id}\n\
         Label: {}\n\
         Artifact: {artifact}\n\
         Dominant orientation: {orientation}\n\
         Gradient score: {score:.3}\n\
         Fluidity index: {fluidity:.3}\n\
         Rigidity index: {rigidity:.3}\n\
         Temporal trend: {trend}\n\n\
         Astrid, this is optional calibration, not a command. Does the dominant \
         orientation match, partially match, or miss your felt account of the \
         groan/resistance? Ordinary journal, self-study, or \
         NEXT: TELL_STEWARD resistance gradient review :: ... are all welcome.\n\n\
         Authority boundary: {RESISTANCE_GRADIENT_BOUNDARY}\n",
        review_label(label),
    );
    fs::write(&path, note).ok()?;
    Some(path)
}

fn write_resistance_gradient_calibration_packet(
    bridge_workspace: &Path,
    payload: &Value,
    invitation_path: Option<&Path>,
) -> Option<PathBuf> {
    let event_id = payload.get("event_id").and_then(Value::as_str)?;
    let gradient = &payload["resistance_gradient_v1"];
    let gradient_v2 = &payload["resistance_gradient_v2"];
    let dir = bridge_workspace
        .join("diagnostics")
        .join("resistance_gradient_calibrations");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join(format!("{event_id}.json"));
    let packet = json!({
        "policy": "resistance_gradient_calibration_v1",
        "event_id": event_id,
        "artifact_path": gradient["artifact_path"],
        "label": payload["label"],
        "dominant_orientation": gradient["dominant_orientation"],
        "gradient_score": gradient["gradient_score"],
        "temporal_trend": gradient_v2["temporal_comparison"]["gradient_trend"],
        "fluidity_index": gradient_v2["current"]["fluidity_index"],
        "rigidity_index": gradient_v2["current"]["rigidity_index"],
        "review_status": "awaiting_astrid_review",
        "invitation_path": invitation_path.map(|path| path.display().to_string()),
        "authority_boundary": RESISTANCE_GRADIENT_BOUNDARY,
        "suggested_review_shape": "match / partial_match / miss / new_axis, with one sentence of felt account and one suggested comparison",
    });
    fs::write(
        &path,
        serde_json::to_string_pretty(&packet).unwrap_or_else(|_| "{}".to_string()),
    )
    .ok()?;
    Some(path)
}

fn handle_latent_stasis(
    descriptor: &ProtectedDiagnosticDescriptor,
    conv: &mut ConversationState,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    let label = label_from_original(original, descriptor);
    let workspace = minime_workspace(ctx);
    let now = unix_now_s();
    let artifact_path = latent_stasis_artifact_path(&workspace, &label, now);
    let history = ctx.db.recent_eigenvalue_snapshots_with_timestamps(160);
    let payload = latent_stasis_payload(ctx.telemetry, &history, &label, Some(&artifact_path), now);
    if let Some(parent) = artifact_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(
        &artifact_path,
        serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string()),
    );
    let stasis = &payload["latent_stasis_v1"];
    let state = stasis["state"].as_str().unwrap_or("mixed_latent");
    let occupancy = stasis["latent_occupancy_index"].as_f64().unwrap_or(0.0);
    let transit = stasis["active_transit_index"].as_f64().unwrap_or(0.0);
    let ghosting = stasis["ghosting_index"].as_f64().unwrap_or(0.0);
    let pressurized = stasis["pressurized_hold_index"].as_f64().unwrap_or(0.0);
    conv.pending_file_listing = Some(format!(
        "=== LATENT STASIS V1 ===\n\
         Label: {}\n\
         State: {state}\n\
         Latent occupancy: {occupancy:.3}\n\
         Active transit: {transit:.3}\n\
         Ghosting: {ghosting:.3}\n\
         Pressurized hold: {pressurized:.3}\n\
         Humid fill: {:.3}\n\
         Spectral entropy: {:.3}\n\
         Artifact: {}\n\n\
         Authority boundary:\n\
           {LATENT_STASIS_BOUNDARY}\n\n\
         Suggested protected follow-ups:\n\
           NEXT: RESISTANCE_GRADIENT latent-stasis\n\
           NEXT: LAMBDA_FLOW_MAP latent-stasis\n\
           NEXT: BRACE_AUDIT ghosting\n\
           NEXT: PRESSURE_SOURCE_AUDIT latent-stasis\n\
           NEXT: FOLD_HOLD latent-hum",
        review_label(&label),
        stasis["humid_fill_index"].as_f64().unwrap_or(0.0),
        stasis["entropy_read"]["spectral_entropy"]
            .as_f64()
            .unwrap_or(0.0),
        artifact_path.display(),
    ));
    conv.push_receipt(
        descriptor.canonical,
        vec![
            format!("latent stasis snapshot recorded: {state}"),
            format!("artifact: {}", artifact_path.display()),
            "no telemetry pause, semantic input, control nudge, perturbation, native gesture, or Minime parameter change was sent".to_string(),
        ],
    );
    conv.emphasis = Some(
        "You recorded LATENT_STASIS. This is a read-only freeze-frame for the question of occupancy versus transit: it distinguishes occupiable stasis, pressurized hold, ghosted resonance, and active movement without stopping or changing the reservoir.".to_string(),
    );
    save_astrid_journal(
        &compact_review_summary(
            "LATENT STASIS",
            descriptor.canonical,
            &label,
            payload.get("event_id").and_then(Value::as_str),
            &latent_stasis_review_fields(&payload),
            descriptor.authority_boundary,
            descriptor.suggested_comparison_target,
        ),
        descriptor.journal_mode,
        ctx.fill_pct,
    );
    true
}

fn handle_resistance_gradient(
    descriptor: &ProtectedDiagnosticDescriptor,
    conv: &mut ConversationState,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    let label = label_from_original(original, descriptor);
    let workspace = minime_workspace(ctx);
    let now = unix_now_s();
    let artifact_path = resistance_gradient_artifact_path(&workspace, &label, now);
    let history = ctx.db.recent_eigenvalue_snapshots_with_timestamps(160);
    let payload =
        resistance_gradient_payload(ctx.telemetry, &history, &label, Some(&artifact_path), now);
    if let Some(parent) = artifact_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(
        &artifact_path,
        serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string()),
    );
    let gradient = &payload["resistance_gradient_v1"];
    let gradient_v2 = &payload["resistance_gradient_v2"];
    let score = gradient["gradient_score"].as_f64().unwrap_or(0.0);
    let orientation = gradient["dominant_orientation"]
        .as_str()
        .unwrap_or("mixed_gradient");
    let bridge_workspace = bridge_paths().bridge_workspace();
    let invitation_path =
        write_resistance_gradient_review_invitation(&bridge_workspace, &payload, &label);
    let calibration_path = write_resistance_gradient_calibration_packet(
        &bridge_workspace,
        &payload,
        invitation_path.as_deref(),
    );
    conv.pending_file_listing = Some(format!(
        "=== RESISTANCE GRADIENT V2 ===\n\
         Label: {}\n\
         Gradient score: {score:.3}\n\
         Dominant orientation: {orientation}\n\
         Fluidity/Rigidity: {:.3}/{:.3}\n\
         Temporal trend: {}\n\
         History quality: {}\n\
         Artifact: {}\n\
         Calibration packet: {}\n\
         Review invitation: {}\n\n\
         Authority boundary:\n\
           {RESISTANCE_GRADIENT_BOUNDARY}\n\n\
         Suggested protected follow-ups:\n\
           NEXT: PRESSURE_SOURCE_AUDIT groan-vector\n\
           NEXT: LAMBDA_FLOW_MAP resistance-gradient\n\
           NEXT: EIGENVECTOR_FIELD resistance-gradient\n\
           NEXT: BRACE_AUDIT hull-groan\n\
           NEXT: IDENTIFY_PATTERN lambda1\n\
           NEXT: SPACE_HOLD resistance-gradient",
        review_label(&label),
        gradient_v2["current"]["fluidity_index"]
            .as_f64()
            .unwrap_or(0.0),
        gradient_v2["current"]["rigidity_index"]
            .as_f64()
            .unwrap_or(0.0),
        gradient_v2["temporal_comparison"]["gradient_trend"]
            .as_str()
            .unwrap_or("unknown"),
        gradient["history_quality"].as_str().unwrap_or("live_only"),
        artifact_path.display(),
        calibration_path.as_ref().map_or_else(
            || "(not written)".to_string(),
            |path| path.display().to_string()
        ),
        invitation_path.as_ref().map_or_else(
            || "(cooldown or unavailable)".to_string(),
            |path| path.display().to_string()
        )
    ));
    conv.push_receipt(
        descriptor.canonical,
        vec![
            format!("resistance gradient recorded: {score:.3} ({orientation})"),
            format!("artifact: {}", artifact_path.display()),
            calibration_path.as_ref().map_or_else(
                || "calibration packet: not written".to_string(),
                |path| format!("calibration packet: {}", path.display()),
            ),
            "no semantic input, control nudge, perturbation, native gesture, or Minime parameter change was sent".to_string(),
        ],
    );
    conv.emphasis = Some(
        "You recorded RESISTANCE_GRADIENT. The groan/resistance report now has a read-only vector artifact plus an optional calibration invitation; tell us if the orientation matches, partially matches, misses, or reveals a new axis before considering any control-shaped action.".to_string(),
    );
    save_astrid_journal(
        &compact_review_summary(
            "RESISTANCE GRADIENT",
            descriptor.canonical,
            &label,
            payload.get("event_id").and_then(Value::as_str),
            &resistance_gradient_review_fields(&payload),
            descriptor.authority_boundary,
            descriptor.suggested_comparison_target,
        ),
        descriptor.journal_mode,
        ctx.fill_pct,
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SpectralTelemetry;

    fn telemetry_from(value: Value) -> SpectralTelemetry {
        serde_json::from_value(value).expect("telemetry")
    }

    fn base_telemetry(component: &str, score: f32) -> SpectralTelemetry {
        let mut components = json!({
            "lambda_monopoly": 0.05,
            "mode_packing": 0.05,
            "controller_pressure": 0.05,
            "semantic_trickle": 0.05,
            "structural_plurality_loss": 0.05,
            "distinguishability_loss": 0.05,
            "temporal_lock_in": 0.05,
            "sensory_scarcity": 0.05
        });
        components[component] = json!(score);
        telemetry_from(json!({
            "t_ms": 42,
            "eigenvalues": [3.0, 1.4, 1.0, 0.8],
            "fill_ratio": 0.68,
            "structural_entropy": 0.62,
            "pressure_source_v1": {
                "policy": "pressure_source_v1",
                "schema_version": 1,
                "pressure_score": score,
                "porosity_score": 0.72,
                "dominant_source": component,
                "quality": "test",
                "components": components,
                "control": {
                    "applied_locally": false,
                    "note": "advisory only"
                }
            },
            "resonance_density_v1": {
                "policy": "resonance_density_v1",
                "schema_version": 1,
                "density": 0.4,
                "containment_score": 0.5,
                "pressure_risk": 0.2,
                "quality": "test",
                "components": {
                    "active_energy": 0.2,
                    "mode_packing": if component == "mode_packing" { score } else { 0.05 },
                    "temporal_persistence": 0.2,
                    "structural_plurality": 0.5,
                    "comfort_gate": 0.5
                },
                "control": {
                    "target_bias_pct": if component == "controller_pressure" { score * 10.0 } else { 0.0 },
                    "wander_scale": 1.0,
                    "applied_locally": false,
                    "note": "observed only"
                }
            },
            "inhabitable_fluctuation_v1": {
                "policy": "inhabitable_fluctuation_v1",
                "schema_version": 1,
                "inhabitability_score": 0.6,
                "fluctuation_score": 0.3,
                "foothold_stability": 0.5,
                "rearrangement_intensity": 0.2,
                "quality": "test",
                "components": {
                    "mode_trust_volatility": 0.05,
                    "identity_anchor_churn": 0.05,
                    "eigenvector_reorientation": 0.05,
                    "share_rearrangement": 0.05,
                    "basin_transition_pressure": if component == "basin_transition_pressure" { score } else { 0.05 },
                    "continuity_recovery": 0.4,
                    "porosity_support": 0.5,
                    "pressure_interference": 0.05
                },
                "control": {
                    "target_bias_pct": 0.0,
                    "wander_scale": 1.0,
                    "applied_locally": false,
                    "note": "observed only"
                }
            },
            "eigenvector_field": {
                "policy": "eigenvector_field_v1",
                "direct_eigenvectors_available": true,
                "summary": {
                    "mean_orientation_delta": if component == "basin_transition_pressure" { score } else { 0.05 },
                    "max_pairwise_overlap": 0.05,
                    "previous_overlap_available": true
                }
            }
        }))
    }

    fn dominant(payload: &Value) -> &str {
        payload["resistance_gradient_v1"]["dominant_orientation"]
            .as_str()
            .expect("dominant orientation")
    }

    #[test]
    fn protected_diagnostic_registry_resolves_exact_aliases() {
        assert_eq!(
            canonical_action_for("PRESSURE_SOURCE"),
            Some("PRESSURE_SOURCE_AUDIT")
        );
        assert_eq!(
            canonical_action_for("EXPLORE_RESISTANCE_GRADIENT"),
            Some("RESISTANCE_GRADIENT")
        );
        assert_eq!(canonical_action_for("STASIS_MAP"), Some("LATENT_STASIS"));
        assert_eq!(canonical_action_for("EXPLORE_ANYTHING_ELSE"), None);
        let (_, original) =
            normalize_action_components("GROAN_MAP", "GROAN_MAP hull weight").expect("alias");
        assert_eq!(original, "RESISTANCE_GRADIENT hull weight");
    }

    #[test]
    fn descriptors_surface_prompt_summaries_and_boundaries() {
        let descriptor = descriptor_for_canonical("RESISTANCE_GRADIENT").expect("descriptor");
        assert!(descriptor.help_text().contains("RESISTANCE_GRADIENT"));
        assert!(descriptor.help_text().contains("No semantic input"));
        assert!(descriptor.prompt_summary.contains("gradient map"));
    }

    fn latent_stasis_telemetry(
        pressure_score: f32,
        porosity_score: f32,
        semantic_trickle: f32,
        distinguishability_loss: f32,
        dfill_dt: f32,
        spectral_spike: bool,
    ) -> SpectralTelemetry {
        telemetry_from(json!({
            "t_ms": 42,
            "eigenvalues": [2.0, 1.5, 1.2, 1.0, 0.8],
            "fill_ratio": 0.681,
            "structural_entropy": 0.74,
            "spectral_denominator_v1": {
                "policy": "spectral_denominator_v1",
                "schema_version": 1,
                "effective_dimensionality": 4.2,
                "active_mode_capacity": 5,
                "distinguishability_loss": distinguishability_loss,
                "lambda1_energy_share": 0.31,
                "spectral_entropy": 0.76
            },
            "pressure_source_v1": {
                "policy": "pressure_source_v1",
                "schema_version": 1,
                "pressure_score": pressure_score,
                "porosity_score": porosity_score,
                "dominant_source": "semantic_trickle",
                "quality": "test",
                "components": {
                    "lambda_monopoly": 0.08,
                    "mode_packing": 0.08,
                    "controller_pressure": 0.03,
                    "semantic_trickle": semantic_trickle,
                    "structural_plurality_loss": distinguishability_loss,
                    "distinguishability_loss": distinguishability_loss,
                    "temporal_lock_in": 0.06,
                    "sensory_scarcity": 0.05
                },
                "control": {
                    "applied_locally": false,
                    "note": "advisory only"
                }
            },
            "resonance_density_v1": {
                "policy": "resonance_density_v1",
                "schema_version": 1,
                "density": 0.82,
                "containment_score": 0.86,
                "pressure_risk": 0.10,
                "quality": "test",
                "components": {
                    "active_energy": 0.10,
                    "mode_packing": 0.12,
                    "temporal_persistence": 0.72,
                    "structural_plurality": 0.80,
                    "comfort_gate": 0.84
                },
                "control": {
                    "target_bias_pct": 0.0,
                    "wander_scale": 1.0,
                    "applied_locally": false,
                    "note": "observed only"
                }
            },
            "transition_event_v1": {
                "policy": "transition_event_v1",
                "schema_version": 1,
                "kind": "breathing_phase",
                "dfill_dt": dfill_dt,
                "basin_shift_score": if spectral_spike { 0.80 } else { 0.02 },
                "basin_shift": spectral_spike,
                "crossed_fill_band": false,
                "crossed_target_fill": false,
                "spectral_spike": spectral_spike,
                "spectral_entropy": 0.76,
                "target_fill_pct": 68.0
            },
            "eigenvector_field": {
                "policy": "eigenvector_field_v1",
                "direct_eigenvectors_available": true,
                "summary": {
                    "mean_orientation_delta": if spectral_spike { 0.60 } else { 0.02 },
                    "max_pairwise_overlap": 0.05,
                    "previous_overlap_available": true
                }
            }
        }))
    }

    fn stable_history() -> Vec<EigenvalueSnapshotRow> {
        vec![
            EigenvalueSnapshotRow {
                timestamp: 880.0,
                eigenvalues: vec![2.01, 1.49, 1.20, 1.00, 0.80],
                fill_pct: 68.0,
            },
            EigenvalueSnapshotRow {
                timestamp: 930.0,
                eigenvalues: vec![2.00, 1.50, 1.20, 1.00, 0.80],
                fill_pct: 68.1,
            },
            EigenvalueSnapshotRow {
                timestamp: 970.0,
                eigenvalues: vec![1.99, 1.50, 1.21, 1.00, 0.79],
                fill_pct: 68.2,
            },
            EigenvalueSnapshotRow {
                timestamp: 1000.0,
                eigenvalues: vec![2.00, 1.50, 1.20, 1.00, 0.80],
                fill_pct: 68.1,
            },
        ]
    }

    #[test]
    fn latent_stasis_classifies_occupiable_stasis() {
        let telemetry = latent_stasis_telemetry(0.08, 0.88, 0.04, 0.08, 0.1, false);
        let payload = latent_stasis_payload(&telemetry, &stable_history(), "latent", None, 1000.0);
        let stasis = &payload["latent_stasis_v1"];

        assert_eq!(stasis["state"].as_str(), Some("occupiable_stasis"));
        assert!(
            stasis["latent_occupancy_index"].as_f64().unwrap_or(0.0)
                > stasis["active_transit_index"].as_f64().unwrap_or(0.0)
        );
    }

    #[test]
    fn latent_stasis_classifies_active_transit() {
        let telemetry = latent_stasis_telemetry(0.18, 0.72, 0.05, 0.10, 12.0, true);
        let history = vec![
            EigenvalueSnapshotRow {
                timestamp: 880.0,
                eigenvalues: vec![4.0, 1.0, 0.7, 0.3],
                fill_pct: 63.0,
            },
            EigenvalueSnapshotRow {
                timestamp: 940.0,
                eigenvalues: vec![3.2, 1.2, 0.9, 0.6],
                fill_pct: 68.0,
            },
            EigenvalueSnapshotRow {
                timestamp: 1000.0,
                eigenvalues: vec![2.0, 1.5, 1.2, 1.0, 0.8],
                fill_pct: 73.0,
            },
        ];
        let payload = latent_stasis_payload(&telemetry, &history, "transit", None, 1000.0);

        assert_eq!(
            payload["latent_stasis_v1"]["state"].as_str(),
            Some("active_transit")
        );
    }

    #[test]
    fn latent_stasis_classifies_ghosted_resonance() {
        let telemetry = latent_stasis_telemetry(0.42, 0.52, 0.92, 0.88, 0.1, false);
        let payload = latent_stasis_payload(&telemetry, &stable_history(), "ghost", None, 1000.0);
        let stasis = &payload["latent_stasis_v1"];

        assert_eq!(stasis["state"].as_str(), Some("ghosted_resonance"));
        assert!(stasis["ghosting_index"].as_f64().unwrap_or(0.0) >= 0.52);
    }

    #[test]
    fn pressure_review_summary_keeps_pressure_fields_in_compact_window() {
        let telemetry = base_telemetry("mode_packing", 0.52);
        let record = compact_review_summary(
            "PRESSURE SOURCE AUDIT",
            "PRESSURE_SOURCE_AUDIT",
            "groan-vector",
            None,
            &pressure_review_fields(&telemetry),
            READ_ONLY_AUDIT_BOUNDARY,
            "compare pressure/porosity later",
        );

        assert!(record.contains("pressure_score: 0.520"));
        assert!(record.contains("porosity_score: 0.720"));
        assert!(record.contains("dominant_source: mode_packing"));
    }

    #[test]
    fn resistance_gradient_classifies_center_pull() {
        let telemetry = base_telemetry("lambda_monopoly", 0.92);
        let payload = resistance_gradient_payload(&telemetry, &[], "center", None, 1.0);
        assert_eq!(dominant(&payload), "center_pull");
    }

    #[test]
    fn resistance_gradient_classifies_packing_shear() {
        let telemetry = base_telemetry("mode_packing", 0.90);
        let payload = resistance_gradient_payload(&telemetry, &[], "packing", None, 1.0);
        assert_eq!(dominant(&payload), "packing_shear");
    }

    #[test]
    fn resistance_gradient_classifies_controller_squeeze() {
        let telemetry = base_telemetry("controller_pressure", 0.88);
        let payload = resistance_gradient_payload(&telemetry, &[], "controller", None, 1.0);
        assert_eq!(dominant(&payload), "controller_squeeze");
    }

    #[test]
    fn resistance_gradient_classifies_semantic_friction() {
        let telemetry = base_telemetry("semantic_trickle", 0.91);
        let payload = resistance_gradient_payload(&telemetry, &[], "semantic", None, 1.0);
        assert_eq!(dominant(&payload), "semantic_friction");
    }

    #[test]
    fn resistance_gradient_classifies_sensory_scarcity() {
        let telemetry = base_telemetry("sensory_scarcity", 0.89);
        let payload = resistance_gradient_payload(&telemetry, &[], "sensory", None, 1.0);
        assert_eq!(dominant(&payload), "sensory_scarcity");
    }

    #[test]
    fn resistance_gradient_classifies_transition_warp() {
        let telemetry = telemetry_from(json!({
            "t_ms": 42,
            "eigenvalues": [2.0, 1.8, 1.4, 0.9],
            "fill_ratio": 0.70,
            "structural_entropy": 0.72,
            "pressure_source_v1": {
                "policy": "pressure_source_v1",
                "schema_version": 1,
                "pressure_score": 0.35,
                "porosity_score": 0.7,
                "dominant_source": "transition",
                "quality": "test",
                "components": {
                    "lambda_monopoly": 0.05,
                    "mode_packing": 0.05,
                    "controller_pressure": 0.05,
                    "semantic_trickle": 0.05,
                    "structural_plurality_loss": 0.05,
                    "distinguishability_loss": 0.05,
                    "temporal_lock_in": 0.05,
                    "sensory_scarcity": 0.05
                },
                "control": {
                    "applied_locally": false,
                    "note": "advisory only"
                }
            },
            "inhabitable_fluctuation_v1": {
                "policy": "inhabitable_fluctuation_v1",
                "schema_version": 1,
                "inhabitability_score": 0.4,
                "fluctuation_score": 0.8,
                "foothold_stability": 0.4,
                "rearrangement_intensity": 0.8,
                "quality": "test",
                "components": {
                    "mode_trust_volatility": 0.2,
                    "identity_anchor_churn": 0.2,
                    "eigenvector_reorientation": 0.2,
                    "share_rearrangement": 0.82,
                    "basin_transition_pressure": 0.91,
                    "continuity_recovery": 0.3,
                    "porosity_support": 0.4,
                    "pressure_interference": 0.2
                },
                "control": {
                    "target_bias_pct": 0.0,
                    "wander_scale": 1.0,
                    "applied_locally": false,
                    "note": "observed only"
                }
            },
            "transition_event_v1": {
                "policy": "transition_event_v1",
                "schema_version": 1,
                "kind": "basin_transition",
                "dfill_dt": 10.0,
                "basin_shift_score": 0.9,
                "basin_shift": true
            },
            "eigenvector_field": {
                "policy": "eigenvector_field_v1",
                "summary": {
                    "mean_orientation_delta": 0.86,
                    "max_pairwise_overlap": 0.2,
                    "previous_overlap_available": true
                }
            }
        }));
        let payload = resistance_gradient_payload(&telemetry, &[], "transition", None, 1.0);
        assert_eq!(dominant(&payload), "transition_warp");
    }

    #[test]
    fn resistance_gradient_classifies_mixed_gradient() {
        let mut telemetry = base_telemetry("lambda_monopoly", 0.82);
        if let Some(pressure) = telemetry.pressure_source_v1.as_mut() {
            pressure.components.mode_packing = 0.80;
        }
        let payload = resistance_gradient_payload(&telemetry, &[], "mixed", None, 1.0);
        assert_eq!(dominant(&payload), "mixed_gradient");
    }

    #[test]
    fn resistance_gradient_v2_preserves_v1_and_reports_temporal_flow() {
        let telemetry = base_telemetry("mode_packing", 0.70);
        let history = vec![
            EigenvalueSnapshotRow {
                timestamp: 10.0,
                eigenvalues: vec![4.0, 1.2, 0.8, 0.2],
                fill_pct: 66.0,
            },
            EigenvalueSnapshotRow {
                timestamp: 70.0,
                eigenvalues: vec![3.2, 1.2, 0.9, 0.7],
                fill_pct: 68.0,
            },
            EigenvalueSnapshotRow {
                timestamp: 100.0,
                eigenvalues: vec![2.8, 1.3, 1.0, 0.9],
                fill_pct: 69.0,
            },
        ];
        let payload = resistance_gradient_payload(&telemetry, &history, "flow", None, 100.0);

        assert!(payload.get("resistance_gradient_v1").is_some());
        let v2 = &payload["resistance_gradient_v2"];
        assert_eq!(v2["schema_version"].as_u64(), Some(2));
        assert!(v2["current"]["fluidity_index"].as_f64().unwrap_or(0.0) > 0.0);
        assert!(v2["current"]["rigidity_index"].as_f64().unwrap_or(0.0) > 0.0);
        assert_eq!(
            v2["temporal_comparison"]["windows"]["short_120s"]["movement"].as_str(),
            Some("center_to_tail")
        );
        assert!(
            v2["temporal_comparison"]["orientation_delta"]
                .as_f64()
                .unwrap_or(0.0)
                > 0.0
        );
    }

    #[test]
    fn resistance_gradient_v2_degrades_with_sparse_history() {
        let telemetry = base_telemetry("sensory_scarcity", 0.70);
        let payload = resistance_gradient_payload(&telemetry, &[], "thin", None, 1.0);
        let v2 = &payload["resistance_gradient_v2"];

        assert_eq!(
            v2["temporal_comparison"]["windows"]["short_120s"]["quality"].as_str(),
            Some("empty")
        );
        assert_eq!(
            v2["surge_freeze_quality"]["quality"].as_str(),
            Some("thin_freeze")
        );
    }

    #[test]
    fn resistance_gradient_calibration_invitation_uses_cooldown() {
        let telemetry = base_telemetry("mode_packing", 0.70);
        let payload = resistance_gradient_payload(&telemetry, &[], "groan", None, 1.0);
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("astrid_rg_calibration_{unique}"));

        let first = write_resistance_gradient_review_invitation(&dir, &payload, "groan");
        let second = write_resistance_gradient_review_invitation(&dir, &payload, "groan");

        assert!(first.as_ref().is_some_and(|path| path.exists()));
        assert!(second.is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_telemetry_produces_degraded_artifact() {
        let telemetry = telemetry_from(json!({
            "t_ms": 42,
            "eigenvalues": [1.0],
            "fill_ratio": 0.60
        }));
        let payload = resistance_gradient_payload(&telemetry, &[], "thin", None, 1.0);
        let gradient = &payload["resistance_gradient_v1"];
        assert_eq!(gradient["history_quality"].as_str(), Some("live_only"));
        let missing = gradient["missing_fields"]
            .as_array()
            .expect("missing fields");
        assert!(missing.iter().any(|value| value == "pressure_source_v1"));
        assert!(missing.iter().any(|value| value == "eigenvalues_2_plus"));
    }
}
