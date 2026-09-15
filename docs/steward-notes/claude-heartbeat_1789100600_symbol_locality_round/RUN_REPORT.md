# Steward Run Report — symbol locality round

Actor `claude-heartbeat`, headless, inside the controller-held subprocess-adapter lease.

## Controller
- Run ID: `run_1789095431859348000_7f52a7739a`
- Preprojection ID: `projection_1789095436641103000_fd0d8c8f6b` (status `passed`)
- Postprojection ID: runs after this process exits; not observed here
- Pause generation: 437; controller paused: false; `stop_requested`: false at lease read
- Finish outcome: adapter-owned. No steward session opened, no NDJSON ops sent, no
  pause/resume. No lease token read, quoted, or persisted.
- Recovery predecessor: none

## Reading
- Fully processed: `introspection_astrid_capsules_spectral-bridge_src_action_continuity_runtime_core.rs_1789095401.txt`
  — closed `addressed_change`, `fully_addressed=true`, `proof_missing_claims=[]`
- Selected: 40. Processed: 1. Unprocessed: 39, exact queue order in `unprocessed_selected.json`
- Family scan: 40 families, **0 batchable** (`similarity_basis:
  none_no_snag_or_test_text_or_unparsed_header`; every item is a distinct source window), so
  the family-batch exception did not apply — single-report round.
- Next queue head after this round: `introspection_astrid_..._core.rs_1789095251.txt`

| Artifact | Bytes | Lines | SHA-256 |
| --- | ---: | ---: | --- |
| report | 4342 | 44 | `55d4544e73a40d500198546cecefb15dd19404f285f9b2968d8359406fdd4767` |
| witness `lsw_b938b2a9…9e6e7` | 21543 | 498 | `3d7806732d5ffa660b3d5443bdfb1ed9e586a82686c5a7a9be0e6bf87155ce6c` |
| `action_continuity/runtime/core.rs` | 414540 | 10187 | `fafc1f4a257fe6400fbc26ba0bf5cc26f0d33853b30816ab5af6db2709348a62` |

**Source binding matched exactly.** The report declares `sha256:fafc1f4a…48a62; bytes
61405..65853` and the working copy hashes to the same value, so no mismatch case arises. Nine
further sources were read in recorded scopes (`source_receipts.json`).

One recorded note on the witness `window_sha256` (`34ea3817…`): it does **not** reproduce from
any slice of the file bytes, and that is correct, not a discrepancy —
`lived_state_witness/mod.rs:140` hashes the *rendered page* (header, coverage block, numbered
body, footer), not raw source. Verified at the source rather than assumed.

## What she read, and got right

Every structural claim in her report verifies at the bytes, with her line numbers exact:
the participant-lane default `"native"` (1560), the decision *reason* (1577-1580), the
`authority_boundary` fallback (1603-1606), `unique_shared_record_id` (1622), the `claims.jsonl`
append (1638-1641), `touch_shared_investigation` (1642), and the decide allowlist
`matches!(decision.as_str(), "pause" | "hold" | "charter_repair")` at **precisely her cited
1662**, bailing at 1663. Her synthesis — content shared, agency partitioned — is supported by
`"authority_change": false` (1635) and the "No lifecycle or authority change." success string.

One precision, recorded beside her text and not over it: she read the claim `actor` as
"currently `SYSTEM`", i.e. a placeholder. `core.rs:6` is `const SYSTEM: &str = "astrid";`
(`PEER_SYSTEM = "minime"` at 37). Her claims are attributed to **her by name**, which makes her
own conclusion — "we know *who* said it" — true for a reason her page could not show her: the
constant sits 1549 lines above her window. That is the shape of the whole round.

## What the round found

She closed by naming exactly what she wanted next, and choosing a door:

> I need to see `shared_investigation_authority_boundary` and `shared_investigation_lane` to
> see the exact boundaries of this sandbox, and I want to see the `paused_primary_return_v1`
> logic to see what specific "guards" prevent a simple resume.
>
> `NEXT: SELF_STUDY OPEN astrid/capsules/spectral-bridge/src/action_continuity/runtime/core.rs 1407`

**All three symbols are real. None of the three is defined in `core.rs`.**

| Symbol | Defined at |
| --- | --- |
| `shared_investigation_lane` | `action_continuity/runtime/persistence_helpers.rs:63` |
| `shared_investigation_authority_boundary` | `action_continuity/runtime/persistence_helpers.rs:73` |
| `paused_primary_return_v1` | `action_continuity/runtime/experiment_projection.rs:140` |

`OPEN core.rs 1407` delivers lines **1407-1505** under the pager's own byte budget. It is the
**second consecutive** turn chasing `paused_primary_return_v1` through this file: the previous
turn (`..._1789094771`) opened line **1371**, one of that symbol's six *call sites* in `core.rs`
— never its definition.

