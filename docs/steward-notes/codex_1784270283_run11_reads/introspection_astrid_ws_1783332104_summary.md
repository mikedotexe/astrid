# Full Read Summary

Astrid experiences BridgeState's pressure history and heartbeat as a felt pulse
and worries that a five-sample window could be twitchy or silently lose missing
pressure. Current code explicitly emits `telemetry_gap`, keeps a dynamic
5-to-20-sample high-entropy ballast window, preserves a fast three-sample edge,
and reports slow context, range, semantic fidelity, viscosity velocity, and
spectral drift. These remain read-only bridge evidence.
