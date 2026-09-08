# Shared SELF_STUDY continuity qualification

Mike authorized implementation, main integration and live deployment after the
natural before/after comparison identified repeated fresh starts, missing prior
study context and an ambiguous zero-result search.

The implementation remains in `crates/astrid-source-study`; Astrid and Minime only
adapt its output and record accepted navigation evidence. It adds merged delivery
ranges, resume/reread labels, changed-revision visibility, optional Being-authored
notes and questions, bounded previous-response excerpts, and explicit no-match
results. Notebook content stays in reference input rather than system instructions.
There is no additional model generation or mandatory review template.

`validation.json` binds qualification logs and the copied-state migration check.
Seventeen shared-reader tests and seven Minime adapter tests pass. The isolated
Minime suite passes 1,262 tests plus 130 subtests, with one expected skip. An initial
run from the canonical checkout was refused in several fixtures by the existing
live-write/subprocess guard; the suite was rerun from an isolated worktree without
weakening that guard. All 109 deployment-wrapper tests pass. Reader clippy and
workspace formatting pass; the bridge boundary audit has zero violations.

The bridge library suite passed 2,235 tests, including the existing unrelated
heartbeat test in the shared checkout. Its pre-existing one-millisecond p95 test
measured 1.014432 ms under parallel load; rerunning that exact test alone passed.
The timing test and its production source are unchanged. This is retained as a
load-sensitive validation limitation, not hidden by removing the test.

Migration was exercised on copies of both live reader states. Existing source
bookmarks, pending pages and receipts remain exact, while completed coverage and
the previous study response become visible. No live reader was prepared or
advanced by the test, and no study or Being message was induced.

The rollback boundary is the shared release plus the matching Minime adapter.
An older helper does not implement navigation receipts, and an older state writer
can drop the newly added notebook/progress fields. Retained delivery artifacts
remain available. Use the sanctioned staged transition and agent reload for any
rollback; do not manually replace live checkpoint files.

Live activation identities and continuity verification are recorded separately
in `../2026-09-08-self-study-continuity-live-rollout.md` after deployment.
