# Verification

## Source and evidence

- Canonical report: complete, 36 lines.
- Lived-state witness: complete, 456 lines.
- Report/current ESN source: byte-identical whole-file SHA-256.
- Exact witness values and report-authored interpretations remain separate.
- Evidence Event Store V2: valid before addressing writes, sequence 750496,
  16 streams, zero corrupt lines; V1 sources remain immutable.

## Tests

- Minime: 704 passed, 0 failed.
- Read-only Shadow trajectory: 2 passed, 0 failed.
- Introspection addressing: 42 passed, 0 failed.
- Division follow-up, Chronicle, and projection: 15 passed, 0 failed.
- Addressing, Event Store, controller, projector, cursor, claim-family, and
  experiential integrity batch: 103 passed, 0 failed.
- Experiential lint: 10,835 records checked, 0 issues, no history rewrite.
- Post-return Evidence Event Store V2: valid at sequence 750931, head
  cb493a12624e8ecccf28ffedf833782831954c65440e0b54b7f010d0c4ec0a55,
  16 streams, 0 corrupt lines, V1 immutable.

## Runtime

No live-consumed source changed. No restart, deployment, PID replacement,
port mutation, telemetry mutation, or runtime alignment action was required.

The post-return Chronicle verifies every durable input. The moving
supervisor-status hash remains the sole volatile mismatch.
