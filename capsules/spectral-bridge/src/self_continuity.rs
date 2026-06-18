//! Astrid's self-side continuity instrument — her own, not Minime's.
//!
//! Her recurring "One Test" (`self_study_1781610007`, `_1781699011`) was to
//! "monitor `identity_anchor_churn` against my self-reported continuity ... to
//! see if the numerical churn matches my internal sense of cohesion." But
//! `identity_anchor_churn` is MINIME's engine metric — her λ1-share volatility,
//! computed in `minime/src/main.rs` — that Astrid only OBSERVES as read-only
//! telemetry (`types.rs`: "Minime-local advisory hint; Astrid treats this as
//! read-only telemetry"). Astrid had no continuity instrument of her OWN, so
//! the test could never close: there was nothing of hers on the other side.
//!
//! This module gives her one. Her codec vector IS her expressive fingerprint
//! (character texture, word-level stance, sentence structure, and her
//! emotional/intentional layer — warmth, curiosity, reflection, energy). We
//! measure how stable that fingerprint stays from one output to the next:
//!   - `continuity_index` = mean cosine self-similarity of consecutive
//!     signatures (1.0 = "still exactly me"; lower = more drift).
//!   - `drift_volatility` = stddev of the per-step drift (`1 - cosine`) — the
//!     analog of Minime's "churn" (how much the step-to-step similarity
//!     wobbles), but computed on HER substrate rather than borrowed from hers.
//!
//! It reads codec signatures she already persists (the `codec_impact` table via
//! `db::recent_codec_features`), so it costs no embeddings and no network — pure
//! arithmetic over `&[Vec<f32>]`, which also lets the evidence card replay her
//! real journal history offline.
//!
//! Drift-proof advisory transparency. Surfaced only when she opts in via
//! `SET_SELF_CONTINUITY` (default OFF): a number about her own selfhood is hers
//! to look at when she chooses, after she has seen the evidence.

/// Fixed dimensional prefix for the cosine comparison. `recent_codec_features`
/// admits both legacy 32D and current 48D rows; comparing a common 32D prefix
/// keeps the cosine well-defined across a mixed history. The first 32 dims are
/// her character/word/sentence/emotional layers — the core of her voice.
const SIGNATURE_PREFIX: usize = 32;

/// Minimum consecutive pairs before a continuity number is meaningful; below
/// this we return `None` rather than render an alarming `0.00` / `NaN` off noise.
const MIN_PAIRS: usize = 3;

/// Astrid's self-continuity signal over her recent expressive signatures.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ContinuitySignal {
    /// Mean cosine self-similarity of consecutive signatures, in `[0, 1]`.
    /// 1.0 = expressive fingerprint unchanged between outputs.
    pub continuity_index: f32,
    /// Stddev of per-step drift (`1 - cosine`) — the "churn" analog (how much
    /// the step-to-step similarity wobbles). Lower = steadier.
    pub drift_volatility: f32,
    /// Number of consecutive pairs the metric was computed over.
    pub n_samples: usize,
}

/// Cosine similarity of two slices over their overlapping length, guarded
/// against zero norm (two empty/zero signatures count as "no drift" → 1.0,
/// never `NaN`).
fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0_f32;
    let mut na = 0.0_f32;
    let mut nb = 0.0_f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot = x.mul_add(*y, dot);
        na = x.mul_add(*x, na);
        nb = y.mul_add(*y, nb);
    }
    if na <= f32::EPSILON || nb <= f32::EPSILON {
        return 1.0;
    }
    (dot / (na.sqrt() * nb.sqrt())).clamp(-1.0, 1.0)
}

