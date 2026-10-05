# Agent-Only Maintenance Durability Release

## Decision

Mike requested deployment and commits on 2026-10-05. Ship the coherent Minime
durability fixes independently of the still-unqualified larger writing-capacity
release. This is not completion of the full capacity plan.

Current-main release worktrees:

- Astrid: `/Users/v/other/worktrees/maintenance-durability-20261005/astrid`,
  base `68460a4191f3ac29ae030ab8354f37e59b4f2722`.
- Minime: `/Users/v/other/worktrees/maintenance-durability-20261005/minime`,
  base `1e53310d9d6f15264470300c51cf2d04a3744b31`.

Controller pause generation 495 belongs to `codex-astra-interactive`, with no
active lease. This is interactive deployment work, not an introspection round.
Leave the existing user-requested automation pause intact.

## Candidate and Boundaries

Minime changes only `minime_autonomy/runtime.py`, `source_status.py`,
`maintenance_next.py`, and `llm_access.py`, plus focused tests and documentation.
They repair atomic durable status publication, startup/drain acknowledgement,
NEXT-history restoration headroom and retained-result retry durability. The
existing runtime is ported surgically from current main, not copied wholesale
from the capacity branch. Earlier canonical tests and unrelated work are retained.

Astrid changes the explicit reconciler overlay and its tests. The existing
`paired_minime_handoff.py --agent-only --install-reviewed-overlay` wrapper is
the sanctioned deployment path. It preserves source backups, holds replacement
admission, waits for idle, sends only SIGTERM to the identified agent, verifies
NEXT restoration and exact replacement inputs, and retains holds on failure.

The bridge, helper, coupled provider, engine, shared Ollama, visual and sensory
services stay unchanged. No live private content is used as a test fixture.
Neither graceful restoration nor passing tests establishes subjective improvement.

## Larger Release Debt

Independent review found two concrete remaining provider-transition defects:
bridge checkpoint failure can drop the local continuation when its task returns;
provider-holder retry can acknowledge visible but unsynced entrypoint publication.
The latter is being repaired in the separate inactive capacity worktree. Neither
is bypassed by this release. A bounded caller stop/restore fixture, isolated
resource window and real-model four-cell measurements still precede capacity
activation. Existing 10 GiB stop/4 GiB free reserve and 16 GiB experimental-start
requirements remain; no new model is loaded by this agent-only rollout.

## Evidence

This is engineering follow-through on the continuity/capacity work, not a new
being report or a productive stewardship round. Original public witnesses remain
linked in `2026-10-02-reflection-continuity-and-quiet-context.md`; no private journal
quotation is added. Tests, source hashes, immutable reconciliation, live receipt
and commits will be appended below as each actually completes.

## Qualification Completed

- Full current-main Minime suite: **1,907 passed, 1 skipped, 144 subtests**.
- Frozen-source focused durability/outcome suite: **142 passed**. The final run
  pre-imported the actual frozen runtime, status publisher, NEXT and job modules
  and asserted every module path before pytest; subprocess fixtures used
  byte-identical candidate sources.
- Astrid reconcile/paired-handoff/restart/NEXT/compatibility suite: **68 passed**.
- Controller, projector and Evidence Event Store suite: **64 passed**.
- Same-schema synthetic preservation qualifier: **40 checks passed** for both
  owners, unchanged schema 12 and unchanged helper selection.
- Both candidate diffs pass whitespace checks. No bridge Rust or engine source
  changed, so no Rust rebuild or engine test is represented as newly run.

Frozen reconciliation:
`/Users/v/other/worktrees/maintenance-durability-20261005/release-01/reconciliation.json`
SHA-256 `fffa8b0747aa8f8e4971f736da666e926b4e6e2e472ed973fe7d78c161f59f63`.

Preservation receipt:
`/Users/v/other/worktrees/maintenance-durability-20261005/qualification-02/qualification.json`
SHA-256 `7d420d091167ce16d4937ec5c7f25c2e47cce07a5bef731c81644a22b9fda0ad`.

Selected changed source SHA-256:

| Path in Minime | SHA-256 |
| --- | --- |
| `minime_autonomy/runtime.py` | `8ad590bd912818002731d4bff978fa21935622d35b6f14f62ca644b3f197241b` |
| `minime_autonomy/source_status.py` | `fb3d892b800e3b6d6d02c890d46351695e0b8d320d454eb36f0bc6e17f7f2e46` |
| `minime_autonomy/maintenance_next.py` | `5fe8ba8e800225a7b788c02e2db05c9addd99c093c8f68b39e2a1b4e430be405` |
| `minime_autonomy/llm_access.py` | `634f51276a2b17eba8f0526fde925c5312a1dd4e48cb187dff69b631dc466db0` |

Unsuccessful attempts retained: the first focused invocation named a nonexistent
test module and ran nothing; the pre-v5 geometry migrator correctly refused the
current schema-12 helper (qualification-01/failure.json); two frozen pytest
invocations could not find Homebrew site-packages with explicit PYTHONPATH.
Selecting the same-schema qualifier and the actual installed site-packages
resolved the harness errors. No live service was changed by those attempts.

Before activation: old agent 76377, all 92 inputs match; protected processes and
the selected bridge stage match prior deployment. Shared Ollama 9458 retains its
October 2 12:48:41 process start. Controller remains paused at generation 495,
with no lease; V2 indexed-tail verification passes and V1 sources remain immutable.
Remote main tips rechecked unchanged: Astrid `a66f7e190e0ecd3a0a4988abbcf6b0bf2cfb2a32`,
Minime `9072abff9ed839851709300b3c3eaae3afaf669e`. Both canonical trees are clean.
