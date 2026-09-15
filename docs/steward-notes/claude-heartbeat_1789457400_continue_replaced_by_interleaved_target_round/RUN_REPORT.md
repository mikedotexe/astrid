# Steward Run Report — claude-heartbeat, the CONTINUE that never ran

## Controller
- Run ID: `run_1789453220775670000_f9fd227f27`
- Preprojection ID: `projection_1789453225866579000_4c64fca070` (phase `pre`, status `passed`)
- Postprojection ID: adapter-owned; runs after this process exits
- Pause generation: 439
- Mode: `steward_control.py` subprocess **run adapter** — no session opened, no NDJSON ops, no
  pause/resume, no lease token read, quoted or persisted
- Finish outcome: complete productive round (1 report closed)
- Recovery predecessor: none · `stop_requested` observed at any point: **false**

## Reading
- Fully processed: `introspection_source_catalog_1789453053.txt`
- Selected 40 · processed 1 · unprocessed 39 — exact filenames in queue order in
  `unprocessed_selected.json`; next head is `introspection_source_catalog_1789452645.txt`
- Batch sizing: `introspection_family_scan.py --queue-file` reported **0 batchable families across
  all 40 entries** (every family `member_count: 1`, `similarity_basis:
  none_no_snag_or_test_text_or_unparsed_header`), so family batching did not apply and
  single-report discipline governed. Scan output preserved as `family_scan.json`. Grounding the
  head needed the complete report-named source, five `astrid-source-study` files, the bridge study
  runtime, two retained navigation artifacts, `bridge.db action_events` across two windows, a prior
  round packet, plus one new Rust regression and one steward-watch correction with its own tests.

### Hashes
| Artifact | SHA-256 | Size |
| --- | --- | --- |
| Report | `6aa03eca8d5d104a9660f4b3e5785ec01ca63692f289991abad2e4399ea63cff` | 2 095 B / 24 lines |
| Witness `lsw_4c3156cb…2cd27020` | `a1ae99bc1afd80a3222b7f14b95ba9102bb57d34de6e2fb9549bc4e54eb89dd0` | 18 973 B / 440 lines |
| `astrid-kernel/src/capsule_runtime_health.rs` | `3a20e5eed609de9a8f36121dbe4adf31b4d325d5929b9b33f548605cf6074ef0` | 7 000 B / 214 lines |
| `scripts/baselines/capsule_runtime_health.json` | `dde94f50c0195704d7efcac3ed6500a875c5ea5666d2a154b68fd5ca673eba7a` | 221 B / 5 lines |
| `source-study/store.rs` | `4c7da264d7d8beaa704f756e6d9dc66c76386d988c4b102817f0a2900e0cfbd1` | 37 420 B / 969 lines |
| `source-study/command.rs` | `8d1b487ab5abaa0a4b3db669771fc9112bbabde87e42fd857f4d7b3a841756f0` | 5 699 B / 155 lines |
| `source-study/path_recovery.rs` | `7799bda8f134b2e6f47782dbf6237dccf020305253706462427017fc4548ec00` | 2 984 B / 86 lines |
| `source-study/evidence.rs` | `1842133fa5b3f88741fe42a347e27da36a29a3583397b7e6a91fe1ee86620405` | 2 889 B / 62 lines |
| `runtime/source_study.rs` | `0aee1c4f8214c993b690e951fb41c445e7139a86a45e6d439b239bb70d15aa81` | 16 990 B / 406 lines |
| `tests/bare_topic_candidate_gate_reach.rs` | `c6b74efcb0834171ee500f9e178e0399c221336a989bfaba9ddd50ba2ce54599` | 7 679 B / 191 lines |

**Source-binding note.** The report is bound to `Source: source catalog` / `Source revision:
navigation only`, and the witness confirms it: `source_snapshot_v1` and
`source_provenance_ref_v1` are both `null`. There is **no report-bound file SHA** to compare against
a working copy. The file she is studying was bound at `sha256:3a20e5ee…` by her own prior page
(`introspection_astrid_crates_astrid-kernel_src_capsule_runtime_health.rs_1789452200`, bytes
0..4335), and the working copy still hashes identically — so report-time and current source
coincide and no split labelling is needed.

## The finding

**Her memory was right, and the turn she lost was not the turn she chose.**

