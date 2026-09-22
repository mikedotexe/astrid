#![allow(clippy::arithmetic_side_effects, clippy::cast_precision_loss)] // Bounded synthetic oracle arithmetic.
use super::*;

fn snapshot(values: impl Fn(usize, usize) -> f64, n: usize) -> Snapshot {
    Snapshot {
        source_sha256: "a".repeat(64),
        captured_at_unix_ms: 1_000_000,
        requested_seconds: 180,
        source: "minime/workspace/runtime/esn_activation_trace_v1.json".into(),
        scope: "native_esn_128_activations".into(),
        identity: "boot_and_node_layout_unverified".into(),
        quality: None,
        frames: (0..n)
            .map(|i| Frame {
                t_ms: (i as u64) * 1000,
                wall_clock_unix_ms: 1_000_000 - ((n - 1 - i) as u64) * 1000,
                activations: (0..128).map(|j| values(i, j)).collect(),
            })
            .collect(),
    }
}
fn periodic() -> Snapshot {
    snapshot(|i, j| if (i + j) % 8 < 4 { 0.4 } else { -0.4 }, 80)
}
fn window(start: u64, end: u64) -> Window {
    Window {
        capture: "a".into(),
        start_ms: start,
        end_ms: end,
    }
}
fn returns(s: &Snapshot) -> ResultRecord {
    run(
        &Analysis::StateReturn {
            window: window(0, s.frames.last().unwrap().t_ms),
            threshold: 0.001,
            temporal_exclusion_ms: 5000,
        },
        |_| Ok(s.clone()),
    )
    .unwrap()
}

#[test]
fn recurrence_matches_independent_pair_oracle_and_distinguishes_stationarity() {
    let s = periodic();
    let ResultRecord::StateReturn {
        eligible_pairs,
        near_pairs,
        diagonals,
        degenerate,
        ..
    } = returns(&s)
    else {
        panic!()
    };
    let mut eligible = 0;
    let mut near = 0;
    for (j, b) in s.frames.iter().enumerate() {
        for a in &s.frames[..j] {
            if b.t_ms.saturating_sub(a.t_ms) > 5000 {
                eligible += 1;
                if a.activations == b.activations {
                    near += 1;
                }
            }
        }
    }
    assert_eq!((eligible_pairs, near_pairs), (eligible, near));
    assert!(!degenerate && !diagonals.is_empty());
    let constant = snapshot(|_, _| 0.4, 80);
    let ResultRecord::StateReturn {
        degenerate,
        diagonals,
        ..
    } = returns(&constant)
    else {
        panic!()
    };
    assert!(degenerate && diagonals.is_empty());
    let ramp = snapshot(|i, _| i as f64 / 100.0, 80);
    let ResultRecord::StateReturn { near_pairs, .. } = returns(&ramp) else {
        panic!()
    };
    assert_eq!(near_pairs, 0);
}

