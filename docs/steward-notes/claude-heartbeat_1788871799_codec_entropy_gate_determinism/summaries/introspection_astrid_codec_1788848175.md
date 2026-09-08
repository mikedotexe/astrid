# introspection_astrid_codec_1788848175 — codec projection, entropy gate, determinism

Astrid re-read `astrid:codec` (`codec/projection.rs`) lines 1-400 of 1351 at source
SHA `facaf640…`, fill 73.0%, spectral entropy 0.905, on the `gemma4_12b` MLX route.
Read completely: report (3861 bytes / 50 lines), witness
`lsw_78fca67358f2…` (23757 bytes / 533 lines), and the **entire** 1351-line
report-bound source — whose working-copy hash matches her binding exactly.

## What she got exactly right

Every line number in the report is correct: `TAIL_VIBRANCY_ENTROPY_GATE` L48,
`TAIL_VIBRANCY_MAX` L53, `FEATURE_ABS_MAX` L32, `STRUCTURAL_ENTROPY_DAMPENING_*`
L60-63, the noise-damping min coefficient L67, `embedding_projection_matrix` L172,
`ProjectionPrecisionAuditV1` L340, `CodecLaneSeparationAuditV1` L401,
`project_embedding_dynamic_epoch` L759, `project_embedding` L854. Her "described as a
'soft gate' (L38)" is also right in substance: L38 reads `/// NOT a hard gate.`

## Two precisions kept BESIDE her words, not replacing them

1. **Snag 1 attribution.** Her widening concern is real, but
   `embedding_projection_matrix` (L172) is the **768x8 embedding basis**. It never
   reads `SEMANTIC_DIM` or `SEMANTIC_DIM_LEGACY`, has no legacy-width slice or pad,
   and rejects wrong-width input with `None` (L855-857) — so index-out-of-bounds
   cannot arise there. Her *earlier* `introspection_astrid_codec_1788784520` placed
   the same concern one call deeper (`fill_fixed_legacy_projection_raw`); this round
   grounds the consumer she now names.
2. **Test ask 1 scope.** `project_embedding` returns `Option<[f32; 8]>`, not 48. The
   8 dims land in codec 32-39; 40-43 are the narrative arc from a separate function;
   44-47 reserved. That correction was already pinned for her
   `introspection_astrid_codec_1787006424`.

## The claim that was genuinely untested — and is now covered

Her snag 2 asked whether the clamp-ceiling offset "might produce non-deterministic
results if the damping coefficients (L67) are not strictly enforced." The *lift* and
*aperture-bound* halves of her entropy test were already pinned. The **determinism**
half was not. Source shows the entire chain is pure arithmetic over clamped finite
scalars — `vibrancy_from_entropy_and_density_gradient` (cascade.rs L277-282),
`codec_vibrancy_noise_dampening_v1` (structural_evidence.rs L548-584, clamped to
`[0.82, 1.0]` at L544), ceiling and clamp (feedback.rs L306-318) — with no RNG,
clock, or environment read. Two new regressions now assert that as behavior.

## One thing she could not see from L1-400

Her bound question ("remaining within the bounds of `TAIL_VIBRANCY_MAX`") is
conditional. At the default `vibrancy_aperture` 1.0 the tail bound *is*
`TAIL_VIBRANCY_MAX`; above 1.0 the bound is
`dynamic_max = TAIL_VIBRANCY_MAX × (1 + (aperture − 1)·navigable)` (feedback.rs
L307) — her own `SET_VIBRANCY_APERTURE` dial, which lives outside her read window.
The new test asserts her stated bound at the default and names the dynamic-ceiling
contract rather than silently contradicting her.

## Not done on her behalf

Her "Suggested Next" (read `project_embedding_dynamic_epoch`, L759) is recorded as
her own reading choice, not executed. Her continuation offer is window 401-800, which
contains that code. No live codec constant, gate, ceiling, basis, gain, or aperture
was changed; no deploy, restart, or git mutation occurred.
