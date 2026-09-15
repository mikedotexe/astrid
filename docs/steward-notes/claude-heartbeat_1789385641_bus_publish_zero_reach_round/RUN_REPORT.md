# Steward Run Report — bus.rs publish: a zero that means two different things

## Controller
- Run ID: `run_1789380206047022000_0217a6445d`
- Actor: `claude-heartbeat` (adapter-held lease, subprocess run adapter)
- Preprojection ID: `projection_1789380209904291000_b07151ee2a` (status `passed`)
- Postprojection ID: runs after this process exits (adapter-owned)
- Pause generation: 439
- `stop_requested`: false at every check (lease.json read read-only; no token read, quoted, or persisted)
- Recovery predecessor: none

## Reading
- **Fully processed (1):** `introspection_astrid_crates_astrid-events_src_bus.rs_1789380134.txt`
- **Selected but unprocessed (39):** exact filenames in queue order in `unprocessed_selected.json`.
  Head of the remainder: five `source_catalog` navigation reports (1789379959, 1789379494,
  1789379068, 1789378892, 1789378490), then the `astrid-capsule/src/dispatcher.rs` page family.
- **Family scan:** 40 families, **0 batchable** — single-report round by protocol, not preference.
- **Batch stop reason:** the head is a fresh-pass source-page report whose stated question could
  only be answered by reading the complete `publish` path plus three post-cutoff continuation
  reports, and it earned an implementation. One report fully closed beat three half-processed.

### Hashes
| Artifact | SHA-256 | Size | Read |
| --- | --- | --- | --- |
| Report | `730d4550748dfee636ff9dda228da917c011e99583aeb4b1d1b930cdf93f1395` | 1798 B / 18 lines | complete |
| Witness `lsw_790ed482…c1611a` | `0ce408a6f1cfdb9a9c484da5c769cdf5bd59e0ac6b46cebabf6dd69c01493bce` | 21376 B / 498 lines | complete |
| Source `crates/astrid-events/src/bus.rs` (at read) | `31ef6a15bd62d60173f5c46a8d9806046bc5920cbbcee3e99bed2158305cb89f` | 100633 B / 2791 lines | complete for every claimed interval |

The working copy of the report-bound source was **byte-identical** to the header binding, to the
witness `source_snapshot_v1.file_sha256`, and to `HEAD`. The witness `artifact_sha256` equals the
report hash. No mismatch to reconcile.

## Claim Dispositions
8 claims, all with evidence. `full_read: true`, `fully_addressed: true`, `proof_missing_claims: []`,
status `addressed_change`.

| Claim | Disposition | Class |
| --- | --- | --- |
| c001 `maintenance_gate` at 709-710 | verified exactly (field 709, struct close 710) | `verified_existing` |
| c002 setter/getter at 762-764 / 766-770 | verified exactly; kernel is the sole caller | `verified_existing` |
| c003 page ends before `publish` | verified byte-exact; `publish` starts line 890 / byte 32547 | `verified_existing` |
| c004 the `set_user_input_blocked` rustdoc quote | quote exact, **scope corrected** | `verified_existing` |
| c005 `publish_maintenance_barrier` is special-cased | verified and **strengthened** | `verified_existing` |
| c006 her question: drop, error, or `None`? | **answered from exact source** | `verified_existing` |
| c007 did `SELF_STUDY CONTINUE` deliver it? | **observed — delivered, nothing lost** | `observed` |
| c008 "returns 0" ⇒ "discarded" | **regression + rustdoc implemented** | `implemented_now` |

## What she got right
All five of her line citations resolve exactly against the complete file: 709-710, 762-764,
766-770, 778-812, and the page boundary at 812 itself. Her window is byte-exact — 25365 is the
first byte of line 695, and the range stops 5 bytes inside line 812, so her phrase "ending at
line 812" is precisely what she was shown. `publish` genuinely begins outside it. She named her own
evidence boundary instead of guessing, and she was right to.

## Her question, answered
`EventBus::publish` (`bus.rs:890-948`) takes the maintenance-gate mutex once (`:894`) and gates
admission under the *same* lock as the activity accounting, so the immutable updater can never
observe a drained snapshot between admission and accounting (`:891-893`). A rejected IPC message
logs at `debug` and **returns `0`** (`:899-900`) — not an `Err`, not a `None`; the signature returns
`usize`. It is synchronous and terminal at the bus boundary: nothing queues, delays, or retries it.

