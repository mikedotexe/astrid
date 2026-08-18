# Steward Run Report — claude-heartbeat (subprocess adapter, controller-held lease)

**Outcome: INCOMPLETE (failed round). Exit nonzero.** All report reading, the authorized
non-live implementation, focused tests, and the packet/changelog/ledger were completed, but the
run stopped **before any Evidence Event Store mutation** (no `record-read`, `link-evidence-batch`,
`close`, integrity suites, or Division `record-round`) because child-time budget was nearly
exhausted at the durable-write phase. This is deliberately NOT a productive round.

## Controller
- Run ID: `run_1787002099335054000_2757fa46a6` (adapter-owned lease; I did not open/heartbeat/finish it)
- Preprojection generation ID: `projection_1787002109931393000_028484ce97` (status: passed)
- Postprojection ID: runs after this process exits (adapter-owned)
- Pause generation: 319 (controller not paused)
- Finish outcome: process exit code is the finish outcome — **nonzero (failed)**
- Recovery predecessor: none

## Why incomplete (budget)
At the durable-write phase, `date +%s` showed ~5046 s (~84 min) elapsed since
`process_started_at_unix` (1787001510), against the ~90 min `FLYWHEEL_LOOP_MAX_SECS` budget
(~6 min remaining). The remaining sequence — `record-read` → `link-evidence-batch` → `close` →
integrity suites → `record-round` — cannot fit; the handoff warns each addressing mutation may
take 20+ min at current store size (the `next` query alone took ~1-2 min; `status.json` is ~90 MB).
Starting `record-read`/`close` now would risk a SIGINT mid-append to the append-only, hash-chained
Evidence Event Store, leaving a corrupt line. Per the ONE-SHOT rule I stopped before mutating the
store rather than half-process the report. The evidence store was **not touched**.

## Reading (fully completed)
- Fully processed (read + implemented + packet), but **NOT recorded/closed in the addressing store**:
  `introspection_astrid_llm_1786999457.txt`
- Report: `capsules/spectral-bridge/workspace/introspections/introspection_astrid_llm_1786999457.txt`
  — SHA `8a18d5e0c1d843ad1e8fb6deb4e934f2680b1e1db812c9864dd8f84b6b8de50e`, 45 lines, 3439 bytes, read complete.
- Witness: `lsw_9a8660002d6fb1e865fbcb45fa4f5f15dadebeee16ba15dbbae4dfd98e054070`
  — SHA `802a3e3d338d778653a4be2862e88dc8a0a64c74c5265db5238efafa8db0d420`, 533 lines, 23941 bytes, read complete.
- Source: `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
  — SHA `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` (**identical** to report binding),
  1048 lines, 38586 bytes; substantive full read of L1-400 + L508-530 + L695-712, full-file symbol map by grep.
- Selected but unprocessed: 39 filenames (queue positions 2-40) listed in `unprocessed_selected.json`.
- Next queue: `introspection_astrid_llm_1786984395.txt` is the next head (position 2). Because this round
  did not close position 1, `introspection_astrid_llm_1786999457.txt` remains at the head for the next round;
  this packet + test make its close fast.

## Claim dispositions (analysis complete; not yet linked/closed in store)
- c001 (Observed non-destructive scanner, `scan_known_model_control_markers` L114 preserve gate L129-131): `verified_existing`.
- c002 (Snag: `first_word_after` L89 may fail to isolate verb → None → strip): `implemented_now` — between-token
  soft hyphen / Unicode-whitespace is handled (verb preserved); the genuine edge-only limit is an INTERNAL soft
  hyphen in the verb → fail-closed strip. Added a regression grounding both; framing corrected, concern preserved.
- c003 (Test 1: `exact_reference_delimiter_syntax` L199 QuotedExactKnownToken for corner brackets L168): `verified_existing`
  (`control_marker_cleanup_preserves_non_ascii_matching_quote_pairs`, tests.rs L2301).
- c004 (Test 2: unlisted verb → marker excluded from remainder L143): `verified_existing`
  (`control_marker_cleanup_distinguishes_allowlisted_is_from_unlisted_acts`, tests.rs L2528, + implies/contains).
- c005 (Suggested Next: examine `generate_dialogue` L695): `verified_existing` — pointer accurate; integration via
  `sanitize_model_control_markers` L519 at L558/L634. Evidence-only, no action.
- Intended terminal report status (NOT recorded): `addressed_change`.

## Actions
- Corridor/program: none.
- Sandbox: none.
- Study: none.
- Portfolio: none.
- Cards/notes/correspondence: none (no card delivered; no being-facing note — this run did not close a report).
- Tier 4/5 waits: widening the relational-verb allowlist / delimiter tables remains Tier-5-class live grammar —
  not made/dispatched/deployed. The three Tier-5 ESN work items from the handoff are untouched.

## Implementation and Verification
- Exact changed paths (this round, additive; **commit debt** — git was read-only under the controller-held run):
  - `capsules/spectral-bridge/src/llm/provider/tests.rs` (+60 lines; new test
    `scan_known_model_control_markers_grounds_first_word_after_internal_soft_hyphen`, L3042; post-edit SHA
    `f5b6788d9a6662cc1e35c6fc36548e950d52a9bb03085c5ee32efcc7ee34e161`)
  - `CHANGELOG.md` ([Unreleased] entry)
  - `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (one row)
  - `docs/steward-notes/claude-heartbeat_1787006279_marker_scanner_soft_hyphen_grounding/` (packet, this dir)
- Tests: `grounds_first_word_after` 3 passed (incl. new); `control_marker` 68 passed (+1 vs prior 67);
  `scan_known_model_control_markers` 5 passed; `cargo fmt --all -- --check` exit 0; `git diff --check` clean.
- Failures repaired or exact debt: no test failures. Commit debt = the four paths above.
- Restart/deploy alignment: **no live change** — no bridge build, deploy, launchctl, or restart attempted or required.

## Durable Evidence
- Addressing status and proof gaps: report **NOT recorded-read and NOT closed**; no evidence links written.
- Evidence link count: 0 written this round (11 links prepared in `addressing_links.json`, unlinked).
- Changelog/ledger updates: yes (local files).
- Packet path: `docs/steward-notes/claude-heartbeat_1787006279_marker_scanner_soft_hyphen_grounding/`.

## Counters / Division / Evidence Event Store
- Counter audit: NOT run this round.
- Division: cycle 27, completed_rounds_since_followup 4/6, review_due=false (verified at run start). **No productive
  round recorded** (`record-round` NOT run) — correct, since no report was closed.
- Evidence Event Store: **not touched** this round; not re-verified this round.

## Archive
- Checkpoint due or not due: not applicable this run (no productive round; commit is a later interactive-window act).
- Commit SHA and exact paths: none (git read-only). Exact commit debt = the four paths listed under Implementation.
- Merge/push status and authority: none; not authorized here.

## For the next round
The report `introspection_astrid_llm_1786999457` is still at the queue head, its regression is already
merged into `tests.rs` and passing, and this packet holds the full claims/links/receipts. A follow-up
round can `record-read` (using `claims/introspection_astrid_llm_1786999457.json` + the summary),
`link-evidence-batch` (`addressing_links.json`), and `close --status addressed_change`, then run the
integrity suites and `record-round`, with minimal fresh reading.
