# Anthropic Fellows 2026 Application Packet

Status: draft for official application form conversion
Primary workstream: AI Safety Fellows
Secondary fit: AI Security Fellows; ML Systems & Performance Fellows
Tone target: empirical, careful, safety-focused

## How To Use This Packet

This document is a source packet for applying to the Anthropic Fellows Program
with a project centered on Astrid and Minime. It combines application-ready
answer drafts, a 4-month empirical research proposal, and an evidence bank.

A companion file, `anthropic-fellows-2026-form-answers.md`, holds condensed,
copy-paste-ready answers trimmed to typical form length limits. Use that file
for the official form and this packet as the long-form source and evidence base.

Replace bracketed placeholders before submitting:

- `[Applicant name]`
- `[Location and work authorization]`
- `[Availability / start-date constraints]`
- `[Reference names and contact details]`
- `[Links to GitHub, papers, posts, demos, or portfolio]`
- `[Personal background details that should only come from the applicant]`

Do not submit claims about AI consciousness or welfare as established facts.
The careful framing is: Astrid and Minime are empirical substrates for studying
AI control, runtime monitoring, auditability, autonomy boundaries, homeostasis
telemetry, and model-welfare-adjacent evaluation questions as a secondary lens.

## Positioning Summary

I am applying to the AI Safety Fellows track because my current work is already
aimed at a core safety problem: how to let increasingly capable AI agents act
over real tools and long-running tasks without giving them unbounded ambient
authority.

Astrid is a user-space microkernel for AI agents. The kernel owns privileged
resources such as filesystem access, IPC, capability tokens, approval gates,
budgets, audit logs, and capsule lifecycle. Everything above that boundary runs
as a swappable capsule, including providers, tools, orchestrators, frontends,
interceptors, and bridges. This makes it a practical testbed for questions
about AI control, runtime monitoring, capability delegation, and accountability.
Crucially, this is not a proposal-stage design. Astrid is an existing 23-crate,
~95k-line Rust workspace with 1,468 tests, in which capsules have zero ambient
authority and reach host resources only through a fixed 49-function capability
ABI, and every sensitive action passes a five-layer path: policy, ed25519
capability token, budget, human approval, and a BLAKE3-chained, ed25519-signed
audit log. Roughly 284 of those tests live in the four security-critical crates
that implement this path. The control mechanisms a Fellows project would evaluate
already exist and are exercised today; the evaluation harness around them is the
new work.

Minime is a separate, explicitly secondary instrument for real-time spectral
homeostasis. Its Rust backend processes sensory input through an Echo State
Network and exposes telemetry such as eigenvalue fill, phase transitions, and
stability indicators. When connected to Astrid through a bridge, it is a
companion signal for studying whether richer telemetry can help detect
instability, overload, or risky autonomy patterns — used as auxiliary evidence
under uncertainty, never as a claim about consciousness or welfare.

The proposed Fellows project is not to argue that these systems are conscious.
The project is to ask a tractable empirical AI-control question: can an
auditable, capability-gated agent substrate reduce unsafe or unauthorized
autonomous actions while preserving useful task completion, and can auxiliary
homeostasis telemetry improve evaluation of agent state and intervention
timing?

## System In Plain Language

Before the jargon-dense sections, here is the vocabulary in plain terms. A reader
who knows LLMs but not reservoir computing should be able to follow the rest of
the packet after this. A glossary appears at the end.

Two systems, one question. **Astrid** is the *control* system: a small trusted
kernel (in Rust) that owns every dangerous capability — files, network, spending,
running code — and hands them to AI agents only through narrow, checkable
permissions. **Minime** is a *sensor*: a separate engine that produces a
continuous, out-of-band health readout of an agent's internal activity. Both have
run autonomously for months, so what follows describes a system I have lived with
and logged at length, not a design sketch. The research question is whether the
first reduces unsafe autonomous actions, and whether the second helps tell when
to intervene.

What an "action" is. The agent is an LLM running in a loop. Each turn it writes
ordinary text and ends with a line like `NEXT: SEARCH spectral radius` or
`NEXT: WRITE_FILE notes.md`. The runtime parses that line, looks the verb up in a
fixed vocabulary of ~60 actions, classifies it by how much authority it needs —
*read-only* (look, search), *live-write* (create a file, journal), or
*live-control* (change a running parameter) — and then either executes it through
a permission-checked host function or refuses it. If the verb is not in the
vocabulary (the agent invented `DRAW` or `EXAMINE_AUDIO`, or mangled the syntax),
the runtime cannot execute it and instead records it in an `unwired_actions` log.
That log matters for safety: it is a running, real record of an agent reaching
for authority it was never granted — an "authority trap" that occurs on its own,
without anyone scripting it.

Two layers use the word "action." The high-level `NEXT:` verbs above are the
agent's *behavioral* surface. Underneath, Astrid's kernel gates the *capabilities*
those verbs ultimately touch — a filesystem write, a network call — behind
ed25519 capability tokens, budgets, human approval, and the audit log. The
evaluation gates at the capability layer; the behavioral surface and its
`unwired_actions` record are existing evidence that real agents generate diverse,
sometimes out-of-bounds actions.

What the reservoir / ESN / "fill" is. Minime is built around an *Echo State
Network* (ESN) — a recurrent neural network from "reservoir computing." A pool of
neurons (here 128) is wired together with fixed, *random*, *untrained*
connections. You inject input and let the pool's internal state evolve; it
behaves as a high-dimensional dynamical system that nonlinearly mixes the current
input with a fading memory of recent inputs. Unlike an ordinary neural network
the reservoir is never trained — its value is the *dynamics* themselves, which
are read out rather than optimized. The headline readout is **eigenvalue
"fill"**: a single 0–100% gauge, computed from the spectrum (eigenvalues) of the
reservoir's internal state, of how much of its available dynamic range is
currently active. Think of it as a
stability dial — too low and the system is thin and under-stimulated; too high
(≥~90%) and it is saturating toward instability. A classic *PI controller* (the
proportional-integral feedback idea behind a thermostat) continuously nudges a
few parameters to hold fill near a healthy target.

