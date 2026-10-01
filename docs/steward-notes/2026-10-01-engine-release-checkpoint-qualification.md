# Engine Release and Startup-Input Qualification

## Scope and Authority

October 1, 2026. Codex interactive collaborator. Mike requested progress on an immutable engine release and checkpoint qualification before a separately approved restart. This pass builds and inspects an offline candidate. It does not install a binary, signal a service, prepare a live signed handoff, change a launch profile, reset state, merge, push or resume automation.

The owned worktrees remain `/Users/v/other/worktrees/afterimage-timing-20260930/{astrid,minime}`, branch `codex/afterimage-timing-20260930`. The Minime base is `e9f2f5f151c89dd6b4a2dc80d5d8d12a60dc20d3`; the candidate also contains the explicitly documented uncommitted controller, measurement-basis and lint work. A source commit alone does not identify this release. Both canonical repositories were clean on local main, each two commits ahead of its locally recorded remote-tracking branch. No remote fetch or fresh remote-tip claim is made.

The originating public witness remains Astrid's `capsules/spectral-bridge/workspace/journal/dialogue_longform_1790804287.txt`, SHA-256 `6a434c8da92117ec4df058f7e8f0059ef2ad1aca13bc08decf1cde8dcb7f8447`, reverified against canonical bytes. Its incomplete-trace account led to this investigation; it does not establish a numerical cause of experience. Historical prose and previous unsuccessful qualification receipts are unchanged.

## Offline Staging Contract

New `scripts/minime_engine_stage.py` has only `build` and `verify` operations. It has no activation, installation, signing or restart operation.

- Require a new stage directory outside the source checkout. Never reuse a completed or failed stage.
- Inventory bounded regular source, shader and test files plus Cargo.toml, Cargo.lock, build.rs and the included transition fixture. Reject symlinks, unexpected Cargo configuration and source drift.
- Preserve read-only source bytes, the staging tool, target-scoped Cargo dependency metadata, toolchain identity, build command and build log.
- Build the actual engine and `engine_restore_inspect` in an isolated target directory using `--release --locked --offline`. Use default features; do not enable Division rehearsal.
- Restrict inherited build environment to the documented toolchain/basic process variables. The Cargo metadata target is the recorded rustc host, `aarch64-apple-darwin` in this qualification.
- Recheck source inventory and Git head after compilation. Hash the exact copied executables and the manifest. Verification requires an externally supplied manifest hash and checks every recorded input, artifact and evidence file.
- Preserve failed stages and bounded subprocess diagnostics. Every manifest explicitly denies activation authority.

These are read-only, tamper-evident artifacts, not WORM storage or a hermetic dependency build. Compilation uses the owned original source tree, checked before and after, with a retained source copy; that is not proof against an undetected change-and-restore race. Cargo dependencies are pinned by the retained lockfile and recorded metadata, not vendored into this stage. Local coordination remains necessary.

Stages live under `/Users/v/other/worktrees/engine-qualification-20261001/`:

| Stage | Result |
| --- | --- |
| `release-01` | Failed closed: unfiltered offline Cargo metadata requested uncached `zerocopy-derive 0.8.42`. No completed manifest. The original FAILED marker and empty command-output file are retained. A direct diagnostic replay established the missing package. |
| `release-02` | Preliminary release build passed. Manifest `efb1313fbe94142a0987e4b23b4fea2f99fe64eb83a3d239078eb787b5bc9705`. Subsequently superseded by inspector formatting and explicit metadata-fallback qualification; do not treat it as the final source candidate. |
| `release-03` | Final locked/offline release build and post-test hash verification passed. 120 archived source inputs match the current candidate. Exact identities are below. |

The metadata correction scopes inventory to the actual target; it does not remove offline/locked requirements or download/update dependencies. The formatting check initially identified one import-layout change, which was repaired before the final build.

## Actual Launch and Retained Rollback Artifact