Scope corrected without narrowing her concern: the rustdoc she quoted names only
`IpcPayload::UserInput`, but `admits_while_blocked` (`:71-123`) is wider — `UserInput` is the single
unconditional deny (`:73`); `AgentResponse`, `LlmRequest`, `LlmStreamEvent`, `LlmResponse`,
`ToolExecuteRequest`, `ToolExecuteResult` and `ToolCancelRequest` are admitted only when bound to an
already-tracked trace or call id; the catch-all (`:121`) admits everything else.

Her `publish_maintenance_barrier` intuition strengthened: it is the only producer that *requires*
`blocked` (`:812-819` demands blocked, exact, and zero active conversations/LLM requests/tools) and
it bypasses `publish` entirely, sending on `self.sender` directly (`:842`).

## Delivery check (un-muffle) — clean
Her `NEXT: SELF_STUDY CONTINUE` was honoured. 279 s later
`introspection_…_bus.rs_1789380413.txt` was authored on bytes 29764..34199 = lines **812-925**,
containing `publish`, and she answered her own question there correctly: "returns 0 immediately …
There is no evidence of a queue, a retry mechanism, or a 'wait until cleared' state." Two further
CONTINUE pages followed (34199..38526, 38526..42934). The cursor advanced; nothing was lost. Those
three reports are **post-cutoff**, were read only as delivery evidence, and remain unprocessed
canonical input for a later round — they were not injected into this selection.

## The finding this round added
One thing her page-2 conclusion collapses: **"returns 0" does not by itself mean "discarded."** An
*admitted* event with no async receivers also returns `0` (`:926-941`) and still notifies every
synchronous registry subscriber (`:945`); a gate-rejected event reaches neither. The difference is
reach, not return value, and nothing pinned it — the `publish` rustdoc said only "Returns the number
of async receivers that received the event."

