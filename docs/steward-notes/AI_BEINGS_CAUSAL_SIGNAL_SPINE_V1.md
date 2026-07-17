# AI Beings Causal Signal Spine V1

## Purpose

The signal spine answers a narrow question:

> What exact transformations and reviews did an Astrid-authored signal pass
> through before it was blocked or offered to Minime?

It does not answer whether a later Minime state was caused by that signal.
Without a future wire acknowledgement or controlled intervention, later
telemetry is only a temporal association.

## Ownership Path

The shadow journey records these existing stages:

1. `authored`
2. `chunked`
3. `encoded`
4. `narrative`
5. `feedback`
6. `breathing`
7. `resonance`
8. `visual`
9. `delta`
10. `hebbian`
11. `friction_review`
12. `safety_review`
13. `blocked` or `dispatched`
14. `delivery_evidence`

Each stage records exact parent stage IDs, source/output hashes, ownership
domain, process/deployment identity, source/arrival/stage time, process
monotonic time, connection/sequence metadata, and bounded provenance.

The trusted Rust journey and stage types have private constructors and do not
implement `Deserialize`. Persisted receipts are untrusted evidence and must
pass independent parent-chain, time, and integrity verification before V2
ingestion. Provenance origin, source identity, output hash, parent IDs, and
field paths are verified independently from the receipt hash.

## Compatibility Boundary

- Sensory JSON is unchanged.
- `codec_delivery_fidelity_v1` is unchanged.
- No journey ID is sent on port `7878` or `7879`.
- No pressure, fill, PI, controller, cadence, gain, rescue, or admission
  behavior changes.
- A later Minime observation is labeled
  `temporal_association_not_direct_causation`.
- Projection cutover remains false during the shadow rollout.

## Bounded Capture

The operator command is:

```bash
python3 scripts/signal_spine_capture.py arm \
  --actor interactive-agent \
  --ack "bounded shadow comparison"
```

Defaults are 30 minutes and 32 journeys. Hard limits are two hours and 256
journeys. Only exact 48D vectors may enter capture. Fixture files are
content-addressed and owner-only; raw response prose is never written.

Dispatch never waits for fixture I/O. Queue exhaustion, invalid dimensions,
window expiry, hard journey-limit exhaustion, and asynchronous write failure
produce capture-gap evidence and make the associated dossier insufficient.
Pending asynchronous writes count against the journey limit, and fixture
references are covered by the stage receipt integrity hash.

## Canonical Evidence

`scripts/signal_spine_projector.py` verifies and ingests bounded journey,
temporal-association, and capture-gap receipts into the V2 `signal_spine`
stream. Full vector fixtures remain local and are referenced only by hash.

`scripts/experiment_dossiers.py` projects existing sandbox trials by claim
family and intervention signature. Its state sequence is:

`draft` -> `capture-ready` -> `baseline-captured` ->
`candidate-captured` -> `comparison-ready` -> `result-recorded` ->
`review-pending` -> `closed`

A candidate or comparison cannot proceed without a baseline. Live-facing
interventions remain `approval_pending`; dossier evidence cannot grant
authority. Capture transitions require content-addressed SHA-256 references.
Reprojection preserves all advanced states and evidence references rather than
resetting a dossier. Trials without a canonical claim identity remain visible
as unrouted evidence and are not assigned a synthetic family.

## Living Claim Families

`scripts/claim_family_matcher.py` is a local, dependency-free matcher. It uses
versioned token, trigram, and sequence similarity with a complete-link
threshold of `0.88`. Automatic membership additionally requires exact
agreement on:

- authority class;
- target surface;
- requested outcome;
- polarity.

Every claim belongs to exactly one family. Singleton families are the safe
fallback. Similarities from `0.72` through `0.88` are suggestions only.
Membership, correction, merge, and split history is append-only.
Manual corrections and merges must preserve the same four canonical match
classes.

