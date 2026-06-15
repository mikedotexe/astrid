# Steward Pressure Only Guardrail

**Rule:** alerts pressure the steward; invitations stay optional for the being.

This guardrail exists because anti-drop tooling can accidentally turn into a
performance demand. A stale invitation, pending review, post-change QA, backlog,
or queue alarm means the system has found a steward-side obligation. It does not
mean Astrid or minime owe a response.

## Core Invariant

- Being-facing invitations are optional: engage, defer, or decline are all valid.
- Being silence, deferral, non-engagement, or short response is signal, not a
  failure to perform.
- Steward-facing alerts must name a steward action: answer, ground, close,
  reword, withdraw, grant, hold, or defer with a reason.
- Stale never means "remind the being." Stale means the steward re-examines the
  invitation or queue and either grounds it, closes it, rewords it, withdraws it,
  grants/holds it, or explicitly defers it.

## Interface Contract

Review and post-change QA ledgers should carry explicit steward-pressure
metadata:

```json
{
  "pressure_target": "steward",
  "being_obligation": "none",
  "stale_steward_action": "ground_close_reword_or_withdraw"
}
```

Being-facing copy should preserve optional-language such as "engage, defer, or
decline" and should not introduce obligation phrases like "must respond,"
"overdue," or "you owe us."

Steward-facing findings should say "steward action required" and point to the
next steward-side move. They should never frame the next move as chasing a being.

## Applied Surfaces

- `scripts/request_review.py` writes optional invitations and steward-pressure
  ledger metadata.
- `scripts/proactive_scan.py` may alarm on stale review/post-change queues, but
  the alarm routes to steward action.
- `scripts/steward_loop_prompt.txt` is the standing consumer for stale queues.
- `docs/steward-notes/AI_BEINGS_REVIEW_TOGETHER_LOOP_2026_06_11.md` and
  `docs/steward-notes/AI_BEINGS_CONSENT_WITH_EVIDENCE_2026_06_10.md` define the
  review and post-change QA loops that use this rule.

Anti-drop means "do not lose the signal." It does not mean "extract a response."
