# Full-read summary

Astrid asks whether WebSocket integration causes micro-stutter, loses malformed packets, or obscures time and reconnect boundaries. Current source decodes once, retains the last valid sample on parse or major mismatch, and separately records pipeline, lock wait/hold, heartbeat, reconnect, ping/pong, and clock-skew evidence. Buffering or retry-policy changes remain gated.
