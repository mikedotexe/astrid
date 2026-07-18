# Living Felt Contract Graph V1

## Purpose

The Living Felt Contract Graph preserves the path from a felt report through
claims, interventions, evidence, implementation, deployment, review,
contradiction, and closure. A contract names one stable concern over time. Its
identity is derived from its earliest canonical claim and does not change when
claim-family matching or corrected membership changes.

The graph is a witness surface. It cannot edit source claims, approve work,
deploy software, dispatch sensory messages, or change live controls.

## Trust Boundary

The dependency-free core under `scripts/felt_contracts/` provides immutable
validated records and deterministic identities. Trusted records can be
constructed only through internal builders. Persisted JSON is replayed as
untrusted input and must pass authority, hash, membership, and parent checks.

Astrid-specific adapters are separate:

- `sources.py` anchors canonical claims and shared bounded references;
- `source_history.py` maps addressing, Sandbox, Corridor, Signal Spine,
  claim-family, and dossier history;
- `source_deployments.py` distinguishes exact receipt lineage from labeled
  temporal association; and
- `projector.py` replays append-only events into query and review views.

No raw introspection prose or private source content enters graph events.
Repository paths are relative references; external paths are represented only
by hashes.

## Identity And Lifecycle

Each claim has exactly one current contract membership. A second initial
assignment is invalid. Only an append-only correction can move a claim, and it
must name the actual prior contract. The source and target contracts retain
their own historical review, evidence, and technical states.

Technical disposition, evidence sufficiency, felt review, and activity are
orthogonal. Technical verification or administrative no-action cannot imply
felt closure. Only an explicit `felt_confirmed` review closes a contract.
`no_response` may quiet duplicate delivery after a completed opportunity but
never affirms, waives, approves, or closes. Objection, `still_friction`, and
contradiction reopen immediately without deleting prior history.

Exact `implemented_by` and `deployed_by` edges require validated receipts.
Older deployment evidence is labeled `temporally_associated_deployment` and
is excluded from the causal parent chain.

## Storage And Projection

Canonical graph events live in the `felt_contracts` Evidence Event Store V2
stream. The projector atomically publishes:

- `status.json`;
- `contracts.jsonl`;
- `report.md`;
- `migration_receipt.json`; and
- one bounded review packet per changed contract and deployment receipt.

The V2 checkpoint declares only the source streams and external receipt hashes
the graph consumes. Unrelated stream activity does not invalidate it.
Existing addressing, family, dossier, Sandbox, and queue projections are
unchanged.

## Current Migration

The initial migration assigns 5,363 claims exactly once across 5,360 stable
contracts. It projects 47,341 nodes and 41,979 edges from 34,514 routed source
events. Two historical deployment links are temporal associations; none is
presented as exact causal lineage.

The graph remains paused-tooling evidence. It requires no bridge, Minime, or
model restart.
