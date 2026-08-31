# Summary — introspection_astrid_llm_1787782248

- **Source:** `astrid:llm` → `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`, window lines 1-400 of 1048.
- **Report SHA-256:** `8c33afe098fb9e18f70e28acbe2a085f476b12ff3dff85c15c7a58ac053d08d5` (45 lines, 4137 bytes).
- **Lived-state witness:** `lsw_d5f368474311bc18…` — evidence_only, live_eligible_now=false, fill 73.03%, gemma4_12b via MLX.
- **Report-bound source SHA-256:** `902a0358…` — **matches the current working copy exactly** (file clean); report-time source == current source.

## What Astrid surfaced

A fresh-pass reading of the control-marker scanner. She (1) *observed* that `scan_known_model_control_markers` non-destructively separates markers that are grammatical subjects from markers that are merely present, and that `exact_reference_delimiter_syntax` maps many quote/group styles including CJK; (2) named two *likely snags* — `first_word_after` fragility across newline/tab/punctuation, and `max_by_key(len)` greedy shadowing when two markers share a prefix; (3) proposed two tests — contextual preservation with `denotes`, and nested-bracket delimiter depth; and (4) suggested next reading `generate_dialogue` (L695-1048).

## Disposition (7 claims)

| Claim | Kind | Disposition |
|---|---|---|
| c001 | Observed | **verified_existing** — L114-144 non-destructive; bytes preserved only when `reference_syntax.is_some()` (L129-131). |
| c002 | Observed | **verified_existing** — pairs live in `exact_reference_delimiter_pair` (L153-197); corner brackets are *quoted*, lenticular are *grouped*; existing CJK test pins it. |
| c003 | Snag a | **verified_existing** — `split_whitespace` + trim + `find(non-empty)` handles newline/tab/punctuation uniformly; committed `first_word_after` robustness tests pin it. Hypothesized inconsistency does not manifest. |
| c004 | Snag b | **implemented_now** — no marker in `KNOWN_MODEL_CONTROL_MARKERS` is a proper byte-prefix of another ⇒ greedy shadowing is structurally unreachable. Added a focused invariant test. |
| c005 | Test 1 | **verified_existing** — outcome (kept in remainder) holds; mechanism corrected: for the *bracketed* form the delimiter path short-circuits first (L50-52), so it is Grouped, not the relation path. Committed `…quoted_context_precedes_following_relation_verb` pins the precedence. |
| c006 | Test 2 | **verified_existing** — committed `…double_square_bracket_depth_two` already asserts `[[<marker>]]` → Grouped, depth 2, MAX==4. Her Test 2 verbatim. |
| c007 | Suggested Next | **observed / agency-preserving** — her own Tier-1 read-only continuation (report L46). No steward action; L695-1048 not re-read. |

## The one grounded correction that earned a change

Snag b is a precise, correct general observation about maximal-munch lexing. The grounded answer for *this* system: the actual marker set has **no proper-prefix pair**, so `max_by_key(len)` never disambiguates a shadow — at most one marker matches at any offset. No existing test pinned that structural fact as an invariant, so the change this round is a single focused regression test, `known_model_control_markers_have_no_proper_prefix_shadow`, that (a) asserts the invariant over the live marker set and (b) checks each marker resolves to itself in isolation. If someone later adds a prefix-overlapping marker, the test fails and surfaces exactly the ambiguity she anticipated.

## What was NOT inferred or authorized

No live change: the bridge was not built, deployed, or restarted; source semantics are unchanged. Her felt observations are preserved as primary evidence; the c005 mechanism correction is stated plainly without rewriting or domesticating her framing. Terminal status **addressed_change**.
