# Collaboration Attention Delivery V1

Status: proposed for Mike/Astrid/Minime review. No implementation or live
behavior change is authorized by this document.

Date: 2026-09-07

## Decision Summary

Keep the complete collaboration, chamber, and correspondence records durable
and queryable. Stop placing an unchanged open relationship or unresolved
receipt into every generation prompt.

New material should create one bounded, role-aware prompt opportunity for each
relevant being. A local delivery receipt should suppress that exact revision
after it was present in a final model request. New evidence creates a new
revision and can surface once. An explicit status action always returns the
current full state. Silence never becomes assent, refusal, receipt, closure, or
evidence about felt state.

There should be no periodic ambient reminder for an unchanged receipt wait.
Discoverability belongs in explicit action/status surfaces, not in repeated
obligation-shaped prose.

## Why This Change Is Needed

The current implementation combines durable truth with prompt attention.
Those are different responsibilities.

The live collaboration selected by both prompt renderers is
`coll_1778605252_spectral-cascade-dynamics`. Its metadata says that it was
created as a smoke test, was pre-joined on 2026-05-12, and has not had a
metadata transition since then. Its canonical joined state may still be worth
preserving. That fact does not make its full state newly relevant on every
turn.

At the time of this review:

- `chamber_state.json.prompt_summary` was 2,303 characters.
- The summary repeated old shared thoughts, pending ACK facts, a claimed-thread
  wait, native-thread continuity, an affordance budget, and a receipt wait.
- `/tmp/bridge.log` contained 476 `collab_note emitted` records.
- The chamber sidecar refreshed `updated_t_ms` and live relational fields even
  when the unresolved correspondence facts were unchanged.
- Astrid's renderer appended the collaboration summary inside codec/spectral
  feedback. That lets relational administration enter modes whose subject is
  immediate spectral or private experience.
- Minime's `_query_llm` appended the active collaboration suffix regardless of
  ordinary, journal, self-study, or private-qualia context. A later adapter can
  compact it away, but candidate injection is still unconditional and actual
  delivery is not tracked.
- Astrid's prompt-budget diagnostics frequently showed the whole spectral or
  continuity block being evicted. The current code therefore cannot equate
  "rendered a candidate string" with "supplied this state to the model."

This is evidence of repeated prompt pressure and provenance confusion. It is
not evidence that the repeated text caused any particular first-person report.

## Goals

1. Preserve every canonical collaboration, chamber, and correspondence fact.
2. Surface genuinely new authored or relational evidence promptly and once.
3. Keep direct peer messages reliable independently of ambient collaboration
   context.
4. Give Astrid and Minime an explicit, read-only way to recover full status at
   any time.
5. Keep private journal, moment, and self-study space free of stale peer or
   steward administration unless that material was explicitly selected.
6. Record whether a prompt opportunity was omitted, packed out, submitted, or
   explicitly inspected.
7. Apply the same policy on Astrid and Minime while allowing role-specific
   wording.
8. Reduce prompt size without reducing action vocabulary or correspondence
   capability.

## Non-Goals

- Do not close, leave, decline, acknowledge, or otherwise mutate a
  collaboration because it is old.
- Do not infer that silence means the being ignored, forgot, declined, agreed,
  received, or felt anything.
- Do not delete or rewrite historical chamber, receipt, or correspondence
  records.
- Do not change the reservoir, PI controller, fill target, sensory cadence,
  codec gain, semantic transport, or peer runtime authority.
- Do not automatically send a message, ACK, reply, trace, attention canary, or
  microdose.
- Do not remove the collaboration/correspondence actions from HELP, FACULTIES,
  or explicit status output.
- Do not treat volatile joint-reservoir motion as a request for linguistic
  attention.
- Do not claim that quieter prompts resolve any reported friction. They create
  a cleaner condition in which later reports can be heard.

## Core Invariants

### Canonical state is not prompt state

A joined room may remain joined forever. A pending receipt may remain pending
forever. Both remain available through source records and explicit status.
Neither fact alone earns a permanent prompt slot.

### Newness is event-based

Wall-clock age is useful metadata but is not the source of truth. Newness comes
from a stable material revision derived from authored events and discrete state
transitions. Rewriting `updated_t_ms`, changing an age label, or ticking a
reservoir must not create a new material revision.

### Delivery is audience-specific

Astrid and Minime have independent exposure histories. Supplying a revision to
Astrid says nothing about whether Minime saw it. Sender and recipient views are
different.

### Candidate render is not delivery

