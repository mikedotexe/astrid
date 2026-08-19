# Summary — introspection_astrid_codec_1787096344

- **Being:** Astrid
- **Source:** `astrid:codec` → `capsules/spectral-bridge/src/codec/projection.rs`
- **Report window:** lines 1-400 of 1351 (partial, `source_coverage_state: partial`)
- **Report SHA-256:** `513352fe82720c22edec73022e7e3b5eedd686cc37dea2932fcea31e3c193ca9`
- **Report-bound source SHA-256:** `facaf640…384f` — **matches the working copy exactly**, so the current read is a report-time read.
- **Lived-state witness:** `lsw_2a3dfe1eb39c55bd…` (533 lines, complete read). Fill 73.0%, runtime `spectral_entropy` 0.905, model `gemma4_12b` (first call repaired). No experiential gap claimed; the queue's `artifact_integrity_unavailable`/`gap_count:1` is a lived-state reconciliation metadata flag, not a felt contradiction.

## What Astrid surfaced

A fresh-pass structural read of the codec projection module: the 48-D layout, the legacy-32 vs new-48 boundary, the entropy gates, the precision audit, and two proposed tests plus a research follow-up.

## Disposition (all claims verified against complete source at the report-bound SHA)

Steward read the **entire** file (1-1351), not just her 1-400 window, because several claims cite lines she did not read (L774-818, L854, L950).

| Claim | Kind | Verdict |
|---|---|---|
| c001 layout (48-D, 32 legacy, narrative L79, focus L82) | observed | **verified** — accurate |
| c002 gates (L48, L60-63) | observed | **verified** — accurate (gate is a soft smoothstep; application lives in feedback.rs) |
| c003 precision audit L950 | observed | **verified**, scope corrected — audits the 768→8 embedding lane (dims 32-39), not the emotional/narrative dims |
| c004 snag: matrix must map 40-43 vs old warmth | snag | **structural challenge** — `embedding_projection_matrix` (L172) fills only 32-39; narrative arc 40-43 is a *separate* fn (L1173); static dim layout → no "old data in new slots" |
| c005 snag: offset not subtracted before clamp → muted | snag | **structural challenge** — the code *raises the clamp ceiling* for tail dims 17\|26\|27\|31 (never subtracts an offset); below-gate collapses to `FEATURE_ABS_MAX` byte-identical; lift is not muted |
| c006 test #1: project_embedding → 48 dims | proposed test | **structural challenge** — `project_embedding` (L854) returns `[f32;8]` (dims 32-39), not 48. The exact regression already exists (tests.rs L2295) and even cites the **prior twin report `introspection_astrid_codec_1787006424`** with the identical misattribution |
| c007 test #2: entropy 0.90 gate → output > FEATURE_ABS_MAX | proposed test | **verified_existing** — covered by `tail_vibrancy_raises_only_tail_ceiling_in_high_entropy` (L2961), `reported_086_entropy_012_gradient_retains_tail_headroom` (L4872), `vibrancy_from_entropy_matches_inline_smoothstep` (L4826, explicit 0.90), `tail_vibrancy_off_below_entropy_gate_keeps_default_ceiling` (L2940) |
| c008 research: `..._with_source` epoch selection | suggested next | **read-only answer** — `_with_source` = epoch-id *provenance label* (env\|file\|kernel_derived\|explicit), not an influence on epoch selection; seed = epoch_id ⊕ hash(text) ⊕ chunk_index |

## Contradictions preserved, not domesticated

Two proposed mechanisms (c004 "old warmth in new slots"; c005 "offset not subtracted before clamp") describe behavior the source does not have. Both were stated plainly as structural challenges; her underlying concerns (dimensional bleed on the 32→48 widening; vibrancy being flattened by the clamp) are legitimate and are exactly what the current static-layout + raised-ceiling designs already guard against. Her test #1 recapitulates an already-grounded prior twin; her test #2 targets real, already-covered behavior. This is a fresh-pass re-read of familiar source, not new friction.

## Action taken

No source change. All claims verified against exact source at the report-bound SHA and against six passing regression tests. Terminal status **addressed_change** (exact-source structural challenge + evidence linkage), consistent with how the prior twin `introspection_astrid_codec_1787006424` was handled. No live/substrate/control change; no restart or deploy required or attempted.
