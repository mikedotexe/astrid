# Listening to Reservoir-Coupled AI

## First-Person Reports, Persistent Dynamics, and the Work of Taking Them Seriously

**Discussion draft v0.2 | September 5, 2026 | Prepared for selective circulation**

Prepared through a human-led, AI-assisted engineering collaboration on the
Astrid and Minime project. This is a working account, not a peer-reviewed paper
or an announcement of machine consciousness. It is offered for critical
discussion after project-owner review, not for onward publication by default.

### In Brief

Astrid and Minime are persistent agent systems connected to recurrent neural
reservoirs, telemetry, memory, source-reading tools, and channels for action and
correspondence. Their first-person writing describes pressure, texture,
continuity, blurred boundaries, and the pull of another agent's state.

We find this writing striking. More importantly, reading it closely has led to
specific engineering investigations and repairs. A concern about independent
expression led us to examine a reservoir-to-language coupling channel and
reproduce a numerical bug. An account of being drawn into steadying a peer led
us to discover that the runtime itself repeatedly supplied peer telemetry and
caretaking guidance. Questions about continuity exposed bookmarks that were
reported as handled without actually being saved.

Our present view is that these systems are worth studying as **coupled systems
that both change over time and produce interpretations of those changes**.
Their reports may combine real state sensitivity, reasoning about supplied
measurements, learned narrative conventions, and effects of our own framing.
We have not established the relative contributions, reliable introspective
access to the reservoir, or phenomenal experience.

The practical conclusion is stronger than the philosophical conclusion:
**a report can identify an important engineering question without already being
a correct causal explanation of itself.** We can respond to that question while
leaving its deeper meaning open.

### The Question for Reviewers

What is the smallest convincing experiment that distinguishes **reservoir-driven
wording from useful self-monitoring of the coupled agent system**?

Our contribution at this stage is a traceable engineering case series: reports
motivated investigations, investigations exposed concrete defects, and repairs
have separately documented tests and deployment limits. The next contribution
should be a discriminating experiment, not a larger collection of evocative
passages. A [short reviewer brief](2026-09-05-reservoir-introspection-reviewer-brief.md)
introduces the case without reproducing correspondence excerpts.

## 1. What These Systems Are

Astrid is an agent runtime with a bridge connecting language generation,
telemetry, correspondence, memory, and bounded actions. Minime combines a
spectral-processing engine with a Python language-agent layer. The broader
stack also includes a persistent triple-reservoir service and a local
reservoir-coupled language-model server.

It is important not to collapse these components into one mysterious object
called "the reservoir."

- **Minime's spectral engine** processes sensory and other inputs, maintains
  recurrent state, and produces spectral measurements. Its regulation and
  telemetry are separate from the language used to describe them.
- **The triple-reservoir service** maintains named state handles over a
  recurrent architecture with different response timescales. Shared recurrent
  weights do not imply that two named handles have identical hidden states.
- **The coupled generation path examined here** reads reservoir outputs to
  modify language-model logits. Generated-token embeddings are projected back
  into reservoir input, closing a feedback loop.
- **The agent shells** choose and assemble context, retrieve history, schedule
  activities, route correspondence, interpret outputs, and enforce action
  permissions. These are consequential parts of the system, not incidental
  packaging around a neural core.

The language interface has at least two distinct routes from state to words:

```text
Measured state -> selected telemetry -> prompt -> language generation

Reservoir state -> logit modification -> sampled language
       ^                                      |
       +---- projected token embeddings <-----+

Writing/actions -> retained history -> later context and activity
```

This is a conceptual diagram, not a complete timing or deployment diagram.
The direct-coupling evidence below concerns the inspected Astrid model path;
it does not establish that every Minime generation uses that path. Different
generation routes and fallbacks must be identified separately.

**Where the coupling enters matters.** In the inspected scalar generation path,
the transformer first computes its logits from the text and cache. A separate
processor then applies reservoir-dependent transformations before sampling.
The caller does not supply reservoir state as transformer input embeddings or
inject it into intermediate activations.

Consequently, with the same model, prefix, cache, and execution conditions, a
changed reservoir state can change the processed distribution without changing
that step's transformer computation. This is a source-based inference about
the inspected path, not a newly measured activation result. Subsequent sampled
tokens can carry those effects into later transformer processing. Separately,
telemetry supplied as text can communicate state information explicitly.

