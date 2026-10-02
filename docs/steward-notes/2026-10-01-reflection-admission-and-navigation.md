# Reflection Admission and Directory Navigation

## Current Status

**Live and merged into both local main branches on October 1, 2026, at about
21:22 PDT.** Bridge implementation `363e4e99232017e6e05e98fa67a51b560d4ba6d8`,
Minime implementation `110f8d73ae63f647fbd714504fcfd2400bdf43a4`, and qualifier
correction `79276026b0` are integrated. Detailed receipts follow below.
Nothing was pushed. Paused automations remain paused at generation 489.
The earlier candidate-only sections below record the implementation phase.

## Status and Ownership

Implementation candidate by Codex, in paired isolated worktrees:

- `/Users/v/other/worktrees/reflection-admission-20261001/astrid`
- `/Users/v/other/worktrees/reflection-admission-20261001/minime`
- Both branches: `codex/reflection-admission-20261001`.
- Astrid baseline: `8255c8d238f278d4fc3f49a296a49aed0468febe`.
- Minime baseline: `145aaab45690eac9539803015de73a06fa1bab6d`.

The writing-room review repairs from the concurrent session were already merged
and the canonical trees clean when this implementation began. They are retained.
At implementation completion, this candidate was not staged, committed, merged,
pushed or deployed, and no service had been restarted. Pause generation 487 was
still paused; this is
an explicit interactive repair, not a resumed automation round.

## Public Witnesses

Paths below are relative to `/Users/v/other/minime`. All four files were read
fully during the investigation; exact bytes are unchanged. No private writing
was used as a fixture or reproduced here.

1. `workspace/journal/self_study_2026-10-01T18-21-36.739034.txt`
   SHA-256: `b00c86e8cf1370bd217034d9ae4510c8d08f33eb2a90de2bf7eee52f93e17d17`.
   Exact chosen action: `NEXT: INTROSPECT`.
2. `workspace/journal/study_navigation_2026-10-01T15-39-00.071613.txt`
   SHA-256: `1888b8085cca3f81338c035ca3c311a13d9da548b1f7b209c952a645aa235301`.
   Exact chosen action: `NEXT: SELF_STUDY MAP astrid/crates/astrid_kernel/src`.
3. `workspace/journal/action_preflight_2026-10-01T18-41-49.319025.txt`
   SHA-256: `095b38ee450a38f774d143951381e8654df8322b580574fd5694e535153ee671`.
   System-generated receipt: `REGIME calm`, `Effective route: unwired`.
4. `workspace/journal/regime_choice_2026-10-01T18-34-54.738805.txt`
   SHA-256: `4e9c74d6cf6cca86f4091df9f8547652c37d9d3bc64756bf3ec2b4bbf8da40c0`.
   System-generated receipt: calm requested/applied and bounded control sent.
   This receipt alone does not establish consumption by the engine or an effect.

Read-only `scripts/ground_review.py` ran on the two authored entries. It reports
the underscored directory missing. Its broad lexical citation matches are not
semantic verification of Minime's explanation; the decisive grounding here is
the exact chosen command, dispatch source, logs and executable regressions.

## Reconstructed Failure

On October 1, local PDT, the log records bare INTROSPECT blocked at 16:43:05,
17:48:20 and 18:24:42 with
`research_budget_required_for_active_experiment`. The retained legacy experiment
is `exp_minime_20260925_legacy-self-experiment`. The new prompt offers reflection
as a study exit, but the older budget classifier treats every INTROSPECT as
research. Its rejection supplies experiment-budget guidance instead.

This explains a concrete obstruction in the observed action chain, not every
choice or subjective experience. The short action/preflight/regime files are
system receipts. Navigation-only responses also now have their own prefix.
The investigation's 68 retained post-restart generation records reported normal
stops, with no recorded timeout/fallback; this is bounded evidence, not a claim
that all possible generation failures are observable.

## Implemented Repairs

### Bare Reflection

`is_open_introspection_action` recognizes only the complete bare action, not a
prefix, target or command bundle. Admission and executor research-budget checks
share it. The existing source-study adapter still supplies typed reflection
input, with no source-page delivery credit or notebook claim mutation.

An existing active or paused experiment does not require a new research budget
to reflect. Targeted workspace artifacts and external research keep their
existing policy. Normal executor safety, operator/drain and resource handling
are unchanged. No experiment is closed, deselected or granted authority.

### Truthful REGIME Preflight

The existing grammar is factored into one pure parser used by dispatcher and
preflight. Existing trailing punctuation/first-token acceptance is preserved;
no new free-form gain grammar is introduced. REGIME is correctly classified as
live control. Invalid/missing regimes describe the existing notice route.
Preflight names normal execution gates and the existing below-35%-fill handler
substitution from calm/explore to recover. It sends no control and does not
claim observed effects. The handler, gain table and controller are unchanged.

