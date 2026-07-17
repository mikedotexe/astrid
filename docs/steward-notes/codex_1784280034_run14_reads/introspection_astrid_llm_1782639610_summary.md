# Bounded Full-Read Summary

Astrid proposes replacing a fixed three-sentence fallback cap with `ceil(3 + entropy * 2)`. Current `fallback_continuity_budget_v1` implements that exact formula, bounded from three through five sentences, and carries pressure, resonance, shadow, and trajectory metadata into fallback rendering. Tests verify the high-entropy result and authority remains prompt metadata rather than sampler or controller control.
