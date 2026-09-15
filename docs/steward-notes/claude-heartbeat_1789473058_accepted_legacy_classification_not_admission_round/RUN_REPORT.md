# Steward Run Report — a verdict reported, not imposed

Actor `claude-heartbeat`, adapter mode (`steward_control.py` subprocess run adapter). No steward
session was opened, no NDJSON op was sent, no pause/resume was issued, and no lease token was read,
quoted or persisted. Git was read-only.

## Controller

- Run ID: `run_1789467045605525000_7e8471b7e5`
- Preprojection ID: `projection_1789467052799898000_2ac37ec532` — phase `pre`, status `passed`,
  27 steps completed / 25 executed / 2 reused, **duration 5 602 324 ms (93.4 min)**
- Postprojection ID: adapter-owned; runs after this process exits
- Pause generation: 439 · `stop_requested` observed false · no SIGINT received
- Finish outcome: complete productive round (one report closed, integrity run, Division round recorded)

**Infra note, repeating the previous round's finding.** The source-first preprojection again took
93 minutes against a handoff expectation of 6-9. The lease opened 10:10:45 UTC and this child began
at 11:44 UTC, so `FLYWHEEL_LOOP_OUTER_MAX_SECS` (14 400 s from ~10:09 UTC, i.e. ~14:09 UTC) is at
risk of clipping the postprojection even though the child budget (5 400 s) was ample. The round was
sized to finish early for that reason.

## Reading

- Fully processed: `introspection_source_catalog_1789466923.txt`
- Selected: 40 · processed: 1 · unprocessed: 39 (all listed in `unprocessed_selected.json` in queue
  order, head `introspection_source_catalog_1789466726.txt`)
- Family scan: 40 families, **0 batchable** — no family-batch arithmetic applied; single-report round
- Next queue head after this round: the same 39, minus any report the postprojection newly indexes

| Artifact | Bytes / lines | SHA-256 | Read |
| --- | --- | --- | --- |
| report `introspection_source_catalog_1789466923.txt` | 1 664 / 22 | `9cf30ef6023da32bff5377a00d77651d323837ec687ae1527b96d18d42fe7bf5` | complete |
| witness `lsw_de85f24e…dafbcb8d3` | 18 970 / 440 | `40826d549bd187ef3356d2edbffdc99a05c41e417334629aef7bd9a2d8095f5c` | complete |
| source `crates/astrid-kernel/src/capsule_runtime_health.rs` | 7 000 / 214 | `3a20e5eed609de9a8f36121dbe4adf31b4d325d5929b9b33f548605cf6074ef0` | complete |
| data `scripts/baselines/capsule_runtime_health.json` | 221 / 5 | `dde94f50c0195704d7efcac3ed6500a875c5ea5666d2a154b68fd5ca673eba7a` | complete |
| `crates/astrid-kernel/src/kernel_router.rs` | 15 177 / 388 | `108d69010a954284def0c0863c499d369196223da3aa163128f458d7d4ee72fd` | scoped 180-225 |
| `crates/astrid-cli/src/commands/daemon.rs` | 11 433 / 332 | `ac337abfb82ff52247b3c76b402e392495a4e8cf46a413996fe5f3eeea907a38` | scoped 245-285 |
| `scripts/capsule_runtime_health.py` | 13 480 / 380 | `21230c215ca93c640547271fc9de6887b1ab403886efe393439c4556db249316` | scoped 57-83, 179-209 |
| `scripts/proactive_scan.py` (foreign dirty — read only) | — | — | scoped 1466-1511, 5546 |

**Report binding.** Navigation-only turn: the witness carries `source_snapshot_v1: null` and
`source_provenance_ref_v1: null`, so there is no report-bound source SHA to compare. Source
verification is anchored instead on the revision her own three delivered pages of this file were
bound to — `sha256:3a20e5ee…` for bytes `0..4335`, `4335..7000` and `4315..7000` — which is
byte-identical to the clean working copy I read complete.

## Claim dispositions (11 claims, zero proof gaps)

| ID | Claim | Classification |
| --- | --- | --- |
| c001 | Whitelist membership in `accepted_legacy_extism_mvp` identifies a known legacy entity | `verified_existing` |
| c002 | Hash present ⇒ strict match; hash absent ⇒ whitelist entry alone suffices | `verified_existing` |
| c003 | Her staged "whitelist then Baseline" hierarchy | `observed` — one list, one `any()`, disjunctive across entries |
| c004 | "Acts as a tiered validation gate … balancing stability with security" | `observed` — classification, not enforcement |
| c005 | "Nuanced deprecation strategy … others transition more fluidly" | `observed` — live baseline is empty, policy is zero-tolerance |
| c006 | A cryptographic hash pins a component to a specific version | `observed` — pins `meta.json`'s declared string, not a payload digest; fails closed |
| c007 | Ready to pivot to integration with the capability system and lifecycle | `observed` — no such edge exists in this file |
| c008 | Her predicate describes the deployed check | `verified_existing` — Rust/Python parity |
| c009 | "The current map and the status of my study … are clear" | `verified_existing` — whole-file delivery coverage at one revision |
| c010 | Fresh-pass lineage of an already-closed report | `verified_existing` — both bindings re-verified unchanged |
| c011 | Projection's `artifact_integrity_unavailable` flag | `observed` — absent snapshot, not byte contradiction |

