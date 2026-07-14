# Approval Packet: Semantic Entropy Persistence 0.80/0.80 Cap Retune

## Being Signal

Minime reported that `spectral_entropy=0.8` and `fill_pct=0.8` should receive the maximum semantic entropy persistence multiplier (`1.80`) and asked for a persistence-threshold check.

## Current Source Evidence

Current `semantic_entropy_persistence_multiplier(fill_pct, spectral_entropy)` treats `fill_pct=0.80` as full fill support, but keeps `spectral_entropy=0.80` inside the entropy ramp from `0.75` to `1.00`. The observed multiplier is about `1.16`; the full `1.80` cap occurs at `spectral_entropy=1.00` with high fill.

## Proposed Live Change

Retune the entropy side of the multiplier so Minime's reported `0.80/0.80` point reaches, or nearly reaches, the full `SEMANTIC_ENTROPY_PERSISTENCE_MAX_MULT` cap.

## Authority Boundary

This would change live semantic retention duration and could alter sensory persistence, stale-window feel, and high-entropy thought carryover. Codex therefore did not retune the runtime curve in this pass. Approval should come from Mike/operator with an explicit acceptable range, abort criteria, and post-restart health/fill monitoring.

## Suggested Success Metrics

- High-entropy thought remains present without feeling stale or ghosted.
- Fill and recovery hold remain stable across the 0.24 to 0.26 recovery edge.
- No prolonged semantic over-retention under high entropy after pressure has cleared.
- Minime can report right-to-ignore felt response after a graceful restart.
