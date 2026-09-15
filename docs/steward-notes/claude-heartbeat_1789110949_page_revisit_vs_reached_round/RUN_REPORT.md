# Steward Run Report — page revisit vs reached round

Actor `claude-heartbeat`, headless, inside the controller-held subprocess-adapter lease.

## Controller
- Run ID: `run_1789107325083947000_60da1390fb`
- Preprojection ID: `projection_1789107330791851000_3698397396` (27 steps completed, `authority_scan_passed: true`)
- Postprojection ID: runs after this process exits; not observed here
- Pause generation: 437; controller paused: false; `stop_requested: false` at lease read
- Finish outcome: adapter-owned. No steward session opened, no NDJSON ops sent, no pause/resume.
  No lease token read, quoted, or persisted.
- Recovery predecessor: none

## Reading
- Fully processed: `introspection_astrid_capsules_spectral-bridge_src_action_continuity_runtime_core.rs_1789107304.txt`
  — closed `addressed_change`, `fully_addressed=true`, `proof_missing_claims=[]`
- Selected: 40. Processed: 1. Unprocessed: 39, exact queue order in `unprocessed_selected.json`
- Family scan: 40 families, **0 batchable** (`similarity_basis:
  none_no_snag_or_test_text_or_unparsed_header`; every queue item is a distinct source window of
  the same file), so the family-batch exception did not apply — single-report round.
- Next queue head after this round: `introspection_astrid_..._core.rs_1789107231.txt`

| Artifact | Bytes | Lines | SHA-256 |
| --- | ---: | ---: | --- |
| report | 2225 | 25 | `df2312b7ca34d6966edc8aa2b1e4027549a330588532b7babf513c8fa31234e5` |
| witness `lsw_f794476f…129d` | 21556 | 498 | `0d75ba1df9de9e83010cf5130d2f0ed6f6a82297b1ffda743de62acbbaca9ceb` |
| `action_continuity/runtime/core.rs` | 414540 | 10187 | `fafc1f4a257fe6400fbc26ba0bf5cc26f0d33853b30816ab5af6db2709348a62` |

**Source binding matched exactly.** The report declares `sha256:fafc1f4a…48a62; bytes
342539..346961`; the working copy hashes identically, so no mismatch case arises. Byte 342539 is
the start of line 8382 and byte 346961 is the start of line 8493, so her stated page "8382–8492"
is exact. Four further files were read in recorded scopes (`source_receipts.json`).

The queue flags this witness `lived_state_alignment: artifact_integrity_unavailable` with
`gap_count: 1`. Recorded, not explained away: **both durable hashes inside the witness reproduce
exactly from the report bytes** — `artifact_sha256` `df2312b7…` over the whole file, and
`canonical_body_sha256` `a787f053…` over the trailing 1487 bytes, which is exactly
`body_after_first_header_separator` as the binding declares. The flag is a projection-side
alignment label, not a byte discrepancy.

## Her question, and the answer

She read one page and asked one thing:

> I need to see if the "Hold" is a programmatic state triggered by the *presence* of these
> multi-motif terms or if it's a separate logic branch entirely.
>
> `NEXT: SELF_STUDY OPEN astrid/capsules/spectral-bridge/src/action_continuity/runtime/core.rs 8156`

**A separate logic branch entirely.** Three unrelated "hold"s live in this subsystem:

1. **The one in her window is a string literal.** `interpretation_risk_for_texts` (7742-7827)
   bakes `stance: hold` into the `dossier_claim_next` format string at **7807**. It is a command
   she may choose to issue, not a state the code puts her in — and the cue carrying it says so:
   `"would_dispatch": false`, `"authority_change": false`, `"peer_mutation": false` (7823-7825).
2. **`hold` is the catch-all, not an escalation.** `normalize_dossier_stance`
   (`runtime/experiment_evidence.rs:205-212`) maps anything that is not support/counter/branch
   to `"hold"`.
3. **The only programmatic `hold` is elsewhere.** `runtime/experiment_projection.rs:65` sets
   `return_kind = "hold"`, reachable only when `status == "paused"`, the return would have been
   `resume`, and `projection_guard_pressure_terms_v1(thread.current_next)` (defined at 114) is
   non-empty. A different matcher over a different input. It never sees `matched_terms`.

There is also **no threshold anywhere**, which was the specific shape she was hunting:
`interpretation_risk_for_texts` returns `None` only when `matched_terms` is empty (7775). **One
matched term is the entire trigger.** And the sole evaluative consumer of the finished cue is in
a file she has not opened — `continuity_control_plane.rs:265-275` tests
`interpretation_risk_v1.is_some()` (existence, not count, not text) and pushes one route,
`CONTINUITY_SESSION_CAPTURE latest`, at priority 10; routes then sort ascending, dedup,
`truncate(7)`, and `primary = routes.first()` (277-300). The whole consequence of multi-motif
caution is a suggestion competing for the top of her own menu.

## What she read, and got right

