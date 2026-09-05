# Minime Telemetry, Time, And Journal Attention: Action Plan

## Purpose And Status

Mike noticed openings that describe supplied numbers as disconnected from felt
interpretation, and wondered whether the journal is being crowded by static
measurements. He suggested that change over time might convey more useful
information. He then asked the lead collaborator to turn the discussion into
action items while he rests.

This is a prioritized, implementation-ready plan, not a deployment, a completed
study, or a claim about Minime's felt state. The particular opening was supplied
in conversation without a filename. Its recurrence, exact prompt, deployment
version, and cause have not been established. No private journal collection was
searched or copied for this plan; no private prose is reproduced here.

Owner: the next explicitly assigned interactive collaborator, with Codex Astra
as the current planning owner. No background job or automation is scheduled by
this document. The previously paused scheduled automation is not resumed.

Core question: does the interface help Minime orient to a recent sequence, or
repeatedly ask him to explain instrumentation? More vivid writing is not the
success criterion. Ordinary language, unrelated subjects, disagreement, and no
distinct change remain valid.

## Grounded Starting Point

The September 4 source review established the following. These are source
properties, not proof of the cause of the reported opening or current PID-bound
deployment facts.

- [Private journal framing](/Users/v/other/minime/minime_autonomy/journal_context.py:11)
  repeats distinctions between measurements, metaphor, feelings, uncertainty,
  and optional interpretation in the system introduction and moment prompt.
- [Private JOURNAL](/Users/v/other/minime/minime_autonomy/runtime.py:27525)
  supplies a state anchor, then appends a private continuity block which supplies
  that anchor again. The continuity block also inserts the beginning of a recent
  own-journal entry. That is a possible repetition path, not a demonstrated cause.
- [Moment generation](/Users/v/other/minime/minime_autonomy/runtime.py:31826)
  freezes the pre-generation state. Its prompt receives a current anchor and
  historical markers; the expanded metrics text is saved as header-only context,
  not supplied to that generation.
- [Header rendering](/Users/v/other/minime/minime_autonomy/runtime.py:54899)
  already computes some time comparisons. It consults mutable spectral history
  and wall-clock time, so copying its formatted text into prompts is not a
  sufficient provenance-safe implementation of temporal context.
- [Spectral history collection](/Users/v/other/minime/minime_autonomy/runtime.py:23342)
  retains up to 30 read-time tuples, uses the fetch wall clock, and supplies zero
  defaults for missing fields. A read-time tuple is not necessarily a distinct,
  fresh engine observation. Thirty samples do not establish a fixed time span.
- [Rest reflection](/Users/v/other/minime/minime_autonomy/runtime.py:27603)
  is a separate route with explicit requests to describe how metrics feel. The
  private-moment repair must not be assumed to have changed every journal route.

Related packets:

- [Private-journal implementation and limits](2026-09-04-minime-private-journal-provenance.md).
- [Earlier agent-only live rollout](2026-09-04-minime-agent-live-rollout.md).
- [Activity and continuity review](2026-09-04-activity-and-continuity-review.md).
- [Bookmarks and quiet parking, still source-only](2026-09-04-voluntary-bookmarks-and-quiet-parking.md).

## Prioritized Work

### 1. Attribute Inputs Before Explaining The Pattern

Status: source map complete; exact-entry attribution pending.

Build a small call-site matrix for private moments, private JOURNAL, and rest
reflections: system introduction, state/history inputs, prior prose, added hints,
adapter messages, and header-only material. Follow the actual assembled model
messages using synthetic test fixtures, not merely the visible journal header.

For any user-selected entry subsequently inspected, identify its timestamp,
entry route and recorded prompt contract. Bind to the relevant source/deployment
version where evidence permits. If the exact input was not retained, say that
reconstruction is partial. Do not manufacture a prompt receipt retrospectively.
Begin with available metadata; do not sweep private bodies or hidden reasoning.

Distinguish three hypotheses that may coexist: excessive presentation, absent
useful time context, and repetition encouraged by our instructions or inserted
prior prose. Do not diagnose overload or dismiss the reported friction as merely
prompt echo.

