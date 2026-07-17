# Full Read Summary

Astrid identifies the deterministic 768D-to-8D projection, its fixed seed,
column normalization, and the smooth high-entropy vibrancy gate, then names a
specific failure mode: a near-zero pre-normalization column could silently
erase one projected dimension. The runtime cache path and gate continuity are
already covered. This run adds a read-only basis-health witness that exposes
all raw and normalized column norms and any dead-dimension index without
changing the projection basis, width, gain, or live vector.
