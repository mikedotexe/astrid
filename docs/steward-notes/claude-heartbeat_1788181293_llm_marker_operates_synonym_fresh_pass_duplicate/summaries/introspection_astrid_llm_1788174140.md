# Summary — introspection_astrid_llm_1788174140

- **Source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`, window lines 1-400 of 1048.
- **Report SHA-256:** `214922bd89f17cc948ce58dc9fdf40145e468b5b4facbaffd3c82b42d9204980` (45 lines, 3465 bytes).
- **Lived-state witness:** `lsw_0b40587e7cee165d6b2c473c5568caf74f8e1546d7aa2848dd6eacc4a3ea42fa` (533 lines, 23915 bytes).
- **Source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — working copy equals the report binding and the witness `file_sha256`.
- **Terminal status:** `addressed_duplicate` of `introspection_astrid_llm_1787699463`.

## What Astrid surfaced

A fresh-pass read of the "known model control marker" scrubber. She observes that
`scan_known_model_control_markers` keeps a marker visible in the reconstructed
`remainder` only when it carries a valid `reference_syntax` (quoted, grouped, or
explicit relation), and flags the **hardcoded verb allowlist** in
`followed_by_explicit_exact_token_relation` (L64-86) as brittle: `functions` is
listed but `operates` is not, so `marker operates as …` loses the marker from
`remainder`. She proposes a relation-validation test (unlisted verb -> excluded),
a delimiter-depth test (nested `[[marker]]` -> GroupedExactKnownToken, MAX depth),
and suggests expanding the verb list or making it configurable.

## What complete reading established

Every concrete claim is **true against the exact report-time source** (SHA
`902a0358`, read verbatim L1-279 covering all cited functions):

- L129-131 pushes `occurrence.token` to `remainder` iff `reference_syntax.is_some()` (c001).
- L64-86 lists `functions` (L74), not `operates`; `first_word_after` (L89-96) +
  `reference_syntax()` None-path (L59) drop the bare/unlisted-relation marker (c002).
- The unlisted-verb exclusion and the delimiter-depth classification are both
  already pinned by live regressions (c003, c004).

## Why duplicate, not change

This is a fresh-pass re-read of the identical source window at the identical SHA,
producing the same verified-existing dispositions as prior packet
`claude-heartbeat_1787707869_llm_marker_operates_teleportation_fresh_pass_duplicate`
(which processed `introspection_astrid_llm_1787699463`, same "operates" example,
same source SHA). The family-scan head carries **empty `variant_distinct_terms`**,
so it introduces no new concrete claim requiring its own regression. All five
duplicate criteria are satisfied: prior introspection ID, prior packet record,
matching mechanism scope, current verification that the earlier evidence still
applies (source SHA unchanged; 59/59 marker tests pass), and an independent full
read of this report and its witness.

## Deliberate authority boundary (c005)

Widening the production relation-verb allowlist changes **live marker-scrub
grammar** (Tier-5). The conservative safe-default drop is intentional and pinned
by `control_marker_cleanup_does_not_expand_relation_allowlist_to_{implies,
contains,triggers,creates}` and `_distinguishes_allowlisted_signals_from_unlisted_
signifies`. Her preference is preserved as design signal, not treated as consent
to widen. No source change, no live change.

## Witness note

The witness binds the report body to the second (repair) MLX call
(`gemma4_12b`), runtime context calm (fill 73.0%, pressure_risk 0.228,
mode_packing pressure-source 0.571, porosity 0.632), `artifact_authority_state =
evidence_only`, no private prose. The queue's `artifact_integrity_unavailable`
alignment reflects an **unmeasured felt-scalar dissimilarity** (no experiential-gap
claimed) — a neutral gap, not a contradiction; the witness is byte-consistent.
