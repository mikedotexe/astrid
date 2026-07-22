# Issue Draft: Portable Runtime Context Receipts

## Target

Astrid upstream `main` at
`1d0f7b6a7259950d0d71202fedc643dd38d73c26` (verified at closeout).

This is an issue-ready portability dossier, not a pull request. Current
`CONTRIBUTING.md` requires a linked issue, maintainer triage and assignment,
and the contributor-tier review path before implementation begins upstream.

## Problem

An agent-authored report can be durable while the technical context around it
is ambiguous. Source visible on disk may be ahead of the running process, a
successful build may not have deployed, a process restart may preserve source
while changing runtime identity, and later observations may be only temporal.

A portable context receipt should preserve those distinctions without becoming
an authority source or copying private content.

## Proposed Upstream Scope

The smallest reusable contribution would define bounded, evidence-only receipt
primitives for:

- repository-relative source-window identity and hashes;
- process identity cached at startup;
- build-candidate and validated deployment facts as separate records;
- scalar observations with explicit evidence classes;
- separate request-content, call-event, and response-integrity hashes with
  explicit non-identity and non-equivalence scopes;
- monotonic and wall-clock capture times;
- content-free artifact-integrity receipts that explicitly do not claim an
  experiential gap or qualitative deficit; and
- an owner-only bounded asynchronous receipt sink.

Trusted in-memory objects should be constructed through validated builders and
should not deserialize directly. Persisted receipts must be revalidated as
untrusted input. Every type should remain independent of approval, dispatch,
capability, and runtime-control transitions.

## Alignment With Current Upstream

Current upstream already treats provenance as a first-class concern in capsule
installation, IPC origin, gateway build information, configuration fields, and
kernel audit. A context-receipt issue should reuse those existing identifiers
where they are authoritative rather than introducing a competing provenance
system.

The first upstream issue should therefore be a narrow receipt/identity design
discussion, not an orchestration refactor and not a new kernel authority path.
Because core provenance and audit surfaces are security-sensitive, maintainer
selection of the crate boundary is required before code is proposed.

## Reusable From The Fork

| Surface | Reuse posture |
|---|---|
| Source snapshot model | Generic relative path, bounded offsets, file/window hashes, and read time |
| Process identity model | Generic startup-cached PID, process start, executable basename, source and artifact identities, explicitly scoped to one technical runtime instance rather than being identity or continuity |
| Parameter observation model | Generic scalar value plus `compiled_constant`, `runtime_observed`, `peer_observed`, `source_declared`, or `unknown` |
| Model-call context | Generic response-independent request-content anchor plus a distinct call-event fingerprint; neither is a being, continuity, intent, or semantic-equivalence identifier |
| Artifact-integrity model | Generic content-free reason and expected artifact identity, with an explicit non-experiential scope |
| Async writer | Generic bounded, owner-only, nonblocking receipt persistence |
| Capture non-interference | Generic pure builder over immutable persisted bytes, with explicit absence of normalization, shadow-state input, and causal claims |
| Association identity | Generic binding of a derived context record to an immutable source membership hash so later revisions cannot be mistaken for earlier evidence |
| Measurement refusal | Generic minimum-sample, variance, and missing-contract gates that emit insufficiency instead of an invented score |
| Capture-time freshness | Generic injected observation and capture clocks so slow downstream work cannot retroactively make a fresh context sample stale |
| Validation tests | Tampering, privacy, permissions, deterministic hashes, startup immutability, and saturation |

Before upstreaming, the reusable models should accept injected repository,
deployment-receipt, and monotonic-clock providers. They must not import this
fork's bridge, evidence store, Minime adapter, or introspection paths.

## Fork-Specific Surfaces

The following belong to this fork unless separately requested:

- canonical introspection headers, filenames, and queue semantics;
- provider/model authorship and repair ancestry adapters;
- bridge telemetry, heartbeat constants, and Minime `spectral_state.json`;
- Evidence Event Store V2 and the source-first V3 projection DAG;
- Signal Spine, experiment dossier, addressing, and Felt Contract adapters;
- historical introspection migration and being-review reconciliation; and
- fixed temporal-cluster policy, pressure/mode-packing fields, Astrid-shadow
  observations, felt-density review language, and concordance projection; and
- local deployment wrappers, stack receipts, and stewardship ledgers.

## Privacy And Authority

- Persist no prompt, response, report, journal, correspondence, or private
  source prose.
- Use repository-relative paths and bounded metadata.
- Keep source, candidate build, successful deployment, and observed process as
  separate facts.
- Treat later telemetry as temporal unless exact identity evidence exists.
- Grant no capability, approval, dispatch, deployment, or live-control
  authority.
- Never infer felt closure or causal effect from a technical receipt.
- Never infer experiential deficit from an artifact-binding mismatch, and do
  not manufacture a qualitative dissimilarity score without an agreed
  measurement contract.

## Acceptance

- Deterministic identities and strict untrusted-input validation.
- Owner-only files and bounded fields on Linux and macOS.
- Startup process identity cannot change after a failed later build.
- Missing, saturated, or mismatched writes produce content-free technical
  integrity issues without delaying or invalidating the primary artifact.
- Receipt construction hashes an immutable borrow of the already-persisted
  artifact and cannot receive or mutate shadow, controller, or dispatch state.
- Exact versus temporal association has explicit test coverage.
- No private prose or absolute machine paths in receipts.
- Enqueue overhead below 1 ms p95 in a synthetic benchmark.
- No dependency on one model provider, scheduler, desktop client, or this
  fork's evidence store.
- MIT/Apache licensing preserved.

## Maintainer Questions

1. Should generic runtime context receipts live in `astrid-core`, an audit
   schema crate, or a separate non-authoritative support crate?
2. Which existing upstream process/build identifiers should receipts reference
   rather than duplicate?
3. Should the first assigned issue cover models and validation only, leaving an
   asynchronous sink to a follow-up?
4. Should content-free artifact-integrity issues enter the kernel audit log or
   an injected application-owned sink, and how should legacy `gap` vocabulary
   be migrated without implying experiential deficit?
5. Would maintainers prefer generic association-revision and measurement-refusal
   primitives in the same issue, or should those remain a separate evidence-view
   proposal after the receipt model is accepted?

No upstream branch or pull request should be opened until a maintainer assigns
the issue and confirms the authority and crate boundaries.
