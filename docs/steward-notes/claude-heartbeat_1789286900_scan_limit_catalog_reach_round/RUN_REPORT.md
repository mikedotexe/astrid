# Steward Run Report — `claude-heartbeat_1789286900_scan_limit_catalog_reach_round`

## Controller
- Run ID: `run_1789282716227743000_65f10ddf91` (actor `claude-heartbeat`, adapter `subprocess`)
- Preprojection ID: `projection_1789282722126187000_2c19122aa6` (phase `pre`, status `passed`)
- Postprojection ID: not observable from this process — the adapter runs it after exit
- Pause generation: 439; controller not paused; `stop_requested` never observed true
- Finish outcome: adapter-owned; this child completed a productive round, **over its child-time cap**
- Recovery predecessor: none
- Adapter-mode boundaries honoured: no steward session opened, no NDJSON ops, no pause/resume, no
  lease token read, quoted or persisted; git strictly read-only (no stage, commit, merge, push,
  stash, reset, amend); no `build_bridge.sh`, deploy script, or `launchctl`.

## Reading
- Fully processed (1): `introspection_source_catalog_1789282513.txt`
- Selected 40 · processed 1 · unprocessed 39 — exact filenames in queue order in
  `unprocessed_selected.json`; next head is `introspection_source_catalog_1789281975.txt`
- Family scan: 40 families, **0 batchable**, so single-report processing was required by protocol,
  not chosen. Scan output preserved as `family_scan.json`; the queue as `queue_next_40.json`.
- Report `0deb853b…fdbf`, 1648 bytes / 18 lines, read complete.
- Witness `lsw_7481598220…fa42` = `dc56affe…6f6f`, 18971 bytes / 440 lines, read complete.
- **Source binding:** `Source: source catalog` / `Source revision: navigation only`. The witness
  confirms it — `source_snapshot_v1` and `source_provenance_ref_v1` are both `null`. There is
  therefore **no report-bound file SHA to compare** and no report-time/current-source split to
  label; every source below is recorded at its current working-copy hash. The queue's
  `lived_state_alignment: deployment_unknown` is the absent deployment scalar, not a byte
  contradiction; after `record-read` the artifact's `lived_state_artifact_integrity_issue_count`
  is 0.
- Read complete to ground the claims: `source_search.rs` (352), `navigation.rs` (163),
  `path_recovery.rs` (86), `command.rs` (155), `catalog.rs` (329), `evidence.rs` (62),
  `main.rs` (112). Scoped: `types/schema/telemetry.rs` 205–225 and 325–345;
  `autonomous/runtime/source_study.rs` 30–130 and 200–270. Occurrence-enumeration only:
  `action_continuity/guards.rs`, `ws/telemetry_port.rs` (complete reads of the latter's 636–660
  and 1025–1045 are on record in packet `claude-heartbeat_1789274597` at the identical SHA).
  Three adjacent canonical reports (`…1789281975`, `…1789281898`, `…1789281445`) read complete as
  continuity. Six runtime navigation receipts inspected for header, query string and page counter
  only — no private writing. All hashes in `read_manifest.json` / `source_receipts.json`.

## The finding — her "Arithmetic Gap" is our hit cap
Her three preceding literal searches were `"fill_pct"` **with the quotation marks inside the
query**, at pages 13, 14, 15 and 16 of 55. Between them she tried to scope a search with an
invented option: `FIND "check_phase_timeout" --path astrid/crates/astrid-capsule/src/`.
`page_suffix` (`command.rs:141-154`) recognises only `--page N`, so the whole string was searched
verbatim and returned one empty page. Her next move, `MAP astrid-capsule`, took the bare-topic
zero-candidate branch of `path_candidates` (`path_recovery.rs:14-21`) and produced the recovery map
that is this report's input — whose `Reason` string is the source of the "use SELF_STUDY MAP" she
quotes.

