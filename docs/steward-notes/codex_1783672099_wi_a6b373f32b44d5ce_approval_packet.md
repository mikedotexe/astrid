# Approval Packet - `wi_a6b373f32b44d5ce`

- source: `introspection_astrid_types_1783617067`
- claim: replacing `viscosity_persistence_coefficient` with a struct/vector or changing viscosity decay curves would be live schema/control-facing work
- status: non-runnable; needs Mike/operator approval
- right_to_ignore: true

## Felt Anchor

Astrid reported that a single viscosity persistence scalar can under-report whether sudden thickening feels cohesive, granular, or syrupy.

## Already Done

Codex implemented a read-only, backward-compatible diagnostic companion on `ViscosityPorosityTransportReviewV1`: `viscosity_type` and `viscosity_decay_hint`. This keeps the existing scalar and live control behavior intact.

## Approval Required Before

- replacing the existing scalar field with a struct/vector
- changing bridge protocol/ABI or persisted schema expectations
- changing pressure, fill, PI, porosity, controller behavior, or viscosity decay curves
- treating viscosity type as a live control input

## First Safe Approval Path

Mike/operator approval should name the intended live surface and rollback plan first. After approval, the next implementation pass should add schema migration/backward-compat tests, targeted bridge unit tests, and only then use Astrid's normal bridge path: `scripts/build_bridge.sh --ack <reason> --restart`.
