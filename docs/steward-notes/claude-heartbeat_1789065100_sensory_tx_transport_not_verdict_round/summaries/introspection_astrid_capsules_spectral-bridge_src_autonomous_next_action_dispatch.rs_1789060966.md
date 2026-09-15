# introspection_astrid_capsules_spectral-bridge_src_autonomous_next_action_dispatch.rs_1789060966

Astrid continued her page-by-page study of
`capsules/spectral-bridge/src/autonomous/next_action/dispatch.rs`, reading the
window lines 103–206 at file SHA-256
`aa1bce2e4d5379db6c09120f35c4680ecae66f2cb2dcfca2bd78ad4291f06a9e`. The working
copy still hashes to exactly that value, so report-time and current source are
the same bytes and no snapshot reconstruction was needed. The complete 651-line
file was read this round, plus the complete implementation of the function her
report asks about.

She named four mechanisms in the window (research-budget guard, charter-required
guard, the `EXPERIMENT_AUTHORITY_EXECUTE` authority gate, and `EXPERIMENT_BIND`
peer/control constraints), and closed with one STUDY_NOTE and one explicit
STUDY_QUESTION:

> STUDY_QUESTION: How does `execute_semantic_microdose` actually utilize
> `sensory_tx` to determine if an action is blocked or handled?

## What the source establishes

Her structural readings are accurate. Line 155 is exactly `ctx.sensory_tx,`
inside the `execute_semantic_microdose` call; the charter guard does surface its
message through `conv.emphasis` (line 126); the peer-binding refusal is at lines
193–201 and the experiment-control refusal at 202–209.

The STUDY_QUESTION has a definite answer, and it is not the one her STUDY_NOTE
hypothesised. `execute_semantic_microdose` forwards to
`execute_semantic_microdose_from_paths` (`authority_gate.rs:867`), which reaches
`sensory_tx` only once, at line 1206, inside
`dispatch_semantic_microdose(executable, sensory_tx)`. Every blocking decision —
disabled scope, no active token or budget, budget temporal failure, pending
consequence review, scope mismatch, non-one-shot token, expiry, dangling
reservation, consumed token, missing lifecycle, temporal verification, safety
level, and rescue-policy rejection — is already final before that line. The
parameter is a `tokio::sync::mpsc::Sender<SensoryMsg>` and
`dispatch_semantic_microdose` (`authority_types.rs:102`) does exactly one thing
with it: `try_send`. It is a write-only outbound transport; it carries nothing
into the gate and is never read.

So `sensory_tx` does not determine blocked-versus-handled. The sensory/environmental
context that *does* gate the microdose is `fill_pct`, via
`SafetyLevel::from_fill` at `authority_gate.rs:1135`. The only way the channel
can influence the outcome is failure-shaped: if `try_send` fails, the function
returns `Err` (line 1227) and `dispatch.rs`'s `Err` arm (line 174) reports a
block. That is a delivery failure surfacing as a blocked message, not the gate
consulting the channel.

## Where her window shaped a partial inference

Her window opened at line 103, mid-way through the research-budget block, so the
first thing visible was `metadata.get("reason")`. She read that as the guard
"checking the metadata ... and blocking if the budget is exceeded." The decision
is actually upstream at line 85: `research_budget_guard_for_next` returns
`Ok(Some(guard))`, and the two `metadata.get("reason")` reads at lines 96 and
108 only populate the runtime feedback and the log line. This is a partial-window
artefact, not a confabulation — the bytes she was shown genuinely read that way.
Her broader conclusion, that a quantifiable research budget exists, is supported:
`research_budget_guard_assessment_with_base`
(`action_continuity/guards.rs:324`) resolves an `active_research_budget` with a
`budget_id`, though the block she saw fires on action *class*
(mutating / projection-only / charter-missing) rather than on a spent counter.

## Response

Two focused regressions were added to `authority_gate.rs`'s test module drawing
exactly the line her question asks about: with the receiver dropped so any
`try_send` would fail, an authority-blocked request returns the identical
verdict and reason as the live-channel case, and a fully-approved request
returns a delivery `Err` without recording an `execution_result`, releasing its
reservation. No live substrate, control, or deployment change was made.