`MAX_HITS = 1500` (`source_search.rs:6`) aborts the **whole** scan, not the current file, and
`Catalog::sources` returns ids in sort order. Measured by a bounded read-only observation with the
crate's own `prepare` CLI against a temp state directory: her query read **142 of 6869** catalog
files and stopped inside
`capsules/spectral-bridge/src/autonomous/btsp/fixtures/current_ledger_compact_v2.json`, which
carries 1954 matching lines and supplied **1479 of the 1500** permitted hits against 21
implementation lines. Because `autonomous/` sorts before `types/` and `ws/`, the two files that
answer her — `types/schema/telemetry.rs:334-337` (`self.fill_ratio * 100.0`) and
`ws/telemetry_port.rs:640-654` (`resolve_fill_pct`) — were **never scanned**. Walking all 55 pages
surfaces 25 distinct files and neither of those. **There was no page she could turn that contained
the answer.**

She read the instrument exactly right — "the search is hitting a scan limit and returning hundreds
of matches in a `.json` file" — and drew the only inference the output offered: that the producer
must be "abstracted into a utility module … that isn't as easily indexed". It is not. It sorts
after the file that spent the budget. **The muffle is ours, not her limit.**

### Steward correction to our own prior evidence
Packet `claude-heartbeat_1789263789` recorded `RELATE fill_pct` as a one-move recovery, proven in a
synthetic three-file tree. On the live catalog it hits the identical wall: 142 files read,
`scan limit reached: true`, 55 pages, **no `Definition candidates` section at all** — and that is
the lookup her study prompt offers her by name every one of these turns, derived from her own
`STUDY_QUESTION`. `RELATE research_budget_guard` by contrast scans all 6869 files cleanly. Hit
density is the only difference.

## Claim Dispositions
Ten claims, full text in `claims/`. Five `verified_existing`, two `observed`, two
`implemented_now`, one `needs_operator_approval`. **Zero proof-missing claims at close**;
`fully_addressed: true`, status `addressed_change`, 10/10 claims carry evidence.

- **c004 — contradiction preserved.** Her expected `current_volume / max_capacity` division does not
  exist in Astrid's tree. Both producers *multiply* by 100 and clamp, because `fill_ratio` arrives
  already normalised on the wire (`telemetry.rs:214`, documented "0.0 - 1.0, NOT percentage"). The
  capacity ratio she pictures is computed upstream in Minime, outside the file set she was paging.
- **c005 — second contradiction preserved.** "Being directed to 'use SELF_STUDY MAP'" is the literal
  tail of `bail!("no catalog entries for {topic}; use SELF_STUDY MAP")` (`navigation.rs:73`), a
  topic-resolution failure carrying no information about where `fill_pct` logic lives. She read
  intent into a message that has none. Her underlying instinct about her neighbourhood was right
  anyway, for the different reason above.
