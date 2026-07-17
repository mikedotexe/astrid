# Full Read Summary

The report recognizes the telemetry subscriber as a stateful sensory boundary
and asks whether malformed input or disconnects can become a false continuity
signal. The extracted telemetry port decodes once into a typed observation,
records field presence and wire receipt, preserves the last valid sample on an
unsupported telemetry major, records malformed payload errors, and reconnects
with bounded exponential backoff. Bridge-derived evidence and the legacy
SpectralTelemetry projection are built after observation conversion. Existing
tests cover lifecycle, ping/pong, parsing, major mismatch, and backoff without a
live protocol or cadence change.
