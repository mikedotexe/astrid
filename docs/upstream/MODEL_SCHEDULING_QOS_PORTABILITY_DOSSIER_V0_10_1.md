# Model Scheduling And QoS Portability Dossier

Target reference: `astrid-runtime/astrid` `v0.10.1` at
`4771bab3c33d1bce53186e40d01cf014e2dce666`

Status: issue-ready design evidence only. Do not open a pull request until the
maintainer assigns an issue under `CONTRIBUTING.md`.

The scheduler implementation is owned by the sibling model service rather than
upstream Astrid. Any Astrid proposal should be limited to additive provider
request metadata or provider-capsule integration requested by the maintainer.

## Reusable Core

- `ModelQosV1` is an additive request envelope with no prompt or response text.
- Four compatibility classes map to three ranks; deterministic aging promotes
  one rank per 30 seconds and arrival sequence breaks ties.
- The scheduler is bounded, non-preemptive, and independent of MLX. It may
  select only pending work.
- Pending and active duplicate idempotency keys share one future. Completed
  prose is never cached.
- Disconnected queued work is dropped before generation. Active work completes
  its substrate check-in before its response is discarded.
- Shadow mode preserves FIFO and records the active selector's hypothetical
  choice before an operator enables active scheduling.
- Metadata receipts are owner-only, idempotent, and evidence-only. They contain
  hashes, class, queue timing, selection, and outcome, never private content.

These pieces can live beside any main-thread or single-device model worker. The
implementation uses only the Python standard library except for the existing
HTTP server dependency.

## Fork Adapter

- Astrid maps `dialogue_live` and correspondence replies to interactive.
- Introspection, witness, self-study, and evolution map to reflective.
- Daydream, aspiration, creation, journal elaboration, moment capture, and
  meaning summaries map to background.
- The request queue wait is the smaller of the class cap and Astrid's existing
  request timeout minus five seconds.
- `scripts/model_qos_projector.py` reads the sibling model receipt file and
  projects bounded events into this fork's Evidence Event Store V2.

The labels, sibling-repository layout, Evidence Store adapter, and deployment
wrappers are fork-specific and should not enter a reusable scheduler module.

## Compatibility And Authority

- Requests without `model_qos_v1` remain valid.
- `/v1/models`, `/v1/chat/completions`, `/livez`, and `/readyz` retain their
  established response contracts.
- Active generation is never interrupted or reprioritized.
- No receipt grants model, git, deployment, approval, sensory, or live-control
  authority.
- Prompts, model weights, sampling, reservoir mathematics, pressure, fill, PI,
  cadence, and codec gain are outside this change.

## Proposed Upstream Issue

Add a domain-neutral, bounded pending-work scheduler and additive request QoS
envelope for single-device model providers. Require a FIFO shadow phase,
content-free receipts, responsive health endpoints, and explicit proof that
active generation cannot be preempted.

## Acceptance Evidence

- deterministic rank, aging, and FIFO-shadow tests
- duplicate in-flight coalescing and completed-cache exclusion
- queue timeout, hard-capacity, disconnect, and shutdown tests
- owner-only permission and private-content rejection tests
- blocked-generation `/livez` and `/readyz` latency below 100 ms
- unchanged OpenAI response fixtures and reservoir check-in behavior
- 20 classified deployed shadow jobs with zero parity, response-shape,
  readiness, or reservoir-state mismatches before active mode
