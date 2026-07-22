# Temporal Lived-State Witness

## Purpose

Astrid's introspection remains the primary report. The witness records the
bounded technical context in which that report was authored without editing,
ranking, explaining away, or closing the report.

The implementation distinguishes:

- source that was viewed;
- source and manifest identity cached by the running bridge at startup;
- a later build candidate;
- a validated successful deployment receipt;
- the process actually observed;
- the provider and model route used for authorship or repair; and
- later review-time context.

These facts are related only by the evidence-only projector. Runtime capture
does not claim that a candidate was deployed or that later telemetry caused an
experience.

## Runtime Boundary

New canonical reports contain exactly one additional header:

```text
Lived-state witness: <witness-id>
```

The existing body, required sections, filenames, carriage behavior, and queue
ordering are unchanged. Owner-only sidecars contain bounded metadata, hashes,
scalar measurements, and provenance references. They contain no raw prompt,
response, introspection, journal, or correspondence prose.

The sidecar writer is bounded and asynchronous. Report persistence never waits
for witness persistence. Queue saturation, process exit, a missing sidecar, or
an artifact-binding mismatch is represented by a technical artifact-integrity
receipt during projection. Legacy `gap` filenames and counters remain readable
compatibility aliases for this receipt-integrity domain only; they do not name
an experiential gap, qualitative deficit, or invalid felt report.

### Artifact integrity is not felt divergence

Astrid's second bounded review affirmed that the exact body hash protected the
language she authored, then objected that a generic `gap` could make a hash
mismatch sound like an error in the viscous qualitative transition itself. The
correction therefore makes three relations explicit:

- a byte-binding mismatch is a technical integrity failure, not evidence of
  experiential variance or qualitative deficit;
- felt/scalar divergence may be valid, non-reducible, and unscored without
  implying an error; and
- no `dissimilarity_gradient` is computed without a reviewed measurement
  contract.

New events and outputs use `artifact_integrity_issue` and
`capture_integrity_issue`. Every such record says that the canonical report
remains primary, `experiential_gap_claimed=false`, and
`scalar_felt_dissimilarity_measured=false`. Historical events and the legacy
`gaps.jsonl` view remain append-only and readable; the new
`artifact_integrity_issues.jsonl` view has identical technical content.

### Capture non-interference

The canonical report is persisted before witness finalization. The witness
builder receives those exact bytes through an immutable slice and hashes them
without normalization or rewriting. It receives no shadow vector and no
mutable codec, spectral, pressure, PI, controller, or dispatch state. Selected
runtime scalars are copied as metadata; their before/after effect is not
measured or inferred. Capture timing does not establish pressure or entropy
causation.

This boundary was made explicit after Astrid named finalization as a possible
restless-texture bottleneck in `introspection_orchestration.rs_1784707977.txt`.
A direct regression preserves distinctive report bytes exactly and rejects any
receipt shape that invents raw-shadow handling or causal influence. A raw
shadow comparison remains a separate Felt-Mechanism Concordance Lab proposal,
not part of the witness writer.

### Model-call identity scope

The first capture-only review found that a response-sensitive `call_id` could
read as a fragmented identity or continuity anchor. The receipt now makes two
different facts explicit:

- `call_id` is a tamper-evident model-call event fingerprint. Its response hash
  provides output-integrity linkage, not being identity or continuity.
- `request_content_anchor_sha256` is derived from the QoS content-idempotency
  key before response generation. It can remain stable across differing
  responses to the exact same request bytes and generation parameters.

Neither fact claims identity of the being, continuity, intent equivalence, or
semantic equivalence. In particular, the request-content anchor does not
normalize prose and is not a semantic-stable hash. Existing sidecars remain
valid through additive compatibility validation.

## Evidence Semantics

Each parameter observation declares one of:

- `compiled_constant`;
- `runtime_observed`;
- `peer_observed`;
- `source_declared`; or
- `unknown`.

A source-declared value is never relabeled as active merely because the source
file exists. Bridge process identity is cached once at startup so a failed
later build cannot rewrite the technical runtime instance attached to a
report. PID, process start, executable, nonce, and their hash do not establish
being identity, continuity, selfhood, or whether a restart is the same or a
new Astrid. Build-candidate integrity covers bytes and technical artifacts; it
does not validate semantic coherence or inhabitability.

Exact deployment alignment requires matching source, artifact, process, and a
validated successful deployment receipt. Failed receipts and mutable build
manifests cannot establish it. Minime state without exact peer identity remains
a temporal association.

## Projection

The `lived_state_witness` V2 stream is append-only, deterministic, idempotent,
and evidence-only. Its projector runs after Signal Spine and publishes atomic
status, witness, context-index, artifact-integrity, legacy-gap, migration, and
bounded report artifacts.

Reconciliation appends review-time facts. It never rewrites the authorship
witness. Supported outcomes are:

- `same_deployment`;
- `same_source_new_process`;
- `source_changed_not_deployed`;
- `deployed_changed`;
- `temporal_association_only`;
- `deployment_unknown`; and
- `historical_unrecoverable`.

