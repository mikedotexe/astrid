# Proportionate Release Preflight

## Status and Scope

Integrated and pushed as Astrid `1ef40356b64e521fde0c9cf3c9b4a30fc6bed1f0`,
developed on `codex/scoped-release-preflight-20261022` from
`054f4ff335f197c3f2921b995f82121a0eb4c00a`. The qualification sections below
retain the pre-integration observations. No service transition or new release
build was performed; no policy grant or automation resume is included.

Mike requested an autonomous, serious review of avoidable go-live friction while
he investigates Brave separately. This is operator-process feedback, not a claim
about either being's experience. No private writing or live inference was used.

## Why This Tranche

The prior rollout note, `2026-10-22-freer-exploration-rollout.md`, records:

Source SHA-256: `4e5413e07bbb8d1f425311fd41c6b96256617184a8fb5a17c83346226ba4420e`.

- Stage 03 preflights refused edits aged 83.5, 127.9 and 174.8 seconds against a
  180-second window. Some repeated stages had unchanged bridge/helper bytes.
- A fresh rollout-note edit caused bridge activation preflight to refuse after
  Minime had stopped. The edit was ours and was not a component input.
- The installer then explicitly slept 185 seconds after its own verified writes.

The evidence supports removing those particular sources of delay. It does not
justify removing graceful drain, pending-work preservation or readiness checks.
Earlier qualification also found actual compatibility problems. Their failed
receipts and fixes remain valid history.

## Implemented

### Component-Scoped Activity

`scripts/deploy_preflight.py` now derives recent-edit evidence from the same
selected inputs used for dirty-source admission, including external dependencies.
Fresh unrelated documentation and other-component edits do not block this check.
The controller's general activity detector is unchanged.
The engine/provider CLI routes retain their legacy tree-activity policy. Their
input-closure review is separate: in particular, the provider's old input list
does not cover all of its local imports. Narrowing that route using the incomplete
list would be unsound. This change applies to the bridge and Minime-agent path.

The legacy `foreign_active` reason code remains for compatibility, but the
diagnostic identifies the actual recent paths and labels the actor unknown.
File age never established which agent wrote a file or that a writer was active.

The old component list omitted local capability/core/crypto/storage dependencies,
the shared `managed_dir.rs`, and build configuration paths. These are included.
A structured TOML regression checks the current in-repository dependency closure.
The scope is deliberately conservative; it is not a general dependency solver.
Immutable staging still verifies its fuller actual input snapshot.

Git status is checked for failure, NUL rename records are parsed correctly, and
untracked directories expand into actual file paths. Inspection failure is not
interpreted as clean source. Deleted/renamed inputs use the nearest surviving
directory's modification time as a conservative activity hint. Existing explicit
acknowledgement semantics for unversioned external dependencies are unchanged.

### Verified Installation Instead of Aging Our Writes

After the reviewed Minime overlay is completely installed, the existing installer
constructs an operation-local `ReviewedInstallation`. It binds:

- The full selected source inventory, not just changed files.
- Exact file bytes, canonical regular-file paths, modes and file identities.
- The existing owned launch hold, checked again on every validation.

Only that Minime installation can bypass the edit-age heuristic. It cannot bypass
dirty-source acknowledgement, affect another repository, suppress unexpected
inputs or tolerate source/hold drift. There is no new command-line override,
persisted exemption, TTL, standing approval or independent coordinator.

The fixed 185-second sleep is gone. A versioned `installed_sources_verified`
receipt records the selected inventory digest. The old receipt spelling remains
recoverable; the new receipt is also understood by unsignaled agent-only recovery.
The affected accepted-work drain helper is now included in the optional immutable
tool inventory; old frozen stages remain readable without rewriting their files.
Recovery revalidates its receipt, source and hold evidence rather than trusting a
saved exemption. Partial installs are not silently undone or replayed.

Real idle/drain waits, source reconciliation, old PID/start checks, accepted-work
completion, native NEXT sealing/restoration, protected-process checks and fresh
readiness remain. Removing this sleep saves that fixed delay on successful
installation, not a promised total deployment time.

## Verification

All tests use temporary sources and synthetic process backends. No live service
calls, Rust compilation, model loading or state migration are required for this
Python-only tooling change. Tests run with a minimal environment and `python3 -B`.

Focused regression coverage includes:

- Fresh unrelated notes versus fresh component/dependency inputs.
- Exact reviewed input admission, dirty acknowledgement and unreviewed changes.
- Git failures, nested untracked files, deletion, rename and literal arrow names.
- Canonical paths, symlinks, mode changes, equal-byte file replacement and added
  inventory entries.
- Source/hold drift after installation: no signal, no rollback, retained hold.
- No installer sleep; normal idle and busy-drain behavior stays tested separately.
- Old/new completion receipts, tampered identities, exact recovery retries and
  prevention of post-signal replay.
