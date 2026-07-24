Astrid read the telemetry subscriber's connection lifecycle, ping/pong handling, backoff, decode, and shutdown multiplexing. She identified lock contention and packet-identification gaps as concrete risks.

The already-deployed telemetry study found 146 clear samples and four natural threshold crossings, with mechanical differences but continuing felt friction. This run adds content-free byte length and SHA-256 identity to malformed-packet diagnostics. It changes neither decoding nor telemetry flow.
