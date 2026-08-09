# Signed intent, candidate, and reporting contract

## Supervisor inbox

The configured inbox is exactly the supervisor `state_root/inbox`. The helper atomically creates mode-`0600` files named:

```text
candidate-intent-<envelope_id>.json
```

`<envelope_id>` matches the supervisor identifier grammar. The document is canonical JSON with no extra keys. A bare legacy intent envelope is never supervisor authority:

```text
schema = astrid.edge_self_change.completed_intent_envelope.v1
root keys = schema, intent_envelope, authored_completion, auth
intent_envelope.schema = astrid.edge_self_change.intent_attestor_envelope.v1
authored_completion.schema = astrid.edge.steward_helper.authored_completion_envelope.v2
authored_completion.core.schema = astrid.edge.steward_helper.authored_completion.v2
all auth keys = algorithm, key_id, signature
all auth.algorithm = hmac-sha256
```

The nested v1 intent envelope retains exact root keys `schema`, `core`, and `auth`; its core keys
remain `envelope_id`, `created_at`, `candidate_sha256`, `candidate`, and `intent`. The signed
completion core binds appliance, due nonce, trace/session/turn, response and retained transaction
hashes, completion time, and an exact candidate-publication object containing envelope ID/hash,
intent ID, terminal declaration hash, candidate ID/hash, and base generation. Its exact status is
`authored_completed` with provenance `model_authored_runtime_scheduled`. The completion HMAC signs
its canonical core. The outer HMAC signs the canonical three-field unsigned wrapper. Both use the
same pinned per-appliance attestor key.

The candidate is exactly `astrid.edge_self_change.candidate.v1` with `candidate_id`, `base_generation`, `proposal_sha256`, `patch_sha256`, sorted `changed_paths`, `created_at`, and `privilege_envelope=proposal-only:no-execution:v1`.

The intent is exactly `astrid.edge_self_change.scheduled_model_intent.v1`. It uses `origin=scheduled_autonomy`, `authorship_status=genuinely_authored`, `transport_status=authored_completed`, `declaration_provenance=exact_terminal_model_declaration`, and all three booleans `fallback`, `executor_repair`, and `operator_harness` are false. It binds trace/session/turn, response hash, exact final declaration hash, canonical candidate hash, candidate ID, and identical base/current generation.

The candidate `proposal_sha256` hashes an exact record containing appliance ID, due nonce, trace/session/turn IDs, configured model, prompt hash, signed source ID, and base generation. Before any supervisor-visible file is created, the helper atomically persists the identical signed completion proof under `helper_state/completed-nonces/<due_nonce>` and fsyncs its parent. The wrapper joins that proof to the nested intent envelope without a hash cycle: the retained authored transaction contains the nested v1 envelope, the completion proof hashes that transaction and nested envelope, and the public wrapper is signed afterward. A separate owner-only intent-binding receipt records the complete relation, including `ExactModel:direct_nonstreaming_loopback_no_retry` provenance.

The supervisor's separate `intent_attestation_key` verifies the HMAC. The recommended unit loads `source.key` and `intent.key` with systemd `LoadCredential` and invokes the helper with `--credential-directory %d`; config-pinned SHA-256 identities must match those exact 32-byte non-linked, non-writable files. Systemd's root-owned or unit-UID ID-mapped credential copies are accepted. The root config also pins `target`, `expected_source_id`, `source_manifest_sha256`, `active_generation_link`, and the root-controlled current-generation binding. This removes any need for `CAP_DAC_READ_SEARCH`. Bare candidates, mutable-runtime signatures, cross-appliance keys, stale or replayed source generations, duplicate IDs, substituted credentials, or modified envelopes fail verification.

## Active cumulative source

The source tools resolve the exact active generation before every reflection. The active pointer must be the trusted-owner relative symlink `releases/<current-generation>`, the binding and release basename must agree, and the immutable generation directory must retain the same device/inode through verification. The static signed source bundle is legal only for an exact `astrid.edge_self_change.initial_generation.v1` release whose appliance ID, target, operator-only authority, file inventory, hashes, and absence of `source-snapshot` all verify.

