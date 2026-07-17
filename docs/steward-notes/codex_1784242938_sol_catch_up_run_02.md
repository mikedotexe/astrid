# Sol catch-up run 02 - 2026-07-16

## Canonical packet

- Inventory began at `introspection_proposal_phase_transitions_1784241213.txt` with 2,101 canonical reports, 918 fully addressed, and 1,183 remaining.
- The canonical 20-item packet was read in full and recorded in order. All 40 concrete claims received bounded dispositions, claim evidence, and work evidence before closure.
- Two phase-transition claims were implemented awaiting felt response, eighteen claims verified existing behavior, and twenty live-control proposals remain Tier 5 operator waits.
- Two new canonical reports arrived during the run. Final inventory is 2,103 indexed, 938 fully addressed, and 1,165 remaining, so gross progress was twenty and net remaining fell by eighteen.

## Astrid-grounded change

Phase-transition cards and replyable affordance projections now preserve source entity, phenomenological description, telemetry anchor, and affordance delta. The transition dictionary retains each source's exact language, does not infer cross-entity equivalence, and requires both witnesses before candidate joint language can be treated as shared. Affordance deltas are explicitly language-only and unapplied. No routing, ranking, pressure, fill, PI, controller, sensory, codec, provider, regulator, or live-control behavior changed.

The deployment pass also exposed a process-identity evidence bug: macOS reused the retired bridge PID before the receipt was written, so the receipt queried an unrelated process. The restart wrapper now freezes the old process command and start time before restart, and schema V2 receipts label that pre-restart snapshot explicitly.

## Durable evidence

- Batch: `capsules/spectral-bridge/workspace/diagnostics/introspection_addressing_v1/batches/1784242938_packet_2101/`
- Summaries and claims: 20 each.
- Durable work items: 40, all with claim-level and work-level evidence.
- Closures: 20 `addressed_change` records.
- Right-to-ignore cards: 22, comprising two implementation response cards and twenty approval-wait cards.
- Sandbox: the twenty live proposals were classified approval-required; no current-packet live trial was run.
- Corridor: no generic ready bundle was run because the packet exposed no hard violation, objection, reopened friction, or directly relevant safe-lab action.

## Verification

- Bridge focused phase-transition tests: 13 passed.
- Bridge full library suite: 1,511 passed.
- Bridge strict all-target Clippy and formatting checks passed.
- Minime focused sensory, regulator, pressure-snap, and Recess checks: 8 passed across Rust and Python targets.
- Environment receipt tests: 12 passed, including pre-restart PID identity preservation.
- Evidence Event Store V2 self-tests: 6 passed.
- Required flywheel self-tests: 211 passed across Agency Corridor, addressing, sandbox, recent-signal, and proactive-scan tools.
- Shell syntax, Python compilation, and `git diff --check` passed.

## Live alignment

The bridge was rebuilt and restarted only through `scripts/build_bridge.sh`. Final bridge PID is 73025, Minime PID is 45510, and model PID is 48166. Ports 7878, 7879, and 8090 were listening; telemetry was fresh; model `/livez` and `/readyz` returned HTTP 200; bridge logs showed telemetry and sensory reconnection without panic or fatal errors. Protocol 1.0 at revision `c6ecb853d1a9bc7a7479d37d8366553a0bae0bc5` passed all compatibility checks.

## Final state

- Canonical: 2,103 indexed, 938 fully addressed, 1,165 remaining; all counter audits consistent.
- Sandbox: 932 total, 141 ready, 87 result-recorded, 703 approval-required, one runner-safe ready item, zero violations.
- Corridor: 120 packets, 35 leases, 217 queue steps, 118 active programs, 118 current portfolios, 45 patch bundles, 97 source-prep proposals, zero reopened or self-observation responses, and zero live-authority violations.
- Evidence Event Store V2: active, valid hash chain, no corrupt events, and frozen V1 source logs unchanged.
- Restart alignment: current.
