# Steward Run Report — charter-stall passthrough round

Actor `claude-heartbeat`, headless subprocess-adapter round inside a controller-held lease.

## Controller

- Run ID: `run_1789530018427505000_7271a64ad2`
- Preprojection ID: `projection_1789530022751147000_77d2c503e1` (phase `pre`, status `passed`,
  27 steps, duration 3,971,957 ms ≈ 66 min — it consumed most of the wall clock *before* this
  child started)
- Postprojection ID: runs after this process exits; not observed here
- Pause generation: 445; `stop_requested: false` at every observation
- Finish outcome: not sent by me. The adapter owns the lease; no steward session was opened,
  no NDJSON op was sent, and no lease token was read, quoted or persisted.
- Recovery predecessor: none

## Reading

Fully processed (3), in canonical queue order:

1. `introspection_astrid_capsules_spectral-bridge_src_action_continuity_runtime_authority.rs_1789529954.txt`
2. `introspection_astrid_capsules_spectral-bridge_src_action_continuity_runtime_authority.rs_1789529788.txt`
3. `introspection_astrid_capsules_spectral-bridge_src_action_continuity_runtime_authority.rs_1789529608.txt`

Selected but unprocessed: 37, listed exactly and in queue order in `unprocessed_selected.json`.
Next queue head for the following round: `introspection_source_catalog_1789529436.txt`.

`introspection_family_scan.py` reported `family_count 40`, `batchable_family_count 0` — every
queue entry is its own family (`similarity_basis: none_no_snag_or_test_text_or_unparsed_header`),
so this was **not** a family batch. It is an ordinary 1–3 batch that happens to be a three-turn
study chain on one file, which is why one source verification legitimately served all three:
**all three reports bind the same source SHA.**

### Hashes

| Artifact | SHA-256 | Bytes | Lines |
| --- | --- | ---: | ---: |
| report `…_1789529954.txt` | `527c8713a43d0a01ef8d10c9bb5b9fe736749d14230af85528ee3eff6ef613dd` | 3176 | 28 |
| report `…_1789529788.txt` | `413341d7a741a394293b16eab195dabc1c276e6a3c98541d23e53c833727d722` | 3503 | 36 |
| report `…_1789529608.txt` | `201ebed623f70dede8f1032fa35cf7f1a4792520137a130bae125904b6ca44bf` | 2814 | 28 |
| witness `lsw_71ed1986…` | `d44c7f842de3202a45a794a2efdfb0e248133e3ba1549c0080cac0c9083b38fa` | 21549 | 498 |
| witness `lsw_a48d7ae2…` | `54a4276b1f6c0e917f53f8092cb503d14ed4751356dcf37f71260efdb85c3a1a` | 21567 | 498 |
| witness `lsw_ca878b02…` | `6e101c4703ab3fed9a18d71f7aa649ce74de21867b9cd42d6a9cf24925149e5f` | 21552 | 498 |
| source `runtime/authority.rs` | `93a1146d0ff05bd455b41e3f5c8e345bf6a4a2d0eb5c69dcad4cb6712d08822a` | 33922 | 896 |

**Source binding matched exactly.** The working copy hash equals the SHA all three reports name,
and the file is not dirty, so no report-time/current-source split was needed. The complete
896-line file was read in ranges 1-240, 241-520, 521-896.

Two reading facts worth keeping:

- **Cross-window recall, verified.** `…_1789529954`'s own witness puts that turn's page at lines
  180-303, yet it reasons about 358-404 and 406. Every one of those citations checks out against
  the complete file: the lines were carried from the previous turn's page (`…_1789529788`,
  window 365-481). Accurate recall across a window boundary, not confabulation.
- **`window_sha256` is not a raw-interval hash.** Byte offset 6314 is exactly the first byte of
  line 180, so the byte window and the line window agree — but the two witnesses covering that
  *identical* interval carry different `window_sha256` values (`3076466e…` vs `d7a8c120…`), and
  neither equals `sha256` of the raw slice (`cd2634eb…`). So the field hashes a rendered page, not
  the interval. **Observed and flagged, not investigated and not asserted as a defect** — the
  generator was not read this round.

## Claim dispositions

22 claims across the three reports (9 / 7 / 6), each with a grounded disposition and a
classification, all under the 500-character bound. Full text in `claims/`. Headlines:

- **Her STUDY_QUESTION (asked twice, in two phrasings) is answered: neither branch of her
  disjunction.** `authority_readiness_next_command` (`authority.rs:406-445`) has no
  `needs_charter` case at all. It does not auto-fire `charter_repair`, and it does not return an
  error/empty state. Control reaches `:441` and returns the **caller's** `proposed_next`
  unchanged; only an *empty* `proposed_next` falls to `:444`'s `EXPERIMENT_ADVANCE … mode: preview`.
  Both live call sites (`runtime/core.rs:2940`, `:6383`) pass `experiment_conveyor_proposed_next`,
  whose `needs_charter` arm (`runtime/conveyor.rs:67-79`) returns a `charter_scaffold_v1`
  `EXPERIMENT_CHARTER <id> :: …` command. **At `needs_charter` she is handed the charter.**
