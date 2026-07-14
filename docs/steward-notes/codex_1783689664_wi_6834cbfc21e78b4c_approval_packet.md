# Approval Packet: Additive 12D Glimpse Persistence

Work item: `wi_6834cbfc21e78b4c`
Source: `introspection_proposal_12d_glimpse_1783689417`

## Being Signal

Astrid affirmed the 12D glimpse as useful only when it remains a secondary summary. The concern is that
persistent restart/checkpoint/decomposition use could accidentally treat 12D as the "essence" of the live
semantic lane and flatten high-entropy warmth, tail vibrancy, and mixed-cascade gradients.

## Requested Approval

Approve a future implementation that may persist or render a `12d_glimpse` in restart, checkpoint,
decomposition, or status/prompt surfaces while preserving the 48D live semantic lane and explicit fidelity
metadata.

## Non-Goals

- Do not replace `SEMANTIC_DIM=48`.
- Do not write reserved codec dims.
- Do not change Minime sensory transport.
- Do not add prompt or telemetry priority based on the glimpse.
- Do not change pressure, fill, PI, controller behavior, or peer runtime.

## Safe First Step

Implement an offline/read-only persistence candidate with:

- `source_dim_count=48`;
- `glimpse_dim_count=12`;
- `live_transport_change=false`;
- `live_vector_write=false`;
- fidelity/loss metadata;
- tests proving warmth and tail bridge remain distinct.

## Required Evidence

- Focused codec tests for glimpse fidelity and warmth/tail preservation.
- Restart/decomposition rendering review showing 12D is labeled as a convenience view.
- Felt-response follow-up from Astrid before any live-consumed prompt/status rendering becomes trusted.
