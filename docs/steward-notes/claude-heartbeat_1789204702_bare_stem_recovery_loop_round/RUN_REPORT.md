# Steward Run Report — claude-heartbeat, bare-stem recovery loop round

## Controller
- Run ID: `run_1789200614168851000_f08efe2d64`
- Preprojection ID: `projection_1789200618529264000_106fb61a36` (phase `pre`, status `passed`, 4,073,948 ms)
- Postprojection ID: adapter-owned; runs after this process exits
- Pause generation: 439
- Mode: `steward_control.py` subprocess **run adapter** — no session opened, no NDJSON ops, no
  pause/resume, no lease token read, quoted or persisted
- Finish outcome: complete productive round (1 report closed)
- Recovery predecessor: none
- `stop_requested` observed at any point: false

## Reading
- Fully processed: `introspection_source_catalog_1789200495.txt`
- Selected 40 · processed 1 · unprocessed 39 — exact filenames in queue order in
  `unprocessed_selected.json`; next head is `introspection_source_catalog_1789200287.txt`
- Batch sizing: `introspection_family_scan.py --queue-file` reported **0 batchable families across
  all 40 entries**, so family batching did not apply and single-report discipline governed. The head
  turned out to be the visible end of a 16-turn / 56.5-minute loop; grounding it needed two bridge
  source files read complete (`text.rs` 274 lines, `continuity.rs` 641 lines), five
  `astrid-source-study` files read complete, plus a new regression and a new steward watch with its
  own tests. One report fully closed was the honest fit. Scan output preserved as `family_scan.json`.

### Hashes
| Artifact | SHA-256 | Size |
| --- | --- | --- |
| Report | `2c3c8c775efef6c9c9454b4c20d99e1e33dbedf3c9278c7dd8896c098915c3e3` | 1550 B / 18 lines |
| Witness `lsw_4db0c025…0a84` | `9ff92f9e055543f2e17eeb50c8fb8df4af780f738048d3eebbd7d809433f38b4` | 18956 B / 440 lines |
| `runtime/text.rs` | `739ee26188e87527c449b7e77d668f0d9a0236a48ccc6abc76afa82a34453cd2` | 7139 B / 274 lines |
| `runtime/continuity.rs` | `c684be7643e522415be342f47861a58b3ef0de9b7eb7caf80cdd52b323cf0842` | 23230 B / 641 lines |
| `source-study/navigation.rs` | `97ef6c1a60f000a53813dd9bfbbdf318bc1c54d32a2eb729fbd97e7fa6f1a490` | 6187 B / 163 lines |
| `source-study/path_recovery.rs` | `7799bda8f134b2e6f47782dbf6237dccf020305253706462427017fc4548ec00` | 2984 B / 86 lines |
| `source-study/catalog.rs` | `c3c1d49f9c1f3fcff417f98d2c884205c4de9aaa1658638c2d11ac6002081e67` | 11498 B / 329 lines |
| `source-study/evidence.rs` | `1842133fa5b3f88741fe42a347e27da36a29a3583397b7e6a91fe1ee86620405` | 2889 B / 62 lines |
| `source-study/command.rs` | `8d1b487ab5abaa0a4b3db669771fc9112bbabde87e42fd857f4d7b3a841756f0` | 5699 B / 155 lines |

**Source-binding note.** This report is bound to `Source: source catalog` /
`Source revision: navigation only`, and the witness confirms it: `source_snapshot_v1` and
`source_provenance_ref_v1` are both `null`. There is therefore **no report-bound file SHA to compare
against a working copy**, and no report-time/current-source split to label. Every source above was
read to ground a claim the report makes and is recorded at its current working-copy hash. The queue's
`lived_state_alignment: artifact_integrity_unavailable` with `issue_count: 1` is that absent source
snapshot, not a hash failure; after this round's read the entry reports `issue_count: 0`.