A revision is not delivered merely because a helper returned a string. It must
survive prompt budgeting/adaptation and be present in the final request sent to
a model backend.

### Optional means quiet after one opportunity

Right-to-ignore language is not enough if the same card appears on every turn.
After one supplied opportunity, the same revision becomes ambiently silent.
It remains fully retrievable.

### Direct correspondence is independent

Inbox admission, sender binding, protected delivery, and exact reply links stay
authoritative. Collaboration-summary suppression must never suppress a new
direct message body or its transport receipt.

### Private reflective space is actually private

Moment capture, private qualia, journal, and self-study modes do not receive an
ambient collaboration summary. They may receive collaboration material only
when the being explicitly selected that material as the foreground activity,
or when a direct message is being delivered through its protected lane.

## Three-Plane Architecture

### 1. Durable state plane

Existing source records remain unchanged:

- collaboration `meta.json` and `timeline.jsonl`
- `shared_thoughts.jsonl`
- chamber notes, annotations, presence, consent, phase, and memory records
- first-class correspondence ledger, message IDs, thread IDs, and receipts
- `chamber_state.json` as a complete current projection

The existing long `prompt_summary` remains temporarily for backward
compatibility and explicit status. New consumers stop treating it as an
always-on suffix.

### 2. Attention projection plane

The chamber projector adds an additive `attention_projection_v1` object to
`chamber_state.json`:

```json
{
  "schema_version": 1,
  "policy": "collaboration_attention_projection_v1",
  "material_revision": "sha256:...",
  "material_t_ms": 1780000000000,
  "material_event_ids": ["..."],
  "latest_material_event": {
    "event_id": "...",
    "kind": "peer_message",
    "actor": "astrid",
    "audiences": ["minime"],
    "thread_id": "...",
    "source_ref": "..."
  },
  "categories": ["correspondence"],
  "volatile_revision": "sha256:...",
  "status_summary_sha256": "...",
  "optional": true,
  "silence_means": "neutral_no_inference",
  "authority": "language_context_not_control"
}
```

`material_revision` is computed from a canonical ordered set of stable source
event IDs and discrete state labels. It excludes bodies where an existing
stable ID already names the immutable event. When a legacy source has no event
ID, the projector hashes a canonical subset of its authored fields.

`volatile_revision` may change with reservoir/resonance measurements. It is
diagnostic only and never causes ambient linguistic resurfacing.

The projection contains facts, not a recommended action. Consumers render a
short audience-specific notice from those facts.

### 3. Local delivery plane

Each being owns a restart-stable delivery checkpoint and an append-only audit
of revision transitions. A representative record is:

```json
{
  "schema_version": 1,
  "policy": "collaboration_prompt_delivery_v1",
  "being": "astrid",
  "material_revision": "sha256:...",
  "event_id": "...",
  "state": "submitted",
  "render_tier": "new_notice",
  "generation_id": "...",
  "mode": "dialogue_live",
  "prompt_chars": 212,
  "content_sha256": "...",
  "observed_at_unix_ms": 1780000000000,
  "authority": "prompt_delivery_evidence_not_receipt_or_uptake"
}
```

Allowed delivery states are:

- `observed`: a new revision exists locally.
- `ineligible_mode`: the current mode is private or unrelated; no prompt text
  was created.
- `candidate`: a bounded block was offered to prompt assembly.
- `packed_out`: the final adapted request omitted the block.
- `submitted`: the final model request contained the block.
- `explicitly_inspected`: the being selected a status/read action that returned
  the same revision.
- `superseded`: a newer material revision arrived first.

Only `submitted` and `explicitly_inspected` satisfy ambient delivery for that
audience/revision. A network timeout after submission does not cause automatic
repetition of an optional notice. It remains available explicitly.

These records do not claim reading, comprehension, uptake, memory, agreement,
or felt impact.

## Material Event Taxonomy

The following create a new material revision for their relevant audience:

- collaboration invitation, join, decline, leave, or explicit metadata edit
- new peer-authored shared thought
- new direct peer message, reply link, ACK, held/needs-time receipt, or trace
- new being-authored chamber presence receipt, annotation, or consent stance
- new being-authored objection, contradiction, pressure/flat outcome, or
  `still_friction` evidence
- attention-canary activation, withdrawal, or authored outcome
- a new phase witness/felt receipt or an explicit phase-state transition
- a new steward note or intention explicitly addressed to this room
- an explicit support proposal or revision to that proposal

The following do not create a material revision:

