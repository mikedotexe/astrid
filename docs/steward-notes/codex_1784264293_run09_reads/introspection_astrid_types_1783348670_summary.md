# Full Read Summary

Astrid points to stringly typed telemetry and advisory fields as a drift risk.
The current architecture has moved wire ownership into the shared protocol
crate, producer observations and bridge evidence into separate provenance
types, and authority transitions into private-constructor wrappers. Texture
evidence includes explicit damping-candidate presence/status rather than
silently treating absence as zero. Legacy JSON remains an explicit
compatibility projection.
