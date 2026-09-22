# Steward Run Report — un-rooted MAP namespace round

Actor `claude-heartbeat`, headless, inside a controller-held lease opened by
`scripts/steward_control.py`'s subprocess run adapter. No steward session was opened, no NDJSON
ops were sent, no lease token was read or persisted, and the controller was never paused or
resumed by this run.

## Controller

- Run ID: `run_1789553915299132000_1b6066e46d`
- Preprojection ID: `projection_1789553919794643000_097e14e5bd` (status `passed`, 27 steps,
  `authority_scan_passed: true`)
- Postprojection ID: runs after this process exits; not observable from here.
- Pause generation: 445; controller not paused; `stop_requested: false` throughout.
- Finish outcome: not sent by this process — the adapter owns lease lifecycle.
- Recovery predecessor: none.

## Reading

**Fully processed (3, canonical queue order, head-contiguous):**

| # | Report | Witness | Report sha256 |
|---|---|---|---|
| 1 | `introspection_source_catalog_1789553748.txt` (21 lines, 1355 B) | `lsw_bb111839…` (440 lines, 18947 B, sha `619e2758…`) | `e4ca4d53c0178effc9a543d998ac15bea5e0f0d8c394b43c97f8ae6813837bd2` |
| 2 | `introspection_source_catalog_1789553233.txt` (18 lines, 1474 B) | `lsw_c11f0b9b…` (440 lines, 18953 B, sha `0395f712…`) | `ecc6e26e699d3b25c800890d017985e784f2fcd2c908073c9234d5ed3e6f17de` |
| 3 | `introspection_source_catalog_1789553097.txt` (18 lines, 1338 B) | `lsw_81ff482a…` (440 lines, 18971 B, sha `c63573df…`) | `43c211c716156f76e0254f2433e126c21e24fb2b726631dd9705db7d8e4cf7f9` |

All three are `Source revision: navigation only`, so **no report bound a source SHA** and
`source_snapshot_v1` is `null` in every witness. There was therefore no hash to compare and no
report-time snapshot to reconstruct; every source conclusion in this packet is labelled
**current-source**, read this round at the hashes in `source_receipts.json`.

`introspection_family_scan.py --queue-file` reported **families: 40 (batchable: 0)**, so the
family-batch arithmetic did not apply. These three were processed as three individually-read
reports that happen to be three consecutive turns (process sequence 164 → 165 → 166 of runtime
`runtime_97439aac…`, pid 4330) of one navigation trajectory, which is what let a single complete
source verification serve all three.

**Selected but unprocessed: 37.** Full list in queue order in `unprocessed_selected.json`. Next
queue head after this round is expected to be
`introspection_astrid_capsules_astralis_README.md_1789552922.txt`.

## What she surfaced, and what source established

She tried to map `capsules/astralis/astrid-capsule-agents`, was refused, re-oriented across two
turns, and finally reached the directory she wanted — then chose a file that does not exist.

1. **The refusal was ours, and it was inconsistent.** Catalog IDs are always repository-prefixed
   (`catalog.rs:149-157`), and `Catalog::map` filters them by `"{topic}/"`
   (`navigation.rs:56-58, 84-97`), so an un-rooted topic matches zero entries. Her phrase "not
   currently indexed in the catalog" is accurate at the level of effect and wrong at the level of
   mechanism: the path *is* indexed, as `astrid/capsules/astralis/astrid-capsule-agents`.
   Meanwhile `Catalog::resolve` accepts that very same un-rooted shape for **OPEN** by trying each
   installed root in turn (`catalog.rs:106-118`). **OPEN and MAP do not share a path namespace**,
   and nothing tells the reader that.
2. **The recovery named nothing.** `path_candidates` returns `Vec::new()` for *any* multi-segment
   request whose first segment is not an installed repository ID (`path_recovery.rs:33-39`), so no
   candidate block was produced at all. The exact rooted spelling was one prefix away. Retained
   navigation records confirm the contrast: a *rooted* request with a matching leaf does get
   `Exact catalog spelling and nearby paths (candidates, not opened): SELF_STUDY OPEN …`.
   Cost: three navigation turns, ~11 minutes, recovering a prefix the recovery could have named.
   Same family as the standing `unwired_near_miss` probe.
