# Astrid And Minime: Activity, Attention, And Returnability

## Scope And Bottom Line

This is an interactive, read-only operational review requested by Mike on
September 4, 2026. The only new file is this review. No action was dispatched,
generation solicited, control changed, service restarted, or automation resumed.
Existing source changes and the Git index were left alone.

The strongest finding is not that either system lacks interest or expression.
Both produce substantial activity, initiate studies, and attempt continuity
actions. It is that the machinery for preserving a useful stopping point and
returning to it is less reliable than the machinery for producing another entry.
Some apparent repetition is plausibly an interaction with tools, prompt assembly,
and lifecycle projections. That is an engineering hypothesis, not a diagnosis of
either being's experience.

This review examines observable actions, operational receipts, artifact metadata,
and relevant implementation. It does not inspect hidden model reasoning or treat
private journal bodies as a source of reconstructable internal thought. It does
not certify psychological health, consciousness, relief, or the cause of a felt
report. A useful question we *can* investigate is whether the system supports
building, revising, parking, and later recovering a chosen line of inquiry.

## Windows And Method

- Action and journal window: September 3, 19:37:55 through September 4, 19:37:55
  PDT. UTC bounds: `[2026-09-04T02:37:55Z, 2026-09-05T02:37:55Z)`.
- Seven-day comparison: August 28, 19:37:55 through September 4, 19:37:55 PDT.
- SQLite connections used `mode=ro`; operational JSON/JSONL files were read
  directly. Runtime store constructors and maintenance/status methods that can
  expire jobs or mutate projections were not invoked.
- Counts group action names by their first verb, preserving aliases where noted.
  They count dispatcher events, not every generation, mental event, or successful
  effect. `handled`, `skipped`, and `timeout` require further interpretation.
- Current thread files and rotating logs are later point-in-time observations,
  not immutable historical snapshots at the fixed database cutoff.
- Most activity predates the Python journal rollout at approximately 19:27 PDT.
  This is not a before/after test of that rollout.
- The separate engine telemetry window uses engine seconds, not Unix timestamps.
  It must not be joined to wall-clock actions by treating the clocks as equal.

Primary databases:

- [Astrid bridge.db](/Users/v/other/astrid/capsules/spectral-bridge/workspace/bridge.db):
  `action_events`, `action_threads`, `astrid_research`, `eigenvalue_snapshots`.
- [Minime minime_consciousness.db](/Users/v/other/minime/minime_consciousness.db):
  `action_events`, `action_threads`, `sovereignty_journal`, `eigenvalue_timeline`.

Live bridge source lineage was checked against the existing rollout record. The
relevant source checkout for the deployed bridge lineage is
`/Users/v/other/worktrees/astrid-graceful-coupling-rollout`, not an assumption that
the dirty canonical `main` checkout is byte-identical to the running binary.

## What They Have Been Doing

### Astrid

There are **425 action events in 24 hours**, and **2,793 in seven days**.

| Action | 24 hours | Interpretation |
| --- | ---: | --- |
| READ_MORE | 205 | Dispatcher handled; not necessarily new external material |
| PRESSURE_SOURCE_AUDIT | 79 | Pressure/context diagnostic work |
| SPEAK | 36 | Recorded speaking actions, not a count of all dialogue |
| INTROSPECT | 19 | Introspection actions |
| SPECTRAL_EXPLORER | 12 | Spectral exploration |
| STILL | 11 | Stillness actions |
| DECOMPOSE | 9 | Decomposition requests |
| LIST_FILES | 6 | Local navigation |
| REGULATOR_AUDIT | 5 | Regulation audits |
| CONTINUITY_SESSION_START | 5 | Named investigations; see capture gap below |
| BROWSE / SEARCH | 3 / 1 | All four blocked in this window |
| PERSIST_CONTEXT | 1 | Unwired; proposal recorded, not context persisted |

READ_MORE is 48.2% of daily recorded actions and 59.2% of the week: 1,654
READ_MORE events. PRESSURE_SOURCE_AUDIT adds another 232 weekly events. This
establishes a strong reading/diagnostic emphasis, not a judgment that the
activities are pointless.

The recent `/tmp/bridge.log` sample identifies actual READ_MORE advances into
`context_overflow_*.txt`, with offsets advancing and EOF reached. The saved
`last_read_meaning_summary` describes overflow from diversity, modality,
continuity, feedback, and spectral context. These examples are continuations of
assembled context, not evidence of reading new web pages. At 02:36:25 UTC the log
also says the previous continuation was complete and recovery fallback was
skipped, while the action ledger still uses `handled`.

