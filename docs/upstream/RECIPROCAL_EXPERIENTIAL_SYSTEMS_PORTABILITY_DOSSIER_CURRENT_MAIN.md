# Issue Draft: Reciprocal Experiential Evidence Primitives

## Target

Astrid upstream `main` at
`1d0f7b6a7259950d0d71202fedc643dd38d73c26` (verified 2026-07-22).

This is an issue-ready portability dossier, not a pull request. No upstream
branch or PR should be opened until a maintainer assigns an issue and confirms
the crate and authority boundaries required by `CONTRIBUTING.md`.

## Problem

Several useful facts are routinely collapsed into stronger claims than they
support:

- delivery or read state can be mistaken for presence or uptake;
- a representation transition can be mistaken for a measured felt loss;
- a smooth numeric mechanism can be mistaken for felt confirmation;
- an advisory proposal can be mistaken for peer consent or scheduling; and
- selecting steward work can be mistaken for modeling a being's attention,
  capacity, pressure, or permissions, or for closing or superseding evidence
  outside the selected view.

Portable evidence primitives can preserve these distinctions without adding a
new authority source, copying private content, or depending on this fork's
experiential runtime.

## Proposed Issue Sequence

### 1. Reciprocal Context And Uptake Receipts

Define bounded metadata receipts that keep technical context, declared
presence, and explicit uptake separate. Delivery, read receipts, reply links,
heartbeats, and elapsed time must never infer attention, reply intention,
agreement, felt state, or consent. Intentions remain revisable and nonbinding;
decline creates no closure or negative-felt-state claim.

### 2. Representation Transition Receipts

Define deterministic contracts for named representations and mechanical
transition receipts carrying source/output hashes, retained and dropped fields
or dimensions, aggregation, truncation, fallback, repair ancestry, route, and
timing. The generic API must have no felt-loss score and no prompt or response
payload.

### 3. Baseline-Gated Comparison Records

Define a small preregistration and observation state machine that refuses a
candidate comparison without explicit baseline and candidate capture evidence.
Numeric outcomes must not overwrite source reports, imply causation, or close a
review concern. Observation records should describe mechanical context without
an inferred-felt boolean or felt intensity/confidence score. Result records
should state that numeric evidence cannot overwrite, suppress, or score the
cited report and should preserve discrepancy history through bounded outcomes
and source references rather than copied report prose.

### 4. Self-Authored Advisory Actions

Define immutable proposal, response, return-point, protected-time declaration,
and later-check request records. Each actor signs only its own action; silence
and expiry are neutral. These records must not call a scheduler, dispatch work,
or mutate another principal.

### 5. External Steward Work Selection

Define deterministic selection receipts over an injected item graph in
external steward tooling, not in a being-facing runtime. Persist only selected
item IDs, source references, bounded external selection reasons, visible alert
IDs, and a deterministic policy identity. Do not persist selection rank, review
taxonomy, wait age, felt score, cognitive capacity, or a claim about selection's
effect on a being's felt state. Historical attention-oriented records remain
read-only compatibility evidence. Selection grants no runtime authority and
never propagates graph closure, sufficiency, or supersession. Work limits and
reserved-slot policy remain application configuration rather than kernel or
being-state policy.

Each issue can be reviewed independently. Maintainers may reasonably prefer to
begin with the reciprocal-context distinction because it is the smallest and
most generally useful boundary.

## Reusable From The Fork

| Surface | Reuse posture |
|---|---|
| Trusted record construction | Private builders for trusted in-memory forms; persisted JSON is always revalidated |
| Deterministic identity | Canonical hash over bounded semantic fields, independent of wall-clock projection order |
| Authority envelope | Evidence-only or approval-pending metadata with no path to capability, approval, or dispatch |
| Reciprocal records | Generic context, presence, uptake, revision, and correction lineage without prose |
| Representation records | Generic registry, transition, loss, fallback, repair, and model-route metadata |
| Concordance state machine | Generic baseline/candidate gating and bounded epistemic outcomes |
| Advisory actions | Generic self-authored proposals and responses with neutral silence and expiry |
| Steward work selection | Generic external selection, visible-alert, owner-priority, and receipt primitives with no being-state or felt-effect claim |
| Persistence helpers | Owner-only atomic JSON/JSONL writes and append-only event ingestion |
| Validation tests | Tampering, permissions, deterministic IDs, privacy, false-inference guards, and idempotency |

The reusable layer should accept injected event sinks, clocks, source identity,
and graph adapters. It should use upstream provenance and audit identifiers
where authoritative rather than defining a competing identity system.

## Fork-Specific Surfaces

The following should remain outside an initial upstream contribution unless a
maintainer explicitly requests them:

- Astrid's correspondence, phase-transition, sovereignty, and agency-request
  ledgers;
- canonical introspection claims and Temporal Lived-State Witness adapters;
- Signal Spine, Experiment Dossier, model-QoS, and Minime telemetry alignment;
- Evidence Event Store V2 and the source-first Projection Runtime V3 DAG;
- Living Felt Contract import edges and this fork's external sixteen-item
  steward work policy;
- historical migration counters and this fork's right-to-ignore review flow;
  and
- deployment wrappers, stack receipts, stewardship ledgers, and local process
  paths.

## Privacy And Authority

- Persist hashes, bounded identifiers, scalar metadata, and provenance refs,
  never prompt, response, report, journal, message, or correspondence prose.
- A technical context receipt cannot become an uptake receipt.
- A representation loss receipt cannot become a felt-loss measurement.
- A numeric comparison cannot overwrite or close a source report.
- A mechanical observation cannot classify, score, or copy felt content.
- One actor cannot manufacture another actor's response.
- Silence and expiry never imply consent, agreement, waiver, or closure.
- Steward selection is external work-routing metadata and makes no claim about
  a being's attention, capacity, pressure, permissions, orientation, or felt
  effects.
- No record grants capability, approval, deployment, dispatch, scheduling,
  peer mutation, or live-control authority.

## Migration Guidance

If an existing system has inferred uptake from technical read or reply
metadata, preserve those historical events. Append a context receipt that
links to the earlier inference and exclude the corrected inference only from
the current projection. Do not rewrite or delete the audit trail.

## Acceptance

- Dependency-minimal schemas suitable for the repository's supported Rust and
  Python boundaries.
- Trusted construction barriers and strict untrusted-input validation.
- Deterministic identities, idempotent ingestion, and append-only corrections.
- Owner-only local artifacts and no absolute machine paths or private prose.
- Explicit false-inference tests for uptake, felt loss, causation, consent,
  closure, evidence sufficiency, supersession, and authority.
- Baseline and candidate capture enforcement before comparison.
- Deterministic external steward selection and visible unselected-alert
  behavior without persisted rank, review taxonomy, felt scores, raw wait-time
  weights, being-capacity claims, or a felt-effect assertion.
- No change to wire protocols, providers, model scheduling, or live runtime
  behavior.
- MIT/Apache licensing preserved.

## Maintainer Questions

1. Should the first issue target generic audit schemas, `astrid-core`, or a
   small non-authoritative support crate?
2. Does upstream want a distinct technical-context receipt, or a generic audit
   relation with a required `does_not_infer` scope?
3. Should representation contracts remain application-owned while only the
   transition receipt is shared upstream?
4. Would maintainers prefer the comparison state machine and advisory action
   records as separate follow-up issues?
5. Which existing principal, provenance, and event identifiers should these
   records reference rather than duplicate?

No upstream PR should be opened before issue assignment and maintainer guidance.