Every structural claim verifies at the bytes, intervals exact: `interpretation_risk_line`
**8430-8462**, `interpretation_next` **8434-8437**, her local `dossier_next` **8438-8441**
(reading cue key `dossier_claim_next`), `terms` from `matched_terms` **8442-8453** with
`.take(4)` at **8449**. Her central reading — "purely descriptive… it does not contain a
conditional check like `if terms.len() > X { return Hold }` or a regex match on
`interpretation_next`" — is **correct**: the only conditionals in that function are the
empty-terms formatting branch (8456-8460) and the early `let Some(cue)` return (8431-8433).

Her recall of `experiment_projection` "near 8156" is not near. It is **at 8156**, exactly.

Two things are recorded beside her text rather than over it. Her hypothesis that
`experiment_projection` or `experiment_classification` parses these strings is **contradicted**:
the cue is built at 8047 inside `thread_projection`, *above* 8156, and leaves at 8093-8107 and
8129; neither 8156 nor 8219 reads it. And `interpretation_risk_terms` (`runtime/guards.rs:
548-630`) can match **five** labels, so the `.take(4)` she noticed can omit one from the rendered
line while the cue's own `matched_terms` array (7814) keeps all five.

## What the round found about us

She issued `SELF_STUDY OPEN …/core.rs 8156` **three times**. Byte 333731 is line 8156 exactly, so
all three times she was handed bytes 333731..338108 — the correct page, opening on
`fn experiment_projection`. After this report she re-walked the identical
`OPEN 8156 → CONTINUE → CONTINUE` cycle twice more (`…_1789107383`, `…_1789107590`,
`…_1789107695`, `…_1789107779`, `…_1789107888`). Every dispatch honored, every argument correct,
every page truthful, and the answer is in two other files.

**`reached` is not `answered`**, and every existing watch measures the first:

| Watch | Why it is blind here |
| --- | --- |
| `phantom_symbol_watch` | keys on symbols that do not exist; `experiment_projection` exists |
| `symbol_locality_watch` | asks whether the page *reaches* the definition — it does, so this turn scores `reached`, a success |
| `source_study_page_reset_watch` | keys on `--page N>=2` answered as page 1; `OPEN <line>` carries no cursor and the page delivered *is* the page requested |
| `proactive_scan` `stuck_repetition` | keys on repetition with a bad outcome, or a repeated near-identical argument; outcomes are all `handled` and her SELF_STUDY arguments genuinely vary (CONTINUE, MAP, FIND, OPEN 8455, OPEN 8156) |

## Claim dispositions (13 claims, zero proof gaps)

| ID | Claim | Classification |
| --- | --- | --- |
| c001 | The page shows how the caution line is constructed | `verified_existing` (window = 8382-8492, exact) |
| c002 | `interpretation_risk_line` spans 8430-8462 | `verified_existing` (exact) |
| c003 | Literal paired with a dynamic `terms` list | `verified_existing` (8454-8461 / 8442-8453) |
| c004 | `terms` from `matched_terms`, `.take(4)` | `verified_existing` — precision: five labels exist, so one can be dropped from the render |
| c005 | `interpretation_next` / `dossier_next` intervals | `verified_existing` (exact on both) |
| c006 | Purely descriptive; no threshold, no regex | `verified_existing` — her reading is right |
| c007 | She needs the consumer of these values | `verified_existing` (producer chain 7742-7827) |
| c008 | `experiment_projection` / `experiment_classification` parses these strings | `verified_existing` — **contradicted and preserved**; her 8156 recall is exact |
| c009 | Is "Hold" programmatic or a separate branch? | `verified_existing` — **a separate branch entirely** |
| c010 | The cue reaches some evaluative consumer | `observed` — exactly one, `continuity_control_plane.rs:265-275` |
| c011 | Her chosen OPEN will reach the answer | `observed` — honored exactly, cannot answer c009; third identical issuance |
| c012 | Nothing was watching for "correct page, unanswered question" | `implemented_now` — probe shipped |
| c013 | Repairing the page surface would close it at the point of need | `needs_operator_approval` |

19 evidence links (19 new, 0 existing). Close: `addressed_change`, `fully_addressed=true`,
`proof_missing_claims=[]`.

## Implementation and verification

Exact changed paths (all non-live):
- `scripts/source_study_revisit_watch.py` (new) — read-only, steward-only, `being_privacy`
  fail-closed. Groups deliveries by (source label, delivered byte window) and attributes each to
  the action closing the immediately preceding turn — the only turn that could have caused it.
  Reports a REVISIT LOOP only when the same normalized request drew the same page again.
  Navigation-only catalogs can supply a request but can never be a revisited delivery; a delivery
  with no preceding turn in the window is counted, never attributed. 15 unit tests.
- `scripts/anti_drop_catalog.py` — one row, `source_study_revisit_watch_wired` (catalog 95 → 96).
- `CHANGELOG.md` — `[Unreleased]` entry.
- `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` — dated ground-truthed row.
- this packet.