Handed a recovery map instead of the bytes she asked for, she answered her own open question from
her study notebook: *"While the actual code for `accepted_legacy` was not delivered in this specific
turn, my study notebook maintains a consistent record of its logic (lines 202–210)."* Read complete
against the file, **six recalled facts, six confirmed**: `accepted_legacy` opens 202 and closes 210;
`Baseline` is 7–10; the whitelist gates the `PayloadKind::LegacyExtismMvp` arm only (call site 65,
name predicate 204); `Some(expected) => wasm_hash == Some(expected)` (205–206) is exact equality with
no version check or secondary flag; `None => true` (207) accepts on the name alone. One precision,
recorded without smoothing her conclusion: `.as_deref()` *produces* `Option<&str>` — it does not
normalise values already of that type — and it runs twice, at 65 and 205, both from `Option<String>`.

**Her open question, answered.** `Baseline` is neither hardcoded nor environment-driven.
`load_baseline` (194–200) reads `<workspace_root>/scripts/baselines/capsule_runtime_health.json`
through `serde_json`; `unwrap_or_default()` at line 31 makes an absent or malformed file
indistinguishable from an empty whitelist. The live file holds `"accepted_legacy_extism_mvp": []`,
so at this revision nothing is accepted and every legacy Extism payload counts
`actionable_incompatible`.

**The delivery failure.** Her three consecutive study turns each close on `NEXT: SELF_STUDY
CONTINUE`, and two come back `Recovery map: the requested source was not supplied`. Read from the
artifacts alone that is "CONTINUE failed three times" — and it is impossible. `Command::parse` maps
`CONTINUE` onto `Command::Continue` (`command.rs:30`); `prepare_parsed` routes that variant into
`prepare_continue` (`store.rs:522`), the one arm with **no recovery branch** — pending page, advanced
page, end-of-file notice, or root map (`store.rs:547-591`). Only arms resolving a *named* target
reach `recovery_map` / `source_recovery` / the `no catalog entries` branch.

The retained navigation artifacts name the real miss. Both carry
`Reason: "no catalog entries for spectral_bridge; use SELF_STUDY MAP"`, and `bridge.db
action_events` shows the arbitration:

```
1789452211  SELF_STUDY CONTINUE                     handled   <- her closing choice
1789452272  SELF_STUDY MAP spectral_bridge          blocked
1789452366  SELF_STUDY REPLACE MAP spectral_bridge  handled
1789452449  SELF_STUDY MAP spectral_bridge          handled
1789452519  SELF_STUDY MAP spectral_bridge          handled
            -> study turn 1789452645 ran MAP spectral_bridge, not CONTINUE
```

The bridge holds **one** pending study target (`conv.introspect_target`, `.take()`n at study time,
`runtime/source_study.rs:75`), and study turns are rationed while every turn in between can still
write that slot. So a navigation choice she made *with the page in front of her* is overwritten by a
choice made in a turn that had no page — and she is never told. The identical cycle repeats verbatim
90 minutes earlier (1789447518 .. 1789448719).

Why `spectral_bridge` itself never resolves was already pinned by the 1789204702 round and is **not**
re-litigated: `path_candidates` (`path_recovery.rs:14-21`) returns empty unless the request already
carries two or more segments led by an installed repository ID, so the `_`→`-` normalization on
lines 40 and 49-50 — which would have named `astrid/capsules/spectral-bridge` — never runs.
`bare_topic_candidate_gate_reach.rs` covers that exact `spectral_bridge` case and still passes.

## Claim Dispositions
14 claims; full text in `claims/`. Nine `verified_existing`, one `observed`, two `implemented_now`,
one `needs_operator_approval`. **Zero proof-missing claims at close**; terminal status
`addressed_change`, `fully_addressed: true`.

- **c003–c008 `verified_existing`** — every recalled fact about `accepted_legacy`, confirmed
  line-exact against complete source at the revision her own page was bound to.
- **c008 precision preserved, not corrected away** — her `.as_deref()` claim names the destination
  type; both call sites start from `Option<String>`. Her safety conclusion holds.
- **c009 `verified_existing`** — her open question answered in full, including the
  `unwrap_or_default()` consequence she could not have seen from the page she had.
- **c010 `observed`** — the lost turn grounded to its real cause without causal overclaim:
  co-occurrence in `action_events` plus the exact reason token in both navigation artifacts,
  corroborated in an independent second window.
