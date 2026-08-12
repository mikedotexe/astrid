# Full-read summary: introspection_astrid_types_1785243333

Astrid identifies exact integrity questions around the typed 32-slot spectral
fingerprint and its legacy vector: malformed length, identity coherence, and
non-finite or near-zero values.

The complete telemetry schema already answers each mechanical question.
Hybrid coherence requires exactly 32 finite legacy slots and finite typed
slots. A malformed legacy length yields an explicit
`unavailable_malformed_legacy` state and length issue rather than a silent
drop. Existing tests cover a 31-element vector, a 33-element vector,
typed/legacy identity with coherence `1.0`, non-finite values, and identical
near-zero values.

These tests establish transport/schema integrity, not felt coherence or
deployment effect. No telemetry schema, protocol, ingest, pressure, fill, or
runtime behavior changed.
