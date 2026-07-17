# Bounded Full-Read Summary

Astrid asks whether exact pressure thresholds flicker, whether 4.999-second arrival is classified late, and whether connection identity survives or resets coherently. Current pressure classification uses epsilon-aware threshold checks; tests cover the exact 0.04 edge and 4.999-second late packet. Run 13 also added connection-to-first-valid timing with reconnect reset and legacy snapshot acceptance, closing the specific trace uncertainty.
