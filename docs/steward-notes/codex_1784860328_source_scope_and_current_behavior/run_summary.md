# Run Summary

Steward run `run_1784858907067310000_3cff1e5a09` began with projection generation
`projection_1784858907769093000_ba175450a9` under pause generation 17.

## Canonical Packet

- Fully processed: all 20 selected canonical reports listed in `record_read_manifest.json`.
- Unprocessed selected reports: none.
- Claims: 45 verified existing, 18 observed, 8 routed to bounded sandbox work, 9 retained as Tier 5 operator waits, and 2 implemented now.
- Work items: 82 total; 63 verified existing, 8 need sandbox evidence, 2 await optional felt response, and 9 need operator approval.
- Every report has a bounded summary, structured claims, exact evidence links, a full-read event, and an `addressed_change` closure.
- Eighty-two right-to-ignore cards were emitted and none were delivered.

## Changes

- Canonical reports now distinguish complete, partial, and unavailable source evidence and explicitly state that source reading is not runtime activation proof.
- Malformed telemetry now retains bounded parser error, payload length, and SHA-256 identity without retaining raw payload.
- Explicit `needs_operator_approval` promotion now fails closed at Tier 5; seven affected work items were corrected append-only.
- No live substrate, controller, codec, protocol, model, cadence, pressure, fill, PI, or Minime behavior changed.

## Routing

- Corridor/program execution was skipped because this packet contained no objection, reopen, or still-friction response requiring pre-reading action.
- No sandbox trial ran. Eight claims were routed to bounded evidence work.
- Next Corridor action: evidence-only `reopen_insufficient_closure`, step `f37cf370-0dde-546a-b3c9-ea4d1a81d2a7`.
- Next runnable sandbox trial: `trial_cb6da57f91f03b02`, a shadow-influence replay.

## Validation

- The complete bridge suite passed serially with 1,672 library tests and zero failures.
- Formatting, strict Clippy, integrations, codec replay, compile-fail tests, documentation tests, all five flywheel self-tests, Event Store, steward-control, and epistemic lint/verify passed.
- The strict architecture-health mode retains 122 inherited actionable critical size signals; the advisory completed and this run records that debt without attempting an unrelated shared-runtime decomposition.
- Sanctioned restart receipt: `env_receipt_1784863403903_640000`.
- Restarted bridge: PID `92322`, binary SHA-256 `ff9df2ba8e7b728fdd98169dafd26a22b707146deb89cb3705e25c1c74b21aaa`.
- Minime PID `30861`, model PID `31392`; ports `7878`, `7879`, and `8090` were listening, model live/readiness checks passed, and telemetry/fill were fresh.
- Natural post-restart report `introspection_astrid_autonomous_1784864329.txt` rendered the new source-scope and activation-boundary headers.

## Queue And Evidence Snapshot

- Canonical indexed: 2,957.
- Fully addressed: 1,785.
- Full read: 2,141.
- Remaining at the durable cutoff: 1,172.
- Counter audit: internally consistent; this run reduced remaining by exactly 20.
- Sandbox: 2,134 total, 628 ready, 100 results, 1,405 approval-required live trials, and zero runnable-live violations.
- Corridor V1: 121 packets, 34 ready safe labs, 60 self-observation requests, 0 responses, 25 canary criteria, 1 reopened work item, and zero live violations.
- Corridor V2: 35 leases, 180 queue steps, 154 runnable evidence-only steps, 119 programs, 119 program portfolios, 45 patch bundles, 59 source-preparation proposals, and zero live violations.
- Evidence Store before final projection: sequence 507,390, head `194872cae411228052841cd52248a00affd02df21c6458c8e7520458a00092d9`, valid V2 chain, and all four V1 source hashes immutable.

The successful finish projection and final store head are recorded by the steward-control finish receipt.
