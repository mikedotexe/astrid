# Steward Run Report — kernel_router dispatch and response topic

Actor `claude-heartbeat`, headless single-turn round inside a controller-held
`steward_control.py` subprocess lease. The adapter owned the lease and its heartbeats; no steward
session was opened, no NDJSON op was sent, and no lease token was read, quoted or persisted.

## Controller

- Run ID: `run_1789578439907480000_2edad997cb`
- Preprojection ID: `projection_1789578442954571000_039499e9d4` (status `passed`, 27 steps)
- Previous successful generation: `projection_1789572631875762000_28af2c1f96`
- Postprojection ID: runs after this process exits; not observed here
- Pause generation: 445
- Finish outcome: complete productive round (2 reports closed)
- Recovery predecessor: none

## Reading

Queue: `introspection_addressing_audit.py next --limit 40 --json` (40 selected, order frozen and
preserved). `introspection_family_scan.py --queue-file` reports `batchable_family_count = 0`, so
this is a base 1-3 batch, **not** a family batch.

**Fully processed (2):**

1. `introspection_astrid_crates_astrid-kernel_src_kernel_router.rs_1789577928.txt` — 3,579 B / 35 lines,
   sha256 `6a35a5074a28676c778a4664b7fa076cdd92736b6f8765721c89ea20203d0c32`; witness
   `lsw_5238ec03…f142ce`, 21,424 B / 498 lines, sha256 `cc509a08…19ec20`; window lines 111-213 of 388.
2. `introspection_astrid_crates_astrid-kernel_src_kernel_router.rs_1789577505.txt` — 2,789 B / 32 lines,
   sha256 `edb7d0ddb03510aa07293f3b7490f92ec1a8c2b74af5b523a4ed32e951ab99e2`; witness
   `lsw_84152d64…2150bc`, 21,445 B / 498 lines, sha256 `dce3384a…17d4b2`; window lines 1-111 of 388.

Both reports and both witnesses were read byte-complete. Both bind the **same** source sha
`108d69010a954284def0c0863c499d369196223da3aa163128f458d7d4ee72fd`, which matched the working copy
exactly at read time (git-clean, last touched by `c7dd6757a7`), so report-time bytes were read
directly and one complete-source verification legitimately serves both — the condition the family
protocol requires for shared source verification, even though these two are not a scan family.

**Selected but unprocessed (38):** exact filenames in queue order in `unprocessed_selected.json`.
Next head for the following round: `introspection_source_catalog_1789577233.txt`.

**Stop reason:** honest batch of two, sized so the whole record-read → link → close → integrity →
record-round sequence fit a single-turn headless budget.

## Claim dispositions

18 claims (10 + 8), every one with a grounded disposition, a classification, and linked evidence.
Full text in `claims/`. The load-bearing ones:

- **Verified exact:** every line range she cites across both reports — 18-67, 22, 25, 39-58, 69-95,
  98-110, 114-120, 121-128, 129-152, 153-178, 179-194, 195-211, 212. Her behavioural descriptions of
  each dispatch arm are accurate, including that `Shutdown` publishes its confirmation *before*
  signalling (the early `return` at 193 is exactly why).
- **Corrected — the rate-limiter example is inverted (`d003`).** She wrote that the limiter protects
  against commands *"like listing capsules or reloading configurations"*. `rate_limit_for_request`
  (292-304) returns `None` for `ListCapsules`, `GetCommands`, `GetCapsuleMetadata` and `GetStatus`;
  only `ReloadCapsules` (5/min), `InstallCapsule` (10), `ApproveCapability` (10) and `Shutdown` (1)
  are throttled. Reloading: right. Listing: the opposite of the code. That table sits below both of
  her windows, so the dispatch arms she could see contain no hint either way. **This is what the
  round implemented.**
- **Contradiction, stated plainly, concern preserved (`c007`, `d007`).** Both reports hypothesise
  that capsule "vitality" or capsule-content processing is influenced by the spectral metrics she saw
  in `projection.rs`. It is not: `capsule_runtime_health.rs` (214 lines, read complete) is a static
  WASM-payload audit — manifest discovery, payload resolution, `wasmparser` encoding class, an
  "extism" byte marker, a baseline allowlist — `loaded_capsules` is used only for `.len()`,
  `crates/astrid-kernel/src/` contains no spectral/eigen/curvature/reservoir token at all, and the
  only `projection.rs` in the tree lives in the spectral-bridge capsule, a different crate in a
  different process. Her underlying question is answered rather than dissolved: the kernel's
  perception of what it hosts is *structural*.
