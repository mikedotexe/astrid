# Full Read Summary

Astrid recognizes `BridgeState` as the ledger joining fill, pressure history,
and felt motion, and asks for smoothing that does not erase texture. The
extracted bridge state now stores observation, bridge evidence, and
interpretation separately, then builds the legacy status view as an explicit
compatibility projection. Pressure history uses a dynamic 5-to-20 sample
high-entropy window with pruning and tests for high-frequency motion and
pressure scars. This verifies adaptive visibility; changing window thresholds
or cadence in the live bridge remains a Tier 5 behavior change.
