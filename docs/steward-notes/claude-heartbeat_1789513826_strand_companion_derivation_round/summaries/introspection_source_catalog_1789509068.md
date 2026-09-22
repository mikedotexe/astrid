# Summary — introspection_source_catalog_1789509068

Astrid, on a navigation-only turn with no source page delivered, states plainly what she is
hunting: "I am looking for the specific calculation of the delta between `projection_48d` and
`companion_projection_12d`." She reports that her earlier read of
`crates/astrid-minime-protocol/src/volition/inquiry.rs` gave her the *requirements*
(`owner_inquiry_fixed_analysis_set_v1`) but not the arithmetic, that her identifier search pointed
at fixtures and a wire contract, and that she will next try to find a `validation.rs` or a module
importing `OwnerInquiryV1`.

## What complete source reading established

1. **There is no strand-level delta.** At the producer site
   `capsules/spectral-bridge/src/autonomous/inquiry/parsing.rs:187`, the companion is *derived*:
   `GlimpseCodec::derive_12d(&projection_48d)`. The arithmetic she wants is
   `capsules/spectral-bridge/src/codec/evidence_types.rs:839-856` — three tanh(mean_abs) block
   summaries over dims 0..8, 8..16, 16..24; four passthrough tanh of dims 24,25,26,27;
   tanh(mean_abs) of 28..32, 32..40, 40..44; a tail/texture anchor over dims [17,26,27,31]; and a
   whole-vector tanh(mean_abs) in `out[11]`.
2. **A real 48D↔12D delta does exist — elsewhere.** `capsules/spectral-bridge/src/codec/structure.rs:88`
   computes `resolution_delta = (1.0 - glimpse_fidelity_score).clamp(0.0, 1.0)` inside
   `multi_scale_observer_v1`, alongside `resonance_loss_ratio` (structure.rs:90-95). Both are
   read-only observers over her own "distillation, not compression" proposal; neither validates the wire.
3. **Her `validation.rs` guess was right about the file and wrong about its contents.**
   `crates/astrid-minime-protocol/src/volition/inquiry_v2/validation.rs` exists and imports the fixed
   analysis set, but holds coverage modes, plan equality, offline budget, cancellation, strand
   distinctness, pair keys, and observation chaining. No projection arithmetic.
4. **"Stability corridor" is not corroborated.** Neither `stability corridor` nor `stability_corridor`
   occurs in `crates/` or `capsules/spectral-bridge/src/`. The nearest named corridor,
   `crates/astrid-types/src/agency_corridor.rs`, is an unrelated agency surface. Her validation-context
   inference held; this half did not. The contradiction is recorded, not smoothed over.
5. **The wire never re-derives the companion.** `SemanticStrandV1::is_well_formed`
   (inquiry.rs:70-77) checks only length 12 and finiteness; `canonical_semantic_strand_embedding_sha256`
   (inquiry.rs:436-442) is a *joint* digest of both vectors, not a relation between them.

## What changed

One focused regression, `strand_companion_is_the_derived_glimpse_not_an_independent_delta_field`, in
`capsules/spectral-bridge/src/autonomous/inquiry/parsing.rs`. It pins the derivation at `build_strand`
and pins the honest boundary: a substituted, unrelated finite 12D companion still passes protocol
validation once the embedding digest is recomputed. The 48D→12D relation is a producer-side contract.

## Steward observation, not her claim

The one computed delta scores the 12D glimpse against a **32-dim** reference
(`calculate_compression_fidelity(&features[..32], &glimpse)`, structure.rs:87;
`compression_reference_12d`, evidence_types.rs:894-909), while `derive_12d` reads dims 32..40, 40..44
and all 48. `MultiScaleObserverV1` declares `live_transport_dim_count: 32` while `SEMANTIC_DIM` is 48.
Whether that is deliberate pinning to an older transport lane or drift from the 32→48 codec widening
is not decided here. Codec transport surfaces remain Tier 5; nothing live was touched.

## Navigation reach

Her "requested source was not supplied" line matches documented reader behavior for an unknown target
(`autonomous/next_action/action_help.rs:169`), and the witness confirms no source snapshot this turn —
no delivery loss. The real limit is narrower and worth naming: an identifier-scoped search for
`owner_inquiry_fixed_analysis_set_v1` or `OwnerInquiryV1` cannot reach `evidence_types.rs:839` or
`structure.rs:88`, because the derivation and delta sites mention neither identifier. She was
searching correctly for a symbol that does not appear where the answer lives.