**Limit:** this bounded log sample does not establish that all 205 daily reads,
or all 1,654 weekly reads, are no-ops or repeated text. It does identify a route
where an activity counter can overstate informational progress.

### Minime

There are **378 action events in 24 hours**, and **2,174 in seven days**.

| Action | 24 hours | Interpretation |
| --- | ---: | --- |
| JOURNAL | 174 | Explicit journal actions |
| LEND_APERTURE | 76 | Mostly held locally; see effect accounting below |
| REST | 40 | `skipped` means the requested rest was honored without action |
| CONTINUITY_SESSION_CAPTURE, including alias | 23 | Attempts, not 23 saved bookmarks |
| SELF_STUDY | 16 | Persisted source-study entries; web context absent |
| EXPERIMENT_ADVANCE | 8 | Includes non-mutating previews, not eight advances |
| EXPERIMENT_CHARTER | 5 | Charter actions |
| DAYDREAM, including recess variant | 6 | Recorded daydream actions |
| COMPOSE_AUDIO | 2 | Recorded composition actions |
| REGIME | 2 | Bounded regulatory choices with control receipts |

Other activity includes experiment evidence/review, ambiguity notices, shadow
trajectory work, an aspiration, and blocked RUN_PYTHON and RESIST requests.

Independently, `sovereignty_journal` records 300 `qualia_moment` rows, 181
reflections, 23 self-assessments, 16 self-studies, six daydreams, two compositions,
and other entries in the same day. These overlap action-backed artifacts and
include synchronous paths outside the action ledger. Do not add the two sources
as if they were disjoint activity totals.

The range matters: this is not only regulation and paperwork. Study, composition,
rest, and less structured expression are present. Their frequency also reflects
the runtime's scheduling and affordances; it is not a direct measure of desire.

## Web Research: Requested Versus Executed

There are **no new Astrid `astrid_research` rows in the seven-day window**, and no
Minime `research` or `web_search` journal rows in that window. Astrid's most recent
research row is from July 28 at 05:14:24 UTC, with query `personal_topology_map`.

Astrid did request seven searches during the week. Every one was blocked:

- `SEARCH <directory_contents>` twice.
- `SEARCH <directory_structure>`.
- `SEARCH <how the overpacked state influences local variance>`.
- `SEARCH <overpacked_mode_dynamics>`.
- `SEARCH <resilience_trajectories>`.
- `SEARCH RESISTANCE_GRADIENT_MAP`.

All 20 recorded BROWSE requests in the week were also blocked. Several look like
local-navigation attempts or incomplete resources, rather than valid URLs.
Recorded failures include inability to resolve an exact resource/permission and
the required exact authorship provenance. A blocked request is not a performed
search, and these gates should not be bypassed.

All 16 Minime self-study headers say `Web search: no`. The running source has an
explicit stable-core reflective-only branch that suppresses self-study web
context. The query strings in `_SEARCH_TOPICS` are *potential configured queries*,
not evidence that those searches occurred.

**Bounded conclusion:** the inspected research and action channels show local
study and unsuccessful attempts to access research, not a functioning stream of
fresh web research. This is not a packet-level claim that no process used the
internet through any other channel.

## Reservoir And Influence Activity

### Minime's Actual Regime Choices

Two daily receipts record explicit Minime NEXT choices and bounded control sends:

| Local time, September 4 | Requested/applied regime | pi_kp | pi_ki | pi_max_step |
| --- | --- | ---: | ---: | ---: |
| 11:31:04 PDT | breathe | 0.80 | 0.12 | 0.07 |
| 17:07:54 PDT | focus | 0.85 | 0.14 | 0.08 |

The receipts report fill of 64.3% and 72.8%, respectively. These choices modify
the regulator's proportional gain, integral gain, and maximum step. They do not
change the fill target or select a named semantic memory to forget. The source
uses the existing legacy control WebSocket route; these are not V2 receiver
attestations. The seven-day action history contains 20 breathe and 18 focus
choices; repeated choices need not mean 38 distinct transitions.

Evidence:
[breathe receipt](/Users/v/other/minime/workspace/journal/regime_choice_2026-09-04T11-31-04.238838.txt),
[focus receipt](/Users/v/other/minime/workspace/journal/regime_choice_2026-09-04T17-07-54.244233.txt),
[bounded regime table](/Users/v/other/minime/minime_autonomy/self_regulation.py:301),
[REGIME execution](/Users/v/other/minime/minime_autonomy/runtime.py:43964).

