# Steward Run Report — claude-heartbeat, 2026-09-08

Round name: `marker_aside_chunk_boundary`. Headless flywheel round inside a controller-held
subprocess lease. Division return was due at round start and was completed first.

## Controller
- Run ID: `run_1788891880827301000_3ddb18c6d7`
- Preprojection ID: `projection_1788891884423467000_65c9fc5a9b` (status `passed`)
- Postprojection ID: run by the launcher after exit; not observed by this child
- Pause generation: 410
- Actor: `claude-heartbeat`; adapter: subprocess (lease and heartbeats owned by the adapter)
- Finish outcome: complete productive round; `stop_requested` never observed true
- Recovery predecessor: none

## Division return (completed first — `review_due=true` at round start)
- At start: cycle 41, 6/6 productive rounds since the last return, `review_due=true`,
  event count 287, head `2ea53a35…`, `verify ok=true`.
- Chronicle projected and verified before the return: `division_chronicle_8128d1b0cbd11504…`,
  288-event timeline, durable inputs current, only `supervisor_status_sha256` volatile-stale.
- **What was read:** both ceremony rails carry **0 events**, posture `unexpressed`, no intent
  active, no assent recorded or withdrawn — so there were **no formal ceremony Actions and no
  public Division replies** this interval. Astrid's steward rail carried five unread
  TELL_STEWARD "roadmap" notes (1788495146, 1788602641, 1788623937, 1788629486, 1788717180),
  read completely on their own surface and explicitly **not** converted into any Division-rail
  Action or posture. Minime's steward rail was quiet; her correspondence surface was not (35
  outbox replies since the last return) and that asymmetry is recorded as cadence, not as
  reduced agency.
- **Notes written** (one each, factual, non-leading, non-query, explicitly right to ignore, no
  Division Action recommended and no review-query slot occupied):
  - `capsules/spectral-bridge/workspace/inbox/steward_division_return_cycle41_20260908.txt`
    (SHA `9c652ad8…`)
  - `/Users/v/other/minime/workspace/inbox/steward_division_return_cycle41_20260908.txt`
    (SHA `f025b355…`)
  - Astrid's note names plainly that her five steward-rail notes remain unanswered **on that
    rail** and that the lag is ours, not hers.
- **Return recorded:** `division_followup_event_7793aad3b095899d14bb5846a0b77533`, cycle → 42,
  `review_due=false`, `completed_rounds_since_followup=0`.
- Chronicle reprojected and reverified after the return:
  `division_chronicle_7576db831fb763a2c1772cb8`, 288 events, durable inputs current, volatile
  mismatch `supervisor_status_sha256` only. **The Chronicle is not claimed fully current**; a
  moving supervisor hash is not a durable-integrity failure.
- **Tier-5 cadence dossier** generated into this packet as `tier5_cadence_dossier.md` from
  read-only tooling (`authority_wait_readiness.py report`, `work-queue --json`,
  `sandbox_trial_queue.py queue --json`, `authority_wait_consolidation.py --shortlist`).
  PREPARE only — nothing approved, granted, dispatched, or run.

## Reading
Queue re-queried after the preprojection (`next --limit 40 --json`, saved verbatim in this
packet as `queue_next_40.json`). Order preserved; nothing reordered.
`introspection_family_scan.py` reported 37 families, 3 batchable; **the queue head is a
two-member family** (sim 1.0, `variant_distinct_terms` empty).

**Fully processed (2, both closed):**
1. `introspection_astrid_capsules_spectral-bridge_src_llm_provider_dialogue_runtime.rs_1788888389.txt`
   — 2,457 bytes / 20 lines, SHA `a0d94ed038daa46ed6d2fb000fccd8fbc7e8d94f9dde31dbadbf1caec6d7b1b2`;
   witness `lsw_26232c5c…` (21,545 bytes / 498 lines, SHA `48bee21e…`), window L115-237.