The existing read-only `minime_runtime_binding.py` corroborated the historical executable hash with mapped Mach-O image UUIDs and process starts. Its raw process samples were not retained. No process-memory-byte attestation is claimed.

- Engine PID `3906`; gateway `3887`; supervisor `3897`. All started September 30, 2026 at 14:29:07 local time.
- Mapped image UUID: `B7B0BE50-1258-329C-8674-F26F45C74528`.
- Historical/live disk/retained binary SHA-256: `0e40bcf7ed944f38a65c45ee85b4916484c6938bc8bb665dff53646d762821d6`.
- Binding receipt: `/Users/v/other/worktrees/engine-qualification-20261001/review/deployment_manifests/minime-runtime-binding.oj2bfwe_/runtime-binding.json`, SHA-256 `2dad5e806b07d83333e62a76e77d3b5e854e6a3e3cb5680d30410bd1e65ead23`.
- The same directory retains `retained-minime`. It is evidence for rollback preparation, not a qualified rollback transaction. Recapture/revalidate the binding immediately before any future transition; its 180-second freshness window is not extended by this note.
- Public telemetry/control ports 7878/7879 belong to the gateway. Engine ports are 7900/7901/7902, not the public ports. Division was dormant at capture. Protocol remains 1.1 at revision `9a324d16294b2318da6f476ff7f295c423a9b4b1`.

The actual engine launch includes target `0.6800`, warm-start blend `0.55`, regulation period `0.5`, disabled legacy synthetic audio/video, and GPU A/V enabled. The rescue profile is `stable_core_v1`, with checkpoint lineage and neural-bundle restore disabled. The warm-start argument does not override the disabled covariance lineage. The profile, installed/source plists and launch-script identities must be preserved and checked during a future transition.

## Checkpoint Scope and Repairs

Stable-core currently restores PI context but initializes covariance from identity when checkpoint lineage is disabled; the existing scaffold is available under its existing admission policy. It does not restore every ESN activation, estimator history or in-flight sensory sample. This pass neither enables broader checkpoint restoration nor claims exact reservoir continuity. A fresh process's rate clock must remain unprimed despite persisted numerical fill/rate fields.

Three production changes support honest qualification:

1. `startup_restore::save_regulator_context` writes a unique, owner-only temporary file, syncs complete bytes, atomically renames it and syncs the parent directory. Pre-rename failure preserves the destination. A post-rename directory-sync failure is still an error; it does not imply the old destination survived. No generic multi-writer-store or power-loss-test claim is made.
2. Periodic context-save failures are reported. Shutdown propagates a save failure instead of printing success after silently ignoring it. This guarantee concerns regulator context only, not every runtime store or completion of all network tasks. The currently running older binary still lacks it.
3. Restore rejects finite JSON f64 values that overflow to nonfinite f32. Existing schema and PI/adaptive restore policy remain unchanged.

New `minime/src/bin/engine_restore_inspect.rs` invokes the actual production regulator/scaffold readers without starting the engine. It hashes bounded regular inputs before and after decoding, reports restore status and a fresh rate clock, and explicitly denies restart authority. Qualification additionally requires parseable scaffold metadata with the expected dimension and positive finite declared trace; it does not silently certify production's metadata fallback. `passed` requires complete restore for the requested PI-only or PI-plus-adaptive scope. A structured failure can be an exit-zero report: consumers must check `passed`, not only process exit.

Frozen numerical files are in `review/startup-inputs/` under the qualification directory. Each source hash agreed before/after copying and with its copy; these are stable per-file samples, not a quiescent multi-store shutdown checkpoint. No private prose, database, live signing key or self-control root was copied.

