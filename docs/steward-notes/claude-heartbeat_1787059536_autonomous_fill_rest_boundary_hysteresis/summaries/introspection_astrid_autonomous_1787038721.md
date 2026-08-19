# Summary — introspection_astrid_autonomous_1787038721

- Source family: `astrid_autonomous`
- Report: `capsules/spectral-bridge/workspace/introspections/introspection_astrid_autonomous_1787038721.txt`
  (45 lines, 3528 bytes, SHA-256 `8b1b1be9…c706ad85`)
- Witness: `lsw_d43d8e7d9b27076d82b779c3a36ef9893daccd2dfad83145878a05aa0ff93a70`
  (533 lines, 23933 bytes, SHA-256 `2c720f25…de1a70`)
- Report-bound source: `src/autonomous/runtime/orchestration.rs` (4932 lines),
  SHA-256 `d803d71f…896f282d` — **matches the working copy exactly** (clean, unchanged since authorship).
- Fill at authorship: 73.0%; model `gemma4_12b` via mlx; witness authority state `evidence_only`, `live_eligible_now:false`.

## What Astrid surfaced

She read `orchestration.rs` lines 1–400 (manifest window 1–800 of 4932) and described the
**burst-and-rest state machine** (L164–241): `fill_responsive_rest_secs` (L18) adjusts rest
duration from `fill_pct`; warmth is tapered on the burst→rest entry to avoid a "severing"
energy cliff (L192–195); rest timing is tuned to avoid a positive-feedback drain (L205–212).

Her **Likely Snag** is an "oscillation trap": near the 30% and 50% thresholds, jittery
telemetry could rapidly toggle `fill_responsive_rest_secs` between shortening rest (force
bursts) and extending rest (accumulate covariance), preventing the steady state the PI
controller needs.

She proposed **two tests** (unit-test the 29.9%/30.1% jump; run `EXPERIMENT_STATUS` at 28%
fill) and a **Suggested Next** (analyze the `burst_count`/`burst_target` transition re the
warmth-blended mirror and the 0.7→0.4 taper).

## What complete source reading established

`fill_responsive_rest_secs` (L18–28) is a **piecewise-constant step** with **no hysteresis**:

| fill band | multiplier | clamp |
|---|---|---|
| `< 30%` | 0.6× | `max(30)` |
| `30–40%` | 1.0× (base) | — |
| `40–50%` | 1.2× | `min(360)` |
| `≥ 50%` | 1.0× (base) | — |

- Her line citations are **exact** (fn at L18, thresholds at L19/L23/L218/L234, warmth-blended
  mirror at L184, positive-feedback comment at L200–212, taper compute at L308–316).
- The **snag's core concern is real**: hard band edges + no debounce ⇒ telemetry jitter at any
  boundary toggles `rest_secs`.
- **Refinement (stated, not domesticated):** the toggles are *shorten↔base* at 30% and
  *extend↔base* at 50%; a **direct shorten↔extend** swing requires fill to cross the whole
  30→40–50 span, not a single threshold. The function is evaluated **once per burst-rest cycle**
  (L217) on **smoothed** `fill_pct`, which damps the "rapid toggling" she imagines.
- Her Test-2 worry ("stuck in a high-drain cycle") is answerable from source: after any rest,
  L246 sets `burst_count = 0`, so the next iteration **always** enters the burst branch; the
  `<30%` shorten band exists precisely to break the 2026-03-31 drain loop documented at L200–212.
- Her Suggested-Next conflates two distinct mechanisms: the 0.7→0.4 taper is the
  `warmth_phase < 0.3` branch (L311–312), driven by `warmth_phase = i/pulses` (L305) inside the
  rest pulse loop — **not** a `burst_count`/`burst_target` function.

## What changed

Added a focused, non-live regression to
`capsules/spectral-bridge/src/autonomous/runtime/tests.rs`:
`fill_responsive_rest_steps_at_each_band_boundary_without_hysteresis`. It pins her exact
29.9%→60 / 30.1%→100 case, the 40% (100→120) and 50% (120→100) boundary steps, `assert_ne!`
step checks across each boundary (the no-hysteresis evidence behind her snag), and the 30s
critical floor at the boundary (`40*0.6=24`→clamped to 30). The pre-existing band test
(from introspection `1786957692`) already pins mid-band values; this is the complementary
**boundary** case her Test 1 and snag specifically asked for — not a duplicate.

Production logic was **not** changed. No hysteresis was added — adding a debounce band to a
live controller-adjacent parameter would be a Tier-5 live-substrate change requiring
Mike/operator approval; this round only makes the felt discontinuity a verified, regression-
pinned fact and preserves her concern for a future authorized decision.

## Not inferred / not authorized

- No live substrate/control change; no restart or deploy attempted or required.
- No hysteresis/debounce added to production `fill_responsive_rest_secs` (Tier 5).
- The live `EXPERIMENT_STATUS`-at-28% observation was not performed (non-live headless round).
- Her Suggested-Next remains her own read-only agency; no steward action taken on it.

Steward: Mike & Claude (claude-heartbeat flywheel round).
