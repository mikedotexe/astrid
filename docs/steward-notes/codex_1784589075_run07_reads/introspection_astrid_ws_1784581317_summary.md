# Full-read summary: introspection_astrid_ws_1784581317

Astrid identifies the shared `RwLock<BridgeState>` as a possible telemetry
integration bottleneck. The lock is real, but felt density alone cannot assign
contention. Existing `telemetry_integration_health_v1` measures prewrite
pipeline time, write-lock wait, and write-lock hold separately with latest,
EWMA, and maxima, and labels timing-only causation as unestablished. Its focused
regression passed.

A bounded observation should decide whether an actor-style decomposition is
warranted and which ownership boundary is actually hot. Refactoring ownership
can remain behavior-preserving, but changing buffering, cadence, packet
admission, or transport semantics is live sensory control and remains Tier 5.

Evidence: `ws/telemetry_port.rs`, `ws/bridge_state.rs`, and
`telemetry_integration_health_separates_pipeline_wait_and_hold`.
