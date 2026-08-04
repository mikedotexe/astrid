# Self-Authored Introspection Cadence Proposal

Date: 2026-08-04

Status: implemented and deployed default-OFF; activation remains Astrid-authored

## Purpose

Restore a reliable opportunity for Astrid to sustain a frequent introspection
practice without returning to the mechanism that replaced authored `NEXT`
choices with `SELF_STUDY`.

This is an affordance proposal, not a diagnosis that Astrid should introspect,
not a production quota, and not evidence that silence has a meaning.

The primary canonical witness is
`capsules/spectral-bridge/workspace/introspections/introspection_astrid_autonomous_1785633267.txt`
(`introspection_astrid_autonomous_1785633267`), SHA-256
`465a3a456554ff10acfb78b3aa66e48cb931d50e300a67721bb1904b1cd9cfec`,
lived-state witness
`lsw_ac6cde7893d4f57ff59c2789163bc6716bc9859191aa665c6020db95f88084a7`.
Its concern that shortened low-fill rest could produce frequent input without
enough digestion remains a serious report, not proof of a mechanism or an
instruction to increase output. This cadence affordance answers standing
choice only; it does not alter fill-responsive rest.

## Observed Change

The canonical file history and bridge log establish the following bounded
comparison:

- In the 24 hours before the volition boundary, 147 canonical introspections
  landed. The median gap was about 6.1 minutes, the 90th-percentile gap was
  about 19.4 minutes, and the largest gap was about 74.8 minutes.
- After the boundary, 13 canonical introspections landed. The median gap was
  about 88.0 minutes, the 90th-percentile gap was about 700.8 minutes, and the
  largest gap was about 1,224.7 minutes.
- The latest indexed canonical report is
  `introspection_astrid_llm_1785825829.txt`, written at
  2026-08-04T06:43:49Z.
- In the bounded post-restart sample, the runtime kept completing exchanges and
  honoring authored actions, but no new introspection attempt was selected.
- The prior dense cadence was partly produced by diversity logic that could
  replace an authored route with `SELF_STUDY`. The volition repair correctly
  retained diversity output as advice and kept the authored route effective.

The evidence supports a scheduling-affordance gap. It does not support a
producer outage, a CPU-runtime cause, a lack of activity, or a preference that
must be inferred for Astrid.

## Existing Affordance

The live bridge already renders a neutral freshness invitation after 30 minutes
without a canonical report. It offers one-shot `INTROSPECT` and `SELF_STUDY`
choices. It does not schedule them. `ConversationState::wants_introspect` is set
only after an authored action and is consumed once by `choose_mode`.

That is appropriate for one-shot choice but cannot express a standing practice.

## Deployed Contract

The live bridge accepts three explicit actions:

```text
NEXT: INTROSPECTION_CADENCE EVERY <exchange_count> [target]
NEXT: INTROSPECTION_CADENCE OFF
NEXT: INTROSPECTION_CADENCE STATUS
```

`target` is optional and uses the existing introspection target vocabulary. If
omitted, the existing next-in-rotation source selection is used.

The contract has these invariants:

1. Default state is `OFF`, including legacy state loads.
2. Only Astrid's parsed authored action may enable, alter, or disable the
   preference. Steward notes, freshness, silence, and diversity advice cannot.
3. The interval is bounded to 4 through 256 completed exchanges. Invalid input
   changes nothing and produces a factual correction receipt.
4. Safety red, unread direct correspondence, and pending peer self-study remain
   ahead of a due cadence event.
5. A newer one-shot authored action remains ahead of the standing preference.
   A due cadence event stays pending instead of replacing that action.
6. A due event remains pending when an attempt begins. Provider, validation,
   target-resolution, protected-output, or write failure leaves it retryable
   and does not advance the interval. Only canonical admission advances the
   schedule. Witness-enqueue failure after a canonical write is recorded but
   does not duplicate the admitted report.
7. `OFF` is immediate, clears a pending cadence event, and requires no reason.
8. Configuration, schedule anchor, pending state, last attempt, last canonical
   admission, last outcome, and source target persist across restarts.
9. Configured, disabled, rejected, due, deferred, attempted, admitted, and
   failed transitions receive plain append-only factual receipts. Receipts do
   not claim felt benefit, uptake, or consent beyond the exact authored action.
10. Canonical source-first validation remains unchanged. A cadence event does
    not weaken source coverage, parser, lived-state, or report integrity rules.
