# Issue Draft: Portable Cooperative Steward Sessions

## Target

Astrid `v0.10.1`.

This is an issue-ready design dossier, not a pull request. `CONTRIBUTING.md`
requires an issue, maintainer triage, assignment, and the appropriate
contributor approval before implementation is proposed upstream.

## Problem

Long-running external maintenance or evidence-projection sessions need a
portable way to:

- pause durably;
- claim one cooperative lease;
- receive a stop request;
- record bounded run receipts;
- detect accidental git identity changes; and
- recover from a dead session.

This lifecycle should not depend on one model provider, desktop client,
scheduler, home-directory layout, or platform hook system. It must not acquire
git, deployment, approval, or runtime-control authority.

## Proposed Upstream Scope

Introduce a small standard-library Python package with:

- script-relative TOML configuration;
- owner-only atomic state;
- opaque 256-bit lease tokens with persisted hashes;
- `status`, `verify`, `pause`, `resume`, `begin`, `heartbeat`, `finish`,
  `run`, and `reconcile`;
- cooperative `SIGINT` for wrapped subprocesses;
- read-only repository identity receipts; and
- scheduler examples for launchd, systemd, and cron.

The upstream lifecycle should expose narrow protocols for evidence validation,
event sinks, and optional pre/post projectors. The default implementation can
use local owner-only receipts without requiring this fork's event store.

## Reusable From The Fork

| Surface | Reuse posture |
|---|---|
| `scripts/steward_control/model.py` | Atomic owner-only writes, canonical hashing, locks, token generation |
| `scripts/steward_control/config.py` | Portable TOML/environment/CLI path resolution |
| `scripts/steward_control/lease.py` | Pause generation, exclusive cooperative lease, stale recovery |
| `scripts/steward_control/git_state.py` | Read-only repository identity and policy comparison |
| `scripts/steward_control/activity.py` | Read-only recent-tree activity evidence |
| `scripts/steward_control/executor.py` | Shell-free subprocess adapter and graceful interruption |
| Lifecycle tests | Temporary-repository tests for authority and git non-mutation |

Before upstreaming, `controller.py` should depend on small `EvidenceVerifier`,
`ReceiptSink`, and `ProjectionHook` protocols so the reusable lifecycle has no
dependency on fork-specific evidence modules.

## Fork-Specific Surfaces

These should remain in the fork unless the maintainer separately requests the
underlying architecture:

- `scripts/steward_control/projection.py`;
- Evidence Event Store V2 activation and V1 source-log immutability;
- introspection addressing, Sandbox, Corridor, Signal Spine, claim-family, and
  experiment-dossier projectors;
- Astrid/Minime repository topology;
- being-feedback ledgers, cards, and review budgets; and
- live bridge deployment wrappers.

## Security And Privacy

- Persist only lease-token hashes.
- Store no subprocess output, prompt text, private journal content, or raw pause
  reason in canonical receipts.
- Use explicit argument arrays and never interpolate shell commands.
- Start paused and never preempt a live lease.
- Make all receipts witness-only.
- Keep scheduling and privileged actions outside the controller.

## Acceptance

- Python 3.12+ and standard library only.
- Linux and macOS atomic-state and scheduler examples.
- Lease exclusivity, stale recovery, pause races, token secrecy, watchdog,
  crash, duplicate-finish, and idempotency tests.
- A temporary-git-repository proof that HEAD, index, branches, and remotes do
  not change.
- Portability scan rejecting machine paths, provider hooks, and credentials.
- Modules below 1,000 lines.
- MIT/Apache licensing unchanged.

## Maintainer Questions

1. Is a repository-owned Python helper acceptable for this lifecycle, or
   should it live in a separate operations package?
2. Should upstream receipts default to local JSON, the kernel audit log, or an
   injected sink with no default persistence?
3. Which scheduler examples belong in-tree?
4. Does the maintainer want the optional projector-hook protocol in the first
   issue, or a lifecycle-only first contribution?

No upstream branch or pull request should be opened until the maintainer
assigns the issue.

## Follow-Up Boundary

The completed cooperative lifecycle now coordinates a separate, domain-neutral
Living Felt Contract Graph candidate. Its identity, replay, and privacy core is
described independently in
`LIVING_FELT_CONTRACT_GRAPH_PORTABILITY_DOSSIER_V0_10_1.md`; fork-specific
projectors remain outside this steward-control issue.

## V3 Follow-Up: Incremental Projection Runtime

Production-scale validation exposed a second portable lifecycle concern: a
cooperative lease must remain valid while pre- and post-run projectors execute,
and a successful `begin` must not return a token already near expiry. A
separately assigned upstream issue could reuse:

- the lease-renewal guard and poll-based, one-`SIGINT`, never-force-kill child
  runner;
- token-free active-generation status and bounded command receipts;
- immutable input identities, resumable generation journals, and
  dependency-aware checkpoint reuse;
- a rebuildable SQLite offset and idempotency index over an append-only JSONL
  source; and
- atomic latest-generation publication only after final source verification.

The derived index is explicitly disposable and non-authoritative. Its
validation anchors to a fully verified canonical head, reconciles only the
locked tail, and rejects truncation, tampering, offset drift, or counter
disagreement. Generic modules use only the Python standard library, contain no
machine paths or provider names, and remain below 1,000 lines.

The following remain fork-specific and should not be proposed in the portable
lifecycle issue:

- Astrid's source-first step graph and canonical introspection inventory;
- Sandbox, Corridor, Signal Spine, claim-family, dossier, model-QoS, authority,
  and felt-contract schemas;
- migration baselines, felt-review budgets, and being-feedback ledgers; and
- any bridge, Minime, deployment, approval, dispatch, or live-control surface.

Copied-production acceptance over more than 351,000 canonical events passed
the no-input, 20-report delta, and full-rebuild time and memory gates, with
exact full/incremental felt-contract parity and no no-input family or contract
event growth. No upstream pull request should be opened until the maintainer
assigns the lifecycle or incremental-runtime issue and confirms the preferred
canonical event-store boundary.
