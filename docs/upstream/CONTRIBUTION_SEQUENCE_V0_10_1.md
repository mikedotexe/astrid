# Upstream Contribution Sequence For Astrid v0.10.1

## Baseline And Posture

This sequence was rechecked against `astrid-runtime/astrid` `v0.10.1` at
`4771bab3c33d1bce53186e40d01cf014e2dce666` on 2026-07-18.

The fork is 168 commits ahead of and 232 commits behind the common history with
upstream. None of the five tranche commits should be cherry-picked wholesale.
Each accepted contribution must start from the maintainer's then-current
`main`, contain only the assigned scope, and satisfy the contributor tier and
security-path rules in upstream `CONTRIBUTING.md`.

## Recommended Order

| Order | Candidate | Upstream disposition | Current gate |
|---|---|---|---|
| 1 | Cooperative steward lifecycle | Offer as a small operations helper | Issue #1271 open; wait for assignment |
| 2 | Lifecycle receipt/projector ports | Follow-up only if requested in #1271 | Do not propose independently |
| 3 | Living contract identity/replay core | Separate design issue after lifecycle feedback | Hold; no issue opened |
| 4 | Temporal authority evidence | Contribute design/test evidence to #1229 and #694 | Maintainer-owned security scope |
| 5 | QoS scheduler and delivery receipts | Split by owning repository and protocol domain | Do not submit as Astrid fork code |

## Candidate 1: Cooperative Steward Lifecycle

The first contribution should contain only:

- script-relative standard-library configuration;
- owner-only atomic state and file locking;
- opaque lease tokens with persisted hashes only;
- pause, resume, begin, heartbeat, finish, run, and reconcile lifecycle;
- cooperative subprocess interruption;
- read-only repository identity snapshots; and
- temporary-repository and portability tests.

Before preparing the patch, the fork implementation needs one mechanical
extraction: `StewardController` must depend on injected verifier, receipt-sink,
projection-hook, and source-lag ports. The upstream default should not import
Evidence Event Store V2, introspection addressing, the source-first DAG, or
spectral-bridge workspace conventions.

Explicitly excluded from the first patch:

- `scripts/steward_control/projection.py`;
- `scripts/steward_control/evidence.py`;
- `scripts/steward_control/events.py` unless reduced to an injected local sink;
- all Astrid/Minime projectors and workspace layouts;
- scheduler installation or service mutation;
- deployment, git-write, approval, capability, or live-control actions.

## Candidate 2: Receipt And Projection Ports

This is an optional follow-up to #1271, not part of the first contribution.
It would define narrow protocols for:

- evidence validation;
- bounded receipt emission;
- optional pre/post projection; and
- source-lag reporting.

It should not prescribe a canonical store. Upstream issue
[#693](https://github.com/astrid-runtime/astrid/issues/693) already owns signed
runtime compliance receipts; external steward receipts must remain clearly
separate unless the maintainer deliberately connects them.

## Candidate 3: Living Contract Identity And Replay

Only these modules are plausible reusable inputs:

- `scripts/felt_contracts/identity.py`;
- the domain records and validation in `scripts/felt_contracts/model.py`; and
- focused deterministic identity, acyclicity, correction, and privacy tests.

The current source adapters and projectors depend on fork-only streams,
introspection identities, claim-family projections, Minime, environment
receipts, and being-facing review semantics. They must not enter an upstream
proposal.

This candidate should wait for feedback on #1271 because the maintainer may
prefer a separate library, Rust audit primitive, or no repository-owned
experiential graph.

## Candidate 4: Temporal Authority Evidence

Do not open a competing implementation issue. The reusable findings should be
offered as testable design properties for:

- [#1229 authenticated control connections and scoped delegation](https://github.com/astrid-runtime/astrid/issues/1229);
- [#694 auto-expiring grants](https://github.com/astrid-runtime/astrid/issues/694); and
- the companion upstream authority RFCs named by #1229.

Relevant evidence includes exact process/deployment binding for writes,
bounded clock-skew checks, reserve-before-dispatch, one-shot consequence
budgets, and `outcome_unknown_consumed` crash recovery. The spectral bridge,
pause-generation path, Minime scopes, and V2 projector are fork adapters.

This is a security-critical area. Any code contribution requires explicit
maintainer ownership and the contributor tier required by `CONTRIBUTING.md`.

## Candidate 5A: Model Scheduling And QoS

The scheduler implementation lives in the sibling model service, not in
upstream Astrid. Its natural open-source unit is a provider-neutral bounded
Python scheduler with:

- FIFO shadow mode;
- deterministic priority aging;
- non-preemptive selection;
- in-flight idempotency coalescing;
- disconnect and timeout handling; and
- content-free receipts.

An Astrid upstream proposal, if requested, should be limited to an additive
provider request metadata shape or provider-capsule integration. Fork labels,
MLX process layout, reservoir check-in, and V2 projection remain downstream.

## Candidate 5B: Delivery Receipts And Mutual Address

`astrid-minime-protocol` and the spectral bridge do not exist in upstream
Astrid. Their protocol 1.1 implementation cannot be submitted as an upstream
patch.

A future upstream design issue could instead ask whether `astrid-uplink` needs
a generic optional delivery envelope and same-connection receipt carrying:

- canonical payload hash;
- sender and deployment identity;
- accepted, duplicate, rejected, policy-blocked, or partial outcome;
- bounded deduplication; and
- explicit unknown-delivery behavior with no automatic retry.

Minime sensory payloads, beings, correspondence records, spectral
noncausation, and port numbers remain fork-specific. This proposal should wait
until the maintainer has answered #1271 and the control-connection direction
in #1229 is clearer.

## Pull Request Discipline

For every assigned contribution:

1. Fetch the current upstream tip again.
2. Create a fresh fork branch from that exact tip.
3. Reimplement only the assigned scope; do not merge fork `main`.
4. Keep files below 1,000 lines.
5. Add focused tests and an `[Unreleased]` changelog entry.
6. Run upstream formatting, Clippy, workspace tests, and requested checks.
7. Fill every section of the upstream pull request template.
8. Link the assigned issue and wait for the required approval label.

No branch, pull request, deployment, or runtime change is authorized by this
sequence.
