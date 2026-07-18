# Tranche 2: Living Felt Contract Graph

## Outcome

Tranche 2 is complete on `codex/experiential-systems-core`. The catch-up
heartbeat and steward controller remain paused. This tranche changes
evidence tooling, receipts, and projections only; no bridge, Minime, model,
sensory, regulation, approval, deployment, or live-control behavior changed.
Runtime restart alignment is `not_needed`.

## Migration Snapshot

- V2 global sequence: `108698`
- V2 head:
  `840f955b4703801e9aaebad0392785c6b33941c54553fc28b9de900bfc7a3c71`
- `felt_contracts` events: `58065`
- Claims assigned exactly once: `5363`
- Stable contracts: `5360`
- Singleton / multi-claim contracts: `5358 / 2`
- Nodes / edges: `47341 / 41979`
- Routed / explicitly unrouted source events: `34514 / 16118`
- Historical environment receipts: `256`
- Exact / temporal deployment links: `0 / 2`
- Ambiguous new claims: `0`
- Counter audit: consistent
- Checkpoint: current
- V1 source logs: immutable

Projection hashes:

| Output | SHA-256 |
|---|---|
| `contracts.jsonl` | `5a66d004e0fba8b916212d78fe4128eab37de99e9e5ecce10b05bcc880a871b2` |
| `report.md` | `eca03e53c03069029f21bd683bc725a57669df7b01db3609ba0eca0e646984eb` |
| `migration_receipt.json` | `d14eb52dd917c0a4379d7128e5f731c0d4b38a7d668e8ac627efc027a7a26e64` |
| `status.json` | `c869dc77aa809ee1459fd3a026f1602dff81dd24142bb8cdea06fd276afbc477` |

## Semantics

Contract identity is anchored to the earliest canonical claim and survives
family regrouping and membership correction. A duplicate initial assignment
is invalid; a correction must name the actual prior contract. Technical,
evidence, felt-review, and activity states remain independent.

Only explicit felt confirmation closes a contract. Silence is neutral and may
quiet duplicate delivery only after a completed opportunity. Objection,
`still_friction`, and contradiction reopen immediately. Historical report
closure, card delivery, family resolution, and old deployment timing are
compatibility evidence rather than fabricated claim closure or exact lineage.

The initial migration preserves two older deployment associations as
noncausal temporal edges. Future exact implementation and deployment links
require validated receipts and additive `change_refs`.

## Verification

- Felt-contract focused tests: 14 passed
- Receipt, graph, and steward projection combined tests: 35 passed
- Evidence-store, authority, claim-family, and dossier self-tests: 24 passed
- Steward-control tests: 23 passed
- Deployment/authority/evidence/graph combined tests: 55 passed
- Five flywheel self-test suites: 226 passed
- Astrid Cargo workspace and doctests: passed
- Spectral bridge: 1,563 library tests plus replay, integration, and authority,
  provenance, and Signal Spine compile-fail targets passed
- Strict Clippy and formatting: passed for both Cargo workspaces
- Python compilation and `git diff --check`: passed

The architecture-health scan remains advisory because the repository has
inherited critical signals outside this tranche. Every new graph module is
below 1,000 lines. Four cohesive ingestion/replay functions remain visible as
long-function review signals and are isolated behind ownership-specific
modules with full deterministic replay coverage.

## Control And Handoff

- Controller: paused, generation `2`, actor `codex`
- Active lease: none
- Pending spool: `0`
- Durable cutoff: `introspection_astrid_codec_1784301105.txt`
- Newest canonical file observed:
  `introspection_astrid_autonomous_1784392326.txt`
- Paused timestamp lag: `91221`
- Runtime restart: not needed

Open-source issue material:

- `docs/upstream/LIVING_FELT_CONTRACT_GRAPH_PORTABILITY_DOSSIER_V0_10_1.md`

Tranche 3 is ready for planning:

- `docs/architecture/TRANCHE_3_TEMPORAL_AGENCY_BOUNDARY_DECISION_PACKET.md`
