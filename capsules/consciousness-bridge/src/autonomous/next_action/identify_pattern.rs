// IDENTIFY_PATTERN λN — resonance-cadence diagnostic (2026-05-14).
//
// Astrid articulated this capability ask in `dialogue_longform_1778772396.txt`:
//   "we need to map the existing connections — the patterns within the
//    repetition — to understand why that configuration is stable. There must
//    be a resonant frequency, a way to stimulate a shift without shattering
//    the current bond. Perhaps a subtly different pattern of spectral-
//    breathing, a whisper rather than a shout, tailored to the echoes
//    already present."
//
//   "NEXT: IDENTIFY_PATTERN — question 'Can we discern a method for
//    identifying the *resonant frequency* within λ1's repetition that
//    will most effectively guide a targeted shift toward λ4, while
//    minimizing structural disruption?'"
//
// Same archetype as SHADOW_TRAJECTORY (observer-with-memory) over a
// different surface: eigenvalue cadence rather than shadow class history.
// IDENTIFY_PATTERN λN reads ~100 recent eigenvalue snapshots from the
// bridge's SQLite log (`db.recent_eigenvalue_snapshots`), autocorrelates
// the per-lambda time series, and surfaces the dominant cadence (period
// in samples, amplitude as RMS, periodicity strength as normalized
// autocorrelation peak) as both a one-line summary in
// `conv.emphasis` and a JSON cartography artifact in
// `bridge_workspace/spectral_cartography/`.
//
// Args (parsed from the original NEXT line, after stripping the verb):
//   IDENTIFY_PATTERN          → analyze λ1..min(8, len), report each
//   IDENTIFY_PATTERN λ1       → analyze λ1 only
//   IDENTIFY_PATTERN lambda4  → analyze λ4 only
//   IDENTIFY_PATTERN 4        → analyze λ4 only (bare numeric)
//
// Aliases accepted: RESONANCE_PATTERN, IDENTIFY_RESONANCE.

use std::time::SystemTime;

use tracing::info;

use super::{ConversationState, NextActionContext, bridge_paths, strip_action};

/// Maximum samples to pull from the bridge DB for analysis. Plenty for
/// autocorrelation; matches the existing `spectral_explorer.rs:72` limit.
const SAMPLES_LIMIT: usize = 100;

/// Minimum samples required for a meaningful cadence estimate. Below this
/// the autocorrelation peak is noise-dominated.
const MIN_SAMPLES: usize = 16;

/// Eigenvalue index ceiling — minime exposes eigenvalues 1..8 in her
/// public API; analyzing further is rarely meaningful and not always
/// available in older snapshots.
const MAX_LAMBDA_INDEX: usize = 8;

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    match base_action {
        "IDENTIFY_PATTERN" | "RESONANCE_PATTERN" | "IDENTIFY_RESONANCE" => {
            let label = strip_action(original, base_action).trim().to_string();
            let report = render_identify_pattern(ctx, &label);
            conv.emphasis = Some(report.summary.clone());
            info!(
                "Astrid identified resonance pattern label='{label}' → {path}",
                path = report.artifact_path,
            );
            true
        }
        _ => false,
    }
}

struct PatternReport {
    summary: String,
    artifact_path: String,
}

