# Temporal Agency Boundary Portability Dossier

Target: upstream Astrid `v0.10.1`

## Reusable Core

- Private trusted temporal context verification with an untrusted persisted DTO.
- Five-second future-clock tolerance, monotonic in-process identity, and explicit expiry.
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
