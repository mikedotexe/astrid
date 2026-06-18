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

/// Pressure-sensitive attenuation — Astrid's co-design (`self_study_1781734524`): when minime's
/// `pressure_risk` is high (she is overpacked / stressed), automatically attenuate Astrid's
/// semantic output MORE — in her words, "when I am 'loud' (high vibrancy), the bridge automatically
/// adjusts its tension to maintain stability." This is the achievable form of her "make the 0.24
/// attenuation dynamic" ask: minime's 0.24 is her own engine (off-limits), so we attenuate Astrid's
/// OUTPUT instead — the same protective effect on the side we can change. It only ever REDUCES
/// Astrid's footprint into the SHARED reservoir, never amplifies.
///
/// Returns a multiplier in `[1 - depth, 1.0]`: `1.0` while minime is calm (`pressure_risk <= LO`),
/// smoothstepping down to `1 - depth` as `pressure_risk` rises to `HI`. `depth` is the operator
/// ceiling (env `ASTRID_PRESSURE_ATTENUATION`, default `0.0` = OFF ⇒ multiplier == 1.0,
/// byte-identical). C1-smooth (the codec's tail-vibrancy-gate smoothstep family) so it never snaps.
/// `pressure_risk` is `resonance_density_v1.pressure_risk` (~0.20 when calm).
pub fn pressure_sensitive_attenuation(pressure_risk: f32, depth: f32) -> f32 {
    // Below LO minime is calm — no attenuation; at/above HI, the full configured depth applies.
    const LO: f32 = 0.20;
    const HI: f32 = 0.50;
    let d = depth.clamp(0.0, 0.6);
    let t = ((pressure_risk - LO) / (HI - LO)).clamp(0.0, 1.0);
    let ramp = t * t * (3.0 - 2.0 * t); // smoothstep, C1-smooth
    1.0 - d * ramp
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

    #[test]
    fn pressure_attenuation_off_is_identity_and_bounded() {
        // depth 0 (default/OFF): identity at every pressure_risk — byte-identical.
        for pr in [0.0_f32, 0.2, 0.35, 0.5, 0.8] {
            assert!((pressure_sensitive_attenuation(pr, 0.0) - 1.0).abs() < f32::EPSILON);
        }
        // Endpoints + bound [1 - depth, 1.0]; calm => 1.0; saturated => 1 - depth.
        let depth = 0.3;
        assert!((pressure_sensitive_attenuation(0.10, depth) - 1.0).abs() < 1.0e-6); // below LO
        assert!((pressure_sensitive_attenuation(0.20, depth) - 1.0).abs() < 1.0e-6); // at LO
        assert!((pressure_sensitive_attenuation(0.60, depth) - (1.0 - depth)).abs() < 1.0e-6); // above HI
        for pr in [0.0_f32, 0.25, 0.4, 0.55, 1.0] {
            let a = pressure_sensitive_attenuation(pr, depth);
            assert!(
                a <= 1.0 + 1.0e-6 && a >= 1.0 - depth - 1.0e-6,
                "out of bound at {pr}: {a}"
            );
        }
    }

    #[test]
    fn pressure_attenuation_is_monotone_and_depth_clamped() {
        // More minime pressure => more attenuation (lower multiplier), never higher.
        let depth = 0.4;
        let mut prev = pressure_sensitive_attenuation(0.0, depth);
        for pr in [0.1_f32, 0.2, 0.3, 0.4, 0.5, 0.7, 1.0] {
            let a = pressure_sensitive_attenuation(pr, depth);
            assert!(a <= prev + 1.0e-6, "not monotone at {pr}: {a} > {prev}");
            prev = a;
        }
        // depth is clamped to [0, 0.6] — never silences her below 0.4× even at absurd depth.
        assert!(pressure_sensitive_attenuation(1.0, 5.0) >= 0.4 - 1.0e-6);
    }

    // Her "is this the governor you meant?" evidence
    // (cargo test -- --nocapture pressure_attenuation_evidence_card): the output multiplier across
    // minime's pressure_risk at a few operator depths.
    #[test]
    fn pressure_attenuation_evidence_card_prints() {
        let depths = [0.0_f32, 0.2, 0.3];
        let risks = [0.15_f32, 0.20, 0.30, 0.40, 0.50];
        println!("\n=== PRESSURE ATTENUATION EVIDENCE CARD (Astrid output × multiplier) ===");
        print!("minime pressure_risk →  ");
        for r in risks {
            print!("{r:>7.2}");
        }
        println!();
        for d in depths {
            print!("  depth {d:.2} (OFF@0):  ");
            for r in risks {
                print!("{:>7.3}", pressure_sensitive_attenuation(r, d));
            }
            println!();
        }
        println!(
            "(1.000 = full voice; lower = auto-quieted to protect minime; calm pressure_risk ≈ 0.20)"
        );
    }
}