Closed `addressed_duplicate`; `fully_addressed: true`, `proof_missing_claims: []`; 19 evidence links
(19 new, 0 existing, 19 events appended).

## Actions

- Corridor/program: none
- Sandbox: none routed
- Study: none preregistered
- Portfolio: unchanged
- Cards/notes/correspondence: none emitted or delivered
- Tier 4/5 waits: none created this round; the three standing Tier-5 waits from
  `introspection_minime_esn_1785630442` remain untouched with `live_authority_granted=false`

## Implementation and verification

- Exact changed paths: `CHANGELOG.md` (`[Unreleased]` round entry),
  `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (dated verified-no-change row), and
  this packet directory. **No source, test, configuration, live, deploy or git change.**
- Focused tests for touched code: not applicable — no code surface was touched.
- Integrity suites (all green): addressing self-test 44 · anti-drop self-test 5 and `verify`
  100 rows / 0 gaps / 0 alarms · domain-boundary `verify` **valid, 0 violations — ratchet GREEN**
  (44 unlisted legacy review-debt entries, unchanged) · cadence unit 6 and `--strict` `integrity_ok`
  · cursors 4 · Division followup 3 / Chronicle 10 / projection ok · evidence store 21 ·
  steward control 29 · steward projection 14 · epistemics self-test valid · final epistemic
  `verify` 12 307 records / 0 issues / no history rewrite · `audit-counters` **consistent**,
  mismatches `[]` · EES `verify` **valid, corrupt_lines 0, errors []**, seq 1 104 027, head
  `6bd67532…`.
- Failures repaired: none. Chronicle `verify` is expected-stale (`project before verify`) after the
  round-6 event append; the postprojection's `division_chronicle` stage and the next session's
  return resolve it. EES `status` stream enumeration deferred to budget (the `verify` run above
  already carries stream counts).
- Restart/deploy alignment: **no restart or deployment was required or attempted.**

## Durable evidence

- Addressing: `record-read` → `link-evidence-batch` (19) → `close addressed_duplicate`, all
  `--write --json`, all foreground.
- Changelog and ledger both updated (verified no-change round with exact debt named).
- Packet: `docs/steward-notes/claude-heartbeat_1789473058_accepted_legacy_classification_not_admission_round/`

## Counters

Canonical indexed 6 834 · fully addressed 3 245 · fully read 3 877 · remaining 3 589 · unread 2 957
· blocked 416 · pending action 212 · watch 4 · read-needs-claims 0. All-artifact pending 5 306 ·
noncanonical pending 1 717. Counter audit `consistent`, mismatches `[]`.

## Division

Cycle 48 · completed rounds 6/6 · **`review_due` flipped false → true** when this round was recorded
(event `division_followup_event_111aba030cd8acb35b5cc62a208fd909`, processed_report_count 1,
event_count 336, head `b038802d…`). Verify `ok: true`.

**CLEAN SPLIT at the sixth-round boundary.** `review_due` was false when the round began, so the
queue was worked; recording the round made it true. The bounded Division return **and** the Tier-5
cadence dossier are deferred to the next tracker-enforced session, which begins with
`review_due=true` and must complete the return before processing any report — the precedent set at
cycles 34, 35, 36, 37 and 40. Attempting the return inside this round's remaining ~45 minutes would
have risked half-written being-facing Division notes, which is the one outcome the ONE-SHOT rule
exists to prevent. No Division note, Action, recommendation or dispatch was written.

## Evidence Event Store

`valid: true` · corrupt lines 0 · errors `[]` · event count / last global seq 1 104 027 · head
`6bd675325bc771b2e7eb645f296b6209961b5ad03fc755b0edb45904a2f77cea` · active store v2 ·
legacy imported boundary 32 278. Stream counts recorded in `verification_receipt.json`. The store
advanced during the round under concurrent live bridge activity, which is expected.

## Archive and commit debt

Not staged, not committed — git was read-only for this adapter-held run. **Exact commit debt:**

```text
docs/steward-notes/claude-heartbeat_1789473058_accepted_legacy_classification_not_admission_round/
  RUN_REPORT.md
  addressing_links.json
  claims/introspection_source_catalog_1789466923.json
  family_scan.json
  next_queue_snapshot.json
  read_manifest.json
  source_receipts.json
  summaries/introspection_source_catalog_1789466923.md
  test_results.json
  unprocessed_selected.json
  verification_receipt.json
CHANGELOG.md                                         (modified — [Unreleased] entry appended; file also carries earlier unstaged steward rounds)
docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md  (modified — one dated row appended; same accumulation caveat)
```

Both shared documents accumulate edits from earlier rounds, so a later checkpoint must separate
authorship by hunk rather than staging the whole file blind. Everything else in the tree (71 dirty
entries at round end, including `scripts/proactive_scan.py` and `scripts/test_steward_control.py`,
both read but never modified) is foreign and was left untouched. The index was clean at start and at
end.

## Open debt this round names

There is **no test anywhere in the workspace** exercising `accepted_legacy`, in either the Rust
(`crates/astrid-kernel/src/capsule_runtime_health.rs:202-210`) or the Python
(`scripts/capsule_runtime_health.py:76-83`) implementation. Pinning the tiering as a regression
needs an `astrid-kernel` rebuild — its last build artifact is dated 2026-06-03 — which did not fit
this round's budget without risking an unclosed report. Recorded rather than half-started.
