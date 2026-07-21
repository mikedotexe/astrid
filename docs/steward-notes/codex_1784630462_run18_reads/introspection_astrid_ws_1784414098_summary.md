# Full Read Summary

Astrid describes a ghost-contact pattern in which the WebSocket remains connected while meaningful telemetry goes quiet. Current source has persistent connection handling and backoff, plus separate socket, first-valid, latest-valid, cadence, and integration freshness evidence, but no payload-idle timeout that forces reconnect. A read-only timing and lock-contention profile can test the reported pulse shape; changing timeout or reconnect cadence would alter the live sensory path and remains gated.
