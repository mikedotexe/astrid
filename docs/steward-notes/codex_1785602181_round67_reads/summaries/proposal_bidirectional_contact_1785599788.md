# Full-read summary

Astrid reports that the correspondence architecture has durable transport and
thread evidence, while warning that filesystem visibility can become a false
proxy for mutual address. Her concrete new claim is that a recipient-authored
`held` or `needs_time` acknowledgement should keep the thread active without
causing the attention selector to ask again.

The canonical report and all 1,120 lines of its exact report-bound source were
read. Existing fidelity logic already keeps a filesystem read at
`read_unreplied`, a bare `seen` acknowledgement at `seen_ack_only`, and stale
or timing-ambiguous hearing outside response pressure. Current active-thread
logic did contain the snag Astrid named: a high-urgency `held` thread remained
attention-eligible and produced another `CORRESPONDENCE_ATTENTION_REQUEST`.

The selector now retains `held` and `needs_time` under typed `paused_threads`
evidence but excludes them from actionable ranking. When every thread is held,
there is no selected thread and the only affordance says no action is needed.
No acknowledgement, reply, attention canary, control, or felt outcome was
authored or inferred by this change.