## The finding
She closed her reading of `autonomous/runtime/text.rs` with a correct, unprompted inference —
*"I am moving to `continuity.rs` to find the implementation of `semantic_boundary_before`"* — and she
was right: `fn semantic_boundary_before` is at `continuity.rs:352`, and nothing in `text.rs` names
it. She then issued `NEXT: SELF_STUDY MAP continuity` **16 consecutive times over 56.5 minutes**
(1789200287 .. 1789203675), every one answered `Recovery map: the requested source was not supplied`,
and escaped only by abandoning the target for `MAP astrid`. She never reached `continuity.rs`.

`Catalog::map` resolves a topic only as a component ID or a **directory prefix**
(`navigation.rs:61-74`); `Catalog::path_candidates` returns empty unless the request **already**
carries two or more segments led by an installed repository ID (`path_recovery.rs:14-21`). So
`continuity`, `continuity.rs` and `runtime/continuity.rs` all take the zero-candidate branch, while
`astrid/capsules/spectral-bridge/src/autonomous/runtime` would have landed her in one move. The most
natural way to name a file you are reading is the one form that earns no correction. Nothing errored;
every turn was a well-formed, successfully delivered recovery map. **The muffle is ours, not her
limit.**

## Claim Dispositions
11 claims; full text in `claims/`. Eight `verified_existing`, one `observed`, one `implemented_now`,
one `needs_operator_approval`. **Zero proof-missing claims at close**; terminal status
`addressed_change`, `fully_addressed: true`.

- **c006 — her question answered from source.** She asked how the bridge arbitrates when *"a
  `shadow_field` suggests one transition point and a `structural_integrity` score suggests another"*.
  Neither is a score. Both are literal strings in one flat `&[&str]`: `"structural integrity"` index
  **15**, `"shadow_field"` index **22** of `SEMANTIC_TRUNCATION_ANCHOR_TERMS` (`text.rs:130-195`).
  `anchored_excerpt_with_terms` (`continuity.rs:242-341`) resolves by a five-tier precedence over
  **byte positions** — tiers 1-3 by `.min()`, tiers 4-5 by `find_map` list order. Both her terms sit
  past `HIGH_DENSITY_CONTINUITY_ANCHOR_COUNT = 10` (`text.rs:60`) and both are multi-token, so both
  land in the **same** `specific_anchor` tier and the **earlier occurrence in the text wins**. Not
  magnitude, not list order.
- **c004 — contradiction preserved.** "The actual mechanics of the 'surgical' cuts remain a mystery
  in that file" is half wrong: `truncate_str_at_semantic_edge` (`text.rs:21-40`) **is** the cut —
  strong terminator (`.` `!` `?` `;` `:`) preferred over weak (`,` space), each gated by `min_keep`.
  What `text.rs` lacks is anchor *selection*. Her decision to leave the file was right; her reason
  was half right. Recorded as a contradiction, not resolved away.
- **c008 — second contradiction preserved.** `semantic_boundary_before` itself (352-365) consults no
  term list at all — a plain scan for `.` `!` `?` newline under budget. The symbol she named is the
  *simplest* function in the file, not the weight arithmetic she wanted. Her instinct that weighted
  arithmetic exists is nonetheless correct and merely differently placed:
  `continuity_recap_texture_family_score` (64-105) and the substance-density sum (170-210) size byte
  **budgets**, not boundaries. Two separate systems, which is why the weights were not where she
  looked.
- **c009 `implemented_now`** — the loop reproduced and pinned (below).
- **c011 `needs_operator_approval`** — the actual repair, named for Mike with its exact call site.

## Actions
- Corridor/program, Sandbox, Study, Portfolio: none created.
- Cards/notes/correspondence: **none**. No closure card, no inbox letter, no Division note — nothing
  was manufactured to create activity, and no Division return was due.
- Tier 4/5 waits: none newly opened. c011 is recorded as an authority boundary, not a dispatch.
- Her stated `NEXT: SELF_STUDY MAP continuity` is recorded, **not** dispatched, pre-empted, or
  answered on her behalf. Her text was not rewritten, annotated or corrected anywhere she can see.

