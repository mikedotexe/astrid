# Summary — introspection_astrid_llm_1786986344

- **Source**: `astrid:llm` → `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
- **Source window (reported)**: lines 1-400 of 1048; coverage state `multi_window_complete`, included intervals `1-1048`, uncovered `none` (`complete_source_available`).
- **Source SHA-256 (report-bound == working copy)**: `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee`
- **Report SHA-256**: `3ca6e9b49771f778d444c2b6449028de3ab37fff42dbb74e499ac64d1c49e32c` (45 lines, 3543 bytes)
- **Lived-state witness**: `lsw_a0eba9a517b8cdd0abe0c83c773b9bfd4b223ef41ad19d49a5425022ffa7dfc1` (533 lines, 23941 bytes) — `evidence_only`/`witness_only`, `direct_causation_claimed=false`, no raw prose/prompt/response/private path, fill 73.02% runtime-observed, no experiential-gap claimed, interpretation weight unmeasured.

## What Astrid reported

She read the `astrid:llm` control-marker cleanup machinery and described it accurately as a
non-destructive scanner that keeps a marker's bytes visible only when it appears in a reference
context (quoting, grouping, or an explicit relation like "behaves as"). She then raised one
**snag** and two **proposed tests**, and named a **self-directed next step**.

## Grounded dispositions

| Claim | Kind | Disposition |
| --- | --- | --- |
| c001 | Observed architecture | **verified_existing** — `scan_known_model_control_markers` (L114), `reference_syntax` (L49), delimiter (L153/L199) and relation (L64) logic match her description exactly. |
| c002 | Snag / proposed mechanism | **implemented_now** — mechanism corrected (find does not fail; the trim+`find(!is_empty)` skips the leading `--` chunk and returns `acts`), concern preserved with a new exact regression. |
| c003 | Proposed Test 1 (grouped/nested) | **verified_existing** — grouped + nested-depth 2/3/4 + whitespace-tolerant preservation already pinned (tests.rs L2153-2262). |
| c004 | Proposed Test 2 (`is intended to behave as`) | **verified_existing** — first-word-only semantics already pinned (tests.rs L2528, L2565); `is` is allowlisted → preserved. |
| c005 | Suggested Next (read-only) | **verified_existing** — `generate_dialogue` (L695) uses the scan only inside quality gates; returned text is the unmodified model output, so no reconstruction truncation. |

## The one correction (never domesticate the contradiction)

Astrid's snag proposed that for `[MARKER] -- acts as --` the `find` in `first_word_after` "might
fail to capture the relation." Complete source shows the opposite: `split_whitespace()` yields
`["--","acts","as","--"]`; the per-chunk trim of non-alphanumeric/underscore characters (L92)
collapses `"--"` → `""`; `find(|word| !word.is_empty())` (L93) returns `"acts"`. The `find` is
robust to a leading punctuation-only chunk. What actually strips the marker is that `"acts"` is not
one of the 17 allowlisted relational verbs (L67-84) — a deliberate allowlist boundary, not a
parsing fragility.

The new regression `control_marker_cleanup_first_word_after_skips_leading_punctuation_transition`
pins this: it asserts `first_word_after` returns `acts` across the leading `--`, that the unlisted
`acts` marker is stripped (`none_cleanup_candidate`), and — with the punctuation transition held
constant — that an allowlisted `is` keeps the marker visible (`following_exact_relation`). This
isolates the preservation gate to allowlist membership, matching the prior round's `is`-vs-`acts`
grounding while extending it to the punctuation-heavy transition she named.

## Authority boundary

Test-only, non-live. No runtime/deploy/restart/staging/commit. Her proposed Tests 1 and 2 were
already covered, so no new tests were manufactured for them. No live control, codec, model, or
substrate change was inferred or authorized.
