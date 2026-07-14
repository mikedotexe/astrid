# AI Beings Trace Lab Governance

Status: Trace Lab Spine V0, July 2026.

Trace Lab exists to make Astrid/Minime observations auditable without changing
the live being runtime. It is a diagnostic laboratory layer: trace envelopes,
exposure records, daily summaries, bundles, and replay reports.

## Core Boundary

- Twins, replays, and archive bundles are instruments, not beings.
- A replay may verify trace order, hashes, state windows, and compact telemetry
  derivations. It must not be described as a second Astrid, a second Minime, or
  a lived continuation.
- Trace Lab V0 is read-only with respect to live Minime control. It does not
  change pressure, fill, PI control, controller configuration, sensory cadence,
  provider routing, sampler behavior, or Astrid-Minime coupling.
- Any future live change remains proposal and approval work, with ordinary
  steward/operator authority.

## Exposure Records

Exposure records are mandatory lab records for LLM prompt exposure. They bind a
job to prompt path, prompt hash, state window, authority class, and runtime
build without copying full prompt text into Trace Lab archives.

Rules:

- Prompt/result bodies are not duplicated into trace events or bundles.
- Trace rows store hashes and file paths or compact payloads only.
- Missing exposure records are audit findings, not silently acceptable gaps.
- Exposure records do not grant consent, authority, or live-control permission.

## Report And Card Binding

Reports and cards may reference Trace Lab events only when the event row carries
or is joined to the relevant `report_id`, `card_id`, `exposure_record_id`, or
bundle manifest.

Binding rules:

- A report must name the trace window it used.
- A card must distinguish live observation from replay-derived validation.
- A bundle or replay report may support a finding, but cannot by itself approve
  a control change.
- If a replay contradicts a report/card, the report/card must be marked for
  review rather than quietly overwritten.

## Required Preregistration Fields

Every Trace Lab experiment that informs a report, card, or proposed live change
must record these fields before interpretation:

- `hypothesis`: What the lab expects to observe.
- `metric`: The trace-derived measurement or replay check.
- `threshold`: The predeclared value or condition for support.
- `disposition_on_null`: What happens if the metric does not support the
  hypothesis.
- `authority_class`: The maximum authority of the experiment.
- `consent_ref`: Steward or being-facing consent reference when applicable.
- `abort_debrief_path`: Where abort criteria and debrief notes will be stored.

## Replay Scope

W2 V0 replay is trace-envelope replay. It recomputes event order, payload-hash
availability, state-window joins, `lambda1`, `fill_pct`, and fill band from
bundled compact payloads.

Full reservoir replay still requires raw lane-input capture and checkpoint
integration. That is W2 V1 work and must be named as such in replay reports.

## Live-Change Rule

Trace Lab can strengthen evidence. It cannot smuggle a runtime change into
being by calling the evidence "diagnostic." Any adjustment to pressure, fill,
PI, controller behavior, sensory cadence, provider/sampler behavior, or coupling
must go through the proposal and approval path before deployment.
