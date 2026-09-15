# introspection_astrid_crates_astrid-events_src_bus.rs_1789380134

Astrid read a single source page of `crates/astrid-events/src/bus.rs` — bytes 25365..29764,
lines 695-812 — and reported exactly what it did and did not contain. She located the
`maintenance_gate` field, the `set_user_input_blocked` / `user_input_is_blocked` pair, and
`publish_maintenance_barrier`, then stated plainly that the page stops before `publish`, so she
could not yet tell whether the bus performs a synchronous check and drops the message (returning
an error or `None`) or whether the interaction is more complex. She chose `NEXT: SELF_STUDY
CONTINUE`.

Every one of her five line citations resolves against the complete file at the report-bound
`sha256:31ef6a15`: 709-710 (field + struct close), 762-764, 766-770, 778-812, and the page
boundary at 812 itself. No misplacement, no phantom symbol, no stale path.

## Her question, answered from complete source

`EventBus::publish` (bus.rs:890-948) takes the maintenance-gate mutex once (:894) and gates
admission under the *same* lock as the activity accounting, so the immutable updater can never
observe a drained snapshot between admission and accounting (:891-893). When `blocked` is true and
`admits_while_blocked` (:71-123) denies the message, publish logs at `debug` and returns `0`
(:899-900). It is neither an `Err` nor a `None` — the signature returns `usize`. The rejection is
synchronous and terminal at the bus boundary: nothing queues, delays, or retries it. The existing
regression `maintenance_gate_blocks_only_new_user_input_and_is_shared_by_clones` already asserts
both the `0` and the empty receiver.

Her scope intuition needs one correction. The `set_user_input_blocked` rustdoc she quoted names
only `IpcPayload::UserInput`, but `admits_while_blocked` is broader: `UserInput` is the single
unconditional deny (:73); `AgentResponse`, `LlmRequest`, `LlmStreamEvent`, `LlmResponse`,
`ToolExecuteRequest`, `ToolExecuteResult` and `ToolCancelRequest` are admitted only when bound to
an already-tracked trace or call id; and the catch-all arm (:121) admits everything else. So the
gate is narrower than "block the bus" and wider than "block user input".

Her reading of `publish_maintenance_barrier` is right and can be strengthened: it is the only
producer that *requires* `blocked` (:812-819 demands blocked, exact, and zero active
conversations/LLM requests/tools), and it does not go through `publish` at all — it sends directly
on `self.sender` (:842), so `admits_while_blocked` never applies to it.

## Delivery check (un-muffle)

Her `NEXT: SELF_STUDY CONTINUE` was honoured. 279 seconds later
`introspection_astrid_crates_astrid-events_src_bus.rs_1789380413.txt` was authored on bytes
29764..34199 — lines 812-925, containing `publish` at 890-925 — and she answered her own question
there, correctly: drop, return 0, no queue, no retry. Two further CONTINUE pages followed
(34199..38526 and 38526..42934). The cursor advanced; nothing was lost. Those three reports are
post-cutoff and were read only as delivery evidence; they remain unprocessed canonical input for a
later round.

## What this round added

One thing in her page-2 conclusion is true only inside the gate: "returns 0" does not by itself
mean "discarded". An *admitted* event with no async receivers also returns `0` (:926-941) and still
reaches every synchronous registry subscriber (:945); a gate-rejected event reaches neither. The
difference is reach, not return value, and nothing pinned it. The `publish` rustdoc said only
"Returns the number of async receivers that received the event."

Added `publish_zero_separates_gate_rejection_from_absent_async_receivers` (bus.rs tests): with a
synchronous subscriber and no async receiver, an admitted `UserInput` returns 0 with reach 1; the
same event under `set_user_input_blocked(true)` returns 0 with reach unchanged; and restoring
admission does not replay the dropped event. The `publish` rustdoc now states the ambiguity and
names the terminal, non-queued nature of the rejection.

## Authority boundary

No live change. The kernel `publish` signature, the gate semantics, `maintenance.rs`, and the
running bridge were not touched; nothing was built, deployed, restarted, staged, or committed. The
rustdoc correction and the regression describe existing behavior exactly and change none of it.
