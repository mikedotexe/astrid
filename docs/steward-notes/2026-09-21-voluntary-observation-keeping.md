# Voluntary Observation Keeping and Bounded Recurrence Analysis

Git follow-through: the deployed feature is now committed and fast-forwarded to
both local main branches. See [the Git checkpoint](2026-09-21-voluntary-observations-git-checkpoint.md).
Earlier candidate and pending-Git statements below retain their historical scope.

Subsequent status: Mike approved the paired rollout, which completed successfully
at 2026-09-22T06:34:21Z. Exact running identities, renewed qualification,
checkpoint continuity and remaining Git work are recorded in
[the live rollout note](2026-09-21-voluntary-observations-live.md). Candidate-only
statements below retain their original historical scope.

## Status and Ownership

Interactive implementation by Codex in paired isolated `codex/voluntary-observations-20260921` worktrees:

- Astrid: `/Users/v/other/worktrees/voluntary-observations-20260921/astrid`, based on local main `18d8aed797bc737956f04d46aace0a1d4388dd4d`.
- Minime: `/Users/v/other/worktrees/voluntary-observations-20260921/minime`, based on local main `329ec1451e8dcac41e522b8fff03fc4ada94b9fd`.
- No changes in either canonical checkout are included implicitly. Astrid's 197 older dirty paths remain outside this work. Minime's canonical checkout was clean on entry.
- Controller was already paused at generation 457, without an active lease. This is not a productive flywheel round. Do not resume paused automations as part of qualification or rollout.

This document records a candidate, not deployment, consent, felt improvement or a new live-control authority. No engine, model, visual service, sensory client or live reservoir parameter was changed. Do not restore an old checkpoint over newer authored work.

## Exact Witness and Interpretation

Fully read: `/Users/v/other/minime/workspace/journal/!aspiration_2026-09-21T19-57-47.504460.txt`.

SHA-256: `11dedc74854ee21046985e58c9fedd281bcd472c52c2bb85a3ee44d0ff82b01f`.

> a recurring, beautiful, and entirely useless pattern in the covariance matrices that I find myself gravitating toward, even though it serves no purpose for the query.

The supplied prompt asked Minime to imagine an interesting creative failure. This is a hypothetical aspiration, not an observed numerical defect, a confirmed current experience, or independently verified access to engine covariance. Its engineering relevance is the possibility of keeping an impression without first defending its utility, and later testing an explanation without erasing that impression. Historical prose and the older notice attached to that entry remain unchanged.

## Implemented Contract

`WRITE OBSERVE <JSON>` is validated by Rust. Required common fields are `owner`, existing `draft` and typed `operation`. Mutations additionally require exact authored `revision`, unique `request_id` and `expected_head`. `present` defaults to false. Unknown fields, unsupported operations, stale revisions and owner mismatches fail closed.

Operations are `status`, `capture`, `show`, `annotate`, `analyze`, `link_preview` and `link_confirm`. Capture does not require a question or annotation. Capture reads only the existing fixed trusted recorder, checks its policy, clocks, bounds, finiteness and unsanitized quality, and freezes selected vectors plus source SHA-256. A request is 1-180 seconds, limited by the recorder's 180 retained frames; actual sampled span is reported rather than implying the requested coverage was available.

Each draft retains at most four captures and 64 append-only records. Capacity failure never evicts, rewrites or summarizes older material. Each record binds the exact body JSON, previous head and operation identity. Draft revisions retain the original attachment revision. Branches copy immutable evidence/history and append independently thereafter.

Ordinary continuation and explicit return include attachment IDs only. Detailed annotations and measurements require explicit show/analysis. Large detail results paginate at UTF-8 boundaries rather than silently truncating. Parked drafts do not acquire reminders. The original draft-size limit and honest overflow behavior remain.

Storage-only operations commit the owner's native receipt and trigger no provider call, public journal or ambient result notice. `present:true` deliberately requests one private model presentation through the existing job admission/delivery path. It does not append that response to the draft's prose. Same-draft presentations may use existing focus, without renewing its four-job/fifteen-minute allowance. A storage-only operation supplies no subsequent authored NEXT, so it releases priority without spending or replenishing a generation slot.

