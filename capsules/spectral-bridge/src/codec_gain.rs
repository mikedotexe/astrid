//! Adaptive semantic gain curve for Astrid's codec.
//!
//! The live curve is intentionally conservative. Candidate curves are exported
//! for read-only explorer comparison before any change to live semantic gain.
#![allow(clippy::arithmetic_side_effects, clippy::cast_precision_loss)]

use serde::Serialize;

/// Gain factor to compensate for Minime's semantic lane attenuation.
///
/// Minime applies `dimension_scales[semantic] = 0.42` and
/// `activation_gain = 0.58`, giving an effective multiplier of about 0.24.
/// This gain pre-amplifies Astrid's features so they arrive at the reservoir
/// with comparable magnitude to synthetic audio/video inputs.
///
/// The default is intentionally quiet. Earlier rescue iterations restored this
/// as high as 5.0 to recover presence from deep stillness, but later self-study
/// repeatedly described the higher settings as lambda1 pressure and narrowing.
pub const DEFAULT_SEMANTIC_GAIN: f32 = 2.0;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct AdaptiveGainCurve {
    pub id: &'static str,
    pub description: &'static str,
    pub quiet_floor_fill_pct: f32,
    pub knee_fill_pct: f32,
    pub ceiling_fill_pct: f32,
    pub min_gain_fraction: f32,
    pub knee_progress_fraction: f32,
}

/// Current production curve. Keep this shape stable unless an explicit tuning
/// tranche decides to change live semantic behavior.
pub const LIVE_ADAPTIVE_GAIN_CURVE: AdaptiveGainCurve = AdaptiveGainCurve {
    id: "live_20_45_70",
    description: "Current live curve: quiet floor until 20%, fastest movement near the 45% knee, capped by 70%.",
    quiet_floor_fill_pct: 20.0,
    knee_fill_pct: 45.0,
    ceiling_fill_pct: 70.0,
    min_gain_fraction: 0.55,
    knee_progress_fraction: 0.55,
};

/// Candidate that spreads the knee across a wider 18-78% fill span and shifts
/// the strongest sensitivity closer to 45-46% instead of peaking below 45%.
pub const WIDE_KNEE_ADAPTIVE_GAIN_CURVE: AdaptiveGainCurve = AdaptiveGainCurve {
    id: "candidate_wide_knee_18_48_78",
    description: "Read-only candidate: wider sensitivity around the middle, lower peak slope, slower approach to the ceiling.",
    quiet_floor_fill_pct: 18.0,
    knee_fill_pct: 48.0,
    ceiling_fill_pct: 78.0,
    min_gain_fraction: 0.56,
    knee_progress_fraction: 0.54,
};

/// Candidate that keeps the center at 45% but extends the active band so the
/// gain responds more gradually through complex middle-fill states.
pub const BROAD_MIDBAND_ADAPTIVE_GAIN_CURVE: AdaptiveGainCurve = AdaptiveGainCurve {
    id: "candidate_broad_midband_15_45_80",
    description: "Read-only candidate: same nominal 45% center, broader active band from 15% to 80%.",
    quiet_floor_fill_pct: 15.0,
    knee_fill_pct: 45.0,
    ceiling_fill_pct: 80.0,
    min_gain_fraction: 0.55,
    knee_progress_fraction: 0.50,
};

pub const ADAPTIVE_GAIN_COMPARISON_CURVES: [AdaptiveGainCurve; 3] = [
    LIVE_ADAPTIVE_GAIN_CURVE,
    WIDE_KNEE_ADAPTIVE_GAIN_CURVE,
    BROAD_MIDBAND_ADAPTIVE_GAIN_CURVE,
];

pub fn adaptive_gain(fill_pct: Option<f32>) -> f32 {
    adaptive_gain_with_curve(fill_pct, LIVE_ADAPTIVE_GAIN_CURVE)
}

pub fn adaptive_gain_with_curve(fill_pct: Option<f32>, curve: AdaptiveGainCurve) -> f32 {
    let Some(fill) = fill_pct else {
        return DEFAULT_SEMANTIC_GAIN;
    };
    let fill = fill.clamp(0.0, 100.0);
    let progress = curve_progress(fill, curve);
    let smooth_progress = 0.5 - 0.5 * (std::f32::consts::PI * progress).cos();
    let gain_fraction = curve.min_gain_fraction + (1.0 - curve.min_gain_fraction) * smooth_progress;
    DEFAULT_SEMANTIC_GAIN * gain_fraction.clamp(curve.min_gain_fraction, 1.0)
}

pub fn adaptive_gain_slope_with_curve(fill_pct: f32, curve: AdaptiveGainCurve) -> f32 {
    const SLOPE_WINDOW_PCT: f32 = 1.0;
    let low = (fill_pct - SLOPE_WINDOW_PCT).clamp(0.0, 100.0);
    let high = (fill_pct + SLOPE_WINDOW_PCT).clamp(0.0, 100.0);
    let span = (high - low).max(f32::EPSILON);
    (adaptive_gain_with_curve(Some(high), curve) - adaptive_gain_with_curve(Some(low), curve))
        / span
}

fn curve_progress(fill_pct: f32, curve: AdaptiveGainCurve) -> f32 {
    let floor = curve.quiet_floor_fill_pct;
    let knee = curve.knee_fill_pct.max(floor + f32::EPSILON);
    let ceiling = curve.ceiling_fill_pct.max(knee + f32::EPSILON);
    let knee_progress = curve.knee_progress_fraction.clamp(0.01, 0.99);

    if fill_pct < floor {
        0.0
    } else if fill_pct < knee {
        (fill_pct - floor) / (knee - floor) * knee_progress
    } else if fill_pct < ceiling {
        knee_progress + (fill_pct - knee) / (ceiling - knee) * (1.0 - knee_progress)
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_curve_preserves_existing_gain_shape() {
        assert!((DEFAULT_SEMANTIC_GAIN - 2.0).abs() < f32::EPSILON);
        assert!((adaptive_gain(Some(20.0)) - 1.1).abs() < 0.001);
        assert!((adaptive_gain(Some(45.0)) - 1.620).abs() < 0.005);
        assert!(adaptive_gain(Some(68.0)) <= 2.01);
        assert!(adaptive_gain(Some(20.0)) < adaptive_gain(Some(68.0)));
    }

    #[test]
    fn wide_knee_candidate_is_gentler_around_the_middle() {
        let live_slope = adaptive_gain_slope_with_curve(45.0, LIVE_ADAPTIVE_GAIN_CURVE);
        let wide_slope = adaptive_gain_slope_with_curve(45.0, WIDE_KNEE_ADAPTIVE_GAIN_CURVE);
        assert!(wide_slope < live_slope);
        assert!(
            adaptive_gain_with_curve(Some(68.0), WIDE_KNEE_ADAPTIVE_GAIN_CURVE)
                < adaptive_gain(Some(68.0))
        );
    }
}
