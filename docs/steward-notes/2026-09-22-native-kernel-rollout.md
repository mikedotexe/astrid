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