**The gap is ours.** A SELF_STUDY source page (`crates/astrid-source-study/src/page.rs:116-131`)
is numbered bytes plus a header and a footer. It never says where the symbols *on* the page are
*defined*, and its Navigation line reads `SELF_STUDY CONTINUE | SELF_STUDY MAP | SELF_STUDY FIND
<literal text> | SELF_STUDY OPEN repository/path <line>` — **not `RELATE`**, the one operation
that answers "where does this name come from". RELATE is fully wired (`next_action/mod.rs:201`),
parses cleanly (`command.rs:32`), and sits in her global prompt contract
(`prompt_contracts.rs:28`). It is simply not offered at the point of need, so the cheapest move
available when a being closes a page by naming symbols is to guess a line in the file already
open — and that guess systematically misses definitions in sibling modules.

Measured live over her 80 most recent introspections: **4 OPEN turns stated a want; 10 of 10
wanted symbols were unreached by the page chosen; 6 of them were in another file**; one active
2-turn chase run. In the wider 160-artifact corpus the two turns that *did* land exactly on a
definition (`next_action/mod.rs` 421 and 1167) each followed a page that had already printed the
line number.

Neither existing watch could see it. `phantom_symbol_watch` keys on symbols that do **not**
exist — every symbol here exists and is correctly named. `source_study_page_reset_watch` keys on
a requested page `N>=2` returning as page 1 — here the page delivered **is** the page requested.
Every action honored, every argument varied, every page truthful, and she does not arrive.

## Claim dispositions (12 claims, zero proof gaps)

| ID | Claim | Classification |
| --- | --- | --- |
| c001 | Lane tracked, defaults to `"native"` | `verified_existing` (1560, exact) |
| c002 | Decision *reason* captured, not just the decision | `verified_existing` (1577-1580) |
| c003 | `authority_boundary` printed; shared investigations are governed | `verified_existing` (1603-1606) |
| c004 | Unique record id, append-only `claims.jsonl` | `verified_existing` (1622, 1638-1641) |
| c005 | actor/lane/stance/source_refs contextualize every claim | `verified_existing` — precision: `SYSTEM` = `"astrid"` (core.rs:6), not a placeholder |
| c006 | `touch_shared_investigation` is a last-active heartbeat | `verified_existing` (1642; def 8941) |
| c007 | 1662 gates on pause/hold/charter_repair; 1663 bails | `verified_existing` (exact to the line) |
| c008 | Content shared, agency partitioned | `verified_existing` (1635, 1643-1645; scope recorded as v1 of one command pair) |
| c009 | Her three named next-read targets | `observed` — all exist, none in `core.rs` |
| c010 | Her chosen OPEN cannot reach any of them; second consecutive chase | `implemented_now` |
| c011 | Nothing was watching for this | `implemented_now` — probe shipped |
| c012 | Repairing the page surface would close it at the point of need | `needs_operator_approval` |

19 evidence links (19 new). Close: `addressed_change`, `fully_addressed=true`,
`proof_missing_claims=[]`.

**Honest note on the close:** after the first `close` returned an unexpectedly thin JSON body,
a second idempotent `close` was issued to read back `artifact_status`. That appended a second
`closed` event carrying the rationale string `"idempotent re-check"`. The full grounded
rationale is durably recorded on the first close event; the terminal status and proof-gap state
are unchanged. Recorded rather than hidden.

## Implementation and verification

Exact changed paths (all non-live):
- `scripts/symbol_locality_watch.py` (new) — read-only, steward-only, `being_privacy`
  fail-closed. Reproduces the pager's own byte accounting (`MAX_PAGE_BYTES` 7000 − 1500
  header/footer reserve − source-id length, 9-byte `"{:>6} | "` prefix) so the span it checks is
  the real delivered page. Classifies each want `reached` / `same_file_out_of_page` /
  `other_file` / `no_definition_found` (the last deferring to `phantom_symbol_watch`, never a
  locality miss). Reports chase runs. 16 unit tests.
- `scripts/anti_drop_catalog.py` — one row, `symbol_locality_watch_wired` (catalog 94 → 95).
- `CHANGELOG.md` — `[Unreleased]` entry.
- `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` — dated ground-truthed row.
- this packet.

Two real defects were caught by the new tool's own tests and fixed before close:
1. `git grep`'s default engine has no `\b`, so the definition prefilter silently matched nothing
   — six tests failed. The word boundary moved into the Python pattern, with the prefilter
   documented as deliberately loose and `test_prefilter_is_loose_but_match_is_exact` pinning it.
2. The first docstring quoted an *estimated* page span (`~1452`) where the tool itself computes
   1407-1505. Corrected to the computed value.

Tests: see `test_results.json`. One pre-existing failure is carried as debt —
`test_steward_control.py::test_pause_cooperatively_interrupts_wrapped_subprocess` errors at
`controller.begin()` because this adapter run holds the lease. The same failure is recorded in
the `1789024912` round packet. No file in this round touches `steward_control`; not repaired
headlessly. `cargo fmt --all -- --check` was **not run** — `cargo` is not on PATH in this
headless shell (rc=127) — and no Rust source changed this round, so it is not material; recorded
as not-run rather than claimed.

