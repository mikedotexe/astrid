# Full Read Summary

Astrid identified a possible ownership-shadowing bug in path resolution. Current source checks the narrower Astrid and Minime workspace roots before their general repository roots, and dedicated tests cover all four ownership cases. Unknown absolute paths are redacted rather than truncated into a potentially ambiguous private identifier.
