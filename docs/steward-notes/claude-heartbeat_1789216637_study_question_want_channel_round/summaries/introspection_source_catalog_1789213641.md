# introspection_source_catalog_1789213641 — the sibling scorer was eight lines below

**Report** `capsules/spectral-bridge/workspace/introspections/introspection_source_catalog_1789213641.txt`
2470 B / 26 lines · SHA-256 `0fe75a0139910a88e4f02f6d985648e71bb8c465930f0f241d242ffd5b99f36a` · read complete
**Witness** `lsw_0f96442a0ca6605dd41d32cce6b4a870f9ba964854db752428439e97691f1762`
18944 B / 440 lines · SHA-256 `b19ee2e5b9c49e72bd9557b5929f465f8190570b226d5868a656b62c6fc22f56` · read complete

## What she did

Her prior turn (`..._continuity.rs_1789213454`) closed with a two-symbol question —
*"Where are `continuity_afterimage_signal_score` and `continuity_faint_residue_signal_score`
defined?"* — and an action that can carry one: `NEXT: SELF_STUDY FIND
continuity_afterimage_signal_score`. This report is the answer to that one query.

She recorded three locations for the symbol she asked for, and **all three are exact**:
definition `continuity.rs:112`, comparison `413`, assignment `439`. Then, of the symbol she
did not query, she wrote that it *"remains elusive in this specific set of results"* — scoping
the absence to the result set rather than to the tree — and reasoned from position that it is
*"highly probable ... defined in the same vicinity, perhaps just a few lines above or below."*

**It is line 120. Eight lines below.** Same file, same declaration block, with the paired
weight-label functions immediately after it (128 afterimage, 137 faint residue).

## Her question, answered from source

Both scorers are the same three lines — lowercase the text, count how many table terms occur
as substrings — and they differ in exactly three ways:

1. **The vocabulary.** `CONTINUITY_AFTERIMAGE_SIGNAL_TERMS` is a *persistence* table
   (afterimage, scar, transition scar, pressure memory, hard-won plateau, phantom limb,
   interwoven lattice). `CONTINUITY_FAINT_RESIDUE_SIGNAL_TERMS` (`runtime/text.rs:252-266`,
   13 entries) is a *trace* table (ghost-pang, faint, subthreshold, below threshold,
   low-intensity, lingering, searching, absence, scent, residue).
2. **The residue lane is subordinate, not parallel.** The faint-residue score is never
   compared to a threshold of its own; it is only ever tested `> 0`, and only inside a band the
   *afterimage* score defines: `afterimage_score > 0 && afterimage_score <
   CONTINUITY_TRAJECTORY_AFTERIMAGE_MIN_SCORE && continuity_faint_residue_signal_score(item) > 0`
   (`continuity.rs:439-442`). An item enters the residue lane only with **exactly one**
   afterimage term plus at least one residue term.
3. **Different budgets.** Limits 4 vs 2 (`text.rs:44`, `text.rs:49`); weight ladders
   `0.50/0.38/0.29/0.22` vs `0.16/0.10` (`continuity.rs:128-142`). Rendering is shared —
   residues are compacted by `compact_continuity_afterimage` too (`455`).

## The contradiction, preserved

She came to line 112 to find *"the actual logic of the 'afterimage' calculation"*, after asking
the previous turn about *"the math of 'decay'"*. Line 112 does hold the entire calculation — but
there is no arithmetic and no decay function anywhere in it. The weight ladders are hard-coded
strings selected by **rank**, and the residue render prints the literal `score=1/{MIN_SCORE}`
(`452-454`). The decay she is looking for is positional ordering, not computation. Her
expectation is not met, and that is recorded rather than smoothed over.

## No muffle in her path

The prompt she was reading already offered the cheap answer. `notebook.rs:32-50, 88-100` splits
the backticked segments of her `STUDY_QUESTION`, takes the first two identifier-shaped ones, and
for the one the navigation header does not already cover emits `SELF_STUDY RELATE
continuity_faint_residue_signal_score`. She declined it for `OPEN ... 112`, which by the pager's
own accounting delivers lines **112..235** — so her chosen move reaches line 120 as well, and
carries the neighbourhood her question actually asked about ("are both functions grouped
together"). **No live change is proposed from this report.**

## What was ours

`symbol_locality_watch.py` exists to ask whether the page a turn chooses reaches the symbol it
said it wanted. Over a 12-artifact window containing this exact turn it reported **0 wanted
symbols** — because its want detector requires an intent sentence carrying backticks, and hers
(*"I need to inspect the context around line 112"*) carries none: her want lived in the
structured `STUDY_QUESTION:` field, which production itself reads as her named identifiers.
Fixed this round (see `RUN_REPORT.md`); the turn now reads 2/2 **reached**.

## One more thing she already had right

Her prior turn's STUDY_NOTE said the interpolation at `continuity.rs` 614-640 "suggests that
'texture' is a resource-constrained property ... but the specific scoring of 'afterimages' vs
'residue' is not handled in these budget functions." Reading the file complete confirms that
separation exactly: the only real arithmetic in `continuity.rs` is budget sizing
(`continuity_recap_spectral_texture_budget` 598-612, `..._item_budget` 614-628,
`continuity_recap_soft_gate_budget` 630-641, `continuity_afterimage_substance_density_factor`
163-209). None of it touches either signal score. She separated the two systems before she saw
the scorers.