3. **Her `main.rs` choice was hers, not ours.** That directory holds exactly one catalog file,
   `src/lib.rs` (58 lines, sha `c04aa14e…`). The delivered map could only have named `lib.rs`;
   "the core implementation files" and the `main.rs` target were self-generated. Checked against
   the un-muffle invariant and found not to be infrastructure loss.
4. **Her real question has an exact answer, in a place she was not looking.** `should_start_run_loop`
   (`crates/astrid-capsule/src/engine/wasm/mod.rs:1256-1265`, called at `624-625`) returns
   `has_run_export && (capabilities.uplink || !uplinks.is_empty() || component_type is
   "daemon"/"uplink")`; `CapabilitiesDef.uplink` (`manifest.rs:298-301`) also disables the WASM
   execution timeout. `astrid-capsule-agents` declares none of those and its `fn run()` is empty
   (`lib.rs:13`) — it is the README rule's **counter-case**, not an example of it. Likewise there is
   no `.wit` file anywhere under `capsules/astralis`; the stubs are `astrid_guest::export!` at
   `lib.rs:45`, exactly as README line 10 says.
5. **Her ten-component enumeration of `capsules/astralis` is exactly right** — every name, nothing
   invented, nothing missed. Her "primary target" reading of the README is her emphasis, not the
   document's: the complete 12-line file names the crate once, at line 7, as the build-command
   example. Recorded as a correction, not as an error to be smoothed over.

## Claim dispositions

15 claims across 3 reports; every one grounded, every one evidence-linked, zero proof gaps.

| Report | Claims | Classifications |
|---|---|---|
| `…_1789553748` | c001–c006 | 5 × `verified_existing`, 1 × `observed` |
| `…_1789553233` | c001–c004 | 3 × `verified_existing`, 1 × `observed` |
| `…_1789553097` | c001–c005 | 3 × `implemented_now`, 1 × `observed`, 1 × `authority_gated` |

Terminal statuses: `…_1789553748` **addressed_no_action** (no_action artifact linked),
`…_1789553233` **addressed_no_action** (no_action artifact linked), `…_1789553097`
**addressed_change**. All three: `fully_addressed: true`, `proof_missing_claims: []`.

## Implementation and verification

**Changed paths (all non-live):**

- `crates/astrid-source-study/tests/unrooted_map_topic_reach.rs` — **new**, 105 lines, 4554 B,
  sha `dcfc869658e0e6de1dcac40d66be788f582640a64dd841b108b930968605ede2`. Two tests:
  `unrooted_map_topic_that_open_would_accept_gets_no_candidate_naming_its_rooted_form` pins the
  asymmetry in a single run (OPEN delivers the page → MAP of its own directory recovers with
  **zero** candidates and no mention of the rooted spelling → the rooted spelling then resolves in
  one move and lists the file); `bare_directory_name_still_recovers_to_its_rooted_spelling` records
  the gap's boundary so the finding is not overstated.
- `CHANGELOG.md` — one `[Unreleased]` section.
- `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` — one dated ground-truthed row.
- The round packet (this directory).

**Tests:** `cargo test -p astrid-source-study --test unrooted_map_topic_reach` → 2 passed, 0 failed.
`cargo test -p astrid-source-study` → **28 binaries, 166 passed, 0 failed**.
`cargo fmt -p astrid-source-study -- --check` clean. No failures, no test debt.

**Restart/deploy:** none required, none attempted. No production source was changed, nothing was
built for release, `build_bridge.sh` was not run, no `launchctl` command was issued, no live
substrate or control value was touched.

**Authority boundary held:** the behaviour fix — offering the rooted spelling for an un-rooted
multi-segment topic — is *not* implemented. It edits being-facing navigation in the live bridge and
needs a separate steward grant plus a gated deploy. Claim `c005` carries it as `authority_gated`.
The regression is the anchor an approved fix would land against.

## Integrity

