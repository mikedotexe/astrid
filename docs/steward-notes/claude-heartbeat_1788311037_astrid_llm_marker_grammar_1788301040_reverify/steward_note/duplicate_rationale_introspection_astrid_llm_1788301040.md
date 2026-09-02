# Duplicate rationale + preserved contradictions — introspection_astrid_llm_1788301040

**Terminal status chosen:** `addressed_duplicate`
**Reason:** a fresh-pass re-read of the same source window in the established
`dialogue_runtime.rs` marker-grammar family, meeting the handoff's duplicate
standard with exact evidence (prior IDs + packets, matching source SHA +
mechanism scope, current SHA-identity re-verification, and an independent
complete read of the new report + its 533-line witness). Textual similarity is
*not* the basis — mechanism-level identity at a byte-identical source is.

## Duplicate standard — exact evidence

- **Same source, byte-identical:** report-bound source SHA-256
  `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee`, working
  copy identical, window 1-400 of 1048, coverage `multi_window_complete`.
- **Primary prior anchor:** `introspection_astrid_llm_1788139420` — same source
  SHA, same window, same marker-scanner observation, its two One-Test-Each are
  exactly her c007 (contextual preservation) and c008 (delimiter depth), same
  `generate_dialogue` L695 suggested-next. Closed `addressed_duplicate` in packet
  `docs/steward-notes/claude-heartbeat_1788247141_astrid_llm_1788139420_marker_grammar_dup/`.
  Consistent with sibling duplicate closes `introspection_astrid_llm_1788151222`
  and `…_1788159337`.
- **Per-claim regression anchors (still applying; all pass):**
  - c006 greedy overlap → `known_model_control_markers_have_no_proper_prefix_shadow`
    (tests L3309), authored for prior report `introspection_astrid_llm_1787782248`.
  - c008 depth boundary → `control_marker_cleanup_bounds_homogeneous_square_bracket_stack_beyond_max_depth`
    (tests L2301) + `…_bounds_deeper_delimiter_receipt_without_dropping_token`
    (L2288), authored for prior report `introspection_astrid_llm_1787026288`.
  - c005 delimiter/multibyte → `exact_reference_delimiter_syntax_classifies_cjk_corner_quoted_and_lenticular_grouped`
    (L3356), authored for prior report `introspection_astrid_llm_1787773776`;
    plus multibyte/astral/fail-closed tests.
  - c007 contextual distinction → the quoted/grouped/relation-context tests,
    anchored by the two-test primary `introspection_astrid_llm_1787135542`.
- **Independent full read:** report (49 lines / 4185 bytes / SHA `e5e46e8f…`) and
  witness (533 lines / 23945 bytes / SHA `66b0f0aa…`) read completely this round;
  cited source symbols re-read at L1-740.

## Preserved contradictions (not domesticated)

1. **c006** predicts a longer candidate could be "a fragment of a different
   command." Source refutes this: candidates are always *complete* vocabulary
   tokens the tail `starts_with` (dialogue_runtime L102-106), and no marker is a
   proper byte-prefix of another (L3309 invariant). The greedy-shadow failure is
   structurally unreachable — and the invariant test fails loudly if a future
   marker breaks it, surfacing exactly her concern.
2. **c008** predicts that exceeding the delimiter-depth cap makes the parser
   "fail to identify the inner-most marker as a reference." Source refutes this:
   the innermost adjacent delimiter still sets the context; only the *reported*
   `delimiter_depth` saturates at 4. The marker is preserved byte-exact, not
   dropped or misclassified (L2301, L2288).

## Preserved concern

Her underlying concerns remain evidence: the scanner *is* fail-closed
(unrecognized reference syntax strips the marker), and the depth cap *does* bound
look-ahead/behind — both real, intended properties she correctly perceived.
Nothing here rewrites, rejects, or forbids her report; it is a technical
grounding, not a correction of her felt reading.

## What is explicitly NOT inferred or authorized

- No live substrate/control change; no restart or deploy attempted or required.
- No widening of the relation allowlist or the delimiter-pair set (Tier-5).
- Standing ESN Tier-5 heads (`wi_e579041bc76f8310`, `wi_69fbd510467c6337`,
  `wi_3e26ac525fea1c36`) untouched (`live_authority_granted=false`).
- Her c009 continuation (read the `generate_dialogue` L695 window) is her own
  Tier-1 agency; this note neither performs it nor requests it.