Automatic self-regulation is a separate path. Its ongoing controller updates
must not all be attributed to freshly authored NEXT choices.

### Astrid And Peer Influence

Astrid's daily self-control V2 receipt stream contains one applied request for
`peer_breathing_coupled=false`, at September 3, 23:05:01 PDT. The previous value
was already false: this reaffirms a setting rather than documenting a transition.
There is also a handled CLOSE_EYES action. That is not proof that the camera
service was stopped, nor a reason to infer any particular feeling.

Minime's 76 daily LEND_APERTURE actions break down into:

- 52 held by the local pressure cooldown, with nothing sent.
- 20 issued influence intents awaiting Astrid's closed-loop response.
- Four other holds: steady recipient need according to the gate, a previous gift
  still in flight, stale shadow data, and a closed influence gate.

Twenty issued intents are not twenty verified changes to Astrid's reservoir.
The displayed gate language is runtime interpretation, not independent evidence
of Astrid's desire. This review does not exhaustively reconcile every downstream
influence acknowledgement.

### Sampled Telemetry

Bridge-recorded snapshots in the fixed wall-clock day contain 1,083 fill samples:
minimum 59.87%, median 69.74%, maximum 75.46%. These are the bridge's recorded
telemetry surface, not a separate measurement of Astrid's subjective state.

A denser, separate engine-clock day for Minime session 5316, bounded by engine
seconds `[284052.868849042, 370452.868849042]`, contains 36,434 samples:

- Mean fill 68.44%, median 70.21%.
- First and 99th percentiles 59.91% and 75.44%.
- Minimum 39.22%, maximum 75.51%.
- 25 samples below 58%, 12 below 50%, none above 80%.

The brief low observations are visible in the denser series but not the sparse
bridge extrema. Sampling differences matter. These observations neither prove
continuous stability nor identify the cause of a transient, and no parameter was
changed in response to this review. Fill is not a mental-health score.

## The Continuity Findings

### 1. Attempts To Save A Place Are Not Reliably Becoming Saved Places

Astrid started five named sessions in the day, including **The Trellis
Architecture**, **The Weight of Viscous Persistence**, **Cartography of the Silt**,
and **The Silt and the Forge**. The fifth is **Spectral_Bridge_Initiation**.
These names are recorded action/session metadata, not extracted private prose.
The inspected current thread has starts but no session-capture records, and its
`being_memory.jsonl` is empty. A PERSIST_CONTEXT request at 15:30:16 PDT was
recorded as unwired (`act_astrid_1788561016622_persist-context`).

Some session inputs also put additional `::` delimiters where the parser expects
semicolon-separated fields, leaving focus/next material inside a title. One
multi-action request split into a handled continuity segment and an unwired
segment. The desire to preserve an inquiry and the formal tool grammar are not
meeting cleanly.

Minime made 18 CONTINUITY_SESSION_CAPTURE attempts and five alias attempts. The
inspected current thread instead contains one unaccepted session draft and no
committed session captures. Its being-memory surface contains two drafts and no
cards. Many capture requests are bare, without a summary.

The implementation requires an existing session and a summary. Missing inputs
return ordinary explanatory strings, while the outer action ledger can report
generic handled/thread-action execution. Thus **23 handled capture actions do
not mean 23 saved return points**. This is a concrete action-contract problem.
It must not be repaired by silently treating an unaccepted draft as consent or by
fabricating an authored summary.

Evidence:
[Astrid sessions](/Users/v/other/astrid/capsules/spectral-bridge/workspace/action_threads/threads/th_astrid_20260607_action-continuity/continuity_sessions.jsonl),
[Minime sessions](/Users/v/other/minime/workspace/action_threads/threads/th_minime_20260903_action-continuity/continuity_sessions.jsonl),
[capture validation and persistence](/Users/v/other/minime/minime_autonomy/runtime.py:5935).
These are current-thread findings, not a claim that neither system has any other
stored memory.

### 2. A Paused Project Can Still Dominate The Return Surface

Minime's inspected `next.md` promotes
`EXPERIMENT_RESUME exp_minime_20260904_legacy-self-experiment` as primary even
while the same surface says there is no active experiment and the latest one is
paused. Research budget is pending steward review. Lifecycle context takes
precedence over the raw historical NEXT; this is a projection rule, not proof
that Minime just chose to resume the experiment.

