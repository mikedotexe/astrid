# Tranche 3 Decision Packet: Temporal Agency Boundary

## Decision To Plan

Make time an explicit part of every transition from evidence to pending
approval, granted authority, and live executability. The boundary should
prevent stale, replayed, future-dated, or context-detached grants from becoming
current action authority.

Tranche 3 should continue on `codex/experiential-systems-core` while both
steward automations remain paused. It should not merge `main` or change live
control behavior before the remaining architecture tranches pass.

## Why Next

The first two tranches now make stewardship sessions and felt-contract history
durable. The remaining ambiguity is temporal: source time, arrival time,
recording time, monotonic process time, grant validity, deployment identity,
and one-shot consumption can be individually correct while referring to
different runtime moments.

The next boundary should make those distinctions typed and verifiable rather
than inferred from filenames, wall-clock order, or a currently running PID.

## Candidate Model

Plan private-constructor records for:

- a unified temporal envelope with source, arrival, recorded, monotonic, and
  process-start anchors;
- an authority validity window with not-before, expiry, scope, budget, token
  identity, and clock-skew policy;
- transition receipts for evidence, pending, granted, executable, consumed,
  expired, interrupted, and revoked states;
- a runtime-context binding to source commit, process identity, deployment
  receipt, and safety/fill observation; and
- explicit temporal relations such as exact predecessor, concurrent window,
  delayed observation, and unknown ordering.

Persisted receipts remain untrusted and cannot deserialize directly into
granted or executable wrappers. Evidence tooling remains unable to mint
authority.

## Required Invariants

- Wall-clock order alone cannot establish causation or current authority.
- A grant is valid only for its exact scope, token, budget, lifecycle,
  deployment/process context, and bounded time window.
- Restart, PID reuse, clock regression, excessive skew, expiry, consumption,
  revocation, or changed safety context invalidates live executability.
- Pause and interruption preserve evidence without extending authority.
- Resume requires a fresh transition check; it never silently revives a stale
  grant.
- One-shot consumption and existing safety, rescue, budget, and consequence
  semantics remain unchanged.
- Contract, review, Corridor, dossier, and steward receipts remain witness
  evidence and cannot cross the authority boundary.

## Open-Source Boundary

The reusable contribution candidate is a domain-neutral temporal envelope,
clock/skew validator, transition ledger, and context-binding interface.
Astrid-specific capsule authority, Minime fill/safety inputs, deployment
receipts, and sensory dispatch remain adapters.

## Planning Questions

1. Which clock is authoritative for expiry when a process restarts or the wall
   clock moves backward?
2. What bounded skew is acceptable between external approval time and local
   receipt time?
3. Which context changes require full reapproval versus a fresh executability
   check under the same unexpired grant?
4. Should interruption consume a one-shot grant, preserve it as nonexecutable,
   or require explicit operator disposition?
5. Which existing Rust and Python authority records become compatibility
   readers, and where should the canonical transition ledger live?

## Entry Gate

Begin planning only after Tranche 2 has its three reviewed commits pushed,
the V2 graph and checkpoint verify, the integration branch is clean, both
stewards remain paused, and no runtime restart debt exists.
