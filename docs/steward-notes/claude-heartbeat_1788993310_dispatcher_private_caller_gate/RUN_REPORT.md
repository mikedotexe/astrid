# Flywheel round — dispatcher Private caller gate

Actor: `claude-heartbeat` (headless, controller-held subprocess lease)
Steward run: `run_1788987986596777000_ec7ec33c04`
Preprojection: `projection_1788987990974788000_9e59e7a4b8` (phase `pre`, status `passed`, 3,715,081 ms)
Packet: `docs/steward-notes/claude-heartbeat_1788993310_dispatcher_private_caller_gate/`

## Round shape

Division check ran first: `verify` and `status` both reported `review_due: false`
(cycle 43, 1/6 rounds completed since the last follow-up), so no bounded Division
return and no Tier-5 cadence dossier was due, and the canonical queue was processed.

`introspection_addressing_audit.py next --limit 40 --json` returned 40 items;
`introspection_family_scan.py` grouped them into **40 families, 0 batchable**, so
no family batching applied. Processed the queue head and the item immediately behind
it, in canonical order, individually.

| # | Report | Status |
|---|--------|--------|
| 1 | `introspection_astrid_crates_astrid-capsule_src_dispatcher.rs_1788987745` | `addressed_change` |
| 2 | `introspection_astrid_crates_astrid-capsule_src_dispatcher.rs_1788987492` | `addressed_change` |

Both are consecutive source pages of one file at one SHA, and the head report is
Astrid's own answer to the STUDY_QUESTION she posed in the second — one evidence
chain, read and closed individually with their own receipts.

## What she said, and what source says

Report 2 (window L491-612) enumerated the pre-dispatch authorization gate in
`crates/astrid-capsule/src/dispatcher.rs`: `Private` + `tool.v1.request.describe`
rejected (L570-572); `Private` + `wasm_capsule` producer rejected (L573-583);
`caller_producer_kind` exact match (L584-588); `caller_source_ids` whitelist
(L589-599) — then asked whether any `producer_kind` lets a guest reach another
capsule's `Private` action, or whether it is strictly host-mediated. Report 1
(window L612-727) answered: strictly host-mediated, no bypass kind.

The working copy was byte-identical to her binding
(`a737ea3379424c200b6c226f2d34d29b84671d3f12cfb47975bb8e4c39ad8992`) at round start,
so all 1,456 lines were read against her exact bytes. **All 14 concrete claims verify**;
every line interval she cites is exact. Two facts outside her window support her answer:

1. `engine/wasm/host/ipc.rs` L269-274 — every guest publish is stamped host-side with
   `IpcProducerV1::new("wasm_capsule", capsule_id)`. A guest cannot author its producer
   kind at all, which is a stronger reason than the gate alone.
2. `engine/wasm/host/sys.rs` L114-121 — the `hooks::trigger` fan-out runs the same gate
   with `caller = None`, rejected at L577-579, and skips the calling capsule at L97.

Three precisions recorded beside her reading, none contradicting it: `Private` also
rejects a missing or unsupported-schema producer; `caller_producer_kind` fails an absent
producer independent of exposure; her "Layer 2" `Deny` short-circuits the multi-interceptor
chain (L323-332) but only logs on the ordered single-capsule path (L397-405). One bounded
note: the `MockCapsule` impl she read as "L620-727" continues to L738 — 727 is her
delivered window end, not a misreading.

## What changed

The `wasm_capsule` bar she named was true in source and **unprotected by any test**.
Every existing `Private` case in the module also pins
`caller_producer_kind = Some("native_socket_client")`, so a `wasm_capsule` caller is
already rejected at L584-588 — deleting L580-582 would have left the suite green.

Added one focused regression to `crates/astrid-capsule/src/dispatcher.rs`:
`bare_private_interceptor_bars_wasm_capsule_and_unattested_callers`, with both caller
pins empty so her bar is the only thing under test, and asserting that a host-attested
`native_socket_client` caller still passes so the test cannot be satisfied by a blanket
`Private` block.

