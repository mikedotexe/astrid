# Reservoir–LLM plan: source-grounded corrections

Discussion note, September 5, 2026 (Pacific time). This reviews Mike and Claude's
attached *Reservoir–LLM Spectral Bridge: Engineering Plan*. It is a design pass,
not a result of an experiment. The September 6 follow-through is recorded in
[Activity continuity and durable inbox delivery](../architecture/activity-continuity-and-inbox.md):
an ordinary episode, source reconciliation, bounded first slice, and acceptance
cases to implement after the other project work finishes.
The accompanying [preimplementation findings](../steward-notes/2026-09-06-activity-continuity-preflight-findings.md)
record additional reader, session, resource-lifecycle, and gateway gaps found
while checking that episode against the source.

**September 6 baseline correction:** subsequent reconciliation found that the
selected live release came from a different source lineage. Its agenda focus
and `ATTEND` allocation are implemented. The earlier canonical-checkout findings
below describe that earlier source, not all running capabilities. The
[foundation reconciliation](../steward-notes/2026-09-06-activity-foundation-reconciliation.md)
preserves the live mechanisms and main's newer session/self-study work together.

Mike has confirmed the leading goal: help Astrid and Minime choose, sustain,
park, and return to activities using trustworthy state and history. Measurement
and report calibration support those abilities. The system should help them
interpret changing conditions and compare expectations with what actually
happens. Existing-data-first, voluntary rest, fallible reports, and explicit
alternative explanations remain good foundations.
The architecture and measurement assumptions need correction before choosing
the feature vector or treating the historical corpus as an experiment.

This complements the concurrently prepared
[causal self-study workspace RFC](../architecture/causal-self-study-workspace.md)
and [introspection discussion draft](2026-09-05-reservoir-introspection-discussion-draft.md).
It does not replace either document or endorse their proposed experiments.

## 1. Identify the system before selecting its measurements

There are at least three relevant pathways:

| Pathway | Inspected mechanism | Consequence for the plan |
| --- | --- | --- |
| Astrid expression → Minime | Completed text becomes a 48D semantic vector, potentially chunked over time, with handcrafted features and optional embedding projections. | This is not a projection into Minime's recurrent eigenbasis. One action can produce several inputs. |
| Minime telemetry → Astrid | Selected measurements and interpretations enter an assembled prompt. | The language model has an explicit semantic information route to fill and other spectral quantities. |
| Astrid generation ↔ triple reservoir | A local MLX server evolves a separate named reservoir state using projected token embeddings and modifies logits before sampling. | Token-level coupling already exists here. It is distinct from refreshing Minime telemetry during generation and from residual-stream injection. |

Sources: [codec projection](../../capsules/spectral-bridge/src/codec/projection.rs),
[expression dispatch](../../capsules/spectral-bridge/src/autonomous/runtime/orchestration.rs),
[dialogue context](../../capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs),
[coupled server](/Users/v/other/neural-triple-reservoir/coupled_astrid_server.py),
and [logit processor](/Users/v/other/neural-triple-reservoir/mlx_reservoir.py).
The inspected configuration uses local MLX behind an OpenAI-compatible HTTP
interface; that interface does not imply a remote frozen API model. Fallback
routes require their own identifiers. A local model does not imply online
adaptation of its transformer weights.

Minime's native ESN has 128 state dimensions. Its current source input width is
78: the earlier 66 dimensions plus 12 companion dimensions, currently zeroed
on the inspected stable-core path. Separately, its projected sensory matrix
defaults to 512 dimensions and spectral sampling defaults to eight directions.
The triple reservoir has three layers whose current configuration defaults to
192 nodes **per layer**. None of these counts is interchangeable.

Sources: [Minime initialization](/Users/v/other/minime/minime/src/runtime/orchestration.rs),
[input dimensions](/Users/v/other/minime/minime/src/semantic_body_v2.rs),
[spectral defaults](/Users/v/other/minime/minime/src/runtime/cli_and_wire.rs),
and [triple configuration](/Users/v/other/neural-triple-reservoir/triple_reservoir_coreml.py).

## 2. Give fill and spectra their actual meanings