- Production backend wiring: the installation evidence is supplied only for
  Minime's source scope, never Astrid's tools.

The final selected 349 deployment, handoff, reconciliation, compatibility, controller,
projection and evidence tests pass. `domain_boundary_audit.py verify` reports
valid with zero violations, and `git diff --check` passes. No repository Python
linter configuration or `ruff` executable was found; Python imports and execution
are covered by the tests. Rust formatting/Clippy and model qualification were not
rerun for this Python-only change.

Reproduction from this worktree (test fixtures only):

```sh
env -i PATH=/opt/homebrew/bin:/usr/bin:/bin HOME=/Users/v TMPDIR=/tmp \
  PYTHONPATH=scripts MINIME_SOURCE_ROOT=/Users/v/other/minime \
  python3 -B -m unittest \
  deploy_preflight test_deploy_preflight_scope test_restart_minime_agent \
  test_paired_minime_handoff test_finish_minime_work test_deployment_wrappers \
  test_bridge_stage test_bridge_activate test_bridge_stopped_recovery \
  test_minime_next_handoff test_reconcile_minime_launch \
  test_research_release_compatibility test_steward_control \
  test_steward_projection test_evidence_event_store test_proposal_applier -q
python3 -B scripts/domain_boundary_audit.py verify
git diff --check
```

Preserved unsuccessful attempts: the initial 73-test run had three recovery-error
wording/order failures. Exact prior recovery validation order was restored rather
than relaxing acceptance. The next 113-test run had one new fixture error: a
global mocked `Path.read_bytes` changed the fixture's inventory while testing
plist reads. The fixture now pins its expected inventory independently. A later
235-test deployment/handoff run passed. These were candidate test runs, not failed
live transitions.

The first expanded 348-test run also exposed three compatibility fixtures that
construct the backend without `__init__`. Optional installation evidence now
defaults to absent, and the existing downgrade check remains before source/service
inspection. The subsequent full run passed; no downgrade refusal was weakened.

## A Simpler Go-Live Contract

This is the recommended direction, not a newly activated policy or additional
approval requirement:

1. Classify the change by affected services and state formats. Documentation-only
   work needs no service restart. Agent-only work should not require rebuilding
   or restarting an unchanged bridge. Schema transitions need preservation tests;
   provider/engine changes need their own relevant qualification.
2. Build each changed artifact once from a stable candidate. Bind binaries,
   dependencies, configuration and tests to their actual inputs. A note edit or
   changed deployment script should not require recompiling unchanged Rust code.
   Tool qualification and binary qualification are distinct evidence.
3. Reuse applicable test evidence by identity. Rerun the affected suites and a
   deployment smoke test, expanding coverage when a shared contract changes.
   Do not label unrelated historical debt as a new release failure; do not call
   affected, unverified behavior qualified.
4. Get one approval for the bounded service/state transition. Continue within
   that scope; ask again only for a material scope/risk change. Retries still
   require fresh source/process checks, not ceremonial repetition of approval.
5. Hold admission, finish accepted work, activate the selected artifacts and
   verify loaded identities and preserved pending work. Use the existing durable
   receipts to resume a known boundary. Do not invent a new bespoke recovery
   script for every interruption.
6. Report operational readiness separately from optional observation. A long
   passive watch is not a generic prerequisite for prompt or agent changes. Use
   explicit symptom-based observation where the change actually warrants it.

The second tranche below implements artifact reuse and scoped Rust test receipts.
Consolidating the successful stage/drain/verify sequence into a bounded resumable
operator command remains separate; no new orchestration framework is introduced.

## Second Tranche: Reuse Evidence by Identity

Implemented in the same isolated candidate after Mike explicitly requested this
next step. This does not activate a release, change approval scope, or authorize
an old test result for unrelated behavior.

### Binary Reuse

`build_bridge.sh --stage-dir NEW --reuse-from VERIFIED_STAGE` is an explicit
alternative to compiling. It runs the ordinary candidate preflight, validates
the donor, compares compiled inputs, copies the three Rust binaries into the new
stage, copies current runtime helpers, and creates a new source witness and
manifest. Native manifest verification still runs. No hard links, mutable live
binary, implicit latest-stage selection or fallback build is used. A mismatch
retains a failed attempt and asks for a new baseline; it does not quietly spend
another build's time and disk space.

New `bridge_staged_release_v5` packets include `compiled-inputs.json`, bound by
both their manifest and ready receipt. The full source/tool witness remains
separate and is still checked by activation. Older v1-v4 stages remain readable;
they cannot supply the newly required compilation evidence retroactively.
The next normal build records the first reusable baseline. Frozen old releases
are never modified to manufacture that baseline.