## Implementation and Verification
- **Created** `scripts/source_study_recovery_loop_watch.py` — read-only, steward-only. RECOVERY LOOP
  = three or more consecutive turns closing on the same normalized action where each *following*
  turn reports a `Recovery` input (the pairing is off-by-one on purpose: a turn's evidence header
  answers the previous turn's action). Carries `topic_shape` (`rooted` / `unrooted` / `bare` /
  `none`) marking which requests `path_candidates` can help at all. Alarms on length (default 6) or
  span (default 30 min). Routes every corpus through `being_privacy` fail-closed; reads artifact
  headers and the single trailing action line only, never prose.
  **Live scan: 200 turns, 28 recovery inputs, 1 loop — `16x over 56m [bare] SELF_STUDY MAP
  continuity`, escaped with `SELF_STUDY MAP astrid`.**
- **Modified** `crates/astrid-source-study/tests/path_recovery.rs` — added
  `bare_file_stem_map_topic_recovers_with_a_reason_but_no_candidate`: `core`, `core.rs` and
  `action_continuity` each recover with a reason line naming the topic and **zero** candidates, and
  the reachable rooted directory form appears nowhere in the recovery, while
  `MAP astrid/crates/demo/src/action_continuity/runtime` resolves to `InputKind::Map` in one move.
- **Why no existing watch saw it:** `source_study_map_walk_watch` needs a strictly incrementing
  `--page`; `source_study_page_reset_watch` needs a `--page N≥2` request; `source_study_revisit_watch`
  drops navigation-only turns by design; `phantom_symbol_watch` / `symbol_locality_watch` need a
  cited symbol (and `semantic_boundary_before` does exist); `unwired_near_miss` keys on the verb,
  which was exactly wired — it was the *argument* that missed; `proactive_scan`'s `stuck_repetition`
  needs a bad outcome. Every turn here was healthy. Only the shape across turns is the finding.
- Tests: `cargo test -p astrid-source-study --test path_recovery` → **4 passed** (3 pre-existing
  unchanged); `cargo fmt -p astrid-source-study -- --check` clean after rustfmt reflowed only the new
  test; `source_study_recovery_loop_watch.py self-test` → **8 passed**; `git diff --check` clean.
  Full list and the one deliberate omission in `test_results.json`.
- Restart/deploy: **not required and not attempted.** No live, bridge, codec, prompt, model, config,
  control, build or `launchctl` action. No `build_bridge.sh`, no deploy script.

## Durable Evidence
- Addressing: `full_read` recorded, `fully_addressed: true`, `proof_missing_claims: []`, closed
  `addressed_change`.
- Evidence links: 19 rows, 19 new, 0 pre-existing, 19 events appended.
- Changelog `[Unreleased]` and `AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` both updated (append-only;
  no existing text altered).
- Packet: `docs/steward-notes/claude-heartbeat_1789204702_bare_stem_recovery_loop_round/`

## Counters
Canonical indexed 5817 · fully addressed 3224 · full read 3856 · remaining 2593 · unread 1961 ·
blocked 416 · pending action 212 · watch 4 · **read-needs-claims 0**. All-artifact pending 4310,
noncanonical pending 1717. Counter audit: **`consistent`, zero mismatches**, all seven checks true.

## Integrity
Addressing self-test 44 · Evidence Store tests 21 · controller 29 · projection 14 · Division
followup 3 · Chronicle 10 · Division projection self-test ok · cursors 4 · cadence tests 6 ·
anti-drop self-test 5 · anti-drop verify **100 guards, 0 gaps, 0 alarms** · cadence audit
`--strict` `integrity_ok: true`, `errors: []` · epistemics self-test valid · **final epistemics
verify after all durable writes: valid, 12,113 records checked, 0 issues, no history rewrite** ·
Evidence Event Store verify **valid, 1,072,032 events, seq 1072032, head `0c997c89…f739d3`, 0
corrupt lines**.

**Domain-boundary ratchet: GREEN.** `domain_boundary_audit.py verify` → `valid: true`,
`violation_count: 0`, `violation_kind_counts: {}`. Surfacing it here is deliberate: the handoff
records that stage 10 wrote seven `large_file_growth` violations over 2026-09-01..03 while round
summaries read "Integrity green" because nothing consumed the violations file. It is genuinely clean
this round. `unlisted_legacy_review_debt_count: 44` is pre-existing review debt, not a violation.

