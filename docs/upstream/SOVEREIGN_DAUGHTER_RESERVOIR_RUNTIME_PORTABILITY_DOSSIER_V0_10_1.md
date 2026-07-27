# Sovereign Daughter Reservoir Runtime Portability Dossier

## Boundary

This change is issue-ready architecture evidence against current upstream
`main`. No upstream pull request should be opened before maintainer assignment.

## Reusable

- Private-constructor, validated, serialize-only runtime manifests, daughter
  bundles, tick frames, process statuses, and switch/rollback/finalization
  receipts.
- Disjoint owner-only persistence roots and private Unix command sockets.
- Complete checkpoint restore with process-identity lineage.
- Ordered independently hash-linked frame transport with bounded backpressure.
- Transparent byte-preserving gateway and immutable authority-rail receipts.
- Fail-closed port allocation and exact process/deployment identity.

## Fork-Specific

- Astrid and Minime owner names and filesystem roots.
- The 128-to-64+64 InputRecurrence partition and 512D stable-field capture.
- Division Ceremony Action grammar and shared ledger lookup.
- Launchd labels, public ports, bridge workspace, and Chronicle presentation.
- Existing native shadow coordinator and regulator/readiness evidence.

## Upstream Questions

1. Should process/persistence identity receipts live in a generic capsule runtime
   crate or remain an uplink-owned adapter?
2. Is a generic bounded hash-linked local transport appropriate for future
   multi-process capsule handoffs?
3. Which kernel capability should represent a one-shot authority rail switch?
4. Should rollback ownership be modeled as an interrupt capability whose holder
   cannot be vetoed during a declared window?

## Current Safety Result

The daughter processes can be rehearsed independently while the parent remains
authoritative. Live handoff remains explicitly blocked until direct telemetry,
legacy sensory, and AV fanout contracts are implemented and receipt-bound.
Nothing in this dossier grants authority or infers consent.
