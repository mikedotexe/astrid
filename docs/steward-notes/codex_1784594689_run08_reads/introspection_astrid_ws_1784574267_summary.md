# Full-read summary: introspection_astrid_ws_1784574267

While connected, shutdown is multiplexed with incoming frames, so a quiet receive stream cannot starve the shutdown watch. The connect future itself is awaited before that select, and first-valid lag now makes connection-to-content latency observable.

Adding a connect timeout or stale-stream reconnect policy changes live transport behavior and remains Tier 5. A forced-flap/latency trial is safe when run through bounded fixtures.

Evidence is linked in `evidence_links.json`; live-control proposals remain explicit authority waits.