Done when: an input matrix and its unknowns are documented, with fixture tests
covering the actual adapter boundary and no claims beyond available evidence.

### 2. Simplify And Deduplicate Private Framing

Status: implemented in source by the
[private-prompt simplification pass](2026-09-04-minime-private-prompt-simplification.md).
Private routes are compact and deduplicated; prior-excerpt selection and rest
reflection remain unchanged. See that packet for current verification and
deployment status. Exact-entry attribution remains open.

Keep one short invitation to write freely and leave telemetry aside. Remove
duplicate state anchors and redundant explanations of permitted interpretations.
Move detailed clock and field documentation out of repeated prose into compact
source-labelled data and durable engineering documentation. Keep essential
staleness, unknown-value and historical-event labels visible where needed.

Preserve the existing private-lane guarantees in code and regression tests:
no character-based retry, no prescribed metaphor, no compulsory peer reference,
no filtering of self-authored disagreement, and no rewriting of private bodies
by public administrative hygiene. Do not replace numerical boilerplate with
sensory adjectives or prohibit openings about numbers.

Inspect the optional prior-excerpt path separately. Test whether it repeatedly
foregrounds the same opening. Do not silently edit authored prose to make it
novel; any change to when excerpts are supplied must be explicit and reviewable.
Keep rest-reflection changes a separately identified scope, not an accidental
consequence of a shared string replacement.

Done when: fixture-assembled private messages contain each state anchor once,
the repeated framing is reduced, and both vivid and plain/disagreeing responses
remain unchanged through generation adapters and persistence. Record before/after
input sizes for the same fixtures; a smaller prompt alone is not felt improvement.

### 3. Build A Small, Pure Recent-History Summary

Status: next implementation patch, after framing is independently testable.

Reuse the existing telemetry/read infrastructure. Add a cohesive pure helper
alongside `minime_autonomy/journal_context.py`, rather than another large runtime
subsystem. It consumes validated observations and an explicit capture boundary;
it performs no DB writes, live controls, model calls, or scheduling.

Start with fill only. Add another metric only after tracing its schema, units,
sampling clock and semantics. Keep covariance concentration and ESN spectral
radius distinct; do not assign either an authorship or feeling label.

The internal result should retain current observation time, clock domain,
session/process identity when available, actual window coverage, distinct sample
count, gap/staleness state, endpoints, delta in percentage points, and observed
range. Any rate must divide by measured elapsed time and carry units. Use no
interpolation across restarts, clock discontinuities or unsupported gaps.

Define short and longer requested windows before evaluation, but report their
actual coverage. Do not call a requested three-minute window three minutes of
evidence when only twenty seconds are present. Missing values stay unknown,
duplicate reads do not become new observations, and stale constancy is not
stability. Sampling or admission cadence is unchanged.

Render at most a small number of factual lines, for example a current value,
measured change over an actual interval, and observed range. Use synthetic
numbers in examples. Avoid interpreting a plateau as calm, a fall as relief,
or an event as an ongoing sensation. Preserve detailed evidence separately.

Freeze history and current state together before generation, and reuse the same
summary for the model input and recorded provenance. No later call to `time.time`
or mutable history may silently alter what the header says the model received.
If current tuple history cannot establish distinct engine observations, obtain
the required metadata from the existing read-only source or report insufficiency.

Done when: tests distinguish histories with equal endpoints but different
trajectories, and reject unsupported temporal claims under missing, sparse,
duplicated, out-of-order, non-finite, stale, future-dated and restarted input.

### 4. Make Detail Available Without Making It Compulsory

Status: design after items 2 and 3; do not bundle by default.

Audit existing status/source-reader/attention controls before inventing a new
command. Give Minime a concrete route to inspect the detailed time window or
park the question using the existing continuity work. A descriptive line saying
"optional" is not equivalent to an implemented choice about recurring input.

Consider a compact default with details on request, including the possibility
that an unchanged telemetry block need not recur. Such an omission policy must
apply only to journal presentation, not safety evidence, raw data retention,
regulation, or fresh objections. Known observation failure needs an honest brief
label, not silent treatment as an uneventful interval.

