# Reading and mailbox rollout

September 6, 2026, Pacific time. Operator: Codex `/root`, actor
`codex-activity-rollout`, under Mike’s explicit instruction to complete the
reading/mailbox experience, make it live, and prepare review toward remote main.

## Release identity

| Item | Observed identity |
| --- | --- |
| Verified bridge source | `e4761122e32749b15ef7c2958d2e136cdd5f4613` |
| Retained source checkout | `/Users/v/other/worktrees/astrid-activity-release-20260906` |
| Stage | `.runtime/bridge-stages/20260906-activity-01` within that checkout |
| Native binary SHA-256 | `1c0ab481be0e8a16c5cb15d9084873f61981a60a1928c531463db27417394190` |
| Manifest SHA-256 | `f0cb36afb9e36dc388b9d0fb3330ab6f744ee6f91cf12d5efe2e4bac395d34f1` |
| Source-input SHA-256 | `e1dd1305ea7384d50114ff95c26e6798a6cf7a8ebef1eadbeab0f920b8e2597c` |
| Activation transaction | `eb901a066bc743209af30a8f7574547f` |
| Prior process | PID 56043, retained learning-clock release |
| Started process | PID 42916 |

The source passes 2,100 distinct bridge library tests, strict library/binary/test
Clippy, formatting, and a domain audit with zero violations and no ceiling
increases. Detailed contracts and limits are in the
[runtime implementation record](2026-09-06-reading-mailbox-runtime.md).

## Activation and observed verifier defect

The sanctioned `scripts/build_bridge.sh` staged the clean, committed source and
verified its native manifest. The pre-activation coupled-stack witness passed
(`env_receipt_1788747208933_713000`). The old process acknowledged producer drain,
saved exchange 191422, and exited after SIGTERM. No forced termination was used.
Its exact stopped checkpoint SHA-256 was
`7543817ef598143b2b8decb7556bc3b2af4c70c4e2ce698196015af7ee2855d9`.

The new process passed the checkpoint and signed self-control startup gates.
However, the activation verifier inspected its legitimate transient `starting`
phase just before `running` was published. It treated that phase as a failure,
retained the original failed transaction and restored its owned launch hold.
The running process was left intact; no automatic rollback occurred.

Verification-only recovery is being completed through the sanctioned wrapper.
This record does not yet claim completed live verification or canonical manifest
publication. The original release, checkpoint, signed handoff, failed receipt and
source checkout are preserved for review.

The verifier repair runs from the current operator checkout and does not change
the retained release's source inputs. Subsequent script or documentation commits
therefore do not change the running binary's source identity above. Recovery is
bound to the original failed transaction, selected immutable stage, exact PID and
process start identity, stopped checkpoint, signed state and owned launch hold.
It records a separate witness, including durable intent before releasing the
hold, so publication and release can be retried after interruption. It does not
build, drain, signal, prepare another handoff or restart the running process.

The recovery change passes 48 focused activation tests. Root also ran 48
adjacent stage, drain, release-selection and environment-receipt tests; all pass.
Shell syntax and whitespace checks pass. Independent review verified the crash
and retry cases, process/selection ownership, immutable source checks and
original-receipt preservation. These tests use synthetic process/model surfaces.

## Scope and authority

This rollout gives Astrid the implemented saved-text and durable-mailbox
capabilities. Ordinary receipt of inbox material no longer forces the reading
activity away. No being was asked to choose a demonstration, endorse the change,
or report an experience for this verification. A naturally completed exchange
establishes runtime operation; it cannot establish comprehension or felt benefit.

Minime and model source changes from the preceding tasks were preserved. This
rollout does not deploy their pending adapters, change regulator settings, or
remove the independent usage-saving scheduler pause. Mechanical bookmarks and
accepted-delivery artifacts remain private runtime data and are excluded from
Git and the PRs.
