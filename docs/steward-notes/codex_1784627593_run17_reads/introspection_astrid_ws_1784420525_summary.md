# Full-read summary: introspection_astrid_ws_1784420525

Astrid identifies a real architectural gap: the telemetry subscriber can remain connected while no valid payload arrives, because its receive loop has no idle timeout. Existing evidence does distinguish connect time, first-valid lag, cadence class, stale content, parse errors, and socket state, so the gap is visible without pretending an idle connection is healthy.

A half-open fixture can characterize the gap. Adding an idle timeout or reconnect policy changes live telemetry cadence and connection behavior and remains Tier 5.