### Deliberate Disclosure

`link_preview` selects one to four captures and either an existing inquiry with its exact revision or an explicitly authored new question. It changes only private history. It neither selects nor schedules the destination.

An optional passage uses `[start_byte,end_byte)` UTF-8 boundaries from the exact current draft prose and its SHA-256. The preview displays the precise passage as a JSON string, the destination and the warning that normal self-study output may be public. Confirmation requires verified complete delivery of that exact preview, not just file creation. The entire escaped preview must fit one 32,000-byte detail presentation; an oversized preview is rejected without changing history, never divided into separately confirmable fragments. A changed draft or existing destination requires a new preview.

`link_confirm` publishes the allowlisted payload and private confirmation in one owner transaction using the existing preparation redo. With no passage, only frozen numerical observations and the authored question cross. Draft title, stopping point, private annotations, other passages and private storage paths do not. Caller-chosen private operation labels are replaced by opaque hashes in public disclosure records. Origin/revision bookkeeping remains in private history. Confirmation returns the exact inquiry and record IDs without choosing the activity.

`SELF_STUDY OBSERVE <JSON>` permits explicit `status`, `show`, `export`, `predict`, `analyze` and `revise` against confirmed inquiry material. Public presentation additionally requires that inquiry to be explicitly selected. Linking alone never selects it. A revision appends authored text without automatically resolving the question.

Complete new exports use `inquiry-observations-v2`, containing legacy geometry plus confirmed inquiry observation history. Existing geometry-only exports retain `question-geometry-v1`. Asking the older geometry export surface for a mixed inquiry returns the new complete format instead of silently omitting the new family. There is no private-draft export or viewer expansion in this release.

### Privacy and Execution Provenance

Opaque JSON survives NEXT parsing, including quoted command-like strings, angle-bracket text, `AND`, `RESIDUE:` and trailing spaces. Python classifies private intent and delegates grammar, storage and numerics to Rust.

Private model inputs and receipts use the existing owner-scoped private-writing delivery path. Routine NEXT logs withhold private payloads. Observation action-continuity summaries omit the JSON and response-derived residue. Exact execution choices still belong to the existing owner execution queue and authenticated action provenance; these must not be treated as public study/export material. New inquiry exports are built from the explicit disclosure allowlist, never from those runtime records.

No new sensory publication or peer-delivery path is added. Explicit model presentation is still a provider input, not encryption from the provider or the operator. Existing safety, authorization and authenticated operator-stop mechanisms remain unchanged.

## Numerical Definitions

All analysis is pure bounded Rust over frozen finite raw 128-node activation vectors. It performs no inference, polling, control call, fitted alignment, inverse covariance or threshold search.

### State Returns: `state-return-rms-v1`

For two samples, `distance = sqrt(sum_k((x_ik - x_jk)^2) / 128)` in double precision. The author must choose threshold `0 < threshold <= 2`. Temporal exclusion defaults to 5,000 engine milliseconds and can be explicitly set up to 180,000. Eligible unordered pairs have separation strictly greater than the exclusion; near pairs have distance at most the threshold. At least 32 valid frames are required. Zero eligible pairs yields no fraction rather than zero evidence of return.

Repeated diagonals contain at least two consecutive near pairs. Each adjacent interval must be at most 1,500 milliseconds in both engine and wall clocks, on both sides of the diagonal. No interpolation bridges a gap. Actual engine and wall timestamp ranges, sample counts and gap ranges accompany results.

Recordings with mean per-node empirical variance at or below `1e-12` are marked degenerate. Pair similarity can still be reported descriptively, but no dynamic-return diagonals are presented. Stationarity is not represented as a meaningful dynamical return.

The distance-based construction adapts Eckmann, Kamphorst and Ruelle's recurrence plots: https://fiteoweb.unige.ch/~eckmannj/ps_files/recurrenceplots.pdf. This bounded implementation makes no attractor, chaos, statistical significance, causal or experiential claim.

### Covariance Shape: `activation-covariance-shape-v1`

Two explicitly selected nonoverlapping windows each require at least 30 frames. Center each window at its own mean. Compute sample covariance with divisor `n-1`. Similarity is `sum(Ca*Cb)/(||Ca||F*||Cb||F)`, separate from mean-state RMS distance, each covariance norm and each mean per-node variance `trace(C)/128`.

