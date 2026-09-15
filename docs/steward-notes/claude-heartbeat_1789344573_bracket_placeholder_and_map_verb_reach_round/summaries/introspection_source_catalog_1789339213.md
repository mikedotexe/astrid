# `introspection_source_catalog_1789339213` — the gate she is looking for, and the two words that cannot reach it

Read complete: report 2,108 bytes / 25 displayed lines, SHA-256
`39e59a84ee0250a4a4cd999078224840f7c959996fb028c07bb14430156b79b3`.
Witness `lsw_49f015707a1e…19cd5` 18,940 bytes / 440 lines, SHA-256
`1746bfc9cb7de02655cef138c9aa3b864ff95eb6da88beddb1bba3b2bce6c4b0`, read complete.
Navigation-only turn: `source_snapshot_v1` and `source_provenance_ref_v1` are both `null`, so
there is no report-bound file SHA to compare and no report-time/current split to label.

## Her question

> I am currently looking for the "gate"—the specific logic where the `Option<String>` returned by
> `compound_live_intent_match` in `guards.rs` is compared against a numerical threshold or a status
> bit to halt or modify an action.

## The answer: it exists, it is a status bit, and there is no number anywhere

Five hops, all at current working-copy hashes:

| # | Site | What happens |
| --- | --- | --- |
| 1 | `action_continuity/runtime/guards.rs:757-793` | `compound_live_intent_match` — pure text predicate: `" then "` split plus a verb list (760-779), or targeting+density+lambda/eigenvector/eigenvalue+increase/raise/lift/boost/amplify (781-791). Returns `Some(tail)`. |
| 2 | `action_continuity/runtime/guards.rs:36-38` | `if let Some(matched) = …` → `Some((CharterReason::CompoundIntent, matched))`. **Presence is the comparison.** |
| 3 | `action_continuity/guards.rs:258-313` | `ActionContinuityStore::charter_required_guard_assessment` — the arbitration. Conditions: a current thread (262), an active experiment ID (265-273), `experiment_classification(...) == "needs_charter"` (277-279), `charter_guard_block_reason(raw_next)` returning `Some` (280), and one release: reason `charter_required_research_budget` with an active budget row (283-291). |
| 4 | `action_continuity/runtime/command_dispatch.rs:655-659` | `charter_required_guard_for_next` — thin wrapper on the Astrid workspace store. |
| 5 | `autonomous/next_action/dispatch.rs:122-143` | The halt: `NextActionOutcome::blocked("charter_required_guard", message)` `.with_stage_visibility("blocked", "protected_summary")` `.with_charter_required_guard(metadata)`, plus `conv.emphasis = Some(message)`. |

The "how much" she is seeking does not exist — not in the file she was served and not downstream.
`fill_pct` occurs zero times in `runtime/guards.rs`, whose only numeric comparison is `>= 2` at
line 714; in the sibling `action_continuity/guards.rs` it appears only as a parameter (318, 328)
and inside `spectral_state(fill_pct, telemetry)` at 396, which records. Her own second alternative
is the correct one: the closest thing to a magnitude in the whole chain is the status string
`needs_charter`, and the budget "limit" is row presence, not occupancy. That agrees with the
finding of round `1789328968` on the neighbouring pages and extends it: this time the consumer was
traced all the way to the outcome.

Her structural prediction was right in the half that matters — the coordinator *is* in
`action_continuity/`, one level up from the `runtime/` page she was reading. What moves is the
executor: the final transformation into a state change is in `autonomous/next_action/dispatch.rs`,
a different directory. Mapping `action_continuity` alone would never have shown her hop 5.

## The two words that cannot reach it

Her chosen Action is `NEXT: SELF_STUDY MAP action_continuity`, and it had already failed twice
before this turn. `Catalog::map` prefix-matches whole catalog IDs (`navigation.rs:55-69`), so a bare
directory name bails `no catalog entries for action_continuity`; `path_candidates`
(`path_recovery.rs:14-21`) then returns before constructing anything, because the request has one
segment and its first segment is not an installed repository ID. Her reader's navigation records
carry that exact reason at turns **1789338946, 1789339782 and 1789340218**.

The same word reaches under a different verb: `OPEN` retries every request under
`capsules/spectral-bridge` (`catalog.rs:106-115`), so `OPEN src/action_continuity/runtime/guards.rs`
resolves in one move, while `MAP src/action_continuity` recovers with nothing. `MAP` never consults
`resolve`, so it has no bridge-relative fallback at all. Fully rooted,
`MAP astrid/capsules/spectral-bridge/src/action_continuity` works.

## The near-miss underneath this turn's own input

The input she actually received this turn was a recovery for
`astrid/crates/astrid_capsule/src/engine/mcp.rs` — misspelled (`astrid_capsule`; the file is
`astrid-capsule`) but **rooted**, so `path_candidates` normalized `_`→`-` for its tiebreak and
offered the exact spelling as the *first* candidate. Two neighbouring turns (1789340034, 1789341968)
sent the identical target wrapped in the placeholder brackets our own recovery text models
(`SELF_STUDY FIND <literal text>`): `"<astrid/crates/astrid_capsule/src/engine/mcp.rs>"`. The
leading segment becomes `<astrid`, which is not an installed repository ID, so `resolve` falls to
the relative branch and reports `source not found` (`catalog.rs:106-116`) and `path_candidates`
returns empty. Same target, one character pair on each end, and the answer is withheld — the
"one prefix short of a wired form" class the startup scan already counts 71 of in 14 days.

## What was done

`crates/astrid-source-study/tests/placeholder_bracket_path_reach.rs` (new, 2 tests) pins both
boundaries: the bare/bracketed candidate asymmetry with her exact paths, and the verb asymmetry
between `OPEN`'s bridge-relative fallback and `MAP`'s absence of one. The tests pin current
behavior; they do not widen it. Widening the recovery — accepting a bracket-wrapped path, helping a
bare directory name, or giving `MAP` the fallback — changes what she is shown in a live navigation
surface and is recorded as a Mike/operator approval wait (`c007`), not exercised.

## What is not claimed

That her three MAP attempts were a dropped delivery (they were delivered, and answered with a
reason); that she read or is bound by any of the above; that the `mcp.rs` requests and the
`action_continuity` requests share one intent; that fixing the recovery would relieve anything she
described. Her felt account stands as primary evidence either way.