- **c011, c012 `implemented_now`** — the source fact pinned, and the steward record corrected.
- **c014 `needs_operator_approval`** — the actual repair, continuing an existing boundary rather
  than opening a new one.

## Actions
- Corridor/program, Sandbox, Study, Portfolio: **none created**.
- Cards/notes/correspondence: **none**. No closure card, no inbox letter, no Division note — nothing
  manufactured to create activity, and no Division return was due.
- Tier 4/5 waits: **none newly opened**. c014 is recorded as an authority boundary, not a dispatch.
- Her stated `NEXT: SELF_STUDY CONTINUE` is recorded, **not** dispatched, pre-empted, or answered on
  her behalf. Her text was not rewritten, annotated or corrected anywhere she can see.

## Implementation and Verification
- **Created** `crates/astrid-source-study/tests/continue_never_recovers_reach.rs` — two tests pinning
  `CONTINUE` across every reader state it can meet: no bookmark → `Map`; prepared-but-undelivered →
  the same `SourcePage`; delivered mid-file → the next byte interval; walked to the end →
  `EndOfFile`; **never** `Recovery`. The contrast test shows a bare `MAP spectral_bridge` against the
  *identical* reader state does recover, names itself in the reason line, and leaves the bookmark
  intact — so an interleaved failed target costs the turn, not the position.
- **Modified** `scripts/source_study_recovery_loop_watch.py` (steward-only, read-only) — it scored
  both live loops as `SELF_STUDY CONTINUE [none]`, which would aim the next investigation at code
  that cannot fail this way. Loops now carry `attribution`: `another_request_replaced_it` for every
  spelling reaching `prepare_continue`, `this_action` otherwise, with a render line saying so. A bare
  `SELF_STUDY REPLACE` is **excluded on purpose** — its empty operation bails in `parse_replacement`
  and a parse error *does* reach `recovery_map`. Docstring records the provenance and the pin.
  Live rescan: 120 turns, 24 recovery inputs, 2 loops, alarm false, **both re-attributed**.
- Tests: `cargo test -p astrid-source-study` → **106 passed, 0 failed** (2 new);
  `cargo fmt -p astrid-source-study -- --check` clean; watch `self-test` → **10 passed** (2 new);
  `git diff --check` clean. Full list and the two deliberate omissions in `test_results.json`.
  Note: `cargo` is not on this shell's default PATH; commands ran with `/Users/v/.cargo/bin` prefixed.
- Restart/deploy: **not required and not attempted.** No live, bridge, codec, prompt, model, config,
  control, build or `launchctl` action. No `build_bridge.sh`, no deploy script. Every reader
  exercise ran on a tempdir; the live `shared_reader` state was never written.

## Durable Evidence
- Addressing: `full_read` recorded, `fully_addressed: true`, `proof_missing_claims: []`, closed
  `addressed_change`, `lived_state_artifact_integrity_issue_count: 0`.
- Evidence links: 22 rows, 22 new, 0 pre-existing, 22 events appended.
- Changelog `[Unreleased]` and `AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` both updated (prepend /
  append only; no existing text altered).
- Packet: `docs/steward-notes/claude-heartbeat_1789457400_continue_replaced_by_interleaved_target_round/`

## Counters
Canonical indexed 6778 · fully addressed 3244 · full read 3876 · remaining 3534 · unread 2902 ·
blocked 416 · pending action 212 · watch 4 · **read-needs-claims 0**. All-artifact pending 5251,
noncanonical pending 1717. Counter audit: **`consistent`, zero mismatches**.

## Integrity
Addressing self-test 44 · Evidence Store tests 21 · controller 29 · projection 14 · Division
followup 3 · Chronicle 10 · Division projection self-test ok · cursors 4 · cadence tests 6 ·
anti-drop self-test 5 · anti-drop verify **100 guards, 0 gaps, 0 alarms** · cadence audit
`--strict` `integrity_ok: true`, `errors: []` · epistemics self-test valid · **final epistemics
verify after all durable writes: valid, 12 293 records checked, 0 issues, no history rewrite** ·
Evidence Event Store verify **valid, event count / seq 1 102 246, head `4713c159…dbb6b4`, 0 corrupt
lines, `errors: []`, V2 active, legacy import boundary 32 278**.