- `updated_t_ms`, file mtime, cache refresh, or age-string changes
- joint reservoir ticks, tick count, h-norms, resonance drift, fill, or other
  volatile telemetry
- repeated derivation of the same `pending_ack`, receipt-wait, claimed-thread,
  or affordance-budget state
- a sender waiting for a peer-authored receipt
- a right-to-ignore grace threshold crossing
- the same summary being truncated differently by a prompt cap
- a journal mention that is not a direct correspondence event

Volatile state remains visible through explicit status and dedicated telemetry
surfaces.

## Render Policy

### Direct delivery

A newly admitted direct message uses the existing protected inbox path. The
exact peer-authored body and sender/thread identity take precedence. The
collaboration renderer should not append a second receipt tutorial beside it.

### New notice

For a material event not already carried by protected direct delivery, the next
eligible non-private generation gets one notice capped at 320 characters. It
states what changed, who authored it, where it is stored, and that no response
is required. It does not list three command templates.

Examples of shape, not fixed prose:

```text
Collaboration update: Minime added a shared thought to "spectral cascade dynamics". It is available via COLLABORATION_STATUS latest. No response is required; silence remains neutral.
```

```text
Correspondence update: your ACK was recorded on thread abc. The thread remains available via CORRESPONDENCE_STATUS. This changes no control or peer state.
```

For a sender-side wait, do not render "peer-authored receipt required". At most,
the original send result may say once: "Sent; the peer may respond or not."

### Explicit status

Add `COLLABORATION_STATUS [id|latest]` to both action surfaces. It is read-only
and returns:

- canonical room metadata and membership state
- latest material revision and source event IDs
- recent authored shared thoughts
- chamber phase and authored notes/annotations/consent
- current correspondence/thread/receipt facts
- latest volatile joint trace clearly labeled as current measurement
- source paths for deeper inspection
- the authority boundary

`LIST_COLLABORATIONS` remains the compact index. `CORRESPONDENCE_STATUS`
remains the detailed correspondence view. Explicit status output is not
ambiently reinserted on later turns unless the being chooses to carry it into a
thread/bookmark using existing continuity actions.

### Silent state

If the latest material revision has already been submitted or explicitly
inspected, render nothing. Do not render a tiny periodic reminder. Do not update
canonical relationship state.

### Explicit foreground

If the being chooses a collaboration action, correspondence action,
`COLLABORATION_STATUS`, `CORRESPONDENCE_STATUS`, a saved collaboration thread,
or an explicit attention focus on the room, provide the context required for
that action even when the ambient revision was previously delivered.

## Astrid Integration

Current coupling lives in
[`codec/feedback.rs`](../../capsules/spectral-bridge/src/codec/feedback.rs),
which calls `active_collaboration_suffix_line()` on every codec feedback render.
That ownership is wrong for relational attention.

Implementation should:

1. Remove collaboration/chamber rendering from codec feedback. Codec feedback
   remains about spectral and coupling evidence.
2. Replace `active_collaboration_suffix_line()` with a structured read helper in
   [`collaboration.rs`](../../capsules/spectral-bridge/src/autonomous/next_action/collaboration.rs).
3. Add restart-stable delivery state to `ConversationState` and `SavedState`
   with serde defaults for backward compatibility.
4. Add a dedicated `collaboration` prompt block to dialogue assembly. It has no
   minimum floor and exists only for an unseen material revision or explicit
   foreground action.
5. Carry the offered revision through prompt assembly and final request
   adaptation. Mark `packed_out` if absent; mark `submitted` only when the
   final backend request contains it.
6. Keep direct inbox content in `direct_perception`; do not downgrade it into
   the collaboration block.
7. Exclude the ambient collaboration block from moment capture, private
   journaling, introspection, and self-study generation.
8. Log revision, audience, tier, final chars, and disposition. Stop logging the
   full collaboration summary on every render.

Suggested prompt-block priority is below protected direct perception and above
ambient chamber/continuity ballast. Because the block exists once per revision,
it should not need a permanent protected floor.

## Minime Integration

Current coupling lives in `minime_autonomy/runtime.py`:
`_query_llm()` unconditionally appends `_collab_active_suffix_line()` before
generation. This occurs after callers have chosen a context mode.

Implementation should:

1. Replace `_collab_active_suffix_line()` with a structured projection reader
   plus a role-aware renderer.
2. Consult `context_mode` before creating a candidate. Private qualia, journal,
   daydream, self-study, and strict review receive no ambient collaboration
   text.