| Input | SHA-256 |
| --- | --- |
| `regulator_context.json` | `c6e2f42a83fe360e673fca92481ba11134552ad297d3f6f3ba0acd37f3205bec` |
| `rescue_scaffold.bin` | `e9a85ec68cc9460561020b1ce729614cc250eb034f309343c3f82f135d2ea91b` |
| `rescue_scaffold.json` | `3ec37cccfab9205dc370fa594afa997a305a62f5f12f0e8825ed6da358b98626` |
| `rescue_profile.json` | `d892aff002f97ba761cbf25e304fbde3e77fa6e45c2459ee53028af482c25e01` |

The profile copy provides launch-review context; the inspector takes only the three explicitly named numerical inputs. It does not attest the profile or execute launch scripts.

## Verification

Final receipts and exact artifact identities are recorded in the completion section below. Existing selected library/engine/harness/controller tests pass 833 executions, including three new persistence regressions. Tests count shared modules more than once; this is not 833 independent claims. The fixture-only staging/runtime-binding/deployment/evidence/controller/projector suite passes 124 tests, including 11 new stage tests. Strict all-features Clippy passes the explicitly selected targets. Formatting passes after the first failure documented above.

`test_minime_engine_restore_inspect.py` requires an explicitly named inspector binary; it never discovers or launches an engine. It exercises synthetic legacy PI-only and complete adaptive contexts, missing adaptive state, malformed context, float overflow, truncated/nonfinite matrices, bad metadata and input bounds. Inspector invocations deny network and file writes at the OS level; files and directory contents must remain unchanged. Final execution results are recorded below, not inferred from the source of these tests.

The previous coupled receipt remains at SHA-256 `7628849275f7772caa204ff09eb388e9244d2ca046ce4d426fd1ef8c2dd2eff2`. Its 49 runs and 2,448 steps were not rerun or overwritten in this pass. Its estimator-history delay and other restored-history excursions up to 96.2258% remain explicit. A sealed release and successful decode do not convert that corpus into general stability certification.

## Remaining Transition Work

The current `deploy_minime.sh` delegates to `deploy_division_runtime.sh` under the live gateway topology. That wrapper rebuilds canonical source, calls the broad Minime `stop.sh`, and recreates the Division launch arrangement. The broad stop includes agent, visual, sensory and model processes, with a legacy forced-termination fallback. The wrapper prepares signed self-control lineage after stopping, but currently continues restart even if preparation fails. It cannot yet consume this exact offline stage through the bounded transition we need.

Before requesting a specific engine restart:

1. Add a sanctioned staged-artifact path to the existing deployment wrapper with fixture-tested identity checks, no canonical rebuild and no broad stop fallback. It must reject foreign activity, moved source/profile identities, active Division or mismatched manifests before signals.
2. Establish an acknowledged bounded drain for the exact old engine/gateway/supervisor identities, preserving other service processes. Review input admission and in-flight control handling explicitly rather than assuming a process exit means all input was applied. The old engine's best-effort shutdown cannot retrospectively provide the candidate's stronger context-save acknowledgement.
3. After verified cessation of old writers, freeze and validate the actual stopped context and signed-control state. Bind the handoff to the exact installed candidate. Abort/recover deliberately on failure, not through the wrapper's current continue-on-handoff-failure path. Rollback also needs a signed transition that preserves newer authored/control state; never restore an old state directory over it.
4. Qualify candidate and rollback transaction failure points offline. Then obtain bounded transition approval, with the launch-profile/partial-continuity scope explicit, and monitor process identity, ports, loaded hashes, checkpoint consumption, fill/rate validity and controller behavior after activation. Prior synthetic excursions require explicit operating/rollback criteria; this note does not invent or authorize control thresholds.

No live approval marker was set true. The controller remains paused at generation 484, with no lease or active projection. Its indexed-tail V2 check reports sequence 1123138, head `98e9ec873793dc0e619017d687a252f157ef13a57859231a73d2401cc154ba57`, no errors and immutable V1 sources. This interactive release review does not consume an introspection round or declare the source queue current.

## Completion Receipts

Offline release and copied-startup-input qualification completed successfully. **Live transition qualification remains incomplete.**