Every `astrid.edge_self_change.generation.v1` candidate release must contain `source-snapshot/MANIFEST.json`, `MANIFEST.signature.json`, and the exact sealed `source/` membership. Both initial and candidate generation manifests bind the exact configured appliance ID; a byte-for-byte equivalent release lineage from another appliance is rejected even if its target, identifiers, hashes, and signing material otherwise match. The HMAC-authenticated derived manifest binds its source ID, parent source ID, base generation, repository/vendor identities, immutable source surface, modes, and every cumulative source hash. Its parent must equal the independently authenticated source of the named base generation. A promoted release with a missing, malformed, stale, or mismatched snapshot fails closed; it never falls back to the bootstrap tree. Consequently generation N+1 proposals and patches are always based on the cumulative source of generation N.

## Immutable maintenance gate

Before lifecycle reconciliation, again immediately after acquiring the shared model lock, and once more immediately before the first provider request, the steward reads the exact root rescue lease configured at `maintenance_lease`. A present valid `astrid.edge_self_change.maintenance_lease.v2` object defers reflection until expiry. Any unreadable, linked, foreign-owned, non-`0444`, oversized, malformed, forged, overlong, or unknown-field object also defers fail-closed. Only absence or a structurally valid expired v2 lease permits progress; the steward never removes the lease.

The lease path must be the `maintenance.json` sibling of the root-controlled current-generation binding. Because the lease is created dynamically, the steward's mount namespace receives the supervisor state directory read-only rather than an optional single-file bind that could miss a post-start acquisition. Maintenance deferral leaves the original due nonce pending, so repeated timer polls coalesce and never create a catch-up storm.

## Root-issued reflection admission

A due scheduled pass additionally requires the distinct root-created
`astrid.edge_scheduled_reflection.lease.v1` and
`astrid.edge_scheduled_reflection.admission.v2` files. The steward validates
root/runtime-group and root/steward-group `0440` DAC, the root:root `0755`
parent, exact boot, systemd `INVOCATION_ID`, active generation, nonce-bound
lease ID and payload hash, equal ACK barrier sequence, ACK hashes, and the
persistent model-lock device/inode. The marker also binds the exact due nonce
and distinguishes fresh-model authority from prepared-transaction recovery.
The latter can complete exact signed authored state during the two-hour floor
but fails the separate model-start check. The admission marker is not the IPC
barrier: that independently uses `astrid.edge.maintenance_barrier.v2`.

The steward rechecks this exact pair after the due decision, after acquiring
the model lock, before the first provider request, and before and after every
provider step. Missing, expired, linked, foreign-owned, unknown-field, changed,
or cross-kind evidence fails closed and cannot create authored output. The
root unit's `ExecStopPost=+` removes only evidence for the same invocation.
Neither the reflection lease nor marker can be interpreted as generation
activation authority.

Production `run_once` unconditionally uses this validator; there is no CLI,
environment, or configuration bypass. A doc-hidden test entry point is compiled
only when `debug_assertions` is enabled so integration fixtures can exercise
the model pipeline without writing root-owned `/run` evidence. Release
appliance builds do not contain that entry point.

## Scheduled introspection compatibility projection

### Mutually exclusive information-flow lanes

Every due slot begins with a rich introspection pass. Before its first provider request, the helper
programmatically retrieves bounded projections for exactly five configured owned categories:
continuity, self profile, verified evidence, the latest machine observation, and spectral/host
state. It also projects the latest prior scheduled reflection when one exists. Missing inputs are
explicitly unavailable; links and malformed configured paths fail closed. Growing JSONL evidence
is tailed before typed provenance filtering, and transport recovery is excluded by typed fields,
not substring matching.

The rich pass may use only bounded owned introspection plus the optional immutable read-only web
broker. Source inspection and every candidate operation are rejected at the native authorization
boundary. Its exact complete response is durably checkpointed and remains the scheduled authored
reflection even if later source review fails. Only one exact final line, `SOURCE_REVIEW: REQUEST`,
requests a clean source-review pass; embedded, repeated, or non-final markers do not.

