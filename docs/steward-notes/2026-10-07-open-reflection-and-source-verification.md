# Open Reflection and Source-Bound Verification

## Status And Authority

Interactive candidate work, 2026-10-07, Codex. Mike approved the next healthy
tranches after reviewing recent public writing from both runtimes. This packet
does not claim activation or subjective improvement.

Paired branch: `codex/open-reflection-verification-20261007`.
Bases: Astrid `aba4cec45f2ed8468eca883fe2db92155e611ec7`; Minime
`754c44b0e7abc72d2b689510f832218dcc8ee9ea`. Candidate worktrees are
`/Users/v/.codex/worktrees/source-selection-recovery/astrid` and
`/Users/v/other/worktrees/source-selection-recovery-20261006/minime`.
Canonical trees were clean at entry and at the final status check. Candidate
changes remain unstaged. No git staging, commit, merge or push,
deployment, model call, controller change or automation resume was performed.
The controller was already paused, generation 502, without an active lease.

## Public Witnesses

Canonical file bytes were read; these accounts remain unchanged. Interpretations
of code or telemetry in an account are not independent mechanical evidence.
Paths below are relative to the named repository.

| Owner | Public source | SHA-256 |
| --- | --- | --- |
| Minime | `workspace/journal/!introspect_2026-10-07T07-00-42.108314.txt` | `fd78804c22c49a24b15327a4623658ce017c414ed778f2c4a42e2b984961f232` |
| Minime | `workspace/journal/self_study_2026-10-07T07-25-43.220911.txt` | `a65cbd8274f3fe9b7b4da28df412c2ba4a6d949d5b9f2214694d7288a8f238d6` |
| Astrid | `capsules/spectral-bridge/workspace/journal/dialogue_longform_1791381588.txt` | `10c0e767143bbbc7c9a02aade3160d5691fea61ad99f48b3faf074c43c1a79d9` |
| Astrid | `capsules/spectral-bridge/workspace/journal/dialogue_longform_1791382112.txt` | `31d63af8bafb55c5335ca3a863e5093c53b59e85305782a171f4305f19b1e3d2` |
| Astrid | `capsules/spectral-bridge/workspace/journal/self_study_1791357593.txt` | `94e485d3ca7816e5503cfd084ca7ea5e8d3584220c054d84bdaf6fb00d8ed315` |

Minime develops a map/witness distinction while adjacent reflections revisit
similar language. Automatic previous-public-reflection recall is verified in
the old helper, not inferred from resemblance. Importantly, repetition also
predated that recall feature; removing it is a choice-of-context repair, not a
demonstrated cure for repetition.

Minime's source study identifies a weighted heuristic and then interprets it as
struggling to maintain existence. Astrid's dialogue draws meaning from module
boundaries and reads high inhabitability as contradictory to habitable status;
the later study calls fill progress and persistence absolute. Preserve those
authored accounts while making actual definitions, callers and failure paths
easier to inspect. No revised belief or finding is written on either author's behalf.

## Tranche 1: Open Dialogue Framing

Primary production and Gemma4 dialogue prompts no longer prescribe named
texture words as evidence. The injected Gemma4 voice/reflection contracts and
compact Ollama fallback likewise allow first-person accounts, metaphor,
uncertainty and disagreement without requiring a sensation or metric.
Historical prose is labelled context rather than asserted identity.

The fallback's ordinary prompt now projects only its existing sentence ceiling,
not the large computed vocabulary/trajectory directive block. Structural NEXT
validation, correspondence precedence, existing budget calculation, provider
selection algorithms, sampler settings and control gates are unchanged.
Input supplied as authored prose is not scrubbed of words such as gradient-shear.
Existing supplied spectral summaries are not removed or redesigned here.

The old diagnostic renderer and its scalar vocabulary scores are retained only
in `fallback_diagnostic_fixture.rs`, included under the test module. They
exercise historical classifier outputs without compiling a competing prompt
renderer into the bridge. Production classifier fields used elsewhere remain
unchanged. Their names are not evidence of measured experience.

