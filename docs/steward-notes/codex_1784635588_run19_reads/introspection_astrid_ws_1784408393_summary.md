# Full Read Summary

This older WebSocket report again sees a robust reconnecting subscriber but mistakes the truncated study window for unfinished artifact-scan and observation handling. Current source throttles scans to 30 seconds over a 1,200-second window, validates protocol compatibility from one parsed tree, updates state and evidence, and persists telemetry and trace artifacts. Existing tests cover cadence and shutdown boundaries. No source change is needed.