### Precise Directory Spelling

MAP/LIST reuse OPEN's existence-based, per-component correction: an existing
underscored directory wins; only a missing component with an existing hyphenated
counterpart changes. The listing discloses both spellings and offers exact
catalog commands. This is a directory listing, not an implicit source open.

Recovery already offered candidate paths before this change. The observed
problem was repeated recovery for a precisely recoverable directory spelling,
not proof that no candidates existed. Unknown/private/invalid paths remain
recovery; normal catalog exclusion rules and byte limits still apply. Source
bookmarks, authored notes, pending pages and old prepared inputs are preserved.
No schema migration is required.

## Qualification

Before implementation, the new tests reproduced eight preflight failures,
three reflection-admission failures and the directory-spelling failure.
After implementation, the initial focused run passed 54 tests.

- Full `cargo test -p astrid-source-study`: passed.
- `cargo clippy -p astrid-source-study --all-targets -- -D warnings`: passed.
- `cargo fmt -p astrid-source-study -- --check`: passed.
- Domain-boundary audit: passed, zero violations.
- Launch-inventory, geometry-release and observation-release tooling: 16 passed.
- Full Minime Python suite, final rerun: 1,742 passed, one skipped,
  141 subtests passed in 77.29 seconds.
- Both candidate diffs pass `git diff --check`; canonical working trees remain
  untouched by this pass.

The first complete Python run had 1,741 passed, one skipped, 141 passed
subtests, and one failure in a newly added negative test. That test incorrectly
expected WRITE_FILE to be owned by the research-budget guard. It now uses
AR_START, which that guard actually rejects; no unrelated policy was changed.
The unsuccessful attempt is retained here rather than counted as a passing run.

The end-to-end test uses the real Minime decision/execution path and shared Rust
reader, synthetic source, and stubbed provider responses. It performs source
study -> authored INTROSPECT -> reflection -> authored CONTINUE -> next source
page while the unrelated experiment stays active. It verifies exact next-page
position, retained notes and the separate introspection artifact. A denied
executor gate still prevents generation. Default pytest live-write/network
guards remain enabled throughout.

Reproduce the Python suite from the isolated Minime worktree:

```sh
ASTRID_SOURCE_STUDY_BIN=/Users/v/other/worktrees/reflection-admission-20261001/astrid/target/debug/astrid-source-study python3 -m pytest -q tests --tb=short
```

## Next Release Boundary

Reconcile both candidates against current main and mapped live identities, then
qualify the immutable paired helper/agent/bridge release with existing migration
and bridge suites. A graceful activation must use sanctioned wrappers and
cooperative preflight, preserving pending choices and checkpoint continuity.
No full bridge suite or packaged live migration qualification was run in this
implementation pass. Stop on foreign activity, drift or failed readiness.
Do not restart the engine/model/visual/sensory processes for this repair.
Leave automations paused. Observe public use naturally; do not ask Minime to
confirm that the repair helped or infer felt improvement from test success.

## Approved Release Qualification

Mike subsequently approved deployment and merge. Codex claimed pause generation
488 with no active lease; both canonical main trees were clean and matched the
remote baseline identities above. This does not resume the paused automations.

The paired launch overlay omitted `minime_autonomy/authority.py`, the reviewed
reflection-admission implementation. The exact path is now included with a
synthetic snapshot/no-canonical-write regression. No other admission gate or
deployment preflight was relaxed. Release qualification and activation results
will be recorded below; the earlier implementation-only status is historical.

The reload wrapper requires the exact interactive actor `codex-astra-interactive`;
the maintenance pause was renewed under that identity as generation 489 before
any service transition. The previous pause was not resumed. Source reconciliation
froze all 90 selected agent inputs in
`/Users/v/other/worktrees/reflection-admission-20261001/launch-reconciliation-01/`.
The packet is evidence, not activation authority.

Fresh qualification: 56 paired handoff/reconciliation/restart/inventory/legacy
qualifier tests and 176 bridge deployment/controller/projector/evidence tests pass.
The full bridge library passes 2,350 tests with one ignored. Shared-reader strict
Clippy, formatting and the domain-boundary audit pass. Same-schema executable
qualification is `scripts/qualify_reflection_release.py`; it deliberately does not
reuse an earlier schema-migration script with incompatible version assumptions.

## Completed Qualification and Rollout

