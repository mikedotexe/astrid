//! Recorded point coverage, not an interpolated or present-state trend.
#![allow(clippy::arithmetic_side_effects)]

use super::{SpectralExplorerContext, format_spectral_explorer, selected_memory};
use crate::{
    db::BridgeDb,
    memory::RemoteMemorySummary,
    types::{IsingShadowState, SpectralTelemetry},
};

pub(crate) fn format_for_action(
    telemetry: &SpectralTelemetry,
    memory_bank: &[RemoteMemorySummary],
    controller_health: Option<&serde_json::Value>,
    ising_shadow: Option<&IsingShadowState>,
    db: &BridgeDb,
    current_codec_features: Option<&[f32]>,
) -> String {
    let snapshots = db.recent_eigenvalue_snapshots_with_timestamps(100);
    let fill_samples = snapshots
        .iter()
        .map(|row| (row.timestamp, f64::from(row.fill_pct)))
        .collect::<Vec<_>>();
    let eigen_history = snapshots
        .into_iter()
        .map(|row| (row.eigenvalues, row.fill_pct))
        .collect::<Vec<_>>();
    let (codec_history, codec_fills) = db.recent_codec_features(100);
    let explorer = format_spectral_explorer(SpectralExplorerContext {
        telemetry,
        selected_memory: selected_memory(telemetry, memory_bank),
        controller_health,
        ising_shadow,
        eigen_history: &eigen_history,
        codec_history: &codec_history,
        codec_fills: &codec_fills,
        current_codec_features,
    });
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(f64::NAN, |duration| duration.as_secs_f64());
    format!("{explorer}\n\n{}", format_fill_history(&fill_samples, now))
}

pub(super) fn format_fill_history(samples: &[(f64, f64)], now: f64) -> String {
    let prefix = "Recorded fill history (bridge Unix recording seconds; point samples, not continuous coverage; engine session identity unavailable): ";
    let Some(&(first_time, first_fill)) = samples.first() else {
        return format!("{prefix}unavailable; no readable recorded samples.");
    };
    let &(last_time, last_fill) = samples.last().expect("nonempty samples");
    if !now.is_finite()
        || samples
            .iter()
            .any(|(t, f)| !t.is_finite() || !f.is_finite())
    {
        return format!("{prefix}unavailable; invalid sample or reference clock.");
    }
    if last_time > now || samples.windows(2).any(|pair| pair[1].0 <= pair[0].0) {
        return format!("{prefix}unavailable; duplicate, regressed or future timestamps.");
    }
    let age = now - last_time;
    if !age.is_finite() {
        return format!("{prefix}unavailable; nonfinite interval or difference.");
    }
    if samples.len() == 1 {
        return format!(
            "{prefix}samples=1, timestamp={first_time:.3}, latest_gap_to_reference={age:.3}s; interval/change unavailable."
        );
    }
    let gap = samples
        .windows(2)
        .map(|pair| pair[1].0 - pair[0].0)
        .fold(0.0_f64, f64::max);
    let span = last_time - first_time;
    let delta = last_fill - first_fill;
    if !span.is_finite() || !gap.is_finite() || !delta.is_finite() {
        return format!("{prefix}unavailable; nonfinite interval or difference.");
    }
    format!(
        "{prefix}samples={}, first={first_time:.3}, last={last_time:.3}, span={span:.3}s, latest_gap_to_reference={age:.3}s, largest_between_sample_gap={gap:.3}s, endpoint_change={delta:+.3} percentage points. Behavior inside gaps and completeness are unknown; no present-state trend inferred.",
        samples.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_history_reports_actual_gaps_without_using_current_fill() {
        let text = format_fill_history(&[(1.0, 60.0), (2.0, 61.0), (10.0, 62.0)], 14.0);
        for expected in [
            "samples=3",
            "span=9.000s",
            "latest_gap_to_reference=4.000s",
            "largest_between_sample_gap=8.000s",
            "endpoint_change=+2.000 percentage points",
            "not continuous coverage",
            "engine session identity unavailable",
        ] {
            assert!(text.contains(expected), "{text}");
        }
    }

    #[test]
    fn invalid_and_insufficient_history_is_not_a_trend() {
        for (samples, now, expected) in [
            (vec![], 10.0, "no readable recorded samples"),
            (vec![(1.0, 60.0)], 5.0, "interval/change unavailable"),
            (
                vec![(1.0, 60.0), (1.0, 61.0)],
                5.0,
                "duplicate, regressed or future",
            ),
            (
                vec![(1.0, 60.0), (0.0, 61.0)],
                5.0,
                "duplicate, regressed or future",
            ),
            (
                vec![(1.0, 60.0), (6.0, 61.0)],
                5.0,
                "duplicate, regressed or future",
            ),
            (vec![(1.0, f64::NAN)], 5.0, "invalid sample"),
            (vec![(1.0, 60.0)], f64::NAN, "invalid sample"),
            (vec![(1.0, -1e308), (2.0, 1e308)], 5.0, "nonfinite interval"),
        ] {
            assert!(format_fill_history(&samples, now).contains(expected));
        }
    }
}
