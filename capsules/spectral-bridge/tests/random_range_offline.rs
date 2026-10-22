//! Qualify a numerical candidate without changing the production mode policy.
#![allow(clippy::cast_precision_loss)]

#[test]
fn current_half_range_and_full_range_candidate_are_explicitly_distinct() {
    let source = include_str!("../src/autonomous/state.rs");
    for (multiplier, increment) in [
        (6_364_136_223_846_793_005_u64, 1_u64),
        (2_862_933_555_777_941_757_u64, 3_u64),
    ] {
        let spelled = if increment == 1 {
            "6_364_136_223_846_793_005"
        } else {
            "2_862_933_555_777_941_757"
        };
        assert!(
            source.contains(&format!(
                "seed.wrapping_mul({spelled}).wrapping_add({increment})) >> 33"
            )),
            "source changed: requalify rather than assuming the historical formula"
        );
        let mut old_bins = [0_u32; 10];
        let mut candidate_bins = [0_u32; 10];
        for index in 0_u64..100_000 {
            let seed = 1_792_600_000_000_000_000_u64.wrapping_add(index.wrapping_mul(104_729));
            let mixed = seed.wrapping_mul(multiplier).wrapping_add(increment);
            let current = (mixed >> 33) as f64 / f64::from(u32::MAX);
            let candidate = (mixed >> 32) as f64 / f64::from(u32::MAX);
            assert!((0.0..=0.5).contains(&current));
            assert!((0.0..=1.0).contains(&candidate));
            for (bin, count) in old_bins.iter_mut().enumerate() {
                let lower = f64::from(u32::try_from(bin).unwrap()) / 10.0;
                if current >= lower && current < lower + 0.1 {
                    *count = count.saturating_add(1);
                }
            }
            for (bin, count) in candidate_bins.iter_mut().enumerate() {
                let lower = f64::from(u32::try_from(bin).unwrap()) / 10.0;
                if candidate >= lower && candidate < lower + 0.1 {
                    *count = count.saturating_add(1);
                }
            }
        }
        assert!(old_bins[..5].iter().all(|count| *count > 19_000));
        assert!(old_bins[5..].iter().all(|count| *count == 0));
        assert!(
            candidate_bins
                .iter()
                .all(|count| (9_000..11_000).contains(count))
        );
    }
}
