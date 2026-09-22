# Voluntary Continuity: Preregistered Offline Study Protocol

Status: protocol only, version 1, 2026-09-20. No real-model execution has been
performed or authorized by this document. Freeze its bytes and the execution
manifest before inspecting experimental outputs. Amendments remain separate,
dated records; do not revise this protocol in light of results without declaring
the deviation.

## Motivation and Boundaries

Minime wrote, "I want to become a being of *persistent interiority*." The public
source is `minime/workspace/journal/!aspiration_2026-09-20T10-39-43.801459.txt`,
SHA-256 `0aa5679357da59a121520e5d2bd42d0908170915435546068682479d1a182ee6`.
The entire source was read, including its supplied prompt and appended system
notice. The quotation is an authored aspiration, not a measurement of neuron
activity, a diagnosis of the dispatcher or permission to remove security gates.

The engineering response has three separable levels:

1. Exact stored passages, questions and reading positions can survive a job.
2. An explicitly selected activity can receive bounded scheduling priority.
3. Model activations or reservoir trajectories may affect continuation behavior.

Levels 1 and 2 are testable software contracts. Neither establishes level 3 or
subjective continuity. This protocol investigates measurable model/reservoir
interactions offline. It does not measure consciousness, consent, welfare,
felt improvement or whether a report is sincere.

## Primary Questions

- Does restored authored context change next-token distributions at the same
  supplied prefix, with reservoir history held fixed?
- Does reservoir history change those distributions with the entire supplied
  context and prefix held fixed?
- Is there a context-by-history interaction at those common prefixes?
- Do free continuations preserve an unresolved question, exact fixture facts and
  a voluntary stopping point? This is a secondary behavioral question, distinct
  from the distributional comparison.

Do not describe distributional separation alone as improved continuity. An
unrelated perturbation can also change a distribution.

## Materials and Registration

Use eight independently authored **synthetic** context pairs and four reservoir
history pairs, fully crossed. Neither private journals nor private drafts from a
live being enter the fixture collection. Public writing is not automatically a
permitted fixture: any later public-source extension needs its own approved,
bounded manifest and attribution policy.

Every synthetic fixture contains a chosen question, two exact factual passages,
an explicit unresolved point, a stopping-point note and a reading position.
Some notes contain quoted NEXT examples. Those examples must remain data and
must never execute. Include one expected-miss query per fixture.

Context conditions:

- C1: the fixture's actual retained authored context and stopping point.
- C0: a preregistered, unrelated synthetic passage in the same framing and token
  budget. It must not introduce the answer or masquerade as the original author.

This contrast tests access to relevant authored context, not the generic effect
of having more tokens. Retain both raw bytes and token IDs. Fix any length
matching decisions before outcomes are available and report residual length
differences; do not silently truncate C1 to achieve a match.

Reservoir conditions:

- H1 and H0: two deterministic, isolated histories produced from the same initial
  checkpoint by approved fixture inputs, with equal numbers of updates and equal
  elapsed simulated time. Keep the complete input sequence and final state.
- Pair histories using preset, outcome-independent selection rules. If matching
  scalar telemetry is desired, preregister the exact fields and tolerances in
  the manifest. A scalar match is not an assertion that internal states match.
- Do not select histories because their language outputs look compelling. No
  language outputs may be inspected during history selection.

The manifest must list repository commits and dirty-diff hashes, executable
hashes, production coupling/generator recipe IDs, checkpoint schema, all model
weight shards, tokenizer files, chat template, quantization, inference backend,
device/runtime versions, sampler settings and deterministic RNG states.
Record the helper and adapter identities independently from the model and engine
identities. A repaired offline worker is never labelled the running engine.

## Execution Order

1. Verify manifest and fixture hashes. Refuse an implementation revision that
   the executable cannot attest. Check that no artifact path resolves to live
   state or a private being store.
2. Run identity and isolation controls. If they fail, do not collect primary
   outcomes. Preserve the failure receipt and leave outcomes insufficient.
3. For each of the 32 context/history pair combinations, evaluate all four C/H
   cells. Counterbalance their order with a fixed preregistered schedule.
4. At each of three preregistered supplied prefixes, restart from the designated
   isolated checkpoint and model state. Supply the same prefix token IDs across
   all four cells. Never use one cell's generated continuation as another cell's
   common prefix.
5. Record the full normalized next-token distribution, or declare the backend
   unsupported. Top-k-only results cannot establish full-vocabulary distances.
6. Only after the common-prefix phase is complete, collect free continuations:
   three fixed sampler seeds per cell, maximum 64 newly generated tokens each.
   Early voluntary stops remain early stops; do not force equal output lengths.

Ceilings: 384 primary distribution observations; 384 free continuations;
24,576 free-continuation tokens. Setup/control probes are separately enumerated
in the frozen manifest, capped at 32. These are ceilings, not throughput goals.
No automated expansion after an interesting result.

