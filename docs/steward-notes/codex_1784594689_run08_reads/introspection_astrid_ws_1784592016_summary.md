# Full-read summary: introspection_astrid_ws_1784592016

The telemetry subscriber records attempts, connection identity, reconnects, disconnects, and valid-payload state separately. The additive heartbeat diagnostic now distinguishes a hot socket awaiting valid telemetry from one carrying current telemetry, and records first-valid lag and entropy without treating either as handshake or felt quality.

A soft reconnect, timeout, or cadence policy change would alter the live telemetry transport and remains an exact Tier 5 wait. A bounded connection-flap and first-valid-lag observation is safe evidence work.

Evidence is linked in `evidence_links.json`; live-control proposals remain explicit authority waits.
