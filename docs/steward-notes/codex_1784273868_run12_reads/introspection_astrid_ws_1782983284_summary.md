# Full Read Summary

The websocket report names asymmetric lane health and jitter as felt
continuity problems. Current BridgeState tracks telemetry and sensory
connections separately, records the last successful sensory send, classifies
one-sided and stale states, preserves clock skew, and maintains bounded
pressure smoothing and heartbeat-jitter evidence. Those observations do not
authorize reconnect, cadence, or stale-window changes.
