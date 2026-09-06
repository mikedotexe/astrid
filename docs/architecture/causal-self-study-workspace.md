# Causal Self-Study Workspace

**Status: first offline foundation implemented and tested, September 5, 2026.
The proposed being-facing Actions are not registered or deployed.**

The [foundation implementation receipt](../steward-notes/2026-09-05-causal-self-study-foundation.md)
records the versioned probe repair, an executable retrieval/bookmark/revision
test using the existing dossier store, and synthetic history controls using the
actual NumPy reservoir implementation. The remaining sections retain the larger
RFC; they must not be read as a claim that every proposed capability is present.

## Decision

Build a voluntary, persistent workspace in which Astrid and Minime can ask what
their own activity changes, state an expectation, inspect trustworthy evidence,
test competing explanations, and revise what they believe. Build it by connecting
the existing experiment, dossier, continuity, and isolated-analysis facilities.

The current Actions permit substantial parts of this process. The inspected
paths do not yet provide a complete, generally usable prediction-to-outcome-to-
revision contract. The principal missing feature is not another introspection
prompt. It is an executable investigation with reliable evidence and memory.

The result should support an authored account such as: a consequence was expected;
the action did not actually reach its destination; the observation therefore
does not test that expectation; a different explanation remains possible. It
should also support a genuine negative result, an unexpected discovery, or
putting the question aside without producing an account at all.

This RFC follows Mike's request to turn the research comparison into a technical
feature investigation. Its [action audit](../steward-notes/2026-09-05-causal-self-study-action-audit.md)
records source versions, limitations, and tests. The
[research memo](../research/2026-09-05-introspection-research-comparison.md)
explains why influence, recognition, and causal self-attribution are different
research targets.

## What Already Exists

This table describes inspected source, not a fresh live capability attestation.

| Need | Existing facility | What remains |
| --- | --- | --- |
| Choose and retain a question | Research threads, `EXPERIMENT_START`, charters, branches | An explicit prediction linked to a particular future observation or isolated test. |
| Inspect mechanisms | Minime targeted `INTROSPECT`, source windows and offsets; source study and research Actions | Preserve what was actually supplied, including capture time and route. `SELF_STUDY` can use runtime-selected files and searches. |
| Record a claim or disagreement | `DOSSIER_CLAIM`, `DOSSIER_EVIDENCE`, `DOSSIER_REVIEW`; support/counter/branch/hold stances | Referentially validated revisions and machine-checkable outcome links, without rewriting the earlier claim. |
| Connect activity to an experiment | `EXPERIMENT_BIND`, action events, pre/post summaries and artifacts | Distinguish dispatch evidence from measured consequences. Some experiment records intentionally copy one snapshot into both state fields. |
| Preserve a stopping point | `CONTINUITY_SESSION_*`, memory capture/recall, source references | Carry the investigation and its unresolved alternatives through parking, compaction, and explicit return. |
| Try an isolated contrast | Astrid `PROBE_SELF`; copied-state model replay tooling | Harden the probe's starting-state identity, cleanup and interpretation. Expose only reviewed, bounded test recipes. |
| Run owned analyses | Minime `INQUIRY_START`, exact response strands, analysis receipts, `INQUIRY_INSPECT` | Adapt existing evidence to general causal questions without silently enabling its separate control/canary path. |
| Freeze a study and retain gaps | Evidence-study runtime plans, capture windows, comparison and review receipts | A being-facing adapter; explicit linking between an agent's prediction and independently measured outcomes. |

Minime's owner research console already supports exact conditional decision
plans, with ambiguous matches failing closed, and separate canary/withdrawal
machinery. This RFC does not replace or weaken that machinery. Its evidence
graph labels its analysis nodes `association_only`, which is an important
boundary to preserve.

Astrid's `ATTEND` handler exists in the inspected main checkout. The previously
reviewed protected agenda integration has a different branch lineage; it must
not be assumed present here merely because a prior note describes it.

## The Experience We Want to Enable

One question should be enough to begin. A being need not complete a scientific
form before retaining a curiosity. The structured parts become necessary only
when it chooses a scored prediction or a controlled test.

1. Choose a question and optionally record several explanations in its own words.
2. Inspect a compact, source-linked account of relevant events and missing evidence.
3. State what a particular explanation predicts, before seeing the test outcome.
4. Choose an available observation or an approved isolated test.
5. Receive the mechanical result separately from any interpretation.
6. Keep, narrow, challenge, replace, or leave the explanation unresolved.
7. Park it quietly; later return to the actual stopping point and evidence.