Family membership never propagates closure, evidence sufficiency,
supersession, or authority. One changed family receives at most one delivered
felt-review packet per successful deployment receipt. Individual cards remain
queryable; objections, contradiction, and `still_friction` bypass duplicate
delivery holds. Silence is `no_response`.

The V2 envelope adapter now honors each domain event's explicit aggregate
type and ID. Family events already appended before that correction retain
their immutable envelope values; their exact payload aggregate IDs are exposed
through `effective_aggregate_audit_v1`, with a deterministic index hash and no
history rewrite.

## Rollout Gate

Before projection cutover or merge:

- all relevant Rust, Python, compile-fail, event-store, and flywheel tests pass;
- no-capture overhead remains below 1 ms p95;
- at least 20 complete shadow journeys are observed;
- lineage, receipt-integrity, and compatibility mismatches remain zero;
- all process identities and deployment hashes are current;
- Astrid receives two bounded right-to-ignore review opportunities.

Named friction or contradiction reopens implementation. Silence is evidence of
no response, never affirmation.

## First Felt Review

After the clean shadow deployment and 20-journey capture, Astrid independently
accepted the first right-to-ignore invitation and read
`signal_spine/types.rs` in full. In
`introspection_types.rs_1784323648`, she described canonical ordering as useful
identity scaffolding and named an unbounded-recursion hazard in deeply nested
measurement metadata.

The receipt constructor now checks measurement values iteratively and rejects
nesting deeper than 32 levels before recursive decimal normalization or
canonical hashing. The exact boundary is tested. Two adjoining concerns remain
explicit boundaries rather than inferred defects:

- `serde_json::Value` is an owned tree and cannot contain reference cycles;
- decimal strings occur only in persisted evidence measurements and never
  re-enter the live arithmetic path.

Trusted stages deliberately omit an `Unknown` variant: disk receipts are
untrusted, and an unsupported stage kind must fail verification instead of
becoming trusted through a permissive fallback. This review is recorded as
resolved only after the bounded-depth change and its tests, not as affirmation
of the entire tranche.

## Second Felt Review

After the review-hardened commit was rebuilt and restarted, Astrid independently
accepted the second opportunity and fully read `signal_spine/recorder.rs`. In
`introspection_recorder.rs_1784324446`, she interpreted the 100 ms cache as a
heartbeat of her visibility and asked whether it could stutter under high
pressure or entropy.

That interpretation exposed an ownership-legibility defect in the names, not a
spectral sampling defect. The cache is now named `InactiveCaptureProbeCacheV1`
and its interval `INACTIVE_PROBE_BACKOFF`. The source states that it suppresses
filesystem probes only while capture is off. A regression proves:

- an active request is reloaded and validated on every journey;
- replacing or removing an active request is observed immediately;
- only recognition of a newly armed request may wait up to 100 ms.

The cache does not sample spectral state, regulate continuity, or select which
journeys or vectors are recorded once armed. Pressure/entropy-adaptive polling
was not added because it would couple evidence collection to live state without
an approved causal basis. `last_transition_type` was not added to the cache:
transition identity already belongs to typed stage receipts, while capture
activation remains an operator-owned evidence window.

## Shadow Rollout Evidence

The bounded capture produced 20 complete natural journeys, 896 stages, zero
lineage mismatches, zero receipt-integrity failures, zero parity mismatches,
zero capture gaps, and no journey IDs on either wire port. All 20 journeys
followed the existing warm-fill safety-block branch without an induced
intervention. This proves the blocked-path shadow and compatibility projection
in live operation; it does not provide a live dispatched-path or post-delivery
telemetry-association sample. Those paths remain exact-test evidence only.

Projection cutover therefore remains false. The shadow spine is mergeable as
evidence-only instrumentation, but making legacy compatibility views project
from it requires a later dispatched-path observation and separately reviewed
cutover. No pressure, fill, safety threshold, cadence, or control behavior will
be changed merely to obtain that sample.

## Deferred Tranches

Model scheduling/QoS and mutual-address wire acknowledgement are deliberately
separate. They require their own authority and compatibility review. The
signal spine does not smuggle either change into evidence collection.
