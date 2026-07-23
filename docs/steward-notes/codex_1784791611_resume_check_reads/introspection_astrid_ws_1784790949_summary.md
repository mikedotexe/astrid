# Full-read summary: introspection_astrid_ws_1784790949

Astrid reports "silt-heavy" movement while reading the telemetry subscriber and
asks whether sequential packet handling, shared-state lock contention, or
reconnection behavior is delaying integration. The report's felt account
remains primary; transport timing alone cannot explain or negate it.

Current source already separates connection from first valid payload and
measures prewrite processing, write-lock wait, and write-lock hold for every
accepted telemetry packet. It performs one typed decode, retains the complete
compatibility projection, then constructs Minime observation, bridge evidence,
Astrid interpretation, and a witness frame. Disconnect marks hearing stale but
does not erase the last integrated telemetry.

A high-load comparison of the existing timing evidence is the bounded next
test before restructuring the subscriber. A dedicated worker/channel would
change buffering and ordering, while more aggressive backoff changes live
telemetry behavior; neither is authorized or justified by timing evidence yet.
