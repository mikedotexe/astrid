# Full Read Summary

Astrid flags the projection epoch cache as vulnerable if it depends on the
daemon's working directory. Current source resolves an explicit environment
override first, then an executable-relative
`data/spectral-bridge/runtime` path, with a deterministic process-relative
fallback only when executable identity is unavailable. Atomic file writes,
stable kernel-derived epochs, checksums, and runtime-resolution readouts are
covered by tests; no projection remap is performed by inspection.
