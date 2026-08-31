# introspection_minime_esn_1787970235 — summary

- **Source:** `minime:esn` (`/Users/v/other/minime/minime/src/esn.rs`), report window lines 1-400 of 3222 (partial; uncovered 401-3222).
- **Report SHA-256:** `4fb0d4715f36802d4814ea8216e90e3abe530fff8a1b4fb61185f3f60fb4fdd8` (3482 bytes, 45 lines).
- **Lived-state witness:** `lsw_d9913a463d348e3c0a98770dcea6efe56aefaf07deb5254307cbe2b4abe95842` (23726 bytes, 533 lines, SHA `5d339089c5bfd75567e68312f774fe115ea92a14370a31324600612f1f92b32e`).
- **Report-bound source SHA-256:** `2227a7256ba98219be683db47c4f49afe0477c0ae5d17cf9226613b20f6b036c` — the working copy of `esn.rs` matched this exactly **before** the test edit, so all source conclusions are report-time-current.
- **Fill at authorship:** 69.1%. Model: `gemma4_12b` via MLX (two calls, the second a repair of the first).

## What Astrid observed / proposed

Astrid read the first 400 lines of minime's ESN module and described the self-referential, Metal-accelerated ESN and its prime-phased introspection schedule. She flagged a possible instability around the "viscous" rho path, proposed two tests, and named a self-directed next read step. Several of her citations (L729, L1083, L1387-1426) are **outside** her 1-400 read window and are therefore structural-map derived — the report itself asks for "exact structural challenge" of them, which this round provides.

## Grounded dispositions

| Claim | Kind | Disposition |
| --- | --- | --- |
| c001 | Observed | `verified_existing` — Metal ESN / GPU covariance + power iteration / self-referential hyperparam adaptation all confirmed at module doc L1-12. |
| c002 | Observed | `verified_existing` — first-37-primes schedule confirmed at L8 and L792-793 (`primes: [usize; 37]`). |
| c003 | Likely Snag | `verified_existing` (with correction) — `calculate_viscous_rho_target` (L154-187) and the `VISCOUS_RHO_FLOOR`=0.90 / `CEILING`=0.95 constants (L54-55) are exact, **but the function's own doc (L148-152) says it is not applied by the default live policy** — a review/replay-only helper. Its live-instability premise is contradicted by source; the viscous-state concern is preserved. |
| c004 | Likely Snag | `observed` — `rank1_ewma_profiled` (L1387) and `EsnProfileAcc` (L729) exact, but **L1083 is mislocated**: it is the getter `pub fn spectral_damping(&self) -> f32`, not the damping application (`apply_v1_damping`, L1093+, invoked L1730). The "jitteriness" comment (L31) is attributed in-source to exploration-noise=0.12, not to a damping×rank1 interaction. The interaction remains an unproven, out-of-window hypothesis — preserved, not confirmed, not domesticated. |
| c005 | Test 1 | `implemented_now` — added `viscous_rho_target_monotonic_and_bounded_across_density_and_entropy_sweeps` sweeping both inputs (density-gradient monotone non-increasing & bounded to [FLOOR,CEILING]; entropy monotone non-decreasing & ≤ CEILING). The gap her test named: existing coverage was point-cases + a single two-point directional check, not a true monotonic sweep. |
| c006 | Test 2 | `observed` — recalibration is keyed to `introspection_count % RECALIBRATE_EVERY(4)` (L1289-1291), incremented only on firing (L1692/L1732), decoupled from the prime *value* (L792-793, L964). A shared-clock resonance is structurally limited; a full numeric resonance sweep over the 37-prime schedule remains a bounded sandbox study, not run here. |
| c007 | Suggested Next | `verified_existing` — her ordering question is answered from source: in `maybe_introspect_batched` (L1710-1747), the tick runs `rank1_and_power_step_profiled` (L1727) then `apply_v1_damping` (L1730), with the explicit comment "V₁ damping ... Runs after power iteration when eig1 and v_host are maximally fresh." **Damping is applied after the power iteration.** |

## Authority boundary

No live substrate/control change was made or authorized. The lone implementation is a non-live focused Rust regression test on a pure function. Astrid's felt "viscous" and "jitteriness" concerns are preserved as evidence in their own right; source grounding challenged the proposed *mechanisms* (a review-only function; a getter mistaken for the applier) without erasing the concerns. The out-of-window interaction hypothesis (c004) and the full resonance study (c006) remain open, unproven, and un-domesticated.