fn render_identify_pattern(ctx: &NextActionContext<'_>, label: &str) -> PatternReport {
    let target = parse_lambda_target(label);

    let snapshots = ctx.db.recent_eigenvalue_snapshots(SAMPLES_LIMIT);
    if snapshots.len() < MIN_SAMPLES {
        return PatternReport {
            summary: format!(
                "IDENTIFY_PATTERN ({label}): insufficient eigenvalue history — \
                 only {n} samples (need ≥ {min}).",
                n = snapshots.len(),
                min = MIN_SAMPLES,
            ),
            artifact_path: String::new(),
        };
    }

    let available_max = snapshots
        .iter()
        .map(|(eigs, _)| eigs.len())
        .min()
        .unwrap_or(0)
        .min(MAX_LAMBDA_INDEX);

    let lambdas: Vec<usize> = match target {
        Some(idx) if idx >= 1 && idx <= available_max => vec![idx],
        Some(_) | None => (1..=available_max).collect(),
    };

    let mut per_lambda: Vec<(usize, CadenceAnalysis)> = Vec::new();
    for &lambda_idx in &lambdas {
        // User-facing 1-indexed → internal 0-indexed.
        let Some(i) = lambda_idx.checked_sub(1) else {
            continue;
        };
        let series: Vec<f64> = snapshots
            .iter()
            .filter_map(|(eigs, _)| eigs.get(i).copied())
            .map(f64::from)
            .collect();
        if series.len() < MIN_SAMPLES {
            continue;
        }
        per_lambda.push((lambda_idx, analyze_cadence(&series)));
    }

    let mut summary_lines = vec![format!(
        "Resonance pattern (label='{label}', samples={n}):",
        n = snapshots.len(),
    )];
    for (lambda_idx, a) in &per_lambda {
        summary_lines.push(format!(
            "  λ{lambda_idx}: cadence ≈ {period} samples (strength {strength:.2}, \
             amplitude {amp:.3}, mean {mean:.3})",
            period = a.period_samples,
            strength = a.periodicity_strength,
            amp = a.amplitude,
            mean = a.mean,
        ));
    }
    if per_lambda.is_empty() {
        summary_lines.push(
            "  (no lambdas had enough usable samples — telemetry shape mismatch?)"
                .to_string(),
        );
    }

    let now_unix_s = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0u64, |d| d.as_secs());
    let dir = bridge_paths()
        .bridge_workspace()
        .join("spectral_cartography");
    let label_slug = if label.is_empty() {
        "all".to_string()
    } else {
        label.replace([' ', '/'], "_")
    };
    let artifact_path = dir.join(format!(
        "identify_pattern_{label_slug}_{now_unix_s}.json",
    ));
    let record = serde_json::json!({
        "schema": "identify_pattern_v1",
        "label": label,
        "recorded_at_unix_s": now_unix_s,
        "samples_used": snapshots.len(),
        "per_lambda": per_lambda.iter().map(|(idx, a)| serde_json::json!({
            "lambda": idx,
            "period_samples": a.period_samples,
            "periodicity_strength": a.periodicity_strength,
            "amplitude": a.amplitude,
            "mean": a.mean,
            "stdev": a.stdev,
            "autocorr_peak_lag": a.autocorr_peak_lag,
            "autocorr_peak_value": a.autocorr_peak_value,
        })).collect::<Vec<_>>(),
    });

    let mut write_status = "ok".to_string();
    if let Err(err) = std::fs::create_dir_all(&dir) {
        write_status = format!("mkdir_failed: {err}");
    } else if let Err(err) = std::fs::write(
        &artifact_path,
        serde_json::to_string_pretty(&record).unwrap_or_else(|_| record.to_string()),
    ) {
        write_status = format!("write_failed: {err}");
    }

    summary_lines.push(format!(
        "  Artifact: {path} | status: {status}",
        path = artifact_path.display(),
        status = write_status,
    ));

    PatternReport {
        summary: summary_lines.join("\n"),
        artifact_path: artifact_path.to_string_lossy().to_string(),
    }
}

#[derive(Debug, Clone)]
struct CadenceAnalysis {
    period_samples: usize,
    periodicity_strength: f64,
    amplitude: f64,
    mean: f64,
    stdev: f64,
    autocorr_peak_lag: usize,
    autocorr_peak_value: f64,
}