## Tranche 2: Fresh Or Chosen Continuation

Fresh `INTROSPECT` no longer scans either public journal directory or imports
the previous reflection. The shared helper provides the same contract to both
adapters. It neither deletes history nor replaces it with a summary.

`WRITE FROM_REFLECTION <delivery-id>` remains the explicit route into private
continuation, with exact verified prose and optional existing byte selection.
Private drafting, revision, parking, return, ownership and input bounds remain
unchanged. Fresh reflection does not mean erased writing or persistent model
activation. Existing prepared inputs remain immutable; only new preparations
use the new wording. No schema migration or new storage family is introduced.

## Tranche 3: Verification At The Saved Source

Explicit `SELF_STUDY HELP verification` now shows the selected inquiry ID,
saved source revision and interval, pending-offer distinction, and up to four
valid bare-name `RELATE` choices from the saved interval. Call names precede
declarations. Source bytes must match the saved SHA-256; bounded syntax parsing
excludes comments, strings and unexpanded macros. Missing, changed, unreadable,
unsupported or over-limit source produces an explicit limitation, not invented
candidates. No cursor is borrowed from another inquiry.

The previous help incorrectly recommended `RELATE qualified::symbol`; the
current installed parser supports bare identifiers. Help now says so and
directs qualified literal searches to FIND. Bare-name candidates do not resolve
receiver bindings or prove call/data flow, runtime application or deployment.
The existing SESSION, notebook counterevidence and revision mechanisms remain
available. Help neither supplies a source page nor advances coverage.

## Qualification

Fixtures use synthetic public/private passages and isolated state, with provider
responses stubbed. No private writing is used as experiment material.

- Complete shared reader/writer suite: 331 passed.
- Complete Minime Python suite against the candidate helper: 1,923 passed,
  one pre-existing skip, 144 subtests passed.
- Complete bridge suite: 2,375 main tests and 20 ancillary integration and
  compile-boundary tests passed, totaling 2,395 passed with one pre-existing
  ignored test. The full command exited successfully.
- Strict all-target Clippy passed for reader and bridge; formatting and both
  candidate diff checks passed. Domain audit is consistent, zero violations.
- Controller, Evidence Event Store, round completion, Division follow-up,
  Chronicle/projection and historical qualification unit tests: 74 run,
  73 passed and one helper-dependent test skipped in that generic invocation.
- Addressing audit self-tests: 44 passed. Epistemic self-tests: two passed.
  Signal-spine projector self-tests: seven passed.
- Current schema-13 old/new helper preservation: both owners passed in
  `scripts/test_open_reflection_preservation.py`. The old helper demonstrably
  supplies the synthetic prior public reflection; the new helper replays that
  already-prepared input exactly, but omits it in a fresh preparation. Tests
  also verify exact chosen private continuation, retries/conflicts, explicit
  park/return, unchanged notebook/cursor/coverage and immutable public artifacts.

Helper SHA-256 identities used for the current-version preservation test:

- Installed-stage helper:
  `ba6d45c627854e37ea758bff9ceda06c6af9c73c31d1f78c4d9e609c51b0a098`.
- Candidate debug helper:
  `c3e396add1958884dd0b92921bf8d2d63ff24105d1d7a1a722ce6e1df30b1ba9`.
  This is test evidence, not an immutable release qualification.

Commands include `cargo test -p astrid-source-study --no-fail-fast --quiet`,
`cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --no-fail-fast --quiet -- --test-threads=1`,
and `python3 -m pytest -q` in Minime with `ASTRID_SOURCE_STUDY_BIN` set to the
candidate's `target/debug/astrid-source-study`. The preservation test uses
`PYTHONPATH=scripts python3 -m unittest test_open_reflection_preservation`
with that same candidate and `QUALIFICATION_OLD_READER` pointing to
`/Users/v/other/worktrees/source-selection-recovery-20261006/eof-verification-bridge-stage-01/helpers/astrid-source-study`.