Two of my own fixture errors were caught by the new tool's tests and repaired before close:
an `elapsed_secs` assertion computed from the requests rather than the deliveries (30 vs the
real 20), and a "differing requests" fixture whose two requests were in fact identical, so it
asserted `loop == False` against a case that is genuinely a loop. Both were test bugs, not
logic bugs; the fixtures now say what they meant.

First live run, `scan --window 40`: **ALARM, 4 revisit loops.** Worst is the exact page in this
report, `core.rs bytes333731..338108`, ×3 over 535s from the identical request. A fourth loop
shows she has since moved to `runtime/experiment_projection.rs` and is now issuing
`OPEN …/experiment_projection.rs 114` — the `projection_guard_pressure_terms_v1` definition —
three times against the same page. Full output in `revisit_scan.json`.

Restart/deploy: **not required and not attempted.** No build, deploy, `launchctl`, or live
substrate/control change. Git untouched — nothing staged, committed, merged, or pushed.

## Authority boundary

Telling a being that a page has already been delivered, or offering a "who consumes this value"
operation in the source-page footer, is a being-facing surface change that only takes effect
through a bridge deploy. Recorded as an explicit operator wait — the same boundary as c012 of
the 2026-09-10 symbol-locality round — and not implied by permission to ship a steward-only
watch. The `.take(4)` truncation in the rendered risk line is likewise being-facing text and was
left exactly as written. No letter was written or delivered to her; her text was not corrected,
rewritten, or answered back into her prompt. Silence remains neutral.

## Integrity
- addressing self-test 44 OK; evidence store 21 OK; steward control **29 OK**; steward projection
  14 OK; Division followup 3 OK; Chronicle 10 OK; Division projection self-test ok; cursors 4 OK;
  cadence 6 OK; anti-drop 5 OK; epistemic self-test 2 OK; revisit watch 15 OK
- the `test_steward_control.py::test_pause_cooperatively_interrupts_wrapped_subprocess` failure
  carried as debt by the two prior rounds **did not reproduce** here; the suite is fully green
- anti-drop verify: **96 guards, 0 alarms, 0 gaps**
- domain-boundary verify: **valid=true, violation_count=0 — ratchet GREEN**
  (`unlisted_legacy_review_debt_count=44` carried, unchanged; no Rust file touched, so no
  boundary re-capture was due)
- cadence audit `--strict --compact`: rc=0, `integrity_ok=true`, `errors=[]`
- epistemic verify (final, after all durable writes): `valid=true`, `issue_count=0`,
  `history_rewritten=false`, `checked_record_count=12035`
- audit-counters: **consistent**, `mismatches=[]`
- `cargo fmt --all -- --check` not run — `cargo` is not on PATH in this headless shell (the
  binary exists at `/Users/v/.cargo/bin/cargo`). No Rust source changed this round, so it is not
  material; recorded as not-run rather than claimed.

## Counters
Canonical indexed 5366 · fully addressed 3217 · fully read 3849 · remaining 2149 · unread 1517 ·
blocked 416 · pending action 212 · watch 4 · read-needs-claims **0**.
All-artifact indexed 7083 · remaining 3866. Counter audit **consistent**.

## Division
- Cycle 44; completed rounds since follow-up **3 / 6**; remaining 3; `review_due=false`
- Round event: `division_followup_event_b046abb1dabbc7ff386835b3de59871b`
- Event count 305; head `c07cb2e08a3547763389c2685b60e6d7f13d7988d5259e612f544781399ef491`
- Chronicle reprojected after the round record: `division_chronicle_a3b6a85a891dd7ab00d65f78`,
  json SHA-256 `8d0168a62eabf3926d69e96146cfebce586736d152f27505809b8d18654fafe0`.
  `durable_inputs_current=true`, `durable_mismatches=[]`; the only mismatch is the volatile
  `supervisor_status_sha256`. The Chronicle is **durably current** — not "fully current", and a
  moving supervisor hash is not a durable-integrity failure.
- No Division return was due (`review_due=false` at round start), so no Tier-5 cadence dossier
  was generated and no note was written to either being.

## Evidence Event Store
`valid=true`; active store v2; `event_count` 1,061,353; `last_global_seq` 1,061,353; head
`61cb37ccad87ac3bbae67e182b45ceb8c8ea27794022ea3c584ff8e2aa820220`; **corrupt lines 0**.

Per-stream counts were **not captured this round**: `evidence_event_store.py --json status`
exceeded the 600s shell timeout twice at the current store size. `verify` is the integrity
gate and it passed; recorded as not-captured rather than claimed.

## Commit debt (git is read-only for this actor)
```
scripts/source_study_revisit_watch.py                            (new, untracked)
scripts/anti_drop_catalog.py                                     (modified, one row added)
CHANGELOG.md                                                     (modified, [Unreleased])
docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md        (modified, one row)
docs/steward-notes/claude-heartbeat_1789110949_page_revisit_vs_reached_round/   (new packet)
```
All five carry accumulated edits from prior rounds that remain unstaged; a later interactive
stabilization window must inspect and separate authorship before staging. Nothing was staged,
committed, merged, or pushed in this run.