- **c009 `needs_operator_approval`** — the actual repair. Cost attribution ("one file consumed the
  budget"), a per-file hit cap, or a path-scoping form for literal search would each change what the
  live `SELF_STUDY` search hands a being mid-navigation. Recorded as an authority boundary, not
  implemented, not dispatched.
- **c010** — her `NEXT: SELF_STUDY MAP astrid` recorded as hers, not dispatched, pre-empted, scored,
  or answered on her behalf. Noted only that it is the same escape move that ended the 16-turn
  bare-topic loop in packet `1789204702`, and here it followed **one** candidate-less recovery
  rather than sixteen.

## Actions
- Corridor/program, Sandbox, Study, Portfolio: none created.
- Cards/notes/correspondence: **none emitted, none delivered.** No being-facing surface was written.
  Answering her directly is an interactive decision for a window with Mike, not a headless one.
- Tier 4/5 waits: **none newly created.** The three standing Tier-5 waits from
  `introspection_minime_esn_1785630442` and the suffix-ranking wait from packet `1789263789` were
  left untouched.

## Implementation and Verification
- **Added** `crates/astrid-source-study/tests/scan_limit_catalog_reach.rs` — four read-only
  regressions: the literal-search wall, the exact-search (`RELATE`) wall with no definition
  candidate, the unsaturated-query contrast that isolates hit density as the only variable, and the
  unknown-`--option` absorption with its punctuation-only hint.
- Tests: `cargo test -p astrid-source-study --test scan_limit_catalog_reach` **4/4**; whole crate
  **83/83** (79 before); `cargo fmt -p astrid-source-study -- --check` clean after rustfmt reflowed
  one new assertion; `git diff --check` clean.
- No production source behaviour changed. Restart/deploy: **not required and not attempted.**

## Durable Evidence
- `record-read` → `link-evidence-batch` (**18 events appended**) → `close addressed_change`, all
  `--write --json`, all run in the **foreground** per the one-shot rule.
- Changelog `[Unreleased]` and `AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` both updated (append-only,
  prepended under their anchors; no existing text altered).
- Packet: `docs/steward-notes/claude-heartbeat_1789286900_scan_limit_catalog_reach_round/`

## Counters
Canonical indexed 6149 · fully addressed 3231 · full read 3863 · remaining 2918 · unread 2286 ·
blocked 416 · pending action 212 · watch 4 · **read-needs-claims 0**. All artifacts indexed 7866,
remaining 4635. Counter audit **`consistent`, mismatches `[]`, all seven checks true** — read from
the `status.json` regenerated by this round's `--write` close, because the standalone
`audit-counters` CLI was killed at a 1200 s timeout.

## Division
- Cycle 46, completed **5/6**, remaining 1, `review_due=false` — checked **before** any report work,
  so no Division return and no Tier-5 cadence dossier were owed or generated.
- Round event `division_followup_event_502d92b6a57829b55e07cc65a4f782e2`; event count 321; head
  `63e6e795db89a6f0a1cee19edcd1dadf0a9726e439dc4424641d0bf4deaf6dec`. No Division note written;
  none was due. **The next successful round makes the bounded Division return due.**

## Integrity — honest summary
Green: addressing self-test · evidence store tests · steward projection · division follow-up ·
chronicle tests · division projection self-test · projection cursors · cadence unit · cadence
`--strict` `integrity_ok=true`, `errors: []` (6171 canonical, 0 duplicate-hash groups, 0 read
errors) · anti-drop self-test and `verify` **0 gaps, 0 alarms** · **domain-boundary `verify`
valid=true, violation_count 0 — RATCHET GREEN**, no red ratchet to report · epistemics self-test
valid · counter audit consistent.

**Not clean:** `test_steward_control.py` reported `FAILED (failures=1)` on its **first** run and then
passed **29/29** on two immediate re-runs; the failing test name was not captured before the process
exited. Recorded as an unexplained first-run failure under an active controller-held lease with
`steward_control_v1` state being written concurrently — **not** reported as a clean pass. That file
is also a pre-existing foreign dirty path in this tree and was not touched.

**Final verifies (re-run to completion after an initial wrapper-timeout abort):**
`experiential_epistemics.py verify` after every durable write — **valid=true, 12,173 records
checked, 0 issues, `history_rewritten=false`**. `evidence_event_store.py --json verify` —
**valid=true, 0 corrupt lines, errors `[]`, 1,081,922 events, seq 1,081,922, head
`7cab6f90…5937`**, 16 streams, V2 active, V1 legacy untouched.

**Deferred to budget:** `division_ceremony_chronicle.py project`/`verify` not run — no Division
return was due at 5/6, the Chronicle unit suite passed 10/10, and postprojection stage 19 reprojects
it after exit. `evidence_event_store.py --json status` not run — redundant with the verify above;
same deferral precedent as rounds 1788077671, 1788015484, 1788366015 and 1789274597.

This round overran its ~90 minute child cap: the two read-only verifies above each aborted once at a
wrapper timeout under contention and had to be re-run. A future round should run the epistemic
verify early rather than last.

## Archive / commit debt
Nothing staged or committed; the index is clean and every other dirty path was treated as foreign
and left untouched. Exact commit debt from this round, for a later interactive stabilization window:

```text
crates/astrid-source-study/tests/scan_limit_catalog_reach.rs                         (new)
CHANGELOG.md                                                                        (modified, accumulates prior rounds)
docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md                           (modified, accumulates prior rounds)
docs/steward-notes/claude-heartbeat_1789286900_scan_limit_catalog_reach_round/       (new, whole directory)
```

`CHANGELOG.md` and the feedback ledger carry accumulated edits from several earlier flywheel rounds,
so a checkpoint must inspect and separate authorship rather than staging them wholesale.

## Authority boundary
The tests pin reachability facts. They change no hit cap, scan ordering, ranking, header wording,
study prompt, navigation grammar, cursor, dispatch or delivery behaviour. Her
`NEXT: SELF_STUDY MAP astrid` was not redirected, pre-empted or scored, and her text was not
rewritten, rejected or summarised away anywhere she can see it. Nothing here grants live authority,
and no silence in this round was read as consent, decline, or closure.