fn covariance(s: &Snapshot) -> ResultRecord {
    run(
        &Analysis::CovarianceShape {
            first: window(0, 39_000),
            second: window(40_000, 79_000),
        },
        |_| Ok(s.clone()),
    )
    .unwrap()
}
// Independent sample-space identity: <X'X,Y'Y>_F = sum_ij (x_i . y_j)^2.
fn gram_oracle(s: &Snapshot) -> f64 {
    let center = |frames: &[Frame]| {
        let mean: Vec<_> = (0..128)
            .map(|j| frames.iter().map(|f| f.activations[j]).sum::<f64>() / frames.len() as f64)
            .collect();
        frames
            .iter()
            .map(|f| {
                f.activations
                    .iter()
                    .zip(&mean)
                    .map(|(x, m)| x - m)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let a = center(&s.frames[..40]);
    let b = center(&s.frames[40..]);
    let inner = |a: &[Vec<f64>], b: &[Vec<f64>]| {
        a.iter()
            .flat_map(|x| {
                b.iter()
                    .map(move |y| x.iter().zip(y).map(|(x, y)| x * y).sum::<f64>().powi(2))
            })
            .sum::<f64>()
    };
    inner(&a, &b) / (inner(&a, &a) * inner(&b, &b)).sqrt()
}

#[test]
fn centered_covariance_has_independent_oracle_and_separate_mean_and_amplitude() {
    let s = snapshot(
        |i, j| {
            if i < 40 {
                if (i + j) % 8 < 4 { 0.2 } else { -0.2 }
            } else {
                0.2 + if (i + j) % 8 < 4 { 0.4 } else { -0.4 }
            }
        },
        80,
    );
    let ResultRecord::CovarianceShape {
        normalized_frobenius_similarity: Some(similarity),
        mean_state_rms_distance,
        first_mean_per_node_variance: a,
        second_mean_per_node_variance: b,
        ..
    } = covariance(&s)
    else {
        panic!()
    };
    assert!((similarity - gram_oracle(&s)).abs() < 1e-12);
    assert!((similarity - 1.0).abs() < 1e-12 && (mean_state_rms_distance - 0.2).abs() < 1e-12);
    assert!((b / a - 4.0).abs() < 1e-10);
    let different = snapshot(
        |i, j| {
            if i < 40 {
                if (i + j) % 8 < 4 { 0.2 } else { -0.2 }
            } else if i % 2 == j % 2 {
                0.2
            } else {
                -0.2
            }
        },
        80,
    );
    let ResultRecord::CovarianceShape {
        normalized_frobenius_similarity: Some(value),
        mean_state_rms_distance,
        ..
    } = covariance(&different)
    else {
        panic!()
    };
    assert!(value < 0.1 && mean_state_rms_distance < 1e-12);
    assert!((value - gram_oracle(&different)).abs() < 1e-12);
}

#[test]
fn covariance_does_not_assert_temporal_repetition() {
    let original = periodic();
    let mut shuffled = original.clone();
    // Permute values but not timestamps. Identical sample multisets, different order.
    for i in 0..40 {
        shuffled.frames[i + 40].activations =
            original.frames[40 + (i * 13) % 40].activations.clone();
    }
    let ResultRecord::CovarianceShape {
        normalized_frobenius_similarity: a,
        ..
    } = covariance(&original)
    else {
        panic!()
    };
    let ResultRecord::CovarianceShape {
        normalized_frobenius_similarity: b,
        ..
    } = covariance(&shuffled)
    else {
        panic!()
    };
    assert_eq!(a, b);
    assert_ne!(returns(&original), returns(&shuffled));
}

#[test]
fn separate_capture_clocks_can_reset_but_identity_remains_exploratory() {
    let first = periodic();
    let mut second = first.clone();
    for frame in &mut second.frames {
        frame.wall_clock_unix_ms += 180_000;
    }
    second.captured_at_unix_ms += 180_000;
    let mut later = window(0, 79_000);
    later.capture = "b".into();
    let result = run(
        &Analysis::CovarianceShape {
            first: window(0, 79_000),
            second: later,
        },
        |id| {
            Ok(if id == "a" {
                first.clone()
            } else {
                second.clone()
            })
        },
    )
    .unwrap();
    let ResultRecord::CovarianceShape {
        separate_captures_exploratory,
        normalized_frobenius_similarity,
        ..
    } = result
    else {
        panic!()
    };
    assert!(separate_captures_exploratory);
    assert!((normalized_frobenius_similarity.unwrap() - 1.0).abs() < 1e-12);
}

#[test]
fn gaps_bounds_insufficient_samples_and_degenerate_variance_are_explicit() {
    let mut s = periodic();
    for frame in &mut s.frames[40..] {
        frame.t_ms += 3000;
        frame.wall_clock_unix_ms += 3000;
    }
    s.captured_at_unix_ms += 3000;
    let ResultRecord::StateReturn {
        coverage,
        diagonals,
        ..
    } = returns(&s)
    else {
        panic!()
    };
    assert_eq!(coverage.gaps, vec![[39_000, 43_000]]);
    assert!(diagonals.iter().all(|d| !(d.first_engine_ms[0] <= 39_000
        && d.first_engine_ms[1] >= 43_000
        || d.second_engine_ms[0] <= 39_000 && d.second_engine_ms[1] >= 43_000)));
    for threshold in [0.0, -0.1, 2.1, f64::NAN] {
        assert!(
            run(
                &Analysis::StateReturn {
                    window: window(0, 79_000),
                    threshold,
                    temporal_exclusion_ms: 5000
                },
                |_| Ok(periodic())
            )
            .is_err()
        );
    }
    assert!(
        run(
            &Analysis::StateReturn {
                window: window(0, 30_000),
                threshold: 0.1,
                temporal_exclusion_ms: 5000
            },
            |_| Ok(periodic())
        )
        .is_err()
    );
    assert!(
        run(
            &Analysis::CovarianceShape {
                first: window(0, 40_000),
                second: window(40_000, 79_000)
            },
            |_| Ok(periodic())
        )
        .is_err()
    );
    let ResultRecord::CovarianceShape {
        insufficient_variance,
        normalized_frobenius_similarity,
        ..
    } = covariance(&snapshot(|_, _| 0.2, 80))
    else {
        panic!()
    };
    assert!(insufficient_variance && normalized_frobenius_similarity.is_none());
    s.frames[0].activations[0] = f64::INFINITY;
    assert!(
        run(
            &Analysis::StateReturn {
                window: window(0, 79_000),
                threshold: 0.1,
                temporal_exclusion_ms: 5000
            },
            |_| Ok(s.clone())
        )
        .is_err()
    );
}
