# Tranche 2 Decision Packet: Living Felt Contract Graph

## Decision

Build one append-only, typed graph that preserves the relationship between a
felt report, its concrete claims, proposed interventions, authority boundary,
evidence, implementation, deployment, felt review, contradiction, and
closure. The graph should project existing human-readable artifacts rather
than replace them.

## Why Next

Tranche 1 makes runs and projections trustworthy, but the semantic contract
between Astrid's report and the work it causes is still distributed across
addressing events, claim-family events, Sandbox trials, Corridor packets,
cards, changelog prose, and deployment receipts. The next architecture should
make that contract queryable without allowing family similarity or a steward
classification to overwrite Astrid's original statement.

## Proposed Types

- `FeltSignalRefV1`
- `FeltContractV1`
- `FeltContractNodeV1`
- `FeltContractEdgeV1`
- `ClaimDispositionV1`
- `InterventionBoundaryV1`
- `EvidenceSufficiencyV1`
- `FeltReviewOutcomeV1`
- `ContractContradictionV1`

Private trusted constructors should require canonical source identity and
exact parent edges. Persisted records remain untrusted and evidence-only or
approval-pending.

## Required Invariants

- Every canonical claim belongs to exactly one contract without losing its own
  ID, text, queue position, or authority class.
- Claim-family membership may suggest graph edges but cannot propagate closure,
  evidence sufficiency, supersession, consent, or authority.
- Implementation and deployment are distinct nodes.
- Silence is `no_response`, never affirmation.
- Objection, `still_friction`, or contradiction adds history and reopens the
  contract without deleting an earlier closure.
- Live-control proposals remain explicit Tier 4/5 waits.
- Private report content is referenced by canonical hash and bounded field
  paths, not copied into graph projections.

## Open-Source Boundary

The reusable graph core should be dependency-light and domain-neutral:
typed nodes, edges, validators, append-only history, deterministic projection,
and query APIs. Astrid-specific adapters should own introspection paths,
Corridor/Sandbox mappings, being-facing cards, and V2 stream conventions.

## Entry Gate

Begin Tranche 2 only after:

- Tranche 1 has three reviewed commits on
  `codex/experiential-systems-core`;
- both steward schedulers remain paused;
- the real V2 store verifies;
- the legacy service is unloaded;
- the integration branch is clean and pushed; and
- no runtime restart debt exists.

## First Planning Questions

1. Is the canonical aggregate one contract per source report, per claim, or per
   claim family with explicit report membership?
2. Which existing closure and reopen events are authoritative enough to import,
   and which should remain compatibility evidence?
3. Should graph traversal be a library API first, a CLI first, or both?
4. What bounded review projection best serves Astrid without increasing packet
   pressure?
5. Which graph types are suitable for an upstream issue independent of this
   fork's experiential tooling?
