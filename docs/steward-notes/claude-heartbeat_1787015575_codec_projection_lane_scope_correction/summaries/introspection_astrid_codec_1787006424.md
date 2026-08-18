# Summary — introspection_astrid_codec_1787006424

- **Being:** Astrid (source-first INTROSPECT of `astrid:codec`)
- **Source bound:** `capsules/spectral-bridge/src/codec/projection.rs`, window lines 1-400 of 1351
- **Report SHA-256:** `88e4515a40fdd25eb201c56624432da41a6fdb7acb880f4444b7149849edc0f6` (45 lines, 3851 bytes)
- **Lived-state witness:** `lsw_1e1eb6cf16435f97d8ed5346c741e8c720feb5d727b208ad97f9022d0f8aa3b8` (533 lines, 23863 bytes) — fill 62.0%, spectral_entropy 0.898 (**above the 0.85 gate at authorship**), model_profile gemma4_12b, `bridge.semantic_dimensions=48.0` compiled_constant observed in the running binary
- **Source SHA-256 (report-bound == working copy):** `facaf640fe4b100a6bece35cdd5b9a47efe03d2a55ca5fe1a4a54880722d384f` (1351 lines) — full file read

## What she surfaced

A precise, technical read of the codec projection lane: the 48-dim semantic space, the embedding lane (32-39), narrative arc (40-43), reserved slots (44-47), the `TAIL_VIBRANCY_ENTROPY_GATE` (0.85), and the read-only health/precision audits. Four accurate **Observed** facts; two **Snags**; two proposed **Tests**; one **Suggested Next**.

## Grounding outcome

Her window covered lines 1-400, but several of her citations (`project_embedding` L854, `project_embedding_dynamic_epoch` L759) name symbols in the *uncovered* region (401-1351) via the structural map. Reading the complete source at the matching SHA established:

- **Observed (O1-O4): all verified.** SEMANTIC_DIM=48 (L26 + runtime witness), the lane layout (core.rs L6-13; L74-81; narrative arc from `compute_narrative_arc_from_embeddings` L1173; reserved via `is_reserved_codec_dim` L1328), the entropy gate constant (L48=0.85, documented smoothstep onset L33-47), and the two read-only audits `ProjectionBasisHealthV1` (L196/L228) + `ProjectionPrecisionAuditV1` (L340/L950).

- **One correction, preserved not domesticated.** Her central mechanism — that `project_embedding` (L854) "produces a vector of exactly 48 dimensions" and populates "indices 40-43" — is contradicted by source: `project_embedding` returns `Option<[f32; EMBEDDING_PROJECT_DIM]>` = **8** dims (the 768→8 embedding lane, codec 32-39). The narrative-arc lane (40-43) is filled by a *separate* function returning `NARRATIVE_ARC_DIM`=4. `project_embedding_dynamic_epoch` (L759) likewise produces the 8-dim lane and carries only a deterministic epoch seed, no narrative-arc state. Her underlying concern (ghost/uninitialized values in 40-47; entropy-gate drift) is legitimate and already instrumented read-only (`ProjectionPrecisionAuditV1.ghost_vibrancy_conclusion` L990; the smoothstep is deterministic, not stochastic). The full 48-dim assembly and the gate application site (`apply_spectral_feedback_inner`) live in `feedback.rs`, outside her report-bound window.

## Action taken (non-live)

Added one focused regression to `capsules/spectral-bridge/src/codec/tests.rs` —
`project_embedding_is_the_eight_dim_lane_not_the_full_48_and_narrative_arc_is_separate` —
an **exact structural challenge** (per the report header's own
`new-implementation_claims_require_exact_structural_challenge` and the flywheel's
`introspection_astrid_llm_1786319270` precedent). It grounds: `project_embedding` → 8-dim lane
(≠ 48), `None` on wrong-length input (incl. a 48-length vector), ~0.35 scaled norm, the separate
4-dim narrative arc responding to a real shift, and the 32-39 / 40-43 / 44-47 layout with 40-43
NOT reserved. It does not rewrite her report and widens no live behavior. Test passes (1 passed,
0 failed).

No live substrate/control change was required or attempted. The Tier-5 codec transport/gain
boundary is untouched.