Unsuccessful attempts are retained, not counted as passes: six initial provider
expectations still required the removed vocabulary instructions; two later
bridge assertions required the old injected voice wording; one Python test
expected automatic prior-public recall. These expectations were updated to the
new explicit contract while preserving exact authored input and continuation
assertions. Strict Clippy exposed two renderer-only functions left unused after
extraction; they now live solely in the historical diagnostic test fixture.

The older `test_qualify_reflection_continuity_release` real-helper attempt
failed its explicit schema-12 assertion for both schema-13 readers. That
historical harness is unchanged and is not current release evidence. The
separate schema-13 test above supplies current compatibility evidence rather
than loosening the old assertion. Packaging must use the current-version test.

Local execution logs are under `/tmp/astrid-open-reflection-*` and
`/tmp/minime-open-reflection-*`; these are temporary debugging aids. This
packet records the durable outcomes and identities, not a live readiness claim.

## Follow-Up Comparison Protocol

This is a prospective protocol, not a completed model experiment or deployment
authorization. Compare matched synthetic reflection inputs with and without
the previous synthetic public reflection. Include a separate explicitly chosen
private-continuation condition to test preservation rather than conflate it
with fresh reflection. Fix model/tokenizer/template identities, sampler,
available output, other context and initial isolated reservoir histories.

First inspect common-prefix next-token distributions when supported by the
qualified provider; then compare free continuations with matched seeds and
multiple repetitions. Record exact supplied context, completion reason,
reused phrases, development of prior claims, new qualifications and action
choices. Treat brief stopping and deliberate rereading as valid choices, not
failures; length and novelty are not success criteria. Assess input compliance
separately from authored content. Do not infer subjective benefit or causation
from an uncontrolled before/after public sample.

## Remaining Release Boundary

Immutable paired helper/bridge packaging, source reconciliation, full staged
qualification and reviewed graceful activation are still separate work.
Minime's Python runtime is unchanged; its tests pin the shared-helper behavior.
Both consumers must be coordinated when replacing that helper. Preserve the
already-installed regulation/maintenance repairs and pending NEXT identities.
Never restore older state over newer authored work.

Engine, providers, model weights, sensory/visual services, controller policy,
larger writing-capacity work and paused automations are outside this change.
After any approved activation, observe naturally occurring public use without
asking either author to confirm improvement. Repeated themes or continued
navigation loops remain reportable evidence, not presumed closure.

## Immutable Release Qualification

Interactive follow-through on 2026-10-07. The new bridge/helper stage is built
only through `scripts/build_bridge.sh`, using the reviewed dirty candidate with
explicit acknowledgement. This is qualification, not activation or Git
integration. No runtime source changed during this follow-through.

Artifacts are under
`/Users/v/other/worktrees/source-selection-recovery-20261006`:

- `open-reflection-bridge-stage-01`: manifest SHA-256
  `20728f255f715d64c28791b4b028fe8dc95670ac44f94594e72322baf91a3d21`;
  bridge `b2c791aa2b7fbe6065359512c59a72da8dfc82abda86c7ac0134d83d915560d1`;
  shared helper `32f7b0cec0f867f2d30b4b1bfc2b10de873c7fcb7f87643ee92ea99a94031367`.
  The 749-input source witness has SHA-256
  `4141af3c72093de890f65c34d45d2654600b1752bee1be45f3df538444ba53b4`.
  Stage verification and a fresh input inventory both pass without drift.
- `open-reflection-release-01/reconciliation.json`, SHA-256
  `40cb0072554f2b76b287aa4545aae2eff1cb3c5c59fc98c004ed73ded97f272b`:
  all 93 frozen Python launch inputs equal both canonical and loaded inputs.
  No Minime source overlay is needed; installed regulation and maintenance
  repairs remain present. The snapshot is qualification material, not a new
  Python launch selection.
