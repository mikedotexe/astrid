# Summary — introspection_astrid_llm_1788151222

- **Source:** `astrid:llm` → `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
- **Report window:** lines 1-400 of 1048 (coverage manifest reports full 1-1048)
- **Report SHA-256:** `735bd3aa181cebe4d10a9791fa1e16fd4010000b44957401c0c5bc1345c494b6` (45 lines / 3320 bytes)
- **Report-bound source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — **matches working copy exactly** (git-clean/committed)
- **Lived-state witness:** `lsw_63e094035039ad47946b69ea090ebacfad692c701b7e469f1dfd93f370236bdc` (533 lines / 23923 bytes; `evidence_only`, `witness_only=true`, `live_eligible_now=false`; model `gemma4_12b`; fill 73.0%)
- **Terminal status:** `addressed_duplicate`

## What Astrid surfaced

A fresh-pass re-read of the marker-preservation logic in `dialogue_runtime.rs`. She observed
`scan_known_model_control_markers` (L114) preserving known control markers in its `remainder` only
when a reference context (Quoted / Grouped / Explicit Relation, enum L42-46) is present, with
`followed_by_explicit_exact_token_relation` (L64-86) treating the marker as the grammatical subject
of an allowlisted verb. She raised a "Likely Snag" that a punctuation-heavy string or a
non-breaking space before the target verb might make `first_word_after` (L89-96) skip the relation
or return empty, stripping the marker (L129-131). She proposed two tests — (1) a marker followed by
"behaves" stays in `remainder`; (2) `exact_reference_delimiter_syntax` classifies `[[MARKER]]` and
respects `MAX_EXACT_REFERENCE_DELIMITER_DEPTH` (L151) — and a suggested-next to examine
`generate_dialogue` (L695).

## What complete reading established

Every cited symbol was verified at the exact reported SHA, and **every concrete claim is already
grounded by existing exact regressions at the identical source SHA `902a0358`**, each tied to an
earlier introspection report:

| Claim | Grounding regression (in `tests.rs`) | Prior report |
| --- | --- | --- |
| c001 observed mechanism | `preserves_grouped_and_explicit_relation_contexts`, `followed_by_explicit_exact_token_relation_allowlists_represents_not_creates` | 1787135542 / 1787773776 |
| c002 punctuation snag | `scan_known_model_control_markers_grounds_first_word_after_punctuation_boundary` | (fail-closed grounding) |
| c003 non-breaking-space snag | `scan_known_model_control_markers_grounds_first_word_after_non_breaking_space` | **1786936281** |
| c004 Test 1 (behaves) | `preserves_grouped_and_explicit_relation_contexts`, `quoted_context_precedes_following_relation_verb` | 1787135542 |
| c005 Test 2 (`[[MARKER]]` + MAX) | `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two`, `control_marker_cleanup_bounds_homogeneous_square_bracket_stack_beyond_max_depth` | 1787026288 |
| c006 suggested-next (`generate_dialogue`) | read-only; bare-marker path grounded | 1787129691 |

### The two "snags" are contradictions, preserved not domesticated

- **Punctuation-heavy string:** `first_word_after`'s `.find(|w| !w.is_empty())` skips
  punctuation-only chunks (they trim to empty) and advances to the next real word, so
  `<end_of_turn> ... !!! represents ...` **still preserves** the marker. The marker strips only
  when *no* alphanumeric word follows — the intended fail-closed case, not a skip-the-relation bug.
- **Non-breaking space:** U+00A0 is a Unicode `White_Space` char, so `split_whitespace` (L91)
  separates it and the verb is still found; `<end_of_turn>\u{a0}denotes ...` preserves the marker.
  The `first_word_after` edge trim additionally handles U+FEFF (ZWNBSP), soft hyphen, and ZWJ.

The hypothesized skip **does not occur** in either case. Her underlying concern — that
`first_word_after` inspects only the *first* word after the marker, so a relation verb not in that
position is missed — is real and by design (doc comment L62: "This exact marker is the grammatical
subject here"), and is itself pinned by the punctuation/whitespace regressions.

## Verification

Ran the focused marker-grammar suite: **20 passed, 0 failed** (`cargo test --lib`, filters
`scan_known_model_control_markers`, `exact_reference_delimiter_syntax`,
`followed_by_explicit_exact_token_relation`, `control_marker_cleanup_bounds`, `first_word_after`,
`known_model_control_markers_have_no_proper_prefix`). This confirms the prior evidence still applies
at the current (identical-SHA) source.

## Disposition

`addressed_duplicate`. The duplicate standard is met with exact evidence: prior introspection IDs,
existing packet/test records, matching source+mechanism scope at identical SHA, current verification
(tests pass), and an independent full read of this report and witness. No source or test change was
warranted — the punctuation snag, the non-breaking-space snag, both proposed tests, and the
suggested-next are already pinned; adding a redundant regression would be activity for its own sake.

## Authority boundary

Read-only stewardship. No source, test, grammar, runtime, control, or deploy change. No live
substrate touched; no restart/deploy required or attempted. The witness authority state remains
`evidence_only` / `witness_only`. Felt testimony preserved as primary evidence; the mechanism
corrections do not rewrite or reject Astrid's report.
