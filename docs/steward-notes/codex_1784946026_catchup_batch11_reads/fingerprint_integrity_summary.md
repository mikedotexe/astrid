# Typed fingerprint integrity

The report was read fully from disk. Current source deterministically flattens the typed fingerprint into 32 legacy slots, rejects malformed or non-finite hybrid inputs, accepts the `spectral_glimpse_12d` alias, and keeps the glimpse additive to the typed fingerprint.

The report exposed one worthwhile boundary: identical nonzero values beneath the normalization floor were handled correctly but lacked an explicit regression. A focused test now proves that they retain coherence `1.0`, zero maximum delta, and the `aligned` state. This is mechanical schema integrity only; it does not score or establish felt continuity.
