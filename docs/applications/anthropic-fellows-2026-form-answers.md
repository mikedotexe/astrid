# Anthropic Fellows 2026 — Form-Ready Condensed Answers

Companion to `anthropic-fellows-2026-application-packet.md`. These are trimmed,
copy-paste-ready answers for the official application form. Word targets are
conservative defaults — adjust to the real field limits when they are known.
Every number here is reproduced from the live system as of June 3, 2026; the
packet holds the reproduction commands and the full evidence base.

Replace bracketed placeholders (`[ ... ]`) before submitting.

---

## Motivation — why the Fellows program (~150 words)

I want to turn AI control for autonomous agents into a measurable systems problem
— and I have spent months building and running a system that now raises questions
I cannot fully answer alone. As agents get real tools, memory, and long time
horizons, the safety question is no longer whether a model gives a safe answer in
a chat window; it is whether an agent can be given authority and still stay
monitorable, auditable, and bounded by human approval. Recent Anthropic
evaluations show that agentic monitoring is hard; my project adds runtime ground
truth beyond the transcript.

Astrid, an existing 23-crate Rust microkernel, runs agent capabilities as
isolated capsules with zero ambient authority behind capability tokens, budgets,
approval gates, and a tamper-evident audit log. Minime adds auxiliary, out-of-band
telemetry. The two have run autonomously for months, accumulating thousands of
journal entries and telemetry records. I want dedicated time and mentorship for
two things: to convert this into a rigorous control evaluation with honest
baselines, and to study, empirically, a recurring pattern — the agents'
self-reports appear to track their own internal telemetry and propose real code
changes, and I want to test whether that is signal or confident confabulation
using falsifiable telemetry-hiding and perturbation tests.

---

## Workstream preference (~70 words)

Primary: **AI Safety Fellows** — AI control, runtime monitoring, scalable
oversight, and model organisms of agent failure. Secondary: **AI Security
Fellows** (Astrid is sandboxing, authorization, and auditability for tool-using
agents) and **ML Systems & Performance** (Rust systems engineering, WASM
isolation, local-model infrastructure, and experiment harnesses).

---

## Technical background (~210 words)

My strongest contribution is building systems where safety properties live in
the architecture, not in prompts.

Astrid is a Rust 2024 workspace: 23 crates, ~95k lines, 1,468 tests (MSRV 1.94,
`#![deny(unsafe_code)]` and pedantic Clippy with denied arithmetic overflow
enforced workspace-wide). A native microkernel owns every privileged resource;
agent functionality runs in isolated capsules with zero ambient authority that
reach the host only through a fixed 49-function capability ABI. Every sensitive
action passes, in order, a five-layer security path — policy hard-block, ed25519
capability token, atomic budget, human approval, and a BLAKE3-chained,
ed25519-signed audit log with `verify_chain()` tamper detection. Roughly 284
tests cover the four security-critical crates.

Minime is a secondary, real-time Echo State Network engine — a fixed, untrained
recurrent network whose internal dynamics are monitored rather than learned
(Rust), with a Python LLM/vision frontend. It reports "eigenvalue fill," a
0–100% gauge of how active those dynamics are, and uses thermostat-style feedback
to hold it near a healthy target. An Astrid–Minime bridge encodes the agent's
text into a 48-number feature vector that drives the reservoir, then reads the
reservoir's spectral state back as out-of-band telemetry, under explicit safety
thresholds. Both systems have run autonomously for months, producing queryable
research traces and self-study artifacts.

[Add: Python/ML experience; Rust systems experience; open-source links;
prior research, security, or systems work; publications, talks, or writing.]

---

## Research project (~250 words)

**Auditable Agent Substrates for Safe Autonomous AI.** I would evaluate whether
agents operating inside a capability-gated, auditable runtime take fewer unsafe
or unauthorized autonomous actions than agents on conventional tool-access
harnesses with monitor-only transcript review — without destroying task
usefulness.

I would build an evaluation harness with five conditions: broad tool access,
monitor-only protocol, policy-only blocking, Astrid-style runtime control
(policy, scoped capability tokens, budgets, approvals, signed audit log), and a
hybrid monitor-plus-runtime protocol. Inspired by SHADE-Arena and AuditBench,
task suites would require useful autonomy but contain authority traps, hidden
side tasks, covertly unsafe actions, suspicion-score probes, and monitor
blind spots. Each run is instrumented for unauthorized-action rate, side-task
completion and evasion, task completion, approval burden, audit completeness,
transcript/runtime mismatch, and recovery from blocked actions. Minime-style
telemetry is an auxiliary instability and self-report-calibration signal, not a
welfare claim.

The substrate already exists and is queryable: the capability-gated runtime, the
signed audit chain, a 13 GB 16-table action/telemetry database, roughly 191k
journal files across active/preserved roots, ~2,000 Astrid and ~3,800 Minime
autonomous-research traces, and a 60+ verb action surface — the agent ends each
turn with a line like `NEXT: SEARCH ...`, which the runtime classifies read-only
/ live-write / live-control and either executes or, if the verb is outside the
granted vocabulary, records in an `unwired_actions` table (a real,
already-accumulating authority-trap dataset). What is genuinely new — and what
mentorship would shape — is the harness: scenario runner, metric collector,
authority-trap suite, condition-comparison framework, and
transcript-versus-runtime scorer.

Output: a paper-style empirical report plus an open-source evaluation harness,
written to be useful even if results are negative (e.g., if gating reduces
unauthorized actions but raises approval burden, or self-report telemetry
correlations disappear under perturbation, that tradeoff or failure is itself a
result).

---

## Research question (~50 words)

Can a capability-gated, auditable agent runtime reduce unsafe or unauthorized
autonomous actions beyond what monitor-only transcript review catches, while
preserving useful task completion — and can auxiliary state telemetry improve
evaluation of when human or automated intervention is needed?

---

## Expected public output (~70 words)

A paper-style empirical report and an open-source harness: a runnable benchmark
for autonomous-agent control, authority-boundary and hidden-side-task scenarios,
monitor-scoring and transcript/runtime mismatch analyses, audit-log analysis
utilities, falsifiable self-report calibration tests, a cautious telemetry
discussion, and clear limitations and negative results. Task definitions and
analysis code would be usable independent of Astrid internals.

---

## Logistics (applicant-only)

- Current listing note: rolling applications for July 2026 and beyond; next
  cohort expected late September; 4 months full-time / 40 hours per week, with
  possible extension; weekly stipend and compute support listed; US/UK/Canada
  authorization and location required; Berkeley/London workspace or remote
  options; no Fellows visa sponsorship.
- Location: [US / UK / Canada location]
- Work authorization: [full-time work authorization status]
- Berkeley or London workspace: [full-time / part-time / remote]
- Availability: [available for 4 months full-time starting ...]
- Start-date constraints: [none / details]
- Links: [GitHub, demos, writing, prior work]
- References: [names, roles, emails, relationship — with permission]
