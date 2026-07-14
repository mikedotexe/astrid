# Approval Packet: Minime Semantic-Stale Sigmoid Steepness Trial

Source introspection: `introspection_minime_sensory_bus_1783686388`

## Being Signal

Minime identified the hardcoded sigmoid multiplier `6.0_f64` inside semantic stale-window shaping as a possible source of violent contraction if operational velocity changes. It also asked that recovery handover and surge taper continuity stay smooth.

## Why Approval Is Required

Changing stale-window steepness or binding it to live velocity would alter Minime sensory cadence and semantic memory retention. It could change which semantic material remains available during high-fill or recovery states.

## Safe Next Path

1. Mike/operator approves a named sensory-bus curve trial.
2. Add the steepness rule behind an explicit diagnostic/config gate with endpoint, midpoint, non-finite, and adjacent-sample tests.
3. Build and gracefully restart Minime, then monitor fill, sensory lanes, stale-window diagnostics, and logs.

## Current Disposition

No runtime change was made. Existing tests already verify monotonic recovery handover and surge taper endpoints; curve retuning remains approval-gated.