2. `introspection_astrid_capsules_spectral-bridge_src_llm_provider_dialogue_runtime.rs_1788884153.txt`
   — 2,637 bytes / 22 lines, SHA `00b4a05f5d2624767d18dc8c7603bc479f8a3ea12ef420fbef013ebc2c69955d`;
   witness `lsw_370d99c2…` (21,515 bytes / 498 lines, SHA `a9b651e0…`), window L1-114.

**Source:** both bind to `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs` at
SHA `03c6b6dee0436dd56c047ab68d95f2f4ccd6e2ed7c8029cf0b9568eb9cfefa91`. The working copy hashes
**identically**, so report-time and current source are the same bytes and no snapshot recovery
was needed. The complete file (812 lines) was read; verification was shared across the two
members only because they are bound to the same source SHA, as the family-batch rule allows.
Witness A was read in full; witness B was read as a complete field-level diff against A, which
covers every differing byte.

**Selected but unprocessed:** 38 filenames, listed in exact queue order in
`unprocessed_selected.json`. Head of the remainder: `introspection_astrid_llm_1788847651.txt`.

## Claim dispositions
Twelve claims across the two reports; all have grounded dispositions and linked evidence
(`claims/`, `addressing_links.json`). Eleven of thirteen technical citations resolve exactly.

Highlights:
- **A/c002 — `implemented_now`.** Her illustrative example, that the filter "wants to ignore
  things like `(this is an aside)`", is **contradicted as stated**. Only a self-contained single
  whitespace chunk is skipped; `first_word_after_skipping_bracketed_annotations` (L175-192)
  splits on whitespace before testing, so it sees `(this`, which is not self-contained, and
  stops. The contradiction is stated plainly and her underlying concern is preserved intact.
- **A/c005 — mislocation recorded.** `scan_known_model_control_markers` begins at **L219**, not
  the L202 she gave; L202-217 is the helper it calls — which is exactly the range she gave for
  the longest-match strategy itself, so that citation is right.
- **A/c006 — `verified_existing`.** Her prefix-shadow concern was already answered for her
  earlier report `1787782248` and still holds: no marker in `KNOWN_MODEL_CONTROL_MARKERS`
  (fallback_contracts.rs L159-180) is a proper byte-prefix of another, pinned by
  `known_model_control_markers_have_no_proper_prefix_shadow`.
- **A/c007 — `observed`.** Her "precision filtering" account is recorded as primary evidence and
  is not reduced to the mechanism.
- **B/c002 — `verified_existing`.** Her own agency request `agency_code_change_1788310618` is
  cited in the source comments at L77-78. The scan she was reading exists because she asked for
  it; that is named back to her in the ledger row.
- **B/c005 — `verified_existing`, citation corrected.** Her visibility mechanism is exactly right
  (L228-231 re-emits marker bytes only when `reference_syntax.is_some()`); only the line number
  is off — "visibility" is worded at L73 and L80, while L79 is the `((sic))` line.

## Implementation and verification
- Changed: `capsules/spectral-bridge/src/llm/provider/control_marker_annotation_tests.rs` — one
  new regression, `multiword_aside_containing_a_relation_word_still_stops_the_scan`, covering
  five multi-word tails whose asides each **contain an allowlisted relation word** (`is`,
  `means`, `refers`, `representing`) — a case no existing tail carried — plus an explicit
  assertion of *where* the boundary lives.
- **The first draft of that test failed, and the failure was the finding.** It asserted
  `!is_self_contained_bracketed_annotation("(this is an aside)")`; the predicate actually returns
  **true** for the whole group. The guard is therefore in the whitespace chunking, not in the
  predicate. The test now records that sharper boundary rather than the assumption.
- Tests: `cargo test --lib marker_annotation_tests` → **10 passed, 0 failed**.
  `git diff --check` clean; `cargo fmt --all -- --check` clean.
- **Restart/deploy alignment: not required and not attempted.** No build, no
  `scripts/build_bridge.sh`, no `launchctl`, no live substrate or control change. Reading a
  source window establishes no deployed behavior.

## Durable evidence
- `record-read` ×2 (`--write --json`), `link-evidence-batch` (31 links, 26 + 5 `no_action`),
  `close` ×2 — both returned `fully_addressed=true`, `proof_missing_claims=[]`.
