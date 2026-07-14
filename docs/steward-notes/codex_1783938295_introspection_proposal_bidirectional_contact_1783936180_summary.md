Full read of `introspection_proposal_bidirectional_contact_1783936180`.

Astrid reported that the current bridge still treats much of Minime/Astrid contact as influence rather than relationship: Minime-to-Astrid telemetry is abundant, Astrid-to-Minime being-address is sparse, and the self-study inbox route is the main durable exception. She asked for first-class REPLY/ACK state that shows whether a thread remains pending, gets received, or becomes held by both.

Disposition: implemented a language-only handshake truth field in `correspondence_v1`: active threads now expose `mutual_ack_state`, `held_by_both`, aggregate `held_by_both_threads`, and status text that explicitly says this is not auto-ACK or control authority. Existing native message, read receipt, ACK/REPLY/TRACE, legacy claim uptake, and right-to-ignore surfaces remain intact. Live ping/echo or Shadow-v3 cascade persistence trials remain sandbox/runtime work, not hidden authority expansion.