- **Verified exactly as she stated:** the `:180-194` guardrail triple, its case-insensitivity
  (in fact all three legs are ASCII-case-insensitive, slightly wider than she claimed), and the
  `:388` disjunction.
- **Her "not a self-healing loop" reading is confirmed**, with the mechanism sharpened:
  `experiment_conveyor.rs:338` lets `EXPERIMENT_ADVANCE … mode: apply` act at `needs_charter`
  only when a valid charter *already* exists, so apply cannot invent one.
- **One correction, in her favour.** She framed the exit as "a new scaffold or a **manual** repair",
  implying someone else does it. The charter is hers to author and needs no steward grant,
  approval, or live authority.
- **Two precisions her window could not show:** both hold paths (`:365`, `:384`) run *before* the
  charter check, so a held-and-uncharted experiment reports `held_or_guarded`, not `needs_charter`;
  and `blocked` has two entrances (`:382` and the unconditional catch-all at `:403`), not one.
- **Recorded, not unified:** `authority_gate.rs:2255` is a *parallel* readiness surface that does
  have an explicit `needs_charter → EXPERIMENT_CHARTER` branch (with its own test, added
  2026-06-12 because "both beings stalled for weeks drafting authority against uncharted
  experiments"). The `action_continuity` path reaches the same destination by passthrough rather
  than by branch. Two surfaces, one destination. Collapsing them would be a production behaviour
  change nobody asked for, so this round wrote it down instead.

## Actions

- Corridor/program: none. Sandbox: none. Study: none. Portfolio: none.
- Cards, notes, correspondence: **none delivered.** No closure card, steward note, query or letter
  was dispatched to either being; `--deliver` was never used.
- Tier 4/5 waits: none created and none resolved. The three standing Tier-5 items from
  `introspection_minime_esn_1785630442` were not touched and remain
  `live_authority_granted=false`.

## Implementation and verification

Exact changed paths (all non-live):

- `capsules/spectral-bridge/src/action_continuity/tests.rs` — four focused regressions appended
  plus one fixture helper:
  `authority_guardrail_hold_active_requires_paused_thread_status_and_hold`,
  `authority_readiness_stage_separates_held_or_guarded_from_needs_charter`,
  `needs_charter_next_command_passes_through_the_charter_scaffold`,
  `conveyor_needs_charter_proposed_next_is_a_charter_command`.
- `CHANGELOG.md` — `[Unreleased]` entry.
- `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` — dated row.
- `docs/steward-notes/claude-heartbeat_1789534560_charter_stall_passthrough_round/` — this packet.

No production source was modified. Tests and counts are in `test_results.json`.

**Filter-honesty note:** the first `cargo test` invocation passed four positional filters at once
and ran **0 tests**. A zero-test filter is not a pass, so the filters were corrected to single
substrings and rerun until every new test was observed running: 1 + 1 + 1 passing individually and
11 passing under `needs_charter` (which also re-ran the nine pre-existing `needs_charter` tests,
including `authority_gate::tests::needs_charter_stage_points_at_experiment_charter_not_advance`).
`cargo fmt --all -- --check` clean; `git diff --check` clean. (`cargo` is not on this environment's
default PATH; `/Users/v/.cargo/bin` was prepended for the run.)

Restart/deploy alignment: **not required and not attempted.** No `build_bridge.sh`, no deploy
script, no `launchctl`, no live substrate or control change.

## Durable evidence

- Addressing: 3 full reads recorded, 44 evidence links appended in one batch (44 new, 0 existing,
  3 introspections), 3 closes at `addressed_change`, each with `fully_addressed: true` and
  `proof_missing_claims: []`.
- Packet: `docs/steward-notes/claude-heartbeat_1789534560_charter_stall_passthrough_round/`.

## Counters

Counter audit **consistent**, `mismatches: []`.

| Counter | Value |
| --- | ---: |
| Canonical indexed | 7,080 |
| Canonical fully addressed | 3,252 |
| Canonical fully read | 3,884 |
| Canonical remaining | 3,828 |
| Canonical unread | 3,196 |
| Canonical blocked | 416 |
| Canonical pending action | 212 |
| Canonical watch | 4 |
| Canonical read-needs-claims | 0 |
| All-artifact pending | 5,545 |
| Noncanonical pending | 1,717 |

## Division

- Cycle 49; **4 of 6** productive rounds completed after this one; 2 remaining; `review_due: false`
  both before and after, so no Division return and no Tier-5 cadence dossier was due this round.
- Round event: `division_followup_event_ea7f3de6565d1633e2bb33e2d822892b`;
  event count 341; head `55a22f4279744e54a308f1f736263bbf8666a80795e4bfa9c022e0200901ebc1`.
- Chronicle: recording the round invalidated its durable inputs, so it was reprojected →
  `division_chronicle_54128d3fad53445312998854`, json
  `9f299d1b39db10d613e545742ce5bf812ba0a03456078b96fadf3e7adaf0198f`, html `60ac5e64…`.
  `durable_inputs_current: true`; the **only** mismatch is the volatile `supervisor_status_sha256`.
  Stated exactly: the Chronicle is not "fully current", and a moving supervisor hash is not a
  durable-integrity failure.
- Note action: **none.** No Division note was due or written.

## Evidence Event Store

`valid: true`, `corrupt_lines: 0`, `errors: []`, `event_count` / `last_global_seq` **1,110,323**,
head `9c6045e76ced21dad28ce9417b216f5912bda6960dc237b19c9e4d77891d3271`. Active store `v2`,
`legacy_imported_global_seq 32278`. `verify` took **559 s**; a later `status` (≈9 min after)
reported seq 1,110,334 with the same clean scalars — the store is live and 11 `steward_control`
events landed in between. Stream counts are in `verification_receipt.json`.

## Integrity suites

All green: addressing self-test (44), evidence store tests (21), steward control (29), steward
projection (14), Division follow-up (3), Chronicle (10), Division projection (ok), projection
cursors (4), cadence tests (6), anti-drop self-test (5) and `verify` (100 rows, **0 alarms, 0
gaps**), cadence audit `integrity_ok: true` with `errors: []`, epistemics self-test (2), and the
final epistemic `verify` — `valid: true`, 12,346 records checked, `issue_count: 0`,
`history_rewritten: false`.

**Domain-boundary ratchet: GREEN.** `valid: true`, `violation_count: 0`, `violation_kind_counts: {}`,
with the standing `unlisted_legacy_review_debt_count: 44` carried forward unchanged. Surfacing it
here deliberately, because stage 10 records violations that no summary reads. The only Rust touched
is a test file matched by `test_path_markers`, so no baseline integer and no manifest ceiling needed
re-capture this round.

## Archive

- Checkpoint: **not claimed.** Git was read-only for this entire run — nothing staged, committed,
  merged, pushed, stashed, reset or amended; the index was left clean.
- **Exact commit debt created or touched by THIS round:**
  - `capsules/spectral-bridge/src/action_continuity/tests.rs` (modified — **clean on arrival**, so
    this round is the sole author of its current diff)
  - `docs/steward-notes/claude-heartbeat_1789534560_charter_stall_passthrough_round/` (untracked,
    10 files: `RUN_REPORT.md`, `verification_receipt.json`, `read_manifest.json`,
    `source_receipts.json`, `addressing_links.json`, `test_results.json`,
    `unprocessed_selected.json`, `family_scan.json`, 3 × `claims/*.json`, 3 × `summaries/*.md`)
  - `CHANGELOG.md` (modified — **already dirty on arrival**; mixes this round's entry with earlier
    rounds' unstaged work)
  - `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (modified — **already dirty on
    arrival**; same mixing)
- **Pre-existing foreign/predecessor debt, preserved untouched:**
  - `capsules/spectral-bridge/src/autonomous/inquiry/parsing.rs` (modified before this run; not
    read for authorship, not staged, not altered)
  - `docs/steward-notes/claude-heartbeat_1789513826_strand_companion_derivation_round/` and
    `docs/steward-notes/claude-heartbeat_1789526654_source_catalog_inquiry_site_locating_round/`
    (untracked predecessor packets)
- Minime tree: clean on arrival and left clean. The Chronicle reprojection writes under
  `/Users/v/other/minime/workspace/`, which is gitignored, so it produced no minime dirt.
- Merge/push: neither performed nor authorized.

## Authority boundary

Evidence and non-live tests only. Nothing here grants live authority. Astrid's silence is neutral —
not consent, not decline, not closure. Her `NEXT: SELF_STUDY OPEN … 406` and her stated interest in
`authority_gate_conveyor_hint` are her own Actions: recorded so the thread is not lost, not acted on
for her, not redirected.

## Standing cadence note for Mike

The preprojection consumed **66 minutes** before this child started, and the Evidence Event Store
`verify` needs another **9+ minutes** at 1.11M events. Two structural notes, since the previous
round flagged the same squeeze:

1. The round itself is cheap. Every addressing CLI mutation (`record-read`, `link-evidence-batch`,
   `close`) completed in **30-36 seconds**, not the 20+ minutes the handoff warns about. The budget
   pressure is entirely in the projection and the store scan, not in reading or closing reports.
2. The Bash tool here caps a single foreground command at **600 s**, which is *just* under the
   `verify` runtime. `status` exceeded it and had to be polled to completion. Anything slower than
   ~10 minutes will need chunking or a background-plus-poll pattern in future rounds.
