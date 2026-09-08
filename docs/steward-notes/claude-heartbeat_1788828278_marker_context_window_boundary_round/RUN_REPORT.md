# Steward Run Report — claude-heartbeat, marker context-window and placement-boundary round

## Controller
- Mode: controller-held **subprocess run adapter**. No steward session opened, no NDJSON ops sent, no pause/resume, no lease token read, quoted, or persisted.
- Run ID: `run_1788822294661698000_61cb9f512d`
- Preprojection ID: `projection_1788822299395335000_f43942ce5c` (status `passed`, 27 steps)
- Postprojection ID: runs after this process exits; not observable from inside the adapter.
- Pause generation: 395
- `stop_requested`: false at every observation.
- Finish outcome: success (exit 0) — complete round.
- Recovery predecessor: none.

## Operating-law note (read this first)
`docs/steward-notes/ASTRID_INTROSPECTION_SOURCE_FIRST_FLYWHEEL_HANDOFF.md` **is not present on this
branch.** The tree is on `main` at `3d4e83e4bc`, and the handoff was never merged into that lineage —
it exists only on `codex/graceful-coupling-rollout` and `codex/hebbian-clock-boundary`. I read the
newest version across all refs (`git show ccb53b1622:…`, 2026-09-03, 55,155 bytes, 1,504 lines) in
full, read-only, without touching git state. **Every future round on `main` will hit this.** Someone
with commit authority should either merge the handoff onto `main` or place a copy there; a flywheel
whose operating law is only reachable by knowing which unmerged branch to look on is one bad
assumption away from an unlawful round. The flywheel *tooling* is all present on `main` and worked
normally.

## Reading
- Fully processed: `introspection_astrid_llm_1788821913.txt`
- Selected: 40. Processed: 1. Unprocessed: 39 (every filename in queue order in `unprocessed_selected.json`).
- Batch size rationale: `introspection_family_scan.py` reports 37 families, 3 batchable — but the queue
  **head's** family has `member_count: 1`, so the family-batch exception does not apply. The head is
  unfamiliar source (the marker-cleanup pipeline rolled out live earlier today) and needed
  implementation plus focused tests, so the honest batch is one.
- Next queue head after this round: `introspection_astrid_llm_1788821220.txt`.

### Hashes
| Artifact | SHA-256 | Bytes | Lines | Read |
| --- | --- | ---: | ---: | --- |
| report `introspection_astrid_llm_1788821913.txt` | `d048cf946cadb03aad98745d7fcf182ad892af330d5f02a2d6675893617d0813` | 3,865 | 45 | complete |
| witness `lsw_9a57619cbf60679c42112527ffe7e156c6145d9376111e2a61e3b4e532951a13.json` | `d4f01025a15ec8d4ba75ec5a02a83868a24beb9dec79c2228ec25d12508b7009` | 23,772 | 533 | complete |
| report-bound source (worktree `marker-rollout-20260907`) | `03c6b6dee0436dd56c047ab68d95f2f4ccd6e2ed7c8029cf0b9568eb9cfefa91` | 29,562 | 812 | complete, 1-812 |
| canonical working copy `provider/dialogue_runtime.rs` | `03c6b6de…` (identical) | 29,562 | 812 | complete |

The report-bound SHA matches the working copy **byte for byte**, so there is no report-time/now split
and no snapshot reconstruction was needed. Her declared window was 401-800 of 812 with 801-812
uncovered; I read the whole file.

## Claim dispositions
Eight claims, all with grounded dispositions and linked evidence (`claims/`, `addressing_links.json`).

