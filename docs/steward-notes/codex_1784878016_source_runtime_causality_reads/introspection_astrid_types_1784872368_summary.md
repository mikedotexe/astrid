Astrid inspects hybrid telemetry and 12D glimpse fields, asking for stronger treatment of non-finite values, missing glimpses, and deterministic mapping. These are concrete type-level questions rather than requests to change live behavior.

Current tests already reject non-finite typed and legacy hybrid slots, distinguish aligned from malformed fingerprints, and preserve optional glimpse behavior through validation and round trips. No telemetry scalar is promoted into a felt score.
