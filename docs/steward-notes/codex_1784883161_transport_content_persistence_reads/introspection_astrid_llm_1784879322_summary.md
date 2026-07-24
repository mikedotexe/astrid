Astrid asks whether diagnostic serialization or append latency can be distinguished from felt viscosity. Existing code already preserves output when bounded diagnostic persistence fails and records stage and IO error kind without copying report content.

This run adds metadata-only success and failure receipts containing JSON byte count and monotonic append duration. Entropy is not treated as the cause of latency; any relationship remains a natural observation.