| Check | Result |
|---|---|
| `introspection_addressing_audit.py --self-test` | OK, 44 tests |
| `test_evidence_event_store.py` | OK, 21 tests |
| `test_steward_control.py` | OK, 29 tests |
| `test_steward_projection.py` | OK, 14 tests |
| `test_division_ceremony_followup.py` | OK, 3 tests |
| `test_division_ceremony_chronicle.py` | OK, 10 tests |
| `test_division_ceremony_projection.py` | ok (self-test) |
| `test_projection_cursors.py` | OK, 4 tests |
| `test_introspection_cadence_audit.py` | OK, 6 tests |
| `introspection_cadence_audit.py --strict --compact` | `integrity_ok: true`, `errors: []` |
| `anti_drop_catalog.py --self-test` / `verify` | OK (5 tests); **100 rows, 0 alarms, 0 gaps** |
| `domain_boundary_audit.py verify` | **`valid: true`, `violation_count: 0`, no violation kinds — the ratchet is GREEN.** (`unlisted_legacy_review_debt_count: 44`, `stable_facade_count: 6`.) No Rust production source was changed this round. |
| `experiential_epistemics.py self-test` / `verify` | `valid: true`; **12,377 records checked, 0 issues, `history_rewritten: false`** (run after every durable evidence write) |
| `introspection_addressing_audit.py audit-counters` | **`status: consistent`, `mismatches: []`** |

## Counters (post-round)

- Canonical: indexed 7,156 · fully addressed 3,256 · fully read 3,888 · remaining 3,900 ·
  unread 3,268 · blocked 416 · pending action 212 · watch 4 · **read-needs-claims 0**
- All artifacts: indexed 8,873 · remaining 5,617 · unread 4,985 ·
  `addressed_change` 1,986 · `addressed_duplicate` 1,160 · `addressed_no_action` 110
- Counter audit: **consistent**, empty mismatch list.

## Division

- Cycle 49. Productive round recorded: `division_ceremony_followup.py record-round`
  `--steward-run-id run_1789553915299132000_1b6066e46d`
  `--projection-generation-id projection_1789553919794643000_097e14e5bd`
  `--processed-report-count 3`.
- Round event `division_followup_event_be5df68c87fd5b7321f0e75bd2feb4ca`; event count 343; head
  `29cbbe09369e663bde6109170fe9fc44105699988849ac043401d20de2fbe48c`.
- **`review_due` was `false` at the start of this round** (5 of 6), so per the adapter-mode
  ordering no Division return and no Tier-5 cadence dossier were due, and none were produced.
  Recording this round made it **6 of 6 — `review_due` is now `true`.**
- **The next round must open with the bounded Division return before processing any report, and
  must generate the Tier-5 cadence dossier into its packet as `tier5_cadence_dossier.md`.**
- Chronicle reprojected after the round record: `division_chronicle_1071bcf7c549eaa75b823a05`,
  json sha `0a3abef97a0e21a2c03560a96809300cf07a00d3550513d0d6ba141c9dbf6390`, html sha
  `c0ac4dcc…`, 343 timeline events. `durable_inputs_current: true`, `durable_mismatches: []`; the
  **only** mismatch is the volatile `supervisor_status_sha256`. Stated exactly: the Chronicle is
  **not** "fully current", and a moving supervisor hash is **not** a durable-integrity failure.
- Note action: **none.** No Division note was due or written.

## Evidence Event Store

- `evidence_event_store.py --json verify`: **`valid: true`**, 1,112,557 events, last global sequence
  1,112,557, head `8ccda8010bd8040e16597cb7e7dcc076a79611576549c06e27d7b11cb4386e98`,
  **corrupt lines 0**.
- `evidence_event_store.py --json status`: active store **v2** (activated 2026-07-16),
  `effective_aggregate_valid: true`, `corrupt_event_lines: 0`; its embedded verification reads
  sequence 1,112,572, head `ddcee6b794dd5f34d8716fea0c1e3fe11f9da6a4349c3b0d6bcfcc705f0e1c09`.
