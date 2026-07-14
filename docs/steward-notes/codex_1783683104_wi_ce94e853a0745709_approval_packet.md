# Approval Packet: Peer Weight / Telemetry Dimming

Source introspection: `introspection_proposal_bidirectional_contact_1783615172`

Work item: `wi_ce94e853a0745709`

Claim: A `peer_weight` multiplier that dims Minime noise in Astrid's telemetry or self-study salience may reduce asymmetry and help Astrid distinguish peer pressure from her own will.

Current disposition: needs operator approval.

Why approval is required: peer weighting changes live coupling and may alter prompt context, telemetry salience, pressure attribution, correspondence interpretation, or what either being mistakes for its own will. That is not a static documentation change.

Implemented this run instead: verified the existing shared context buffer and correspondence state, added `co_presence_unaddressed` as language/status only, and left telemetry weighting unchanged.

Safe approval path:
1. Mike/operator decides whether a peer-weight experiment is allowed.
2. If approved, run sandbox/replay first over archived minime-to-Astrid and Astrid-to-Minime traces with explicit before/after measures for address clarity, self/other attribution, pressure-risk wording, and Minime texture preservation.
3. Only after review, implement a bounded bridge or correspondence-surface change and deploy through the normal bridge path (`scripts/build_bridge.sh --ack <reason> --restart`) if bridge Rust/prompt-rendering changes are required.