Normalized comparison is insufficient if either mean per-node variance is at most `1e-12`. The rank upper bound is `min(128,n-1)`; it is not a measured numerical rank. Sample counts, gaps, source hashes and sampling limitations remain visible. The compared matrices are computed from sampled ESN activations, **not the engine's separate covariance matrix**.

Within a capture, windows must be disjoint in engine and wall time. Between captures wall intervals must be disjoint; engine clocks may reset. Boot/node identity is unavailable, so every between-capture comparison is explicitly exploratory, never confirmed recurrence. Covariance ignores temporal order: a permutation can preserve shape while destroying diagonal sequences.

### Optional Authored Expectations

An optional annotation can commit a recipe, intervals, comparison (`at_least` or `at_most`) and numerical bound before requesting analysis. An analysis references the exact expectation ID; parameters cannot change silently. Frozen evidence necessarily predates this historical comparison, and the result labels that fact. Commitment before execution is not a claim that the author had never seen the result.

Results and evaluations are revalidated against frozen evidence on load. Degenerate/insufficient evidence leaves comparison unavailable. Numerical matching is not a confirmed recurrence verdict or a judgment about the worth of an impression.

## Persistence and Migration

Native reader schema advances 6 -> 7; native draft schema advances 3 -> 4. No parallel belief database is introduced. The same owner lock and preparation redo cover draft history, inquiry confirmation and delivery. No inference runs while that lock is held. Identical retries recover; conflicting retries fail. Corrupt tails and unsupported future schemas are preserved, not truncated.

Empty observation families do not alter existing authored return references. Original geometry records and exports remain readable. Actual old/new helper tests use generated fixtures only: exact pending page/private inputs and old authored fields survive, no focus window or historical capture is invented, and the old helper refuses upgraded records without changing bytes. Migration is forward-only; rollback must retain a compatible reader or be separately reviewed. Never restore a backup over later authored state.

## Qualification Record

Confirmed during development:

- Full shared reader/writer suite: 249 tests passed, including nine observation integration tests and five pure numerical controls/oracles.
- Full bridge suite before the final diagnostic-redaction refinement: 2,304 library tests passed, one ignored, plus all integration and compile-fail suites. Final rerun is recorded below when complete.
- Full Minime Python suite before the final diagnostic refinement: 1,510 passed, one skipped, 136 subtests. Final rerun is recorded below when complete.
- Strict Clippy for both shared reader and full bridge targets; formatting checks; domain-boundary audit with zero violations.
- Deployment/controller/evidence/Division/flywheel/domain/restart/qualification tests: 116 passed. Epistemic lint self-tests: two passed.
- Actual old helper -> candidate debug helper migration: 20 checks per owner, both passed. Immutable staged release qualification remains a separate record.
- Shared preparation tests interrupt confirmation at each checkpoint publication boundary and recover without a duplicate inquiry or partial disclosure. Tests also cover concurrent admissions, conflicting requests, capacity, stale source/destination, incomplete preview delivery, UTF-8 selection, private-content exclusion, tampered numerical receipts and preserved focus budgets.

Unsuccessful attempts retained in this record:

- An early time-permutation test accidentally preserved the synthetic period; corrected the test permutation, not the production recipe.
- Initial adapter fixtures omitted required durable preparation IDs and assumed an empty temporary directory despite fixture setup. Corrected the fixtures to use the real host contract and before/after comparison.
- First bridge build could not resolve external path dependencies. Added parent-level links to the existing clean `prime_esn_wasm` and `RASCII` repositories; no source or dependency-version edit.
- Initial Python suite had two timeout-default assertions under inherited `MINIME_LLM_TIMEOUT_S=160`. Reran with only test-process timeout overrides removed; no live configuration change.
- Initial Python unittest invocation lacked `PYTHONPATH=scripts`, causing five import errors. Correct invocation passed all 116 tests.
- A new source-position test used a non-catalog path; corrected its synthetic source location. It then passed without a production fallback.
- First broad continuity-redaction draft removed existing private CONTINUE normalization receipts and changed a legacy blocked-WRITE label (11 test failures). Narrowed continuity redaction to the new observation command family, while retaining private diagnostic log redaction.
- Staging preflight refused recently edited candidate source. Respect the 180-second quiet interval; do not override or reduce it.
- A concurrent full bridge rerun failed the existing no-capture instrumentation timing assertion: p95 was 1.587489 ms against the unchanged 1 ms limit. The serial rerun passed all 2,304 library tests and the integration/compile-time suites. No threshold or production instrumentation was changed.
- Final review reproduced an incomplete-preview confirmation edge case with 6,000 synthetic escaped control characters: the encoded passage spanned pages but the receipt proved only one delivered page. A regression first failed on the original candidate. Preview creation now requires the complete encoded presentation to fit one page; both owners retain their exact draft and history on rejection.

