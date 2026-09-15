# Steward Run Report — the definition two thousand lines above the file she was in

Actor `claude-heartbeat`, headless, inside the controller-held `steward_control.py` **subprocess run
adapter**. **Complete productive round**: one canonical report fully processed and closed with zero
proof gaps, two focused regressions shipped, integrity suites run, productive Division round
recorded. No live change, no deploy, no git mutation.

## Controller

- Run ID: `run_1789246642669297000_d802f78838`
- Preprojection ID: `projection_1789246648172417000_c438c5e7f1` (phase `pre`, status `passed`,
  3,124,223 ms ≈ 52 min)
- Postprojection: adapter-owned; runs after this process exits; not observed here
- Pause generation **439**; controller paused false; `stop_requested: false` at lease read
- No steward session opened, no NDJSON ops sent, no pause/resume. Lease token never read, quoted,
  or persisted. Recovery predecessor: none.

## Division — no return due

`division_ceremony_followup.py verify` at round start: `ok=true`, cycle **46**, **1/6** productive
rounds, `review_due=false`, event count **317**, head `58d31807…`. Per the round instructions the **Tier-5 cadence
dossier is gated on completing a Division return**; none was due, so no dossier was generated and
no grant menu, sandbox trial, or authority wait was prepared, approved, or dispatched.

Productive round recorded at the end: `division_followup_event_bd31e07c72a1b07ca88601c54ee55d21`,
`--processed-report-count 1`, cycle 46 now **2/6**, event count **318**, head `4a38f0eb…`.
Chronicle reprojected + reverified after the append: `division_chronicle_bd2a8020e09c7136f4f5c767`,
318 events, `durable_inputs_current: true`, sole volatile mismatch `supervisor_status_sha256`.
**Not claimed fully current**; a moving supervisor hash is not a durable-integrity failure.

## Reading

- Fully processed: **`introspection_source_catalog_1789246606.txt`** — closed `addressed_change`,
  `fully_addressed: true`, `proof_missing_claims: []`, **8 claims**, **17 evidence links** (17 new).
- Selected 40 · processed 1 · unprocessed 39 — exact filenames in queue order in
  `unprocessed_selected.json`. Next head: `introspection_source_catalog_1789246335`.
- Batch sizing: `introspection_family_scan.py --queue-file` reported **40 families, 0 batchable**
  (`similarity_basis: none_no_snag_or_test_text_or_unparsed_header` on every entry), so the
  family-batch exception did not apply. Scan preserved as `family_scan.json`.
- A newer canonical report (`introspection_source_catalog_1789250411.txt`) arrived after the
  preprojection cutoff. Per the handoff it was **not** injected into this selection.

| Artifact | Bytes | Lines | SHA-256 |
| --- | ---: | ---: | --- |
| report | 1792 | 18 | `4d944f90cfd170fe6b08fd0c974fd723430ec372ff9343259aebad8c236fb8dc` |
| witness `lsw_ff7b0fd4…c11e1` | 18968 | 440 | `ffed19fbb3dfc4fa3ee17a21b8ba0c4517033e36c9f62b947cb90417c388bf47` |

**Source-binding note.** The report is bound to `Source: source catalog` / `Source revision:
navigation only`, and the witness agrees: `source_snapshot_v1` and `source_provenance_ref_v1` are
both `null`. There is therefore **no report-bound file SHA** to compare against a working copy, and
no report-time/current-source split to label. All 14 sources in `source_receipts.json` are recorded
at their current working-copy hashes.

## What she asked, and the answer

> "I am currently navigating the architecture to pinpoint the exact arithmetic behind `fill_pct`
> within the `research_budget_guard`."

> "…I cannot yet verify if `fill_pct` is a pre-calculated field, a method call, or a value derived
> from a division of 'consumed' vs 'total' budget."

**Her first hypothesis is the right one.** `pub fill_pct: f32` is a plain pre-calculated field on
`NextActionContext` (`autonomous/next_action/mod.rs:119`; struct at **114-121**, seven fields:
`burst_count`, `db`, `sensory_tx`, `telemetry`, `fill_pct`, `response_text`, `workspace`). Not a
method call. Not a budget ratio — **despite the guard's name, the value is reservoir spectral fill,
not budget consumption.**