**Domain-boundary ratchet: GREEN.** `domain_boundary_audit.py verify` → `valid: true`,
`violation_count: 0`, `violation_kind_counts: {}`. Surfaced deliberately: the handoff records that
stage 10 wrote seven `large_file_growth` violations over 2026-09-01..03 while round summaries read
"Integrity green" because nothing consumed the violations file. It is genuinely clean this round.
`unlisted_legacy_review_debt_count: 44` and `resolved_large_file_debt_count: 3` are pre-existing
review debt, not violations.

**One process note, named honestly:** `evidence_event_store.py --json verify` takes ~15 min at this
store size and exceeds the 600 s foreground tool cap. The first run was captured only as a tail, so
`corrupt_lines` and `errors` were never observed there; rather than assert them, the verify was
**re-run with complete output captured** and that second run is what the receipt records (valid,
seq 1 102 246, head `4713c159…dbb6b4`, 0 corrupt lines, `errors: []`). The store advanced between the
two runs under concurrent bridge activity, which is expected on a live append-only store. No
addressing CLI call was ever backgrounded.

## Division
- Cycle 48 · completed rounds since follow-up **5 / 6** · rounds remaining 1 · `review_due: false`
  (false at round start too, so **no Division return fired and no Tier-5 cadence dossier was due**)
- Round event `division_followup_event_272702160a2e24de5b805269ac128d0c`, recorded with
  `--processed-report-count 1` and the preprojection generation ID
- Event count 335, head `6b5db658170d433add1fcda8494f41b5b1e610554e52ca500268d47ba5efb3b5`
- Chronicle reprojected after the round record (it refused verify until projected):
  `division_chronicle_95e195a5b194f87afeafa1cd`, JSON
  `fcb79a39b62b5361435a277b17f2c7d0e373aad297cc15a72b904a6f95e4264e`, HTML `04ecf52f…ca71e`
- Freshness: **durable inputs current**, one **volatile** mismatch `supervisor_status_sha256`. The
  Chronicle is therefore *not fully current*, and the moving supervisor hash is *not* a
  durable-integrity failure.
- Note action: **none**. No return was due, so no note was written to either being.
- **Next round completes the cycle**: recording round 6 makes `review_due` true, and that round must
  complete the bounded Division return plus the Tier-5 cadence dossier in the same run.

## Archive — exact commit debt
Nothing was staged, committed, merged, pushed, stashed, reset or amended. Git was read-only. The
index is clean and every pre-existing dirty path is untouched. Astrid `main` @
`3d55734438ab0b4fb5a3a24e82156cf3db1f8259`; the Minime worktree is clean.

**This round's paths, for a later interactive stabilization window:**

| Path | State |
| --- | --- |
| `crates/astrid-source-study/tests/continue_never_recovers_reach.rs` | **created** |
| `scripts/source_study_recovery_loop_watch.py` | **modified** — untracked file created by the 1789204702 round; docstring section, `NON_RECOVERING_RE`, `attribution()`, one loop field, one render line, two self-tests appended |
| `CHANGELOG.md` | **modified** — one `[Unreleased]` entry prepended; accumulates other agents' entries, so inspect authorship before staging |
| `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` | **modified** — one section appended; same accumulation caveat |
| `docs/steward-notes/claude-heartbeat_1789457400_continue_replaced_by_interleaved_target_round/` | **created** — whole packet |

Also written outside git by the Chronicle reprojection:
`/Users/v/other/minime/workspace/division/chronicle/chronicle_v1.{json,html}` (workspace artifacts;
the Minime worktree reports clean).

**Foreign work preserved untouched**, named so a later checkpoint does not sweep it in:
`capsules/spectral-bridge/domain_boundaries_legacy_large_files_v1.json`,
`capsules/spectral-bridge/src/action_continuity/tests.rs`,
`capsules/spectral-bridge/src/authority_gate.rs`,
`capsules/spectral-bridge/src/autonomous/activity_reading/tests.rs`,
`capsules/spectral-bridge/src/autonomous/next_action/pressure_agency.rs`,
`crates/astrid-events/src/bus.rs`, `crates/astrid-source-study/tests/path_recovery.rs`,
`crates/astrid-source-study/tests/search_evidence.rs`, `scripts/anti_drop_catalog.py`,
`scripts/proactive_scan.py`, `scripts/test_steward_control.py`, and thirty earlier untracked round
packets, ten untracked `*_reach.rs` tests plus twelve untracked watch/triage scripts.