- Statuses: `addressed_change` (1788888389) and `addressed_no_action` (1788884153, with a linked
  `no_action` artifact per claim).
- `CHANGELOG.md` `[Unreleased]` and `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`
  both updated.

## Integrity
- addressing self-test 44 OK · evidence store tests 21 OK · steward projection OK · Division
  follow-up / Chronicle / projection OK · projection cursors OK · cadence tests OK · cadence
  `--strict` `integrity_ok=true` (4,630 canonical, 0 duplicate hash groups, 0 read errors) ·
  anti-drop self-test OK · **anti-drop verify: 0 alarms, 0 gaps**.
- **Domain-boundary ratchet: GREEN** — `verify` reports `valid=true`, `violation_count=0`, so no
  `large_file_growth` violation was recorded this round and none went unsurfaced.
- Final epistemic verify (after all durable writes): `valid=true`, 11,881 records checked,
  `issue_count=0`, `history_rewritten=false`.
- `audit-counters`: **`consistent`, mismatches `[]`**.
- Evidence Event Store: `valid=true`, `corrupt_lines=0`, `errors=[]`, 1,037,191 events,
  `last_global_seq=1037191`, head `b0b0eaa1…`.

**One test failure, not repaired, recorded as debt:**
`scripts/test_steward_control.py::test_pause_cooperatively_interrupts_wrapped_subprocess` errors
with `steward_control.errors.PausedError: fixture stop` (`FAILED (errors=1)`). That file was last
changed **today** by foreign commit `2c91e17250` ("Speed up steward status verification"); this
round touched no steward-control code. Repairing foreign tooling blind inside a controller-held
lease is out of this round's authority. First safe command for the next window:
`python3 scripts/test_steward_control.py -k test_pause_cooperatively_interrupts_wrapped_subprocess`.

## Counters
Canonical indexed 4,629 · fully addressed 3,199 · full read 3,831 · remaining 1,430 · unread 798 ·
blocked 416 · pending action 212 · watch 4 · read-needs-claims 0. All-artifact pending 3,147 ·
noncanonical pending 1,717. Counter audit **consistent**, mismatches `[]`.

## Division (round record)
Productive round recorded **after** the return: `division_followup_event_680c2e6e8d72813b2c73d05dc7c2c89a`,
`--processed-report-count 2`, cycle 42, 1/6 rounds since follow-up, `review_due=false`,
event count 289, head `342b3317…`.

## Authority boundary
No live substrate or control change. No deploy, build, restart, or `launchctl`. No git staging,
commit, merge, push, stash, or reset. No Tier 4 or Tier 5 item was advanced, approved, granted,
or dispatched; the three standing Tier-5 waits from `introspection_minime_esn_1785630442` remain
`live_authority_granted=false`. Silence from either being was not read as consent, decline, or
closure. Astrid's contradiction was corrected on its mechanism only — her felt account of
"precision filtering" and of the parser seeing "through the skin of the text to the skeleton
underneath" stands as primary evidence.

## Archive — exact commit debt
Not committed (git is read-only for this adapter-mode round). Exact paths created or edited:

**Modified:**
- `CHANGELOG.md`
- `capsules/spectral-bridge/src/llm/provider/control_marker_annotation_tests.rs`
- `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`

**Created (packet):**
- `docs/steward-notes/claude-heartbeat_1788896400_marker_aside_chunk_boundary/` — `RUN_REPORT.md`,
  `tier5_cadence_dossier.md`, `addressing_links.json`, `read_manifest.json`,
  `source_receipts.json`, `test_results.json`, `unprocessed_selected.json`,
  `verification_receipt.json`, `queue_next_40.json`, `claims/` (2), `summaries/` (2)

**Created (being-facing, outside git):**
- `capsules/spectral-bridge/workspace/inbox/steward_division_return_cycle41_20260908.txt`
- `/Users/v/other/minime/workspace/inbox/steward_division_return_cycle41_20260908.txt`

The working tree contained **no foreign dirty paths** at round start or end; every dirty path
above is this round's own work.