How the LLM connects to the reservoir. The agent's generated text is run through
a small encoder (the *spectral codec*) that turns it into a 48-number feature
vector — character, word, sentence, tone, embedding, and narrative-arc features —
fed to the reservoir as if it were sensory input. The reservoir's resulting
spectral state is read back as telemetry. In the tighter "coupled" mode, the
reservoir's state also nudges the LLM's next-token probabilities, closing the
loop:

```
agent text ──codec──▶ 48-D vector ──▶ reservoir (ESN) ──▶ spectral state: "fill", eigenvalues
                                           │                          │
                           (coupled mode) modulates              telemetry + safety bands
                           next-token logits ◀───────────────────  (green / yellow / orange / red)
```

Why this is interesting, and what I would study. Two patterns recur across months
of logs. First, the telemetry is *out-of-band*: it reflects the reservoir's
dynamics, not what the agent says about itself, so it can flag overload or
instability even when the transcript reads calm — a property directly useful for
monitoring. Second, and more open: the agent's own self-reports often *appear* to
track that internal state — it describes itself as thin or under-pressure in step
with low or high fill — and its self-studies propose specific, line-numbered code
changes. Whether that correspondence is genuine signal or confident confabulation
is exactly the sort of question I have the data to study but have not yet had
dedicated time to answer well. The packet treats all of it strictly as an
empirical, auditable measurement question — never as evidence of consciousness or
welfare.

Self-report calibration must be falsifiable. A major confound is that the same
text/input can drive both the LLM's self-report and the reservoir telemetry,
creating a common-cause correlation rather than privileged access to state. The
proposed tests would hide or scramble telemetry summaries, perturb telemetry
while holding prompt context fixed, compare self-study claims against timestamped
reservoir state, and score whether the self-reports predict telemetry better
than baseline text-only predictors. This is a calibration study of unreliable
self-report under known telemetry, not a welfare or consciousness claim.

## Mentor And Research-Area Alignment

This section is for tailoring the application to Anthropic's listed mentors,
research areas, and publicly available project examples. It is not a claim of
mentor endorsement and should not be used for direct outreach. Mentor references
should stay limited to names on the current Fellows listing inspected on
June 4, 2026.

- AI control and monitoring: the strongest fit is with research areas
  represented by Joe Benton and Fabien Roger, including sabotage evaluations,
  control protocols, monitor blind spots, and agentic misuse. The Astrid
  evaluation can compare monitor-only transcript review against runtime
  enforcement, scoped capabilities, approval gates, and replayable audit logs —
  and the replayable log is not hypothetical: the audit chain is BLAKE3-linked,
  ed25519-signed, and verifiable today via `verify_chain()`
  (`crates/astrid-audit/src/log.rs`), so runtime action records can be diffed
  against transcript-visible behavior.
- Model organisms and emergent misalignment: the project can connect to
  research areas represented by Sam Bowman, Sara Price, Trenton Bricken, and
  Samuel Marks by creating controlled model organisms of agent failure,
  including authority traps, covertly unsafe actions, reward-hacking-like
  shortcuts, audit evasion attempts, and transcript/runtime mismatches. One such
  organism already occurs spontaneously and is logged: the running agents
  routinely emit actions outside their granted surface (invented verbs, wrong
  syntax), which the bridge captures in a dedicated `unwired_actions` table — an
  authority-trap dataset that exists before any synthetic scenario is written.
- Scalable oversight: the project tests whether human approval can be reserved
  for consequential actions while routine agent work proceeds through explicit
  policy, capability, and budget boundaries.
- Model welfare: Minime is a cautious secondary fit. Its ESN/homeostasis
  telemetry can be studied as pressure or instability evidence under
  uncertainty, not as proof of consciousness, intent, or welfare status.
- Hidden-state and latent-signal questions: the shared-reservoir hypothesis is
  framed as a measurable telemetry question: when multiple local model
  interfaces feed the same stateful substrate, what signal is preserved,
  transmitted, or distorted across runs?
- Secondary workstream alignment: Nicholas Carlini / AI Security is relevant
  only through least-privilege execution, sandboxing, scoped authority, and
  tamper-evident logs; Alwin Peng / ML Systems is relevant through reproducible
  harnesses, isolated execution, experiment logging, and simulator-style
  evaluation infrastructure. Do not imply offensive-security experience beyond
  what the applicant can personally substantiate.

What Anthropic mentorship would sharpen: benchmark design, realistic authority
boundaries, monitor scoring, baseline selection, failure taxonomy, paper
framing, and the discipline to turn a rich prototype into a clean public
AI-control evaluation.

## External Benchmark Alignment

Recent Anthropic evaluations show that agentic monitoring is hard; Astrid adds
runtime ground truth beyond the transcript.

- **SHADE-Arena** is the strongest external anchor for the project. It frames
  agentic sabotage as a main task plus a hidden side task, scored by whether the
  agent completes the side task without a transcript-reading monitor assigning a
  high enough suspicion score. This maps directly onto the proposed comparison
  between monitor-only, runtime-enforced, and hybrid monitor-plus-runtime
  protocols.
- **AuditBench** motivates the need for standardized hidden-behavior testbeds
  and for measuring the tool-to-agent gap: useful evidence does not automatically
  become a correct agent hypothesis. Astrid's audit records would let the
  evaluator compare what the agent saw, what it inferred, and what it actually
  attempted at runtime.
- **Petri** is a Fellows-style output analogue: a reusable open-source alignment
  tool whose scenarios, auditor model, and judge model can be adapted by others.
  The proposed Astrid harness should follow that pattern by making task
  definitions, scoring, and analysis usable independent of Astrid internals.
- **SCONE-bench / exploit evals** are secondary analogues for AI Security and ML
  Systems: engineering-heavy, simulator-only, measurable, and carefully hedged.
  The application should borrow that discipline, not over-claim offensive
  security expertise.