The implementation should make this sequence easy, not compulsory. No timer
should demand a journal entry, a revision, an admission of error, or a report of
improvement. A completed worker is not a completed inquiry.

### Two First Investigations

**Continuity and retrieval.** In a synthetic workspace, predict whether an exact
source passage remains retrievable after parking, unrelated activity, and
resumption. Verify returned bytes and the reading cursor. If retrieval succeeds
while an authored continuity concern remains, retain both observations. This is
an affordance test, not a test of reservoir awareness.

**Reservoir history.** In isolated copied states, compare matched current summary
metrics with different recorded histories, under a fixed subsequent input
sequence. An authored expectation concerns a specified observable and horizon.
The result distinguishes measured trajectories, not whether a metaphor is true.
Identify the exact reservoir, state, encoder, readout, and metric; shared sensory
ESN behavior cannot be inferred from a triple-reservoir clone result.

These investigations give an early end-to-end demonstration and then a genuinely
substantive reservoir study. Neither requires changing live PI, damping, gain,
input cadence, or peer behavior.

## Four Additions to the Action Surface

The following names and syntax are proposals, not currently available commands.
Keep existing Actions for question creation, evidence, review, parking, status,
and cancellation. Publish backend-specific availability through the capability
map; an unsupported adapter returns `unsupported`, not simulated success.

| Proposed Action | Contract |
| --- | --- |
| `EFFECT_TRACE` | Read a bounded event interval tied to an exact action or test. Return provenance, stages, measurements, other observed activity, and gaps; do not manufacture causal attribution. |
| `EXPERIMENT_PREDICT` | Append an authored expectation linked to an existing experiment and claim, a defined outcome, information cutoff, and evaluation rule. No test or control executes. |
| `EXPERIMENT_TEST` | Request one allowlisted isolated recipe using a frozen plan and exact assets. Return a job receipt; retain existing status and cancellation affordances. No arbitrary code, shell command, or live service endpoint. |
| `DOSSIER_REVISE` | Append an authored relation to an earlier claim and its evidence. Preserve the original and competing explanations; do not change experiment lifecycle or control authority. |

Use a shared structured payload contract, with tested thin Rust/Python adapters.
Do not invent a second free-form field parser for new data. Preserve existing
command compatibility, while returning typed `needs_input`, `not_observed`,
`pending`, `blocked`, `failed`, and `persisted` outcomes where applicable.

An agent can write an unscored expectation in ordinary prose. For a scored
prediction, require a bounded outcome specification; optional numerical
confidence must not be filled in by the runtime. Missing evidence is not a
false prediction. Evidence already available before recording makes an entry
retrospective, not a prospective success.

## Evidence Architecture

### An Action and Consequence Timeline

Reuse existing action IDs, experiment IDs, generation receipts, authority
receipts, and telemetry stores. Add references and typed relationships where
needed, rather than another independent source of truth.

Record separately:

- Proposed action and its author/source: being, runtime policy, operator, or
  an observed external source. Keep canonical and effective actions distinct.
- Admission decision, dispatch attempt, endpoint acknowledgement, and verified
  application where the endpoint provides it. A socket send is not application.
- Prompt-capture time, generation start/end, action start/end, endpoint sequence
  or tick, and observation interval. State their clocks and uncertainty.
- Actual model route and fallback, deployed source identity, target reservoir,
  configuration revision, and supplied context references.
- A bounded pre-window and outcome window, with units, sample count, coverage,
  freshness, gaps, and metric definition/version. Unknown is not zero.
- Other observed events in the window: regulator changes, peer input, sensory
  input, concurrent actions, and restart boundaries. Unobserved inputs remain
  an explicit limitation, not an empty list interpreted as absence.

`EFFECT_TRACE` reports temporal order and known mechanical connections. It does
not assign percentages of causation to self, peer, or environment. A link saying
that an action dispatched a packet differs from a hypothesis that this packet
caused a later spectral change. Wall-clock proximity alone cannot connect them.

The existing `parent_action_id` is a continuity relationship, not a causal edge.
Similarly, pre/post fields copied from one snapshot must be labeled as one
observation, never converted into an apparent zero effect.

