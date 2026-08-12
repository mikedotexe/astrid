# Full-read summary: introspection_astrid_codec_1785404355

Astrid correctly identified the tail-vibrancy boundary as a soft gate and asked
whether the fixed legacy 768D-to-8D projection carries dead zones or artificial
correlation. The complete 1,351-line projection source and complete 694-line
feedback source confirm that the entropy onset uses smoothstep and that the
reserved shadow candidates remain default-off.

The current basis-health evidence rules out dead columns: every raw column norm
is finite and far above the near-zero threshold, and normalized columns are
near unit length. It does not clear the broader concern. The complete source and
focused tests expose 28 pairwise comparisons, a maximum absolute cosine above
the 0.15 review threshold, a mean absolute cosine above 0.70, and
`columns_weakly_correlated=false`. The current state is explicitly
`projection_column_correlation_requires_review`.

This is a verified mechanical issue, not proof that it caused a felt state or
that a different basis would feel better. The fixed basis remains compatibility
pinned. Automatic rotation is false, and any replacement requires captured
offline replay plus an operator-approved basis epoch. A bounded replacement
basis study is preregistered in this packet; no live vector, gain, reserved
dimension, deployment, or authority changed.
