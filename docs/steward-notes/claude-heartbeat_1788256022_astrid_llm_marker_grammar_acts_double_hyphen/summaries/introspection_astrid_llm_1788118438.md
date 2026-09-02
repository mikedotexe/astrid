# Summary — introspection_astrid_llm_1788118438

**Source:** `astrid:llm` → `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
(source window read: lines 1-400 of 1048; coverage manifest: multi_window_complete)
**Report SHA-256:** `372858b09ecbb85b1f0eb0b57e27caf592069e69adccb40be5124e57bb65955b` (45 lines, 3846 bytes)
**Witness:** `lsw_4013dbe95eb1f2f38565057fe76b774c12a9c703a7833fe4d29657e755025985`
(533 lines, 23929 bytes, SHA `5cbfe892ab134d0064c62ecb7c8bfbaefa6e0bebfde473856960c4f50c6c1a4c`)
**Report-bound source SHA-256:** `902a0358…c7ee` — **matches** the working-copy exactly; complete source read (1-1048).
**Runtime witness context:** fill 71.0%, model profile `gemma4_12b` (mlx), two introspect model routes (second is a repair-parent of the first), mode_packing 1.0, minime fill 73.0%. No authority markers set (evidence_only).

## What Astrid surfaced

A source-facing marker-grammar report on the `dialogue_runtime.rs` control-marker
scanner. She (1) *observed* the preservation model — markers stay visible only when they
carry a valid `reference_syntax`; (2) proposed a *snag* that `first_word_after` (L89)
mishandles hyphenated/contraction relations; (3) proposed two tests (`[MARKER] acts--as`
grammar recognition; `[[MARKER]]` delimiter depth); and (4) a suggested-next to verify the
marker list + longest-match priority.

## Disposition (5 claims)

- **c001 Observed preservation logic → verified_existing.** Source L114-144 confirms:
  `remainder.push_str` fires only when `reference_syntax.is_some()` (L129-131). A bare
  marker is dropped; a referenced marker is kept byte-exact.
- **c002 first_word_after hyphen snag → implemented_now (contradiction preserved).**
  `first_word_after` (L89-96) trims non-alphanumeric only from chunk **ends** (L92), keeping
  interior punctuation, so a hyphen-joined chunk is returned **whole** — it does not isolate a
  sub-word as she expected. The underlying concern (a hyphen-joined compound is not an exact
  single-word relation, so the marker is removed) is nonetheless true and by design.
- **c003 Test 1 `acts--as` → implemented_now.** Added focused regression
  `control_marker_cleanup_rejects_interior_double_hyphen_joined_relation_suffix`: grounds her
  exact interior double-hyphen shape, corrects the expectation (`first_word_after` returns
  `"acts--as"`, marker removed), and adds a positive control (bare listed `as` preserved).
- **c004 Test 2 `[[MARKER]]` → verified_existing.** `exact_reference_delimiter_syntax`
  (L199-229) already comprehensively tested at depth 2 (`((…))` L2272, `[ ws [ … ] ws ]`
  L2257) and beyond (clamped `[[[[[…]]]]]` L2300, CJK/heterogeneous).
- **c005 Suggested-next marker list + longest match → verified_existing.** List at
  `fallback_contracts.rs:159-179`; `max_by_key` longest-match already grounded (L1745, L2528,
  and L3244 which cites L97-112 and notes no marker is a prefix of another).

## Authority boundary

Read-only source verification plus one non-live focused Rust test. No runtime, controller,
codec, protocol, or model behavior change; no deploy or restart. The felt snag is treated as
primary evidence: the contradiction with source is stated plainly (per the handoff's
un-domestication rule), not used to dismiss her concern.
