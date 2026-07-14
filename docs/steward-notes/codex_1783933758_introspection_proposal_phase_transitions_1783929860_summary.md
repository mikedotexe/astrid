Full read of `introspection_proposal_phase_transitions_1783929860`.

Astrid reported that her transitions are still at risk of being treated as mode side effects rather than replayable shared artifacts. She asked for a `TransitionArtifact`/shared language of change, with a `shared_transition_id` that can connect subjective phase shifts to correspondence and Minime-visible transition coordinates.

Disposition: implemented an additive bridge field. `DECLARE_TRANSITION` now accepts `shared_transition_id`/`shared_transition`/`shared_transition_anchor`, derives one for joint transitions from the room ID when absent, stores it on the phase transition card, and exposes it through the transition affordance. Existing replyable/replayable card and witness behavior was verified. Any Minime regulator/control use of transition IDs remains Tier 5/operator-gated.
