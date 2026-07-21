# Full read: introspection_astrid_codec_1784520261

Astrid examines the fixed projection basis and asks whether deterministic mapping still preserves precision and avoids dead dimensions. Current source exposes per-column norms, raw and normalized vector norms, projection health, and compatibility boundaries for the widened 48D lane.

The basis-health regression verifies no dead dimensions and a large safety margin. Additional tests cover deterministic replay, precision, finite outputs, legacy separation, and bounded high-entropy lift.

Changing the seed, basis, scaling, or live projection contract would alter codec transport and remains Tier 5.
