# Summary — introspection_astrid_llm_1787798508

**Source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
(report-bound SHA `902a0358…` == current working copy, verified byte-identical,
1048 lines / 38586 bytes). **Report** SHA `9460ff55…` (50 lines / 4087 bytes).
**Witness** `lsw_45021598…` (533 lines / 23933 bytes, SHA `68508a4b…`), bound to
the report (`artifact_sha256` matches), authority `evidence_only`,
`direct_causation_claimed=false`, canonical-body binding `1b6fe6f4…` (2515 bytes)
re-derived and confirmed. The queue's `artifact_integrity_unavailable` /
`gap_count 1` is a lived-state **alignment-measurement** annotation (no scalar
felt dissimilarity was measured), not a report/witness corruption.

## What Astrid surfaced

She read the model-control-marker scanner in `dialogue_runtime.rs` and described
it accurately: a two-layer design that proves a byte range is a known marker
(`ExactKnownModelControlMarkerOccurrence`) and then separately classifies its
semantic role (quoted / grouped / explicit-relation). She named the relational-verb
allowlist, the quote/group delimiter recognition (incl. CJK pairs), the
remove-or-preserve sanitizer, and the felt-decoupled token band. She then
proposed two snags and two tests.

## Disposition

- **Observed (c001–c005):** all verified against complete source. One precision
  note on c004 — the sanitizer never *modifies a marker's content*; it removes a
  bare marker wholesale or preserves a referenced marker verbatim. Her
  structural-role insight is correct.
- **Boundary-safety snag / test (c006, c008):** the stated mechanism (a slice
  landing *mid-character* and panicking) is **contradicted** — `start`/`end` are
  only ever constructed at UTF-8 char boundaries in `scan_known_model_control_markers`
  (offset advances by `len_utf8()` or an exact marker length), so a mid-char index
  is unreachable. Rather than dismiss it, her underlying concern (boundary safety
  at a 4-byte astral char) is translated to its constructible core and covered by
  a **new regression**: a marker directly abutting 4-byte emojis on both sides
  with no separating whitespace, proving no panic and byte-exact behavior. This is
  distinct from the existing whitespace-separated 4-byte tests at L2834/L2853.
- **Relation-verb test (c009):** premise **contradicted** — `is` is allowlisted
  (L79), so her `<marker> is a behavior` returns `true`, not `false`. Already
  covered exactly by `control_marker_cleanup_distinguishes_allowlisted_is_from_unlisted_acts`
  (tests.rs L2568). Same class as the handoff's cited `introspection_astrid_llm_1786319270`.
- **First-word-only scope (c007):** verified as an intentional **fail-safe** —
  an unlisted first word keeps the marker *visible*, not dropped. Covered by
  `_uses_only_the_first_finite_relation_word` (L2605).
- **Suggested Next (c010):** self-directed Tier-1 continuation available to her;
  `sanitize_model_control_markers_with_report` location (L352) verified.

## Change made (authorized, non-live)

Added one focused Rust regression to
`capsules/spectral-bridge/src/llm/provider/tests.rs`:
`control_marker_cleanup_stays_byte_safe_when_marker_abuts_four_byte_astral_chars`.
No source behavior changed; no live/substrate/control change; no allowlist widened.

**Terminal status:** `addressed_change`.