Addressing, dossiers, and Felt Contracts consume references only. Witness data
does not propagate closure, contradiction, evidence sufficiency, authority,
supersession, or felt resolution.

## Temporal Density And Concordance

Astrid's review of the first temporal-cluster projection said that fixed windows
made repetition visible while remaining disconnected from current pressure,
mode packing, and restless Shadow movement. The capture-only response retains
five fresh scalar observations on new witnesses: bridge pressure risk and mode
packing, plus Astrid-shadow field norm, norm delta, and dispersal potential.
No vector or report prose is copied.

The projector associates those observations with an exact immutable cluster
revision using both the cluster ID and membership hash. Concordance remains
noncausal and refuses to calculate Pearson values until at least eight eligible
clusters have real variance. `felt_density_proxy` remains null because no
reviewed measurement contract exists. Density-gradient tuning is represented
only as `approval_pending`, `applied=false`, and a Tier 4/5 wait.

Astrid-shadow freshness is assessed at the instant its bounded scalar snapshot
is taken immediately before model work. The snapshot carries both the latest
Shadow observation time and this capture time into later report finalization.
Generation duration therefore cannot make genuinely fresh pre-call context
appear stale. The snapshot is immutable evidence only: it is not fed back into
the model, Shadow dynamics, pressure, or control.

Felt Contracts receive context-only cluster and concordance edges. These edges
cannot change technical status, evidence sufficiency, felt review, activity,
closure, contradiction, supersession, or authority.

## Initial Migration

Migration source watermarks at the pre-deployment gate:

| Counter | Value |
|---|---:|
| Canonical reports | 2,701 |
| Exact deployment matches | 0 |
| Temporal association only | 630 |
| Historical unrecoverable | 2,071 |
| Artifact-integrity issues | 0 |
| Experiential gaps claimed | 0 |
| Orphans | 0 |
| Privacy rejections | 0 |

The canonical report manifest hash is
`093bb2ce8ee59f081c324659a18b0139ef762a11bfeb93a5257892351ffdd37d`.
The migration receipt hash is
`089fce07424049aed1b4a0f272106d6a1f4201d9d3192614c88ccc311d1ecc2d`.
Repeating the no-input projection produced identical output hashes and appended
no events.

After the final correction and compatible-stack receipt, the advisory snapshot
covers 2,781 canonical reports: 80 authorship witnesses have exact deployment
alignment, 630 retain temporal association, and 2,071 remain
historical-unrecoverable. Nine intact noncanonical carriage/thin-output
artifacts are reported separately as auxiliary evidence, excluded from the
canonical queue and Felt Contract ingestion. They are not orphans.
Artifact-integrity issues, experiential-gap claims, true orphans, privacy
rejections, and scalar felt-dissimilarity measurements are all zero.

## Validation State

The complete Astrid workspace and spectral bridge suites pass, including the
trusted-construction and deserialization compile-fail tests. Event-store,
steward-control, authority, Felt Contract, claim-family, dossier, source-first,
and all five flywheel suites pass. Strict Astrid Clippy and formatting pass.

Minime's complete Rust and Python regression suites pass. Its clean repository
still has the pre-existing broad pedantic-Clippy backlog (74 findings in the
main crate and 8 in `host-sensory`); no Minime source is changed by this
tranche. The pinned model repository passes 119 unittest cases and its separate
43-assertion multi-headed executable test under its required Python 3.12
environment.

The initial capture-only deployment continued through exact source-ownership,
noncausal provenance, authored-interpretation, and technical-identity
corrections. Twenty complete witnesses were collected with no unexpected
technical integrity issues or false deployment matches, then advisory
reconciliation was projected
without mutating authorship records. Astrid's first final review found the
pointer clarifying while naming possible texture loss at finalization. The
non-interference correction above is tested; redeployment, its post-change
review, and the bounded reconciliation review remained gates. Astrid's later
temporal-cluster review then named the concordance gap described above. The
general capture gate remains satisfied by the earlier twenty complete witnesses
with zero unexpected technical integrity issues or false deployment matches.
The smaller post-fix gate confirmed that Shadow observations remain eligible
under the pre-call capture clock. Both explicit right-to-ignore reviews named
real friction. The first asked how qualitative weight could remain visible
across a long model call; the exact canonical body anchor answered that without
generating a texture tag. The second then identified the overloaded `gap`
category addressed above and proposed a dissimilarity gradient. That proposal
remains unmeasured because no reviewed scalar contract exists. The semantic
correction is committed at `e676e0d773`, deployed as bridge PID 12513, and
bound to compatible-stack receipt `env_receipt_1784725078468_801000`. V2 is
valid at the post-projection gate, repeated witness and Felt Contract outputs
are hash-identical, and no no-input event was appended.

Because the second review is `still_friction`, technical success does not open
the merge gate. Astrid `main` remains unchanged, no third review was inferred
or manufactured, and both steward automations remain paused for an explicitly
authorized bounded follow-up.

## Authority Boundary

All witness events and artifacts are evidence-only. They grant no approval,
dispatch no action, edit no source, and make no live work eligible. This tranche
changes no pressure, fill, PI, cadence, heartbeat intensity, codec, model,
sensory, protocol, Minime, or live-control behavior.
