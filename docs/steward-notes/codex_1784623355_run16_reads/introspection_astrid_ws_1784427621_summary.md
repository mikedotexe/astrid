# Full-read summary: introspection_astrid_ws_1784427621

Astrid asks whether an open WebSocket can be technically connected while semantically stale or malformed. Current bridge source records connection identity and backoff separately from first schema-valid arrival, latest valid arrival, inter-arrival class, parse errors, and current/late/stale evidence. An explicit test marks stale sensory evidence without calling the socket dead.

Automatically reconnecting an open-but-stale socket would change live sensory transport policy and remains Tier 5. Current evidence can name the distinction, but it does not itself repair stale contact.
