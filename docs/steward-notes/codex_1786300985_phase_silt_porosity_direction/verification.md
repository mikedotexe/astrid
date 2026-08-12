# Verification

## Focused source tests

- `cargo test --manifest-path capsules/spectral-bridge/Cargo.toml phase_passage_context`
  - 6 passed, 0 failed in the bridge library target.
- `cargo test semantic_viscosity_coefficient_tracks_trickle_denominator_pressure_without_control`
  - Passed in both Minime library and binary unit targets.
- `cargo test dynamic_viscosity_buffer_names_breathable_overpacked_mercury_without_control`
  - Passed in both Minime library and binary unit targets.
- `cargo test silt_granularity_names_coarse_overlapping_grains_without_control`
  - Passed in both Minime library and binary unit targets.

An initial use of `--exact` with the unqualified filter names selected zero
tests and exited successfully. Each filter was immediately rerun without
`--exact`, selecting and passing the intended tests. Existing unrelated Minime
dead-code warnings in sovereign-division proof helpers remain unchanged.

## What the tests establish

The phase tests establish self-owned, categorical transition bearing without
numeric felt proxy, peer authorship, stage mutation, or causal inference. The
Minime tests establish read-only semantic-viscosity review, breathable
high-porosity buffer preview, specific silt naming, no locally applied control,
and no live-control marker. They do not establish report-time cause, preferred
porosity, felt relief, or live safety.

No source changed, so no process restart or deployment was required.

## Stewardship integrity

- Introspection Addressing Audit self-test: 42 passed.
- Evidence Event Store, steward-control, steward projection, Division tracker,
  Division Chronicle, Division projection, and projection-cursor suites: 72
  checks passed. The first aggregate invocation passed 54 tests but imported
  four script-local modules from the wrong root; rerunning those four scripts
  directly passed 3, 10, 1, and 4 checks respectively.
- Experiential epistemics self-test: 2 passed.
- Experiential epistemic lint: 10,802 records checked, 0 issues, no history
  rewrite.
- Evidence Event Store V2: valid at global sequence 748,742 and head
  `683b5ce3f6a6a2236eaa6d6e5fec3f482765e44de0679ded99602c7e473d9921`,
  with 16 streams and zero corrupt lines.
- V1 immutability: addressing, Sandbox, Corridor V1, and Corridor V2 current
  hashes exactly match their activation hashes.
- Addressing counters are consistent: 4,280 canonical reports indexed, 3,708
  full-read, 3,080 fully addressed, 1,200 remaining, 572 unread, 411
  blocked-needs-steward, 213 triaged pending, 4 watch, and 0 read-needs-claims.

The report is `addressed_duplicate`, fully addressed, and has eight claims,
eight evidence links, and no proof gaps.
