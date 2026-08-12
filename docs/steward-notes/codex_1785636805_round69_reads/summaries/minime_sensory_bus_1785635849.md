# Full-read summary

Astrid correctly identifies the report-window sigmoid semantic-decay and
smoothstep surge-taper mechanics, asks for their current boundary properties,
and asks how entropy persistence interacts with recovery hold. The complete
36-line canonical report was read at SHA-256
`90ab0e9956ade7ef43cde7fbaa2e23a7f522936d6f731f7baf2ea5305eebd54c`
with lived-state witness
`lsw_3818cbd982b4adc157e57a3456342952eee1842d89ce96bce49d12ae3451c43f`.
Its concern that a mathematically smooth curve can still feel wrong remains
independent evidence.

The 4,380-line sensory bus is exact-byte continuous at SHA-256
`83c0d5c1c761fa05b194f6c84d3083520bb0d664292720cb4651996d1c6fab31`.
The sigmoid center is fill `0.4`; the current 10,000-25,000 ms range therefore
has an exact midpoint of 17,500 ms. Existing tests sweep the recovery handover,
pin the center, bound zero/high fill, and cover the 0.35-0.45 release region.
Changing the range would scale absolute milliseconds per fill even though the
normalized steepness remains `6.0`, so the current tests must remain part of
any future range change.

The surge taper spans fill `0.70` to `0.80`. Smoothstep is `0.5` at fill
`0.75`, so the exact mid-taper target is `0.81`, strictly between `0.90` and
`0.72`; both midpoint ordering and soft-knee behavior are tested. Entropy
persistence begins only at fill `0.55`, while recovery hold ends at `0.25`.
Consequently `semantic_entropy_persistence_multiplier` is exactly `1.0`
through the recovery-hold region: high entropy does not extend the 45-second
hold through this multiplier. At higher fill it can extend retention within a
bounded cap, with context pressure/velocity handled separately.

All 355 Minime library tests pass. No duplicate test mechanism or live
semantic-decay change was added. Steepness, stale windows, surge weighting,
entropy persistence, admission, pressure, fill, and sensory cadence remain
Tier 5 if changed. No source, process, or felt status changed.