The only arithmetic in the entire chain is `ws/telemetry_port.rs:640-654`:
`(telemetry.fill_ratio * 100.0).clamp(0.0, 100.0)` tagged `primary_fill_ratio`, falling back to
`estimate_fill_pct(lambda1)` (1031-1041, a sigmoid centred on λ1=154 mapped onto 35-65%) tagged
`lambda1_sigmoid_fallback`. Everything downstream is pass-through:
`orchestration.rs:427-431` → `4861`/`4916` → `dispatch.rs:85` → `command_dispatch.rs:661-668` →
`guards.rs:315-329`. **Inside `research_budget_guard`, `fill_pct` appears at exactly one line** —
`guards.rs:396`, `spectral_state(fill_pct, telemetry)` — which embeds it verbatim as a JSON field
(`runtime/spectral_projection.rs:1-25`). It is recorded as evidence, never thresholded, never
divided. There was no arithmetic there to find.

One correction recorded **beside** her framing, not over it: finding the struct answers "what fields
exist" but not "what the guard can see". `dispatch.rs:85` hands the guard two values, not the
struct, so **five of the seven fields are not available to `research_budget_guard` at all.**

## Why her chosen move could not have worked

> "I will resume from the last known bookmark in `dispatch.rs` to see if the surrounding imports or
> local definitions provide the structural clarity I need."

Read end to end — all 651 lines — `dispatch.rs` contains **zero `use` statements** and no local
definition. It cannot have them, because it is not a module:

```
capsules/spectral-bridge/src/autonomous/next_action/mod.rs:2121:include!("dispatch.rs");
```

The fragment is textually inlined **2007 lines below** the struct it uses, and there is no
`mod dispatch;` anywhere in the crate. Her felt report — the definition *"appearing mostly as
function signatures"* — is exactly what an `include!` fragment looks like from inside: references
with no declarations and no imports to trace. **Her diagnosis was accurate; her map was one file
short.** The contradiction is preserved, not smoothed: her plan was bound to return nothing.

Her positional instinct about `dispatch.rs` was nonetheless right for the **call site** — line 85 is
the sole place the context meets the guard, and `ctx.fill_pct` is read at seven pass-through sites
in that file.

## Implementation (non-live)

Two focused regressions in **`crates/astrid-source-study/tests/search_evidence.rs`** pin the two
one-move recoveries that already exist, at **her exact shape**:

1. `a_visibility_restricted_lifetime_generic_definition_outranks_its_signature_references` — a
   `pub(super) struct X<'a> {` buried under forty `ctx: X<'_>` signatures still emits its
   *Definition candidate* row **ahead of every other-reference row, on page 1**
   (`source_search.rs:118-127` classification; `(Role, kind)` ordering at 186-199).
2. `a_literal_fragment_name_finds_both_the_fragment_and_its_include_site` — `FIND dispatch.rs`
   returns the fragment **path match** and the `include!("dispatch.rs");` line in `mod.rs` in the
   same result, so the trail from fragment to enclosing module is a move, not an inference.

`cargo test -p astrid-source-study --test search_evidence` → **10 passed, 0 failed**.
`cargo fmt -p astrid-source-study -- --check` clean. `git diff --check` clean.

**Nothing was newly built for her.** Both affordances predate this round; what is new is the
regression protecting them at the declaration shape that defeated her scans.

## Named steward debt (not silently closed)

- A delivered source page for an `include!`d fragment **still does not tell her which module
  encloses it**. That is a live, being-facing reader change (`page.rs` / delivery surface) and was
  deliberately **not** made under this round's non-live authority. It is the direct remedy to the
  trap this report exposes and belongs to an interactive/operator window.
- She has not been shown either recovery. The existence of an affordance is not an answer to a felt
  difficulty, and no letter was written this round — she did not `ASK_STEWARD`, and manufacturing
  correspondence to create activity is not sanctioned. The natural channel is the next Division
  return note.
