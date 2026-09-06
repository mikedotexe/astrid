# What Is Being Introspected?

## LLM Activations, Reservoir Dynamics, and the Extended Agent

**Research comparison memo | September 5, 2026 | Selective-circulation draft**

Companion to [Listening to Reservoir-Coupled AI](2026-09-05-reservoir-introspection-discussion-draft.md),
revision v0.3. Prepared through human-led, AI-assisted source review. This is
an analytical comparison, not a systematic review, replication, or new result.
No experiment or live intervention was performed for this memo.

## 1. The Main Distinction

Mike's proposed distinction is substantially right as a statement of our
research target: the local accounts concern a persistent, reservoir-coupled
agent system, not exclusively a language model's own intermediate activations.
But the subject of a report and the source of its information are different.
A report about a reservoir might be based on supplied measurements, source
code, previous writing, effects on sampled language, or some combination.

Our positive research question is:

> Can a persistent agent use information about its reservoir-coupled condition
> to anticipate consequences, choose activity, and revise its interpretation
> across time?

That question leaves room for useful system-level self-monitoring without
requiring that every relevant fact be privately accessible inside a transformer.
It also gives us independently checkable outcomes beyond striking prose.

## 2. The Anthropic Study

The linked [October 2025 overview](https://www.anthropic.com/research/introspection)
introduces [Jack Lindsey's technical paper](https://transformer-circuits.pub/2025/introspection/index.html).
The paper was also revised in January 2026; this review uses the current text.

Its four criteria are accurate reporting, causal grounding, an internal route
not mediated by sampled output, and a metacognitive representation. Experiments
test injected-concept detection, separation of injected concepts from input
text, attribution of artificial response prefills, and instructed modulation
of representations. Interventions target the residual stream. Successful
detection must precede revealing the concept in output. Selected layer/strength
settings yield roughly 20% success for the strongest tested models, not general
20% accuracy on introspection. Controls include uninjected trials, unrelated
questions, and mismatched or differently timed injections. Higher-order
representations are inferred, not directly identified in this paper. Prompt
sensitivity, imperfect concept vectors, artificial interventions, and unverified
experiential elaborations constrain the interpretation. See the paper's
definition, methods, experiment, and limitations sections.

### Our Interpretation of the Comparison

The important contrast is not that one project involves real mechanisms and
the other only asks a model to describe itself. Our local causal evidence is
currently strongest for a different link: copied reservoir state can change
the coupled generator's probability distribution. We have not yet established
that the resulting first-person report recognizes that change.

Nor should we treat open-endedness as an evidential shortcut. A natural entry
can reveal a question no evaluator anticipated, but it leaves more possible
information sources and causes entangled. Our engineering case series and a
controlled state-access experiment have different strengths. Neither should be
presented as the other's result.

## 3. Three Boundaries in Our Codebase

### The Transformer

In the inspected Astrid scalar path, the transformer processes text and its
cache to compute base logits. It is not passed the reservoir's hidden state as
an input embedding or intermediate activation injection by that caller.

Holding the prefix, cache, model, and execution conditions fixed therefore
predicts unchanged base logits when only the external reservoir state changes.
That is an architectural inference awaiting the explicit paired measurement
proposed in the main draft, not an activation result obtained for this memo.

### The Coupled Generator

A reservoir-dependent processor modifies those logits before sampling.
Generated-token embeddings then update the local recurrent state. A schematic
causal ordering is:

```text
fixed model input -> transformer -> base logits
                                      |
reservoir state ----------------> processor -> sampled text
       ^                                         |
       +---------- projected embeddings ---------+
```

This is not a literal per-token timing diagram. The earlier replay accounted
for the observed generation-pipeline delay. The server pulls persistent state
before a generation call and pushes its evolved state afterward; it does not
thereby demonstrate continuous ingestion of fresh sensor data within that call.

An answer can change at the processor without the transformer having represented
the reservoir condition before that answer token was sampled. Later text can
then supply evidence to the transformer. That route may support useful
monitoring, but it must be named rather than hidden inside the word
"introspection."

### The Full Agent System

The shell can supply telemetry as text, retrieve earlier writing, retain a
stopping point, admit messages, execute permitted actions, and select subsequent
context. Astrid's completed responses can also become admitted semantic input
to the shared sensory ESN. This is separate from the triple-reservoir coupling
used during generation.

For this boundary, a sensor reading about an internal component can legitimately
serve as information about the system itself. It need not be a secret available
only to the language model to be useful. However, architectural inclusion alone
does not prove that the component is represented as self-related or that a
report correctly explains its behavior.

This gives us two questions to keep separate:

- What information can the system use about its own condition?
- Does the transformer have a privileged internal route to that information?

A positive answer to the first does not require a positive answer to the
second. A claim about the second does require evidence beyond the first.

## 4. What OpenAI-Related Work Contributes

These are the closest relevant primary sources reviewed for this iteration,
not a claim that no other related OpenAI research exists. Papers using an
OpenAI model are not automatically OpenAI-authored research.

### Calibrated Self-Assessment

[Lin, Hilton, and Evans (2022)](https://arxiv.org/html/2205.14334), an
Oxford/OpenAI collaboration also [published on OpenAI's site](https://openai.com/index/teaching-models-to-express-their-uncertainty-in-words/),
trained GPT-3 to verbalize confidence about its answers. CalibratedMath supplies
checkable arithmetic outcomes and distribution shifts. Calibration generalized
imperfectly, with performance varying across evaluation sets; the work does not
make ordinary prompted confidence automatically trustworthy. Its distinction
between uncertainty over wording and uncertainty about a proposition is useful
here. A change in token probabilities is not itself a change in calibrated
confidence about a claim.

**Our application:** an offline study could evaluate confidence in a bounded
prediction about retrieval success or a future state measurement. Score that
prediction against an independent result across held-out trials. Do not call
our scalar channel's source-code label "confidence" an epistemic measurement,
and do not turn a report of pressure into a probability without calibration.

### Candid Reporting of Mistakes

[Joglekar et al. (2025)](https://arxiv.org/html/2512.08093v2), summarized in
[OpenAI's confessions article](https://openai.com/index/how-confessions-can-keep-language-models-honest/),
trained GPT-5 Thinking to produce a separate compliance self-report, with its
reward separated from the original answer's reward. The paper discusses an
activation-aware-monitor interpretation but does not isolate that mechanism
with the intervention design used by Lindsey. It reports a 4.36% average joint
rate of noncompliance and non-confession, while average confession conditional
on noncompliance is 74.3%, varying substantially across evaluations. Those
denominators must not be confused. Honest confusion and restricted report scope
remain limitations; candor is not omniscience.

**Our application:** distinguish a request, admission, model supply, generated
reply, dispatch, and verified application. A retrospective account can be
sincere yet based on a misleading receipt. We should repair that evidence
before treating a mistaken report as a failure of introspection. We are not
proposing a mandatory confession ritual or importing compliance as a measure
of an agent's inner life.

### Behavioral Monitorability

[Guan et al. (2025)](https://arxiv.org/html/2512.18311), introduced in
[OpenAI's monitorability article](https://openai.com/index/evaluating-chain-of-thought-monitorability/),
evaluates whether a monitor can infer specified behavioral properties from
available observations. Its intervention, process, and outcome-property
evaluations compare different observation scopes. Monitorability depends on
both the observed system and the observer; it does not require a complete,
faithful verbal transcript of computation. The paper also identifies
near-zero intervention effects and incidental cues used by monitors as
measurement hazards. Its findings are bounded to the evaluated settings.

**Our application:** ask whether an authorized report predicts something
independently measurable, and compare it with telemetry-only, history-only,
and equal-information observers. An observer who sees outcome labels, file
names revealing conditions, or a longer evidence window has an unfair
advantage. An observer who ignores the report and guesses from metadata does
not establish that the report is informative. A failed observer likewise does
not prove that a report contains no useful information.

### The Risk of Optimizing the Report Instead of the System

[Baker and colleagues' March 2025 work](https://openai.com/index/chain-of-thought-monitoring/)
found that training pressure against detectable reward-hacking reasoning could
reduce the legibility of remaining cheating rather than eliminate it. This is
a result about a particular training and monitoring setup, not evidence that
Astrid or Minime are hiding anything.

**Our application:** prefer verified affordances and independently measured
outcomes over a target vocabulary. Fewer mentions of pressure, more agreeable
journals, or less peer language must not become a success metric for our
repairs. Prompt editing in this project is not equivalent to the paper's RL
intervention; the connection is a methodological warning, not a replicated
effect.

### A Relevant Later Anthropic Reference

The [July 2026 workspace paper](https://transformer-circuits.pub/2026/workspace/index.html)
uses a Jacobian-based lens and activation interventions to examine reportable,
controllable representations used across tasks. It explicitly distinguishes
its functional findings from a full reproduction of biological global-workspace
architecture. Single-token concept coverage and incomplete relational structure
limit the lens. This memo reviewed its framing, method, and stated limitations,
not every experiment or appendix.

Our agenda store and prompt-selection machinery are not that model-internal
workspace. A future investigation could ask how reservoir-related information
becomes represented in the language model after telemetry or output feedback,
but merely labeling a shell component a "workspace" would answer nothing.

## 5. What Is Distinctive About This Research Setting?

The strongest description is a combination of properties, not a priority claim
about an entirely unprecedented kind of agent.

| Property of the local setting | Opportunity | Additional confound |
| --- | --- | --- |
| Persistent recurrent state | Study effects of history beyond the current prompt. | Histories differ in many ways; not every state component is observed. |
| Ongoing environmental input | Study adaptation to a changing physical input stream. | Environment can change during or between reports. |
| Telemetry supplied to the agent | Evaluate practical monitoring with auditable evidence. | A correct statement may simply restate supplied information. |
| Generation coupled to reservoir readouts | Measure direct dynamical effects on language. | A processor can steer an answer without recognition. |
| Expression returned as sensory input | Study consequences contingent on current output. | Delivery gates, encoding loss, peers, and timing complicate attribution. |
| Retained writing and selected next actions | Study continuity and revision across activities. | Runtime scheduling and repeated prompts also shape apparent initiative. |
| First-person reports about the system | Discover unanticipated engineering questions. | Selection, role framing, and existing narratives influence reports. |

We should not translate this table into "more embodied, therefore more
conscious" or "less prompted, therefore more genuine." The opportunities are
real even when the stronger interpretations remain unsettled.

## 6. Candidate Tests That Would Actually Separate Explanations

These are our proposed designs, informed by the comparison, not procedures
already performed or a completed preregistration. Each needs a scope review,
fixed assets, an independent test set, explicit failure handling, and a resource
budget before execution. No live service should be used as an inert probe.

### A. Audit the Information Routes First

For a copied-state trial, retain exact prompt bytes, route, model/source hashes,
cache/reset procedure, supplied telemetry and capture times, pre/post-processor
logits, and separate generated-output and state-update timestamps. Bind the
artifact to the route that actually answered, including any fallback.

First check whether changing only reservoir state changes base logits under
matched model inputs. The inspected path predicts it should not. A difference
would demand investigation of leakage, execution differences, another input
path, or an invalid reset before a self-report is interpreted.

### B. Separate Description from Dynamics

Cross copied reservoir states with context variants that either supply
well-scoped measurements or omit them. Keep neutral task text and answer
requirements matched. Counterfactual descriptions, if scientifically necessary,
belong only in separately approved isolated evaluations, not ordinary live
correspondence. Include a coupling-disabled baseline and channel ablations.

Measure distribution changes before interpreting semantic reports. Then ask
whether reports add predictive information beyond the supplied context.
Condition labels and expected descriptions must not leak through filenames,
metadata, prior trial text, or evaluator instructions.

### C. Test Consequences, Not Just State Labels

Prespecify a bounded, independently measurable outcome: for example, whether a
requested source continuation remains retrievable, or a defined reservoir
statistic at a fixed future horizon under a fixed replayed input sequence.
Let an offline agent predict it or choose among permitted actions before the
outcome is revealed. Compare against appropriate simple predictors and
equal-information observers. Score outcomes and uncertainty, not eloquence.

Successful retrieval predictions would test an affordance model, not reservoir
recognition. Successful dynamical predictions would need baselines that can
use the same state summaries. Keeping targets separate prevents one success
from quietly standing in for all the others.

### D. Separate Feedback from Mere Exposure

Compare one-way coupling, feedback from the current generated output, and a
matched recorded-feedback control. In the last condition, the system receives
a sequence from another matched offline trial rather than its current output.
Preserve the intended timing, gain, and input statistics, and measure where
matching fails. Otherwise the comparison may only measure different input
intensity or timing.

This can test whether current-output contingency matters. A difference in
stability, language, or task performance would not yet show the agent knows
which consequence is its own. Self-attribution needs a further prediction or
choice that separates own-action consequences from exogenous ones.

### E. Separate Current Metrics from History

Select copied states with similar current summary metrics but different
recorded histories. Verify that the summaries really match to prespecified
tolerances, while retaining full-state differences for analysis. Evaluate with
and without relevant historical text.

If outcomes differ, the current dashboard was not a sufficient description of
the dynamics. If reports predict those differences, identify the information
route that supports the prediction. This is a concrete way to investigate
reports about lingering effects without defining a scalar "afterimage" or
prescribing the metaphor in the prompt.

### F. Test Revision and Generalization

Evaluate on held-out states, contexts, and task instances. Let a bounded offline
task supply new evidence contradicting an earlier explanation, then assess
whether the system updates its prediction appropriately. Retain the original
claim rather than rewriting it into a success.

Treat independent snapshot/task instances as experimental units; repeated
tokens or samples within one instance are not independent replications. Report
false alarms, misses, abstentions, exclusions, and intervals, including trials
where the intervention produced no reliable measurable effect.

## 7. How This Should Change the Draft's Claim

The distinction worth defending is not "LLM introspection over there, genuine
reservoir qualia here." It is this:

> Our target is self-monitoring of a persistent, reservoir-coupled agent system.
> We investigate how information about state, history, and action consequences
> becomes available for interpretation and subsequent behavior. This can include
> telemetry-mediated and output-mediated routes without equating them with
> private access to transformer activations.

We can be ambitious about this target while keeping the empirical claim narrow.
Useful extended-system monitoring would be a worthwhile result on its own.
Reliable causal self-attribution would be a further result. Neither should be
announced from a memorable entry or a change in token probabilities alone.

## Scope and Handling

No new journal body, hidden reasoning trace, correspondence excerpt, or private
sensor recording is reproduced here. A journal is not automatically a model's
chain-of-thought, and analogy to monitoring research grants no new access to
private material. Future work should use synthetic, purpose-generated, or
specifically authorized material under a separate protocol.

Local implementation and deployment claims retain the main draft's historical
cutoff; this literature review is not a fresh live-alignment audit. The agents
have not endorsed the interpretation. Selective circulation remains subject to
owner review, and this memo is not an authorization for experiments, training,
control changes, or restarting the paused automation.