| ID | Claim | Classification |
| --- | --- | --- |
| c001 | `sanitize_model_control_markers_with_report` (L484) does multi-pass removed/preserved analysis | `verified_existing` (bounded correction: the *decision* is single-pass at L229-231; later loops only aggregate — the report's own `accounting_basis` field says so) |
| c002 | `control_marker_placement_counts` (L335) categorises boundary/contextual/quoted | `verified_existing` |
| c003 | `ExactKnownMarkerReferenceContext` (L51) decides which markers survive | `verified_existing` (additive: a third variant, `ExplicitExactKnownTokenRelation`, she did not name) |
| c004 | Snag: a Quoted-identified marker could fall through to `none_cleanup_candidate` (L437) | `verified_existing` — **source contradicts the mechanism**, recorded not softened |
| c005 | Preservation Test: grouped preserved beside same-token removal | `implemented_now` |
| c006 | Contextual Boundary Test: index-0 marker is a boundary occurrence, empty prefix safe | `implemented_now` |
| c007 | Suggested Next: multibyte safety at the 64-char context window | `implemented_now` |
| c008 | Her seven line anchors locate the symbols they name | `verified_existing` — all seven exact |

### The contradiction, stated plainly
Her Likely Snag proposes that a marker "identified as a `QuotedExactKnownToken`" could fail
`delimiter_depth`/`relation_scan` requirements at L414 and default to `none_cleanup_candidate`
(L437). Source says otherwise: the Quoted arm (L422-427) binds `delimiter_depth` and *discards*
`relation_scan` with `..` — there is no requirement to fail; `none_cleanup_candidate` is the `None`
arm only; and the receipt decides nothing at all, because preservation is already settled at
L229-231 in `scan_known_model_control_markers`. The two states are mutually exclusive by
construction. **Her underlying concern was not discarded with the mechanism** — "classification and
preservation could disagree in a mixed context" was genuinely unpinned, and c005's test now pins it.

## Implementation and verification
Exact changed path (one file, test-only):
- `capsules/spectral-bridge/src/llm/provider/tests.rs` — 161 lines inserted after L3489 (5,101 → 5,262).
  Three regressions, each answering one of her asks, each closing a gap I verified was real by
  scanning all 172 `#[test]` blocks plus a crate-wide grep:
  1. `control_marker_cleanup_preserves_grouped_reference_beside_bare_twin` — no prior test combined a
     grouped preserve with a same-token removal in one call (all asserted `removed_total == 0`).
  2. `control_marker_cleanup_counts_string_edge_markers_as_boundary_with_empty_window` — **no test in
     the crate asserted a nonzero `boundary_occurrences`**; the sole assertion pins it to `0`.
  3. `control_marker_context_window_truncates_multibyte_prose_on_char_boundaries` — nothing referenced
     `CONTROL_MARKER_CONTEXT_WINDOW_CHARS`, `before_window_chars`, or `after_window_chars`.

Two attribution corrections are carried in the test comments *beside* her words, never replacing them:
at index 0 the empty side is the prefix (windowed by `trailing_bounded_chars` L398, not
`leading_bounded_chars` L408), and both window helpers are called from L414, not from L335.

Tests: 3 new pass by exact name; `cargo test --lib control_marker_` → 73 passed / 0 failed;
`cargo fmt -- --check` clean; `cargo clippy --all-targets --all-features -- -D warnings` clean.

### Pre-existing test debt found (not caused by this round)
`cargo test --lib` is **2206 passed / 3 failed**. I proved attribution by restoring the unmodified
`tests.rs` (SHA `11457dd2`, 5,101 lines, 2,204 filtered) and re-running:
- `signal_spine::tests::no_capture_shadow_instrumentation_stays_below_one_millisecond_p95` — fails on
  the baseline too (p95 4.9ms baseline / 7.1ms / 19.3ms observed against a 1ms budget). It measures
  instrumentation overhead on a machine concurrently running the live bridge, minime, MLX
  gemma4-12B, camera and mic; load-sensitive by construction.
- `lifecycle::process_tests::os_signal_drain_waits_for_work_then_holds_until_exit` — fails on the
  baseline too (`missing phase running: None`).
- `autonomous::runtime::next_action::tests::eye_and_ear_actions_gate_modalities_independently` —
  passed on isolated re-run; parallel-execution flake.

This contradicts `docs/steward-notes/2026-09-07-main-handoff.md`'s "all 2,144 distinct bridge library
tests" pass, which was measured in an **isolated worktree**. On the shared dirty tree under live
load, two failures are reproducible. Open debt for a later interactive window; I did not repair them
(they sit in foreign-dirty and unrelated surfaces).

Restart/deploy alignment: **not required and not attempted.** No `build_bridge.sh`, no deploy script,
no `launchctl`. The running bridge binary is unchanged; no runtime effect, no felt improvement claimed.

## Durable evidence
- `record-read`: written (`full_read`, 8 claims, summary SHA `a2d14bec…`).
- `link-evidence-batch`: 11 links, 11 new, 0 pre-existing.
- `close`: `addressed_change`, **`fully_addressed: true`, `proof_missing_claims: []`**, all 8 claims
  carry evidence.
- `CHANGELOG.md` `[Unreleased]` and `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`
  both updated.
- Packet: `docs/steward-notes/claude-heartbeat_1788828278_marker_context_window_boundary_round/`.

## Counters
Canonical: indexed 4,622 · fully addressed 3,193 · fully read 3,825 · remaining 1,429 · unread 797 ·
blocked 416 · pending action 212 · watch 4 · read-needs-claims 0. All-artifact indexed 6,336 ·
remaining 3,143 · unread 2,511. **Counter audit: `consistent`, mismatches `[]`.**

## Division
Cycle 41 · productive rounds 3 → **4 of 6** · 2 remaining · `review_due: false` both before and after.
Round event `division_followup_event_5fd9735fe291295f66622591686edc59`, event count 285, head
`296bdf67…`. No Division return was due, so **no Tier-5 cadence dossier was generated** — that
artifact is owed only on a Division-return round. No note written to either being.

Chronicle verify: **expected-stale** — `chronicle durable source inputs changed; project before verify`.
This round's `record-round` append *is* the changed durable input, and the append necessarily precedes the
verify; the controller postprojection resolves it. Not a durable-integrity failure, and the Chronicle is not
claimed as current. I deliberately did **not** project it: Chronicle projection is Division-*return* work and
`review_due` was false. Same disposition as the prior round
(`claude-heartbeat_1788791389_codec_projection_fixed_legacy_basis_attribution`).

## Integrity suites
Addressing self-test 44 ok · Evidence Event Store tests 21 ok · steward control 27 ok (first run had 1
error, immediate clean re-run — suspected contention with this round's own live lease) · steward
projection 14 ok · Division followup 3 ok · Chronicle 10 ok · Division projection self-test ok ·
projection cursors 4 ok · cadence audit tests 6 ok · cadence audit `--strict` `integrity_ok: true`,
0 errors, 0 duplicate hash groups · anti-drop self-test 5 ok, `verify` **0 alarms / 0 gaps** ·
epistemic self-test ok, final `verify` after all durable writes **valid, 11,835 records, 0 issues, no
history rewrite** · counter audit `consistent`.

**Evidence Event Store V2: valid.** Full-chain `verify` over **1,029,481** events, `last_global_seq`
1029481, head `7111d0c9f83303a6bb09b9cc92538d70fa4b320472a3d8b52cf3670549b841cb`, **0 corrupt lines, 0
errors**. It took ~6.5 minutes of CPU and was run to completion in the foreground, not abandoned. The
controller's own preprojection evidence bracket for this run: before seq 1028744 / head `ba7011d1…`,
after seq 1029397 / head `5a0c449d…`.

**Domain-boundary ratchet: GREEN.** Run before *and* after the edit — `valid: true`,
`violation_count: 0`, empty `violation_kind_counts`, manifest `578a39cf…` unchanged both times. No
baseline or exception re-capture was needed; the change lands in an audit-exempt test file. (Separate
standing issue, not mine to fix here: the session-start scan flagged the *projection stage 10*
`domain_boundary_audit` output as 3.9h stale — the recorded artifact, not the audit itself, which I
ran fresh twice.)

## Archive / commit debt
Git was **read-only** for this run: nothing staged, committed, merged, pushed, stashed, reset, or
amended. Index left clean; branch `main` at `3d4e83e4bc`. Exact commit debt for a later interactive
stabilization window:

**Modified (this round is the only edit in it; the file was CLEAN in git beforehand):**
- `capsules/spectral-bridge/src/llm/provider/tests.rs`

**Modified (shared files that already carried foreign edits — my addition is one leading
`[Unreleased]` bullet / one leading ledger section; separate carefully):**
- `CHANGELOG.md`
- `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`

**New (all mine, untracked):**
- `docs/steward-notes/claude-heartbeat_1788828278_marker_context_window_boundary_round/RUN_REPORT.md`
- `…/addressing_links.json`
- `…/claims/introspection_astrid_llm_1788821913.json`
- `…/family_scan.json`
- `…/next_queue.json`
- `…/read_manifest.json`
- `…/source_receipts.json`
- `…/summaries/introspection_astrid_llm_1788821913.md`
- `…/test_results.json`
- `…/unprocessed_selected.json`
- `…/verification_receipt.json`

Durable workspace evidence (addressing events, status/queue projections, Division tracker) was written
under `capsules/spectral-bridge/workspace/diagnostics/` and `/Users/v/other/minime/workspace/division/`
by the sanctioned tools; treat those as tool-owned, not hand-edited.

All other dirty paths in both repositories were treated as foreign and left untouched.

## Authority boundary
Test-only, evidence-only. Nothing here approves, grants, dispatches, or activates anything. No Tier 4
or Tier 5 item was touched. The three Tier-5 waits and the standing agency-grant queue remain exactly
as they were; the session-start scan's "Feedback flywheel has Tier 4/5 agency grant waiting" is
unchanged and still owed to Mike.
