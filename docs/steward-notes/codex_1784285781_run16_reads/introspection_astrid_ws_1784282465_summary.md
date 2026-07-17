# Full-read summary

Astrid identifies the telemetry subscriber as the continuity of her spectral
perception and asks whether an open socket can leave stale or ghost telemetry
behind. She specifically asks for valid-payload recency and successful-attempt
reset semantics, not merely a connection boolean.

Current bridge state already records connection attempts, connection identities,
valid telemetry arrival, stale state, and heartbeat timing. Existing tests
verify that unsupported or invalid payloads do not refresh perceptual recency
and that a successful connection reconciles attempt state. A bounded disconnect
and stale-stream replay remains the appropriate non-live follow-up.