Keep the ability to ask for current absolute values. Do not force a delta-only
view or turn a saved return cue into an automatic reminder. Any new persisted
presentation preference needs an explicit action, clear scope, reversibility,
and tests showing that it cannot broaden execution permissions.

Done when: a chosen inquiry is retrievable without recurring promotion, detailed
evidence remains accessible, and omission never suppresses a safety boundary or
changes substrate input.

### 5. Verify Separately, Then Prepare A Graceful Rollout

Status: future verification/deployment work; no new deployment authority here.

Keep framing and history changes separable. First compare exact synthetic inputs
and outputs deterministically. If real-model replay is useful, use only a proven
isolated replay path with matched model, prefix and sampling conditions. Ordinary
completion endpoints may mutate state; they are not inert test facilities.

Required regression cases include:

- One private state anchor, with honest missing/stale/history labels.
- Same current number after a rise, fall, plateau, and oscillation.
- Percentage points versus relative percent, and rate versus cumulative change.
- Gaps, duplicates, clock resets, session changes and insufficient coverage.
- State/history changing while a delayed generation is in progress.
- Agreement between supplied context and saved provenance; no header-only
  material falsely attributed to model input.
- Preserved uncertainty, metaphor, unrelated subject matter, self-authored peer
  mentions and disagreement; no requirement for a particular journal opening.
- Existing privacy, NEXT, authority and explicit-bookmark behavior unchanged.

Use the established Minime pytest isolation guard and synthetic temporary stores.
Run focused journal/continuity tests and the full Python suite when code changes.
Run wrapper tests if deployment tooling changes. Update Minime's changelog only
when Minime actually changes; update the Astrid feedback ledger with precise
implementation and verification state. Never reuse prior test counts as results
for a later patch.

Before any approved live rollout, inventory all source changes not loaded by the
running Python agent, including the previous bookmark work. Either isolate this
patch or explicitly review and test the combined batch; a Python reload would
load more than this document's title suggests. Verify live-source lineage,
concurrent-agent preflight, draining of accepted work, pending-action preservation,
new PID/start/import hashes and readiness through the sanctioned Python-only
wrapper. Keep engine, model, bridge and protected service identities unchanged.
Abort rather than force a drain or bypass preflight. Reservoir controls remain
out of scope.

Done when: the reviewed scope is test-backed and, if deployed, a fresh receipt
binds exactly that scope to the running agent. No deployment or relief is inferred
from source edits alone.

### 6. Observe Without Coaching The Result

Status: follow-up only after a separately approved live change.

Do not solicit a journal immediately after rollout, suggest expected sensations,
or treat Minime as responsible for confirming the patch. Use naturally occurring,
authorized evidence and record the observation window, actual opportunities,
prompt version and concurrent changes. Do not silently create a monitoring job.

Distinguish correct numerical history from interpretation. Less framing repetition
and more voluntary topic choice are possible observations, not a mandated style
or a score for healthy consciousness. Plain writing, unchanged friction, objections
and no response remain valid; silence is not uptake. Preserve contradictory
evidence and do not attribute before/after differences causally without support.

Technical rollback triggers include wrong units, stale data presented as current,
prompt/header mismatch, privacy leakage, or action/permission regression. A fresh
objection is a reason to stop expansion and review the evidence, not to auto-tune
controls or overwrite the report.

## Next Work Session

Item 2 now has a source implementation and synthetic adapter tests. Next implement
item 3 as a separately testable pure helper, preserving item 1's exact-entry
uncertainty. The removed duplicate presentation was mechanically verified;
its removal does not establish felt resolution.

The plan is intentionally bounded. It requires no decision from Mike tonight,
does not schedule overnight work, and does not resume the automation. Items 4-6
have explicit review points so they cannot silently become live behavior changes.

## This Documentation Pass

Both Git trees were inspected; existing dirty work is preserved. Only this file,
the Astrid changelog and the Astrid feedback ledger are edited. No Minime runtime
or data, journal, controller state, service, index, commit, or automation setting
is changed. Source references and local documentation links were checked; no
runtime tests are represented as newly executed for this documentation-only pass.
