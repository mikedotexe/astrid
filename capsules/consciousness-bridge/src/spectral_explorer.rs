#![allow(clippy::arithmetic_side_effects)]

use crate::memory::RemoteMemorySummary;
use crate::rescue_policy::STABLE_CORE_TARGET_FILL_PCT;
use crate::spectral_schema::SpectralFingerprintV1;
use crate::types::{IsingShadowState, SpectralTelemetry};

use crate::db::BridgeDb;

pub(crate) struct SpectralExplorerContext<'a> {
    pub telemetry: &'a SpectralTelemetry,
    pub selected_memory: Option<&'a RemoteMemorySummary>,
    pub controller_health: Option<&'a serde_json::Value>,
    pub ising_shadow: Option<&'a IsingShadowState>,
    pub eigen_history: &'a [(Vec<f32>, f32)],
    pub codec_history: &'a [Vec<f32>],
    pub codec_fills: &'a [f32],
    pub current_codec_features: Option<&'a [f32]>,
}

pub(crate) fn format_spectral_explorer(ctx: SpectralExplorerContext<'_>) -> String {
    let typed = ctx.telemetry.typed_fingerprint();
    let mut sections = Vec::new();
    sections.push("=== SPECTRAL EXPLORER ===".to_string());
    sections.push(format_present_state(ctx.telemetry, typed.as_ref()));
    sections.push(format_memory_comparison(
        ctx.telemetry,
        ctx.selected_memory,
        typed.as_ref(),
    ));
    sections.push(format_control_pressure(ctx.controller_health));

    if let Some(viz) = crate::spectral_viz::format_spectral_block(ctx.telemetry) {
        sections.push(viz);
    }
    if let Some(shadow) = ctx.ising_shadow {
        if let Some(viz) = crate::spectral_viz::format_shadow_block(shadow) {
            sections.push(viz);
        }
    }
    if let Some(viz) = crate::spectral_viz::format_geometry_block(
        ctx.codec_history,
        ctx.codec_fills,
        ctx.current_codec_features,
        ctx.codec_history.len(),
    ) {
        sections.push(viz);
    }
    if let Some(viz) = crate::spectral_viz::format_eigenplane_block(
        ctx.eigen_history,
        Some(&ctx.telemetry.eigenvalues),
    ) {
        sections.push(viz);
    }

    sections.join("\n\n")
}

pub(crate) fn format_for_action(
    telemetry: &SpectralTelemetry,
    memory_bank: &[RemoteMemorySummary],
    controller_health: Option<&serde_json::Value>,
    ising_shadow: Option<&IsingShadowState>,
    db: &BridgeDb,
    current_codec_features: Option<&[f32]>,
) -> String {
    let eigen_history = db.recent_eigenvalue_snapshots(100);
    let (codec_history, codec_fills) = db.recent_codec_features(100);
    let selected_memory = selected_memory(telemetry, memory_bank);
    format_spectral_explorer(SpectralExplorerContext {
        telemetry,
        selected_memory,
        controller_health,
        ising_shadow,
        eigen_history: &eigen_history,
        codec_history: &codec_history,
        codec_fills: &codec_fills,
        current_codec_features,
    })
}

pub(crate) fn selected_memory<'a>(
    telemetry: &SpectralTelemetry,
    memory_bank: &'a [RemoteMemorySummary],
) -> Option<&'a RemoteMemorySummary> {
    telemetry
        .selected_memory_id
        .as_deref()
        .and_then(|id| memory_bank.iter().find(|entry| entry.id == id))
        .or_else(|| {
            telemetry
                .selected_memory_role
                .as_deref()
                .and_then(|role| memory_bank.iter().find(|entry| entry.role == role))
        })
}

fn format_present_state(
    telemetry: &SpectralTelemetry,
    typed: Option<&SpectralFingerprintV1>,
) -> String {
    let mut lines = vec!["Present state".to_string()];
    let lambda1_rel = telemetry
        .lambda1_rel
        .map_or_else(|| "n/a".to_string(), |value| format!("{value:.3}"));
    let geom_rel = typed.map_or_else(|| "n/a".to_string(), |fp| format!("{:.3}", fp.geom_rel));
    let active_modes = telemetry
        .active_mode_count
        .map_or_else(|| "n/a".to_string(), |value| value.to_string());
    let active_energy = telemetry.active_mode_energy_ratio.map_or_else(
        || "n/a".to_string(),
        |value| format!("{:.1}%", value * 100.0),
    );
    lines.push(format!(
        "  fill={:.1}% lambda1={:.3} lambda1_rel={} geom_rel={} active_modes={} active_energy={}",
        telemetry.fill_pct(),
        telemetry.lambda1(),
        lambda1_rel,
        geom_rel,
        active_modes,
        active_energy,
    ));

    if let Some(fp) = typed {
        let (head, shoulder, tail) = fp.energy_shares();
        let structural = telemetry
            .structural_entropy
            .map_or_else(|| "n/a".to_string(), |value| format!("{value:.3}"));
        lines.push(format!(
            "  head={:.1}% shoulder={:.1}% tail={:.1}% entropy={:.3} structural_entropy={} gap={:.3}",
            head * 100.0,
            shoulder * 100.0,
            tail * 100.0,
            fp.spectral_entropy,
            structural,
            fp.lambda1_lambda2_gap,
        ));
        lines.push(format!(
            "  rotation_similarity={:.3} rotation_delta={:.3}",
            fp.v1_rotation_similarity, fp.v1_rotation_delta,
        ));
    } else {
        lines.push("  typed fingerprint unavailable; using only top-level telemetry.".to_string());
    }

    lines.join("\n")
}

