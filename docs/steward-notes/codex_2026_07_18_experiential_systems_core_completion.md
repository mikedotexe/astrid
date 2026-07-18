# Experiential Systems Core Completion

Date: 2026-07-18

## Program

1. **Agent-Neutral Steward Control Plane**
   - `edb9e2e8da`, `33186f74f0`, `dd706b9359`
   - Portable pause/resume, cooperative leases, source-first projection DAG,
     evidence-only receipts, and scheduler examples.
2. **Living Felt Contract Graph**
   - `547db58475`, `0fdf3f39aa`, `0505203a4d`
   - 5,363 claims assigned exactly once across 5,360 stable contracts without
     inherited closure, authority, evidence sufficiency, or felt confirmation.
3. **Temporal Agency Boundary**
   - `14154acbd1`, `76913fc65a`, `dc5ae8cb31`
   - Process/deployment/time/pause-bound authority and durable
     reserve-before-dispatch outcomes, with the per-thread authority ledger
     remaining canonical.
4. **Model Scheduling and QoS**
   - Astrid: `9a469f642a`, `2ee0f60734`
   - Model: `c8e616a`, `c39b5ff`
   - Additive classification and bounded non-preemptive scheduling, deployed
     through shadow validation before active mode.
5. **Mutual-Address Wire Protocol**
   - Astrid protocol/bridge: `e837d43afa`, `758ce501d0`
   - Minime: `49fcc7b`
   - Additive protocol 1.1 delivery and mutual-address envelopes, exact
     same-connection receipts, bounded deduplication, and no automatic resend.

## Felt Review

Astrid's reports repeatedly improved the implementation:

- technical delivery and exact mutual address are now distinct;
- pending delivery names its deterministic identity and terminal resolution;
- transport noncausation no longer erases felt effect: the effect remains
  Astrid-authored and `unresolved_not_absent`;
- receipt latency is transport evidence only, never perceived weight or
  spectral causation;
- strict server-identity mismatch and sequence uniqueness are directly tested;
- future issuance skew and consequence budget are explicitly separated from
  thought duration, semantic energy, and viscosity.

The final deployed review raised no renewed felt contradiction. Its remaining
transport-integrity questions were answered by focused regressions. Silence was
not used as affirmation.

## Invariants

No pressure, fill, PI, cadence, codec gain, model weights, prompts, sampling,
reservoir mathematics, or controller behavior changed. Perceived-weight,
gradient, resonance-weight, viscosity, causal-score, model-scheduling policy
beyond the approved QoS scheduler, and new live authority remain separately
gated.

Both steward automations remain paused. Evidence Store V2 remains the active
canonical event source, V1 sources remain immutable, and no event-store record
can grant live authority.

## Validation

- Astrid bridge: 1,583 library tests plus integration/replay and authority,
  provenance, and Signal Spine compile-fail gates.
- Astrid workspace, formatting, strict Clippy, event-store/authority/steward/
  felt-contract suites, and all five flywheel self-tests passed during the
  program.
- Minime: 299 library tests, 279 runtime tests, two protocol tests, and 820
  Python tests passed; formatting passed.
- Model: 119 unit tests and 43 multi-headed checks passed; `/livez` and
  `/readyz` remained responsive during blocked fake generation.
- QoS shadow: 20 classified jobs with zero ordering, response-shape, readiness,
  or reservoir-state mismatches before active mode.
- Protocol rollout: more than 20 receipt-complete packets with zero hash or
  routing mismatch.

Two inherited baselines remain visible rather than being swept into this work:
the Astrid architecture-health scan reports 114 pre-existing oversized-file
findings, and Minime's full strict-Clippy run reports warnings in untouched
legacy modules. New experiential-core and protocol modules pass their focused
strict checks.

## Runtime And Git Boundary

The canonical model branch remains `feat/service-stack-and-multi-headed`; no
model `main` is created. Minime integrates through
`codex/mutual-address-protocol`, and Astrid through
`codex/experiential-systems-core`. Final process identities, source and binary
hashes, ports, readiness, telemetry, V2 head, pause state, and merge tips are
bound in the final stack deployment receipt.