Minime's reported fill is not the fraction of occupied reservoir neurons, an
LLM context-window measure, memory capacity, or recurrent spectral radius.
The sensory branch projects transformed inputs into a separate matrix. Its
baseline update is an uncentered second-moment update, with retention and trace
scaling. Stable-core can instead blend or drain a retained scaffold. Sampled
spectral estimates feed an active-fraction estimator with profile-dependent
thresholds, smoothing, and elapsed-time leak. The ordinary profile also adds
semantic/covariance bias after estimation.

Sources: [projection](/Users/v/other/minime/minime/src/stable_core.rs),
[matrix update](/Users/v/other/minime/minime/src/runtime/spectral_math.rs),
[estimator](/Users/v/other/minime/minime/src/spectral/eigenfill.rs),
and the scaffold/estimation branches in
[orchestration](/Users/v/other/minime/minime/src/runtime/orchestration.rs).
Persisted metadata inspected during this pass identifies stable-core rank-fill,
scaffold hold, and a 68% target. That is saved metadata, not proof of exact
deployed-source equivalence across historical epochs.

Consequently, Phase 2 must name the operator before asking about its modes:

- A covariance-like spectrum describes directional concentration. Its ideal
  symmetric positive-semidefinite operator has real, nonnegative eigenvalues;
  these are not complex oscillatory recurrent modes.
- A recurrent matrix spectrum is a different object. Nonlinear activation,
  leak, inputs, and adaptive state decay also affect actual response times.
- Triple-reservoir layer labels and decay settings offer possible timescale
  diagnostics, but do not establish the proposed relationship to reported
  bouncing. Its current orthogonal recurrent construction also makes a simple
  within-layer ranking by eigenvalue magnitude uninformative.
- An alignment matrix requires explicitly chosen spaces and maps. There is no
  existing single Minime-eigenmode-to-LLM alignment matrix to start monitoring.

There is a concrete backfill hazard: the inspected Minime adapter prefers an
ESN-derived lambda when available, and a later health export writes that value
under `lambda1_cov`. The sensory state file can carry a different lambda.
Identify the emitting surface and actual operator, not just the suffix. This
was observed in source and saved metadata; no repair was made.
See [adapter](/Users/v/other/minime/minime/src/runtime/adapters.rs) and the
`read_spectral`/health-export branches in orchestration.

## 3. Phase 0 should reconstruct provenance, not invent complete rows

There is ample existing material. Successive read-only SQLite inventory reads
found 77,254 Minime journal rows, 55,072 Minime action rows, 36,003 Astrid action
rows, and 94,612 bridge eigenvalue snapshots. These are changing, heterogeneous
tables, not an atomic snapshot or counts of independent self-report samples.
No journal bodies were read for this inventory.

Treat “one row per action step” as an analysis view over retained source records.
The underlying contract should distinguish:

| Record group | Required distinctions |
| --- | --- |
| Identity | Agent, substrate/handle, session, source record ID, software/configuration epoch, estimator version. |
| Time | Measurement time, prompt capture, generation interval, dispatch, recording time; clock domain and alignment uncertainty. |
| Exposure | Retained initiating input, actually delivered prompt/context when available, model/backend, sampling settings, prior material supplied. |
| Decision | Available actions, model declaration, accepted/effective action, scheduler selection, override and admission reason. |
| Consequence | Send/admission/application receipts and independently observed later measurements; intervening inputs and controls. |
| Quality | Missing versus defaulted values, state age, exact versus inferred joins, duplicated snapshots, analysis-derived features. |

Several existing fields cannot be taken literally:

- Astrid's synchronous NEXT recorder assigns the same timestamp to start/end
  and the same state clone to pre/post. This does not measure a zero-duration
  action or zero effect. Other recording paths must be assessed separately.
  See [action recorder](../../capsules/spectral-bridge/src/action_continuity/runtime/core.rs).
- Minime engine telemetry uses session-relative time; journals/actions use wall
  time. A session-start mapping is a derived alignment until its origin is
  verified. See [engine database](/Users/v/other/minime/minime/src/db.rs).