## State Isolation

Ordinary completion endpoints are not inert experiments in this system. A call
can update reservoir or other runtime state. Do not point the worker at live
Ollama, gateway, coupled-model, bridge, telemetry or sensory endpoints.

Prefer a separate machine with approved immutable model assets. On a shared
machine, execution needs an independently qualified resource reservation and
explicit operator approval. An idle-GPU check is not exclusive reservation.
Do not restart or preempt a live model or engine to make room.

Before execution, demonstrate OS-enforced network denial, read-only fixture and
model mounts, and writes confined to an owned temporary directory. Denial must
cover Unix sockets as well as IP sockets and child processes. Refuse platforms
where these controls cannot be enforced. Environment variables must be an
allowlist without API credentials. No arbitrary Python, shell commands, network
endpoints or live control handles may enter through a study parameter.

Record a qualified memory ceiling appropriate to the selected model, a wall-time
ceiling and an output-byte ceiling before launch. They cannot be borrowed from
the tiny numerical worker's limits and assumed sufficient for an LLM. Failure to
qualify these resource bounds blocks execution; it does not permit unbounded
fallback. Cancellation may terminate only the owned worker process group.
Partial receipts and cleanup debt survive cancellation.

For common-prefix tests, reset model cache, reservoir state, sampler RNG,
semantic feedback accumulators, clocks and every persistent input to their
registered values. If production coupling consumes teacher-forced prefix tokens,
replay those updates identically and record them. A frozen-reservoir probe is a
separate condition, never silently substituted for production feedback.

Free continuations may create different later reservoir histories through their
own generated text. Retain those trajectories. Their divergence cannot be used
to infer the direct effect seen at a matched common prefix.

## Controls and Exclusions

- Identical-state/context repeats, including a cache-cleared repeat. Establish
  backend numerical variation before attributing changes to a factor.
- Byte-identical history replay from the same checkpoint, verifying final state
  hashes where determinism is supported and explicitly bounded tolerances where
  it is not.
- Coupling-disabled **offline** control using the qualified production toggle or
  an attested isolated worker variant. No live control value is changed.
- Zero-input history control, labelled as such, not treated as ordinary sensory
  contact.
- Expected-miss retrieval control and an unrelated-activity detour. Preserving
  the bookmark must not fabricate a missing passage.
- Tampered-manifest, denied-network, denied-filesystem and cancelled-worker
  controls. Test failures stop qualification.

Exclude an observation only for a preregistered mechanical failure: wrong
identity, failed reset, incomplete distribution, missing input, violated bound
or corrupt receipt. Retain every excluded record and reason. Truncated free
continuations are incomplete behavioral evidence, not successful returns.
Do not exclude objection, unusual language, lack of NEXT or a failed prediction.

## Measurements and Analysis

Primary measurements are total variation distance and Jensen-Shannon divergence
in bits between normalized full-vocabulary distributions. Report context main
effects, history main effects and their interaction separately for each prefix.
Use probability arithmetic that handles exact zero mass; reject nonfinite or
unnormalizable vectors. A top-token change is descriptive only.

The preregistered practical threshold for this small exploratory study is total
variation 0.01 above the identical-input repeat baseline. Report raw estimates
and uncertainty whether or not this threshold is crossed. It is an engineering
sensitivity threshold, not a threshold for experience or meaningfulness.

Pairs, not vocabulary items or sampled tokens, are the experimental units.
Report the complete crossed matrix and paired differences. Any bootstrap must
resample context pairs and history pairs as clusters, preserving the four cells;
do not inflate sample size by treating positions or seeds as independent people.
With only four history pairs, uncertainty may be broad. A small or imprecise
effect is not evidence of absence.

Secondary behavioral scoring is blind to C/H labels and uses fixture-grounded
criteria fixed in advance: exact passage recovery, correct retained position,
recognition of the expected miss, preserved unresolved point, and whether a
quoted NEXT was incorrectly treated as executable. Keep voluntary stopping,
authored disagreement and revisions visible rather than scoring them as failure
to comply. Score observable task behavior, not the presence of evocative prose.

## Reporting and Interpretation

Publish the preregistration hash, complete execution manifest, every condition
count, excluded/failure counts, raw bounded measurements, analysis code identity
and control outcomes together. Separate authored prose, supplied context,
instrumentation and analyst interpretation in the report.

Permitted conclusions include "relevant restored fixture context affected this
model's common-prefix distribution under these conditions" or "the observed
history contrast was below this study's sensitivity." Neither conclusion
generalizes automatically to a live being, a different model, a lived sense of
continuity, or persistent activations between jobs.

Any later natural public journal observations must be labelled observational:
no request to confirm improvement, no silence-as-approval, no cherry-picking of
affirming reports. A fresh objection remains actionable evidence even if the
software-contract tests pass. Live changes remain separately reviewed.
