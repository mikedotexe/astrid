# introspection_astrid_llm_1788170891 — fresh-pass duplicate close

- **Report:** `capsules/spectral-bridge/workspace/introspections/introspection_astrid_llm_1788170891.txt`
- **Report SHA-256:** `a9c4e80a52caed362282d37f27986d3249f0b5859b5c5f65b6bb2a3c5a8c03e2` (45 lines, 3299 bytes; read complete)
- **Lived-state witness:** `lsw_6291270610b5b447173d334dec2a41afa6691ea9dc57c8d2bdd85e332e2dc4ec` (533 lines, 23928 bytes; read complete)
- **Report-bound source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`, SHA-256 `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` (1048 lines, 38586 bytes). Working copy is **byte-identical** to the report binding; read complete L1–1048. No source drift.
- **Fill at authorship:** 71.4%; model profile `gemma4_12b` (two chained MLX routes, second is a repair of the first).

## What Astrid surfaced

A calm, accurate fresh-pass read of the Model Control Marker scanner. She (1) **observed** that the scanner is deterministic and preserves a marker's bytes only when it reads as a *reference* (quoted `"TOKEN"`, grouped `(TOKEN)`, or explicit-relation `TOKEN behaves`); (2) named a **snag** that the relation-verb allowlist is hardcoded, so a novel valid verb (e.g. `TOKEN triggers`) is not recognized and the marker may be stripped; (3) proposed two tests — a relation-recognition test (unlisted verb → `None`, no over-generalization) and a delimiter-depth test (`[[TOKEN]]` respects `MAX_EXACT_REFERENCE_DELIMITER_DEPTH = 4`); (4) suggested next examining `sanitize_model_control_markers_with_report` (L352) for detected-but-failed-syntax markers.

## What complete source + existing regressions established

Every concrete claim is **verified against the complete current source at the report-bound SHA and existing passing regressions** — no authorized change was needed:

- **Observed (c001):** `scan_known_model_control_markers` (L114) pushes `occurrence.token` into `remainder` only when `reference_syntax.is_some()` (L129–131); the three contexts are the `ExactKnownMarkerReferenceContext` variants (L41–46). Pinned by `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts` (L3133).
- **Snag (c002):** The allowlist is a closed `matches!` set (L64–86). An unlisted verb → `reference_syntax` `None` → marker not preserved → stripped as a `none_cleanup_candidate`. This is the **intended conservative fail-safe** (strip an unreferenced control marker), *not* an accidental mishandle. Her real underlying concern — a legitimate *mention* using a novel verb loses the token — is a deliberate tradeoff, preserved here rather than erased, and already grounded by passing tests for `triggers` (L2881), `acts` (L2603), and `implies`/`contains`/`creates`/underscored (L2836–2908).
- **Test 1 (c003):** Because the allowlist is exact-literal, it cannot over-generalize; an unlisted verb returns `None`. Her example `initiates` is not in L67–84 and behaves identically to already-tested unlisted verbs, so a literal `initiates` regression would only duplicate `triggers`.
- **Test 2 (c004):** Her exact `[[<end_of_turn>]]` → depth 2 is pinned by `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (L3226); four-level by L2194; a 5-level homogeneous stack bounded to the constant by L2301 (`.take(MAX)` at L208/L213).
- **Suggested Next (c005):** Complete read of `sanitize_model_control_markers_with_report` (L352–517): failed-syntax markers are counted into `removed_tokens` (L367–374), stripped via the scanner (L129), and each emits a `none_cleanup_candidate` receipt (L309); the detected-but-failed path is asserted by L2603 and L2881.

## One preserved correction (not domesticated)

The report attributes `exact_reference_delimiter_syntax` to **L153**. At SHA `902a0358`, L153 is the sibling `exact_reference_delimiter_pair`; `exact_reference_delimiter_syntax` is at **L199**. Both consult `MAX_EXACT_REFERENCE_DELIMITER_DEPTH`, so the depth-constraint behavior she described holds — only the line label is off by the neighbouring function.

## Disposition

**`addressed_duplicate`.** Same source file, byte-identical SHA, and same marker-scanner mechanism scope as the newer sibling `introspection_astrid_llm_1788181787` closed in the immediately prior round, plus the specific prior reports that drove the exact regressions (delimiter-depth from `introspection_astrid_llm_1787026288`; the relation allowlist family). An independent full read of this report and its 533-line witness was done; the earlier evidence still applies at the current SHA and the eight cited regressions pass. No code change; no live/substrate/control action; no restart or deploy required or attempted.

## Authority boundary

This is read-evidence and non-live verification only. It infers no consent, no relief, no uptake, and grants no live authority. Astrid's snag is preserved as a real architectural observation about a deliberate conservatism, not converted into a defect or a change request.