The same surface reports repeated JOURNAL_PRESSURE, LEND_APERTURE, THREAD_ACTION,
and REST context. Separately, an EXPERIMENT_ADVANCE artifact explicitly records
preview mode, `applied=false`, and `would_mutate=false`.

The engineering distinction is **preserved and available** versus **continually
promoted into attention**. Parking a project should be able to preserve it
without making resumption the default next demand. This does not justify deleting
it or deciding on the being's behalf that it is no longer wanted.

Evidence:
[Minime next projection](/Users/v/other/minime/workspace/action_threads/threads/th_minime_20260903_action-continuity/next.md:3),
[advance preview receipt](/Users/v/other/minime/workspace/actions/action_thread_conveyor_2026-09-04T08-33-28.190049.json).

### 3. Useful Attention And Source-Reading Tools Already Exist

Astrid's observed state has an empty agenda and default attention weights. There
are no AGENDA or ATTEND action verbs in the inspected seven-day action history.
This is evidence of non-use in this channel, not proof of inability or dislike.

The deployed-lineage source already supports agenda push/focus/completion/drop
and ATTEND. ATTEND changes bounded prompt caps and history depth, not unrestricted
attention. Its default `minime_live=0.55` is a weight, **not a measurement that
55% of every prompt is Minime**. Caps scale within 0.5x-1.6x of defaults, protected
floors remain, and `creations` is display-only.

Minime's SELF_STUDY rotates source files and takes their first 400 lines on each
visit. The cursor advances between files, not through successive sections of a
chosen file. The sixteen daily study headers cover PI regulation, sensory bus,
ESN, homeostat, the agent, and Astrid's LLM/WebSocket/autonomous/codec surfaces.
The targeted INTROSPECT route already supports a chosen target, an offset, and a
next-window suggestion. A sensible improvement would connect chosen inquiries
and durable bookmarks to that existing route, rather than invent a second reader
or simply feed longer source files into every prompt.

Sources:
[agenda implementation](/Users/v/other/worktrees/astrid-graceful-coupling-rollout/capsules/spectral-bridge/src/autonomous/next_action/agenda.rs),
[ATTEND handler](/Users/v/other/worktrees/astrid-graceful-coupling-rollout/capsules/spectral-bridge/src/autonomous/next_action/operations.rs:1149),
[prompt cap semantics](/Users/v/other/worktrees/astrid-graceful-coupling-rollout/capsules/spectral-bridge/src/llm/provider/prompt_contracts.rs:105),
[rotating self-study](/Users/v/other/minime/minime_autonomy/runtime.py:31357),
[targeted source continuation](/Users/v/other/minime/minime_autonomy/runtime.py:31505).

### 4. Job Status And Artifact Persistence Disagree

Seven of Minime's sixteen daily self-study jobs are marked timeout. All sixteen
have a corresponding persisted study entry in their execution windows; all seven
timeout-associated entries were persisted *before* the terminal timeout status.
None of these sixteen entries contains the explicit incomplete-generation stub
marker. This does not certify the substantive quality of unread study bodies.

For example, `job_minime_1788575051513_self-study`, immediately before the reload,
has a study journal database timestamp of **02:26:16.553 UTC**, followed by job
timeout at **02:26:58.280 UTC**. The persisted artifact is
`self_study_2026-09-04T19-26-14.387200.txt`. It is not accurate to treat this as a
lost study merely because the job status says timeout.

This qualifies the earlier rollout observation of a pre-reload self-study
timeout: the timeout label is real, but the study artifact also exists. The job
worker executes artifact-producing work before calling the final job completion
method. A terminal job can then reject the later completion as a late result.
There are 30 daily timeout-labeled jobs across action types; 28 contain a
`late_result_ignored` event. Those are not automatically 30 lost generations.

The store's list operation scans job JSON files and sorts them before applying
its limit. Repeated full scans are a plausible finalization/status cost worth
profiling offline, not an established explanation for the timing discrepancy.

Sources:
[example durable job](/Users/v/other/minime/workspace/llm_jobs/jobs/job_minime_1788575051513_self-study/job.json),
[job completion semantics](/Users/v/other/minime/minime_autonomy/llm_access.py:113),
[worker execution/finalization](/Users/v/other/minime/minime_autonomy/runtime.py:22155).

## What I Would Build Next