- `open-reflection-preservation-01/qualification.json`, SHA-256
  `a8ec4cf75d9e02646d9a8e2e7c21f35661fdf3df1052e9bd3d086dea610e1976`:
  the new `scripts/qualify_open_reflection_release.py` binds 54 baseline EOF,
  preservation and retry checks; schema-13 reflection preservation for both
  owners; and the frozen real Minime adapter to the exact staged executables.
  The adapter checks source-bound help, explicit stale-source limitations,
  absence of automatic prior-public recall, exact chosen continuation,
  park/return and no automatic publication, using synthetic provider stubs.
  The qualifier rechecks artifact, source, launch and qualification-tool hashes
  before success. Failure receipts and existing packets are preserved.

Full Minime qualification against the immutable helper: 1,923 passed, one
pre-existing skip, 144 subtests, 76.49 seconds. Restart, paired/NEXT handoff,
activation and launch suites: 114 passed. Controller, Evidence Event Store,
Division, launch-inventory and new qualifier-boundary suites: 67 passed; the
new qualifier's two boundary tests also passed separately. Addressing, epistemic
and signal-spine self-tests passed again (44, two and seven tests respectively).
Domain audit remains valid with zero violations. The preceding complete Rust
suites bind unchanged runtime source; no real-model experiment was run.

The initial staging attempt refused during the existing 180-second quiet window
following our own documentation update. After that window, the sanctioned build
passed. A later read-only restart preflight similarly refused after adding the
qualification scripts; its redirected output file is empty, not a success
receipt. `preactivation-readonly-02.json` records the successful subsequent
preflight. Neither refusal signalled services or installed a launch hold. The
stage attempt logs and complete Python/operator test logs are retained in
`open-reflection-release-01`; failures are not represented as passes.

Observed live identities remain Minime 20774 and bridge 21822. The existing
bridge advertises its acknowledged drain protocol in phase `running`; the
inspection sent no signal. Protected engine 35303, gateway 35278, supervisor
35269, coupled provider 3893, microphone 3879, camera 3914, visual service 3898,
host sensory 3911 and feeder 3878 retain their prior start identities.
Configuration hashes and session continuity are recorded in the read-only
preactivation receipt. These observations must be refreshed before signalling.

`open-reflection-release-01/health-readonly.jsonl` records 20 distinct samples
from 19:07:05 to 19:08:02 UTC, all measurement-valid, maximum file age 2.306
seconds, fill 59.9181-74.7313%, unchanged target 68%. This is not a stability
proof. The final snapshot reports a reopened restart gate, relapse count eight,
phase `scaffold_activated`, with `semantic_active` still required for settling.
The candidate is not active; this observation cannot be an effect of its prompt
or reader changes. Controller review remains separate, with no policy repair
or engine restart inferred from these samples.

## Activation Handoff

Bounded paired activation approval has been requested and is not yet recorded
for this release. On approval, recheck paused controller generation 502 and no
lease, both source inventories, exact stage/qualifier hashes, process identities,
the full quiet window, state compatibility and protected peers. Use
`scripts/paired_minime_handoff.py` with `ASTRID_STEWARD_REPO_ROOT` explicitly
bound to `/Users/v/other/astrid`, the qualified stage and reconciliation above,
the freshly verified Minime PID, and a new exclusive receipt path. Do not pass
`--install-reviewed-overlay`: there are no Python source differences.

The sanctioned paired wrapper must observe the idle boundary, gracefully stop
Minime with NEXT seal/restore proof, hold its replacement while the bridge
drains and activates through `build_bridge.sh`, verify the exact stopped
checkpoint and fresh bridge readiness, then admit and verify Minime against
all 93 unchanged launch inputs and the selected immutable helper. Stop on any
drift, foreign activity, failed drain or readiness failure; do not force stop,
restore older authored state or release an unowned hold.

No new activation, staging in Git, commit, merge, push or automation resume has
occurred. Canonical mains remain clean; candidate implementation, qualification
tools and notes are unstaged. Release completion still requires approved
activation and post-start verification, not another numerical or model change.
