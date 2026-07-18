# Issue Draft: Append-Only Experiential Contract Graph

## Target

Astrid `v0.10.1`.

This is an issue-ready portability dossier, not a pull request. No upstream PR
should be opened until the maintainer assigns an issue under
`CONTRIBUTING.md`.

## Problem

Long-lived agent feedback can become scattered across reports, issues,
evidence, implementation notes, deployments, and post-change reviews.
Similarity grouping alone is not a safe aggregate: it can accidentally
propagate closure, evidence sufficiency, supersession, consent, or authority.

A small append-only graph can preserve the concern over time while keeping
each source claim and authority boundary independent.

## Reusable Core

The proposed upstream surface is a standard-library Python package providing:

- deterministic contract, node, and edge identities;
- immutable validated records behind internal builders;
- append-only membership corrections;
- causal-parent ordering and acyclicity checks;
- orthogonal technical, evidence, review, and activity states;
- deterministic projection and bounded traversal APIs; and
- privacy checks that reject raw prose and absolute paths.

The reusable core should accept injected source events and an injected receipt
sink. It should not depend on this fork's workspace layout, V2 streams,
introspection filenames, Minime, or scheduler.

## Fork-Specific Adapters

These surfaces should remain outside an initial upstream proposal:

- canonical introspection addressing and queue positions;
- Sandbox, Corridor, Signal Spine, claim-family, and dossier mappings;
- Astrid/Minime environment receipts and process topology;
- being-facing closure and review cards;
- the `felt_contracts` V2 stream name and source-first DAG step; and
- this fork's Tier 4/5 intervention vocabulary.

## Safety And Privacy

- The graph is evidence-only or approval-pending.
- It cannot grant capabilities, approve work, deploy, dispatch, or control a
  runtime.
- Exact implementation and deployment edges require validated receipts.
- Temporal association is labeled and noncausal.
- Silence never means affirmation or closure.
- Raw report prose, credentials, personal paths, and private source content
  are not persisted.

## Acceptance

- Python 3.12+ and standard library only.
- Modules below 1,000 lines.
- Stable identity across matcher regrouping and membership correction.
- Duplicate assignment, dangling/forward parent, tampering, and authority
  violations fail closed.
- Membership never propagates closure, evidence, supersession, review, or
  authority.
- Deterministic migration, replay, query, and checkpoint tests.
- MIT/Apache licensing unchanged.

## Maintainer Questions

1. Is an experiential contract graph useful as a general audit primitive, or
   should the first issue propose only the domain-neutral identity and replay
   library?
2. Should upstream persistence use the kernel audit log, a repository-local
   JSONL sink, or an injected sink with no default?
3. Should contract review states remain generic, with being-specific labels
   supplied only by adapters?
4. Does the maintainer prefer this as a follow-up to the cooperative steward
   lifecycle issue or as an independent proposal?