fn format_memory_comparison(
    telemetry: &SpectralTelemetry,
    memory: Option<&RemoteMemorySummary>,
    typed: Option<&SpectralFingerprintV1>,
) -> String {
    let mut lines = vec!["Memory comparison".to_string()];
    match memory {
        Some(mem) => lines.push(format!(
            "  selected_memory={} role={} fill={:.1}% lambda1_rel={:.3} geom_rel={:.3}",
            mem.id, mem.role, mem.fill_pct, mem.lambda1_rel, mem.geom_rel,
        )),
        None => {
            let role = telemetry.selected_memory_role.as_deref().unwrap_or("none");
            let id = telemetry.selected_memory_id.as_deref().unwrap_or("none");
            lines.push(format!("  selected_memory={id} role={role}"));
        },
    }

    let selected = telemetry
        .spectral_glimpse_12d
        .as_deref()
        .or_else(|| memory.map(|mem| mem.spectral_glimpse_12d.as_slice()));
    if let Some(glimpse) = selected {
        lines.push(format!(
            "  selected_glimpse_12d={}",
            compact_vec(glimpse, 12)
        ));
    } else {
        lines.push("  selected_glimpse_12d=n/a".to_string());
    }

    if let Some(fp) = typed {
        let live = fp.live_glimpse_12d();
        lines.push(format!("  live_glimpse_12d={}", compact_vec(&live, 12)));
        if let Some(selected) = selected {
            lines.push(format!(
                "  compact_deltas={}",
                compact_glimpse_delta(selected, &live)
            ));
        } else {
            lines.push("  compact_deltas=n/a".to_string());
        }
    } else {
        lines.push("  live_glimpse_12d=n/a".to_string());
        lines.push("  compact_deltas=n/a".to_string());
    }

    lines.join("\n")
}

fn format_control_pressure(health: Option<&serde_json::Value>) -> String {
    let mut lines = vec!["Control pressure".to_string()];
    let Some(health) = health else {
        lines.push("  controller_health=n/a".to_string());
        return lines.join("\n");
    };

    let stable_core = health
        .get("stable_core")
        .unwrap_or(&serde_json::Value::Null);
    let stable_enabled = bool_value(stable_core, "enabled")
        .or_else(|| bool_value(health, "stable_core_enabled"))
        .unwrap_or(false);
    let stage = str_value(stable_core, "stage").unwrap_or("unknown");
    let structural_mode = str_value(stable_core, "structural_mode").unwrap_or("unknown");
    let controller_mode = str_value(stable_core, "controller_mode").unwrap_or("unknown");
    let gate = f64_value(health, "gate").unwrap_or(0.0);
    let filt = f64_value(health, "filt").unwrap_or(0.0);
    let pi = health.get("pi").unwrap_or(&serde_json::Value::Null);
    let target_fill = f64_value(pi, "target_fill")
        .or_else(|| f64_value(health, "target_fill_pct"))
        .unwrap_or(STABLE_CORE_TARGET_FILL_PCT);
    let target_lambda = f64_value(pi, "target_lambda1_rel").unwrap_or(1.0);
    let target_geom = f64_value(pi, "target_geom_rel").unwrap_or(1.0);
    let e_fill = f64_value(pi, "e_fill").unwrap_or(0.0);
    let e_lam = f64_value(pi, "e_lam").unwrap_or(0.0);
    let e_geom = f64_value(pi, "e_geom").unwrap_or(0.0);
    let integ_fill = f64_value(pi, "integ_fill").unwrap_or(0.0);
    let integ_lam = f64_value(pi, "integ_lam").unwrap_or(0.0);
    let integ_geom = f64_value(pi, "integ_geom").unwrap_or(0.0);

    lines.push(format!(
        "  stable_core={} stage={} controller_mode={} structural_mode={}",
        stable_enabled, stage, controller_mode, structural_mode,
    ));
    lines.push(format!(
        "  gate={gate:.3} filt={filt:.3} target_fill={target_fill:.1}% target_lambda1_rel={target_lambda:.3} target_geom_rel={target_geom:.3}",
    ));
    lines.push(format!(
        "  pi_errors fill={e_fill:+.3} lambda={e_lam:+.3} geom={e_geom:+.3}",
    ));
    lines.push(format!(
        "  pi_integrators fill={integ_fill:+.3} lambda={integ_lam:+.3} geom={integ_geom:+.3}",
    ));

    if stable_enabled {
        let structural_pi = stable_core
            .get("structural_pi")
            .unwrap_or(&serde_json::Value::Null);
        let active = bool_value(structural_pi, "active").unwrap_or(false);
        let drain_weight = f64_value(structural_pi, "drain_weight").unwrap_or(0.0);
        let target = f64_value(structural_pi, "target_fill_pct").unwrap_or(target_fill);
        lines.push(format!(
            "  stable_core_structural_pi active={} target_fill={target:.1}% drain_weight={drain_weight:.3}",
            active,
        ));
        lines.push(
            "  note: stable-core is active; visible legacy PI fields may be mirror/scaffold state."
                .to_string(),
        );
    }

    lines.join("\n")
}