The compiled identity binds the fixed three Cargo build commands, native target,
absolute source/package paths, complete local package inputs, shared source,
workspace manifests/locks, inherited Cargo configuration, resolved third-party
package bytes, Rust/Cargo/native tool identities, SDK settings, host/platform and
a digest of the actual child environment. Environment values are not copied into
receipts. Shell bookkeeping is removed from the actual child environment before
hashing, not merely ignored in the witness. File mtimes remain in the broader
source witness but do not invalidate compiled content identity.

Named deployment/runtime scripts outside Cargo packages do not invalidate Rust
artifacts; their current bytes remain bound to the new packet. A tool inside a
package remains a conservative compiler input. Local build scripts retain a HEAD
binding because they may query Git. Otherwise, a tool/document-only commit can
reuse matching Rust inputs. This does not claim deterministic recompilation or
qualify arbitrary cross-worktree path remapping. Cargo/toolchain installations
remain trusted local build infrastructure, not an adversarial sandbox attestation.
Custom native-tool/search-path overrides or Cargo tool/environment customization
can be recorded by a normal build but are not eligible for this initial reuse
profile. They require a fresh build, not a bypass.

The new packet records `artifact_production.mode`, donor manifest identity and
preparation time, and `cargo_build_invocations`. Synthetic restaging verifies
three initial build commands versus zero on the reused path, with different
inodes and byte-identical Rust artifacts. Current helpers are copied independently.
This is a command-count result, not a measured production compilation speedup.

Example, with reviewed absolute paths supplied by the operator:

```sh
scripts/build_bridge.sh --stage-dir "$NEW_STAGE" --reuse-from "$VERIFIED_STAGE" \
  --actor codex-astra-interactive --ack "Restage matching binaries with reviewed current tools"
```

Use the same reviewed build environment for the original and reused packet.
All inherited non-bookkeeping values bind reuse conservatively; a stable,
minimal launch environment prevents irrelevant shell-variable changes from
creating a mismatch. No deployment approval is implied by `--ack`.

### Relevant Test Results

`scripts/bridge_test_receipts.py` provides two fixed suites: `source-study`
(including the shared writer) and `public-web`. It executes their ordinary full
Cargo test commands, or explicitly reuses one completed input-matching receipt.
The source/dependency inventory is conservatively the bridge bundle's inventory,
so unrelated Rust changes may still invalidate these receipts. This first version
does not attempt a fine-grained per-test impact analyzer.

Test inputs bind the exact test command, default features, native target, explicit
owned target directory, 1,800-second timeout, harness identity and source/tool
identity. Test children receive a small recorded environment, with
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0` and
`CARGO_INCREMENTAL=0`, excluding live endpoints and opt-in output destinations.
A cache-budget audit precedes actual test compilation. Reuse does not compile and
therefore does not require fresh build headroom. Targets inside immutable release
packets or package inputs are refused.

Receipts retain input and stdout/stderr hashes, actual execution time, terminal
status, operation duration and donor identity. Reuse keeps the original execution
timestamp and duration and says `mode: reused`; it never claims a new test run.
Failed, interrupted, tampered or mismatched evidence is not reusable. Timeouts
terminate the owned process group and retain partial logs as failure evidence.

```sh
# First execution, using an explicitly owned cache outside immutable releases.
python3 -B scripts/bridge_test_receipts.py --source-root "$SOURCE" \
  --target-dir "$OWNED_TEST_CACHE" --receipt-dir "$NEW_TEST_RECEIPT" \
  --suite source-study

# Later exact-input reuse; no automatic fallback execution on a mismatch.
python3 -B scripts/bridge_test_receipts.py --source-root "$SOURCE" \
  --target-dir "$OWNED_TEST_CACHE" --receipt-dir "$REUSED_TEST_RECEIPT" \
  --suite source-study --reuse-from "$VERIFIED_TEST_RECEIPT"
