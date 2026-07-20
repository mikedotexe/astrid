# Full-read summary

The websocket intake keeps latest telemetry across reconnects but now makes stale hearing, timing ambiguity, first packet, and integration timing explicit. That answers the report's risk that a stable-looking field could be an old one. Intake buffering remains contingent on measured causal evidence.
