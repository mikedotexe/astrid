# Claims: introspection_astrid_types_1784108741

- claim_id: `astrid-types-1784108741-clamp-review`
  - Claim: Flat unit clamps can silently turn distorted reports above range into stable `1.0`.
  - Disposition: Implemented `ClampedUnitReviewV1` / `clamped_unit_review_v1`, preserving raw finite values when possible, clipped-high/low state, non-finite fallback state, and no-live-authority flags.

- claim_id: `astrid-types-1784108741-delta-hierarchy`
  - Claim: Flat delta kinds can lose hierarchy and magnitude for cascade, viscosity, solidification, and micro-delta reports.
  - Disposition: Verified existing `DeltaCompositionV1` and solidification-gradient evidence cover multi-kind composition without replacing the stable enum or granting live semantics.