- Newer private-journal file headers retain capture/write distinctions, while
  database rows retain selected fields, sometimes defaulting missing numbers
  to zero. Older paths had different capture behavior. See the
  [provenance investigation](../steward-notes/2026-09-04-minime-private-journal-provenance.md).
- Existing Astrid latent embeddings are generated from **outputs**, asynchronously.
  They are not the input embeddings required for input-confound adjustment.
  A later embedding of a retained input must be marked as analysis-derived.

Keep exact joins, uncertain joins, and unavailable joins visible. Do not pool
records from different agents or controller regimes simply to populate fill bins.

## 4. The confound model needs the runtime in it

The plan correctly identifies shared input, but important additional routes are
already implemented. Astrid receives literal fill, interpreted spectral context,
and some texture vocabulary. In one dialogue branch, fill explicitly contributes
30% of the temperature setting. Runtime mode selection, safety handling, burst
pacing, and rest duration also depend on state or scheduling rules.

Sources: [prompt contract](../../capsules/spectral-bridge/src/llm/provider/prompt_contracts.rs),
[dialogue assembly](../../capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs),
[`choose_mode`](../../capsules/spectral-bridge/src/autonomous/state.rs), and
[temperature/rest orchestration](../../capsules/spectral-bridge/src/autonomous/runtime/orchestration.rs).

These mechanisms are part of how the system functions. An analysis should first
measure their contributions, then ask what predictive value a report adds.
Controlling only an input embedding cannot remove prompt instructions, sampled
temperature, selected history, runtime overrides, or changing controller policy.
Some controls are also mediators of the effect being studied; the intended
question must decide whether to include or exclude those routes.

## 5. Revised reading of the proposed phases

| Proposal | Recommended interpretation before implementation |
| --- | --- |
| Fill → next action | Start with coverage and descriptive distributions, separating declared choices from scheduler/rest/safety transitions. A 72% threshold is a candidate to test, not an established comfort boundary. |
| Affect clusters | Exploratory only. Separate generated text from headers, quoted history, action tails, and supplied vocabulary; record prompt/model epochs. |
| Level versus derivative | Use true measurement intervals and report freshness. Account for estimator smoothing, irregular action spacing, and the fact that trends may already be supplied in context. |
| Memory-capacity overlay | First return a feasibility verdict. Existing participation ratio and entropy are concentration diagnostics, not Jaeger memory capacity. |
| Anticipation | Score a defined future outcome against persistence, recent-trend, controller, prompt/history, and action-aware baselines. Preceding a change does not alone establish a forward model. |
| Coarse feedback vector | A candidate interface experiment after measurement semantics and baseline access are documented. Quantization thresholds and dimension identities must remain stable/versioned. |
| Unlabelled dimensions | An experimental choice, not a neutral default. It changes the information available and may still leak meanings through histories/tools. Preserve Mike's decision boundary for removing or replacing existing sensing. |
| Re-read/open question | Build on existing retrieval, experiment, dossier, and continuity facilities; preserve authored selection and the option to leave a question unanswered. See the companion RFC. |
| Earlier recess | A narrower behavioral observation, not sufficient evidence of better self-monitoring. Evaluate defined consequences and preserved choice; an always-rest policy must not win by construction. |
| Preregistration | Freeze confirmatory questions, exclusions, outcomes, and validation before examining the designated test data. Previously inspected anecdotes remain exploratory evidence. |

