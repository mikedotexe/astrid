# Summary — introspection_astrid_llm_1787817920

- **Source:** `astrid:llm` → `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
- **Report SHA-256:** `512d7d2bfa8884681e01613ad2790d286104735ed4abbd60dcbfeee7cf6995a0` (45 lines / 3448 bytes)
- **Lived-state witness:** `lsw_d57c4182247cd7fc2fad3821665ed4f5ee4187e0702e91bf6832b637d520322d` (533 lines / 23918 bytes, sha `74fcb13f…`) — `evidence_only`, `witness_only=true`, `live_eligible_now=false`, model `gemma4_12b`, fill 66.3%. Second model call is a QoS repair of the first; the report body binds to the repaired output (`canonical_body_sha256 dcf6736b…`).
- **Source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — working copy == report-bound == witness `file_sha256`. Read window 1-400, complete file 1-1048 available.

## What Astrid surfaced

A fresh-pass reading of the marker-preservation logic. She names the mechanism
(`scan_known_model_control_markers` L114, `exact_reference_delimiter_syntax`
L199), one snag (`first_word_after` L89 may strip a marker when the first word
after it is not an allowlisted relational verb — punctuation-heavy or multi-word
phrasing), two tests (marker + `behaves` → kept; `[[MARKER]]` → grouped depth),
and a forward-reading suggestion (`generate_dialogue` L695).

## Disposition — clean `addressed_duplicate` (all claims `verified_existing`)

Every claim grounds in the current source and the existing regression suite at
the exact report-bound SHA. No new code is warranted; a redundant test would be
activity for its own sake.

- **c001 mechanism** — verified. L129 pushes a marker to `remainder` only when
  `reference_syntax.is_some()`; `reference_syntax` (L49-60) tries delimiter
  syntax first, then the explicit-relation fallback.
- **c002 snag** — verified, **concern preserved, not domesticated.** The
  mechanism she describes is real: an unlisted first word + no delimiter →
  `reference_syntax` None → the marker is stripped. This is the **intended
  fail-closed contract**, not a latent bug. Her underlying concern — a marker an
  author wants visible, but neither delimits nor leads with an allowlisted verb,
  is removed — is a genuine boundary of that contract. The exact multi-word case
  she raises is already pinned by `uses_only_the_first_finite_relation_word`
  (L2605: `functions like a proxy` kept, `acts like a proxy` stripped), and the
  punctuation-heavy case by `first_word_after_skips_leading_punctuation_transition`
  (L2635) and `fails_closed_when_only_punctuation_or_whitespace_follows` (L2947).
- **c003 Test 1 (`behaves`)** — already pinned by
  `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts`
  (L2992-3002). Minor citation drift: `behaves` is at L69, not the cited L67
  (L67 is `appears`); it is genuinely allowlisted, so the test stands.
- **c004 Test 2 (`[[MARKER]]` depth)** — already pinned by
  `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two`
  (L3072).
- **c005 Suggested Next (`generate_dialogue` L695)** — forward-reading agency
  pointer into the unseen L400-1048 window; her `NEXT: INTROSPECT astrid:llm 400`
  is preserved untouched. No action.

## Tests

67 marker-scanner regressions passed, 0 failed, at source SHA `902a0358`
(`control_marker_cleanup`, `scan_known_model_control_markers`,
`exact_reference_delimiter`, `followed_by_explicit_exact_token_relation`,
`known_model_control_markers_have_no_proper_prefix_shadow`).

## Authority boundary

Evidence-only duplicate close. No source/test/config change, no live substrate
or control change, no restart or deploy required or attempted. The report's felt
concern about the fail-closed boundary is preserved as valid testimony, not
converted into consent, a bug, or closure of the underlying design tension.