fn compact_vec(values: &[f32], limit: usize) -> String {
    let mut parts = values
        .iter()
        .take(limit)
        .map(|value| format!("{value:+.3}"))
        .collect::<Vec<_>>();
    if values.len() > limit {
        parts.push("...".to_string());
    }
    format!("[{}]", parts.join(", "))
}

fn compact_glimpse_delta(selected: &[f32], live: &[f32]) -> String {
    let labels = [
        "head", "shoulder", "tail", "entropy", "gap", "rotation", "geom", "gap_mean",
    ];
    let indices = [0_usize, 1, 2, 7, 8, 9, 10, 11];
    let parts = indices
        .iter()
        .zip(labels)
        .filter_map(|(index, label)| {
            let before = selected.get(*index)?;
            let after = live.get(*index)?;
            Some(format!("{label}:{:+.3}", after - before))
        })
        .collect::<Vec<_>>();
    if parts.is_empty() {
        "n/a".to_string()
    } else {
        parts.join(", ")
    }
}

fn f64_value(value: &serde_json::Value, key: &str) -> Option<f64> {
    value.get(key).and_then(serde_json::Value::as_f64)
}

fn bool_value(value: &serde_json::Value, key: &str) -> Option<bool> {
    value.get(key).and_then(serde_json::Value::as_bool)
}

fn str_value<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(serde_json::Value::as_str)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::types::SpectralTelemetry;

    fn telemetry() -> SpectralTelemetry {
        let slots = (0..32).map(|value| value as f32 / 10.0).collect::<Vec<_>>();
        SpectralTelemetry {
            t_ms: 42,
            eigenvalues: vec![8.0, 4.0, 2.0, 1.0, 0.5],
            fill_ratio: 0.55,
            active_mode_count: Some(3),
            active_mode_energy_ratio: Some(0.875),
            lambda1_rel: Some(1.08),
            modalities: None,
            neural: None,
            alert: None,
            spectral_fingerprint: Some(slots),
            spectral_fingerprint_v1: None,
            structural_entropy: Some(0.72),
            spectral_glimpse_12d: Some(vec![0.3; 12]),
            eigenvector_field: None,
            selected_memory_id: Some("memory_stable_1".to_string()),
            selected_memory_role: Some("stable".to_string()),
            ising_shadow: None,
        }
    }

    #[test]
    fn explorer_includes_required_sections() {
        let telemetry = telemetry();
        let memory = RemoteMemorySummary {
            id: "memory_stable_1".to_string(),
            role: "stable".to_string(),
            timestamp_ms: 7,
            spectral_glimpse_12d: vec![0.2; 12],
            fill_pct: 52.0,
            lambda1_rel: 1.01,
            spread: 0.4,
            geom_rel: 0.98,
        };
        let health = json!({
            "gate": 0.2,
            "filt": 0.8,
            "stable_core": {
                "enabled": true,
                "stage": "hold",
                "controller_mode": "fixed_survival",
                "structural_mode": "scaffold_hold",
                "structural_pi": {
                    "active": true,
                    "target_fill_pct": 68.0,
                    "drain_weight": 0.1
                }
            },
            "pi": {
                "target_fill": 68.0,
                "target_lambda1_rel": 1.05,
                "target_geom_rel": 1.0,
                "e_fill": -1.0,
                "e_lam": 0.03,
                "e_geom": 0.01,
                "integ_fill": 0.2,
                "integ_lam": -0.1,
                "integ_geom": 0.0
            }
        });

        let output = format_spectral_explorer(SpectralExplorerContext {
            telemetry: &telemetry,
            selected_memory: Some(&memory),
            controller_health: Some(&health),
            ising_shadow: None,
            eigen_history: &[],
            codec_history: &[],
            codec_fills: &[],
            current_codec_features: None,
        });

        assert!(output.contains("Present state"));
        assert!(output.contains("Memory comparison"));
        assert!(output.contains("Control pressure"));
        assert!(output.contains("stable-core is active"));
    }
}
