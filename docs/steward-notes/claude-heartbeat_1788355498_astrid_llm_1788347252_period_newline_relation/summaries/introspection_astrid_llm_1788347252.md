# Summary — introspection_astrid_llm_1788347252

**Source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs` (window lines 1-400 of 1048; coverage `multi_window_complete`)
**Report SHA-256:** `cfddd3fb5fadd7670b529b6d90e780dde343c0cf50a5fe168e42e819baf29a5c`
**Source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` (working copy = report-bound, exact match)
**Lived-state witness:** `lsw_501399a0...fde0` (evidence_only / witness_only, gemma4_12b via mlx, fill 73.0%)

## What Astrid surfaced

Astrid read the marker-preservation machinery and (a) *Observed* that `scan_known_model_control_markers` (L114) classifies a marker by grammatical context — quoted, grouped, or explicit-relation — via the `ExactKnownMarkerReferenceContext` enum (L42); (b) raised a *Likely Snag*: `first_word_after` (L89), which relies on `split_whitespace()` and an alphanumeric filter, "might skip the intended verb or return an empty string" when a marker is followed by a punctuation-heavy string or a **non-breaking space** before the relation verb (e.g. `behaves`), causing `followed_by_explicit_exact_token_relation` (L64) to fail and stripping a marker that should be preserved; (c) proposed **Test 1** (a marker followed by `behaves` separated by a **period or a newline**, `[MARKER]. behaves as...`) and **Test 2** (nested `[[MARKER]]` respecting `MAX_EXACT_REFERENCE_DELIMITER_DEPTH`, L151); (d) *Suggested Next*: verify `first_word_after` against punctuation-heavy delimiters.

## What complete source reading established

All five cited line numbers are exact against the complete source. The proposed skip is **contradicted** by the source and preserved, not domesticated:

- `split_whitespace()` (L91) uses Unicode `White_Space`, which **includes** U+00A0 NO-BREAK SPACE — so NBSP *is* a separator, not a defeater. A newline is also `White_Space`.
- The per-chunk `trim_matches` (L92) strips leading/trailing non-alphanumeric chars, then `find(|w| !w.is_empty())` (L93) **skips** a standalone `.` / `:` chunk and returns the verb.
- A marker is retained only when `reference_syntax.is_some()` (L129-131); grouping/quoting (L50) is checked before the relation path (L53).

Her NBSP case is already grounded by `scan_known_model_control_markers_grounds_first_word_after_non_breaking_space` (tests.rs L3580). Test 2 (delimiter depth) is already grounded: `((<end_of_turn>))` depth 2 (L2272), unicode-whitespace bracket depth 2 (L2256), homogeneous `[[[[[<end_of_turn>]]]]]` capped at MAX=4 (L2301), heterogeneous 4-deep (L2287).

## What changed / was verified

- **Added** one focused regression `control_marker_cleanup_first_word_after_skips_period_and_newline_before_relation` (tests.rs) grounding the two remaining separators she named — **period** and **newline** — with bare (relation-path), bracketed (grouping-path, depth 1), and unlisted-verb (fail-closed) cases. Passes.
- **Verified existing:** c001 (line citations + scanner behavior), c004 (delimiter depth + MAX), c005 (punctuation-heavy delimiter family).

## Authority boundary

Read-evidence and a non-live focused test only. No grammar was widened, no marker allowlist changed, no live substrate or control change. The felt concern (that a separator could silently strip a preservable marker) is preserved as the reason the regression exists, even though the proposed mechanism does not manifest.
