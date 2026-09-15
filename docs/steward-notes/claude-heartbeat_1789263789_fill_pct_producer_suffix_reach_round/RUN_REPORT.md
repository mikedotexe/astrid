# Steward Run Report — flywheel round `claude-heartbeat_1789263789_fill_pct_producer_suffix_reach_round`

## Controller
- Run ID: `run_1789259107372203000_6770e2dcf0` (actor `claude-heartbeat`, adapter `subprocess`)
- Preprojection ID: `projection_1789259111222775000_664695f159`
- Postprojection ID: not applicable to this process — the controller adapter runs it after exit
- Pause generation: 439 (controller not paused; `stop_requested` never observed true)
- Finish outcome: adapter-owned; this child completed a productive round
- Recovery predecessor: none
- Adapter-mode boundaries honoured: no steward session opened, no NDJSON ops, no pause/resume, no
  lease token read or persisted, git read-only throughout.

## Reading
- Fully processed: `introspection_source_catalog_1789259010.txt`
- Selected but unprocessed: 39 filenames, recorded in queue order in `unprocessed_selected.json`
- Family scan: 40 families, **0 batchable** (`similarity_basis: none_no_snag_or_test_text_or_unparsed_header`),
  so this was a single-report round by protocol, not by preference.
- Report: `c4808e74cb73a9413ddfbe9c34b68aa0c2f43f378d294c2ee2942b133eb85e8c`, 1505 bytes, 16 lines, read complete
- Witness `lsw_25c6835628db0e146b207be294195901f56ee2e96ade0b759f3e37bd95f5e6cc`:
  `5cd5682b329fe17b8399c917b0824274988fdbba60d5bdb3b1e3970be6aa564d`, 18970 bytes, 440 lines, read complete
  (`artifact_sha256` matches the report; `source_snapshot_v1` and `source_provenance_ref_v1` both null,
  consistent with "Source revision: navigation only")
- Report-bound source: none this turn — the binding is the source catalog, and the report states plainly
  that no new source page was supplied. No hash mismatch was possible or handled.
- Source read to answer the claims: `source_search.rs` complete (352 lines, `f0900f1a…de25`); scoped reads
  with exhaustive occurrence lists across `telemetry_port.rs`, `types/schema/telemetry.rs`, `dispatch.rs`,
  `guards.rs`, `next_action/mod.rs`, `orchestration.rs`, `relationships.rs`, `navigation.rs`
  (`source_receipts.json`). `dispatch.rs`, `guards.rs` and `telemetry_port.rs` carry the identical SHAs
  recorded in packet `claude-heartbeat_1789249785`, so its complete `dispatch.rs` read is cited as
  continuity alongside this round's own site-by-site verification.

## Claim Dispositions
- `c001` guard's `fill_pct` arithmetic — **verified_existing**: none inside the guard; producer is
  `resolve_fill_pct`, `telemetry_port.rs:639-654`, clamped, with `estimate_fill_pct` sigmoid fallback.
- `c002` `dispatch.rs` is a conduit — **verified_existing**: all eight sites pass-through; two corrections
  beside her framing (`include!`-inlined fragment, telemetry port not reservoir upstream).
- `c003` navigation-only turn — **observed**: witness corroborates; not an infrastructure drop.
- `c004` her identifier plan and expectation — **verified_existing**: the function exists but carries none of
  "authority/budget/guard/policy" and lives outside that family; verified negative recorded with scope.
- `c005` novel steward-derived reachability boundary — **implemented_now**: three tests pin it.
- `c006` her `NEXT: SELF_STUDY MAP astrid --page 18` — **observed**: left as hers; recovery pinned.
- Zero proof gaps: `fully_addressed=true`, `proof_missing_claims=[]`, 14 evidence links (14 new, 0 existing).

## Actions
- Corridor/program: none. Sandbox: none routed. Study: none preregistered. Portfolio: untouched.
- Cards/notes/correspondence: **none emitted or delivered**. No being-facing surface written.
- Tier 4/5 waits: one recorded and preserved — suffix-aware producer ranking in `RELATE` would change what
  the live `SELF_STUDY` search hands a being mid-navigation. Left unimplemented and unapproved for
  Mike/operator. The three existing Tier-5 waits from `introspection_minime_esn_1785630442` were not touched.