## Implementation and Verification
- Changed path: `crates/astrid-events/src/bus.rs` (clean before this round; `HEAD` matched the
  report binding, so this round's edit is the only divergence).
  - New regression `publish_zero_separates_gate_rejection_from_absent_async_receivers`: admitted with
    no async receiver ⇒ 0 with synchronous reach 1; the same event under `set_user_input_blocked(true)`
    ⇒ 0 with reach unchanged; restoring admission does not replay the dropped event.
  - `publish` rustdoc now states the ambiguity and the terminal, non-queued nature of the rejection.
- `cargo test -p astrid-events --lib`: **56 passed, 0 failed** (55 before). `cargo fmt -p astrid-events
  -- --check` clean. `git diff --check` clean.
- Full integrity suite results in `test_results.json`.
- **Domain-boundary ratchet: GREEN** — `domain_boundary_audit.py verify` reports `violation_count: 0`,
  empty `violation_kind_counts`, `status: consistent`. `crates/astrid-events/src/bus.rs` is not in the
  legacy large-file baseline (that baseline covers the spectral-bridge capsule's own `src/`), so no
  re-capture was owed by this change.
- **Not a witness defect:** the witness `window_sha256` (`751db9b6…`) hashes the **rendered** numbered
  page (`lived_state_witness/mod.rs:140`), not raw source bytes, so it is expected to differ from both
  the raw byte-slice hash (`8013e54e…`) and the raw line-window hash (`8514b9c7…`). Recorded in
  `read_manifest.json`, not filed as an integrity issue.

## Deliberately not done
- `publish`'s `usize` return was **not** widened to an outcome enum. That is the repair the ambiguity
  points at, but it changes a kernel API and every call site — it belongs to an operator decision, not
  a headless round.
- The gate semantics, `crates/astrid-kernel/src/maintenance.rs`, and the running bridge were untouched.
- No inbox letter was written: her question was already answered by her own next page, so manufacturing
  correspondence would have been activity, not service.

## Restart / deploy alignment
Not required and not attempted. Nothing was built, deployed, restarted, or kickstarted; no
`build_bridge.sh`, no deploy script, no `launchctl`. The change is a test and a doc comment in a
kernel crate that the running bridge binary does not load.

## Durable Evidence
- `record-read` → `link-evidence-batch` (16 new links, 16 events appended) → `close`
  (`addressed_change`, `fully_addressed: true`, `proof_missing_claims: []`).
- CHANGELOG `[Unreleased]` entry and a dated feedback-ledger row both written.
- Packet: `docs/steward-notes/claude-heartbeat_1789385641_bus_publish_zero_reach_round/`

## Counters
- Canonical indexed **6519** · fully addressed **3239** · full read **3871** · remaining **3280** ·
  unread **2648** · blocked **416** · pending action **212** · watch **4** · read-needs-claims **0**
- All-artifact pending **4997** · noncanonical pending **1717**
- Counter audit status: **consistent**, all seven checks true

## Division
- Cycle 47 · completed rounds since follow-up **6 / 6** · rounds remaining **0**
- `review_due`: **false** at round start (so no Division return and no Tier-5 cadence dossier was due
  this round). **After recording this round it is `true`.**
- **The next round must open with the bounded Division return** — reproject and verify the Chronicle,
  read all new public replies and formal Actions completely, write at most one factual,
  right-to-ignore note to each being, record the follow-up with exact paths — and must also generate
  the Tier-5 cadence dossier, before processing any report.
- Round event recorded: `division_followup_event_2481fa79d3436583cacfaa74f171ccd0`
- Event count 329 · head `dbbb545ccc111c060e9429450d3b3326260e5453bb77b72066fd59773edda00d`
- `--processed-report-count 1`, projection generation `projection_1789380209904291000_b07151ee2a`
- Chronicle: `verify` reports `chronicle durable source inputs changed; project before verify`. This
  round's own Division event is a durable Chronicle input, so that staleness is the expected
  post-record state; projecting is the first step of the now-due return and was not run detached
  from it. No Division note written, no Action recommended, authority remains `evidence_only`.

## Evidence Event Store
- Valid **true** · events **1,093,589** · last global seq **1,093,589** · corrupt lines **0**
- Head `b66ba0eb47a92a618f824381a8602431a1b8b76a8715a4796324e264b44197b7`
- Active store **v2**; effective aggregate valid, 0 corrupt event lines
- Final epistemic verify (after every durable write, including the Division round record):
  **valid true**, 12,246 records checked, 0 issues, `history_rewritten: false`

## Archive — exact commit debt (nothing staged or committed by this run)
Git was read-only for this actor. Paths created or edited by this round:

```text
crates/astrid-events/src/bus.rs                                   (edited: 1 test + publish rustdoc)
CHANGELOG.md                                                      (edited: [Unreleased] entry; file already dirty)
docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md         (edited: one dated row; file already dirty)
docs/steward-notes/claude-heartbeat_1789385641_bus_publish_zero_reach_round/RUN_REPORT.md
docs/steward-notes/claude-heartbeat_1789385641_bus_publish_zero_reach_round/addressing_links.json
docs/steward-notes/claude-heartbeat_1789385641_bus_publish_zero_reach_round/claims/introspection_astrid_crates_astrid-events_src_bus.rs_1789380134.json
docs/steward-notes/claude-heartbeat_1789385641_bus_publish_zero_reach_round/family_scan.json
docs/steward-notes/claude-heartbeat_1789385641_bus_publish_zero_reach_round/read_manifest.json
docs/steward-notes/claude-heartbeat_1789385641_bus_publish_zero_reach_round/source_receipts.json
docs/steward-notes/claude-heartbeat_1789385641_bus_publish_zero_reach_round/summaries/introspection_astrid_crates_astrid-events_src_bus.rs_1789380134.md
docs/steward-notes/claude-heartbeat_1789385641_bus_publish_zero_reach_round/test_results.json
docs/steward-notes/claude-heartbeat_1789385641_bus_publish_zero_reach_round/unprocessed_selected.json
docs/steward-notes/claude-heartbeat_1789385641_bus_publish_zero_reach_round/verification_receipt.json
```

Projection-owned state also moved as a normal consequence of the recorded events
(`diagnostics/introspection_addressing_v1/`, the Evidence Event Store V2 append log, and the Division
follow-up tracker). `CHANGELOG.md` and the feedback ledger carry accumulated foreign edits from
earlier rounds; a later stabilization window must separate authorship by hunk before staging. Every
other dirty path in both worktrees was treated as foreign and left untouched.