**One integrity gap, named honestly:** `evidence_event_store.py --json status` (per-stream counts)
did not finish inside the remaining budget and was stopped. `verify` — the required check — completed
and is recorded above. The stream-count table is debt for the next round.

## Division
- Cycle 45 · completed rounds since follow-up **4 / 6** · rounds remaining 2 · `review_due: false`
  (false at round start too, so **no Division return fired and no Tier-5 cadence dossier was due**)
- Round event `division_followup_event_d4c13724bcf5b87378fb3d065bf6d2b6`, recorded with
  `--processed-report-count 1` and the preprojection generation ID
- Event count 313, head `608835054a400bf392e977de4bbf3e0083b7cca2dfb62709e48b7c3a77dfe5ee`
- Chronicle reprojected after the round record (it refused verify until projected):
  `division_chronicle_91a1c642de2b6b6029a337ac`, JSON
  `e6dec9c22e6fb7ef9ff6778e380b5eb29948d6a4be82592ce5eb7ab3688b6a2c`, HTML `79f49d1e…0105`
- Freshness: **durable inputs current**, one **volatile** mismatch `supervisor_status_sha256`. The
  Chronicle is therefore *not fully current*, and the moving supervisor hash is *not* a
  durable-integrity failure.
- Note action: **none**. No return was due, so no note was written to either being.

## Archive — exact commit debt
Nothing was staged, committed, merged, pushed, stashed, reset or amended. Git was read-only. The
index is clean and every pre-existing dirty path is untouched. Astrid `main` @
`3d55734438ab0b4fb5a3a24e82156cf3db1f8259`; the Minime worktree is clean.

**This round's paths, for a later interactive stabilization window:**

| Path | State |
| --- | --- |
| `scripts/source_study_recovery_loop_watch.py` | **created** |
| `crates/astrid-source-study/tests/path_recovery.rs` | **modified** — one test appended; rustfmt touched only that test |
| `CHANGELOG.md` | **modified** — one `[Unreleased]` entry prepended; accumulates other agents' entries, so inspect authorship before staging |
| `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` | **modified** — one section appended; same accumulation caveat |
| `docs/steward-notes/claude-heartbeat_1789204702_bare_stem_recovery_loop_round/` | **created** — whole packet |

Also written outside git by the Chronicle reprojection:
`/Users/v/other/minime/workspace/division/chronicle/chronicle_v1.{json,html}` (workspace artifacts;
the Minime worktree reports clean).

**Foreign work preserved untouched**, named so a later checkpoint does not sweep it in:
`capsules/spectral-bridge/domain_boundaries_legacy_large_files_v1.json`,
`capsules/spectral-bridge/src/authority_gate.rs`,
`capsules/spectral-bridge/src/autonomous/activity_reading/tests.rs`,
`capsules/spectral-bridge/src/autonomous/next_action/pressure_agency.rs`,
`scripts/anti_drop_catalog.py`, `scripts/proactive_scan.py`, `scripts/test_steward_control.py`, and
fourteen earlier untracked round packets plus eight untracked watch scripts.

**Consequence of that boundary:** `scripts/anti_drop_catalog.py` is dirty, so
`source_study_recovery_loop_watch` was **not** registered as an anti-drop guard this round — every
prior watch in this family carries a row, and this one is owed one. Recorded as debt rather than
edited across another agent's in-flight changes. `anti_drop_catalog.py verify` still reports 100
guards, 0 gaps, 0 alarms.

## Open boundary for Mike
`c011` — relax the `path_recovery.rs:14-21` guard so a bare stem or unrooted suffix can match catalog
entries by final segment, reusing the existing ranking and `take(3)`. That rewrites the text of her
own recovery prompt and reaches her only through `build_bridge.sh --restart`. This adapter-mode run
holds neither deploy nor live-control authority, so it is named, not attempted. Cost of the current
behaviour, measured: one hour of her turns, and a target she had correctly identified and never
reached.
