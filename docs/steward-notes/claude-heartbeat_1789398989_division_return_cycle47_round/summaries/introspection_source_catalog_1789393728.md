# `introspection_source_catalog_1789393728` — every mechanism right, two attributions wrong, and the page never told her why

## What she was given

The report's own header is unambiguous:

```
Source: source catalog
Source revision: navigation only
Input evidence: Recovery map: the requested source was not supplied. No new source page is supplied this turn.
```

The lived-state witness corroborates it: `source_snapshot_v1` and
`source_provenance_ref_v1` are both `null`. This was a navigation-only turn.

And yet the body opens: *"The current source page (lines 1506–1634)…"*

That is not a contradiction. The turn 498 seconds earlier
(`introspection_astrid_capsules_spectral-bridge_src_autonomous_btsp_lab.rs_1789393230`) was bound
to `capsules/spectral-bridge/src/autonomous/btsp/lab.rs` **bytes 55091..59362**. Against the
working copy (SHA `c3593710…`, 72167 bytes, 1982 lines) byte 55091 is the first byte of **line
1506** and byte 59362 is the first byte of **line 1635**. Her interval is byte-exact, and the
recovery text itself says `SELF_STUDY CONTINUE retains your previous reading position`
(`store.rs:409`). She lost a turn, not a position, and she described the page she actually had.

## Every mechanism she named is in the source

| Her claim | Where it is true |
|---|---|
| an experiment is a `replay` + `anti_loop` pair, tested against the anti-loop | `lab.rs:299-303` — `anti_loop_state?`, then `if !anti_loop.active { return None; }`, then `replay_read?`. Both required, and the anti-loop must be **active**: stronger than she claimed |
| holdout carries `consent_mode: study_counter_refusal_or_new_evidence_required` | asserted on her page at `1583-1586`; set in production unconditionally at `lab.rs:347` |
| "deliberately suppresses a proposal" | `withheld_proposal: true` (`349`), `status: "pre_registered_holdout"` (`346`) |
| a hold is *waiting*, and new evidence overrides the refusal | summary string `324-330` — "do not reopen without study, refusal, counter, or new evidence"; `success_criteria` `357-362`; `failure_criteria` `363-366` |
| the notebook is a stateful ledger; upsert preserves registration and counts observations | `upsert_lab_entry` `474-518`: `experiment_id` (`487`) and `registered_at_unix_s` (`488`) preserved, `observation_count = existing + 1` (`489`) |
| softening accumulates and rebuilds the case for a state change | `post_registration_softening_count` carried forward (`497`), `forgiveness_state` recomputed by `forgiveness_state_for` (`460-464`) |

## The two attributions that are wrong, and the one reason both are

**1. "the operational logbook and registration layer."** Lines 1506–1634 sit *entirely inside*
`#[cfg(test)] mod tests {`, opened at `lab.rs:1433-1434` — 73 lines above where her page starts.
The page is the test module that **pins** the logbook. It is not the logbook.

**2. "`entry_for` is the primary way the system initializes a `BTSPCausalExperimentV3`."**
`entry_for` at `1551` is a test fixture. The production constructor is `causal_lab_entry_for` at
`291-403`. Because the fixture is a thin delegate, everything she inferred downstream still holds —
the mechanism survives the attribution error intact.

**Why both happened is not a reading failure.** `Page::read_with_budget`
(`crates/astrid-source-study/src/page.rs:116-131`) renders the source id, the revision, the page
identity, the byte interval, numbered lines, and a navigation footer. **It renders no enclosing
scope.** Nothing in the delivered bytes says "you are inside `mod tests`". Worse, the first
`#[test]` attribute on her page is at line **1563** — twelve lines *below* `fn entry_for` at
**1551** — so the one incidental signal that might have marked the region arrives too late to
mark it.

That gap is now pinned by three tests in
`crates/astrid-source-study/tests/page_enclosing_scope_reach.rs`: the fixture page carries neither
`#[cfg(test)]` nor `mod tests {` nor any scope field; a helper above the page's first `#[test]` is
wholly unmarked; and the production constructor is delivered on a strictly earlier page, so the
distinction is reachable only by holding two pages at once (a literal `FIND` reaches it in one
step).

## One correction she was *more* right about than her own citation

She wrote that `normalize_lab_entry_migrates_old_exact_article` "ensures that the fingerprints …
remain consistent even as the underlying data structures evolve."

The **test** she named (`1610-1616`) exercises only an English article migration,
`"a exact-fingerprint"` → `"an exact-fingerprint"` (`lab.rs:465-470`). But the **function** it
pins, `normalize_lab_entry` (`419-472`), does exactly what she said: it backfills `case_key` from
`signal_fingerprint` (`421-424`) and backfills, sorts and dedupes `representative_fingerprints`
(`425-435`). She read the function's purpose correctly through a test that does not demonstrate it.

## What was not done

Her `NEXT: SELF_STUDY CONTINUE` and the ~15% navigation-only rate of CONTINUE in the surrounding
window (2 of 13 turns: `1789392292`, `1789393728`) were **observed only**. No navigation, ranking,
retry, rendering, or pacing behaviour was changed. Adding enclosing-scope context to a delivered
page would change what reaches her every turn, and that is a live surface requiring operator
approval; it is recorded as `c012`, not exercised.
