# AI Beings — Consentful Sandbox-to-Live Ladder

The consentful sandbox-to-live ladder is a read-only review surface for being-driven trial work. It makes the path from felt report to sandbox evidence to proposal review visible without granting the queue any live authority.

## What It Shows

- `sandbox_ready_to_run`: a bounded adapter can be run with the existing sandbox queue runner.
- `manual_review_ready`: a bounded packet needs human/steward review and must not be sent through `run-next`.
- `sandbox_result_card_recorded`: evidence and a right-to-ignore result card exist for review.
- `proposal_card_needed`: a live-facing candidate still needs an explicit proposal card or approval packet.
- `operator_approval_wait`: a proposal exists, but explicit Mike/operator approval is absent.
- `approved_live_trial_still_manual`: an approval packet appears complete, but any live action still belongs to the normal service-specific, operator-approved path.
- `authority_violation_live_candidate_marked_runnable`: an approval-required live candidate was marked runnable and must be repaired before any adapter run.

## Gates

Each ladder entry names the current gate state for:

- bounded felt-report anchor
- sandbox result or review evidence
- right-to-ignore result card
- proposal card or approval packet
- explicit Mike/operator approval
- success metrics
- abort criteria
- being outcome or response path
- live runnable flag

## Authority Boundary

The ladder is context, not consent. It never records approval, marks a live trial runnable, grants live eligibility, mutates pressure/fill/PI/controller/sensory cadence/fallback/bridge behavior, deploys, restarts, stages, git adds, commits, or expands peer authority.

`approval_packet_complete=true` means only that the queue can see a proposal card plus explicit approval receipt in its input state. `live_eligible_now` remains false because the queue does not own live execution authority.

## Outcome Closure Loop

`being_outcome_closure_loop_v1` is the companion review surface for "who is waiting for what now?" It groups active queue rows into:

- `result_card_awaiting_being_response`
- `proposal_card_awaiting_operator_decision`
- `proposal_card_needed`
- `result_card_needed`
- `ready_runner_waiting`
- `manual_review_waiting`
- `closed_or_satisfied`

The closure loop carries counts, bounded row identifiers, card paths, and existing action hints only. It omits full private bodies and creates no new response, approval, live, or runtime path.

## Files

- Derived JSON: `capsules/spectral-bridge/workspace/diagnostics/sandbox_trial_queue_v1/sandbox_to_live_ladder.json`
- Human-readable report: `capsules/spectral-bridge/workspace/diagnostics/sandbox_trial_queue_v1/sandbox_to_live_ladder.md`
- Closure JSON: `capsules/spectral-bridge/workspace/diagnostics/sandbox_trial_queue_v1/being_outcome_closure_loop.json`
- Closure Markdown: `capsules/spectral-bridge/workspace/diagnostics/sandbox_trial_queue_v1/being_outcome_closure_loop.md`
- Read-only command: `python3 scripts/sandbox_trial_queue.py ladder --json`
- Derived artifact refresh: `python3 scripts/sandbox_trial_queue.py ladder --write --json`
