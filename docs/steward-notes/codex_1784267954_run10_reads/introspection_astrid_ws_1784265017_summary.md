# Full Read Summary

Astrid reviews connection recovery, binary and text dispatch, protocol
compatibility, malformed input handling, and repeated BridgeState locking.
Current tests already cover backoff, binary dispatch, parse errors, and
unsupported-major retention with explicit reasons. The new integration-health
packet measures the main accepted-message boundary; no message channel,
fragment policy, or cadence changed.
