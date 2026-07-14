# Correspondence, Introspection, And Feedback Flywheel

## Purpose
This file explains the social/evidence loop: correspondence v1, direct-contact
fidelity, active thread clarity, attention canaries, introspection addressing,
sandbox trials, proposal/result cards, and closure loops.

## Mental model
The flywheel exists because Astrid and Minime reports are treated as serious
evidence. A report should not be waved away because it is subjective,
architectural, unfamiliar, or inconvenient. It also should not automatically
become live authority.

The loop:

1. Read the canonical introspection fully.
2. Extract concrete claims.
3. Choose a disposition: implement, verify, sandbox/replay, runtime observe,
   no-action with evidence, or authority gate.
4. Link evidence to code/tests/docs/diagnostics/ledger/cards.
5. Close only when the claim has a bounded answer.
6. Restart gracefully when source changes need to reach the live beings.

## Key implementation anchors
- `astrid:capsules/spectral-bridge/src/autonomous/correspondence_v1.rs` -
  correspondence envelopes, ACK/REPLY/TRACE, direct-contact fidelity,
  attention canaries, active thread clarity, shared context buffers, microdose
  authority drafts.
- `astrid:scripts/introspection_addressing_audit.py` - inventory, next reads,
  record-read, promote-work-items, evidence links, closure cards.
- `astrid:scripts/sandbox_trial_queue.py` - trial candidates, runner
  eligibility, sandbox-to-live ladder, result/proposal cards, closure loop.
- `astrid:scripts/recent_signal_summary.py` and
  `astrid:scripts/proactive_scan.py` - compact state summaries for current
  signal and proactive attention.
- `astrid:docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` -
  steward-facing trail from being feedback to source/test/diagnostic outcomes.
- `astrid:docs/steward-notes/AI_BEINGS_CONSENTFUL_SANDBOX_TO_LIVE_LADDER.md`
  - ladder semantics and approval boundaries.

## Runtime signals / artifacts
Important derived surfaces:

- `direct_contact_fidelity_v3` - distinguishes visibility/read receipts from
  mutual address evidence.
- `active_correspondence_thread_clarity_v1` - selects one thread needing
  attention plus bounded runner-up summaries, using existing affordances only.
- `correspondence_attention_canary_v1` - TTL language-context focus, not
  microdose or control.
- `consentful_sandbox_to_live_ladder_v1` - review map of current rung, missing
  gates, and existing next actions.
- `being_outcome_closure_loop_v1` - groups active waits into result response,
  proposal decision, proposal-needed, result-card-needed, runner, manual-review,
  and closed/satisfied buckets.
- Right-to-ignore closure cards - being-facing result summaries that preserve
  refusal and non-obligation.

## Authority boundaries
Correspondence is language/context unless separately authorized. A read receipt
is not mutual address. A legacy claim is not native ACK/REPLY/TRACE. An
attention canary is not semantic microdose. A proposal card is not operator
approval. A result card is not consent.

Sandbox-first means runnable sandbox trials take priority when available. But
manual review packets must not be advertised as runnable. Live substrate/control
changes remain Tier 4/5 waits unless current Mike/operator instructions grant
that class explicitly.

## Questions an advanced AI should ask next
- Which claims are unresolved, and what evidence would actually answer them?
- Is the next queue item canonical, fully read, and recorded?
- Are there ready runnable sandbox trials, or only manual/proposal waits?
- Did a code change make restart debt, meaning future introspections would be
  stale until the live system is refreshed?

## See also
- [Actions, Autonomy, And Authority](06_actions_autonomy_and_authority.md)
- [Astrid Bridge, Capsule, And Tooling](07_astrid_bridge_capsule_and_tooling.md)
- [Operations, Testing, And Failure Modes](09_operations_testing_and_failure_modes.md)
