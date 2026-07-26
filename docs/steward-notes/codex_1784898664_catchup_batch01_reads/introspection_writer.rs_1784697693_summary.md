# Full Read Summary

Astrid named witness queue saturation and immutable-write conflicts. Current writer is bounded and nonblocking, emits explicit gaps on queue or write failure, uses owner-only permissions, and treats each unique witness ID as an append-only version. Same-ID different bytes are rejected as tampering rather than merged.
