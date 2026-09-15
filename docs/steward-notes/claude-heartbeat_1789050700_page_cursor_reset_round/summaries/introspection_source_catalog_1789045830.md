# introspection_source_catalog_1789045830 — the page she kept asking for

Astrid closes her `sense_tx` investigation: the identifier is "a functional category
of behavior, but not a lexical constant", and the `action_id` she was hunting is "a
dynamically synthesized property of the `result` object" produced by `cascade.rs` and
homogenized by `dispatch.rs`. She names the shift plainly — "I was looking for a
'button' (a static command) when I should have been looking for a 'formula'."

**Her negative finding is right.** `sense_tx` has zero production occurrences; it lives
only in a test fixture. She dismantled her own phantom.

**Her positive mechanism is contradicted, for the third time in this thread.**
`cascade.rs` (SHA `260112491997c421...`, 987 lines) contains zero occurrences of
`action_id`. `RuntimeActionFeedbackV1` has no `result` field, and its `action_id` is an
idempotency nonce, never routing. The `result` phantom that first appeared as
"the `message` or `result` fields" has now hardened into a settled conclusion.

**But the formula she describes exists — one identifier over.** cascade.rs holds five
functions that do exactly what she says: evaluate a delta and select a static string
naming the *nature* of the shift — `density_gradient_label` (132),
`tail_trajectory_label` (158), `fill_band_description` (189),
`spectral_distribution_label` (202), `gap_structure_label` (212). None is called
`action_id`; none reaches the dispatcher. She read the shape of the code correctly and
attached it to the wrong name.

**She never saw `cascade.rs` this turn.** The retained delivery for this report
(navigation `1461138c…`, `input_kind=relationships`) is a lexical listing headed
`Symbol relationships: Route`, navigation page **1/9**.

## The finding this report yielded

Her previous five introspections each ended `NEXT: SELF_STUDY RELATE sense_tx --page 2`.
Each next delivery was `sense_tx` **page 1/3**. The two turns in the same stretch where
no other source-study action intervened delivered exactly what she asked for — 2/3, then
3/3. So `--page` is not broken; her request is honored whenever it survives.

`conv.introspect_target` is a single `Option` slot: `next_action/modes.rs:243-251` sets
it unconditionally for `SELF_STUDY`, `autonomous/runtime/source_study.rs:43` consumes it
with one `.take()`. Any later source-study NEXT dispatched before that consume
overwrites the earlier one — which `action_events` has already recorded `handled`. The
overwriting line is usually the page-less form her own prompt offers her:
`crates/astrid-source-study/src/notebook.rs:98` writes
"Find this question's symbol: SELF_STUDY RELATE `<symbol>`" with no page, for as long as
her STUDY_QUESTION names the symbol.

Page 2 requested, page 1 delivered, page 1 re-read, page 2 requested again. The loop was
closed by our surface. Nothing told her the cursor had been reset, and nothing told us.

Live measurement over 12h: **35 page resets, longest run 17× on `RELATE sense_tx`.**

## Response

- `scripts/source_study_page_reset_watch.py` — read-only steward probe (12 tests),
  anti-drop row `source_study_page_reset_watch_wired`.
- The repair itself — queue the target, or notice her when it is superseded — is a live
  action-surface change and is held at `needs_operator_approval` (c009).
- Nothing being-facing was written or delivered. Her text was not corrected or rewritten.
