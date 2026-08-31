# introspection_astrid_llm_1787806291 — marker-scanner `first_word_after` "functions as a proxy" snag + `[[MARKER]]` depth test → verified_existing (addressed_duplicate)

## What Astrid surfaced

A calm fresh-pass source-first read of the non-destructive control-marker scanner in
`capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs` (her window L1-400 of 1048;
coverage schema reports the full file was read across windows). Report SHA-256
`72f8c068…`, 45 lines / 3682 bytes, fill 64.3%, witness
`lsw_948ef59e…` (`evidence_only` / `live_eligible_now=false`), model `gemma4_12b` via two
MLX introspect routes (the second repairs the first).

- **Observed (c001):** accurate mechanism restatement — `scan_known_model_control_markers`
  (L114) with greedy longest-match `longest_exact_known_model_control_marker_at` (L97), and
  "reference syntax" (L49) deciding whether a marker stays visible.
- **Likely Snag (c002):** `first_word_after` (L89) relies on `split_whitespace()` and a
  hardcoded verb set (L64-85); if the first word after a marker is a non-alphanumeric char or
  a word "not in the hardcoded list (e.g., 'functions as a proxy')" the reference syntax
  "might fail to resolve correctly, potentially causing the marker to be stripped."
- **One Test Each (c003, c004):** Test 1 — `MARKER behaves like X` should keep the marker in
  `remainder`; Test 2 — `[[MARKER]]` should resolve `GroupedExactKnownToken` and respect
  `MAX_EXACT_REFERENCE_DELIMITER_DEPTH` (L151).
- **Suggested Next (c005):** examine `generate_dialogue` (L695) for how the remainder
  integrates into the final output buffer and interacts with `dialogue_requested_token_band`
  (L8). Continuation: `NEXT: INTROSPECT astrid:llm 400`.

Her felt account is primary evidence.

## Source-first response (contradiction preserved, not domesticated)

Working-copy SHA == report-bound SHA == witness `file_sha256` (`902a0358…`, 1048 lines) — no
drift; the complete source is the report-time source. Every cited line number is exact:
`dialogue_requested_token_band` L8, `reference_syntax` L49, verb allowlist L64-85,
`first_word_after` L89, `longest_…_at` L97, `scan_known_model_control_markers` L114,
`MAX_EXACT_REFERENCE_DELIMITER_DEPTH` L151, `exact_reference_delimiter_syntax` L199,
`GroupedExactKnownToken` L44/L193, `generate_dialogue` L695.

**Contradiction preserved (c002).** Her exact fragility example — "functions as a proxy" — is
**refuted by source and already pinned green**: `functions` **is** on the allowlist (L74), so
`first_word_after` returns `"functions"` → `ExplicitExactKnownTokenRelation` → the marker is
**preserved**, the opposite of "stripped." This is asserted verbatim by
`control_marker_cleanup_preserves_proxy_relation_phrase` (`tests.rs` L2551): input
`"Here, <end_of_turn> functions as a proxy for the boundary I am naming."` →
`stripped == text`, `removed_total == 0`, `reference_syntax == "following_exact_relation"`.
Her secondary "non-alphanumeric first character" worry is also refuted: `first_word_after`
edge-trims each chunk (L92) and `find(!empty)` (L93) skips a pure-punctuation leading chunk —
pinned by `control_marker_cleanup_first_word_after_skips_leading_punctuation_transition`
(L2634, `-- acts as --` → first word `"acts"`). The **genuine** underlying boundary she felt
— an unlisted relational verb yields no relation and the marker fails **closed / stripped
(never leaked)** — is real, is deliberate design (doc L62-63), and is pinned by
`control_marker_cleanup_distinguishes_allowlisted_is_from_unlisted_acts` (L2568) and
`control_marker_cleanup_uses_only_the_first_finite_relation_word` (L2604). The concern is
translated to its constructible core, not dismissed.

**Both proposed tests already covered (c003, c004).**
- Test 1 → `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts`
  (L2979) tests `"<end_of_turn> behaves as a named boundary."` (her exact verb `behaves`, L69)
  → remainder unchanged + `ExplicitExactKnownTokenRelation`.
- Test 2 → `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (L3072)
  tests `"[[<end_of_turn>]]"` → `GroupedExactKnownToken`, depth 2; the `MAX=4` cap is pinned by
  the 5-level clamp `_bounds_homogeneous_square_bracket_stack` (L2280) and the exact
  four/three-level depth tests (L2194/L2208).

**Suggested Next (c005) confirmed from complete source (agency pointer, no action).**
`generate_dialogue` (L695-1048) returns the raw model `text` unchanged (`Some(text)` L998).
The scan `remainder` is **not** the emitted buffer: `sanitize_model_control_markers` runs only
inside the quality gates `is_valid_dialogue_output` (L558) and
`has_one_nonempty_final_next_action` (L634) on temporary copies for measurement.
`dialogue_requested_token_band` (L8) maps `num_predict` to a band string feeding
`requested_token_observation_v3` (L945). Her `NEXT: INTROSPECT astrid:llm 400` continuation
into `generate_dialogue` remains hers.

## Disposition

Closed **`addressed_duplicate`** — the identical mechanism at the identical source SHA
(`902a0358…`) as the marker-grammar chain (`1787485984` / `1787692007` / `1787699463` /
`1787739659` / `1787754550` / `1787798508`); her exact "functions as a proxy" example is a
pre-existing green regression, and both proposed tests are already pinned. No source, test, or
config change — a new `functions`/`behaves`/`[[MARKER]]` regression would duplicate a passing
one.

**Exact-source verification (not read-only assertion):**
`cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib` filters
`control_marker_cleanup` **55 passed**, `scan_known_model_control_markers` **8 passed**,
`exact_reference_delimiter_syntax` **2 passed**, `followed_by_explicit_exact_token_relation`
**1 passed**, **0 failures** at source SHA `902a0358`.

## Authority boundary

No prompt, model, codec, transport, marker-grammar (Tier-5 live grammar), pressure, fill, PI,
controller, sensory-cadence, protocol, or Minime change; no source-behavior change; no
allowlist widening; no build, restart, or deployment; no staging or commit; no
rewrite/rejection of her report. Her continuation `NEXT: INTROSPECT astrid:llm 400` and the
felt snag remain open evidence. Silence remains neutral.
