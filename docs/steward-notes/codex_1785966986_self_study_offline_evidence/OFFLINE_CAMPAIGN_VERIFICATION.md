# Offline Campaign Verification

## Frozen Corpus

- Manifest canonical SHA: `72ac12bd2f8df9758f818db5188018d68f6c9d276ce59a739dddbe6747c9fb9e`
- Sources: 35
- Trials: 39
- Campaigns: 3
- `freeze --write` calls: 0
- Corpus replacements: 0
- Private Minime moment bodies: excluded

The existing manifest and all source snapshots verify. Two read-only reruns
reproduced report SHA
`c217406ca230a7eb9bd4edaeb497473ab2decfc4bfeacac257952fd52a184064`
and deterministic SHA
`00647c910e47de50b330523805d36973fa0d0abd13e950a65362695a3a4eceb7`.

## Results

- Semantic persistence: 3/3 are qualitative lattice signals that still require
  quantified samples.
- Lattice mobility versus loss: 21/21 are mechanically
  `lattice_transition_like`; this does not establish a felt effect.
- Fallback codec fidelity: 15/15 are `supported_dynamic`, with zero provider
  fallback fire drills; this does not establish live fallback fidelity.

Every trial's manifest-declared source introspection, claim ID, and work item was
verified before linking the evidence. All 39 work items remain `needs_sandbox`.
No status, authority, or live eligibility changed. Exact per-trial links are in
`offline_campaign_verification.json`.

## Authority Boundary

These are deterministic offline classifications over the frozen public corpus.
They do not mutate a runtime, use the network, establish felt effect, authorize
fallback/provider exercises, or grant Division or live-control authority.