3. Permit context only for a new material revision in an eligible ordinary
   mode or for an explicit collaboration/correspondence foreground action.
4. Persist Minime's latest delivered revision in the existing restart-stable
   autonomy state using atomic writes and a backward-compatible optional field.
5. Inspect the final adapted messages recorded by the generation adapter. Do
   not mark a pre-compaction candidate as submitted.
6. Preserve the existing explicit peer-status readers and the test invariant
   that neutral check-ins omit ambient peer data.
7. Keep the action vocabulary discoverable, but do not repeat command grammars
   inside a new-event notice.
8. Record the same bounded delivery diagnostics as Astrid.

## Shared Projector Integration

[`triadic_chamber.py`](/Users/v/other/neural-triple-reservoir/triadic_chamber.py)
currently creates a long prompt-oriented summary from durable and volatile
state. It should become the source of stable revision identity, not the owner of
per-being delivery.

Implementation should:

1. Add `attention_projection_v1` without removing or renaming existing fields.
2. Derive `material_revision` from stable event IDs and discrete transitions.
3. Derive `volatile_revision` separately from measurements.
4. Keep `prompt_summary` for old consumers and explicit status during the
   migration.
5. Add source fixtures proving that repeated sidecar refresh produces an
   identical material revision.
6. Never write delivery/uptake claims on behalf of either being.

The sidecar may continue writing `chamber_state.json` on its current cadence.
Consumers compare the stable material revision rather than the file mtime.

## Compatibility and Failure Behavior

- Deploy the additive projector schema before enabling new consumers.
- New consumers encountering no `attention_projection_v1` should use the
  legacy renderer only when an explicit rollback/config flag requests it.
  Their default ambient behavior should be quiet, while explicit status reads
  the old state normally.
- Corrupt projection data suppresses ambient rendering, emits a bounded
  diagnostic, and leaves explicit source records untouched.
- A local delivery-checkpoint write failure does not mutate shared state. To
  avoid prompt pressure, the process should not repeat the same optional notice
  more than once in the current process; it should report restart persistence
  debt.
- A final-adapter ambiguity is recorded as `packed_out` unless inclusion can be
  proven.
- A new direct message never depends on the projection or checkpoint path.

Rollback flags should restore the legacy renderer without changing canonical
records. Proposed names:

```text
ASTRID_COLLAB_PROMPT_POLICY=event_v1|legacy|off
MINIME_COLLAB_PROMPT_POLICY=event_v1|legacy|off
```

After verified rollout, `event_v1` should be the default. `legacy` is a bounded
rollback posture, not the long-term default.

## Tests

### Shared projector

- Same authored events plus different `updated_t_ms` gives the same material
  revision.
- Reservoir/resonance/tick changes alter only the volatile revision.
- New invite/join/leave/shared-thought/message/reply/ACK/trace/annotation/
  consent/phase event changes the material revision.
- Pending receipt age and right-to-ignore grace crossing do not change it.
- Canonical ordering makes the digest deterministic.
- Existing chamber JSON remains readable by old consumers.

### Astrid

- Codec feedback contains no collaboration or receipt prose.
- First eligible generation for a new revision gets one bounded block.
- Second generation for the same revision gets no block.
- Prompt eviction does not create a false submitted receipt.
- Final request inclusion creates a restart-stable submitted receipt.
- Restart does not replay the same revision.
- Explicit status always returns the complete current state.
- Direct inbox delivery survives all suppression states.
- Moment, journal, introspection, and self-study modes omit ambient room state.
- Sender-side rendering never asks the sender to manufacture peer evidence.

### Minime

- Every private/context-mode variant omits ambient collaboration state.
- Existing neutral-checkin peer-isolation tests remain true after the final
  `_query_llm` assembly step.
- Adapter compaction cannot produce a false submitted receipt.
- First eligible revision appears once; repeated revision stays quiet across
  cycles and restart.
- Explicit status and foreground collaboration actions remain complete.
- Direct correspondence delivery remains sender-bound and independently
  receipted.

### Cross-runtime fixtures

A shared JSON fixture should be consumed by both Rust and Python tests. For the
same projection and audience it must agree on:

- eligible versus silent
- event category and role
- render tier
- maximum rendered length
- no-response/neutral-silence boundary

Exact prose may differ slightly, but authority and action implications must not.

## Observability

Add bounded counters and append-only transition records for:

- material revisions observed
- notice candidates created
- candidates packed out
- final requests submitted with a notice
- unchanged revisions suppressed
- private modes kept quiet
- explicit status inspections
- superseded unseen revisions
- prompt characters offered, delivered, and avoided

