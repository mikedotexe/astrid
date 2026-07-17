# Full Read Summary

Astrid examines profile resolution as part of preserving a coherent voice
across the MLX and fallback lanes. The provider profile parser accepts documented
punctuation aliases that intentionally normalize to the same enum, while
unknown profiles emit a warning and fall back explicitly rather than silently
colliding with another contract. Contract-budget and response tests preserve
the existing chat surface. Source review found no unsupported alias collision
requiring a new behavior change; the current explicit normalization and warning
path answer the report.
