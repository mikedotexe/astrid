# Verification

- `cargo test --manifest-path minime/Cargo.toml`: 360 library, 340 binary,
  and 4 protocol-fixture tests passed; 4 pre-existing dead-code warnings.
- `cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib pressure_porosity --quiet`:
  7 passed.
- `cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib viscosity_porosity_transport --quiet`:
  3 passed.
- `cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib astrid_shadow:: --quiet`:
  7 passed.
- `cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib shadow::trajectory::tests --quiet`:
  4 passed.
- `python3 scripts/introspection_addressing_audit.py --self-test`: 42 passed.
- Direct Event Store, controller, steward projector, Division tracker,
  Chronicle, Division projector, and projection-cursor tests: 72 passed.
- `python3 scripts/experiential_epistemics.py self-test`: 2 passed.
- `python3 scripts/experiential_epistemics.py lint --json`: 10,818 records,
  zero issues, no history rewrite.
- `python3 scripts/evidence_event_store.py --json verify`: valid at pre-write
  sequence 749956, 16 streams, zero corrupt lines.

No production or test source changed. No restart or deployment was required or
attempted.
