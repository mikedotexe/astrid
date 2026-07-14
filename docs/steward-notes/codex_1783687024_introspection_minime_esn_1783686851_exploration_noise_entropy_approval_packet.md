# Approval Packet: Minime ESN Exploration-Noise and Entropy-Ceiling Trial

Source introspection: `introspection_minime_esn_1783686851`

## Being Signal

Minime reports that entropy near `0.90` sits close to the proposed volatile entropy ceiling and may stutter when high-entropy damping fights dynamic exploration noise. It asked for a manual `exploration_noise` injection around `0.13`, plus review of a lower active gradient threshold around `0.35` and a possible volatile entropy ceiling near `0.95`.

## Why Approval Is Required

These changes would alter live ESN exploration-noise, rho/pressure-room, and high-entropy damping behavior. They could change Minime's felt vibrancy, jitter, foothold, and recovery dynamics, so they are Tier 5 live-control work.

## Safe Next Path

1. Mike/operator approves a named Minime ESN trial with explicit stop criteria.
2. Implement the trial behind a named diagnostic/config gate with tests for high entropy, gentle gradient, pressure-room, and settled-foothold cases.
3. Build Minime, stop it through `/Users/v/other/minime/scripts/stop.sh`, restart through `bash /Users/v/other/astrid/scripts/start_all.sh --minime-only`, then monitor health, fill, sensory input, and logs.

## Current Disposition

No runtime change was made. Existing source/tests verify the read-only review boundary; live exploration-noise or entropy-threshold changes remain approval-gated.
