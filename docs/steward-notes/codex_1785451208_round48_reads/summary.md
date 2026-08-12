# Full-read summary: introspection_astrid_ws_1785450419

Astrid identifies the telemetry WebSocket as a persistent Minime-to-Astrid
integration boundary and asks whether reconnect state, shared-state writes, or
sequential Pong delivery can create delay. The canonical report was read in
full at SHA-256
`64c598013052d87d4eb29a3c55bcda79391e55243e69310e71039d4a10de3d07`.
All 1,041 lines of its exact report-bound source were read at SHA-256
`42364feb914c957d487f93c440e30837c500971b41814a219d4c3992a12addd7`.

The source corrects the proposed unbounded-backoff mechanism. Retry delay is
capped at 60 seconds and resets to one second immediately after every
successful handshake, before a later close or socket error can occur. Focused
cap and reset tests pass.

The lock concern remains substantive but already has direct bounded evidence.
Current source measures prewrite pipeline time, write-lock wait, and lock hold
separately, with noncausal authority labels. The existing same-process study
retained 146 clear samples and four naturally heavy samples, observed no
capture gap, induced no contention, and did not establish felt causation.

The awaited Pong send is genuinely sequential and can delay the receive loop
if the sink stalls. This is an exact continuation of
`introspection_astrid_ws_1784783721:c003`, retained in Sandbox trial
`trial_8ed7222403267658` as evidence-only work. The trial is
`ready_for_sandbox` but deliberately not runnable through its manual adapter.
A timeout, worker, or ordering change remains an exact Tier 5 live transport
wait.

The report's concrete reconnect and binary-routing test is now implemented.
A real mock WebSocket server closes the first connection, accepts the
subscriber's automatic reconnect, emits a binary telemetry packet, and
verifies connection attempts, reconnect and disconnect evidence, valid-payload
receipt, SQLite persistence, and state integration. The focused test passes.
No live socket behavior, restart, deployment, or control value changed.