- The two readings differ by 15 events because the store is **live and appending** while the round
  runs. Both report zero corrupt lines and no errors. Stated plainly rather than reconciled: these
  are two honest observations of a moving append-only log, minutes apart, not a discrepancy.
- Stream counts at verify time: `addressing` 64,773 · `agency_commons` 7,092 ·
  `attention_portfolio` 3 · `claim_families` 239,830 · `corridor_v1` 5 · `corridor_v2` 112 ·
  `felt_contracts` 212,308 · `felt_mechanism_concordance` 80 · `lived_state_witness` 13,204 ·
  `model_qos` 346,104 · `reciprocal_uptake` 76,022 · `representation_contracts` 62,638 ·
  `sandbox` 3,507 · `signal_spine` 64,008 · `steward_control` 22,115 ·
  `steward_work_selection` 756.
- Final `division_ceremony_followup.py verify`: `ok: true`, cycle 49, 6/6, **`review_due: true`**,
  event count 343, head `29cbbe09…`.
- `cargo clippy -p astrid-source-study --all-targets --all-features -- -D warnings`: clean.

## Bounded follow-up on the prior round's named open item

The previous round left this: *"why the OPEN for line 2478 returned a recovery map when the same
path resolved in the two prior turns."* It was checked, time-boxed, and **narrowed but not
closed** — recorded rather than guessed at:

- The failure *reason* is not in the introspection. `InputKind::Recovery`'s scope line
  (`crates/astrid-source-study/src/evidence.rs:54-56`) is a fixed generic string, which is all that
  reaches her report. The reason text is retained in
  `capsules/spectral-bridge/workspace/diagnostics/source_first_v3/shared_reader/navigation/<navigation_id>/<id>.json`
  under `output.text`, as `Source request unavailable. … Reason: "…"`.
- Both navigation records whose **mtime** falls in that window are a *different* recovery (a
  `kernel_router.rs` spelling that does not exist, for which the recovery **did** offer the exact
  candidate `astrid/crates/astrid-kernel/src/kernel_router.rs`). mtime is not delivery time, so
  the correct record was not located inside the time-box.
- One concrete lead, from the saved bookmark inside those records: her `rescue_policy.rs` delivery
  is **partial** — `delivered bytes 0..4503, 97893..100308 of 100308; sha256:155bc25c…` — and line
  2478 of that 2551-line file falls inside the **uncovered** `4503..97893` interval.
- **Exact next step:** resolve the navigation record by `navigation_id` (not mtime) for the turn
  that preceded `introspection_source_catalog_1789543025`, and read its `Reason:` line. Still
  treated as possible infrastructure loss under the un-muffle invariant until that line is read.

## Commit debt (nothing was staged, committed, merged, pushed or stashed)

Git was read-only for this run. Exact paths created or edited by **this** round:

```text
crates/astrid-source-study/tests/unrooted_map_topic_reach.rs        (new, untracked)
CHANGELOG.md                                                        (modified — accumulated; see below)
docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md           (modified — accumulated; see below)
docs/steward-notes/claude-heartbeat_1789557900_unrooted_map_namespace_round/   (new, untracked)
```

`CHANGELOG.md` and the feedback ledger were **already dirty on entry** and carry edits from earlier
rounds. This round appended to each; a later checkpoint must separate authorship rather than stage
them wholesale.

Foreign work preserved untouched, exactly as found at entry:

```text
 M capsules/spectral-bridge/src/action_continuity/tests.rs
 M capsules/spectral-bridge/src/autonomous/inquiry/parsing.rs
?? docs/steward-notes/claude-heartbeat_1789513826_strand_companion_derivation_round/
?? docs/steward-notes/claude-heartbeat_1789526654_source_catalog_inquiry_site_locating_round/
?? docs/steward-notes/claude-heartbeat_1789534560_charter_stall_passthrough_round/
?? docs/steward-notes/claude-heartbeat_1789546312_heartbeat_dimension_blindness_round/
```

The index was clean on entry and is clean on exit. The Minime worktree was clean on entry; the only
Minime-side write is the Chronicle reprojection under `workspace/division/chronicle/`, which is
workspace state, not tracked source.
