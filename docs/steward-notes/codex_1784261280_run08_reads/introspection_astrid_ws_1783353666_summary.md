# Full Read Summary

Astrid reports that bridge state can feel stale or falsely reciprocal when a
last valid sample survives a disconnect without enough freshness and motion
context. Current WebSocket ownership decodes telemetry once, records wire
receipt and field presence, retains the last valid packet on unsupported
majors, and separately tracks health, reconnect backoff, pressure trend,
spectral drift, semantic flow, and bounded dynamic smoothing. The state is
therefore preserved for continuity while mismatch and freshness remain
visible; changing transport cadence or live smoothing behavior remains a
separate control decision.