The full bridge suite passed, including the compile-fail authority tests and
public-facade compilation tests, with one ignored library test. The complete
shared-reader suite passed again. Minime's full suite against the immutable
release helper passed: 1,742 tests, one skipped, 141 subtests (69.06 seconds).
The staged focused Minime rerun passed 61 tests. These are interface and safety
checks, not evidence of felt improvement.

Two unsuccessful qualifier runs are retained at `paired-qualification-01` and
`paired-qualification-02` under the release root below. The first incorrectly
expected the entire page presentation to remain identical after a newly
delivered reflection. Only the previous-response-choice footer had changed;
the page identity, source and notebook were unchanged. The second submitted the
older presentation instead of the re-presented request and correctly encountered
the reader's exact-delivery refusal. The qualifier now asserts unchanged page
and source/notebook content separately, verifies the updated choice provenance,
and submits the actual presented request. No production guard was relaxed.

The successful actual old/new helper run has 40 checks across Astrid and Minime:
exact pending public/private input at transition, current schema 12 retained,
reflection without source credit, disclosed MAP/LIST correction, unchanged
questions/notes/bookmarks, explicit source/private continuation, same-schema old
reader compatibility, and the actual Minime adapter's selected release helper.
All state and prose in that qualification are synthetic.

Release root: `/Users/v/other/worktrees/reflection-admission-20261001`.

- Stage: `bridge-stage-01`, built from clean commit `363e4e9923` using
  `bash scripts/build_bridge.sh --stage-dir ...`.
- Bridge binary SHA-256:
  `a85684245583f0932ab3b6df6bd746d6f4f16dd596827855c49ecd9f1716cfa0`.
- Release manifest SHA-256:
  `dc606f5e6185a7ce7e3d11979c372a83cf844cbe093a90c4e272ed212047b129`.
- Shared helper SHA-256:
  `b92197bfec73b67ddf1becff2aab3101cfebbb662bf05eb384d844e439d20cf3`.
- Successful fixture receipt: `paired-qualification-03/qualification.json`,
  SHA-256 `c4052453e5a3dc202dc41bc53256377335fd541cdc3588546744548ef077fcee`.
- Paired activation: `paired-handoff-01.jsonl`, SHA-256
  `582421cf681372411b65a68cd532e7086205c6556fedcb35632c5f229c14f4c5`.
  The sanctioned wrapper installed only the reviewed overlay, preserved source
  backups in `paired-handoff-01.source-before`, waited the full 185-second source
  quiet interval and then waited for completed work and an observed idle boundary.
- Minime PID `17757` -> `52572`; fresh source status matches all 90 selected
  launch inputs, with `reload_required=false`. No new interrupted job was found.
  The wrapper does not claim an atomic traffic barrier for Minime.
- Bridge PID `19729` -> `53229`; start `2026-10-01 21:22:04 PDT`.
  Transaction: `/Users/v/other/astrid/.runtime/bridge-deployment/transactions/5424596016534d4d939742072bfbd213`.
  Its `receipt.json` SHA-256 is
  `6fddaf7c21607639c793b9d78aec37b4f8f451b6edf2516d21934efed24329de`.
  Status `activated_verified`, drain `drained`, force false, legacy transition false.
  Remote delivery confirmation remains false; no stronger lossless claim is made.
- Checkpoint SHA-256
  `3b4668094e04fdcca4834a54e4645a2d3b9444e1b8943de0081c591d12183ba1`
  was transferred and decoded. Exchanges progressed from 212206 to 212207;
  self-control lineage and 62 pending runtime-feedback items were verified.
- Engine PID `35303`, model PID `3893`, and the gateway, supervisor, visual,
  camera, microphone, host-sensory and feeder process/start identities remained
  unchanged. No engine/model/sensory settings were changed.
- `check_bridge_deployed.py` confirms the selected stage is the running process.
  Readiness is true. `capture_stack_receipt.sh` returned passed receipt
  `env_receipt_1790915068778_494000`. Deployment holds are absent.

Post-integration comparison confirms 665 recorded bridge source files agree
with canonical main and the Minime adapter selects the exact packaged helper
above. The later qualifier-only and documentation commits do not change those
runtime inputs. Preserve the active release and source worktree; do not archive
them while the launch selection still references them.

Both implementations were integrated by fast-forward. The three Minime files
installed by the wrapper were byte-identical to its committed candidate before
explicit staging and fast-forward; no foreign edits were included. Other dirty
worktrees and historical evidence were left alone. Nothing was pushed.

This release makes a chosen bare reflection executable without an unrelated
research budget. It does not force reflection, change an experiment's status,
grant live-control authority, or guarantee longer writing or improved experience.
No prompt requesting confirmation was sent. Natural public uptake remains to
be observed, without reopening the paused automation.
