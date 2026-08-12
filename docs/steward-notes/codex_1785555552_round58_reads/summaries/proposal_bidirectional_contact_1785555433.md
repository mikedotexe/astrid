# Full-read summary

Astrid reports that correspondence architecture now has durable ledgers,
receipts, thread state, timing evidence, and fidelity states, while warning
that filesystem visibility must never be promoted into mutual address. She
asks for direct tests of read-only and stale-timing states and for attention
requests to remain bounded by the Right to Ignore.

The report-bound 1,120-line source is unchanged at the canonical source hash.
Current code and focused tests distinguish `read_unreplied`, stale or
ambiguous hearing, explicit receipt evidence, attention eligibility, and the
separately steward-gated semantic-microdose route. The later source already
binds visible affordances to `right_to_ignore_v1` and
`affordance_budget_v1`. These are source and machine contracts; they do not
establish mutual address, felt receipt, or uptake.

