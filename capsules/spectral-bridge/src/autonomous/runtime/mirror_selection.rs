//! Journal start selection only; independent of rest timing and sensory policy.
use rand::{Rng, SeedableRng, rngs::StdRng};

pub(super) fn start_index(seed: u64, entries: usize) -> usize {
    if entries < 5 {
        return 0;
    }
    let mut rng = StdRng::seed_from_u64(seed);
    // Branch probabilities, not final-entry probabilities: both sampled windows
    // also contain index zero. Subsequent unreadable-entry fallback is unchanged.
    match rng.gen_range(0_u8..10) {
        0..=6 => 0,
        7..=8 => rng.gen_range(0..entries.min(20)),
        _ => rng.gen_range(0..entries),
    }
}

#[cfg(test)]
mod tests {
    use super::start_index;

    #[test]
    fn empty_and_small_histories_keep_the_existing_start() {
        for entries in 0..5 {
            for seed in [0, 1, 1_791_326_183_000_000_000, u64::MAX] {
                assert_eq!(start_index(seed, entries), 0);
            }
        }
    }

    #[test]
    fn timestamp_seeds_reach_recent_and_deep_history_deterministically() {
        let mut newest = 0;
        let mut recent = 0;
        let mut deep = 0;
        for offset in 0_u64..10_000 {
            let seed = 1_791_326_183_000_000_000_u64.wrapping_add(offset);
            let index = start_index(seed, 100);
            assert_eq!(index, start_index(seed, 100));
            assert!(index < 100);
            match index {
                0 => newest += 1,
                1..=19 => recent += 1,
                _ => deep += 1,
            }
        }
        // Broad fixed-seed sanity bounds detect the former always-newest bug;
        // they do not claim uniform clock seeds or measured live probabilities.
        assert!((6_500..8_000).contains(&newest), "{newest}");
        assert!((1_500..2_700).contains(&recent), "{recent}");
        assert!((500..1_200).contains(&deep), "{deep}");
    }

    #[test]
    fn extreme_seeds_and_history_lengths_remain_in_bounds() {
        for seed in [0, 1, u64::MAX, u64::MAX >> 1] {
            for entries in [5, 19, 20, 21, 100, usize::MAX] {
                assert!(start_index(seed, entries) < entries);
            }
        }
    }
}