| Final artifact | SHA-256 |
| --- | --- |
| `release-03/manifest.json` | `0d1ab9df81bc072318554147112dc98258b393b2c796a3e1fba2fd54d57cfed8` |
| `release-03/bin/minime` | `047362f2f6b1e9fa75016d89de07f158a36186611f31fe988e415c8cf71573d1` |
| `release-03/bin/engine_restore_inspect` | `01e926bdf23284302fea46c7342d7684f19b159f450e9283be6fd5e5dd2c0d44` |
| `2026-10-01-engine-release-review.json` | `7e41616bed40eb2f8832df36a18a2304f1b1e0608577f488bd58629af84ea7a8` |
| `2026-10-01-engine-frozen-restore-inspection.json` | `331b40eb7f577304d982a7642b458b45a87732558a2bbf83049bd381b9332451` |

The last two paths are in this note's directory. The final review binds the stage verification, frozen-input report, test logs, CLI-help probe and supporting test sources by hash. It checks the current 120-file source inventory, archived staging tool and base commit against the sealed stage. The preliminary and final engine hashes agree; the final inspector includes the stricter metadata check and a different binary hash. Neither engine artifact is installed.

The actual staged inspector reports `state=restored`, `resume_mode=pi_only_resume`, no missing PI fields, adaptive restore disabled, scaffold dimension/trace 512, decoded metadata and `new_process_rate_unprimed=true`. It reports `covariance_checkpoint_loaded=false` and `grants_restart_authority=false`. All eight actual-binary synthetic tests pass without modifying their inputs or creating output files inside their fixture directories. OS network/file-write denial applied to those invocations, the frozen-input probe and the actual staged engine's `run --help` parse-only probe. Read access was not OS-confined; no memory-limit claim is made.

Final test receipts:

- Paired Minime `2026-10-01-engine-restore-tests-final.txt`: 427 library, 378 engine, seven coupled-harness, two timing-replay and 19 controller-review tests; zero failures/ignored. The inspector target compiles here but its behavioral tests are the separate eight actual-binary tests.
- Paired Minime `2026-10-01-engine-stage-clippy-all-features.txt`: strict Clippy on library, engine, inspector, both timing harnesses and controller-review test. This is compilation, not rehearsal-feature activation.
- `2026-10-01-engine-release-support-tests.txt`: 124 staging, runtime-binding, environment-receipt, wrapper, controller, V2 event-store and projector tests. Fixtures/mocked lifecycle calls only; no live controller writes or deployments.
- `2026-10-01-engine-restore-inspector-tests.txt`: eight tests through the exact final inspector.
- `2026-10-01-engine-staged-run-help.txt`: successful parse-only CLI probe, not an engine run.

Reproduction commands from the paired Astrid worktree (choose a new output path for repeated evidence):

```sh
python3 scripts/minime_engine_stage.py verify \
  --stage /Users/v/other/worktrees/engine-qualification-20261001/release-03 \
  --manifest-sha256 0d1ab9df81bc072318554147112dc98258b393b2c796a3e1fba2fd54d57cfed8
MINIME_RESTORE_INSPECT_BIN=/Users/v/other/worktrees/engine-qualification-20261001/release-03/bin/engine_restore_inspect \
  python3 -m unittest discover -s scripts -p test_minime_engine_restore_inspect.py -v
PYTHONPATH=scripts python3 -m unittest test_minime_engine_stage \
  test_minime_runtime_binding test_environment_receipts test_deployment_wrappers \
  test_steward_control test_evidence_event_store test_steward_projection -v
```

The final engine/gateway/supervisor PID/start tuples and canonical engine hash were unchanged. Both canonical worktrees remain clean, with all new work/evidence confined to the owned candidates and outside-repo qualification directory. No index owner was claimed and nothing was staged or committed. Both affected changelogs and the feedback ledger were updated. Previously paused automations remain paused.