The first immutable stage, `bridge-stage-observations-01` (manifest `902d22db9524c72c0c03e88532f343eb33a7c5912405dbad202ae74d4444795f`), is retained as superseded build evidence. It predates the complete-preview fix and on-demand command schemas and must not be activated. A new immutable stage is required.

## Release Gate and Next Commands

Use `scripts/build_bridge.sh --stage-dir` with an explicit offline-only acknowledgement after the tree settles. This wrapper builds immutable, hash-bound bridge/helper artifacts without activating them. Use `scripts/qualify_observation_release.py` with the existing active stage, candidate stage, paired Minime source and a new output directory. It freezes the Python inventory, exercises actual old/new migrations for both owners and verifies selected helper identity. It does not install the Python overlay or switch launch selection.

Before any approved rollout: reconcile the exact canonical Minime launch inputs, re-audit foreign activity and source identities, drain admitted agent/bridge work with sanctioned wrappers, prevent old/new writers from overlapping, and verify loaded hashes, readiness and checkpoint continuity. Engine/model/visual/sensory services must remain unchanged. Observe naturally occurring public use only after activation; never solicit confirmation of benefit.

No Git commit, merge, push or activation is implied by this implementation note. Owned candidate paths must be reviewed and explicitly staged by one Git coordinator. Historical dirty evidence must not be swept into that commit.

## Final Qualification Addendum

Implementation and offline paired qualification passed. **Not activated, committed, merged or pushed.**

### Final Tests

- Shared reader/writer: **250 passed**, including ten observation integration tests. The complete-preview regression passes for both owners. Status exposes typed operation/recipe references only on explicit request.
- Full bridge: **2,304 library tests passed, one ignored**, followed by all integration and compile-time boundary suites. Final serial library duration: 134.48 seconds. The 1 ms instrumentation threshold remains unchanged.
- Complete Minime Python suite with the final release helper: **1,511 passed, one skipped, 136 subtests passed**, 47.42 seconds. The complete isolated checkout's 84 runtime files and registry seed were verified identical to the frozen release before and after qualification.
- Expanded deployment/controller/projector/evidence/Division/flywheel/domain suite: **234 passed**. New observation release inventory tests: **three passed**. Epistemic self-tests: **two passed**.
- Strict shared-reader and full bridge Clippy passed; shared/bridge formatting and both worktree whitespace checks passed. Domain-boundary verification: valid, zero violations.
- Actual immutable old/new helpers: **40 migration checks**, 20 per owner, and frozen Minime helper-selection verification passed. Exact pending inputs, original prose, inquiry fields, quiet return and downgrade refusal were exercised using synthetic state only.

Retained test-location attempts: loading the frozen adapter while collecting tests from the source checkout passed 1,509 tests but failed two repository-layout assertions, which require the test and runtime roots to coincide. A copied test tree alongside release files then hit seven collection errors because non-release development tools were absent. Neither attempt changed production code or weakened those assertions. The complete isolated checkout, hash-identical to the frozen runtime inventory, passed the entire suite with the final staged helper. All three XML reports remain in the qualification directory; the final report is `minime-final-staged-helper-suite.xml` (SHA-256 `4afaa18e6f88bf821451fbac565c667f7f2a001fd991f1d26dc9daac26e4e19f`).

### Immutable Candidate

