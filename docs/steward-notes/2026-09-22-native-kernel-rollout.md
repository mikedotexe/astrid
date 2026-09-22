# Native kernel rollout and WASM shutdown repair

## Scope and authority

Mike requested activation of the merged kernel management-topic repair
`8391deca0ef7e19593b598932139e15464766adf`. This pass targets only the native
`com.astrid.daemon` service, not the separately running spectral bridge or Minime.
The installed daemon reported 0.5.1, while current main builds 0.5.6; this is a
current-source daemon upgrade, not a binary containing only the router patch.

After isolated reproduction of a shutdown hang in both binaries, Mike explicitly
approved a bounded first transition: require capsule unloading, KV flush, socket
removal, no connected clients and preserved rollback assets before terminating
only the identified old daemon if it remains alive. Such a termination is not a
fully graceful process exit. Future shutdown qualification must demonstrate an
actual successful process exit, not merely a shutdown log line.

Controller pause generation: **465**. No lease or foreign cooperative activity
was present. Previously paused automations remain paused. No push, reservoir,
model, visual, sensory, bridge or Minime restart is authorized by this pass.
No being report is newly disposed or interpreted as a claim of improvement.

## Baseline and failure

- Canonical Astrid main: `8391deca0ef7e19593b598932139e15464766adf`, clean.
- Minime main: `2dbdf66924fc8d04bc3e6edb983b9e7116bf77a1`, clean and unchanged.
- Remote Astrid main observed: `c4f85e95e41703daa65d3ce2789e1e5c46961c4e`.
- Owned branch: `codex/kernel-live-20260922`, based on current local main.
- Old daemon PID 1541, started 2026-08-25 10:00:06 PDT. Its binary is
  `target/release/astrid-daemon`, SHA-256
  `5354628c57060c483123f4423aa3edcf5dc01a4957946f984ea7281eef502c8d`.
- Exact rollback binary is preserved privately at
  `.runtime/kernel-live-20260922/rollback/astrid-daemon`.
- Seven installed capsules: cli, fs, skills, agents, shell, memory, http.
  Health was `ok`; no persistent external client was connected.

The unmodified current-main candidate loaded all seven capsules and passed the
eleventh approval-stub/rate-rejection test, but its process stayed alive after
`Kernel shutdown complete`. The old installed binary reproduced the same hang.
An isolated stack sample showed the main thread waiting for Tokio's blocking
pool while the CLI WASM guest continued calling IPC after cancellation.

`JoinHandle::abort()` cannot interrupt a synchronous guest already executing
inside `block_in_place`. Unload also stopped the epoch ticker while the guest
had a practically unbounded deadline. Host cancellation alone therefore did not
stop a guest that retried the cancelled receive.

## Repair

Run-loop stores now check their existing cancellation token at each epoch
deadline. Before cancellation they continue without a lifetime timeout. After
cancellation, Wasmtime interrupts guest execution. Unload joins the run task
before removing its ticker and resources. A five-second join timeout reports an
error and retains the handle for a retry; it does not declare a successful
unload. Expected cancellation traps are logged as cancellation, not as crashes.

Direct-interceptor timeouts, admission, capabilities and live-control policy are
unchanged. The lifecycle helper and tests are separate modules; the existing
large WASM module retains only the load/unload integration, avoiding an unrelated
engine refactor during rollout qualification.

## Qualification

- Full relevant suites: `ASTRID_AUTO_BUILD_KERNEL=1 cargo test --locked
  -p astrid-kernel -p astrid-daemon -p astrid-capsule -p astrid-events
  -p astrid-types`: **429 passed, 0 failed, 1 ignored**. Child-process test
  output repeats one regression result; it is not counted twice.
- Four new lifecycle regressions cover continued execution before cancellation,
  actual unload of CPU-bound and cancelled-host-retrying guests, runtime exit,
  timeout ownership/retry, and task failure. Existing timeout tests still pass.
