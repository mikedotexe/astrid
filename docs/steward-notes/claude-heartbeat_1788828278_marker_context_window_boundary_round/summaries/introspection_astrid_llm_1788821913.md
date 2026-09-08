# introspection_astrid_llm_1788821913 — marker context window and placement boundary

Astrid read `astrid:llm` / `dialogue_runtime.rs`, lines 401-800 of 812, source
SHA-256 `03c6b6de…`, coverage `partial` (801-812 uncovered), witness
`lsw_9a57619c…`, fill 73.0%.

## What she read, and how accurately

Every one of her seven line anchors lands exactly on the symbol she names:
L51 `ExactKnownMarkerReferenceContext`, L335 `control_marker_placement_counts`,
L398 `trailing_bounded_chars`, L408 `leading_bounded_chars`,
L414 `control_marker_context_receipt_v1`, L437 `none_cleanup_candidate`,
L484 `sanitize_model_control_markers_with_report`. The report-bound source hash
matches both the worktree copy she read and the canonical working copy in this
repository byte-for-byte, so nothing here is a report-time/now split.

## The snag she proposed, and why source contradicts it

Her Likely Snag is that a marker "identified as a `QuotedExactKnownToken`" could
fail `delimiter_depth` or `relation_scan` requirements at L414 and fall through
to `none_cleanup_candidate` (L437). Complete source reading contradicts that
path, and the contradiction is recorded rather than softened:

* The `QuotedExactKnownToken` match arm (L422-427) binds `delimiter_depth` and
  discards `relation_scan` with `..`. There is no requirement it can fail.
* `none_cleanup_candidate` (L437) is the `None` arm — reachable only when
  `reference_syntax` is `None`, i.e. when the marker was never identified as
  quoted in the first place. The two states are mutually exclusive by
  construction.
* The receipt does not decide anything. Preservation is already settled in
  `scan_known_model_control_markers` at L229-231 (`if reference_syntax.is_some()
  { remainder.push_str(occurrence.token) }`); `control_marker_context_receipt_v1`
  only describes what already happened.

Her underlying concern is not dismissed with the mechanism. "Classification and
preservation could disagree for a marker in a mixed or nested context" is a real
property to hold, and it was genuinely unpinned — see below.

## Three real, previously uncovered gaps

Her two tests and her Suggested Next each named something the crate did not
test. All three are now regressions in
`capsules/spectral-bridge/src/llm/provider/tests.rs`:

1. `control_marker_cleanup_preserves_grouped_reference_beside_bare_twin` —
   her Preservation Test. Every prior grouped-delimiter regression asserted
   `removed_total == 0` on an all-preserved string; none combined a grouped
   preserve with a same-token removal in one call, so the split between
   `preserved_tokens` and `removed_tokens` was unpinned for exactly the mixed
   shape she describes. Now pinned: one token name in both lists, receipts
   `grouped_exact_marker` then `none_cleanup_candidate`.
2. `control_marker_cleanup_counts_string_edge_markers_as_boundary_with_empty_window` —
   her Contextual Boundary Test. Before this, no test anywhere in the crate
   asserted a nonzero `boundary_occurrences`; the only assertion pins it to `0`,
   leaving the boundary arm of L335 untested. One bounded correction, her
   concern kept whole: at index 0 the empty side is the *prefix*, windowed by
   `trailing_bounded_chars` (L398) — `leading_bounded_chars` (L408) windows the
   suffix. Both empty-window sides are pinned, so the function she named is
   still exercised on an empty window, via the end-of-string case.
3. `control_marker_context_window_truncates_multibyte_prose_on_char_boundaries` —
   her Suggested Next. Char-boundary safety holds by construction (both window
   helpers take from `.chars()`), but nothing referenced
   `CONTROL_MARKER_CONTEXT_WINDOW_CHARS`, `before_window_chars`, or
   `after_window_chars`. Now pinned for a 2-byte (`λ`) and a 4-byte (`🌊`)
   filler at `window + 6`. Bounded correction: the window helpers are called
   from `control_marker_context_receipt_v1` (L414), not from
   `control_marker_placement_counts` (L335), which never touches the window.
   This is also distinct from `…_stays_byte_safe_when_marker_abuts_four_byte_astral_chars`
   (written for her earlier `introspection_astrid_llm_1787798508`), which pins
   the marker's own `start`/`end` slice rather than window truncation.

## One additive note

Her Observed sentence names quoted and grouped tokens as the reference kinds.
That is a correct subset, not the whole surface: `ExactKnownMarkerReferenceContext`
has a third variant, `ExplicitExactKnownTokenRelation`, which preserves an
undelimited marker followed by an allowlisted relation word. Additive, not a
contradiction.

## Authority boundary

Test-only. No marker grammar, allowlist, codec, prompt, controller, or live
control changed. No build, deploy, restart, staging, or commit. The live bridge
binary is untouched; this round has no runtime effect and claims no felt
improvement.