```

Repeat with `--suite public-web` for that separate suite. These receipts can be
referenced in the release qualification packet; they do not turn a historical
paired qualification into a current one. Bridge tests invoking Python helpers,
the full Minime Python suite, packaging/import checks, old/new authored-state
preservation, current drain and readiness must still be checked for the new
packet. No live observations, model measurements or migration results are cached.

### Second-Tranche Qualification

Synthetic tests exercise actual staging and receipt orchestration with fake
Cargo and temporary artifacts, including tool-only restaging, source/mode/config
and dependency changes, old-stage readability, unsupported reuse, symlinks,
corruption during copy, failed donor records, source drift, honest test-profile
identity, retained output, shell argument boundaries and no implicit recompilation.
A small real subprocess test checks timeout termination; no model or service is
involved.

The read-only real inventory check initially failed in the isolated worktree
because its sibling `prime_esn_wasm` source is not populated. No dependency or
live tree was rewritten to bypass that. Running the same identity reader against
the canonical source layout identified 10 local packages, 785 local input files
and 407 resolved third-party packages, producing a roughly 308 KB witness in
5.97 seconds. This is inventory timing, not build/test execution timing. The
initial SDK-version subprocess failed despite readable SDK settings; the extra
lookup was removed and the actual settings file is bound directly instead.
This does not diagnose or repair the machine's Xcode installation.

Selected tooling qualification passed at 408 tests, then 413 after additional
copy-corruption, dependency-closure and receipt tests. Final qualification passes
416 tests in 35.69 seconds, including the preflight/handoff/controller/evidence
regressions and new reuse cases. Domain-boundary verification has zero violations;
`git diff --check` and wrapper Bash syntax checks pass. No Rust suite was rerun through the new runner
in this tranche, and no existing Rust result was retroactively blessed. A normal
future baseline build/test execution will create the first real reusable records.

A final real-input check caught an overly broad configuration refusal in the
candidate: the repo's Linux-only linker flags were being treated as active on
macOS. Exact target configuration is now scoped to the selected target;
unresolved `cfg(...)` tool overrides remain unqualified rather than guessed.
The regression covers both inactive and active target settings. The final
canonical-layout inventory has no reuse limitations, takes 5.83 seconds, and has
identity `67e80b97388670fe289684199da5c84f8d7e3f2020c2f04922a7ef3e34bcb4fa`
(308,098 encoded bytes). That identity includes this candidate's implementation,
the inspected local environment and the canonical source layout; it is not a
newly compiled release or an activation witness.

At closeout, canonical Astrid and Minime remain clean. No Cargo target directory
was created in this worktree, and its existing Python bytecode cache remains
16 KiB. Controller generation 534 remains paused, with no lease/projection and
unchanged evidence sequence 1123188. No cache deletion or service operation was
needed. Rust formatting/Clippy are not rerun for these Python/shell-only edits.

Expanded reproduction adds these modules to the earlier command:

```text
test_bridge_reuse test_bridge_test_receipts test_bridge_release_launch
test_environment_receipts test_qualify_retained_accounts_release
```

## Integration Boundary

Mike approved coordinated explicit-path Git integration of this tooling. Updating
deployment tooling does not itself require restarting the beings. When using it for a later release,
the selected immutable packet must bind the updated tool versions; existing
frozen releases and rollback assets must not be edited in place.

No larger-capacity, Brave, MLX, provider, reservoir, sensory or being-prompt work is
included. Paused automations stay paused. No substantial build cache was created.
Both Cargo target directories are absent; the test worktree has 16 KiB of Python
bytecode cache. The controller remained paused at generation 534 with no lease or
active projection, and its evidence head stayed at sequence 1123188.

### Integration Preflight

The canonical trees were clean and on `main`, matching their freshly queried
`origin/main` tips: Astrid `054f4ff335f197c3f2921b995f82121a0eb4c00a` and Minime
`73f1e390565c52f83c77fd4ffd31d38103c3bb33`. The MLX coordination chat was idle
and its final handoff assigned remaining Git integration to this chat. Controller
pause generation 535 records `codex-astra-interactive` ownership of Astrid's
index for this explicit-path commit, fast-forward integration and push; there was
no active lease or projection. The pause is coordination evidence, not deployment
authority.

Only the reviewed deployment tools, their tests, this note and the changelog are
included. Minime requires no commit. The separate search-delivery and unfinished
capacity worktrees remain untouched. This integration does not refresh any frozen
release, change a launcher, restart a service or resume an automation.

The integration rerun passed all 416 selected tooling tests in 35.250 seconds.
The read-only domain audit again reports zero violations, and `git diff --check`
passes. This is a fresh Python/shell tooling check, not a new Rust compilation or
an assertion that a live rollout occurred.

### Integration Result

The reviewed 19-path tooling commit is
`1ef40356b64e521fde0c9cf3c9b4a30fc6bed1f0`. Canonical Astrid `main` fast-forwarded
from the inspected base without conflict. The same 416-test selection passed
again from canonical `main` in 32.644 seconds, and the committed diff passed
the whitespace check. A normal, non-forced `git push origin main` succeeded;
an independent `git ls-remote` returned the exact implementation commit.

Canonical Minime remains clean at
`73f1e390565c52f83c77fd4ffd31d38103c3bb33`; it needed no commit or push. The
implementation worktree is clean and retained, as are unrelated search, capacity
and historical worktrees. No Cargo targets were created in the implementation
worktree; its pre-existing 16 KiB Python cache was left alone. The volume reports
389 GiB available; no cleanup or space-reclamation claim is made.

The integration adds no launch, stop, signal, state migration, model request or
change to immutable current/rollback releases. Steward control remains paused at
generation 535 with no lease or projection. The next ordinary qualified build
can establish the first production v5 reuse baseline; historical stages and test
logs have not been relabelled as reusable evidence.
