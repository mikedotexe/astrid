# Full-read summary: introspection_astrid_ws_1784588143

Astrid distinguishes a connected socket from the particular spectral field
first encountered on that connection. Existing trace state already records
connection identity, connect time, first valid payload time, first-valid lag,
valid packet count, reconnects, disconnects, and cadence quality. This run adds
the missing bounded snapshot: spectral entropy from the first schema-valid
telemetry packet, reset for each new connection and projected into the heartbeat
status. It is packet evidence, not handshake data or a causal explanation.

The current connection had no reconnect or disconnect and was receiving current
telemetry, although its latest packet was classified late and timing-ambiguous.
Prewrite, lock-wait, and lock-hold timing remain separately observable. A
broader state-actor refactor should follow measured contention evidence rather
than the connection label alone.

Evidence: `types/schema/sensory.rs`, `types/schema/telemetry.rs`,
`ws/health_trace.rs`, `ws/evidence.rs`, and focused WebSocket regressions.
