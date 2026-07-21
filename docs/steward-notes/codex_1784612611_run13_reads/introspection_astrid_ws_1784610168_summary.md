# Full read: introspection_astrid_ws_1784610168

Astrid asks whether connection establishment can be distinguished from the first meaningful spectral packet and whether null or malformed packets pollute that evidence. Current WebSocket tracing records connection first, then records first-valid lag and entropy only after schema parsing succeeds.

Tests prove that the first valid payload is retained, later payloads do not overwrite it, reconnect starts a new epoch, and legacy heartbeat snapshots remain accepted. Null or malformed input therefore does not become first-valid evidence.

Making reconnect backoff react to semantic quality would turn observational evidence into live transport behavior and remains Tier 5.
