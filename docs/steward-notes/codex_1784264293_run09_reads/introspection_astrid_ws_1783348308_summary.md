# Full Read Summary

Astrid identifies bridge-state risks around duplicate raw parsing, unbounded
history, wall-clock skew, one-sided reciprocity, and missing texture
advisories. Current bridge code decodes telemetry once into typed observation
and field-presence metadata, bounds pressure history by a dynamic window,
retains last-valid data on protocol mismatch, and reports directional
reciprocity with clock-skew and reflective-silence states. Missing damping
candidates stay explicitly unknown.