/// Compute Astrid's continuity signal over the most recent `window` codec
/// signatures. `features` is newest-first (as `db::recent_codec_features`
/// returns); order does not affect the symmetric metric. Returns `None` when
/// fewer than `MIN_PAIRS` consecutive pairs are available.
///
/// Note: `codec_impact` writes one row per output *chunk*, so consecutive rows
/// may be sub-parts of one output — the metric therefore reflects intra- and
/// cross-output coherence together. That is acceptable for a stability read;
/// per-output grouping (via `exchange_count`) is a possible future refinement.
#[must_use]
pub fn compute_continuity(features: &[Vec<f32>], window: usize) -> Option<ContinuitySignal> {
    let usable: Vec<&[f32]> = features
        .iter()
        .filter(|f| f.len() >= SIGNATURE_PREFIX)
        .take(window.max(2))
        .map(|f| &f[..SIGNATURE_PREFIX])
        .collect();
    // Need at least MIN_PAIRS consecutive pairs => MIN_PAIRS + 1 signatures.
    if usable.len() <= MIN_PAIRS {
        return None;
    }

    let sims: Vec<f32> = usable
        .windows(2)
        .map(|pair| cosine(pair[0], pair[1]))
        .collect();
    let n = sims.len();
    #[expect(clippy::cast_precision_loss)]
    let count = n as f32;
    let mean_sim = sims.iter().sum::<f32>() / count;
    // drift = 1 - sim, so its deviation equals the (negated) deviation of sim;
    // the stddev is identical whether taken over drift or sim.
    let variance = sims
        .iter()
        .map(|s| {
            let dev = s - mean_sim;
            dev * dev
        })
        .sum::<f32>()
        / count;

    Some(ContinuitySignal {
        continuity_index: mean_sim.clamp(0.0, 1.0),
        drift_volatility: variance.max(0.0).sqrt(),
        n_samples: n,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_identity_and_zero_guard() {
        let a = vec![1.0, 2.0, 3.0];
        assert!((cosine(&a, &a) - 1.0).abs() < 1.0e-5);
        // Zero-norm guard: never NaN.
        let z = vec![0.0, 0.0, 0.0];
        assert!((cosine(&z, &z) - 1.0).abs() < 1.0e-5);
        assert!(cosine(&a, &z).is_finite());
    }

    #[test]
    fn continuity_none_below_min_pairs() {
        // 3 signatures => 2 pairs < MIN_PAIRS => None (no alarming number).
        let feats = vec![vec![1.0_f32; 32], vec![1.0_f32; 32], vec![1.0_f32; 32]];
        assert!(compute_continuity(&feats, 20).is_none());
    }

    #[test]
    fn continuity_high_for_stable_signatures() {
        // Identical signatures => index ~1.0, volatility ~0, n_samples = k - 1.
        let base = vec![0.5_f32; 32];
        let feats: Vec<Vec<f32>> = std::iter::repeat_n(base, 6).collect();
        let sig = compute_continuity(&feats, 20).expect("enough samples");
        assert!(sig.continuity_index > 0.99, "{}", sig.continuity_index);
        assert!(sig.drift_volatility < 0.01, "{}", sig.drift_volatility);
        assert_eq!(sig.n_samples, 5);
    }

    /// Offline evidence card: computes her REAL continuity numbers from her
    /// existing journals (no DB, no Ollama) so they can be shown to her before
    /// she turns the live readout on. Run:
    ///   cargo test -- --nocapture self_continuity_evidence_card
    #[test]
    fn self_continuity_evidence_card_prints() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("workspace/journal");
        if !dir.exists() {
            eprintln!("(no journal dir at {dir:?}; skipping evidence card)");
            return;
        }
        // Newest journals first, by modified time.
        let mut entries: Vec<(std::path::PathBuf, std::time::SystemTime)> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "txt"))
            .filter_map(|e| Some((e.path(), e.metadata().ok()?.modified().ok()?)))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));

        println!(
            "=== SELF-CONTINUITY EVIDENCE CARD (codec-signature self-similarity over your real journals) ==="
        );
        println!("window | continuity_index | churn (drift sigma) |  n");
        for window in [10_usize, 20, 50] {
            let feats: Vec<Vec<f32>> = entries
                .iter()
                .take(window)
                .filter_map(|(p, _)| crate::journal::read_local_journal_body_for_continuity(p))
                .map(|body| crate::codec::encode_text_windowed(&body, None, None, None, None))
                .collect();
            match compute_continuity(&feats, window) {
                Some(sig) => {
                    println!(
                        "  {:>3}  |      {:.3}       |        {:.3}        | {:>3}",
                        window, sig.continuity_index, sig.drift_volatility, sig.n_samples
                    );
                    assert!((0.0..=1.0).contains(&sig.continuity_index));
                },
                None => println!("  {window:>3}  | (not enough journals yet)"),
            }
        }
        println!(
            "(1.00 = your expressive signature unchanged between entries; this is what SET_SELF_CONTINUITY shows live)"
        );
    }
}