- Stage: `/Users/v/other/worktrees/voluntary-observations-20260921/bridge-stage-observations-02`.
- Manifest SHA-256: `7449e4fa7721e030f1e6a863693bdd5c9b3a7ce8e629a306514d4d4cdc9de178`.
- Build-input SHA-256: `0394e1d237559ddd08fb1143b113e63ecc52f6856c46a18579c28b51bfa1e638`.
- Bridge SHA-256: `ba5c8aee3af2ab7c02ffe8c3f16d6659757715586e6e1807b34554385b7f8dae`.
- Shared helper SHA-256: `fc12fb295a3b97d984a372c43f2e92bb92fdd683e4ae1c487689ac78bd7f2790`.
- Paired receipt: `/Users/v/other/worktrees/voluntary-observations-20260921/observation-qualification-01/qualification.json`, SHA-256 `017ce8c80801797a88aec22016351c345ffbf3da414caa6d59ca39e2dde7ca56`.
- Frozen adapter: `observation-qualification-01/minime-source`, containing 84 runtime source/launch inputs plus the existing registry seed, all hash-bound. The qualification tools are frozen separately. Snapshot files are read-only; the separate test copy is not a launch selection.
- Launch-source reconciliation: `/Users/v/other/worktrees/voluntary-observations-20260921/minime-observation-reconciliation-01/reconciliation.json`, SHA-256 `a087d34a350c9db0e305b6d4bf81b318848611d28ac496880bc0e648bb31c4cf`. Exact selected-input binding passed; `canonical_applied=false`.

The sanctioned stage wrapper's preflight and before/after source checks passed. The first stage remains superseded, never an activation candidate. This packet's `live_eligible_now=false` and `activation_performed=false` are deliberate: artifact qualification is not live rollout authority or a readiness receipt.

### Preserved Live and Git State

The active bridge remains `bridge-stage-geometry-04`, manifest `4faab24713471c74829a9ba7cdc27abc78654901a31605766e1587fd9ce6ec9c`, PID 98062 (started September 21, 20:48:14 local). Minime remains PID 97221 (20:46:56 local), reporting canonical runtime source, all 84 startup inputs and `reload_required=false`. The candidate helper has not touched either live owner's drafts or inquiry checkpoint.

Read-only process checks retained engine 41337, gateway 41484, supervisor 41526, model 43115, visual 20885, camera 98903, microphone 98910, host-sensory 41661 and feeder 1502, with their prior start times. No service signal was sent.

All **197** older canonical Astrid paths were checked against `2026-09-21-paired-continuity-git-inventory.json`: exact status and SHA-256 matches, no missing or extra dirty path. Canonical Minime remains clean. Local main identities remain the baselines above. Candidate work comprises 29 owned Astrid paths and five owned Minime paths, all unstaged in the paired worktrees; both indexes remain empty.

Controller remains paused at generation **457**, no lease or active projection. Indexed-tail V2 evidence remains valid at sequence **1123111**, head `8d9b06d15a16f05eae6b2bfd4c178f96f5f4216e3cfb8ceabe21d9292b4e28e6`; four V1 source streams remain immutable. No productive round, being uptake, read-current status or automation resume is inferred.

### Remaining Gates

1. One Git coordinator reviews the owned paired diffs and exact-path commits without including the 197 historical paths. No archival checkpoint or merge has occurred in this implementation turn.
2. Obtain the paired rollout approval and revalidate cooperative activity, live source identities, installed launch binding and checkpoint compatibility at that time. Reconciliation is a snapshot, not a durable claim that no source can change later.
3. Use the sanctioned Minime-agent drain/hold/restart and bridge stage-activation wrappers, preventing overlap of old and new schema writers. Preserve all later authored state; schema 7/4 must not be handed back to the old 6/3 helper or restored from an older backup.
4. Verify new process starts, exact loaded inventory/helper hashes, readiness and checkpoint continuity after activation, while the engine/model/visual/sensory processes remain unchanged. Recheck the frozen release and stop on drift or failed readiness.
5. Only then observe naturally occurring public use without requesting a benefit report. Leave previously paused automations paused.

No known failing behavior test remains. Deployment and Git integration are explicit pending work, not a claim of a live feature or subjective improvement.