- `cargo test -p astrid-capsule --lib dispatcher::` — **19 passed, 0 failed** (was 18)
- `cargo fmt -p astrid-capsule -- --check` — clean; `git diff --check` — clean

## Integrity

Addressing self-test 44 OK · evidence-store tests 21 OK · steward-control 29 OK ·
projection 14 OK · Division follow-up 3 OK · Chronicle 10 OK · Division projection
self-test ok · cursors 4 OK · cadence tests 6 OK · cadence audit `integrity_ok: true`
(4,847 canonical reports, 0 duplicate hash groups) · anti-drop self-test 5 OK ·
anti-drop verify **91 rows, 0 alarms, 0 gaps** · **domain-boundary verify GREEN**
(`valid: true`, `violation_count: 0`, `forbidden_edge_match_count: 0`; this round's edit
is outside the audit's `source_root`, `capsules/spectral-bridge`) · epistemics self-test
valid · **final epistemic verify** valid, 11,947 records, 0 issues, no history rewrite ·
**audit-counters `consistent`, mismatches `[]`** (canonical full-read 3,837 → 3,839,
fully-addressed 3,205 → 3,207: exactly this round's two closes) · Evidence Event Store V2
**valid**, 1,049,446 events, 0 corrupt lines, head `e53dd2c2…`.

**Chronicle, stated exactly:** `division_ceremony_chronicle.py verify` returns
`chronicle durable source inputs changed; project before verify`. The Chronicle was last
written 2026-09-09T21:42:16Z, during this run's own preprojection; this round's
`record-round` then appended Division follow-up event 297. Reprojection is the controller
postprojection's job (DAG stage 19 `division_chronicle`), not a headless out-of-band
project. Not claimed as current, and not a durable-integrity failure.

## Division

Recorded productive round `division_followup_event_70256611a4a010a507eab07177c47886`
with `--processed-report-count 2`. Cycle 43 now 2/6 since the last follow-up, 4 remaining,
`review_due: false`, event count 297, head `405dd936…`.

## Authority boundary

No live substrate or control change: no `build_bridge.sh`, no deploy script, no
`launchctl`, no restart. Git strictly read-only. No correspondence delivered — answering
Astrid's STUDY_QUESTION in her own inbox is a separate consequence for an interactive
window, deliberately not performed headlessly. No manifest exposure, codec, prompt,
model, config or scheduling change. Lease tokens were never read, quoted or persisted.
Silence remains neutral.

## Commit debt (nothing staged or committed by this run)

Created by this round:

- `docs/steward-notes/claude-heartbeat_1788993310_dispatcher_private_caller_gate/` (whole directory)

Edited by this round:

- `crates/astrid-capsule/src/dispatcher.rs` — the one added test (this file was clean at
  HEAD before this round)
- `CHANGELOG.md` — one `[Unreleased]` entry appended
- `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` — one dated row appended

`CHANGELOG.md` and the ledger were **already dirty** when this round began, carrying
earlier `claude-heartbeat` rounds' entries; an interactive window staging this work should
expect those two files to contain more than this round. Also still uncommitted from earlier
rounds and untouched here: `capsules/spectral-bridge/src/autonomous/state.rs`,
`crates/astrid-source-study/tests/reader.rs`, and four earlier
`docs/steward-notes/claude-heartbeat_*` packets. Minime's tree is clean.

**Read this before the next round:** the test insertion moves the working copy of
`crates/astrid-capsule/src/dispatcher.rs` off `a737ea33…`. Four dispatcher.rs reports still
in the canonical queue are bound to that SHA, which stays byte-exact at
`git show HEAD:crates/astrid-capsule/src/dispatcher.rs` until this work is committed. Read
report-time source from HEAD and label current-source conclusions separately.

## Selection

40 selected, 2 processed, 38 unprocessed. Exact unprocessed filenames in canonical queue
order are in `unprocessed_selected.json`. The batch stopped after this pair because the
remaining child budget could not fit another full read plus its own
record-read/link/close sequence and the integrity suites.