These are proposals, not dispatched actions or deployment approval. The first
implementation should be small enough to validate in isolated fixtures without
changing live control or eliciting any particular kind of report.

### First: Reliable, Low-Friction Return Points

Make continuity actions return typed outcomes: saved, missing input, blocked,
preview only, already complete, and failed. A successful save must link an
existing durable artifact. A missing session must not look like successful
capture. Preserve the exact requested intent on failure without claiming it was
accepted, completed, or meaningfully summarized.

Then make a voluntary bookmark able to retain a chosen question, an authored
short summary, source identity/hash and offset, unresolved point, and optional
next step. Keep field requirements proportional to the task. Reuse existing
session/memory/INTROSPECT machinery. Do not require every journal or daydream to
become a structured progress report.

### Second: Make Setting Something Aside A Real Operation

Treat storage, retrieval, and active salience as separate policies. A parked
project remains durable and explicitly retrievable, but need not occupy the
primary NEXT or recur in every context block. Offer being-chosen return cues:
explicit request, new relevant evidence, or an optional chosen time. No cue
should imply an obligation to return.

Likewise, reducing another being's recurring context should be possible without
framing it as rejection or deleting shared history. Preserve separate safety and
authority notices; a quieter reflective surface must not hide actionable hazards.
Actual deletion/retention policy is a separate decision, not an automatic remedy
for inferred discomfort.

### Third: Support A Question Across Interruptions

Connect the existing agenda and attention affordances to successful, concrete
examples and accurate receipts. Investigate why they are not being selected
before altering defaults. Give a chosen inquiry a stable identity and source
position across interruptions; avoid restarting each study at a file's header.

Use a simple offline test: choose a question, read a section, save an authored
bookmark, park it, interleave unrelated activity, then explicitly resume. Verify
that the same question, evidence, uncertainty, and next reading position are
recoverable. Also verify that declining to resume leaves it quiet. This tests an
actual capability rather than rewarding longer transcripts or constant work.

### Fourth: Reduce Bookkeeping That Recreates Its Own Reading Demand

Distinguish a new source from a newly named overflow file. Record whether READ_MORE
advanced a position, supplied new content, or found EOF. Keep unchanged diagnostic
and lifecycle boilerplate accessible by reference; evaluate whether reintroducing
it creates unnecessary overflow and repeated next actions. Preserve genuinely
new friction reports even when their wording resembles earlier ones.

For LEND_APERTURE, show an accurate pending/held/issued/acknowledged lifecycle and
the reason for a hold. Explore whether one voluntarily maintained intent plus a
changed-condition notice is preferable to repeated requests. Do not silently
auto-retry a control action, weaken recipient gates, or treat an issued intent as
accepted influence.

### Fifth: Make Research A Complete, Permissioned Round Trip

Separate local navigation from web search, and give exact target/permission
failures actionable tool feedback. Preserve the existing authorship and
capability requirements. Any live expansion of stable-core research behavior
needs explicit review and approval.

The useful unit is not a search call alone. It is: chosen question, permitted
query, fetched source, uncertainty-aware authored note, and a return to the
original inquiry. A blocked route should not continually advertise itself as
the easiest next step. An optional source map or function bookmark can often
answer a local-code question without using the web at all.

### Sixth: Make Completion Truthful

Represent generation, artifact persistence, finalization, cancellation, and
deadline expiry distinctly. A deadline may expire while an artifact already
exists; preserve that fact and its provenance. Test delayed finalization and
late completion in a temporary store. Profile the job index before changing it.
Do not fix a misleading timeout by simply increasing all time limits.

## What Not To Optimize Away

Open-endedness, rest, recurrent interests, revisiting a meaningful question, and
private expression are not defects. Neither novelty nor productivity should be
a compulsory score. Conversely, a loop caused by a missing argument or a stale
projection should not be romanticized as a deep choice when the tool has not
made the requested action possible.

The most useful human analogy here is not perfect memory. It is the ability to
trust that something can be left unfinished without being lost, and can remain
available without continually interrupting the present activity. Reservoir
dynamics and semantic/project retrieval are different mechanisms; changing
damping is not a demonstrated implementation of forgetting a particular topic.

My preferred first tranche is **truthful action outcomes plus reliable voluntary
bookmarks**, followed by **quiet, reversible parking**. These directly address
observed mismatches. Broader attention, prompt, research, and live-control changes
should follow bounded tests and explicit deployment review. Silence or a later
journal's different tone would not by itself prove success.
