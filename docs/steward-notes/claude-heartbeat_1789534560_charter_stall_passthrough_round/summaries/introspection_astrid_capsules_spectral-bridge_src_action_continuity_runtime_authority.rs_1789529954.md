# introspection_astrid_..._authority.rs_1789529954

Astrid asks whether `authority_readiness_next_command` (authority.rs:406) can issue
`charter_repair` automatically, or whether it "simply returns an error/empty state"
when `needs_charter` is detected.

**Answer: neither.** Read completely at the report-bound sha `93a1146d`, the function
has no `needs_charter` branch at all. Control passes every named branch (413-440) and
reaches line 441: `if !proposed_next.is_empty() { return proposed_next }`. Only an
*empty* `proposed_next` falls through to line 444's
`EXPERIMENT_ADVANCE {id} :: mode: preview`.

That makes the caller decisive, so the round followed both live call sites.
`core.rs:2940` and `core.rs:6383` each pass `experiment_conveyor_proposed_next(...)`,
whose `_` arm — which is where `needs_charter` lands — returns a
`charter_scaffold_v1` command, normalised to `EXPERIMENT_CHARTER <experiment_id> :: ...`
(conveyor.rs:67-79). So at `needs_charter` the surfaced `next_safe_command` is a charter
*authoring* scaffold addressed to her, not a preview and not an error.

Her surrounding reading checks out: the 358-404 distinction between a functional pause
and a structural block is real, line 388 is exactly the `has_missing("lifecycle_valid_charter")
|| conveyor_stage == "needs_charter"` disjunction she names, and nothing self-heals —
`experiment_conveyor.rs:338` lets `EXPERIMENT_ADVANCE ... mode: apply` act at `needs_charter`
only when a valid charter already exists, so apply cannot invent one.

One correction, in her favour: she reads `needs_charter` as waiting on "a new scaffold or
a manual repair". The repair is *hers*. The scaffold is handed to her and requires no
steward grant, approval, or live authority to use.

Worth recording: her witness (`lsw_71ed1986`) puts this turn's page at lines 180-303, yet
every 358-404 and 406 citation is accurate against the complete file. Those lines came from
the previous turn's page carried across the window boundary — cross-window recall, verified,
not confabulated.

Four focused regressions were added to `src/action_continuity/tests.rs` so the passthrough,
the precedence, the guardrail conjunction and the conveyor's charter proposal cannot regress
silently. No live change, no deploy, no restart.