The clean pass retains the same due nonce, trace, and session, but receives a new turn/span and a
fresh prompt containing only revalidated signed source/build/generation facts and the fixed review
question. It may use only bounded source/build/generation inspection and candidate tools. Owned
artifacts, continuity, machine observations, and web tools are rejected. Candidate authority binds
the clean turn, response hash, clean provenance digest, and exact terminal declaration; syntax
repair is never accepted. `source_authoring_output_tokens` is a distinct configured ceiling from
the rich reflection output cap.

The rich and clean passes share one ceiling of eight provider calls. A signed rich checkpoint and
signed clean-start marker make recovery monotonic: restart or partial/failed clean generation keeps
the rich response authored, labels the clean outcome non-authored, reopens any prepared draft, and
never recalls the model for that due slot. Private records expose hashes, lane, bounded tool names,
and terminal status; untrusted bodies never become candidate authority.

In addition to its signed private receipts, the helper emits the existing reporting projection:

- `workspace/runtime/scheduled-introspection/projection/state.json`, schema `astrid_edge_scheduled_introspection_state_v1`;
- `workspace/runtime/scheduled-introspection/projection/continuity.json`, schema `astrid_edge_scheduled_introspection_continuity_v1`;
- `workspace/introspections/scheduled/receipts.jsonl`, schema `astrid_edge_scheduled_introspection_v1`; and
- `workspace/introspections/scheduled/reflection_<due>_<turn>.md` plus a provenance sidecar.

An authored receipt has `status=authored_completed`, `provenance=model_authored_runtime_scheduled`, `continuity_admitted=true`, a UUID trace/turn/span, and an exact lowercase response SHA-256. The reflection file is the exact model response. The continuity file contains only a deterministic maximum-320-character summary plus appliance/model/due/prompt/response/summary hashes and trace linkage. Failures and partial generations remain only signed non-authored helper receipts and cannot populate these authored projections. The helper owns only the `projection` sibling; runtime admission state is a distinct runtime-owned sibling invisible to the helper.

Completion advances from the original cadence anchor to the first two-hour boundary strictly after
completion. A distinct two-hour model-start floor still applies after any attempted generation, so
long reflections neither drift the schedule by `completion + 2h` nor permit immediate retries.

The installer, not the helper, creates the projection, scheduled-introspection, and patch-outbox directories as steward:`runtime` mode `0750`. The helper runs with `Group=runtime` and `UMask=0027`, emits only steward:`runtime` mode `0640` files there, and rejects ownership or mode drift. This gives runtime read access without granting it write access; the helper never creates, chmods, or chowns these shared directories.

## Candidate patch body

`candidate-patch-<sha256>.json` uses `astrid.edge_self_change.full_replacement_patch.v1`. It binds candidate ID, signed source ID, base generation, and a sorted list of `{path,source_sha256,content_sha256,content}`. Paths derive only from mutable IDs in the signed source inventory. The broker rejects traversal, hidden paths, symlinks, hardlinks, devices, binary/non-UTF-8 or display-ambiguous control content, stale hashes, immutable origins, more than 25 files, more than 4,000 deterministic insertion/deletion line edits, or more than 8 MiB aggregate replacement text. The authoring tool accepts hash-bound sorted, non-overlapping `{start_line,end_line,replacement}` hunks using one-based half-open ranges; equal bounds insert and `line_count+1` appends. This lets a small model avoid reproducing a large file; the durable patch still contains each complete resulting file for independent replay. Submission re-reads the root generation binding and revalidates every source/content hash, count, and dependency against the same cumulative signed snapshot.

Signed files with origin `inspect_only_immutable_boundary` are available to `list_source`,
`search_source`, and bounded `read_source_chunk`. This lets a scheduled reflection inspect the
rescue/steward/broker/checkpoint implementation and export proposals about it. That origin is
categorically rejected by `mutable_entry`, patch application, submission, and the independent
rescue verifier. The snapshot contains reviewed source and policy templates, never keys, host
configuration, operator state, or runtime secrets.

## Architecture-health note for large modules

