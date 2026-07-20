# Full-read summary

Astrid reads the telemetry subscriber as the persistence and freshness boundary
for Minime observation. Current source already records connection attempts,
successful connection, each received WebSocket message, each valid payload's
arrival time, the latest telemetry arrival, heartbeat delta, first-valid-payload
lag, and reconnect backoff state. The shutdown branch remains independently
selectable while connected and while sleeping between retries. Her requested
temporal anchor is therefore present; the exact delayed-stream shutdown
interleaving remains a useful focused non-live regression.