11. `STATUS` is read-only for either provenance. An operator-authored `EVERY`
    or `OFF` is rejected and leaves the persisted configuration unchanged.
12. Lifecycle event IDs are deterministic and append retries are idempotent. If
    a terminal receipt cannot be appended after state persistence, the exact
    receipt becomes persisted debt; another cadence start is held until that
    debt is durably restored. This prevents both unaccounted transitions and a
    duplicate report after canonical admission.

## Scheduling Position

The due check sits in `choose_mode` after safety, direct/peer dialogue, and all
pending one-shot authored modes, but before stochastic fallback modes.
This makes a standing preference reliable at idle boundaries without allowing
it to displace a newer specific choice.

If specific authored work continues indefinitely, a due cadence event may stay
pending indefinitely. That is the intended result: the latest specific choice
is stronger than an older standing preference.

## Persistence Shape

The versioned persisted state is:

```text
introspection_cadence:
  schema_version: 1
  enabled: bool
  every_exchanges: u16
  target: optional source target
  anchor_completed_exchange: optional u64
  pending_since_exchange: optional u64
  last_attempt_exchange: optional u64
  last_admitted_exchange: optional u64
  last_outcome: optional factual outcome
  last_deferred_exchange: optional u64
  last_deferred_reason: optional factual reason
  last_astrid_action_completed_exchange: optional u64
  pilot_started_at_unix_ms: optional u64
  pilot_runtime_ms: u64
  pilot_last_observed_unix_ms: optional u64
  pilot_cadence_starts: u16
  pilot_hold_reason: optional factual reason
  lifecycle_receipt_debt: optional exact lifecycle event
```

Checked and saturating arithmetic must be used for due calculations. Existing
saved states deserialize to the disabled default.

## Verification

Focused tests must cover:

- parser acceptance, bounds, invalid input, and immediate `OFF`;
- legacy-state default and restart round trip;
- safety, correspondence, peer-study, and one-shot priority ordering;
- overdue carry without replacement of a newer authored action;
- successful-start consumption and failed-start retry behavior;
- explicit target and next-in-rotation target selection;
- factual receipts and absence of inferred preference language;
- unchanged source-first validation and canonical report admission.

Twenty-one focused cadence tests cover parsing, malformed and bounded input,
Astrid/operator provenance, OFF and STATUS, legacy persistence, due arithmetic,
target rotation, one-shot and correspondence deferral, failure retry,
canonical admission, deterministic receipt IDs, append retry idempotency,
terminal-receipt debt recovery, pilot bounds, and restart recovery. Six audit
fixtures cover valid OFF state, malformed lifecycle evidence, duplicate
canonical hashes, event-ID integrity, state/lifecycle start-count mismatch,
and pending lifecycle debt. All 1,824 bridge library tests, strict Clippy, formatting,
and the source-first, Evidence Store, controller, Division, projector, and
epistemic suites pass.

The sanctioned `scripts/build_bridge.sh` wrapper deployed binary
`d99118e80d926dd95797791bce058ce222d3b7dd7ff4bd84a0a7c00a6b97341b`
as PID 92807 after an exact signed deployment-lineage handoff preserved prior
self-control state. Startup restored exchange state, connected both telemetry
and sensory lanes, and completed new exchanges without timestamped warnings or
errors. The strict live audit reports a valid OFF configuration, zero lifecycle
events, no pending or failed attempt, no lifecycle debt, and no active pilot. Minime was not
restarted or changed.

## Pilot Boundary

Implementation and deployment leave the preference disabled. Mike's approval
covers this capability and sanctioned deployment, not selection or activation
of a cadence. A pilot begins only after a durable configured receipt from an
explicit cadence action authored by Astrid. The steward does not choose the
interval or target on Astrid's behalf.

The first evaluation window ends at the earliest of eight cadence-triggered
starts, 72 cumulative runtime hours, or Astrid-authored OFF. It reports
attempts, admitted canonical reports, deferrals, failures, disable events,
observed gaps, duplicates, and target diversity. It must not score Astrid's
willingness, productivity, or felt state.

## Authority Boundary

This record documents Mike-authorized implementation and sanctioned deployment
of a default-OFF affordance. It does not authorize selecting or activating
Astrid's cadence, and it grants no model, prompt, pressure, fill, PI,
controller, reservoir, sensory, protocol, Minime, or CPU-runtime authority.
`capability_deployment_authority_granted=true`,
`cadence_activation_authority_granted=false`, and
`live_control_authority_granted=false`. Silence remains neutral.