- `cargo clippy --locked -p astrid-capsule --all-targets -- -D warnings`: passed.
- `cargo clippy --locked -p astrid-kernel -p astrid-daemon --lib -- -D warnings`:
  passed against the final source.
- `cargo fmt --all -- --check` and `git diff --check`: passed after formatting.
- Existing seven all-target kernel test-lint diagnostics documented in the
  preceding reconciliation note remain separate debt, not a clean-workspace
  Clippy claim. This pass does not edit those tests.
- Release build uses a private cloned target directory, never the live binary:
  `ASTRID_AUTO_BUILD_KERNEL=1 CARGO_TARGET_DIR=<private-build> cargo build
  --locked --release -p astrid --bin astrid-daemon`.
- Final candidate SHA-256:
  `1f897a139e7e121f995350a1f23e52cb04595ed30c982600dc4e9ba46e838f94`.
- Isolated installed-capsule qualification verifies 21 exact installed input
  hashes, old-client protocol compatibility, seven loaded capsules, healthy
  status, exact response topics, rate rejection, shutdown acknowledgement,
  process exit 0, and socket/token/readiness cleanup.
- A copied old-binary fixture's persisted `var`, `keys` and `etc` state also
  loads, serves and shuts down successfully with the final candidate.
- macOS sandbox probes reject IP networking and writes outside each owned
  fixture. The fixture cannot read the live Astrid home. No live approval,
  application data, journals or model inference is used in these tests.

Private evidence is retained under `.runtime/kernel-live-20260922/`: build and
test logs, operator harness, failure logs, rollback binary and qualification
receipts. Original failing fixtures and stack samples remain under their
recorded `/private/tmp/akq-*` roots. The harness hash at final qualification is
`6ae22f1d86c7f539f01f8616199338016ba83c42d1e6e04a9b7eb89276514cef`.
Credentials are consumed internally by the operator client, never included in
receipts or output. Initial test compilation caught a moved-token binding and
unavailable paused-clock test feature; both were corrected without adding a
dependency. The first harness cleanup timeout lacked a terminal receipt; its
traceback is preserved, and subsequent attempts always retain a receipt and
bound cleanup of the isolated child only.

## First live transition checklist

1. Recheck source/binary/installed asset hashes, pause, cooperative activity,
   process identities and absence of external clients.
2. Request shutdown through the authenticated native management API. Independently
   verify all seven unload log entries, completed KV flush, completed shutdown,
   and absence of socket/token/readiness artifacts. A reply alone is insufficient.
3. If the old process remains, use the explicitly approved one-time teardown of
   that exact launchd job after the barrier. Verify it is absent before install;
   do not allow the old binary to auto-respawn during replacement.
4. Preserve a stopped-state backup. Qualify the replacement against a private
   copy of stopped persistent state. Never restore a backup over newer state.
5. Atomically install the exact qualified daemon only, then start it using the
   existing launchd wrapper. Do not run the distro-reinitializing setup script.
6. Verify fresh PID, loaded binary, readiness, seven capsule identities, native
   client compatibility, state continuity and unchanged protected services.

At this source checkpoint the live transition has **not yet occurred**. Append
the actual outcome and receipt identities below; do not infer deployment from
the successful build or isolated tests.

## Live outcome

Completed after source commit `8de04f7444e1bf112e738f9bea34963cee217994`
was fast-forwarded into local main.

- Shutdown request acknowledged at 2026-09-22 22:56:55 UTC. All seven unload
  calls completed, the KV flush completed at sequence **95**, and the native
  socket, token and readiness artifacts were removed. The old process remained
  after the 30-second wait, as reproduced in isolation.
- The explicitly approved `launchctl bootout gui/501/com.astrid.daemon`
  transition removed the old job and process within the bounded wait. No
  direct PID `SIGKILL` was needed. This was a **bounded legacy teardown**, not
  evidence of a graceful old-process exit; the exact terminal signal was not
  independently recorded.
