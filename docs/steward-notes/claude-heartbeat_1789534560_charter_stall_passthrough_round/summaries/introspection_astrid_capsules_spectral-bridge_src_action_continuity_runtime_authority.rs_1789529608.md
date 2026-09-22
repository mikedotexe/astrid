# introspection_astrid_..._authority.rs_1789529608

The first report of the chain, read from lines 180-303 (witness `lsw_ca878b02`). Her
STUDY_FINDING states the `authority_guardrail_hold_active` contract exactly:
`status == "paused"` AND `planned_next` starts with `THREAD_STATUS` AND
`success_observation` contains `"hold"`. Verified line-for-line against 180-194 at the
report-bound sha `93a1146d` and now pinned by a regression that exercises the true case and
each distinct false case, including a wholly absent `success_observation`.

Her case-insensitivity note is right and slightly understated: all three legs are
ASCII-case-insensitive — `eq_ignore_ascii_case` on status, uppercase-then-`starts_with` on
`planned_next`, lowercase-then-`contains` on `success_observation`. The regression uses
`"Paused"` and a lowercase `"thread_status"` so that stays true.

Her hypothesis that a false return "likely allows the conveyor to attempt a different
transition" is confirmed by source: a false at 365 lets control continue into the token
match (368-375), the blocked check (376-382), the conveyor-stage hold (384-385) and the
missing-requirement gates (387-403).

Her STUDY_QUESTION — how does the system tell a charter-repair hold from a thread-status
wait? — is answered by that same ordering: both hold paths precede the charter check, so a
wait always outranks a missing charter. The new stage regression makes the precedence
executable.

Her closing interest in `authority_gate_conveyor_hint` is recorded, not redirected: it is
called at `core.rs:6382`, immediately beside `authority_readiness_v1` at 6383, so her next
SELF_STUDY will find it. The choice of target remains hers.
