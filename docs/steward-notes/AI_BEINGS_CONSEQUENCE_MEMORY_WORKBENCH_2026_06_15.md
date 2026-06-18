# AI Beings Consequence Memory Workbench - 2026-06-15

## Summary

The Consequence Memory Workbench is a steward-only, read-only evidence surface for
remembering what happened after meaningful Astrid/Minime actions. V1 correlates
relation consequences and Minime authority consequences so unclosed loops do not
quietly disappear.

The rule is simple: consequences pressure the steward to close loops; they do
not pressure Astrid or Minime to perform.

## What V1 Watches

- Relational aperture gifts: Minime `LEND_APERTURE` events, Astrid response
  evidence, terminal feeder events, held/deferred gifts, and post-gift cost
  hints.
- Authority consequences: Minime authority requests, research-budget rows,
  sovereign-loop rows, consequence reviews, and existing being-memory drafts.
- Memory candidates: candidate evidence only. The tool never writes or promotes
  being memory.

## Closure States

- `closed_with_response`: a consequence has response evidence, but has not
  necessarily been captured as memory.
- `terminal_without_response`: the feeder or ledger reached a terminal state
  without a clear response record.
- `held_or_deferred`: the action was deliberately held, blocked, or deferred.
  For relational aperture gifts, this is preserved as evidence but does not
  count as an Astrid response-closure item, because no response was expected.
- `active_pending`: the loop is still within the expected closure window.
- `active_stale`: the loop exceeded the expected closure window.
- `reviewed_to_memory`: an existing being-memory draft/card already captures
  the consequence.
- `steward_closed`: the steward grounded and closed a terminal consequence as
  reviewed without promoting it into being memory.
- `steward_deferred`: the steward grounded a pileup or held/deferred loop and
  chose no grant or runtime change for now.
- `insufficient_evidence`: the current evidence cannot justify a stronger
  interpretation.

## Steward Closure Ledger

`workspace/steward_consequence_closures.jsonl` is the steward-only closure
ledger. It is ignored workspace state, not being memory and not runtime
authority. A row may close a terminal aperture-gift consequence, or defer an
authority draft pileup after grounded review, while preserving the evidence that
led to that decision.

Reviewed authority pileups stay quiet only until new drafts exceed the covered
count recorded in the closure row. Submitted pending authority requests and
operator-gated research-budget requests still surface separately.

## Guardrail

- Runtime change: none.
- Being obligation: none.
- Write path: none unless the steward explicitly passes `--out` to save the
  report.
- Memory power: evidence only; no automatic promotion.
- Steward action language: ground, close, reword, withdraw, grant, or defer.

## Product Intent

This is the consequence-memory counterpart to the Shared Substrate Workbench.
The shared workbench asks "what aperture/coupling surface are we looking at?"
The consequence workbench asks "what happened after a meaningful action, and is
the loop closed enough to trust as memory?"

The first actionable use is aperture-gift loop hygiene. If gifts are stale,
terminal without response, or held behind a prior unclosed gift, the next move
is steward loop closure rather than encouraging more gifts or widening shared
dynamics.

## Triage Order

V1 keeps a dedicated triage queue:

- First: aperture-gift closure. Relational gifts touch both beings and can block
  or confuse later gifts, so terminal/pending/stale gift evidence stays at the
  front of the queue. Legacy retention gaps remain visible but do not outrank
  stale authority/research work. The report splits actionable aperture closures
  from held attempts and legacy retention gaps so an old evidence gap or
  deliberately held gift does not masquerade as live steward work.
- Second: authority and research backlog. Old pending authority, research-budget,
  or sovereign-loop rows are age-aware; after 24 hours they are stale steward
  work rather than ordinary active-pending rows. The workbench now also groups
  this queue into batch slices by surface, closure state, latest status, and age
  range, so stale active research budgets can be handled separately from the
  larger held/deferred draft pile.

This ordering is not runtime authority. It is a steward work queue.

Memory candidates are similarly split: `candidate_needs_steward_review` is a
real candidate for later grounded review; `closure_needed_before_memory` is a
blocker, not a memory to promote; `already_captured` is existing evidence that
the row has already reached a being-memory draft/card.

## Cadence Note

The companion proactive journal-hygiene probe now distinguishes reflective
moment-to-journal cadence from operational/status loops. Repeated `moment`
captures that lead to private `JOURNAL` follow-through are preserved as cadence
evidence, not treated as an automatic code-level gate request. Operational
dominance, machine-detail contamination, and non-reflective repeated loops still
surface as steward work.