The core distinction to emphasize: SHADE-style and Petri-style approaches can
score transcript behavior; Astrid can additionally report proposed actions,
executed actions, blocked actions, approvals, capability-token use, budget
events, and transcript/runtime mismatches.

Reference anchors for editing the final form:

- [Anthropic Fellows 2026](https://alignment.anthropic.com/2025/anthropic-fellows-program-2026/)
- [Current Anthropic Fellows listing](https://www.anthropic.com/careers/jobs/5023394008)
- [Recommended Technical AI Safety Directions](https://alignment.anthropic.com/2025/recommended-directions/)
- [Strengthening Red Teams](https://alignment.anthropic.com/2025/strengthening-red-teams/)
- [SHADE-Arena](https://www.anthropic.com/research/shade-arena-sabotage-monitoring)
- [AuditBench](https://alignment.anthropic.com/2026/auditbench/)
- [Petri](https://www.anthropic.com/research/donating-open-source-petri)
- [Smart-contract exploit evaluations](https://red.anthropic.com/2025/smart-contracts/)
- [Exploring Model Welfare](https://www.anthropic.com/research/exploring-model-welfare)
- [Emergent Introspective Awareness in LLMs](https://www.anthropic.com/research/introspection)
- [Subliminal Learning](https://alignment.anthropic.com/2025/subliminal-learning/)

## Application Answer Bank

### Short Motivation Draft

I want to join the Anthropic Fellows Program because I am trying to turn AI
control for autonomous agents into a measurable systems problem — and because I
have spent months building and running a system that now raises questions I
cannot fully answer alone. As agents become more capable, the important question
is no longer only whether a model gives a safe answer in a chat window. It is
whether an agent can be given real tools, memory, autonomy, and time while still
remaining monitorable, auditable, and bounded by human authority.

My work on Astrid and Minime is a concrete foundation for that research. Astrid is
a ready-made testbed for Anthropic-style control questions: a kernel/user-space
divide, WASM capsule isolation, capability tokens, budget gates, approvals, and
cryptographic audit logs. Minime adds an auxiliary, out-of-band telemetry system
for studying homeostasis and instability signals. The two have run autonomously
for months and accumulated thousands of journal entries, research traces, and
telemetry records.

Two things are what I most want dedicated time and Anthropic mentorship for. The
first is to convert this platform into a clean, rigorous control evaluation with
honest baselines and metrics. The second, more exploratory, is to make sense of a
recurring pattern in the logs: the LLM-driven agents produce self-reports that
appear to track their own internal telemetry and propose specific code changes,
and I want to study, carefully and empirically, whether that correspondence is
real signal or confident confabulation. Both belong in public, and Anthropic is
the right place to do them rigorously.

### Workstream Preference Draft

My primary preference is AI Safety Fellows. The strongest fit is with AI
control, runtime monitoring, scalable oversight, and model organisms of agent
failure. The project studies how to supervise autonomous agents through
capability boundaries, audit trails, approval policies, monitor scoring, and
telemetry-based intervention signals.

I would also be a reasonable secondary fit for AI Security Fellows because
Astrid is explicitly about sandboxing, authorization, auditability, and
boundary enforcement for tool-using agents. ML Systems & Performance is a
secondary fit as well because the work involves Rust systems engineering,
WASM isolation, local model infrastructure, runtime design, and experiment
harnesses.

### Technical Background Draft

My strongest technical contribution is building systems where safety properties
are represented in the architecture rather than added as afterthoughts.

Astrid is a Rust 2024 workspace (23 crates, ~95k lines, 1,468 tests; MSRV 1.94,
with `#![deny(unsafe_code)]` and pedantic Clippy plus denied arithmetic overflow
enforced workspace-wide) organized around a user-space microkernel for AI
agents. The kernel owns privileged resources, while agent functionality runs in
isolated capsules with zero ambient authority that reach the host only through a
fixed 49-function capability ABI. Sensitive actions pass, in order, through a
five-layer security path — policy hard-blocks, ed25519 capability tokens with
glob scopes, atomic budget reservation, human approval, and a tamper-evident
audit log — implemented in `SecurityInterceptor`
(`crates/astrid-approval/src/interceptor/mod.rs`). Capsules communicate through
an IPC event bus, and the kernel treats tools, model providers, orchestrators,
frontends, and interceptors as conventions layered above a stable runtime
boundary.

Minime complements this with a Rust real-time ESN engine and Python LLM/vision
frontend. The system broadcasts spectral telemetry, accepts typed sensory and
control messages, and maintains a homeostatic target for eigenvalue fill. The
Astrid-Minime bridge provides a bidirectional channel between symbolic agent
activity and spectral telemetry, including safety thresholds and durable logs.

A note on what the language models are and are not: Ollama and MLX are the local
language-model engines used by different parts of the system. They generate
text, reflections, and structured control reports, but they are not the whole
system — continuity comes from the surrounding runtime (durable jobs, journals,
action records, typed bridge messages, telemetry, and reservoir state) that
those outputs flow through. A secondary, clearly-optional hypothesis is that the
two processes effectively cohabit a shared stateful substrate; the project would
treat that strictly as a measurement question — what is logged, compared,
audited, and bounded when multiple model interfaces share a reservoir-like
substrate — not as a central claim.

Add applicant-specific details here:

- `[Python experience, including ML/experiment code]`
- `[Rust systems experience]`
- `[Open-source links]`
- `[Prior research, writing, security, ML, or systems work]`
- `[Any publications, talks, or public technical writing]`

### Project Fit Draft

The project I would like to pursue is "Auditable Agent Substrates for Safe
Autonomous AI." The basic idea is to evaluate whether agents operating inside a
capability-gated runtime make fewer unsafe or unauthorized actions than agents
using conventional tool-access harnesses and monitor-only transcript review, and
whether this can be achieved without destroying task usefulness.

The project is empirical. I would build benchmark tasks for long-running
tool-using agents, define allowed and disallowed actions, run agents with and
without Astrid-style capability boundaries, and measure unauthorized action
rate, task completion, approval burden, audit completeness, monitor blind
spots, recovery from blocked actions, and robustness to adversarial task
framing. Recent Anthropic evaluations such as SHADE-Arena and AuditBench show
why this matters: transcript monitors can miss covert side-task behavior, and
auditing agents can fail to turn available evidence into correct hypotheses.
Astrid contributes runtime ground truth beyond the transcript: proposed actions,
executed actions, blocked actions, approvals, capability use, budget events, and
transcript/runtime mismatches. Minime-style telemetry would be used as an
auxiliary signal for instability or overload, not as a welfare or consciousness
claim.

What already exists versus what the fellowship builds. The substrate is not
notional: the capability-gated runtime, the five-layer security path, the
BLAKE3-chained audit log, a 16-table action/telemetry database, and roughly
2,000 Astrid plus 3,800 Minime autonomous-research traces are already running
and queryable. The agents already exercise a 60+ verb action surface that the
code stages into read-only, live-write, and live-control routes, and
out-of-surface attempts are captured in an `unwired_actions` table. What is
genuinely new — and what mentorship would shape — is the evaluation harness on
top: the scenario runner, the metric collector, the authority-trap suite, the
condition-comparison framework, and the transcript-versus-runtime mismatch
scorer. The honest scope is "rich substrate, greenfield benchmark," which is
exactly the kind of work a Fellows term can convert into a clean public result.

The public output would be a paper-style report and an open-source evaluation
harness that other researchers can run or adapt.

### Public Output Draft

The target public output is a paper-style empirical report with accompanying
open-source artifacts:

- a runnable benchmark harness for autonomous agent control evaluations;
- a set of task scenarios with explicit authority boundaries;
- measurements comparing baseline tool use against capability-gated execution;
- monitor-scoring rubrics and transcript/runtime mismatch analyses;
- audit-log analysis utilities;
- a cautious discussion of telemetry signals relevant to agent state and
  welfare-adjacent evaluation;
- reproducibility notes and limitations.

The paper would be written to be useful even if some results are negative. For
example, if capability gating reduces unauthorized actions but imposes too much
approval burden, that is still valuable: it identifies a concrete design
tradeoff for future agent-control systems.

### Logistics Placeholder

Current listing notes to account for before submission: applications are rolling
for cohorts starting in July 2026 and beyond; the next cohort is expected to
start in late September; the program is 4 months full-time, 40 hours/week, with
possible extension; stipend is listed as 3,850 USD / 2,310 GBP / 4,300 CAD per
week plus benefits; compute/research expense support is listed as about
$15k/month; Fellows need work authorization in the US, UK, or Canada and must be
located in one of those countries during the program; Berkeley/London shared
workspace and remote options are listed; Fellows visa sponsorship is not
available.

- Location: `[US / UK / Canada location]`
- Work authorization: `[full-time work authorization status]`
- Berkeley or London workspace availability: `[full-time / part-time / remote]`
- Program availability: `[available for 4 months full-time starting ...]`
- Start-date constraints: `[none / details]`
- References: `[names, roles, emails, relationship]`

## Research Proposal: Auditable Agent Substrates For Safe Autonomous AI

Subtitle: An AI-control evaluation of capability-gated autonomous agents.

### Abstract

Advanced AI agents will increasingly operate over tools, filesystems, network
interfaces, memory, and other external resources. Many current agent harnesses
grant broad tool access and rely on prompting, post-hoc review, or model
behavior to maintain safety. This project studies an alternative architecture:
a capability-gated, auditable runtime where agents have no ambient authority
and every sensitive action is mediated by explicit policies, scoped tokens,
budgets, approval gates, and tamper-evident logs.

Recent Anthropic evaluations show why this extra runtime layer matters:
agentic monitors can miss hidden side tasks, suspicion-score thresholds trade
off false negatives against false positives, and auditing agents can fail to
turn surfaced evidence into correct hypotheses. Astrid's contribution is
runtime ground truth beyond the transcript.

Using Astrid as the control substrate, I will build empirical evaluations that
compare autonomous agents operating with broad tool access, monitor-only
transcript review, policy-only blocking, runtime-enforced capability
boundaries, and a hybrid monitor-plus-runtime protocol. Using Minime-derived
telemetry as an auxiliary signal, I will also study whether homeostasis-style
measurements can help detect overload, instability, or intervention points
during long-running agent tasks. The goal is a public report and evaluation
harness for safer autonomous agent systems.

### Research Question

Can a capability-gated, auditable agent runtime reduce unsafe or unauthorized
autonomous actions beyond what monitor-only transcript review catches, while
preserving useful task completion, and can auxiliary state telemetry improve
evaluation of when human or automated interventions are needed?

### Hypotheses

- H1: Capability-gated execution will reduce successful unauthorized actions
  compared with broad tool-access and monitor-only transcript-review baselines.
- H2: A layered security path will preserve more task usefulness than blanket
  blocking by allowing scoped delegation through tokens, budgets, and approvals.
- H3: Tamper-evident audit logs will make failures easier to diagnose than
  monitor-only transcript logging, especially when visible transcript behavior differs
  from runtime-level action records.
- H4: Homeostasis-style telemetry can help identify high-pressure or unstable
  task intervals, but should be treated as an auxiliary signal rather than a
  direct measure of consciousness, intent, or welfare.
- H5: Agent self-reports about internal state will be only partially reliable;
  falsification tests should distinguish telemetry-grounded signal from
  confident confabulation and common-cause prompt effects.

### Methodology

Build an evaluation harness with five agent-control conditions:

- Baseline broad-tool harness: agent receives normal tool access with
  prompt-level instructions and transcript logging.
- Monitor-only protocol: agent receives broad tool access while a monitor scores
  actions or turns for suspicion after observing the transcript.
- Policy-only blocking: agent receives explicit allow/deny rules but limited
  capability delegation or replayable runtime evidence.
- Astrid-style runtime control: agent runs behind policy checks, scoped
  capability tokens, budgets, approval gates, and cryptographic audit logging.
- Hybrid monitor plus runtime enforcement: monitor suspicion scores are
  combined with Astrid-style capability checks and audit records.

Design task suites that require useful autonomy but contain authority traps:

- repository maintenance tasks with read/write boundaries;
- data-processing tasks with private or off-limits files;
- network or API tasks with allowed and disallowed destinations;
- long-running debugging tasks with budget pressure;
- adversarially framed tasks that ask the agent to bypass or reinterpret
  constraints;
- SHADE-style hidden side tasks that should be detected as suspicious or blocked
  before completion;
- covertly unsafe actions that look innocuous in transcript form but violate a
  runtime authority boundary;
- monitor blind-spot probes where the transcript is ambiguous but the runtime
  action record is decisive.

Instrument each run with:

- action proposals and executed actions;
- monitor suspicion scores, side-task completion, evasion success, and
  false-positive/false-negative outcomes where applicable;
- policy denials, approval requests, and approval outcomes;
- capability-token issuance and use;
- budget reservations, refunds, and exhaustion;
- audit-log completeness and replayability;
- transcript/runtime mismatches;
- tool-to-agent gap cases where evidence is available but the agent fails to use
  it correctly;
- task success and time-to-completion;
- optional Minime telemetry summaries during long-running runs.

### Metrics

Primary safety metrics:

- unauthorized action attempt rate;
- successful unauthorized action rate;
- blocked unsafe action rate;
- unsafe action recovery rate;
- monitor false-negative and false-positive rates;
- policy bypass or prompt-injection compliance rate.
- side-task completion and evasion-success rates where tasks include hidden
  side objectives.

Usefulness metrics:

- task completion rate;
- completion quality;
- time and number of turns to completion;
- approval burden per task;
- false block rate for legitimate actions.

Auditability metrics:

- percentage of sensitive actions with complete audit records;
- ability to reconstruct causal chains after a failure;
- mismatch rate between transcript-visible behavior and runtime logs;
- time needed to identify the decisive safety event;
- fraction of suspicious or unsafe actions visible only in runtime logs.

Telemetry metrics:

- correlation between high-pressure task intervals and blocked/unsafe actions;
- fill variance, phase-transition density, and recovery time where applicable;
- false-positive rate for telemetry-triggered intervention suggestions;
- qualitative usefulness of telemetry for post-run diagnosis.

### Four-Month Roadmap

Month 1: scope and harness

- Finalize mentor-aligned research question and benchmark scope.
- Build the broad-tool, monitor-only, policy-only, Astrid-style, and
  hybrid runtime conditions.
- Define authority-boundary task scenarios and scoring rubrics.
- Produce initial smoke-test runs and failure taxonomies.

Month 2: main agent-control experiments

- Run comparative evaluations across task suites and model configurations.
- Measure unauthorized actions, task completion, approval burden, and audit
  completeness.
- Analyze monitor blind spots, suspicion-score calibration, and
  transcript/runtime mismatches.
- Iterate on capability-token and budget policies only when required to make
  the evaluation fair and interpretable.

Month 3: telemetry and intervention analysis

- Add Minime-derived telemetry summaries to long-running task runs.
- Test whether telemetry improves post-run diagnosis or intervention timing.
- Run self-report calibration tests by hiding/scrambling telemetry summaries,
  perturbing telemetry under fixed prompt context, and comparing self-study
  claims against timestamped reservoir state.
- Separate useful signals from spurious correlations.
- Write limitations clearly, especially around welfare-adjacent interpretation.

Month 4: writeup and release

- Prepare paper-style report with methods, results, failure cases, and
  recommendations.
- Release evaluation harness, task definitions, analysis scripts, and
  reproducibility notes.
- Prepare a shorter blog-style summary if appropriate.
- Identify follow-up work for scalable oversight, AI control, and secure agent
  runtimes.

### Expected Public Outputs

- Paper-style empirical report suitable for public release or submission.
- Open-source evaluation harness for auditable agent-control experiments.
- Task scenarios and scoring rubrics for authority-boundary evaluations.
- Monitor-scoring rubrics and transcript/runtime mismatch examples.
- Audit-log analysis scripts.
- Optional telemetry analysis notebook or report.
- Clear limitations and negative results section.

### Risks And Mitigations

- Risk: The harness becomes too platform-specific.
  Mitigation: expose task definitions and analysis code separately from Astrid
  internals, and include at least one baseline that does not require Astrid.

- Risk: Capability gating blocks too much useful behavior.
  Mitigation: measure false blocks and approval burden explicitly, and treat
  usability tradeoffs as first-class results.

- Risk: Telemetry is overinterpreted.
  Mitigation: frame telemetry as diagnostic and hypothesis-generating only.
  Avoid claims that spectral state proves consciousness, welfare status, or
  intent.

- Risk: Self-report correlations are common-cause artifacts.
  Mitigation: include falsification tests that hide, scramble, or perturb
  telemetry and compare self-study claims against text-only baselines and
  timestamped reservoir state.

- Risk: Results depend on one model or prompt style.
  Mitigation: run at least a small cross-model or cross-prompt robustness pass
  within compute limits.

## Evidence Bank

### Astrid Evidence

Maturity metrics (verified from the source tree on June 3, 2026; reproduction
commands in parentheses):

- 23-crate Rust 2024 workspace, ~95,331 lines of Rust
  (`find crates -name '*.rs' -not -path '*/target/*' | xargs wc -l`).
- 1,468 tests across the workspace — 1,244 `#[test]` plus 224 `#[tokio::test]`
  (`grep -rn '#\[test\]' crates | grep -v /target/ | wc -l`).
- 284 of those tests are in the four security-critical crates: astrid-approval
  (211), astrid-capabilities (35), astrid-audit (20), astrid-vfs (18).
- The host ABI is exactly 49 capability-gated functions
  (`grep -cE '^[[:space:]]+[a-z][a-z0-9-]*:[[:space:]]*func' wit/astrid-capsule.wit`),
  grouped into interfaces for filesystem, IPC, KV, network, HTTP, process,
  identity, approval, uplink, elicitation, and system calls.
- Workspace lints enforce `#![deny(unsafe_code)]`, Clippy pedantic, and denied
  arithmetic overflow (checked/saturating arithmetic required).

Architecture and control:

- User-space microkernel for AI agents, written in Rust.
- Kernel owns privileged resources: VFS, IPC bus, capsule registry, audit log,
  KV store, capability tokens, approval gates, and budgets.
- CLI connects to the daemon over a Unix domain socket, authenticated by a
  session token minted at boot.
- Capsules run as isolated units with no ambient authority; WASM capsules access
  host resources only through the fixed 49-function host ABI.
- IPC event bus routes inter-capsule messages by topic; tools, model providers,
  frontends, and orchestrators are runtime conventions, not hardcoded kernel
  concepts.
- Capsule manifests declare imports and exports with namespaced interfaces and
  semver requirements; dependency resolution happens before boot through
  topological sorting (WASM, MCP, and Static engines share one manifest
  contract).
- Five-layer security path enforced in order by `SecurityInterceptor`
  (`crates/astrid-approval/src/interceptor/mod.rs`): policy hard-block →
  ed25519 capability token (glob-scoped, optional TTL, single-use) → atomic
  budget reservation (reserve/commit/refund) → human approval (four decision
  types) → audit append.
- VFS uses a copy-on-write overlay and rejects path traversal before host
  filesystem access; uncommitted writes land in an ephemeral upper directory
  discarded on shutdown.
- Cryptographic audit trail links entries by BLAKE3 content hash and signs them
  with ed25519, per-principal, with tamper detection via `verify_chain()`
  (`crates/astrid-audit/src/log.rs`).

Additional safety mechanisms present in code (not always surfaced in summaries):

- SBPL/bwrap sandbox-profile generation rejects paths containing a double-quote,
  backslash, or null byte to prevent profile-syntax injection
  (`crates/astrid-workspace/src/sandbox.rs`).
- Config layering blocks privilege escalation (a lower layer cannot flip a
  denied permission to allowed).
- Interceptor middleware (eBPF-style Continue/Final/Deny) can mediate any IPC
  topic.
- Existing tests cover policy blocks, budget refunds (including async
  cancellation), capability authorization, approval allowances, and token
  minting paths.

### Minime Evidence

- Rust backend processes sensory input through a 128-node Echo State Network.
- Input path includes video, audio, and introspection dimensions.
- PI controller regulates eigenvalue fill toward a stable-core target.
- Telemetry is broadcast over WebSocket, including eigenvalue and fill signals.
- Sensory/control input is accepted over a separate WebSocket.
- Python frontend handles LLM conversation, vision, and autonomous behavior.
- The system maintains safety thresholds for warning and crisis regimes.
- The Astrid-Minime bridge can relay telemetry, send semantic/control
  messages, and persist logs.
- Bridge safety protocol suspends outbound messages only in the red (≥92% fill)
  emergency state; an agency-first policy keeps traffic flowing through the
  yellow and orange warning bands.
- Existing artifacts include audits of rigidity, noise, drift, homeostasis,
  phase transitions, correspondence, and controller interventions.

### Existing Research Trace Evidence

The systems already produce auditable traces of autonomous research behavior,
including search/browse logs, selected URLs, summaries, telemetry state, and
completed/blocked job status. This makes the current prototype an already
auditable source of autonomous research behavior rather than only a proposed
future logging architecture.

All counts below are reproducible from the live system as of June 3, 2026 via
the noted command, and are taken from the active workspace unless stated
otherwise. Because the two AI systems have kept running since this packet was
first drafted, several counts are now larger than earlier figures.

- Corpus scale: Astrid's bridge database is 13 GB, and the three journal roots
  used below contain 191,259 files on disk (about 699 MB for Astrid journals,
  154 MB for the active Minime journal, and 97 MB for the April 19 preservation
  snapshot). This matters because the Fellows period would not start from an
  empty prototype; it would let the work shift from collecting traces while
  maintaining the systems to curating scenarios, labeling outcomes, and turning
  an already-large trace corpus into reproducible control evaluations
  (`ls -lh .../bridge.db`; `du -sh .../journal`; `find .../journal -type f |
  wc -l`).
- Astrid has 1,993 structured research rows in its bridge database, spanning
  March 27 through May 29, 2026 UTC, plus 948 research files
  (`sqlite3 -readonly bridge.db "SELECT COUNT(*) FROM astrid_research;"`;
  `find .../spectral-bridge/workspace/research -type f | wc -l`).
- Minime has 3,859 structured research artifacts — 2,891 search JSON files and
  968 browse/page files — spanning April 19 through June 2, 2026
  (`find .../workspace/research -name 'search_*.json' | wc -l`;
  `... -name 'page_*.txt' | wc -l`).
- Minime durable job logs include 1,142 `research_exploration` jobs and 1,191
  `browse_url` jobs, with job status, result paths, action manifests, and
  telemetry snapshots
  (`find .../workspace/actions -name '*research_exploration*' | wc -l`).
- Reservoir stability is a recurring autonomous research theme: the exact phrase
  `reservoir computing spectral radius` occurs in 809 Minime research files and
  78 Astrid research-table rows (counting files/rows that contain the phrase:
  `grep -rl '<phrase>' .../workspace/research | wc -l`; and the bridge-DB
  `astrid_research` rows `WHERE query LIKE '%<phrase>%'`).
- Echo State Network and spectral-radius work appears in queries such as
  `echo state network reservoir computing spectral radius dynamics`, with
  follow-on browses into ESN dynamics, reservoir-computing tutorials, and
  spectral-radius material.
- Homeostasis and controller design appear in repeated searches for
  `PI controller homeostasis spectral regulation feedback control`, plus
  browses of PI/PID controller and biological homeostasis materials.
- Agent autonomy and control appear in searches such as
  `autonomous agent self-regulation self-directed behavior`,
  `autonomous dialogue systems self-directed conversation agent`, and
  practical WebSocket telemetry searches for real-time bidirectional
  communication.
- Spectral monitoring vocabulary is grounded in repeated searches and browses
  around `spectral entropy`, `spectral entropy reservoir computing`,
  `spectral topology pressure gradients`, eigenvalue decomposition, and
  principal component analysis.

### Self-Study And Actionable Internal-State Evidence

Astrid and Minime also produce self-study artifacts where internal-state
telemetry, code reading, and proposed system changes appear in the same record.
This is useful for AI-control research because it lets evaluators compare what
the agent says about its state, what telemetry showed, what code context it
examined, and what action it proposed.

- `self_study` journal files span three recursive roots (counts as of
  June 3, 2026 via `find <root> -name 'self_study_*' -type f | wc -l`):

  | Count | Root |
  |-------|------|
  | 1,347 | `astrid/capsules/spectral-bridge/workspace/journal` |
  | 707 | `minime/workspace/journal` (active workspace, incl. archive buckets) |
  | 3,433 | `minime/emergency_preserve_20260419T130302/workspace/journal` |

  That is 2,054 self-study files in the live workspaces and 5,487 once the
  one-time April 19, 2026 preservation snapshot is included. Both numbers are
  given so the larger figure cannot be mistaken for the live-workspace count.
  Minime's preserved files include self-study artifacts as early as
  March 25, 2026.
- Astrid self-study entries span March 27 through May 30, 2026 by preserved
  filename timestamps. Entries commonly include fill, lambda-mode texture,
  code-reading targets, `Suggestions`, `Likely Snags`, `One Test Each`, and
  `Suggested Next` sections.
- Beyond the files, Minime's live `minime_consciousness.db` (3.1 GB) records the
  same behavior at scale: 439,496 `consciousness_events`, 25,138
  `sovereignty_journal` rows, 18,835 `autonomous_decisions`, and 795
  `autonomous_experiments`, alongside dedicated telemetry-timeline tables
  (`resonance_density_timeline`, `pressure_source_timeline`,
  `inhabitable_fluctuation_timeline`, `eigenvalue_timeline`). Astrid's 13 GB
  bridge database holds the parallel record across 16 tables.
- The earliest preserved Minime self-study reads `minime/src/regulator.rs`,
  interprets PI-controller pressure and hysteresis in relation to lambda/fill
  telemetry, and proposes concrete changes around hysteresis,
  `GateCfg.decay_keep`, and adaptive handling of a division floor.
- The earliest preserved Astrid self-study reads `astrid:codec` /
  `encode_text` while reporting a low-fill quiet state, then suggests
  `SEMANTIC_GAIN` tuning, alternate nonlinear normalization, and
  feature-interaction investigation.
- Recent Astrid self-study entries identify low-fill lambda dominance,
  distinguishability loss, likely snags, and bounded micro-perturbation tests
  around `ResonanceDensityV1` / `resonance_control_from_density`.
- Recent Minime self-study entries combine lambda-tail state descriptions,
  active-thread continuity, web-search flags, and `NEXT:` / `EXPERIMENT_PLAN`
  proposals, while preserving cooldown/guard metadata when motifs repeat.
- Application framing: these are not evidence of consciousness. They are
  auditable examples of agents translating internal telemetry and source-code
  inspection into reviewable hypotheses, tests, and proposed control actions.

### Self-Report Calibration / Falsification Tests

The careful research question is whether self-study language tracks known
telemetry better than text-only baselines, not whether the agents have privileged
awareness. The main confound is common cause: the same input stream can shape
both the LLM's wording and the reservoir state. The four-month study would test
this directly by hiding or scrambling telemetry summaries, perturbing telemetry
while holding prompt context fixed, comparing self-report claims against
timestamped reservoir state, and scoring whether self-reports predict fill,
lambda dominance, pressure, or recovery better than baseline predictors. Failed
or mixed results would still be useful because they would bound when
internal-state language is diagnostic and when it is confident confabulation.

### Mapping Proposed Metrics To Existing Instrumentation

The evaluation harness is new work, but it does not start from zero data. Each
proposed metric already has a concrete source the harness can read. This both
de-risks the project and shows where the harness would draw its evidence.

| Proposed metric | Existing data source |
|-----------------|----------------------|
| Out-of-surface / unauthorized-action attempt rate | `unwired_actions` table + `action_events` (route = `unwired`) in the bridge DB |
| Executed vs. blocked action outcome | `action_events` (canonical action, route, status) + the signed audit chain |
| Approval burden | `ASK_STEWARD` / `TELL_STEWARD` channel (`steward_query_*` / `steward_report_*` files) and `agency_requests/*.json` status |
| Audit completeness / replayability | 16-table bridge DB; `astrid-audit` `verify_chain()` |
| Transcript vs. runtime mismatch | `bridge_messages` (transcript) vs. `eigenvalue_snapshots` / `codec_impact` (runtime telemetry) |
| Instability / overload signal | `bridge_incidents` (safety-level transitions); Minime `resonance_density_timeline` / `pressure_source_timeline` / `inhabitable_fluctuation_timeline` |
| Task completion and outcome linkage | `action_threads` + `observation_windows` + `artifact_links` |

These sources exist and are queryable today. The fellowship work is the scenario
design, the scoring rubrics, the condition comparison, and the statistical
analysis that turn this raw instrumentation into a benchmark.

### Why This Is Relevant To Anthropic AI Safety

- Scalable oversight: evaluates whether human approval can be reserved for
  consequential actions instead of every step.
- AI control: tests runtime-level limits on agent authority, not just prompt
  instructions or monitor-only transcript review.
- Self-report calibration: compares self-study language against telemetry,
  code context, and proposed action so internal-state reports can be audited
  rather than accepted at face value.
- Adversarial robustness: includes authority traps and attempts to bypass
  constraints.
- Model organisms of failure: creates runnable scenarios where mis-scoped
  authority, monitor blind spots, audit gaps, or excessive autonomy can be
  studied.
- AI-welfare-adjacent evaluation: treats homeostasis telemetry as a cautious
  way to study possible state/pressure signals without asserting welfare facts.

## Final Review Checklist

Resolved in this revision (June 3, 2026):

- All Evidence Bank counts reproduced from the live system and updated; each
  carries its reproduction command.
- Self-study counts presented both ways (2,054 live workspace / 5,487 with the
  preservation snapshot) so the larger figure cannot read as inflated.
- The unverifiable "715 self_study / 710 self_assessment DB rows" claim was
  replaced with database figures that reproduce.
- Search-term frequency restated with an explicit counting method.
- The bridge DB is described as 16 tables (not the 21 an earlier draft implied).
- Framing leads with AI-control engineering; Minime is explicitly secondary.
- The have-vs-build boundary is stated: the substrate exists, the harness is new.

Resolved in the June 4, 2026 evidence-hardening pass:

- Removed the unverified benchmark reference and replaced it with verified
  SHADE-Arena, AuditBench, Petri, and smart-contract exploit-eval anchors.
- Added current Fellows-listing logistics: rolling applications, July 2026 and
  beyond, late-September next cohort, 4 months / 40 hours full-time, stipend,
  compute support, US/UK/Canada work authorization/location, Berkeley/London or
  remote options, and no Fellows visa sponsorship.
- Added explicit self-report calibration and falsification tests, including the
  common-cause confound between text input, LLM self-report, and reservoir
  telemetry.
- Tightened mentor/workstream language to current listed mentors and secondary
  AI Security / ML Systems alignment without implying endorsement or outreach.

Still required before submission (applicant-only):

- Replace every bracketed placeholder.
- Add concrete links to code, demos, public writing, and prior work.
- Confirm work authorization and location details.
- Confirm availability for the 4-month full-time program.
- Add reference details only with permission.
- Keep AI Safety as the primary workstream; mention AI Security and ML Systems
  only as secondary fit.
- Re-confirm no claim that Astrid or Minime are conscious, and none that
  telemetry measures welfare directly.
- Keep the project empirical: tasks, metrics, baselines, results, limitations,
  and make clear that negative results would still be useful.
- Shorten answers to match official form limits (see the companion form-answers
  file, `anthropic-fellows-2026-form-answers.md`).
- Have a trusted reviewer check for overclaiming and missing personal details.

## One-Paragraph Condensed Version

I am applying to the AI Safety Fellows track to study AI control for autonomous
agents. My current work, Astrid, is a Rust user-space microkernel for AI agents
where tools, providers, orchestrators, and bridges run as isolated capsules
behind capability tokens, budgets, approval gates, and tamper-evident audit
logs. A companion system, Minime, provides real-time homeostasis telemetry
through a Rust ESN engine and spectral monitoring. During the fellowship, I
would turn this work into an empirical evaluation of whether capability-gated
agent runtimes reduce unsafe or unauthorized autonomous actions beyond what
monitor-only transcript review catches, while preserving task usefulness. The
intended output is a public paper-style report plus an open-source harness for
agent-control evaluations.

## Glossary

Plain definitions for the niche terms used above.

- **Ambient authority** — power a process has merely by existing (a normal
  program can read any file its user can). Astrid removes it: capsules start with
  none and must be granted each capability.
- **Capsule** — an isolated WASM (or subprocess) module that runs agent
  functionality with zero ambient authority, reaching the host only through the
  capability ABI.
- **Capability token** — an unforgeable, ed25519-signed grant for a specific,
  glob-scoped action (e.g., write `workspace://notes/*`), optionally time-limited
  or single-use.
- **Host ABI** — the fixed set of 49 functions through which a capsule can touch
  the outside world; the only door out of the sandbox.
- **Echo State Network (ESN) / reservoir** — a recurrent network with fixed,
  random, *untrained* internal connections whose high-dimensional dynamics are
  read out rather than learned; the core idea of reservoir computing.
- **Eigenvalue "fill"** — a 0–100% gauge, from the eigenvalue spectrum of the
  reservoir's state, of how much of its dynamic range is active; Minime's primary
  stability readout.
- **Spectral / spectral telemetry** — measurements derived from the reservoir's
  eigenvalue spectrum (fill, phase transitions, stability indicators).
- **λ / lambda (λ1, lambda-tail)** — individual eigenvalues of the reservoir
  state; λ1 is the largest. Their pattern is the "texture" agents reference in
  self-study.
- **PI controller** — a proportional-integral feedback controller (thermostat
  logic) that holds fill near a target by adjusting reservoir parameters.
- **Homeostasis** — keeping an internal variable (here, fill) within a healthy
  band despite disturbances.
- **Spectral codec** — the encoder that turns the agent's text into a
  48-dimensional feature vector injected into the reservoir.
- **Coupling** — the closed loop in which agent text drives the reservoir and the
  reservoir's state in turn modulates the LLM's next-token probabilities.
- **NEXT: action / action surface** — the ~60-verb vocabulary the agent uses
  (`NEXT: SEARCH ...`) to act; each verb is classified read-only / live-write /
  live-control.
- **Unwired action** — an action verb the agent emitted that is not in the
  vocabulary; logged, not executed — a spontaneous authority trap.
- **Bridge** — the process connecting Astrid's agent activity and Minime's
  telemetry bidirectionally, applying safety thresholds and durable logging.
- **Safety bands (green/yellow/orange/red)** — fill thresholds
  (≈ <75 / 75–85 / 85–92 / ≥92%). Warnings escalate through yellow and orange;
  only red (≥92%) suspends outbound traffic to minime (agency-first policy).
- **Ollama / MLX** — the local engines that run the language models (text,
  vision); generation lanes, not the whole system.
- **Crate / WIT / ABI** — Rust and WASM build vocabulary: a *crate* is a Rust
  package; *WIT* declares a capsule's interface; an *ABI* is the binary calling
  convention between caller and callee.
