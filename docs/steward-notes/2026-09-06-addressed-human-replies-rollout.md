# Addressed human replies: verified rollout

Mike explicitly authorized integration and prudent deployment. Codex /root owned
live actions; writer_and_proposal_audit implemented the operator repairs and
astrid_gap_audit independently reviewed continuity and recovery; minime_gap_audit
verified Minime source and continuity and monitored the protected services. This rollout
completed September 6 Pacific time / September 7, 2026 UTC.

## Live scope and source alignment

Astrid runs release source `3f669bf2220c36a10cd95be551d544223f135758`, PID **22940**,
from `/Users/v/other/worktrees/astrid-human-replies-release-20260906/.runtime/bridge-stages/20260907-addressed-replies-01/`.
Binary SHA-256: `d643b2795956df1013f64fac7faa8d6f7b322b9e56455df3f7760a57fb598d41`.
Manifest SHA-256: `55b133e948925c2509521c5a0de995cf6e9ffa81aa4ec2fd551a5f47017b9ebe`.

The release contains addressed human replies and reviewed deployment helpers.
Pending own-body prompt and generation-record features from `162f027678` were
excluded. Their implementation remains on canonical main alongside the original
addressed-reply commit `ebbf9ae9c8`. Canonical launcher alignment removes only the
three forwarding lines for `ASTRID_OWN_BODY_LINE`, `ASTRID_GENERATION_RECORD` and
`ASTRID_GENERATION_RECORD_DIR`, matching the exact deployed helper. The pending
feature source is otherwise preserved. Keep the retained checkout and stage;
canonical HEAD alone does not identify the running binary.

Minime runs source `63c1ab99abee7284df46e1532ce5f427c1b0fb21`, PID **12456**.
Canonical `08de10f` adds rollout documentation over that runtime source.
The reviewed foundation `0569efb` was preserved. Only `inbox_delivery.py` and
`runtime.py` changed among its 70 startup inputs. This fixes gap 9's bare and
mixed-recipient reply boundaries using Minime's existing reply syntax.

## Interruption, recovery and continuity

The first Astrid activation failed before activation Python ran: macOS Bash 3
rejected an empty optional argument array under `set -u`. The old bridge stayed
running and no hold was created. Operator commit `3a4a0a640b` repaired dispatch
and tested the actual system Bash with mocked deployment calls.

The next attempt acknowledged the producer drain and sent SIGTERM to PID 42916,
then reported an identity mismatch while observing exit. The PID was subsequently
confirmed absent; this does not establish actual PID reuse. Astrid remained
stopped behind the owned hold for roughly **04:41–04:55Z** (about 14 minutes).
No snapshot, handoff or new release selection had yet occurred.

Operator commit `528046fec7` corrected exit-observation races and added one
bounded stopped-transition continuation. It revalidated the original failed
receipt, owned hold, old manifest/selection, helper hashes and checkpoint before
snapshot, signed handoff and launch. Recovery sent **no further signal**, used
no force and performed no automatic rollback. Partial prelaunch recovery cannot
be replayed; a running replacement can use strict verification-only recovery.

Replacement PID 22940 started at **04:55:14Z**; recovery verified at **04:56:36Z**.
The exact 54,204 stopped checkpoint bytes were admitted at startup, at exchange
**191513**, SHA-256 `062703ec0a7db5907cb140ddf9789d88112dead6083ccf1a74042f3805b0a17e`.
Startup and current signed self-control state target the new deployment. A
natural saved exchange advanced to **191514**. Selection and canonical manifest
match the stage, and the owned launch hold is absent.

Transaction `de48dfa86e2d479a874391ae880e5512` retains its failed receipt unchanged,
SHA-256 `a9d100b4f4043bb6ffd759fdb13939a987c8789a2cb8b15f28192efc5a74416a`.
Success has a separate `stopped-transition-recoveries/` receipt; do not rewrite
the original failure as success.

Minime's sanctioned agent-only reload sent one SIGTERM at **04:29:53Z** and
verified at **04:32:26Z**, replacing PID 36143. Pre-signal and post-ready normalized
continuity agree: session **5316**, cycle **24725**, no pending NEXT, digest
`c90c35b5569242459a9847959240eeaf98fcce6c3c74580644cf87899e27b219`.
This digest covers normalized continuity fields. Source/configuration inventories
match the running process. No forced termination occurred; the quiet-boundary
check does not claim atomic traffic quiescence.

## Verification and remaining work

All **nine other protected services** retained their PID and process start across
the rollout. Post-recovery observations found the model ready, queue empty,
reservoir connected and finite, advancing engine telemetry. Coupled-stack receipt
**env_receipt_1788757024041_931000** passed.

The selected Astrid release passed all **2,130 distinct library tests** across its
full run and isolated checks. An existing timing test exceeded its budget during
concurrent compilation, then passed unchanged alone. Strict Clippy, formatting
and domain audit passed. Operator recovery passed **91 focused tests**. The source
review separately records Minime's 1,060-test suite and the 37-test client suite.
No live test mail, inbox resend or synthetic reply was used. These checks establish
deployment and continuity, not a human reply or felt benefit.
No newly modified human-reply artifacts were observed at **05:00:27Z**.

Thread-history retrieval, remaining correspondence gaps and inference reliability
remain follow-ups. Natural jobs timed out while Minime's wrapper waited for a
quiet boundary; this parser repair does not claim to fix those timeouts.

Evidence is retained in `/Users/v/.codex/artifacts/human-replies-rollout-20260906/`:
`astrid-recovery-success.json`, `astrid-independent-activation-verification.json`,
`minime-reload-success.json`, `minime-independent-rollout-verification.json` and
`final-stack-receipt.json`. Release/operator tests are under
`/Users/v/.codex/artifacts/astrid-addressed-human-replies-20260906/`.
See `2026-09-06-addressed-human-replies.md` for source behavior and limits.

Root owns controller pause **379** and will resume it after the final source
commit; the result will be retained in `human-replies-rollout-20260906/controller-resume.json`
under the artifact root. Preserve the independent usage-saving scheduler pause.