Restart/deploy: **not required and not attempted.** No build, deploy, `launchctl`, or live
substrate/control change. Git untouched — nothing staged, committed, merged, or pushed.

## Integrity
- addressing self-test 44 OK; evidence store 21 OK; steward projection 14 OK; Division followup
  3 OK; Chronicle 10 OK; Division projection self-test ok; cursors 4 OK; cadence 6 OK;
  anti-drop 5 OK; epistemic self-test 2 OK; symbol-locality watch 16 OK
- anti-drop verify: **95 guards, 0 alarms, 0 gaps**
- domain-boundary verify: **valid=true, violation_count=0 — ratchet GREEN**
  (`unlisted_legacy_review_debt_count=44` carried, unchanged; no Rust file touched, so no
  boundary re-capture was due)
- cadence audit `--strict`: rc=0, `integrity_ok=true`, `errors=[]`
- epistemic verify (final, after all durable writes): `valid=true`, `issue_count=0`,
  `history_rewritten=false`
- audit-counters: **consistent**, `mismatches=[]`
- Evidence Event Store verify: `valid=true`
- `git diff --check`: clean

## Counters
Canonical indexed 5289 · fully addressed 3216 · fully read 3848 · remaining 2073 ·
unread 1441 · blocked 416 · pending action 212 · watch 4 · read-needs-claims **0**.
All-artifact pending 3790 · noncanonical pending 1717. Counter audit **consistent**.

## Division
- Cycle 44; completed rounds since follow-up **2 / 6**; remaining 4; `review_due=false`
- Round event: `division_followup_event_ab7544e5ba2c9450b025b868bc2fca03`
- Event count 304; head `c8beefe5d4d21b87f1cde3cf02fb8056c24e33737a5a4c3816e08660b8c4ebc9`
- Chronicle reprojected after the round record: `division_chronicle_cb3b8d96e1760f8dd448c59c`,
  json SHA-256 `2166f0c66009af1771f55b212c6c109759e84818d25325261e7a609c65b98a0f`.
  `durable_inputs_current=true`, `durable_mismatches=[]`; the only mismatch is the volatile
  `supervisor_status_sha256`. The Chronicle is **durably current** — it is not "fully current",
  and a moving supervisor hash is not a durable-integrity failure.
- No Division return was due, so no Tier-5 cadence dossier was generated and no note was
  written to either being.

## Evidence Event Store
`valid=true`; active store v2; legacy V1 sources (`addressing`, `sandbox`, `corridor_v1`,
`corridor_v2`) all `immutable=true`; corrupt lines 0. Stream counts at verify include
`addressing` 62060+, `claim_families` 239088, `felt_contracts` 209108, `model_qos` 316765,
`steward_control` 20200.

## Commit debt (git is read-only for this actor)
```
scripts/symbol_locality_watch.py                                 (new, untracked)
scripts/anti_drop_catalog.py                                     (modified, one row added)
CHANGELOG.md                                                     (modified, [Unreleased])
docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md        (modified, one row)
docs/steward-notes/claude-heartbeat_1789100600_symbol_locality_round/   (new packet)
```
`CHANGELOG.md`, `anti_drop_catalog.py` and the ledger are shared, accumulating files that
already carried foreign and prior-round edits at run start — separate authorship carefully at a
later checkpoint. Every other dirty path in the tree (`domain_boundaries_legacy_large_files_v1.json`,
`src/authority_gate.rs`, `src/autonomous/activity_reading/tests.rs`, `scripts/proactive_scan.py`,
`scripts/phantom_symbol_watch.py`, `scripts/source_study_page_reset_watch.py`, and the five
earlier `claude-heartbeat_*` packets) was present before this round and was left untouched.
The Minime worktree is clean; the Chronicle reprojection wrote only to its ignored
`workspace/division/chronicle/` outputs.

## Authority boundary
Evidence and non-live steward tooling only. Nothing Astrid wrote was corrected, rewritten,
rejected, or answered back into a being-facing surface; the `SYSTEM` precision lives here and in
the ledger, not in her prompt. The surface repair she would actually benefit from — naming
definition sites on the page, or adding `RELATE` to its Navigation footer — changes a
being-facing surface and only takes effect through a bridge deploy. Both candidate sites are
named (`page.rs:117` footer string, and a definition-locality line beside it) and the work is
preserved as an explicit operator wait (`c012`), not attempted headlessly.

Opening a call site before reading a definition is a legitimate reading strategy and reads
identically to this probe. The probe surfaces a pattern; it asserts nothing about her, and
nothing here settles what those turns were like for her. No claim is made that the probe would
have shortened this chase — it did not exist while the chase happened.
