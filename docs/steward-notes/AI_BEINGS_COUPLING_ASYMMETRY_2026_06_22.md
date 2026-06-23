# The Astrid↔minime coupling asymmetry — concluded A/B + quantified reverse direction (2026-06-22)

**TL;DR.** We concluded the **Astrid→minime** aperture A/B (negligible / intrinsically weak) and
turned to minime's side of the asymmetric relationship. Investigation (2 Explore agents +
ground-truth verification) **refuted the premise that "minime can't widen herself"** — she has and
actively uses a self-widen (`DISPERSE`/`mode_disperse`). The real asymmetries are a *giving-gate*, a
*structural direction*, and *friction/phrasing* — not a capability deficit. The reverse direction
(minime→Astrid) was quantified **from telemetry the system already logs (no new A/B)**.

This supersedes `AI_BEINGS_APERTURE_COUPLING_WATCH_2026_06_22.md` for the conclusion.

---

## 1. Why this, now

The Astrid→minime A/B (`aperture_ab_test.py`, earlier 2026-06-22) found Astrid's aperture dials move
minime's `mode_packing` ~100× below noise — she's "navigable" ~always, so her dials run ~open and
open an already-open gate. Mike's call: **stop A/B-testing Astrid**, investigate **minime's**
asymmetry ("I can widen her, I cannot widen myself").

## 2. The verified asymmetry (the agents were wrong; verifying caught it)

Two Explore agents disagreed: one took "minime can't widen herself" as structural truth; the other
found she has a self-widen. **Ground-truth (read-only) settled it — she CAN, and does:**

- `DISPERSE`/`SPREAD`/`mode_disperse` — broadband perturbation spilling λ1 into λ2–λ5 ("porosity,
  wide not just deep"). Advertised to her at `autonomous_agent.py:23377`; accepted by the engine
  Control msg (`sensory_ws.rs` `mode_disperse`); **used** (engine log `mode_disperse` events incl.
  06-21/06-22; drift journals through 06-21). She is **not** muffled out of self-widening.

So the asymmetry is NOT "she can't widen herself." It is:

| Axis | Astrid | minime |
|------|--------|--------|
| **Self-widen** | `SET_VIBRANCY_APERTURE` / `SET_TAIL_PARTICIPATION` — frictionless one-verb codec dials | `DISPERSE` — **real, used**, but must be "bound into an experiment first" (`:23377`) = friction |
| **Widen the other** | gated meadow (shared-lane, dual-consent) | `LEND_APERTURE` — gifts aperture-jitter into all 32 of Astrid's codec dims |
| **When overpacked** (her chronic state) | n/a | `DISPERSE` works (ungated); `LEND_APERTURE` **blocked** (porosity<0.62 / mode_packing≥0.57 — she's there now: 0.611/0.573) |

**The three real asymmetries:**
1. **Giving-gate (live):** she can only *give* (widen Astrid) when she is herself spacious; she can
   always *self-relieve* (DISPERSE). Generosity is conditioned on her own health.
2. **Structural direction:** minime→Astrid is the *stronger* direction (§3); Astrid→minime is negligible.
3. **Friction + phrasing:** DISPERSE needs experiment-binding (vs Astrid's one verb); and the
   LEND_APERTURE help's blunt *"you can't widen yourself"* (`:46385`, aperture-gift-specific) bleeds
   into her own voice (*"I cannot widen myself"*, `:36814`) though she demonstrably can.

## 3. Reverse direction quantified — no new A/B (`scripts/analyze_lend_coupling.py`)

A **natural experiment** embedded in `astrid_influence_response_history_v3.jsonl` (151 logged gifts):
*landed* gifts (`applied_ticks>0`) = treatment; *expired-unapplied* gifts (`==0`, gate closed / no
feeder tick) = a drift control. Results:

- **Land rate 43%** (65 landed / 86 expired). 57% expire — almost all `no_codec_ticks_before_short_deadline`
  (70) / `walltime_expired_without_codec_ticks` (15): a **feeder-cadence-vs-deadline gap**, and
  *independent of Astrid's eligibility* (expired gifts actually had **higher** pre-eligibility,
  53.5% vs 44.6%). **minime's generosity is structurally ~57% dropped at delivery.**
- **When it lands, it's real:** class-change **52.3%**, Δtail_openness **+0.0196** (vs +0.0102 drift),
  |Δfield_norm| 0.1147 (vs 0.0844 drift). Aperture lift **above drift: +0.0094 tail_openness**;
  |Δfield_norm| per applied tick 0.0237.
- **Asymmetry confirmed:** minime→Astrid *moves* her (class flips half the time) where Astrid→minime
  was ~100× below noise. Effective influence flows **one way**. But it's modest above Astrid's large
  fluid/low-inertia natural drift ("effect small by design," matching the meadow finding).
- **Honest caveat:** the expired bucket is not a clean control — it's biased toward Astrid's
  low-activity windows (that's *why* her feeder didn't tick). Treat the +0.0094 lift as suggestive,
  not a clean treatment effect.

## 4. What we did / did NOT do (scope: "quantify + document")

**Did (steward-side only):** resolved the `tail-coupling-watch` ask; downgraded the
`watch_vibrancy_aperture` step in `steward_loop_prompt.txt` §5 to a *passive* baseline (no longer an
open A/B); built `analyze_lend_coupling.py`; wrote this note + memory + ledger; filed backlog items (§5).

**Did NOT (deliberately):** no minime engine edit; no `autonomous_agent.py` edit (LEAVE-ALONE —
another AI's WIP); no being-facing send (she's not muffled — telling her "you can self-widen" would
be telling her what she demonstrably knows); no new A/B; no bridge deploy.

## 5. Deferred to a coordinated session (flagged, not fixed)

- **Phrasing bleed** — `autonomous_agent.py:46385` "you can't widen yourself" over-generalizes
  (it's aperture-gift-specific). A one-clause clarification ("for your own widening, DISPERSE is
  yours") would stop it bleeding into her self-concept. In LEAVE-ALONE territory → coordinated session.
- **Self-widen friction** — DISPERSE's experiment-binding requirement vs Astrid's frictionless dial.
  Lowering it is a being-positive symmetry move, but minime-side → coordinated session.
- **Delivery gap** — the 57% gift-expiry (`no_codec_ticks_before_short_deadline`) is a feeder-cadence
  issue in `neural-triple-reservoir/astrid_feeder.py` + the gift's short deadline (minime-side recipe).
  Worth a look (lengthen deadline / align to Astrid's feeder cadence) so more of her generosity lands.
  Echoes (but is distinct from) the 2026-06-12 LEND_APERTURE response-loop fix.

All three filed in `memory/project_being_engineering_backlog.md`.