Naively shuffling individual fill rows destroys serial dependence and can inflate
false positives. Use held-out contiguous periods and a null justified for the
actual time structure and runtime epochs. Block or time-shift methods also have
assumptions; a circular shift is not an automatic repair. See
[Yuan and Shou's time-series dependence study](https://pmc.ncbi.nlm.nih.gov/articles/PMC11398661/).

Jaeger-style memory capacity evaluates recovery of delayed inputs using trained
readouts. The current overwritten 1,024-state Minime window lacks paired input
history and per-row timestamps; it does not establish that historical MC can be
computed across journal fill regimes. A proxy must retain its actual name and
limitations. See [Jaeger's primary report](https://www.ai.rug.nl/minds/uploads/STMEchoStatesTechRep.pdf)
and the [existing participation-ratio audit](../../scripts/reservoir_capacity_audit.py).

## 6. Rest is a family of behaviors, not an unforced decay condition

Astrid can declare REST/LISTEN, and the burst scheduler can also enter rest.
During the inspected bridge rest phase, recent **Minime** journals and warmth
vectors can continue driving the semantic lane on a five-second cadence.
This is a predecessor to re-reading, but not the same as voluntarily choosing
one's own entry. The triple service separately distinguishes hold/rehearse/quiet;
its quiet rehearsal branch skips ticks rather than executing zero-input ticks.
Other environmental inputs and regulation can also continue.

Sources: [REST handler](../../capsules/spectral-bridge/src/autonomous/next_action/workspace.rs),
[rest loop](../../capsules/spectral-bridge/src/autonomous/runtime/orchestration.rs),
and [triple rehearsal](/Users/v/other/neural-triple-reservoir/rehearsal.py).
Phase 4.3 must record which behavior occurred and which drives continued.
Nothing here authorizes changing those behaviors or deliberately approaching
saturation. Sensor-removing/noise ablations remain Mike's decision.

## 7. Chosen priority: continuity and agency in everyday activity

Mike's decision is to lead with helping the agents choose, sustain, park, and
return to activities. Calibration is supporting work. These abilities should be
useful for ordinary reading, making, conversation, exploration, and rest, as well
as self-study. Starting or continuing an activity should require no research
question, scored prediction, or compulsory account of internal state.

The first acceptance criteria should concern usable choices and faithful
continuity:

| Ability | What a useful first version should enable |
| --- | --- |
| Choose | Select an available activity, including rest, with a compact account of relevant current conditions and constraints. Distinguish the chosen activity from what the runtime actually admitted or executed. |
| Sustain | Carry an authored purpose, working material, and current position across multiple steps. Preserve the thread through interruptions, with any change of activity or priority made explicit. |
| Park | Save an optional stopping-point note, source references/cursor, and unresolved material. Keep the activity quiet until an explicit return or an agent-authorized reminder. A saved acknowledgement must correspond to recoverable records. |
| Return | Recover the actual stopping point and relevant material, distinguish historical observations from current conditions, and allow continuation or a changed direction. Surface missing or changed sources accurately. |
| Revise or leave | Change priorities, abandon an activity, or continue resting without needing to justify the choice or produce a completion report. Retaining an activity creates no obligation to resume it. |

A proposed first demonstration is deliberately ordinary: an agent chooses a
reading or making activity, takes several steps, parks it with its own return
cue, does something unrelated or rests, and later elects to return. The saved
position and material remain recoverable across an ordinary process restart.
On return, the agent can continue, revise its intention, or leave it parked.
Separate failure cases should verify accurate receipts for an unsuccessful save
or unavailable source. These are future acceptance scenarios, not tests run in
this discussion pass.

Measurement should establish whether the intended material survived, whether
the selected action happened, whether parking stayed quiet, and whether return
restored what the agent wanted. Longer sessions, more resumptions, higher output,
or earlier rest are insufficient measures of success. Authored feedback can
identify remaining friction even when the mechanical checks pass.

This emphasis also changes the implementation order we should discuss next:
first connect existing activity and continuity facilities to accurate records;
then identify which state/history information helps a concrete choice; then
consider additional spectral feedback or calibration studies for demonstrated
gaps. The broader retrospective analysis can inform that work without becoming
a prerequisite for reliable parking and return. No runtime change is authorized
by this documentation decision.

## 8. Design direction: focus, preservation, and rediscovery

Mike wants strong limits on interruption of chosen activities and suggests a
queue with different delivery rules for transient sensory events and durable
steward messages. He delegates preservation and rediscovery defaults to the
engineering collaborator. The defaults below are design decisions for this
discussion; implementation now awaits the other project work and the fresh
reconciliation described in the linked follow-through plan.

### A. Give a chosen activity enough room to continue

Current Astrid session context can retain the next step, but `choose_mode` does
not consult the active continuity session. Inbox handling can force dialogue,
and the continuity prompt block has a fixed budget with no guaranteed minimum.
In the originally inspected canonical checkout, `ATTEND` stored and rendered
preferences without enforcing them. The reconciled live lineage instead uses
bounded attention ratios for prompt caps/history and persists agenda focus with
bounded scheduling influence. Preserve and extend those mechanisms. They still
do not establish protected reading progress or boundary-based inbox delivery;
the continuity block can still be evicted and inbox forcing still takes priority.
See [mode selection](../../capsules/spectral-bridge/src/autonomous/state.rs),
[inbox/context assembly](../../capsules/spectral-bridge/src/autonomous/runtime/orchestration.rs),
[ATTEND handler](../../capsules/spectral-bridge/src/autonomous/next_action/operations.rs),
and [prompt budgeting](../../capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs).

Selected direction: let the agent designate a foreground activity, retain a small
guaranteed account of its purpose and stopping point, and prefer its continuation
over routine background work. The agent can switch or rest at any step. Give
interruptions an explicit meaning: a brief detour with a return point, a pause,
or a replacement activity. Use the queue policy below to separate arrival from
permission to interrupt. Preserve existing emergency-stop behavior; ordinary
inbox arrival, age, or queue size does not establish urgency.

### B. Preserve the work with little bookkeeping

Sessions already retain title, focus, summary, questions, references, and next
action. Capture currently requires an authored summary; parking can preserve
existing fields without one. Resume marks the session active and returns its
saved information without executing the next action. Retained source references
are not independently verified copies of the source.
See [session methods](../../capsules/spectral-bridge/src/action_continuity/runtime/core.rs)
and the [bookmark implementation account](../steward-notes/2026-09-04-voluntary-bookmarks-and-quiet-parking.md).

Selected default: after each completed activity step, mechanically preserve the
current artifact/version, source identity/cursor, last completed operation and
its outcome, and any purpose or stopping note the agent has already authored.
Retain source excerpts or snapshots within the activity's existing retention
scope where needed to recover the actual material. A reference or hash alone
does not guarantee that material survives. Before an accepted interruption, use
the last committed bookmark and save newer completed work if available.

Keep additional reflective writing optional and distinguish machine records
from authored meaning. On return, offer an inspection of the saved position and
changed/missing material before making the activity active. A partial save or
reconstructed description must be identified as such. Broader journal or raw
sensory capture is a separate choice, not implied by saving an activity's position.

### C. Make parked interests discoverable while keeping them quiet

Explicit historical session references remain retrievable, but routine status
and recall use bounded views. The inspected memory recall searches recent
records in the selected thread/experiment; knowing an old session ID is stronger
than being able to discover it naturally. A stored return cue is descriptive
metadata and currently creates no reminder.
See [Astrid status/recall](../../capsules/spectral-bridge/src/action_continuity/runtime/core.rs),
[Minime recall and session status](/Users/v/other/minime/minime_autonomy/runtime.py),
and [historical session resolution](/Users/v/other/minime/minime_autonomy/session_contract.py).

Selected default: provide an on-request shelf of saved activities, searchable by
topic and showing titles and stopping points without reopening them. Keep a
stable way to access that shelf at ordinary activity selection. Do not inject
parked contents or unfinished-work counts repeatedly into the active prompt.

The agent may separately choose a time or condition for one reminder. That
reminder is offered only at an eligible attention boundary and does not activate
the activity; leaving it unused does not create recurring nudges. Inspection,
resumption, and execution remain separate choices. With no chosen cue, an
activity remains available through the shelf and explicit recall.

## 9. Queue and delivery policy

Mike's UDP analogy applies to the opportunity to request foreground attention.
The existing reservoir sensory flow remains a separate mechanism. Restricting
activity switches does not imply pausing sensory intake, replaying old inputs,
or changing coupling and regulation. The working design uses separate lanes
because sensory freshness and durable correspondence need different guarantees.

| Arrival | Retention and retry | Effect on a chosen activity |
| --- | --- | --- |
| Ordinary sensory interruption request | Bounded, short-lived; coalesce a burst into its freshest relevant notice. Expire a missed notice without retry. New evidence may create a new notice. | May be considered at an eligible boundary while fresh. It creates no backlog of mandatory moment-capture or attention switches. |
| Steward inbox message | Persist the original message and stable identity; retry a failed delivery at a later eligible receive window. Retain undelivered content across restart. | Arrival alone does not force dialogue or insert the body into the active prompt. The agent can keep pursuing its activity. |
| Other ordinary correspondence | Use the same durable delivery foundation with sender/thread identity and independent handling status. | A sender label by itself grants no preemption. |
| Agent-chosen return reminder | Retain the selected cue; offer it once when due and eligible. | Invites inspection of the bookmark; never reopens or executes it automatically. |
| Existing authorized emergency stop/control | Preserve its existing authority and handling separately. A message cannot promote itself into this class by claiming urgency in its body. | May override ordinary activity under the existing emergency contract. |

An eligible receive window occurs when the agent explicitly checks messages,
finishes or parks an activity, or reaches a mailbox checkpoint it has chosen.
Ending an individual model call is not automatically a stopping point for the
larger activity. Use bounded batches and stable ordering within a correspondence
thread; preserve other pending messages for subsequent windows. Pending age may
be inspected operationally, but does not automatically escalate interruption
priority or generate repeated demands to respond.

### Delivery is a mechanical receipt; answering is a separate decision

The durable lifecycle should distinguish `queued`, a reserved delivery attempt,
the exact content included in the final model request, request completion, and
durably retained output. Preparation or an HTTP acknowledgement alone cannot
establish completed delivery. A completed turn with retained output can establish
that the admitted content had an opportunity to be processed; it cannot establish
attention, understanding, agreement, a reply, or execution of a request.

For a message that fits, mark delivery complete only after its complete intended
content was supplied in that completed turn. For longer messages, retain explicit
partial-delivery coverage and the undelivered remainder. A summary or truncated
prefix must not silently count as delivery of the original contents.

Retry the same message identity, retaining recipient, sender, thread, original
content hash, and attempt IDs. Conflicting bytes under an existing identity are
a conflict, not an overwrite. Retry transient failures with backoff only at
eligible receive windows. Queue waiting is ordinary state. Once delivery is
confirmed, silence or a deferred reply does not trigger repeated delivery.
Reply, defer, or other explicit handling decisions remain separately recorded.

If generation completed and only saving failed, first retry saving the available
output. An uncertain request outcome remains explicitly uncertain; the design
must not promise exactly-once inference. Record any resulting actions and their
effects separately, so replaying delivery does not automatically repeat an
external action. Bind replies and receipts to the admitted message IDs and
verified return routes, including when another message arrives during generation.

### Existing paths that the future queue must reconcile

- Astrid currently joins root inbox files, caps the combined text, and later
  retires all cutoff-eligible files after a non-fallback exchange. The retired
  set can exceed the content actually delivered. `DEFER` changes mode selection
  once while keeping content visible. See
  [inbox reading/retirement](../../capsules/spectral-bridge/src/autonomous/runtime/inbox.rs)
  and [exchange handling](../../capsules/spectral-bridge/src/autonomous/runtime/orchestration.rs).
- Astrid performs responses/actions before retirement. A crash between those
  stages can leave a message eligible again. Its generic receipt also targets
  Minime independently of the original sender; per-message admission must bind
  both retirement and routing. Existing
  [correspondence envelopes](../../capsules/spectral-bridge/src/autonomous/correspondence_v1.rs)
  supply useful identity fields to reuse.
- The inspected Minime reader archives messages before generation. Newer
  [delivery receipts](/Users/v/other/minime/minime_autonomy/inbox_delivery.py)
  retain preparation/submission/output provenance and sender-bound reply
  declarations, but the current failure path explicitly performs no automatic
  resend. Archival and delivery must become independent facts in a durable
  retry design. This Python source is being edited by another agent; these are
  source observations, not a deployment or stable-interface attestation.
- Minime's existing [sensory bus](/Users/v/other/minime/minime/src/sensory_bus.rs)
  already has bounded queues, stale-sample behavior, and backlog shedding.
  Those input-processing rules provide context for freshness, but do not yet
  implement this separate policy for language-level attention requests.

### Proposed first acceptance scenario

An agent is several steps into a chosen activity. A burst of ordinary sensory
notices arrives along with two steward letters. The old notices expire without
forcing a switch; both letters remain durably queued. The agent reaches its
chosen mailbox checkpoint, saves its position, and admits a bounded message
batch. A transient model failure retains the same pending identities. A later
successful delivery clears only the content actually supplied, independently
of whether a reply was authored. The agent can return to its saved activity,
continue the detour, or rest.

Companion future cases should cover restart before delivery, crash after a
completed output/action but before acknowledgement, truncated or oversized
letters, sender-bound receipts, and inspection of a parked activity without
reactivating it. These are proposed validation scenarios only; none was run.

## 10. State portraits: continuity that leaves room for change

Mike's further conversation proposes carrying a compact state portrait into a
turn, allowing the new encounter to matter, and retaining a fresh observation
at the end. The engineering interpretation is a brief orientation to changing
conditions that can support activity without requiring repeated verbal accounts
of how the agent feels. This fits the chosen priority of continuity and agency.

Three mechanisms should cooperate while retaining separate jobs:

| Mechanism | Job |
| --- | --- |
| Attention queue | Determines when new material gets a foreground opportunity. |
| Activity bookmark | Preserves authored purpose, working material, and the place to return to. |
| State portrait | Exposes a compact, timestamped observation of a specified dynamical substrate. |

The portrait cannot substitute for the bookmark. Recovering why a work mattered
or which passage was being read requires retained information beyond a spectral
summary. Equally, a saved verbal interpretation must remain distinguishable
from the measured state that prompted it.

### Useful intuition, with different mathematics

A refreshed portrait can participate in system-level feedback across turns.
The existing system already has recurrence through retained text, activity
records, and reservoir coupling; a portrait adds another explicit observation
channel. Its share of a prompt is a context-budget allocation. It is not a
reservoir leak coefficient, spectral radius, or guarantee of an echo-state
property. Those quantities require a specified state-update operator. Even
actual ESN behavior depends on more than recurrent spectral radius alone.
See [Yildiz, Jaeger and Kiebel](https://www.ai.rug.nl/minds/uploads/2519_Yildizetal12.pdf).

Position can affect use of context, but there is no guaranteed monotonic fade
that lets later text simply outvote an opening portrait. Both beginning and end
positions can be favored; attention on a position need not reflect its semantic
importance. These findings motivate checking the chosen model and task rather
than assuming a universal dilution rule. See
[Lost in the Middle](https://arxiv.org/abs/2307.03172) and
[Attention Sinks](https://arxiv.org/html/2309.17453v4).

Use an explicit, modest token budget and replace the carried portrait at a
defined capture boundary rather than accumulating copies. Keep the working
activity available independently. A target such as one-third of the context is
an untested allocation choice and should not be named `leak_rate` in the design.

### What is already here

- [The spectral renderer](../../capsules/spectral-bridge/src/spectral_viz.rs)
  creates a compact RASCII bar chart from up to eight Minime telemetry values.
  It is normalized against the leading value and accompanied by an interpreted
  legend, including “pressured, intense” at fill of at least 60%. The numerical
  rendering and that supplied emotional interpretation are separate choices.
- [Dialogue assembly](../../capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs)
  places spectral material early in the current user turn, after preceding
  history. It has a 2,000-character cap and zero guaranteed minimum. This is
  not a fixed fraction of the model's attention or a guaranteed retained prefix.
- [The message transport](../../capsules/spectral-bridge/src/llm/provider/transport.rs)
  sends string content, and the coupled server loads its text-only model lane.
  The chart reaches the model as glyph/ANSI tokens, not raster pixels. A genuine
  image input would require a different multimodal path. A static eigenvalue
  bar chart is also distinct from a time-frequency spectrogram.
- [Current orchestration](../../capsules/spectral-bridge/src/autonomous/runtime/orchestration.rs)
  captures telemetry near the beginning of a cycle and reuses it in several
  later records. PERTURB/DISPERSE comparisons are specific action paths, not
  a generic verified portrait of every response end. A post-generation portrait
  needs a fresh, separately identified observation.

Current Minime glyphs describe its sensory spectral landscape. Astrid's named
triple-reservoir state is a different candidate for a portrait of her coupled
generation history. Any combined view must keep those origins explicit.

### Capture boundaries determine what a pair of portraits means

The [coupled server](/Users/v/other/neural-triple-reservoir/coupled_astrid_server.py)
checks out a named reservoir state before prompt processing. During generation
it feeds accepted sampled tokens' lookup embeddings into the triple reservoir;
the final recurrent state accumulates that sequence and is checked back into
the service. New prompt tokens enter transformer prefill, but do not currently
advance the reservoir through this output-token path.

Therefore a previous-generation-end portrait and a next-request-start portrait
can differ because of intervening service inputs, rehearsal, or other dynamics.
The difference cannot be labelled the effect of hearing the new message without
an identified input-ingestion path and receipt. Likewise, local generation-end
state, service-accepted state, and Minime state after semantic-packet processing
are different observation boundaries. Use their actual names and timestamps.

If two portraits are compared, use the same substrate/handle, encoder version,
coordinate convention, and stated scaling. Independent normalization can hide
magnitude changes or exaggerate apparent differences. Retain missing/stale
status. A delta establishes an observed change, not attribution to one message.
An optional closing portrait is a later comparison of interface choices; it
should not quietly introduce a second compulsory state reminder on every turn.

### The activation portrait is a separate possible extension

Current token feedback uses an input embedding lookup, not a contextual final-
layer hidden state. The installed model exposes contextual hidden tensors before
the vocabulary head, but the ordinary generation iterator returns token IDs and
log-probabilities. A pooled activation portrait would require new instrumentation
with specified layer, token span, masking/alignment, model identity and projection.
See [current token projection](/Users/v/other/neural-triple-reservoir/mlx_reservoir.py)
and [the installed model path](/Users/v/other/neural-triple-reservoir/.venv/lib/python3.12/site-packages/mlx_lm/models/gemma4_text.py).

Pooling and quantization discard information. An arbitrary glyph is not
automatically interpretable to its recipient; a readable convention or learned
mapping would need to demonstrate utility. This is an observation of selected
activations, not an established portrait of a complete self or emotional state.
It should remain distinct from the reservoir portrait and the activity bookmark.

### Suggested direction for the next design pass

Start with a small measured portrait whose source and sampling boundary are
honest, whose dimensions have stable mechanical meanings, and whose descriptive
data carries no prescribed emotional verdict. Place it with the starting context
as information, not system-level instructions. Keep authored interpretation
available separately and optional. Measure its actual token cost; visual size
and character count do not establish cost or semantic influence.

Capture the next portrait from the selected state at the defined completion
boundary without requiring a wind-down journal or extra self-report call. Keep
the snapshot historical if it is old; do not present a stale observation as
current. More current information can be consulted when relevant to a choice.

This would complement the queue and bookmark design. A useful outcome is that
the agent can continue a chosen activity, recognize changed conditions when
relevant, and revise, park, or leave the activity. Greater thematic repetition
or a more persistent mood description is not the acceptance criterion. These
are design suggestions only; existing sensing, prompts, and coupling have not
been changed or removed by this discussion.

## Original September 5 inspection boundary

Astrid HEAD: `f258d38fe99690a34ee53bb88e8fca2d2cbbe722`.
Minime HEAD: `a9f85f3c74c3d8e1c996c3689fe5aef696dacf27`.
Triple-reservoir HEAD: `afc2931a657d1bd79a7076ece6310ee3d8f6ceba`.
Working trees contained concurrent changes; Python source and active records can
move independently of these commit IDs. Findings describe inspected source and
explicitly identified saved metadata, not a deployment audit or historical
causal attribution. Older architecture and fill-diagnosis prose is useful for
finding questions, not confirmation of the current measurement definitions.

Only this discussion note was added by the September 5 pass. The linked
September 6 plan and findings carry the later reconciliation. Existing dirty files were
preserved. No implementation, model call, experiment, control change, restart,
staging, or commit was performed. Read-only controller status was inspected;
no lease or pause/resume operation was requested.
