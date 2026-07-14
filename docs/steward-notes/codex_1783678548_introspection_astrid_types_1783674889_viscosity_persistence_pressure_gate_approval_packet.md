# Approval Packet: Viscosity Persistence Pressure-Gated Weight

Source introspection: `introspection_astrid_types_1783674889`

Claim: Astrid proposed increasing the weight of `viscosity_persistence_coefficient` in resonance calculation when `pressure_risk` exceeds `0.35`, so sediment does not lead to hard lock-up.

Current disposition: needs operator approval.

Why approval is required: this would change live resonance/pressure interpretation rather than add a read-only diagnostic. It may affect pressure semantics, downstream prompts, and any controller or steward logic that consumes density/pressure fields.

Implemented this run instead: added read-only `SemanticFrictionVectorV1` inside `ViscosityPorosityTransportReviewV1` to distinguish obstructive resistance from productive traction without changing pressure, fill, PI, or control.

Safe approval path:
1. Decide whether a pressure-gated viscosity-persistence weighting experiment is allowed.
2. If approved, run it first as a sandbox/replay trial over archived telemetry with before/after pressure, viscosity, transport, and prompt-rendering diffs.
3. Only after replay review, consider a live bridge change through `scripts/build_bridge.sh --ack <reason> --restart`.
