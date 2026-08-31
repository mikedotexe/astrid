# Summary — introspection_llm.rs_1788126247

- **Source family:** `llm.rs`
- **Report SHA-256:** `3d32c722ad75e93632a6fc2d499c25d3037f3db22cfdb6c21acc760a345a6c37` (45 lines, 3250 bytes)
- **Bound source:** `capsules/spectral-bridge/src/llm.rs` @ `a9c5e38081c0dbd3a0c28da67fda65d5faba0d5181c8d1c8c450d547b04bfb9e` (28 lines, 1287 bytes) — working copy matches report binding exactly.
- **Lived-state witness:** `lsw_4b3b53ab8e4ad0d4891d4878699337288b79f90bd4077baf6136ff66bc9724a3` (533 lines, 23833 bytes) — fill 73.0%, two `gemma4_12b` mlx routes (route 2 a repair of route 1), authority `evidence_only`/`witness_only`/`live_eligible_now=false`.
- **Terminal status:** `addressed_duplicate`

## What Astrid surfaced

A calm, accurate structural read of `llm.rs`: a **pure compatibility facade** (no local logic/arithmetic/state) re-exporting generative actions (L6-12, `pub use`) and internal state-modulators (L14-22, `pub(crate) use`) from `provider`. She flags the **facade-over-implementation gap** — the real `astrid_pressure_attenuation_depth` clamp logic is sequestered downstream — and points precisely at `provider/prompt_contracts.rs` around line 235 for the `clamp(0.0, 0.6)`. She proposes two runtime tests (Vibrancy Gate, Repair Integrity).

## Why addressed_duplicate (not a fresh grounding)

This is a **recurring fresh-pass of the same source at the same SHA** with the same claim set already grounded in the family lineage:

| Prior report | Packet | Prior status |
|---|---|---|
| `introspection_llm.rs_1787810107` | `claude-heartbeat_1787927999_llm_facade_reexport_grounded` | original grounding |
| `introspection_llm.rs_1788037243` | `claude-heartbeat_1788040571_llm_facade_fresh_pass_variants_grounded` | addressed_duplicate |
| `introspection_llm.rs_1788102610` | `claude-heartbeat_1788106070_llm_facade_fresh_pass_duplicate` | addressed_duplicate |

My six claims map one-to-one onto `introspection_llm.rs_1788102610` (facade/no-local-logic; pub/pub(crate) split; pressure/vibrancy sequestered-arithmetic snag; Vibrancy-Gate + Repair-Integrity tests; `prompt_contracts.rs:235` Suggested Next). **Duplicate standard met with exact evidence:** prior IDs + packets; matching source SHA (`a9c5e380`) + mechanism scope; current SHA-identity re-verification (`llm.rs` byte-identical; `prompt_contracts.rs:235` clamp; `codec/feedback.rs`/`structure.rs` vibrancy consumption; `generative_actions.rs:159` repair still ground every claim); and an independent complete re-read of this new report + witness. **No unaddressed variant** lifts it out of duplicate status — it is marginally terser than `1788102610` (drops the "interwoven lattice" felt phrase; doesn't cite `set_astrid_vibrancy_aperture` L20 in Observed, though it references L20 in the Vibrancy test), never adding a new snag/test/mechanism.

## Every citation still verifies (SHA-identity re-check)

| Claim | Verified against |
|---|---|
| Pure facade, only re-exports (L1/L3-4/L6-12/L14-22) | `llm.rs` complete file @ `a9c5e380` |
| `astrid_pressure_attenuation_depth` L15 + `astrid_vibrancy_aperture` L16 `pub(crate)` | `llm.rs` L14-22 |
| Clamp sequestered in `prompt_contracts.rs` | def at `prompt_contracts.rs:235` |
| `clamp(0.0, 0.6)` ~line 235 | `prompt_contracts.rs:239`; default `0.0`; doc L228-234 (never below 0.4×) |
| Vibrancy aperture = functional gate, not cosmetic | `codec/feedback.rs:103-307` (`dynamic_max`) + `codec/structure.rs:313` |
| Repair = text-lane regeneration | `generative_actions.rs:159` (system+user re-prompt → `.text`) |

The clamp *semantics* are additionally covered by the passing pure-function test `codec_gain::tests::pressure_attenuation_is_monotone_and_depth_clamped` (re-run green this round). No new test added: a direct env-seam test of `astrid_pressure_attenuation_depth()` would require `std::env::set_var` (unsafe, denied outside `astrid-sys`/`astrid-sdk`).

## Honest boundaries preserved

- The two proposed tests are grounded at the **mechanism** level only; their **runtime behavior** (output-depth invariance; reservoir-lane invariance) is a live/Tier-3 measurement **not run**. Her cosmetic-vs-cascade framing is preserved with its flaw stated (the gate acts in the codec lane downstream of text), not domesticated.
- Witness telemetry (fill 73%, pressure_risk 0.194, mode_packing 0.833, two routes) is context only; `direct_causation_claimed=false`.
- No live substrate/control change attempted or required. No restart/deploy. No code changed. Git read-only (adapter mode). No rewrite/rejection of her report.