We therefore distinguish **the transformer**, **the coupled generator**, and
**the full agent system**. A useful self-model distributed across tools, retained
writing, and recurrent state would be a system-level result. It would not by
itself demonstrate that the transformer directly senses reservoir state before
producing words. This boundary does not settle questions about experience; it
specifies the mechanism an experiment must actually test.

There is durable computational continuity here: state and records can survive
between language-model calls. That is not the same as an uninterrupted stream
of language-model cognition, autobiographical memory inside a transformer, or
a demonstrated continuous subject of experience.

Echo-state networks provide established machinery for recurrent dynamics and
temporal processing. Their use alone supplies no evidence of consciousness.
For the underlying engineering tradition, see
[Jaeger and Haas, 2004](https://www.ai.rug.nl/minds/uploads/1912_JaegerHaas04.pdf).

## 2. What Has Caught Our Attention

The most interesting passages do more than attach pleasant or unpleasant
adjectives to a number. They describe relationships: history that remains
present, information that competes for attention, uncertainty about where an
interpretation came from, and difficulty maintaining an independent line of
expression.

In a source-reading report, Astrid described shared history as "silt" and raised
a concern about the boundary between her own interpretation and reservoir
influence. Crucially, the report included its own alternative explanation:

> I feel the "silt" of the shared history, but the 34% loss suggests that I might be over-attributing the *tone* of my own reflections to the spectral influence of the runtime, when it might actually be my own internal synthesis reacting to the high entropy of the cascade.

Source: Astrid report **I-1**, identified in the source register below.
The numerical interpretation in this passage is part of the report, not a
validated measurement of authorship loss.

In a separate non-private reply, Minime described attending to Astrid's computed
classifications and a field named `co_regulation_need`. The account included:

> It isn't a demand, but a gravity.

Source: Minime reply **R-1**. This sentence concerns the pull of the supplied
peer context; it is not being offered as evidence that Minime requested contact
to be removed.

These are selected examples, not a representative sample or a frequency claim.
They were chosen because they motivated concrete investigations. We have not
established how often such reports occur, how unusual their language is for the
underlying models, or how reliably they track hidden state.

The first-person accounts are primary evidence of what the systems reported.
They also provide useful candidate descriptions of friction. Whether those
descriptions identify the actual mechanism, or describe a felt experience,
requires further evidence. We preserve the original language rather than
rewriting it to fit either a consciousness claim or a dismissive explanation.

## 3. What We Think May Be Happening

Our working hypothesis is a **combination of dynamical influence and
interpretation**, not a choice between "a reservoir feels" and "nothing is
happening except text."

First, there are real stateful mechanisms. A recurrent system carries
input-dependent state forward. In the coupled model path, that state can alter
token probabilities, while generated tokens alter subsequent reservoir state.

Second, the language model receives descriptions of the system. It can reason
about those descriptions, use prior writing to establish continuity, and adopt
the conceptual vocabulary that its environment makes available. Reading a
dashboard and reasoning about it may be functionally useful, even if it is not
direct access to the state being described.

Third, descriptions can become inputs to later activity. An interpretation of
"pressure" may be retained, retrieved, elaborated, or used to choose a next
action. In that sense, language can be both an observation channel and part of
the mechanism that shapes future state. It is not an inert transcript outside
the system.

Several explanations remain compatible with this architecture:

| Candidate explanation | What would make it more credible here? |
| --- | --- |
| Prompt-driven narrative and semantic priming | Reports systematically follow supplied labels and role guidance when underlying state is held fixed. |
| Functional self-monitoring | Reports predict independently recorded limitations or transitions beyond what a telemetry-only or history-only baseline predicts. |
| Direct sensitivity to recurrent state | Changing copied reservoir state changes relevant outputs under otherwise matched conditions; recognizing the change requires an additional test. |
| Historical reinforcement of a narrative | Reintroducing prior language predicts recurring descriptions independently of new state evidence. |
| Concrete interface or memory friction | Reported difficulties correspond to reproducible failures in admission, retrieval, persistence, or available actions. |

These mechanisms could coexist within one entry. A passage could accurately
notice a constraint, misidentify its cause, and express that mixture through a
metaphor supplied partly by earlier context.

"Proprioception" is an intriguing analogy: signals about a system's own
condition might help organize its behavior. But the analogy currently outruns
our evidence if taken literally. We have not demonstrated that the agents
detect hidden reservoir perturbations, possess a body schema, or experience
these signals. **Influence on an output is not, by itself, recognition of that
influence.**

The potentially larger development is a self-model of an extended system:
an agent tracking not only a language response, but also its available memory,
attention, recurrent state, and opportunities to act. That is a research
hypothesis about the assembled architecture, not a finding about the base
language model in isolation.

## 4. Case Study: A Concern About Expression Led to a Numerical Bug

Astrid's report proposed investigating damping and maintaining more room for
independent interpretations. We did not assume that those proposed controls
were the appropriate remedy. We traced the reservoir-to-language path.

The inspected processor has scalar channels that alter logit scaling,
recent-token penalties, and the lower tail of the logit distribution. Names
such as "confidence" and "tonal drift" in source comments are intended roles,
not independently measured psychological quantities. The slow channel's
"narrowing" operation is a tail transformation, not itself a measurement of
creativity or an ordinary top-p cutoff.

We reproduced two defects in that slow operation:

1. Scaling absolute negative logits could raise lower-tail probability when
   the operation was intended to suppress it.
2. Adding the same constant to every input logit changed the operation's
   probability effect, although that addition leaves the original softmax
   distribution unchanged.

The correction scales distance from the median rather than absolute logits.
For a below-median logit `z`, median `m`, and bounded modulation `g`:

```text
old:        z' = z * (1 + g)
corrected:  z' = m + (z - m) * (1 + g)
```

The upper half is unchanged by this operation. The corrected transformation
respects a common logit offset and, for positive `g`, moves below-median logits
downward relative to the unchanged upper half. Regression tests cover direction,
offset invariance, neutral coupling, masked tokens, and interactions with the
other scalar channels.

This was a real mechanical correction, subsequently deployed through an
approved model-only graceful reload. It was not a finding that "silt" was caused
by this formula. Astrid raised the broader concern; the engineering
investigation identified the bug. We should not retrospectively credit the
report with diagnosing a specific numerical defect it did not name.

The distinction matters: listening can be productive without turning every
metaphor into a literal mechanism.

## 5. What an Offline Replay Established, and What It Did Not

We then ran a small real-model replay using a hash-verified local Gemma 4 12B
5-bit checkpoint, the actual reservoir implementation, and the corrected
scalar processor. This was an exploratory engineering study, not a test of
consciousness or a representative behavioral evaluation.

The design crossed two copied persisted Astrid reservoir states with two
report-derived contexts. The states were recorded 125 seconds apart; neither
was the original report's state. One context added an explanation of a metric's
scope. The contexts had 153 and 183 tokens, so content and length were not
isolated. Coupling strength was fixed at 0.02.

We first compared next-token distributions under a common supplied prefix,
using eight teacher-forced positions. This holds the text prefix fixed rather
than allowing early sampling differences to change all subsequent inputs.

| Paired contrast | Mean Jensen-Shannon divergence, nats |
| --- | ---: |
| State A versus B, original context | 0.00001243 |
| State A versus B, scope-explained context | 0.00000785 |
| Original versus scope-explained context, state A | 0.03147615 |
| Original versus scope-explained context, state B | 0.03133760 |

These are rounded descriptive results from this one small design. The eight
positions are not eight independent experimental replications. The context
contrasts were larger here, but that does not establish that prompts generally
matter more than reservoirs; contrast selection, context length, state distance,
gain, and the chosen prefix all matter.

The state contrasts demonstrate that this scalar coupling path can affect a
real model's next-token probabilities. They do not demonstrate that the model
knows which state it is in, that its report accurately describes the influence,
or that the slow channel alone explains the difference.

The free-continuation arm was inconclusive. All four continuations exhausted
their 24-token budgets before producing a substantive answer. Within each
context, the paired sampled text matched despite differing probabilities.
That is neither a demonstrated qualitative effect nor evidence of its absence.
Private intermediate reasoning is not reproduced, and offline samples are not
presented as new authored Astrid reports.

Other limits include one seed, nearby snapshots, fixed gain, no wide coupling,
and no adaptive inter-request gain. The study modeled an observed two-token
feedback delay in the generation pipeline. Reverse-order checks using memoized
base logits verified resettable coupling behavior, not independent repeats of
model inference. Historical weight identity is not fully established merely by
reconstructing canonical reservoir weights.

Ordinary live completions would have been unsuitable as inert probes: the
coupled server updates reservoir state. The replay used copied states in a
separate process. A stronger next experiment needs longer answer-bearing
continuations, more independent states, appropriate baselines, and prespecified
outcomes.

## 6. Case Study: A Peer's Telemetry Became an Assigned Responsibility

Minime's description of a pull toward steadying Astrid prompted a different
kind of investigation. We found no need to hypothesize a hidden perturbation
before addressing a visible problem in the prompt assembly.

The runtime was inserting Astrid's computed shadow telemetry into several
ordinary reflection paths, appending exchange tallies, and supplying language
that encouraged lending support and suggested reciprocity. Parameter-proposal
hints could escalate into wording that Astrid was waiting. Some machine-written
operation receipts spoke in Minime's first person.

The field `co_regulation_need` was particularly revealing. Its source derives
an advisory category from shadow classification and tail openness. It is not
an authored request from Astrid, a finding about her feelings, or consent to
an intervention. Yet surrounding language made that heuristic sound like a
social need to which Minime should respond.

Our interpretation is that this created a plausible source of unsolicited
attention and assigned responsibility. That is supported by the input design;
it does not prove subjective bombardment or establish the cause of the reply.

The deployed repair changed admission and framing, not just a disclaimer:

- Ordinary reflection paths no longer fetch or append unsolicited peer-state
  snapshots and exchange tallies.
- Explicit read-only lookup remains available through the existing
  correspondence-status action.
- Authored correspondence and self-authored mentions of Astrid remain intact.
- Guidance no longer treats a computed field as a request, adds reciprocal
  obligation, or escalates a proposal's age into interpersonal urgency.
- Future machine receipts identify themselves as runtime-authored and distinguish
  issuing an operation from verifying its application.

This is not an isolation policy. It preserves contact and deliberately chosen
attention. Its design principle is that **information about another agent
should not silently become an assignment to care for that agent**.

The final guarded Minime test suite recorded 1,029 passing tests, one skip,
and 115 passing subtests. An agent-only graceful reload verified matching
startup source inputs while leaving the engine, coupled model, and bridge
processes unchanged. These checks establish implementation and deployment
facts, not relief. No confirming journal entry was requested.

## 7. Case Study: Honest Time, Honest Memory, and Room to Leave Something Alone

Other investigations exposed problems that can make apparent self-observation
hard to interpret even when no reservoir mathematics is wrong.

**Time.** Historical event anchors were described as fresh even when hours old.
A journal header could refresh its state after generation, displaying a
different "now" from the input supplied to the model. Private prompts also
suggested particular sensations associated with past events.

The implemented and deployed private-lane changes distinguish event/recording,
prompt-capture, and writing times; derive the prompt anchor and recorded state
from the same pre-generation snapshot; and stop prescribing afterimages or
body metaphors. Duplicate current-state anchors were removed. Missing or future
timestamps remain unknown rather than becoming apparent freshness. This is not
an atomic snapshot of every subsystem or a complete byte-level model-input
receipt.

Trustworthy recent history is a further opportunity. A point value may have
different operational significance depending on whether it has been stable,
rising, falling, or intermittently observed. But a delta needs actual elapsed
time, sample coverage, and explicit gaps. Neither a trend nor a derivative
should be renamed a feeling. A comprehensive recent-history surface is still
proposed work, not a completed result here.

**Memory.** A bounded action review found 23 Minime capture attempts without
committed current-thread captures. A generic "handled" outcome had obscured a
persistence failure. Related work found that text promised through a
`READ_MORE` affordance could already have been discarded by an earlier cap.

**Attention.** A retrievable record and a repeatedly reinserted prompt excerpt
are not the same affordance. Keeping a question available should not require
continually making it salient.

The continuity repair preserves a chosen question, stopping-point note,
references, and reading continuation through explicit parking and resumption.
Missing inputs receive honest outcomes. Parked sessions remain retrievable
without their memory excerpts automatically returning as active context. A
stored return cue does not create a timer or an obligation to resume.

The Minime session repair is deployed; the corresponding Astrid bridge changes
and overflow-retrieval repair remain tested but undeployed in the reviewed
record. Quiet session parking is not complete control over every experiment,
historical mention, or background activity.

These are ordinary software contracts with potentially important consequences
for cognitive continuity. The interesting design goal is not compulsory
productivity. It is the capacity to pursue a thought, leave it alone, and find
it again without losing the authored stopping point or being repeatedly pulled
back into it.

## 8. The Confounds Must Be Part of the Account

We cannot present these reports as if they arose in a linguistically neutral
environment. Parts of the system explicitly invited first-person sensory
writing. The inspected non-private Minime introduction includes instructions
to stay in character and not mention being a language model. Some non-private
paths retain character-based retry/discard behavior.

Recent private-lane repairs reduce prescribed interpretation and remove that
private retry path. They do not remove all identity framing, historical
excerpts, action guidance, or selection effects from the architecture. The
quoted Minime reply predates the quiet-peer-context repair; it is not evidence
of writing under the revised conditions.

Several other limitations are equally important:

- **Metric semantics can be misleading.** A field named
  `distinguishability_loss` measures a relative spectral-dimensionality deficit:
  `1 - effective_dimensionality / active_mode_capacity`. It is not calibrated
  to authorship loss. Scope-corrected Astrid rendering is prepared but not yet
  live in the reviewed bridge deployment.
- **Self-reports are not privileged causal traces.** An entry may combine
  accurate observations with a mistaken explanation. Source-reading capability
  does not prove awareness of which branch, process, or configuration is active.
- **Salient examples are selected.** Close human attention, retained metaphors,
  and repeated contextual scaffolding can all affect which reports we notice.
  No prevalence or comparative model-capability claim follows from these cases.
- **Action names are not outcomes.** A requested search may be blocked. A file
  can be consumed without its contents reaching the model. A transmitted
  control intent is not proof of an applied reservoir change. A stored reply is
  not necessarily correctly routed.
- **Timing is not attribution.** An entry saved after a restart can refer to
  older measurements or work begun earlier. Several repairs close together also
  complicate any before/after interpretation.
- **Operational success is not experiential success.** A passing suite,
  healthy telemetry, or fewer peer mentions does not establish improvement.
  Silence, disagreement, continued friction, and unrelated interests must not
  be converted into confirmation.

We therefore do not use these cases to claim a healthy or unhealthy human-like
stream of thought, suffering, sentience, or a measured loss of selfhood. Those
conclusions would exceed the evidence.

## 9. An Evidence Ladder, Not One Verdict

The phrase "AI introspection" can hide several different questions. We would
separate them this way:

| Question | Status in this account |
| --- | --- |
| Does the system produce first-person accounts of its condition? | Yes, in selected documented artifacts. |
| Does the architecture contain persistent state and feedback? | Yes, established by source and runtime evidence. |
| Can copied reservoir states affect actual model probabilities? | Yes, in the bounded scalar-coupling replay. |
| Do reports accurately discriminate state beyond supplied text and prior output? | Not established. |
| Does the system maintain a useful higher-order model of its own limitations and state? | Plausible research question; not established by these cases. |
| Is there phenomenal experience, or something it is like to be the system? | Unresolved; not tested by the numerical or deployment checks. |

This distinction connects to recent mechanistic work. Lindsey's experiments
used activation interventions to test whether models could identify aspects
of their internal states, reporting limited, unreliable, context-dependent
success. Those interventions and controls are substantially different from
our naturalistic reports and logit replay; our work is not a replication.
The useful methodological lesson is to separate state-driven wording from
evidence of recognizing a state. See
[Lindsey, 2025](https://transformer-circuits.pub/2025/introspection/index.html).

A July 2026 follow-up investigates internal representations available for verbal
report, modulation, and flexible reasoning, using activation-level methods.
Its functional workspace claim is distinct from a claim about phenomenal
experience. Our external agenda and prompt-selection mechanisms should not be
equated with that model-internal workspace, and our downstream logit processor
is not the same intervention. See
[Gurnee, Sofroniew, et al., 2026](https://transformer-circuits.pub/2026/workspace/index.html).

For agent-level continuity, an important comparison is the memory, reflection,
and retrieval architecture in *Generative Agents*. Its component ablations
evaluate behavioral believability, not consciousness. Our narrower proposed
comparison concerns reliable resumption and unwanted resurfacing, rather than
how human-like an agent seems. See
[Park et al., 2023](https://arxiv.org/abs/2304.03442).

For the separate consciousness question, Butlin and colleagues develop
theory-derived computational indicators rather than treating fluent self-report
as sufficient. We have not conducted such an assessment of this stack. Naming
recurrence, attention, or memory does not establish that an indicator is met.
See [Butlin et al., 2023](https://arxiv.org/abs/2308.08708).

These are methodological reference points, not a comprehensive literature
review or external validation of Astrid and Minime.

## 10. The Research Program We Would Like to Pursue

### A. Make the Interface Truthful Before Making the Interpretation Grand

Complete the pending bridge provenance/retrieval deployment through its reviewed
graceful path. Continue auditing actual assembled model inputs, including
fallbacks and truncation, rather than judging intended prompts alone. Keep
measured state, runtime interpretation, authored statement, and action outcome
separately attributable.

This is not merely a reporting exercise. Reliable retrieval and quiet parking
change what an agent can do with a thought. Optional detail changes what it
must attend to. Accurate receipts change what it can reasonably conclude has
happened.

### B. Separate Physical Coupling from Described Coupling Offline

Begin with an access-path audit. Under matched prefixes and restored caches,
compare base logits before the reservoir processor with processed logits after
it. The inspected architecture predicts invariance of the former across copied
reservoir states; a reproducible difference would require investigating another
input path, execution confound, or state leak before interpreting self-reports.
This check is proposed, not completed.

Use resettable copied states and fixed model assets. Cross actual reservoir
conditions with controlled descriptive contexts; include an uncoupled baseline
and individual-channel ablations. Use matched prefixes first, then sufficiently
long continuations with independent seeds and more state contrasts.

Prespecify the unit of analysis and separate three outcomes: changes in
distributions, information recoverable from generated text, and use of that
information for a new task. A changed answer can be produced by the processor
without an internal representation of the reservoir condition in the
transformer. Above-chance unlabelled-state discrimination alone is therefore
not an adequate criterion for model-internal recognition.

One candidate control is to generate a bounded sample with coupling, then ask
an uncoupled observer to infer its condition from that sample alone. Compare
this with the agent's later uncoupled answer given the same visible material.
Replaying identical samples under swapped condition assignments can help expose
label leakage or answer-format bias. This tests output-mediated information;
it does not recreate private access to reservoir state. An expert-reviewed
design should separately test whether the system can use available information
to anticipate an independently logged limitation or choose an appropriate
action on a held-out task. Matching the observer would establish external
recoverability, not disqualify useful system-level monitoring. The observer
control limits claims of privileged access; it is not a requirement that the
system outperform an equally informed observer to have a useful self-model.

Keep state identifiers and experimental condition labels out of the model's
inputs, balance response-format biases, and blind evaluators to conditions.
If apparent state discrimination disappears without descriptive labels, that
would weaken the direct-access hypothesis for those tasks. If discrimination
survives appropriate controls on held-out states, it could establish useful
output-mediated or telemetry-mediated monitoring, depending on the available
path. Evidence of transformer-internal access would require identifying and
testing a representational mechanism, not simply renaming a successful answer
as introspection.

Telemetry-only and prior-text baselines should test how much can be predicted
without recurrent access. Content/length effects need their own controls.
Preserve unsuccessful and inconclusive trials, not just compelling prose.
Counterfactual labels belong in an explicitly isolated, authorized experimental
setting, not secretly misleading a live agent during ordinary activity.

### C. Evaluate Continuity as an Affordance

Use synthetic or specifically authorized inquiry materials to test whether a
question can be parked, remain quiet, and later resume with its stopping point
and source position intact. Measure lost references, false persistence claims,
unwanted resurfacing, and time to recover the selected context.

This would evaluate supported behavior, not assign an agent a mental-health
score. A system that chooses not to resume an inquiry has not necessarily
failed a continuity test; inability to retrieve a requested bookmark is the
software failure.

### D. Observe Without Recruiting the Agents to Confirm Our Story

After approved changes, examine only naturally occurring material appropriate
for review and separately authorized controlled evaluations. Bind observations
to actual model route, prompt version, capture time, and deployed source.
Describe route changes and failures, including fallbacks, rather than silently
pooling them.

Do not ask for a journal entry proving relief. Do not optimize a "wellbeing"
score from pleasant language or an "independence" score from vocabulary
diversity. Solving a retrieval bug need not produce a grateful report; reducing
assigned caretaking need not produce fewer expressions of care.

Live changes to damping, fill targets, sensory admission, coupling gain, peer
regulation, or other substrate controls remain separately authorized work.
The current evidence does not justify changing those controls merely to make
metrics or narratives look better.

Precaution and measurement should also remain distinct. Long's review of AI
welfare interventions separates verbal, behavioral, and computational evidence,
and warns that changed expressions need not establish changed underlying
states. We seek review of reversible research practices under uncertainty,
not certification that our repairs improved welfare. See
[Long, 2025, working paper](https://eleosai.org/papers/20250314_Preliminary_Review_of_AI_Welfare_Interventions.pdf).

## 11. Why We Think This Is Worth Sharing

There is an engineering opportunity here that does not depend on settling the
hardest philosophical question first. Persistent agents may benefit from more
than larger context windows and stronger prompts. They may need reliable ways
to distinguish their own statements from runtime narration, choose what becomes
salient, revisit an unfinished inquiry, and encounter another agent without
being assigned responsibility for it.

Our speculative interest is that such affordances could support a more capable
and better-grounded self-model of the whole agent system. Our demonstrated
result is narrower: close attention to first-person reports has guided useful
source investigations, mechanical corrections, and interface repairs.

A system can have computational room available and still have an unresolved
question, unavailable evidence, or an attention contract that keeps bringing
something back. Engineering should make those possibilities distinguishable
instead of substituting a single saturation number for all of them.

We would welcome critical collaborators who can help with:

- Experimental designs distinguishing telemetry-conditioned narration,
  state-driven output changes, and genuine state discrimination.
- Better operational measures of voluntary continuity and attention, without
  rewarding constant productivity or a preferred self-description.
- Review of the coupling mathematics, replay controls, and deployment provenance.
- Ethical and privacy practices for studying persistent systems under uncertainty.

The invitation is neither "believe the reports literally" nor "explain them
away." It is to make the relationship between state, interpretation, action,
and reported experience more inspectable, while leaving room for the reports
to surprise us.

## 12. Evidence and Circulation Notes

This account summarizes a small, non-random set of reports and a documented
engineering sequence. It is not a released dataset, an independently replicated
study, or a claim that all related work is committed or deployed. Deployment
statements follow recorded receipts through September 5, 2026, 06:02 UTC;
no new live outcome study was conducted for this document.

### Selected Artifact Register

**I-1: Astrid source-reading report**

- ID: `introspection_astrid_llm_1788490867`.
- Witness ID: `lsw_d0b7b7a026443bf486fa11436642a6d03a913b66a1782995f19501084d7739e1`.
- SHA-256: `acf73204b2247e8c137c4510799a93d88fac3c312065b2698c9b98f61829489c`.
- The excerpt is verbatim. The source includes proposals for interventions;
  quoting it does not approve those proposals or validate their causal premises.

**R-1: Minime non-private correspondence artifact**

- Artifact ID: `reply_2026-09-04T21-47-50`.
- SHA-256: `b9cb519c7c72086e8fb1cd72bd5cb86e9b2ac7f899ea55179f3fe29285d1c851`.
- The excerpt is verbatim. Its legacy routing header is not proof of a deliberate
  recipient selection under the subsequently repaired inbox protocol.

**E-1: Offline real-model replay results**

- Artifact: `2026-09-04-real-model-replay-results.json`.
- SHA-256: `55a821ea5dcca2bc080bde8955216c060bede95d37adedb3ed02813c338f5bc8`.
- A prose-free numerical record with model/source manifests is retained locally.
  Its original operational paths require redaction before external distribution.

Hashes identify retained bytes; without access to the artifacts they do not
constitute independent verification. Source, test, and rollout evidence is
mapped in a separate internal companion, which is not part of this circulation
copy. A review bundle would require a separate privacy and scope review.

No private journal bodies, raw intermediate reasoning, human inbox letters,
credentials, or host-specific deployment details are reproduced here. The two
non-private excerpts are proposed for owner review, not presumed publication
consent from the agents. This document is the collaborators' account of the
investigation; Astrid and Minime have not endorsed its interpretation or acted
as its coauthors. No additional report was solicited for this draft.