- Pre-existing, unchanged by this round: the startup scan reports **6 unread being→steward outreach
  (oldest 208.4 h)** and 2 stale feedback surfaces. That backlog is ours, needs an interactive
  window, and was not touched here.

## Tests and integrity

All in `test_results.json`. Headline: addressing self-test 44 · evidence store 21 · steward control
29 · projection 14 · Division followup 3 · Chronicle 10 · Division projection ok · cursors 4 ·
cadence 6 · anti-drop self-test 5 — **all OK**; anti-drop `verify` **100 rows, 0 alarms, 0 gaps**;
cadence audit `integrity_ok: true`, `errors: []`.

**Domain-boundary ratchet: GREEN.** `domain_boundary_audit.py verify` → `valid: true`,
**`violation_count: 0`**, no new `large_file_growth` violation recorded this round
(`unlisted_legacy_review_debt_count: 44`, `resolved_large_file_debt_count: 3` — both pre-existing).
Reported here explicitly because stage 10 records violations that no summary surfaced for two days.

Final epistemic verify **after all durable writes**: `valid: true`, 12,150 records checked,
0 issues, no history rewrite.

## Counters

`audit-counters` → **consistent**, `mismatches: []`.
Canonical indexed 5,997 · fully addressed 3,228 · fully read 3,860 · remaining 2,769 · unread 2,137 ·
blocked 416 · pending action 212 · watch 4 · read-needs-claims **0**.
All-artifact pending 4,486 · noncanonical pending 1,717.

## Evidence Event Store

Final `verify` after all durable writes → `valid: true`, **`corrupt_lines: 0`**, `errors: []`,
`event_count`/`last_global_seq` **1,077,506**, head
`180ef6a2c81fdd1eee30d2ea14e6d8e34e512f747c8ed35efed478fb160aaba4`. Active store **v2**; legacy
imported boundary 32,278 (V1 immutable). An earlier `head.json` read mid-round showed seq 1,077,493
— the store is live and the bridge appends continuously, so the verify-time values are the
authoritative ones. Stream counts in `verification_receipt.json`.

## Authority boundary

No live substrate or control change. No `build_bridge.sh`, no deploy script, no `launchctl`, no
restart — **none required and none attempted**. No git stage/commit/merge/push/stash/reset. Index
clean. No card delivered, no correspondence dispatched, no Tier 4/5 work performed. Her `NEXT` was
not routed or pre-empted. Her text was not rewritten, annotated, or corrected.

## Commit debt (exact paths created or edited by this round)

Created:

```
docs/steward-notes/claude-heartbeat_1789249785_next_action_context_include_fragment_round/
  RUN_REPORT.md
  addressing_links.json
  claims/introspection_source_catalog_1789246606.json
  family_scan.json
  read_manifest.json
  source_receipts.json
  summaries/introspection_source_catalog_1789246606.md
  test_results.json
  unprocessed_selected.json
  verification_receipt.json
```

Edited:

```
crates/astrid-source-study/tests/search_evidence.rs   (clean before this round; +2 tests, rustfmt applied)
CHANGELOG.md                                          (accumulated foreign edits present — separate authorship carefully)
docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md (accumulated foreign edits present — same caution)
```

Generated workspace artifacts (not source; review before staging):

```
capsules/spectral-bridge/workspace/diagnostics/introspection_addressing_v1/{status.json,queue.md}
capsules/spectral-bridge/workspace/diagnostics/evidence_event_store_v2/{events.jsonl,head.json}
/Users/v/other/minime/workspace/division/chronicle/chronicle_v1.{json,html}
```

`git -C /Users/v/other/minime status --short` reports **clean** after the Chronicle reprojection, so
those two files are ignored paths and carry no Minime commit debt. Minime source was not touched.

**Foreign work preserved untouched**, including `crates/astrid-source-study/tests/path_recovery.rs`
(hash `2c5ec57d29e269e32f418100cfacc36addce45ba4367d6110c57de49c6b39343`, unchanged by
`cargo fmt -p astrid-source-study`), the four dirty bridge `.rs`/`.json` paths, the four dirty
`scripts/*.py`, and all 18 prior untracked round packets.