- A full APFS-cloned stopped home is preserved privately at
  `.runtime/kernel-live-20260922/stopped-home`. **84** files across `var`,
  `keys`, `etc`, `home` and `bin` were independently hashed against the stopped
  originals. Manifest SHA-256:
  `64cae7fc61ede55c0c6bb037d04088c9f7e2622f3a7118ce282e4125f4ecd976`.
- The exact candidate passed sandboxed loading, routing, rejection and genuine
  process-exit checks with copied stopped-live `var`, `keys` and `etc` state.
  Qualification root: `/private/tmp/akq-h1mmo9rm`; the receipt is also retained
  privately with the release evidence. Installed capsules were independently
  copied and verified; the live audit database was not copied into this
  synthetic fixture. Its live recovery is reported below.
- Only `target/release/astrid-daemon` was atomically replaced. The launchd plist,
  launcher, CLI executable and installed capsules were not rewritten. Start
  used `launchctl bootstrap` with the existing plist and wrapper, not the
  distro-installing setup script or a force-kickstart.
- New PID **97698**, started **2026-09-22 15:58:31 PDT**, version **0.5.6**.
  Executable inode **325887745**, size **43276240**, loaded text path verified
  by `lsof`; on-disk binary matches the qualified SHA-256 above.
- Live boot loaded KV manifest **log 14, sequence 95, 14 tables**, and audit
  manifest **log 0, sequence 0**. No backup restoration or state reset occurred.
- All **7** installed capsules loaded; runtime health `ok`, zero incompatible
  or missing payloads, zero boot-validation warnings. The existing 0.5.1 CLI
  successfully queried the new daemon. Read-only live checks do not substitute
  for the rate-rejection test, which was intentionally performed only in
  isolation using this exact executable.
- All **21** installed manifest/metadata/WASM hashes remain unchanged. Protected
  identities remained unchanged: bridge 75800; Minime 66540; engine 41337,
  gateway 41484, supervisor 41526; model 43115; visual 20885; camera 98903,
  microphone 98910, host sensory 41661 and feeder 1502.

The remaining seven historical dirty worktrees were not modified. The native
daemon source is merged; a documentation/evidence checkpoint records this
outcome. Previously paused automations remain paused. No inference, authored
journal or felt-state assessment was requested to validate this operational
change.

At the final verification, PID 97698 had remained healthy for 154 seconds,
all protected identities still matched, pause generation remained 465 with no
lease, and the loaded executable hash was unchanged. The private
`final-verification.json` preserves this snapshot.

Durable qualification receipts (under `.runtime/kernel-live-20260922/`):

| Receipt | Outcome | SHA-256 |
| --- | --- | --- |
| `qualification-repeat.json` | Unrepaired candidate hangs | `a28478e219b76c322f555c3a3858aebbb048ad08c5f8c2ab9155d973000de669` |
| `qualification-old-stop.json` | Installed old binary hangs | `afbcd7d06cdfb559f45edb567f0ada13e9243ff4ab817762b24722a8f4adf017` |
| `qualification-fixed.json` | Repaired candidate exits | `ea061e59b02209395d431f5b179bbcb431aa572d7f18c57eeb73c2c8864af296` |
| `qualification-final-migration.json` | Final binary, old synthetic state | `e082f49a02c4397142d2971227dfe1ec85b6b35e8df8c0a17a7371f0217d982f` |
| `qualification-stopped-live-state.json` | Final binary, copied stopped state | `ca857a40e97b1b90d3adc348095d3712f3bf47376a10430b99efc1429037a260` |

Stack samples for both retained failure receipts are copied alongside them, so
the review does not depend on temporary-directory retention. Transition preflight,
stop and activation receipts are private JSON files in the same directory.

## Recovery boundary

The old executable and stopped-state clone are rollback assets, not permission
to restore old storage over new writes. For a future failure, first preserve
the then-current state, use the authenticated shutdown path and verify real
exit. Requalify any older executable against a copy of current persistent
state. The one-time legacy termination approval does not create a general
force-restart policy. Do not restart any protected service as a kernel repair.