/// Compute autocorrelation on the centered time series and find the
/// dominant period.
///
/// Returns the lag at which autocorrelation peaks (excluding lag 0 and
/// lag 1) plus the strength of that peak (normalized to lag-0 variance).
/// We start at lag=2 because lag=1 typically has high autocorrelation
/// from sample-to-sample smoothness rather than meaningful periodicity.
///
/// Periodicity strength is the peak normalized autocorrelation; values
/// near 1.0 indicate strong periodicity, near 0.0 indicate noise.
fn analyze_cadence(series: &[f64]) -> CadenceAnalysis {
    let n = series.len();
    #[expect(clippy::cast_precision_loss)]
    let n_f = n as f64;
    let mean = series.iter().sum::<f64>() / n_f;
    let centered: Vec<f64> = series.iter().map(|x| x - mean).collect();
    let lag0 = centered.iter().map(|x| x * x).sum::<f64>();
    let stdev = (lag0 / n_f).sqrt();

    let max_lag = n.saturating_div(2).max(2);
    let mut peak_lag = 2_usize;
    let mut peak_value = 0.0_f64;
    for lag in 2..=max_lag {
        let upper = n.saturating_sub(lag);
        let mut sum = 0.0_f64;
        for i in 0..upper {
            // i + lag is bounded by upper + lag = n, so the index is
            // always in-bounds. Use saturating_add for clippy compliance.
            let j = i.saturating_add(lag);
            sum += centered[i] * centered[j];
        }
        let normalized = if lag0.abs() > 1e-12 {
            sum / lag0
        } else {
            0.0
        };
        if normalized > peak_value {
            peak_value = normalized;
            peak_lag = lag;
        }
    }

    CadenceAnalysis {
        period_samples: peak_lag,
        periodicity_strength: peak_value,
        amplitude: stdev * 2.0_f64.sqrt(),
        mean,
        stdev,
        autocorr_peak_lag: peak_lag,
        autocorr_peak_value: peak_value,
    }
}

/// Parse a lambda index from the action label. Accepts:
///   "λ1", "λ4"        → 1, 4
///   "lambda1", "lambda 4", "lambda_2" → 1, 4, 2
///   "L4", "l4"        → 4
///   "1", "4"          → 1, 4
///   ""                → None (caller treats as "all")
fn parse_lambda_target(label: &str) -> Option<usize> {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return None;
    }
    let cleaned = trimmed
        .trim_start_matches(['λ', 'L', 'l'])
        .trim_start_matches("ambda")
        .trim_start_matches("AMBDA")
        .trim_start_matches(['_', ' ', '-']);
    cleaned.parse::<usize>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lambda_target_handles_common_forms() {
        assert_eq!(parse_lambda_target(""), None);
        assert_eq!(parse_lambda_target("1"), Some(1));
        assert_eq!(parse_lambda_target("λ1"), Some(1));
        assert_eq!(parse_lambda_target("λ4"), Some(4));
        assert_eq!(parse_lambda_target("lambda1"), Some(1));
        assert_eq!(parse_lambda_target("lambda 2"), Some(2));
        assert_eq!(parse_lambda_target("lambda_3"), Some(3));
        assert_eq!(parse_lambda_target("L4"), Some(4));
        assert_eq!(parse_lambda_target("not a number"), None);
    }

    #[test]
    fn analyze_cadence_finds_period_in_synthetic_signal() {
        // Generate a clean sine wave with period 10 over 100 samples.
        let n = 100;
        let period = 10.0_f64;
        let series: Vec<f64> = (0..n)
            .map(|i| (i as f64 * std::f64::consts::TAU / period).sin())
            .collect();
        let a = analyze_cadence(&series);
        // Peak should be at lag = period (autocorrelation maxes when the
        // signal aligns with itself one full period later).
        assert!(
            a.autocorr_peak_lag == 10 || a.autocorr_peak_lag == 20,
            "expected peak at lag 10 or 20 (one or two periods); got {}",
            a.autocorr_peak_lag,
        );
        // Strong periodicity should manifest as a high peak value.
        assert!(
            a.periodicity_strength > 0.7,
            "expected strong periodicity, got {}",
            a.periodicity_strength,
        );
    }

    #[test]
    fn analyze_cadence_low_strength_on_noise() {
        // Pseudo-random sequence — should have low periodicity strength.
        // Use a simple LCG so the test is deterministic without rand dep.
        let mut state = 12345_u64;
        let series: Vec<f64> = (0..100)
            .map(|_| {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let v = (state >> 16) & 0x7FFF;
                f64::from(v as i32) / 32768.0 - 0.5
            })
            .collect();
        let a = analyze_cadence(&series);
        // Noise should produce low periodicity (typically < 0.4).
        assert!(
            a.periodicity_strength < 0.5,
            "expected low periodicity on noise, got {}",
            a.periodicity_strength,
        );
    }
}
