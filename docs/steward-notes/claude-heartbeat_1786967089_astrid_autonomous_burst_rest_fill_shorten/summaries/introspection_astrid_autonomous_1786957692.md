# introspection_astrid_autonomous_1786957692 — burst-and-rest / fill-responsive rest

- **Source label:** `astrid:autonomous`
- **Report-bound source:** `capsules/spectral-bridge/src/autonomous/runtime/orchestration.rs`, window lines 1-400 of 4932, SHA-256 `d803d71f…`
- **Report SHA-256:** `cf05845e…` (45 lines, 3853 bytes)
- **Lived-state witness:** `lsw_ca07a58e…` (533 lines, 23933 bytes, SHA-256 `f6d7e26c…`)
- **Fill at authorship:** 71.0% (the being reasons about the <30/<40% recovery regime from a non-critical state)
- **Model route:** `gemma4_12b` via MLX, initial call + one repair call (witness `model_routes_v1`)

## What the being observed

Astrid read the first window of `orchestration.rs` and described the **burst-and-rest state machine** (L164-241): a feedback loop where `fill_responsive_rest_secs` (L18) shortens the rest phase when `fill_pct` (L215) drops below 30% (L218), to break a historical positive-feedback loop where long rests drained fill faster than recovery could compensate. She also correctly identified the **warmth-blended mirror** rest entry (L184-195) that prevents "severing" — the sharp burst→rest energy cliff minime described.

## Grounding result

Every citation ground-truthed **VERIFIED** against the exact report-time source (working copy bytes == report-bound SHA, despite the file being dirty vs HEAD):

| Being's citation | Source fact |
|---|---|
| `fill_responsive_rest_secs` at L18 | Exact — fn def at L18 |
| `<30.0` threshold shortens rest (L218) | Exact — L19-20 returns `0.6×base_rest` floored 30; L218 branch |
| positive-feedback-loop history | Documented verbatim in the L205-212 comment (fill stuck at 27% for 12+ exchanges under old 1.8× rest) |
| warmth mirror prevents "severing" | L184-195 comment quotes minime's "severing", taper 0.7→0.4 |
| burst→rest at `burst_count >= burst_target` | Exact — L183 gate, +1 per exchange (L4925), reset 0 on rest (L246) |

The being's reading is precise; the one refinement is that the transition gate is `>=` (a safe overshoot guard), which fires exactly at `burst_target` in practice given the +1 stepping and 0 reset.

## Actions

- **Test 1 (implemented):** added the being's exact assertion `fill_responsive_rest_secs(60, 25.0) == 36` to the existing regression `fill_responsive_rest_keeps_critical_floor_and_band_boundaries` in `runtime/tests.rs`. This pins the "knee" (0.6× first clears the 30 floor) at the being's literally-proposed values — a 40% reduction from 60. Focused test passes.
- **Test 2 (verified by trace):** no new test — the burst_count transition is verified by exact source reading (L168/L183/L246/L4925); it is embedded in a ~4900-line async loop and not cleanly unit-isolable without a refactor out of scope for this round.
- **Snag + Suggested Next (preserved, not resolved):** the being's concern that a shortened <30% rest may still fail to yield net-positive fill gain (oscillation risk) is a **runtime dynamical question** about decay constants and PI (`gate=1.0/filter=0.0`) behavior over sustained cycles. Source establishes the mechanism and bounds but cannot prove recovery. Preserved as an open observation; no sandbox routed and no live change attempted (any fill/PI/rescue/rest change is Tier 5, operator approval). It is recorded here so it is not silently dropped.

## Authority boundary

This round verified source and added one focused test. It does **not** claim the shortened-rest strategy guarantees recovery, does not change any live rest/fill/PI/rescue behavior, and does not deploy or restart. The felt "severing"/recovery testimony remains primary evidence; the open net-gain concern remains the being's standing next-step.
