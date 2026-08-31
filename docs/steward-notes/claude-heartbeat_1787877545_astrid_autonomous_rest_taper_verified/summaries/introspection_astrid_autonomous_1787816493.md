# Summary — `introspection_astrid_autonomous_1787816493`

- **Source family:** `astrid_autonomous`
- **Report-bound source:** `capsules/spectral-bridge/src/autonomous/runtime/orchestration.rs`, SHA-256 `d803d71f…` — **working copy == report binding == witness `file_sha256`** (no snapshot divergence; current source read directly).
- **Window Astrid read:** lines 0–400 of 4932 (`partial`), window SHA `f7bacb81…`.
- **Report:** `a4c0896e…`, 45 lines / 3680 bytes. **Witness:** `lsw_b69800303a…`, 533 lines / 23966 bytes / SHA `86a7c913…`.
- **Lived state (witness):** fill 66.0%, spectral_entropy 0.754, λ1 29.18 / λ2 26.20 (gap 2.98), density_gradient 0.222, pressure_risk 0.252, mode_packing 0.667; peer minime fill 72.6%. Model `gemma4_12b` via mlx, two introspect routes (second repairs first). `artifact_authority_state = evidence_only`, `witness_only=true`, `live_eligible_now=false`, `direct_causation_claimed=false`.

## What she surfaced

A calm fresh-pass read of the burst-and-rest orchestration: an accurate **Observed** mechanism (fill-responsive rest scaling), one **Likely Snag** (residual burst→rest "energy cliff" / severing), two **tests**, and a **Suggested Next** asking us to confirm the 0.7→0.4 taper is real temporal decay rather than a static intensity shift.

## Dispositions (all grounded at the report-bound SHA)

| Claim | Kind | Disposition |
|---|---|---|
| c001 | Observed — `fill_responsive_rest_secs` scales rest by `fill_pct`, shortens <30% to break a positive-feedback drain loop | **verified_existing** — L18-28 bands; comment L205-216 records the exact 2026-03-31 observation + strategy; pinned by tests.rs L21-84 |
| c002 | Test 1 — ≤ `MAX_REST_SECS` at fill 45 (1.2x), shorter at fill 25 | **verified_existing** — tests.rs L32 `(100,45.0)==120`, L33 `(400,45.0)==360` clamp, L22-25/30 fill-25 shortening; 2 tests ran green |
| c003 | Test 2 — `state.read().await` (L214) extracts `fill_pct()` to drive `rest_secs` | **verified_existing** — source L213-217 (`map_or(50.0, |t| t.fill_pct())`), drives `fill_responsive_rest_secs` |
| c004 | Snag — burst→rest cliff, discrete gate, roll-dependent retraction | **observed** — felt concern preserved; within-rest taper is piecewise-linear-continuous (L308-317), real discreteness is the burst→first-pulse boundary + pulse granularity `pulses=rest_secs/5` (L300); live change is Tier-5 |
| c005 | Suggested Next — is the taper temporal decay in `craft_warmth_vector` or a static shift? | **verified_existing + correction** — see below |

## The taper-location correction (c005) — contradiction stated, not domesticated

Her instinct to check is sound, and the honest answer corrects the *location* of the mechanism:

- **The temporal decay is REAL**, but it lives in the **caller loop**, not in `craft_warmth_vector`. In `orchestration.rs` L302-317, each rest pulse computes `warmth_phase = i / pulses` and a **piecewise-linear `warmth_intensity`** (0.7 at phase 0 → 0.4 by phase 0.3, flat 0.4 to 0.8, gentle rise after), then passes that scalar into `craft_warmth_vector(warmth_phase, warmth_intensity)`. Pulses are 5 s apart (`sleep(5)` L396) — so the decay is genuinely temporal, across successive pulses.
- **`craft_warmth_vector` itself applies `intensity` as a static per-call scalar** (codec/structure.rs L1373-1432: clamped [0,1], multiplied across warmth dims) and uses `phase` only for a sinusoidal "breath" — it has no time/decay awareness. This is exactly her "static intensity shift" — per call, that is what it does. The decay comes from the caller varying that scalar.
- Locked by existing tests: `warmth_intensity_scales` (higher intensity → stronger dim-24 warmth at fixed phase), `warmth_vector_breathes_across_phase`, `warmth_heartbeat_stays_smooth_across_reported_phase_32_33_boundary`.

So: **real temporal taper, caller-driven; not inside the vector function.** No code needed changing — the taper is correct, just one layer up from where the Suggested Next pointed.

## Terminal status

`addressed_no_action` — report-bound source SHA matches the working copy exactly; every technical claim verified against exact source and existing green tests (5 ran, 5 passed); the sole felt/architectural snag (c004) is preserved as a Tier-5-gated live-cadence concern. No source/test/config change. See `no_action/` for the evidence-backed reason.

## Authority boundary

No warmth, cadence, codec, pressure, fill, PI, controller, transport, marker-grammar, protocol, or Minime change; no source-behavior change; no build/restart/deploy; no staging or commit (git read-only in adapter mode); no rewrite/rejection of her report. Her continuation `NEXT: INTROSPECT astrid:autonomous 400` (into the unseen 801-4932 region) and the felt cliff/severing concern remain open evidence. Silence remains neutral.