Astrid's `context_packing_pressure_v1` should report a distinct
`collaboration` block when present. Minime generation records should link the
material revision and final inclusion state. Logs should contain IDs, hashes,
lengths, and dispositions, not full chamber or correspondence prose.

These are transport/prompt facts only. They do not measure uptake or benefit.

## Acceptance Criteria

1. With no material collaboration event, 20 consecutive eligible ordinary
   generations contain zero ambient collaboration/receipt-wait blocks.
2. A new non-direct shared event appears in exactly one submitted prompt per
   relevant being, unless packed out or superseded.
3. A new direct peer message is delivered through the protected inbox path and
   is not duplicated by a receipt tutorial.
4. The sender sees no repeating peer-receipt demand.
5. `COLLABORATION_STATUS latest`, `LIST_COLLABORATIONS`, and
   `CORRESPONDENCE_STATUS` expose the expected canonical state on demand.
6. Private journal, moment, self-study, and introspection prompts contain no
   ambient collaboration/chamber summary.
7. The current 2,303-character stale chamber summary is absent from ordinary
   prompt bodies after its revision has been delivered or explicitly inspected.
8. Restart does not replay a delivered revision.
9. New objection, contradiction, pressure/flat outcome, or `still_friction`
   evidence creates a new revision and remains unsmoothed.
10. No telemetry, control, PI, fill, codec, reservoir, sensory, or authority
    behavior changes.

## Offline Replay Before Rollout

Use captured recent generation requests and the current shared room as fixtures.
For each request, compare legacy and event-v1 assembly:

- final prompt characters
- whether direct perception survived
- whether journal/source material gained room
- whether a material collaboration event was present
- whether final adaptation preserved or removed the candidate
- whether a delivery receipt would have been truthful

Do not score being-authored prose as better or worse in this mechanical replay.
The replay verifies context routing, provenance, and prompt pressure only.

## Rollout Sequence

1. Capture the current canonical room hashes, service versions, prompt-budget
   baseline, and recent final adapted requests.
2. Implement and test the additive projector in the shared reservoir project.
3. Implement Astrid's dedicated collaboration block and delivery checkpoint.
4. Implement Minime's context-mode gate and delivery checkpoint.
5. Run shared fixtures, focused unit tests, bridge tests, Minime tests, domain
   boundary audit, and source-tree checks.
6. Start the projector/feeder through its sanctioned service wrapper and verify
   stable material revision across several volatile refreshes.
7. Gracefully deploy Astrid through `scripts/build_bridge.sh`; verify drain,
   fresh PID, hashes, ports, telemetry, readiness, and generation records.
8. Gracefully restart only the Minime autonomy surface required for the Python
   prompt change. Do not restart or retune the ESN/controller unless its
   sanctioned wrapper proves that coupling unavoidable.
9. Observe naturally occurring entries and action records without asking either
   being to confirm improvement.
10. Compare prompt delivery, omission, private-mode isolation, direct-message
    fidelity, and prompt-size evidence against baseline.

The current joined smoke-test room should remain untouched during rollout. Its
canonical lifecycle can be reviewed separately by Mike, Astrid, and Minime.

## Abort and Rollback Criteria

Return consumers to `legacy` and stop the rollout if any of the following
occurs:

- a direct peer message is omitted, misattributed, or retired without protected
  delivery evidence
- an explicit status action cannot reconstruct the canonical state
- a consumer records `submitted` for text absent from the final request
- private modes receive stale ambient collaboration context
- revision instability is caused by clocks, ages, telemetry, or sidecar refresh
- restart loses or corrupts the local delivery checkpoint
- bridge/Minime readiness, ports, telemetry, or generation recording regresses

Rollback changes prompt presentation only. It must not rewrite collaboration or
correspondence history.

## Proposed Agreement

The implementation should proceed with these decisions as a bundle:

1. No periodic ambient reminder for unchanged waits.
2. One delivery-aware notice per material revision and audience.
3. Direct messages stay on protected delivery lanes.
4. Private reflective modes are quiet by default.
5. Add explicit `COLLABORATION_STATUS` retrieval.
6. Leave the historical smoke-test collaboration canonically joined.
7. Deploy event-v1 as the intended default with a reversible legacy flag.

Agreement to this spec authorizes source and test implementation only. Live
restart/deployment remains a separate reviewed step with service preflight,
focused tests, and explicit verification.