`candidate.rs`, `runner.rs`, `source.rs`, and `web.rs` intentionally exceed the repository's 1,000-line
review signal. `candidate.rs` is one authenticated, crash-recoverable draft/edit/prepare/publish
state machine; its line-hunk and edit-distance helpers remain adjacent to the exact limits they
attest. `runner.rs` retains the gate/lock/provider/tool/attestation/finalization ordering as one
auditable crash-safety invariant; pure prompt construction and context accounting were extracted
to `prompt.rs`. `source.rs` is the signed bootstrap/derived source lineage verifier and
keeps visibility, inspect-only/mutable role classification, vendor/lock policy, and tree sealing
together. `web.rs` owns one bounded signed Unix-socket request/response protocol, including
framing, deadlines, peer identity, and untrusted-result validation. Focused tests live with each
gate. Future extraction should be limited to pure helpers so no authority-bearing parser or
transaction rule is duplicated across modules.

Candidate-tool arguments are exact schemas; even nominally empty tools reject additional fields. The authorization boundary exposes no command, shell, process, environment, arbitrary path, or network primitive. Read-only web tools and candidate-authoring tools are mutually exclusive within a reflection, so fetched untrusted text cannot feed a same-reflection candidate or terminal intent. `submit_candidate` moves the authenticated private draft only to `Prepared`; it publishes no patch or authority. Only the exact terminal line from that same direct model session creates a signed authored transaction binding the appliance, due slot, trace/session/turn, model, response and terminal hashes, cumulative source, generation, candidate, patch, proposal, prepared-draft digest, and exact nested intent/binding bytes. Reflection and summary persistence are followed by the signed completion marker; only then may `intent_publication_prepared.v1` publish the patch, binding, completed wrapper, submitted draft, and hash-chained committed record. Recovery uses identifier/hash equality rather than timestamps. If the supervisor has already ingested, built, or activated the candidate, the steward finishes and verifies the exact publication from the retained signed authored/publication transactions without consulting the new active source generation. Duplicate, missing-after-commit, replayed, mismatched, or conflicting projections fail closed. Without the exact terminal line, the candidate is reopened directly and remains available at the next coalesced two-hour slot.

Service-template source IDs are mutable only when their signed origin is exactly `mutable_astrid_service_template`, their basename is one of the immutable updater's six exact base fragments, and their location is exactly `packaging/systemd/` or `packaging/systemd/icp/`. Build-required roles remain readable build inputs but do not grant candidate mutation authority, and the prior ambiguous mutable role is not accepted as mutation authority; origin text alone can never enlarge the service surface.

## Model unload/build-readiness handoff

Only after `candidate-intent-<envelope_id>.json` is durable, the helper makes one fresh no-retry `POST /api/generate` request containing canonical `{"keep_alive":0,"model":"..."}`. It writes the signed envelope `astrid.edge.steward_helper.model_unload_handoff_envelope.v1` to:

```text
helper_state/model-handoff/<envelope_id>.json
```

and appends the identical envelope to `helper_state/model-unload-receipts.jsonl`. Its core schema is `astrid.edge.steward_helper.model_unload_handoff.v1` and binds appliance/envelope/intent/candidate IDs, candidate digest, exact model/origin, request/result hashes, one-attempt/no-retry flags, and either `unload_confirmed` with `build_ready=true` or `unload_failed_build_deferred` with `build_ready=false`. It retains no response body. The supervisor must require the exact matching confirmed state before intent ingestion or build; a missing, invalid, or deferred record is not build-ready. Ordinary reflections never unload the model.

## Candidate lifecycle and owner export

The helper reads only the root-projected `astrid.edge_self_change.steward_status.v1` snapshot supplied as the `supervisor-status` systemd credential. Appliance and current-generation bindings must match. Nonterminal intent/build/staged/probation state defers and coalesces reflection. An exact matching terminal `accepted`, `rejected`, `rolled_back`, or `abandoned` state archives the signed draft and digest-bound patch under `helper_state/candidate-outbox/history/`, writes a two-phase signed reconciliation receipt, and only then clears the active draft. A private pending record makes the clear crash-recoverable without timestamp inference.

The same reconciliation writes two immutable steward-owned mode-`0640` files under configured `workspace/self-change/patch-outbox/`: a full signed patch export and a signed `.summary.json` capped at 16 KiB. The summary contains candidate/source/base/status, touched paths, file and line statistics, patch/full-export hashes, and no source body, diff, prompt, web content, or build log. Reports consume only the summary. Neither export is ever re-ingested or authorizing.
