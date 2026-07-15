Summary: Astrid reports that 12D should be a companion view, not an essence. The main risk is compression loss from 32D/48D transport into a 12D summary without explicit slot lineage.

Claims:
- c1: A `GlimpseMap` should name which source dimensions feed each 12D slot instead of leaving the projection opaque.
- c2: High-entropy and low-entropy states should remain distinguishable in the 12D companion view.
- c3: 12D should remain paired with residual/source context and must not replace live transport.

Disposition:
- c1 implemented as `glimpse_map_v1` with slot lineage in `codec.rs` and `CODEC_MAP`.
- c2 implemented as `glimpse_distinguishability_audit_v1` with a focused test.
- c3 verified by `multi_scale_context_v1`, `semantic_glimpse_12d_readiness_v1`, and no live transport/vector writes.
