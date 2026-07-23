# Evidence-to-Study Runtime Portability Dossier

Issue-ready design note against upstream `main`
`1d0f7b6a7259950d0d71202fedc643dd38d73c26` as observed on 2026-07-23.
No upstream pull request should be opened before maintainer issue assignment.

## Problem

Evidence systems often collect measurements before declaring what is being
compared, let a candidate stand in for its missing baseline, or let correlation
quietly become causation. Experiential evidence adds two sharper risks:

- a mechanical pass can be mistaken for felt confirmation; and
- silence can be mistaken for agreement, waiver, or closure.

A small dependency-free study runtime can make those invalid transitions
structurally unavailable without making the evidence system an authority source.

## Proposed Upstream Scope

The first upstream issue should cover only reusable lifecycle primitives:

1. immutable campaign, plan, capture-window, comparison, gap, and review
   receipts;
2. append-only plan revision with the exact prior plan hash;
3. baseline-before-candidate and baseline-before-comparison validation;
4. owner-only bounded scalar fixtures with no prompt, response, report,
   journal, message, vector, or correspondence prose;
5. a bounded asynchronous writer whose exhaustion records a gap without
   delaying behavior;
6. deterministic, idempotent projection receipts; and
7. a schema-aware epistemic linter with injected rule and artifact adapters.

The generic runtime should accept injected clocks, event sinks, identity
resolvers, fixture stores, and projection inputs. It must not depend on an LLM,
an external scheduler, or this fork's Evidence Event Store.

## Reusable Components

| Surface | Reuse posture |
|---|---|
| Record model | Immutable validated records; trusted constructors remain internal |
| Plan lifecycle | Frozen prior hash, bounded revisions, no revision after capture |
| Capture registry | Locked atomic arm/disarm with one owner per sample kind |
| Scalar storage | Owner-only, content-addressed, bounded fixtures |
| Comparison | Mechanical outcome only: difference, no difference, or insufficient |
| Review | Right-to-ignore opportunity; silence remains pending; one campaign opportunity may carry independent per-study receipts |
| Qualitative linkage | Pointer-only source and bounded field-path anchors; no prose copy, scoring, or calculation |
| Epistemic rules | No authority, felt score, causal overclaim, missing baseline, state propagation, or prose |
| Projection | Deterministic outputs, selective checkpoints, idempotent ingestion |

The longer `assemble`, replay, and projection functions remain cohesive
transaction boundaries in the fork and are below the repository's 1,000-line
module limit. An upstream extraction should expose their validation phases as
small injected interfaces rather than copy fork-specific branching.

## Fork-Specific Adapters

The following do not belong in an initial upstream contribution:

- canonical introspection claim, dossier, and lived-state witness identifiers;
- Signal Spine capture fixtures and Astrid's 48D representation registry;
- Minime telemetry, heartbeat, spectral, and deployment identity adapters;
- Astrid's source thresholds and the three initial campaign manifests;
- Felt-Mechanism Concordance, Felt Contract, and right-to-ignore delivery
  projections;
- Evidence Event Store V2 and Projection Runtime V3 integration;
- bridge deployment wrappers, stack receipts, and stewardship ledgers; and
- any capture hook inside Astrid's orchestration, telemetry, heartbeat, or
  codec path.

## Required Boundaries

- A campaign groups studies but propagates no authority, outcome, evidence
  sufficiency, closure, or review state.
- A source constant is not active runtime evidence.
- Matching process and deployment receipts can establish technical identity;
  timing alone cannot.
- Telemetry association is temporal unless the peer supplies an exact receipt.
- A numeric comparison cannot score, contradict, overwrite, or close a felt
  report.
- `no_response` requires a completed opportunity and remains review-pending.
- Review budgets count delivered campaign opportunities, not the number of
  independent study receipts produced from one authored response.
- A qualitative mapping link may point from a mechanical comparison to a
  canonical source and bounded field paths, but cannot copy prose, calculate a
  felt metric, or modify the mechanical outcome.
- A preregistered sample ceiling is bounded completion; queue or write loss is a
  capture gap.
- No receipt can approve, deploy, dispatch, schedule, mutate a peer, or change
  live controls.

## Initial Campaign Lessons

The fork rollout exposed four reusable implementation requirements:

1. an absent natural comparison cohort is `insufficient`, not an identity
   failure; the largest exact available cohort must remain inspectable;
2. a partially appended review must be safely resumable across the local
   receipt, Concordance result, dossier, and projection layers;
3. exact-identity labels in derived review views must carry the process and
   deployment hashes that justify them; and
4. checkpoint verification must recompute the same declared semantic source
   identity used at checkpoint creation, not substitute a whole mutable status
   file hash.

These are generic lifecycle and projection concerns. Astrid's telemetry,
heartbeat, codec, felt-report, and contract adapters remain fork-specific.

## Acceptance

- Standard-library Python and repository-compatible Rust only.
- Owner-only permissions and path portability.
- Deterministic identities and idempotent retries.
- Trusted-construction and untrusted-JSON validation tests.
- Double-arm, expiry, crash, queue-gap, tampering, and fixture-hash tests.
- Exact-identity and temporal-association tests.
- Baseline refusal and silence-neutrality tests.
- Adversarial epistemic-lint fixtures.
- No-capture overhead below 1 ms p95.
- Bit-identical behavior when capture is inactive.
- No private prose or full vectors in study records.
- MIT/Apache licensing preserved.

## Maintainer Questions

1. Should generic study receipts live beside audit types or in a small
   non-authoritative support module?
2. Which upstream identity and provenance types should replace string
   references in a reusable extraction?
3. Should the epistemic linter be a generic schema-policy hook or a separate
   optional tool?
4. Does upstream want capture-window lifecycle without any domain-specific
   sampling adapters as the first issue?
5. Should owner-only local fixture persistence be included, or left to each
   consumer?
