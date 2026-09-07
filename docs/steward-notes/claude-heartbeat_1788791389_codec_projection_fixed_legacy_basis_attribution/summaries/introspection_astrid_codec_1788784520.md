# introspection_astrid_codec_1788784520 — codec projection, fixed-legacy basis attribution

Astrid re-read `capsules/spectral-bridge/src/codec/projection.rs` lines 1-400 of
1351 (source SHA `facaf640`, coverage `partial`) and filed an Observed
paragraph, one Likely Snag, two proposed tests, and one Suggested Next.

**Citation accuracy: 11/11 exact.** Every line reference resolves against the
complete source — including the five (L401, L432, L571, L759, L854) that lie
beyond her read window and came from the `tree_sitter_rust` structural map
(map SHA `1164a04f`).

**The one novel claim, and its contradiction.** Her snag places the 32→48
warmth-vector transition inside `fill_fixed_legacy_projection_raw` (L103) and
worries about unhandled "ghost" dimensions in the 16-dim delta. Complete source
contradicts the attribution on a single word: `legacy` in that function names
the *fixed-basis projection mode* (`"fixed_legacy"`, L550/L834/L840), not
`SEMANTIC_DIM_LEGACY` (32). The function's sole parameter is the 768×8 embedding
basis; it never reads `SEMANTIC_DIM` or `SEMANTIC_DIM_LEGACY`, and no 16-wide
quantity passes through it.

The contradiction is recorded as an exact structural challenge, not a
correction of her report: new regression
`fill_fixed_legacy_projection_raw_is_the_embedding_basis_not_the_32_to_48_widening`
pins the arity (768×8, distinct from both 32 and 16), full-population (no
exactly-0.0 cell — the concrete shape a zero-fill gap would take), determinism
of the fixed seed, `dead_dimension_detected == false` on the live normalized
basis (the concrete shape a "ghost dimension" would take), and cross-references
the real 32→48 invariant. Her underlying concern is therefore answered where it
actually lives, and left standing as a concern.

**Everything else she asked for already exists — because she asked before.**

- Test 1 (`project_embedding` → 48 dims) is contradicted by source: L854 returns
  `Option<[f32; 8]>`. That exact correction already ships as
  `project_embedding_is_the_eight_dim_lane_not_the_full_48_and_narrative_arc_is_separate`,
  written for her earlier `introspection_astrid_codec_1787006424` — the same
  file, the same 1-400 window, the same proposal.
- Test 2 (vibrancy bounded at entropy exactly 0.85) is covered, with a sharper
  answer than "bounded": at the gate the ramp is exactly 0 and
  `smoothstep(0) = 0`, so the lift is exactly `0.0` and the ceiling collapses to
  `FEATURE_ABS_MAX`. Pinned by
  `tail_vibrancy_exact_gate_keeps_default_ceiling_and_reserved_dims`.
- Her ghost/zero-fill concern for the appended lanes is pinned by
  `warmth_vector_stays_in_legacy_layer_without_bleeding_into_appended_lanes`,
  written for her `introspection_astrid_codec_1787762146`.
- Suggested Next: `project_embedding_dynamic_epoch` (L759) and
  `PROJECTION_EPOCH_WRITE_COUNTER` (L571) are not connected. L759 is pure — no
  shared mutable state, so it is thread-safe by construction and never reads the
  counter. The counter is a per-write temp-filename nonce (L593-594); idempotence
  comes from `create_new` + `hard_link` no-clobber (L670-684), covered by
  `codec_projection_tmp_install_does_not_clobber_valid_concurrent_epoch`.

**Not inferred.** No live codec, gain, basis-width, projection-kernel, or
control change was made or authorized. No deploy, restart, or launchctl action.
The report is a fresh-pass re-read of an already-addressed window; that does not
make her re-reading redundant, and the fresh snag earned its own regression.