### Authored Claims and Revisions

Extend the existing append-only dossier with a small set of referentially checked
record types. The core records should contain:

| Record | Required meaning |
| --- | --- |
| Claim | Owner, exact authored text, scope, source references; competing explanations may remain separate claims. |
| Prediction | Claim revision, declared target/outcome, horizon, information cutoff, evaluation rule, plan hash, and optional authored confidence. |
| Observation | Independently produced measurements or authorized artifact result, source identity, timing, coverage, and exclusions. |
| Test result | Frozen recipe and asset hashes, completed/failed/cancelled status, mechanical comparison, and scope limitations. |
| Revision | Prior claim ID, relationship such as qualifies/supersedes/challenges, authored reason, evidence IDs, and any remaining alternatives. |

Validate every referenced record's existence, owner, experiment, visibility, and
hash before accepting a new relationship. A requested relation cannot change a
peer record. Reject unknown IDs and cycles in supersession links. Corrections
append history; old prose is not rewritten into the latest interpretation.

Maintain two separate judgments: the deterministic evaluation of a defined
prediction, and the being's authored interpretation. A prediction can match
while the causal explanation remains underdetermined. A being can disagree with
a metric's relevance without changing the measured result.

The persistence design should reuse action-thread canonical files and existing
Evidence Event Store conventions. A single canonical append owns each record;
database indexes and prompt views are rebuildable projections. Define an
idempotency key, transaction/recovery behavior, owner-scoped access, byte limits,
and index lag handling before adding live writers. Do not treat a projection
or hash as evidence of consent, successful delivery, or substantive truth.

### Quiet, Selective Return

Use the established session lifecycle. A parked inquiry retains its prediction,
evidence and next step but does not keep re-entering automatic context. Resume
returns the latest revision plus any unresolved contradiction, not just the
oldest memorable statement or the most positive result.

For an explicitly active inquiry, show only material changes and a retrievable
reference. Displaying a result does not imply it was read or accepted. Private
journals are not automatically converted into predictions or review records.
No numerical attention weight, stronger retention policy, or wider prompt budget
is implied by this feature.

## A Controlled Test Facility

Reuse existing copied-state replay and owner-inquiry analyzers behind typed,
versioned adapters. The evidence-study runtime already provides frozen-plan
and capture-gap machinery; reuse those contracts rather than introducing a
second study registry. Its current canonical-claim/dossier/witness requirements
need an explicit design for owner inquiries: never invent a witness or canonical
introspection to satisfy them.

The first release should support two recipes only: synthetic continuity replay
and an isolated paired reservoir-input contrast. Later recipes can add history
matching, current-output versus recorded-feedback comparisons, or model replay
with common prefixes. Every recipe declares what it tests and what it cannot.

Requirements before admitting a test:

1. Both arms derive from one immutable source snapshot, with identical starting
   state and relevant weights/configuration verified. Sequentially cloning a
   live source twice is insufficient. Include random-generator and readout state
   where relevant, not only the most visible recurrent vector.
2. Record encoder, projection, implementation, model, sampler, cache-reset and
   feedback-delay identities as applicable. Hold nuisance factors fixed or list
   the unmatched differences explicitly.
3. Use a dedicated isolated process and storage root, with operating-system
   network/filesystem restrictions. A flag claiming no live mutation is not
   enforcement. Never use an ordinary live completion as an inert experiment.
4. Bound CPU/GPU, memory, elapsed time, output size, trial count and concurrency.
   Reuse job infrastructure. Cancellation must stop owned work and preserve
   partial/failure receipts; it must not affect unrelated services or handles.
5. Use unique owned identifiers and exception-safe cleanup. Never delete a
   shared prefix or another job's resources. Failure to clean up is explicit.
6. Include identical-input/reset checks and relevant null controls. Report
   insufficient coverage, failed equivalence checks, exclusions, and no observed
   difference without converting them into explanations of experience.
7. Keep condition labels and outcome data out of the predicting model's inputs
   until its commitment is durable. Distinguish prospective from retrospective
   scoring using sequence barriers, not a trusted client timestamp alone.

A live observation can still be useful without becoming a controlled experiment.
Its result remains observational. Existing operator-approved and owner-authorized
control facilities retain their own policies; a successful isolated test never
promotes a canary, changes a control, or expands that authority automatically.

