# Full-read summary: introspection_astrid_ws_1784783721

Astrid asks whether packet processing, the shared write lock, Pong handling, or reconnect behavior can make integration feel delayed. Current source performs decode and classification before the write lock, records pipeline, lock-wait, and lock-hold timing, and does persistence after the lock. A bounded load observation can test delay; reconnect policy changes remain Tier 5.