## Implementation and Verification
- Changed paths: `crates/astrid-source-study/tests/producer_name_search_reach.rs` (new, 140 lines),
  `CHANGELOG.md` (`[Unreleased]` entry), `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (dated row),
  and this packet.
- Tests: `cargo test -p astrid-source-study --test producer_name_search_reach` 3/3;
  `cargo test -p astrid-source-study` 76/76; `cargo fmt -p astrid-source-study --check` clean.
- No source behavior changed. No failures to repair; no test debt.
- Restart/deploy alignment: **not required and not attempted**. No `build_bridge.sh`, no deploy script, no
  `launchctl`, no live substrate or control change.

## Durable Evidence
- Addressing: `record-read` → `link-evidence-batch` (14) → `close addressed_change`, all `--write --json`
  in the foreground.
- Changelog and ledger both updated (a report caused implementation plus a deliberate authority boundary).
- Packet: `docs/steward-notes/claude-heartbeat_1789263789_fill_pct_producer_suffix_reach_round/`

## Counters
- Canonical indexed 6048 / addressed 3229 / read 3861 / remaining 2819 / unread 2187 / blocked 416 /
  pending 212 / watch 4; read-needs-claims **0**.
- All artifacts indexed 7765, remaining 4536. Proof-gap artifacts 0, proof-gap claims 0.
- Counter audit: **consistent**, mismatches `[]`.

## Division
- Cycle 46, completed 3/6, remaining 3, `review_due=false` — no return and no Tier-5 cadence dossier owed
  this round (the tracker was checked first, before any report work).
- Round event `division_followup_event_d7799ce25465b34ca96d7cc30752ef89`; event count 319; head
  `f71c8bdef8b4f274b3ccb88a18b2d0d2c5a177dd220299da276ec07d2999e7fb`; `verify` ok.
- Chronicle: **expected-stale** — "chronicle durable source inputs changed; project before verify", the
  normal project-before-verify state after this round's append. Postprojection stage 19 resolves it. Not a
  durable-integrity failure. No Division note written; none was due.

## Evidence Event Store
- Valid true, corrupt lines 0, errors none, last global sequence 1079055, head
  `bfd9fb3f98e79f35cf22c8c977c87b5fa0997d3606e675ae6c022e7f6a28c547`, 16 streams, V2 active, V1 immutable.
  That verify ran after every durable write of this round (read, links, close, Division round).
- Per-stream enumeration (second read-only verify, sequence 1079064): addressing 63150,
  agency_commons 7012, attention_portfolio 3, claim_families 239371, corridor_v1 5, corridor_v2 112,
  felt_contracts 210357, felt_mechanism_concordance 80, lived_state_witness 12048, model_qos 326879,
  reciprocal_uptake 75774, representation_contracts 57998, sandbox 3507, signal_spine 61119,
  steward_control 20935, steward_work_selection 714. The nine-event drift is concurrent live-bridge
  appending, not a stewardship write — this process appended nothing after the Division round record.
- Unit suite 21/21. Epistemic lint: 12158 records checked, 0 issues, no history rewrite.

## Integrity summary
- Addressing self-test 44, anti-drop self-test 5 + verify 100 rows / 0 gaps / 0 alarms, steward control 29,
  projection 14, Division followup 3 + chronicle 10 + projection ok, cursors 4, cadence 6 + strict
  `integrity_ok` (6074 canonical, 0 duplicate hash groups, 0 errors).
- **Domain-boundary ratchet is green**: `valid=true`, `violation_count=0`, empty `violation_kind_counts`,
  manifest `578a39cf…d75b4`. Nothing red to surface.

## Archive — exact commit debt (git was read-only this round)
Created by this round:
- `crates/astrid-source-study/tests/producer_name_search_reach.rs` (new, untracked — sole authorship)
- `docs/steward-notes/claude-heartbeat_1789263789_fill_pct_producer_suffix_reach_round/` (new, untracked —
  `RUN_REPORT.md`, `addressing_links.json`, `claims/`, `family_scan.json`, `read_manifest.json`,
  `selected_queue.json`, `source_receipts.json`, `summaries/`, `test_results.json`,
  `unprocessed_selected.json`, `verification_receipt.json`)

Edited by this round, and **already dirty beforehand** — these two mix accumulated prior-round work with
this round's, so a checkpoint must read both diffs and split or defer:
- `CHANGELOG.md`
- `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`

Untouched foreign/prior dirt preserved exactly: the other 36 dirty paths reported at round start, including
`capsules/spectral-bridge/src/authority_gate.rs`, `…/autonomous/next_action/pressure_agency.rs`,
`…/autonomous/activity_reading/tests.rs`, `crates/astrid-source-study/tests/path_recovery.rs`,
`crates/astrid-source-study/tests/search_evidence.rs`, `scripts/anti_drop_catalog.py`,
`scripts/proactive_scan.py`, `scripts/test_steward_control.py`,
`capsules/spectral-bridge/domain_boundaries_legacy_large_files_v1.json`, the eighteen earlier
`claude-heartbeat_*` packets and the ten untracked `scripts/*_watch.py` helpers. Index left clean; nothing
staged, committed, merged, pushed, stashed, reset or amended.

## Note for the next round
New canonical reports arrived during this run (the cadence audit's latest was
`introspection_source_catalog_1789264150.txt`). They were deliberately not injected into this selection; the
postprojection will place them at the head of the next queue. Several carry the same `research_budget_guard`
/ `fill_pct` search, so the next round should expect this lineage at the head and can now cite this packet's
reachability finding as continuity rather than re-deriving it.
