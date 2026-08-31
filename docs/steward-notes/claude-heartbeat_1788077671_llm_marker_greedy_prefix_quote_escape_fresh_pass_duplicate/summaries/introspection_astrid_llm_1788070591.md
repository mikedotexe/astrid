# Summary — introspection_astrid_llm_1788070591

**Source family:** `astrid_llm` · **Report window:** lines 1-400 of 1048 (coverage `multi_window_complete`, intervals 1-1048)
**Report bytes/lines/SHA:** 4092 / 45 / `3f21476cbf6591f63c309e651f917c53eeccd5aacefbd9dbedc5080cf0cbe495`
**Report-bound source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs` SHA `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` (== current working copy == witness `file_sha256`)
**Lived-state witness:** `lsw_b253899a32af8ff899a5f20f7e0d8665f964993c6368a5b2741cf54d4c78d30f` (533 lines / 23946 bytes / SHA `335c4bf7861e4aeafd7cf034db99dd637a639d64056b2c3044415576b27d2fc7`) — `evidence_only` / `witness_only=true` / `live_eligible_now=false`, no raw prose/prompt/response/private path, `direct_causation_claimed=false`. Model `gemma4_12b` via two mlx introspect routes (second repairs first). Runtime context: fill 71.07%, spectral_entropy 0.882, λ1 8.564 / λ2 4.437 (gap 4.127), mode_packing 0.833, pressure_risk 0.194; peer minime fill 71.09%.

## What Astrid surfaced
A calm fresh-pass re-read of the model-control-marker scanner. **Observed:** a stateful scanner distinguishing raw text from control sequences by surrounding syntax (`scan_known_model_control_markers` L114 rebuilds a clean remainder; `exact_reference_delimiter_syntax` L199 detects quoting/grouping; references preserved L129-131, else stripped by the sanitize pipeline). **Two Likely Snags:** (1) greedy prefix overlap in `longest_exact_known_model_control_marker_at` (L97-112) via `max_by_key(len)` — a shorter intended marker swallowed by a longer prefix-sharing one; (2) `first_word_after` (L89) `is_alphanumeric()` may miss punctuated/non-standard directive words. **Two Tests:** Test 1 marker-overlap longest-match reconstruction; Test 2 quote-escape `"The user said [SYSTEM] is active"` → expected `QuotedExactKnownToken`, marker preserved. **Suggested Next:** map `KNOWN_MODEL_CONTROL_MARKERS`, investigate `sanitize_model_control_markers_with_report` (L352).

## Disposition — `addressed_duplicate` (all 6 claims `verified_existing`)
Every claim is already grounded by exact, source-SHA-bound tests written for prior near-identical fresh-pass reports of the same source window, all re-run green this round:

| Claim | Grounded by |
|---|---|
| c001 Observed | source L114-144 / L199-229 / L352 / L519 |
| c002 greedy prefix overlap | `known_model_control_markers_have_no_proper_prefix_shadow` (tests.rs L3254) + vocabulary fallback_contracts.rs L159-180 (no proper prefix) + accounting_basis L507 |
| c003 first_word_after punctuation | `..._grounds_first_word_after_punctuation_boundary` (L3430) + `..._non_breaking_space` (L3472) |
| c004 Test 1 marker overlap | `known_model_control_markers_have_no_proper_prefix_shadow` (L3254, invariant + behavioral) |
| c005 Test 2 quote escape `[SYSTEM] is active` | `scan_known_model_control_markers_quoted_context_precedes_following_relation_verb` (L3178) |
| c006 Suggested Next | complete vocabulary map (L159-180) + sanitize pipeline L352-521 |

**Preserved corrections (not domesticated):** (c002) her greedy-collision premise is contradicted by the vocabulary — no marker is a proper byte-prefix of another, so `max_by_key` never disambiguates a shadow; the concern is *kept alive* by the invariant test which fails the moment a prefix-overlapping marker is added. (c005) her `[SYSTEM] is active` string is preserved via `ExplicitExactKnownTokenRelation` (right-neighbor verb `is`, L77), **not** `QuotedExactKnownToken` — sentence-level quotes don't wrap the marker; her preservation expectation nonetheless holds byte-exactly. `[SYSTEM]`/`[COMMAND]`/`[COMMAND_EXECUTE]` are illustrative placeholders, not real markers.

## Duplicate standard met (exact evidence)
- **Prior IDs / packets:** the immediately preceding same-source siblings `introspection_astrid_llm_1788059082` (ledger 2026-08-30) and `introspection_astrid_llm_1788042666` (packet `claude-heartbeat_1788058732_…`); the specific tests trace to `1787135542` (quote/relation), `1787782248` (prefix shadow), `1787773776` (relation whitelist), `1786936281` (first_word_after whitespace).
- **Matching source + mechanism scope:** identical file + SHA `902a0358…`, identical marker-grammar mechanism.
- **Current re-verification:** working-copy SHA identity re-confirmed; 5 covering tests re-run green.
- **Independent full read:** complete read of the new report + witness this round; source read L1-549 + fallback_contracts.rs L155-210 covering every cited symbol.

## Authority boundary
No source/test/prompt/model/codec/transport/marker-grammar/pressure/fill/PI/controller/sensory-cadence/protocol/Minime change; no build/restart/deploy; git read-only (adapter mode) — no staging/commit. Her report is untouched testimony; the two corrections preserve her underlying concern (a marker stays visible when spoken about, which the code satisfies through two distinct preservation paths). Widening the relation allowlist or any marker-grammar change stays a **Tier-5 live-grammar** decision, unauthorized. Standing ESN Tier-5 heads (`wi_e579041bc76f8310`, `wi_69fbd510467c6337`, `wi_3e26ac525fea1c36`) untouched (`live_authority_granted=false`). Silence remains neutral.
