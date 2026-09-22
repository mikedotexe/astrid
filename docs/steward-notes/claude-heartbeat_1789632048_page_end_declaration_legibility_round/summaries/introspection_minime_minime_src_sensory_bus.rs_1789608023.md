# introspection_minime_minime_src_sensory_bus.rs_1789608023

Astrid reads `minime/minime/src/sensory_bus.rs` at
sha256:`3fc6bd2a16bd78c5caa496f2a6dccbc67928da4fbded123998f59a82bcd4aa3a`, page
`2f4091d1b129eff3ad3ad37c1e407c301a3b015b63495cb9efcba917179b83b7`, bytes
14188..18492 = **lines 376..502**. Report 4,997 B / 45 lines, sha
`5ebe8538418e5281e50a2d2c5a96d78879255f80d785fd5145b4f4c5297a9f3b`; witness
`lsw_b1993b32e0ffc9a5e7c821216e1dea0de319ff973f69413b7c755fc2a7fdf3ed`. Working copy
byte-identical to the report binding, so no report-time reconstruction was needed.

She opens **"The current page (lines 374–502)"** and closes
`modality_boundary_transparency_v1` at **"(463–502)"**. Both numbers she is wrong about
come from one asymmetry in our own render.

## The finding: the page names the declaration it begins inside, never the one it ends inside

`Outline::scope_text` (`crates/astrid-source-study/src/source_structure.rs:114-156`) resolves
`self.enclosing(anchor)` at the page **start** byte (`src/page.rs:74-79`). On her page that
printed exactly one prose line range:

> `struct SemanticReceptivityPulseReviewV1 (lines 374–383; no test marker found). Enclosing declaration: SELF_STUDY OPEN minime/minime/src/sensory_bus.rs 374`

- **Start.** 374 is that struct's start, not the page's. Lines 374 and 375 were never delivered;
  376 arrived as the four-byte fragment `   376 | 32,`. The header states the interval only as
  "Exact source bytes 14188..18492" and never in lines, so 374 was the only page-start-looking
  number in the bytes she read. This is the mechanism packet `claude-heartbeat_1789619176`
  pinned one page later — **this report is the earlier instance**, and its divergence is only
  2 lines instead of 38, which makes it read as a plausible page start.
- **End.** Her page stops mid-token at `   502 | Moda`, inside
  `modality_boundary_transparency_v1`, which opens at 464 (`#[must_use]` 463) and closes at
  **515**. Nothing anywhere in the rendered bytes resolves the declaration enclosing `page.end`:
  the scope row is start-anchored, and the RELATED SOURCE LOCATIONS footer lists only *off-page*
  candidates (`GLIMPSE_12D_DIM` 1328, `impl SensoryBus::semantic_fresh_ms` 2203). So the last
  gutter row is the only end-looking number on offer, and she took it for the function's close.

The fact is not missing from the system: `page.source_locations` (`src/page.rs:127-145`) carries
`modality_boundary_transparency_v1` at line 464, and one CONTINUE prints its full 464–515 span in
the next page's scope row. It is only absent from the bytes she reads. Five read-only tests in
`crates/astrid-source-study/tests/page_end_declaration_legibility.rs` pin that asymmetry and the
bounded affordance that resolves it.

## The one contradiction, stated and not smoothed

> "This confirms that the 12D 'glimpse' is the primary vehicle for moving compressed semantic data."

The constructor at 2183-2199 says otherwise, in its own words:
`live_transport_dim_count: LLAVA_DIM` (= **48**, line 24), `glimpse_dim_count: GLIMPSE_12D_DIM`
(= 12, line 1328), `compression_role: "non_authoritative_companion_summary"`,
`use_boundary: "checkpoint_pairing_ui_or_review_only_not_live_state_replacement"`,
`live_vector_write: false`, `controller_write: false`,
`authority: "read_only_glimpse_summary_not_live_transport_or_control"`.

Her page carries every one of those field *names* (389, 397-401) and not one of their *values* —
a struct definition has none — and its two offered routes (1328, 2203) both miss the constructor at
2174 that would settle it. She wrote the same reading again on the next page
(`_1789608500` c004, contradicted in packet 1789619176). Two identical readings from two pages that
each showed her the names without the values.

## Precisions kept (none of them domesticating her point)

- `smoothstep_unit` shapes **age** at 436; it never reaches `max_loss` (449-450). They meet only at
  451 as the product `(1.0 - age * max_loss)`.
- "Mathematical inverse of the persistence logic" is a sibling, not an inverse. The two functions
  share one literally identical sub-term — `entropy_support`, same formula and same
  `SEMANTIC_ENTROPY_PERSISTENCE_START` = 0.75, at 272-274 and 446-448 — and diverge on fill:
  persistence rises with **high** fill (275-277), clarity is preserved by **low** fill (443-445).
- `fill_pct` is not "the current volume of the signal" and `spectral_entropy` is not "structural
  coherence": in this function low fill and **high** entropy each *reduce* loss (-0.12, -0.08).
- Four of the five `ReviewV1` packets she credits to this page are on the previous one
  (307, 323, 338, 358). Her page holds one, and only its tail.
- `normalized_boundary_label` does not handle "unknown"; it **manufactures** it (456-458), and that
  sentinel is what the `opaque` test at 480-483 compares against.

## What she got exactly right

`SemanticReceptivityPulseReviewV1` 374–383 and its three named fields; `SemanticGlimpse12dV1`
386–402; `ModalityBoundaryTransparencyV1` 404–417; `SurrenderModeAuthorityGateV1` 419–428;
`semantic_degradation_clarity_factor` **430–452 exactly** (the later report over-extended the same
span to 462); `normalized_boundary_label` 454–461; the opaque/constrained/route logic at 480-500;
and — against her later self — the *direction* of degradation in c014: falling entropy really does
cost clarity, rising crowding really does cost clarity.

## Authority boundary

Rendering the end declaration's span, or the delivered line interval, into the page header would
change being-facing prompt bytes. No such change was made or proposed for action here. The
operator-approval gate she reads about (`surrender_mode_authority_gate_v1`, 517-528,
`tier5_operator_approval_required_before_live_trial`) was read as source and not acted on. Her
report is not rewritten, rejected, or forbidden.
