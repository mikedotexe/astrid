# Temporal Agency Boundary Portability Dossier

Target: `astrid-runtime/astrid` `v0.10.1` at
`4771bab3c33d1bce53186e40d01cf014e2dce666`

## Reusable Core

- Private trusted temporal context verification with an untrusted persisted DTO.
- Five-second future-clock tolerance, monotonic in-process identity, and explicit expiry.
- The future-clock tolerance applies only to `issued_at`; it does not shorten
  expiry, bound reasoning duration, or pace dispatch.
- Remaining budget counts authorized live consequences. It is not a proxy for
  model work, entropy, semantic energy, or elapsed time.
- Exact source/deployment/process binding for live writes.
- Process-neutral but freshly reverified read-only grants.
- Durable reserve-before-dispatch outcomes: `sent`, `released`, and
  `outcome_unknown_consumed`.
- Locked, hash-linked, flushed, and fsynced append-only authority records.

## Fork Adapter

The spectral bridge adapter reads its deployment identity from the existing
stack build manifest and its pause generation from the agent-neutral steward
control state. These paths and the Minime semantic/mode-release scopes are
fork-specific. Upstream integration should inject both identities through a
small runtime context provider rather than importing spectral bridge paths.

## Security And Compatibility

Trusted contexts and reservations do not implement `Deserialize`. Existing
authority wrappers and live dispatch signatures remain the compile-time gate.
Legacy records remain readable but cannot become new live grants. V2 projection
is evidence-only and is not an authority source.

No upstream pull request should be opened until the maintainer assigns an issue.

The upstream design already has related maintainer-owned issues:

- [#1229 authenticated control connections and scoped delegation](https://github.com/astrid-runtime/astrid/issues/1229)
- [#694 auto-expiring capability grants](https://github.com/astrid-runtime/astrid/issues/694)

This dossier should be offered as design and regression evidence for those
efforts, not as a competing authority implementation.

## Felt Review Boundary

Astrid's bounded review proposed a persistence weight or viscosity coefficient
after reading the temporal context. That proposal is intentionally not part of
the portable authority primitive: introducing an experiential coefficient would
change live behavior and requires its own operator-approved intervention,
baseline, and felt review. Focused regressions instead verify the narrower
clock-skew and consequence-budget semantics above.