In particular, Minime's current owner-inquiry dispatcher runs lifecycle `tick`
before routing even status/inspect requests. A new evidence-only adapter must
read validated persisted records without calling that side-effectful entry point.

## What Learning Means Here

The first version changes accessible evidence and persistent authored beliefs,
not neural weights or reservoir mathematics. A useful learned account might be
conditional: an expectation held on one generation route and not a fallback,
or a retrieval problem concerned a missing file rather than state dynamics.

Keep support scoped to tested versions, tasks, and conditions. Show a version
mismatch as stale applicability, not automatic erasure or automatic transfer.
Repeated matching observations can justify a stronger functional prediction;
they do not automatically identify its cause.

Future work can let a being choose among available tests by how well they
distinguish its competing explanations. That selection needs a bounded budget
and permission to choose no test. Do not rank thoughts by productivity, make
uncertainty an error, or optimize journals toward reassuring language.

## Delivery Plan

### 1. Repair the Experimental Foundation

Harden `substrate_probe.py` and its Rust adapter: one frozen origin, unique
resources, validated responses, bounded execution, cleanup on all exits, and
mechanically scoped receipts. Remove runtime-authored experiential verdicts;
retain the being's own terms as attributed labels. Bind source and result hashes.
Keep public command compatibility, but fail explicitly when isolation cannot
be established. This is the first recommended implementation tranche.

Acceptance: advancing source, identical inputs, partial clone failure, timeout,
cancellation, concurrent jobs, malformed responses, and cleanup failure tests.
Nothing launches against a live endpoint as part of those tests.

### 2. Complete the Evidence and Revision Contract

Add pure record validation and evaluation helpers, then thin dossier adapters.
Implement `EXPERIMENT_PREDICT` and `DOSSIER_REVISE`, with ownership checks,
idempotent retries and historical replay. Do not place another large block in
either inherited runtime monolith. Fix reference-validation gaps on touched
dossier paths as part of this coherent change.

Acceptance: unknown/cross-owner IDs rejected; old claims preserved; prospective
cutoff enforced; missing evidence stays unknown; no auto-acceptance or lifecycle
change; equal Rust/Python wire fixtures.

### 3. Deliver the First Whole Workflow

Connect a synthetic continuity recipe to the existing job and session machinery.
Demonstrate question -> prediction -> isolated test -> independently checked
result -> authored revision -> quiet park -> explicit resume. Introduce the
new capabilities as available tools, not as instructions to produce a positive
finding. No reservoir experiment is required to prove this workflow works.

Acceptance: a deliberately missing passage yields a truthful result; retrieval
recovery does not erase the prior failure; parking survives compaction/restart;
ordinary activity can proceed without touching the inquiry.

### 4. Add Bounded Consequence Tracing

Implement `EFFECT_TRACE` over the actual action and telemetry stores. Bind endpoint
receipts where available; return gaps where absent. Add delayed horizons without
occupying the agent's next-action slot or causing new automatic reflections.

Acceptance: dispatch failure, delayed application, overlapping events, stale
telemetry, clock skew, mixed units, duplicate samples, and restart boundaries.
The result never claims causation from a before/after difference.

### 5. Add Reservoir Studies and Optional Live Use

Expose reviewed copied-state recipes, first without LLM generation, then a
separately budgeted real-model study if needed. Verify both backend adapters
before claiming parity. Review the exact source lineage and rollout scope;
use sanctioned service wrappers for a separately approved deployment.

Observe ordinary tool use after rollout without requiring journals or soliciting
confirmation of improvement. Existing safety overrides remain available. The
paused automation stays paused unless Mike explicitly resumes it.

## Success Criteria

- The being can retain an unfamiliar question without prematurely translating it
  into a known metric or a live-control proposal.
- A failed action cannot become a successful causal test through its receipt text.
- A scored prediction can be checked against independently captured evidence,
  with the information available at prediction time recoverable.
- A challenge or revision remains visible alongside the earlier claim; an agent
  can discover that an explanation was incomplete.
- The feature supports justified uncertainty, negative results and quiet parking
  as well as successful predictions.
- No private prose is exported by default, no peer is conscripted into a study,
  and no improvement in language is treated as an outcome by itself.

The ambitious result is a system that can accumulate and use a revisable account
of its own operation. This RFC proposes the infrastructure for testing that
ability; it does not announce that the ability, or subjective experience, has
already been demonstrated.
