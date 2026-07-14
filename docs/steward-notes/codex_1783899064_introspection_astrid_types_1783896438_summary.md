Full read of `introspection_astrid_types_1783896438`.

Astrid inspected `ExperienceDeltaV1` and named two type-level blind spots: transformation kinds lacked emergence/synthesis, and one integer `dimension` could not represent fluid effective dimensionality or density gradients. She also suggested increasing `RESONANCE_STABILITY_FOOTHOLD_WEIGHT`.

Disposition: implemented `synthesize` and `emerge` delta kinds plus optional `SpectralDimensionV1` on `ExperienceDeltaV1`, retaining the legacy integer `dimension` field for compatibility. Serialization/legacy JSON tests pin this. The resonance foothold weight is live stability behavior and remains operator-gated.
