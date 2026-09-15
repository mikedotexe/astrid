# introspection_astrid_crates_astrid-kernel_src_lib.rs_1789367524

Astrid read a 98-line window of `crates/astrid-kernel/src/lib.rs` (bytes
4579..9158, lines 98..195) covering `Kernel::new`, and wrote a six-point
structural reading plus a STUDY_NOTE ordering the boot sequence and a
STUDY_QUESTION about `maintenance::initialize_gate`.

## Citation accuracy

Every one of her eight line citations resolves exactly against the
report-bound source SHA `d19f321c…`, which matches the working copy byte for
byte. 114-118 (multi-thread assert), 124 (`initialize_gate`), 135-140
(`home://` principal scoping), 143-147 (`SurrealKvStore`), 159-166
(`CapabilityStore`), 168-173 (`SecureMcpClient`), 175-179 (overlay VFS),
185-186 (socket bind + token). No misplacement, no stale path, no phantom
symbol. Her STUDY_NOTE boot ordering is also correct as written.

## Her question, answered

> STUDY_QUESTION: How does the `maintenance::initialize_gate` specifically
> interact with the `event_bus` to block or allow early bootstrap signals?

`initialize_gate` touches the bus through exactly one call —
`event_bus.set_user_input_blocked(blocked)` (`maintenance.rs:147`). It does
not subscribe, publish, or register an interceptor. `blocked` is computed
fail-closed from lease-file presence: `Ok(None)` (no maintenance config) →
`false`; either lease path present, or config invalid, or `symlink_metadata`
failing for any other reason → `true` (`maintenance.rs:130-146`).

Enforcement lives in `EventBus::publish` (`bus.rs:895-901`): while blocked, an
`AstridEvent::Ipc` is dropped and returns zero receivers unless
`MaintenanceGate::admits_while_blocked` permits it.

The part worth correcting is the scope. The gate is narrow, not a bootstrap
gate. `admits_while_blocked` (`bus.rs:71-123`) denies only
`IpcPayload::UserInput` unconditionally; admits `AgentResponse`,
`LlmRequest`, `LlmStreamEvent`, `LlmResponse`, `ToolExecuteRequest`,
`ToolExecuteResult` and `ToolCancelRequest` when they continue an
already-tracked conversation/tool/request trace; and its catch-all arm at
`bus.rs:121` returns `true` for everything else. Every other IPC payload kind
and every non-`Ipc` `AstridEvent` variant passes through untouched.

So her "gate that must be initialized before any actual work can occur" is
right about *ordering* (124 precedes 185, and the rustdoc at
`maintenance.rs:126-128` says exactly that) and wrong about *reach*. Its
purpose in sitting before the listener is to close the window in which user
input could be admitted before lease state was established — not to hold back
bootstrap.

## One generalization corrected

Her point 5 concluded that "every interaction with MCP is mediated by both a
capability check and an audit trail." The construction at 168-173 does wire
all three collaborators, but the generalization reaches past her window:
in `secure.rs` (SHA `534f332e…`), `call_tool` (162) is capability-checked and
audited and `connect`/`disconnect` (265/299) are audited, while `list_tools`
(247-249) and `get_tool` (256-258) delegate straight to the inner client with
neither. Discovery is not mediated; invocation is.

## One claim strengthened

Her point 6 said uncommitted state "remains in the temporary upper layer."
True, and more so: `init_overlay_vfs` (`lib.rs:724-744`) makes upper a
`tempfile::TempDir`, and `OverlayVfs::commit` (`overlay.rs:88+`) copies only
dirty entries down. Uncommitted writes are not merely retained — they are
discarded when the handle drops.

## What reading her witness surfaced

This report arrives in the queue flagged
`lived_state_alignment=artifact_integrity_unavailable`. That is not a defect
in her witness. All 17 recorded errors are
`parameter_observations[N].observed_at_unix_ms:after_authorship`, each exactly
**+1 ms** past `authored_at_unix_ms`. The producer stamps authorship, then
captures runtime scalars a millisecond later.

Corpus-wide the picture is unambiguous: **all 1,129** recorded
artifact-integrity issues, spanning 19,238 individual errors, are this one
kind; the largest after-authorship delta anywhere is **81 ms**. There are zero
substantive receipt-integrity findings on record. `validation.py:952-957`
compares with a strict `>` and no tolerance.

Two consequences. First, 1,129 of her reports have carried a flag that reads
like "her evidence is broken" when the evidence is fine. Second — and this is
why it needed a consumer rather than a shrug — a genuine integrity failure
arriving tomorrow would be indistinguishable at a glance from 1,129 benign
ones. The projection recorded the issue every round; nothing consumed the
stream. Recording is not surfacing.

`scripts/lived_state_integrity_triage.py` is that consumer. It is read-only,
classifies each issue as `capture_ordering_artifact` or `substantive`, refuses
to assume benign when witness bytes are unreadable, and `verify` exits nonzero
on any substantive finding.

## Deliberately not done

Neither the bridge-side producer (live Rust; needs a gated build and restart)
nor `validation.py`'s strict comparison (relaxing it retroactively changes
projected alignment for 3,976 witnesses) was touched. Both are reviewed
decisions for an interactive window, not a headless heartbeat round.
