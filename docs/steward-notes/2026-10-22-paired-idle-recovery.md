# Paired Idle Refusal Recovery

The approved Brave rollout's `paired-rollout-01.jsonl` completed its reviewed
source installation, then refused the pre-signal boundary at
2026-10-23T00:12:44Z. Minime's status age advanced from 47.586 to 58.692 seconds
during an 11.101-second final check, leaving its 60-second idle interval. The
receipt explicitly records no signal. The old agent and bridge remained running,
and the owned replacement launch hold was retained. No authored state was reset.

The original installed-state recovery accepted a receipt ending immediately
after installation, but not that same complete installation followed by the
ordinary idle refusal. This operator-only patch recognizes exactly three extra
records: the bound wait, an explicit no-signal idle refusal, and a matching
failure retaining the hold. It rechecks PID/start, installed source identities,
configuration and protected services. Any unknown suffix, signal intent, changed
boundary, forced outcome or missing no-signal evidence refuses recovery.

Existing source-backup, stage, qualification, original-inode, receipt-lock and
hold-adoption checks remain. Recovery still uses normal idle/drain, one-signal,
checkpoint, pending-NEXT and readiness verification. It never edits or truncates
the original receipt, reinstalls sources, cancels generation or relaxes the idle
window. Repeated unsignaled recovery retains its existing exact-receipt protocol.

Developed independently from the frozen Brave source in
`codex/search-handoff-recovery-20261022`. Only operator tooling, synthetic tests
and documentation change. The already-qualified stage 02 binaries and source
tree remain untouched; no rebuild or policy rebinding is required. This does not
change MLX, capacity, providers, the engine, sensory services or paused automations.

The tests cover both pre-signal and timeout refusal, exact history preservation,
and rejection of altered process/source/configuration/protected identities,
uncertain signal state, lost holds, forced termination and unexpected events.
Actual test and recovery outcomes are recorded in the search-delivery closeout.

## Native Accepted-Work Follow-Through

The second attempt also refused before signaling: status age advanced from
42.838 to 55.186 seconds during a 12.348-second final check. Both services
continued; no work was canceled. Repeating the idle strategy was not reliable.

The paired operator CLI now offers `--finish-accepted-work` only with installed
recovery and an explicit prior qualified launch-source packet. It uses the
existing native-v2 finish protocol, not an inferred idle exemption. The loaded
105-input identity matches `freer-exploration-20261021/minime-reconciliation-06`.
The exact successor receipt and ownership checks still apply. After the one
SIGTERM, accepted work must finish, its durable outcome must verify, the old
process must acknowledge drained exit, and the native NEXT seal must verify.
The bridge's existing sanctioned transition must then return a verified graceful
activation receipt; only that bridge PID may change among protected services.
Minime readiness must restore the same NEXT seal. No force, cancellation,
unreviewed source installation, queue replay or backup restoration is added.

The original agent-only CLI and invariants remain. Paired mode is explicit;
unexpected transition scope, missing receipt, forced/undrained bridge transitions,
wrong old PID, unrelated service drift and changed readiness boundaries refuse.
Synthetic tests exercise both paths and exact CLI routing. An initial test
invocation omitted `MINIME_SOURCE_ROOT` in this isolated worktree and failed on
missing sibling fixtures, not a runtime failure; the corrected invocation binds
the canonical Minime launcher explicitly. No model or live data fixture is used.

Qualification: 277 tests passed across native finish, paired recovery, agent
restart, NEXT handoff, deployment wrappers, bridge activation/staging/drain,
source reconciliation and steward control. Receipt:
`/Users/v/other/worktrees/search-delivery-20261022/paired-finish-tests-02.log`.
