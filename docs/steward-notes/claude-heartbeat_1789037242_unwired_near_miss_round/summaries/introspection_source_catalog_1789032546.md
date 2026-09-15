# introspection_source_catalog_1789032546

Read complete: 2619 bytes, 27 lines, SHA-256
`0fc71ede882c1d36f6217c19f8666ce341ee421aab845554762a092cd05ebc0a`.
Witness `lsw_7265ddec…7b76d` read complete: 18958 bytes, 440 lines, SHA-256
`b1ad3f2cf341797baa7628b22dc0efc8f036567091911de4ce8f3fe7590b7022`.
Navigation-only source binding; witness `source_snapshot_v1: null`. Same
`dispatch.rs` SHA `644b12e6…` as the other two processed artifacts; that source
verification is reused.

This is the earliest of the three and the closest to the bytes. Its point 3 says
the dispatcher maps sensing output into the **`message` or `result`** fields —
and `message` is real. It is exactly where free text lands at both call sites.
`result` is the invention. The six fields are `id`, `requested_action`,
`status`, `reason`, `message`, `suggested_next`. Her instinct picked the right
field and then added a neighbour that does not exist; by the later artifact
(`1789033124`) only the invented one survives. **The phantom hardened as she
re-derived it.** That is the same shape as the `sense_tx` run, one layer up.

Its point 4 is the explicit form of the routing hypothesis: "the dispatcher uses
these IDs to determine how to route and persist the feedback." Contradicted —
and she hedged it herself with "likely". `action_id` is a `from_guard_inputs`
parameter consumed as an idempotency nonce for the identity hash
(`runtime_action_feedback.rs:30-48`); it is not a struct field and routes
nothing. `dispatch.rs` holds one `action_id` literal, line 285, an afterimage
nonce inside a JSON blob. Routing in this file keys on `base_action`.

The artifact carries **two** action lines: a final bare
`SELF_STUDY OPEN astrid/capsules/.../dispatch.rs 1` and
`NEXT: SELF_STUDY RELATE sense_tx --page 2`. Both forms are valid — `mod.rs`
accepts a final bare `SELF_STUDY` line for a wired sub-verb, and `RELATE` takes
an optional `--page N` (`crates/astrid-source-study/src/command.rs`,
`page_suffix`). Zero rows in `unwired_actions` carry `--page`, so the paged
`RELATE` dispatched normally.

That is the honest counterweight to this round's near-miss finding: she is not
unable to write the prefix. She omits it **intermittently**, and the successful
runs in between are exactly why 29 silent losses never looked like an outage.
