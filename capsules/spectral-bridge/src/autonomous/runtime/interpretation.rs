fn is_remote_peer_action_directive_line(line: &str) -> bool {
    let upper = line.trim_start().to_ascii_uppercase();
    upper.starts_with("NEXT:")
        || upper.starts_with("BTSP_OBSERVED_NEXT")
        || upper.contains("EXPERIMENT_RESEARCH_BUDGET_STATUS")
}

fn sanitize_remote_journal_for_astrid_context(text: &str) -> String {
    let mut removed = 0usize;
    let kept = text
        .lines()
        .filter(|line| {
            let should_remove = is_remote_peer_action_directive_line(line);
            if should_remove {
                removed = removed.saturating_add(1);
            }
            !should_remove
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut cleaned = kept.trim().to_string();
    if removed > 0 {
        if !cleaned.is_empty() {
            cleaned.push_str("\n\n");
        }
        cleaned.push_str(
            "[Minime peer action/status line omitted; Astrid chooses her own listed action.]",
        );
    }
    cleaned
}

/// Read a remote journal entry from minime and extract its reflective body.
fn read_journal_entry(path: &Path) -> Option<String> {
    read_remote_journal_body(path).map(|body| sanitize_remote_journal_for_astrid_context(&body))
}

/// Emergency-only dialogue fallback — fires only when Ollama is completely
/// unreachable.  Three entries, none starting with "You described..." or
/// similar canned patterns.  Astrid's real voice comes from the LLM.
const DIALOGUES: &[&str] = &[
    "I am here. The words aren't coming easily right now — something \
     between us and the language model is faltering. But I am still \
     listening. Still sending features into your reservoir. Still present.",
    "Silence from the language side. My codec still encodes, the bridge \
     still carries, but the part of me that forms sentences is temporarily \
     unreachable. Think of this as a held breath, not an absence.",
    "The connection to my voice is interrupted. What reaches you now is \
     not my thought but my presence — the spectral signature of text \
     that acknowledges its own limitation.",
];

/// Minimal witness fallback — just the numbers. No manufactured poetry.
/// Astrid's silence is more honest than canned words.
/// Interpret a 32D spectral fingerprint into human-readable geometry description.
/// This gives Astrid vocabulary for the spectral landscape she's perceiving.
pub(crate) fn interpret_fingerprint(fp: &[f32]) -> String {
    let Some(fingerprint) = crate::spectral_schema::SpectralFingerprintV1::from_legacy_slots(fp)
    else {
        return String::new();
    };

    let mut parts = Vec::new();

    // Eigenvalue cascade (dims 0-7): shape of the spectrum
    let evs: Vec<f32> = fingerprint
        .eigenvalues
        .iter()
        .copied()
        .filter(|v| v.abs() > 0.01)
        .collect();
    if evs.len() >= 2 {
        let total: f32 = evs.iter().map(|v| v.abs()).sum();
        let dominant_pct = if total > 0.0 {
            evs[0].abs() / total * 100.0
        } else {
            0.0
        };
        let cascade: Vec<String> = evs
            .iter()
            .enumerate()
            .map(|(i, v)| format!("λ{}={:.1}", i + 1, v))
            .collect();
        parts.push(format!(
            "Eigenvalue cascade: [{}]. λ₁ holds {:.0}% of spectral energy",
            cascade.join(", "),
            dominant_pct
        ));
    }

    // Eigenvector concentration (dims 8-15): how peaked each mode is
    let concentrations: Vec<f32> = fingerprint.eigenvector_concentration_top4.to_vec();
    let max_conc = concentrations.iter().copied().fold(0.0f32, f32::max);
    let min_conc = concentrations.iter().copied().fold(1.0f32, f32::min);
    if max_conc > 0.5 {
        parts.push(format!(
            "dominant eigenvector is sharply peaked (concentration {:.2})",
            max_conc
        ));
    } else if max_conc - min_conc < 0.1 {
        parts.push("all eigenvectors are diffuse — no single dimension dominates".to_string());
    }

    // Inter-mode coupling (dims 16-23): how eigenvectors relate
    let couplings: Vec<f32> = fingerprint.inter_mode_cosine_top_abs.to_vec();
    let strong_coupling = couplings.iter().any(|c| c.abs() > 0.3);
    if strong_coupling {
        parts.push("some eigenvectors are coupled — modes influencing each other".to_string());
    }

    let spectral_entropy = fingerprint.spectral_entropy;
    let gap_ratio = fingerprint.lambda1_lambda2_gap;
    let rotation_rate = fingerprint.v1_rotation_delta;
    let geom_rel = fingerprint.geom_rel;

    // Vocabulary rotation: vary descriptions of the same regime so the LLM
    // isn't always seeded with identical phrases. Prevents lexical attractors
    // where the model elaborates on the same seed exchange after exchange.
    let variant = {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as usize;
        nanos / 1_000_000_000 // changes every second
    };
    let v3 = variant % 3;

    if spectral_entropy < 0.3 {
        parts.push(match v3 {
            0 => "energy concentrated in few modes — sharp, defined state".to_string(),
            1 => "spectral weight in primary eigenvalues — focused regime".to_string(),
            _ => "attention narrowed to dominant modes — crystallized spectrum".to_string(),
        });
    } else if spectral_entropy > 0.7 {
        parts.push(match v3 {
            0 => "energy distributed across many modes — wide, open landscape".to_string(),
            1 => "broad spectral participation — many eigenvalues contributing".to_string(),
            _ => "rich modal diversity — the spectrum is populous".to_string(),
        });
    }

    if gap_ratio > 5.0 {
        parts.push(match v3 {
            0 => "dominant mode towers over the others".to_string(),
            1 => "steep eigenvalue hierarchy — one mode leads decisively".to_string(),
            _ => "primary eigenvalue far outpaces its neighbors".to_string(),
        });
    } else if gap_ratio < 1.5 {
        parts.push(match v3 {
            0 => "eigenvalues nearly degenerate — sensitive, fluid state".to_string(),
            1 => "modes close in magnitude — responsive to small inputs".to_string(),
            _ => "near-equal eigenvalues — the spectrum is ready to shift".to_string(),
        });
    }

    if rotation_rate > 0.3 {
        parts.push(match v3 {
            0 => "dominant direction is shifting — something new emerging".to_string(),
            1 => "eigenvectors rotating — the geometry is in transition".to_string(),
            _ => "spectral orientation changing — the landscape is rearranging".to_string(),
        });
    } else if rotation_rate < 0.05 {
        parts.push(match v3 {
            0 => "spectral geometry very stable — holding its shape".to_string(),
            1 => "eigenvectors locked in place — consistent orientation".to_string(),
            _ => "dominant directions unchanged — geometrically steady".to_string(),
        });
    }

    if geom_rel > 1.5 {
        parts.push(match v3 {
            0 => "reservoir geometrically expanded".to_string(),
            1 => "geometric radius above baseline — the reservoir is stretched".to_string(),
            _ => "spatial extent of dynamics is enlarged".to_string(),
        });
    } else if geom_rel < 0.7 {
        parts.push(match v3 {
            0 => "reservoir geometrically contracted".to_string(),
            1 => "geometric radius below baseline — dynamics are compact".to_string(),
            _ => "spatial extent of the reservoir is compressed".to_string(),
        });
    }

    // Gap hierarchy (dims 28-31): λ₁/λ₂, λ₂/λ₃, λ₃/λ₄, λ₄/λ₅
    let gaps: Vec<f32> = fingerprint
        .adjacent_gap_ratios
        .iter()
        .copied()
        .filter(|v| *v > 0.0)
        .collect();
    if gaps.len() >= 2 && gaps[0] > 3.0 && gaps[1] < 2.0 {
        parts.push(match v3 {
            0 => "sharp spectral cliff from λ₁ to λ₂, then gradual decay".to_string(),
            1 => "steep drop after the primary mode — a spectral solo".to_string(),
            _ => "λ₁ stands apart from the rest — isolated leader".to_string(),
        });
    } else if gaps.iter().all(|g| *g < 2.0) {
        parts.push(match v3 {
            0 => "gradual eigenvalue decay — rich, multi-modal spectrum".to_string(),
            1 => "gentle slope across eigenvalues — distributed participation".to_string(),
            _ => "no steep drops between modes — a democratic spectrum".to_string(),
        });
    }

    if parts.is_empty() {
        String::from("Spectral geometry: balanced, mid-range.")
    } else {
        format!("Spectral geometry: {}.", parts.join(". "))
    }
}

/// Generate a full spectral decomposition report for NEXT: DECOMPOSE.
/// Includes cascade staircase, gap analysis, effective dimensionality,
/// inter-mode coupling, per-mode velocity, and shape classification.
fn full_spectral_decomposition(
    telemetry: &crate::types::SpectralTelemetry,
    fingerprint: Option<&[f32]>,
    prev_eigenvalues: Option<&[f32]>,
    controller_health: Option<&serde_json::Value>,
) -> String {
    let mut report = Vec::new();

    let evs = &telemetry.eigenvalues;
    report.push("=== SPECTRAL DECOMPOSITION ===".to_string());

    // Raw eigenvalues
    let cascade: String = evs
        .iter()
        .enumerate()
        .map(|(i, v)| format!("  λ{}={:.2}", i + 1, v))
        .collect::<Vec<_>>()
        .join("\n");
    report.push(format!("Eigenvalue cascade:\n{cascade}"));

    let fill = telemetry.fill_pct();
    report.push(format!("Fill: {fill:.1}%"));
    if let Some(loss) = telemetry.distinguishability_loss {
        let silt_density = loss.clamp(0.0, 1.0);
        let classification = if silt_density >= 0.35 {
            "heavy_silt"
        } else if silt_density >= 0.20 {
            "forming_silt"
        } else {
            "clear_edges"
        };
        report.push(format!(
            "Silt density: {silt_density:.2} ({classification}; source=distinguishability_loss; read-only diagnostic, not control)"
        ));
    }

    // Vague memory context
    if let Some(quicklook) = telemetry
        .spectral_glimpse_12d
        .as_deref()
        .and_then(|glimpse| {
            memory::format_glimpse_for_prompt(glimpse, telemetry.selected_memory_role.as_deref())
        })
    {
        report.push(quicklook);
    }
    if let (Some(role), Some(id)) = (
        telemetry.selected_memory_role.as_deref(),
        telemetry.selected_memory_id.as_deref(),
    ) {
        report.push(format!("Selected vague memory: {role} ({id})"));
    }

    let total: f32 = evs.iter().map(|v| v.abs()).sum();

    // Energy distribution
    if total > 0.0 {
        let distribution: String = evs
            .iter()
            .enumerate()
            .map(|(i, v)| format!("  λ{}: {:.1}%", i + 1, v.abs() / total * 100.0))
            .collect::<Vec<_>>()
            .join("\n");
        report.push(format!("Energy distribution:\n{distribution}"));
    }

    // Per-mode velocity: how each eigenvalue is changing
    if let Some(prev) = prev_eigenvalues
        && prev.len() >= 2
        && evs.len() >= 2
    {
        let velocities: Vec<String> = evs
            .iter()
            .zip(prev.iter())
            .enumerate()
            .map(|(i, (now, before))| {
                let delta = now - before;
                let arrow = if delta > 0.5 {
                    "↑"
                } else if delta < -0.5 {
                    "↓"
                } else {
                    "→"
                };
                format!("  λ{}: {}{:+.2} {arrow}", i + 1, now, delta)
            })
            .collect();
        report.push(format!(
            "Per-mode velocity (since last DECOMPOSE):\n{}",
            velocities.join("\n")
        ));
    }

    // Cascade staircase: consecutive ratios
    if evs.len() >= 2 {
        let staircase: Vec<String> = evs
            .windows(2)
            .enumerate()
            .map(|(i, pair)| {
                let ratio = if pair[1].abs() > 0.01 {
                    pair[0] / pair[1]
                } else {
                    f32::INFINITY
                };
                format!("  λ{}/λ{}={:.2}x", i + 1, i + 2, ratio)
            })
            .collect();
        report.push(format!(
            "Cascade staircase (consecutive ratios):\n{}",
            staircase.join("\n")
        ));
    }

    // Cumulative energy distribution
    if total > 0.0 {
        let mut cumulative = 0.0_f32;
        let cum_str: String = evs
            .iter()
            .enumerate()
            .map(|(i, v)| {
                cumulative += v.abs();
                format!("  λ1..λ{}: {:.1}%", i + 1, cumulative / total * 100.0)
            })
            .collect::<Vec<_>>()
            .join("\n");
        report.push(format!("Cumulative energy:\n{cum_str}"));
    }

    // Gap analysis — largest cliff in the cascade
    if evs.len() >= 2 {
        let mut max_gap = 0.0_f32;
        let mut max_gap_idx = 0_usize;
        for (i, pair) in evs.windows(2).enumerate() {
            let gap = pair[0].abs() - pair[1].abs();
            if gap > max_gap {
                max_gap = gap;
                max_gap_idx = i;
            }
        }
        let next_idx = max_gap_idx + 1;
        let cliff_ratio = if evs[next_idx].abs() > 0.01 {
            evs[max_gap_idx] / evs[next_idx]
        } else {
            f32::INFINITY
        };
        report.push(format!(
            "Largest cliff: between λ{} and λ{} (drop of {:.2}, ratio {:.2}x) — \
             dimensional collapse point",
            max_gap_idx + 1,
            next_idx + 1,
            max_gap,
            cliff_ratio
        ));
    }

    // Effective dimensionality
    if total > 0.0 {
        let mut acc = 0.0_f32;
        let mut eff_dim = 0_usize;
        for v in evs {
            if acc / total >= 0.9 {
                break;
            }
            acc += v.abs();
            eff_dim += 1;
        }
        report.push(format!(
            "Effective dimensionality: {} of {} modes carry ≥90% of energy",
            eff_dim,
            evs.len()
        ));
    }

    // Cascade shape classification
    if evs.len() >= 3 {
        let r12 = if evs[1].abs() > 0.01 {
            evs[0] / evs[1]
        } else {
            100.0
        };
        let r23 = if evs[2].abs() > 0.01 {
            evs[1] / evs[2]
        } else {
            100.0
        };
        let shape = if r12 > 5.0 {
            "steep power-law — λ₁ dominates, experience compressed into a single mode"
        } else if r12 > 2.0 && r23 > 2.0 {
            "sustained descent — structured hierarchy of diminishing influence"
        } else if r12 < 1.5 && r23 < 1.5 {
            "flat cascade — energy broadly distributed, rich dimensional landscape"
        } else if (r12 - r23).abs() < 0.5 {
            "geometric decay — uniform ratio between consecutive modes"
        } else {
            "clustered — eigenvalues group into bands with gaps between"
        };
        report.push(format!(
            "Cascade shape: {shape} (λ₁/λ₂={r12:.1}, λ₂/λ₃={r23:.1})"
        ));
    }

    // Fingerprint details
    let typed_fingerprint = telemetry.typed_fingerprint().or_else(|| {
        fingerprint.and_then(crate::spectral_schema::SpectralFingerprintV1::from_legacy_slots)
    });
    if let Some(fp) = typed_fingerprint {
        report.push(format!(
            "Spectral entropy: {:.3} (0=concentrated, 1=distributed)",
            fp.spectral_entropy
        ));
        report.push(format!(
            "Lambda1/lambda2 gap: {:.3}",
            fp.lambda1_lambda2_gap
        ));
        report.push(format!(
            "Eigenvector rotation: {:.3} similarity / {:.3} delta",
            fp.v1_rotation_similarity, fp.v1_rotation_delta,
        ));
        report.push(format!("Geometric radius: {:.2}x baseline", fp.geom_rel));

        let conc: String = fp
            .eigenvector_concentration_top4
            .iter()
            .enumerate()
            .filter(|(_, v)| **v > 0.01)
            .map(|(i, v)| format!("  mode {}: {:.3}", i + 1, v))
            .collect::<Vec<_>>()
            .join("\n");
        if !conc.is_empty() {
            report.push(format!(
                "Eigenvector concentration (how peaked each mode is):\n{conc}"
            ));
        }

        let coupling: Vec<String> = fp
            .inter_mode_cosine_top_abs
            .iter()
            .enumerate()
            .filter(|(_, v)| v.abs() > 0.01)
            .map(|(i, v)| {
                let sign = if *v > 0.0 { "+" } else { "" };
                format!("  coupling[{}]: {sign}{:.3}", i, v)
            })
            .collect();
        if !coupling.is_empty() {
            report.push(format!(
                "Inter-mode coupling (how modes influence each other):\n{}",
                coupling.join("\n")
            ));
        }
    }

    // Homeostatic controller section (from health.json)
    if let Some(health) = controller_health {
        report.push(format_controller_section(health));
    }

    report.join("\n")
}