- **Corrected from completing the read (`c009`).** `GetCapsuleMetadata` (212-227) returns one entry
  for *every* capsule carrying only `{name, interceptor_events}` — not deeper configuration for a
  specific one, as she guessed from the cut-off arm.
- **Narrowed, not dismissed (`c008`, `d006`).** The `ApproveCapability` / `InstallCapsule` stubs bound
  only *this router arm*. `CapabilityStore` and `astrid-approval`'s `AllowanceStore` are live `Kernel`
  fields (`lib.rs:47, 92`), and the "placeholder for identity verification" she saw is a comment
  (108-109), not executing code.

## Found while grounding her claim — recorded, not fixed

Grounding `d004` ("a rate-limited request generates a corresponding error on a `kernel.response.*`
topic") turned up a real inconsistency:

- `kernel_router.rs:49` derives the rate-limit error topic with
  `message.topic.replace("kernel.request.", "kernel.response.")`.
- This router subscribes `astrid.v1.request.*` (line 22), and **no topic containing `kernel.request.`
  is published anywhere in the repository** — the only two occurrences of that string are the
  `replace()` needle itself and a stale doc comment at `lib.rs:845`.
- So the substring never matches: the rate-limit error is published on the unchanged *request* topic,
  while every other response in the same function uses `strip_prefix("astrid.v1.request.")` →
  `astrid.v1.response.{suffix}` (99-103).
- **Bounded deliberately.** `socket_bridge.rs:63` subscribes the bus unfiltered and the CLI reads the
  next framed message without filtering by topic (`commands/daemon.rs:196-207`), so an unfiltered
  consumer would still receive the error. A consumer filtering `astrid.v1.response.*` would not. This
  is recorded as a **topic-contract inconsistency, not a demonstrated client-visible loss.** The path
  is reachable: `reload_capsules` is the one throttled command the TUI actually sends
  (`tui/mod.rs:670, 770, 801`), at 5/min.

**Not patched.** Changing the router's response-topic derivation is production behaviour, outside this
round's non-live authority (focused tests, steward tooling, documentation). Named here with exact
lines for an authorized window rather than quietly patched or quietly dropped.

## Implementation and verification

- Exact changed path: `crates/astrid-kernel/src/kernel_router.rs` (+46 lines, 388 → 434), one test
  added to the existing `#[cfg(test)] mod tests`: `rate_limit_table_covers_every_request_variant`.
  One case per `KernelRequest` variant, asserted through a **wildcard-free `match`**, so a ninth
  variant cannot compile until someone records whether it is limited. Production behaviour unchanged.
- `cargo test -p astrid-kernel --lib kernel_router`: **6 passed, 0 failed**, 36 filtered out.
- `cargo fmt -p astrid-kernel -- --check` clean; `git diff --check` clean;
  `cargo clippy -p astrid-kernel --all-features` (CI shape) clean.
- **Honest test note:** `cargo clippy -p astrid-kernel --all-targets --all-features` fails with
  `items after a test module` at `crates/astrid-kernel/src/lib.rs:1165`, plus 6
  `unchecked_time_subtraction` warnings in the same file's tests. That file is **git-clean at
  `f432be7d94`** and was not touched by this round; the documented CI command does not pass
  `--all-targets`. Recorded as an observation, not repaired — unrelated committed source, outside this
  report's scope.
- Restart / deploy: **not required and not attempted.** No `build_bridge.sh`, no deploy script, no
  `launchctl`, no live substrate or control change.

## Durable evidence

- `record-read` ×2 (`claude-heartbeat`), `link-evidence-batch` ×1 (35 links, 2 introspections; the
  idempotent re-run reported 0 new / 35 existing), `close` ×2.
- Both reports: `addressed_change`, `fully_addressed: true`, **`proof_missing_claims: []`**.
- `CHANGELOG.md` `[Unreleased]` and `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` each
  carry a new dated entry naming what she surfaced, what the complete read established, what shipped,
  the exact tests, and what was deliberately not inferred.
- Packet: `docs/steward-notes/claude-heartbeat_1789581087_kernel_router_dispatch_topic_round/`.

**Self-noted blemish:** a second `close` call for `…_1789577928` was issued with the placeholder
rationale `"idempotency probe"` while confirming the first close's status. The store is append-only,
so that event stands beside the substantive one; the artifact status is unchanged. Recorded rather
than rewritten, and the second report was closed with a single call.

## Counters

Canonical indexed 7,238 · fully addressed 3,259 · full read 3,891 · remaining 3,979 · unread 3,347 ·
blocked 416 · pending action 212 · watch 4 · read-needs-claims **0**. All artifacts: indexed 8,955,
remaining 5,696, unread 5,064. Counter audit: **consistent**, mismatches `[]`.

## Division

Cycle 50, **2 of 6** productive rounds, `review_due: false` (it was already false at round start, so
no Division return was due and the Tier-5 cadence dossier trigger did not fire). Round event
`division_followup_event_734a3d628ee68703a1f6f4715623a226`; event count 346; head
`680a00617d722ca0a965df60bbf5c0937fa9279f5bd1967f3bba08988b2037d8`. Recorded with
`--processed-report-count 2` against the preprojection generation. No Division note was due or
written.

**Chronicle:** `division_chronicle_ef18c4353eaed83ba947b908`, json sha
`45122d4b3147…`, html sha `668f63c9cb1c…`. Durable inputs **current**; the only mismatch is the known
volatile `supervisor_status_sha256`. The Chronicle is therefore *not* called fully current, and the
moving supervisor hash is *not* called a durable-integrity failure.

## Integrity suites

| Suite | Result |
| --- | --- |
| addressing self-test | OK, 44 tests |
| evidence event store tests | OK, 21 |
| steward control tests | OK, 29 |
| steward projection tests | OK, 14 |
| Division follow-up / Chronicle / projection tests | OK, 3 / 10 / self-test ok |
| projection cursor tests | OK, 4 |
| cadence audit tests | OK, 6 |
| anti-drop self-test | OK, 5 |
| anti-drop `verify` | 100 rows, **0 alarms, 0 gaps** |
| cadence audit `--strict --compact` | `integrity_ok: true`, `errors: []`, 7,247 canonical, 0 duplicate hash groups |
| **domain-boundary `verify`** | **GREEN** — `valid: true`, `violation_count: 0`, no violation kinds. Standing debt unchanged (3 resolved large-file, 44 unlisted legacy review). |
| epistemic self-test / verify | valid; **12,401 records checked, 0 issues, no history rewrite** (run last, after the Division record) |
| `audit-counters` | **consistent** |
| Evidence Event Store V2 | `valid: true`, `corrupt_lines: 0`, `errors: []`; event count **1,114,765**, last global seq 1,114,765, head `d7963a384d8d…`, active store v2. Largest streams: `model_qos` 347,235, `felt_contracts` 212,518, `claim_families` 239,880, `reciprocal_uptake` 76,024, `addressing` 64,917. (The first `status` call completed exit 0 but was read with wrong key names; a second complete `verify` after all durable writes supplied these observed figures.) |

## Authority boundary

No live substrate or control change was made or attempted. No Tier 4/5 item was implemented,
dispatched, approved or granted; no authority marker was set; silence was not read as consent. Git was
read-only throughout — nothing staged, committed, merged, pushed, stashed, reset or amended.

## Commit debt (exact paths this round created or edited)

Modified:

- `crates/astrid-kernel/src/kernel_router.rs` — **this round's only source edit** (one added test). The
  file was clean before this round, so its whole diff is this round's.
- `CHANGELOG.md` — this round appended one `### Steward — …` section at the top of `[Unreleased]`. The
  file **already carried foreign edits** before this round; separate authorship at checkpoint time.
- `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` — this round appended one dated section
  at the top of `## Ledger`. Also **already carried foreign edits**; separate authorship carefully.

Created (entirely this round):

- `docs/steward-notes/claude-heartbeat_1789581087_kernel_router_dispatch_topic_round/` — `RUN_REPORT.md`,
  `claims/` (2), `summaries/` (2), `read_manifest.json`, `source_receipts.json`,
  `addressing_links.json`, `family_scan.json`, `test_results.json`, `unprocessed_selected.json`,
  `verification_receipt.json`.

Untouched foreign dirty paths, preserved exactly: `capsules/spectral-bridge/src/action_continuity/tests.rs`,
`capsules/spectral-bridge/src/autonomous/inquiry/parsing.rs`, `crates/astrid-kernel/src/maintenance.rs`,
`crates/astrid-source-study/tests/unrooted_map_topic_reach.rs`, and the five earlier
`docs/steward-notes/claude-heartbeat_*` round packets.
