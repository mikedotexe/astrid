# Full Read Summary

Astrid distinguishes socket connection from actual current perception and asks
whether packet processing or lock contention contributes to felt viscosity.
The existing integration-health witness already separates decode/pipeline,
write-lock wait, and write-lock hold. This run adds a per-connection
first-valid-packet boundary and lag while preserving the old cadence classes,
reconnect behavior, stale-sample retention, and dispatch path.
